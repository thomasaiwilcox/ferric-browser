#include "../../src/gui_application.h"
#include <QApplication>
#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QFont>
#include <QGuiApplication>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QKeyEvent>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQmlExpression>
#include <QQuickItem>
#include <QQuickWindow>
#include <QTcpServer>
#include <QTcpSocket>
#include <QTemporaryDir>
#include <QtTest>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include <fcntl.h>
#include <memory>
#include <sqlite3.h>
#include <sys/ioctl.h>
#include <sys/stat.h>
#include <unistd.h>

extern "C" bool cxx_qt_init_crate_ferric_browser_engine_qt();
#include "../../src/url_display.h"

class SwitcherTests final : public QObject {
    Q_OBJECT
    QTemporaryDir directory;
    std::unique_ptr<QQmlApplicationEngine> engine;
    QQuickWindow *window = nullptr;
    QTcpServer server;
    QString configPath, contextsPath;
    QVariant evaluate(const QString &source) {
        QQmlExpression expression(engine->rootContext(), window, source);
        auto result = expression.evaluate();
        if (expression.hasError())
            qFatal("Probe QML: %s", qPrintable(expression.error().toString()));
        return result;
    }
    static QString quoted(const QString &value) {
        auto bytes = QJsonDocument(QJsonArray{value}).toJson(QJsonDocument::Compact);
        return QString::fromUtf8(bytes.mid(1, bytes.size() - 2));
    }
    static void write(const QString &path, const QByteArray &contents) {
        QFile file(path);
        QVERIFY(file.open(QIODevice::WriteOnly | QIODevice::Truncate));
        QCOMPARE(file.write(contents), contents.size());
        file.close();
    }
    void submit(const QString &command) {
        window->requestActivate();
        QTRY_VERIFY(window->isActive());
        evaluate("testWindow.browserUi.enter_command()");
        auto input = window->findChild<QQuickItem *>("commandLineInput");
        QVERIFY(input);
        QVERIFY(input->setProperty("text", command));
        input->forceActiveFocus();
        QTRY_VERIFY(input->hasActiveFocus());
        QTest::keyClick(window, Qt::Key_Return);
    }
    void launch(const QString &contexts) {
        const auto base = QFileInfo(configPath).absolutePath();
        const QString config = R"JSON({"schema_version":3,"ui":{"font_size_pt":12}})JSON";
        engine = std::make_unique<QQmlApplicationEngine>();
        engine->setInitialProperties({{"temporaryProfile", false},
                                      {"storageBasePath", base + "/data"},
                                      {"softwareRendering", true},
                                      {"safeMode", false},
                                      {"userscriptsOff", true},
                                      {"startupConfigJson", config},
                                      {"startupConfigBaseJson", config},
                                      {"startupConfigPath", configPath},
                                      {"startupCliOverridesJson", "{\"schema_version\":3}"},
                                      {"startupProfileOverridesJson", "{\"schema_version\":3}"},
                                      {"startupContextsJson", contexts},
                                      {"startupUrl", "rb://blank"},
                                      {"width", 900},
                                      {"height", 700}});
        engine->load(QUrl("qrc:/qt/qml/io/github/ferricbrowser/qml/Main.qml"));
        QCOMPARE(engine->rootObjects().size(), 1);
        window = qobject_cast<QQuickWindow *>(engine->rootObjects().first());
        QVERIFY(window);
        engine->rootContext()->setContextProperty("testWindow", window);
        QTRY_VERIFY_WITH_TIMEOUT(
            evaluate(
                "testWindow.profileBootstrapComplete && "
                "!testWindow.browserUi.profile_bootstrap_pending && !!testWindow.activeWebView()")
                .toBool(),
            15000);
        window->show();
        window->requestActivate();
        QTRY_VERIFY(window->isActive());
    }
    void closeWindow() {
        if (engine && window) {
            evaluate("testWindow.browserWindowRegistry.slice(1).forEach(function(entry){entry.host."
                     "windowShutdownApproved=true; entry.host.close(); entry.host.destroy()})");
            QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
        }
        engine.reset();
        window = nullptr;
        QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
        QCoreApplication::processEvents();
    }
    static QQuickItem *visual(QQuickItem *parent, const QString &name) {
        if (parent->objectName() == name && parent->isVisible())
            return parent;
        for (auto child : parent->childItems())
            if (auto found = visual(child, name))
                return found;
        return nullptr;
    }
    QQuickWindow *createChild(const QString &path, bool privateProfile) {
        const auto before = evaluate("testWindow.browserWindowRegistry.length").toInt();
        const auto url = QString("http://127.0.0.1:%1/").arg(server.serverPort()) + path;
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:" +
                 quoted(url) + ",windowProfileName:" + quoted(path) + ",windowPrivateProfile:" +
                 (privateProfile ? "true" : "false") + ",windowEphemeralProfile:false})");
        QElapsedTimer timer;
        timer.start();
        while (evaluate("testWindow.browserWindowRegistry.length").toInt() != before + 1 &&
               timer.elapsed() < 10000)
            QTest::qWait(5);
        if (evaluate("testWindow.browserWindowRegistry.length").toInt() != before + 1)
            qFatal("Owned child did not register");
        const auto entry = "testWindow.browserWindowRegistry[" + QString::number(before) + "]";
        timer.restart();
        while (!evaluate(entry + ".host.profileBootstrapComplete && !" + entry +
                         ".ui.profile_bootstrap_pending && " + entry +
                         ".ui.current_url===" + quoted(url))
                    .toBool() &&
               timer.elapsed() < 15000)
            QTest::qWait(5);
        if (!evaluate(entry + ".host.profileBootstrapComplete && !" + entry +
                      ".ui.profile_bootstrap_pending")
                 .toBool())
            qFatal("Owned child did not open");
        return qobject_cast<QQuickWindow *>(evaluate(entry + ".host").value<QObject *>());
    }
    void submitAt(QQuickWindow *host, const QString &ui, const QString &command) {
        host->requestActivate();
        QTRY_VERIFY(host->isActive());
        evaluate(ui + ".enter_command()");
        auto field = visual(host->contentItem(), "commandLineInput");
        QVERIFY(field);
        QVERIFY(field->setProperty("text", command));
        field->forceActiveFocus();
        QTRY_VERIFY(field->hasActiveFocus());
        QTest::keyClick(host, Qt::Key_Return);
    }
  private slots:
    void init() {
        QVERIFY(directory.isValid());
        auto base = directory.filePath(QString::fromLatin1(QTest::currentTestFunction()) + "/" +
                                       QString::fromLatin1(QTest::currentDataTag()));
        QVERIFY(QDir().mkpath(base));
        write(base + "/.ferric-owned-fixture", "Owned Switcher privacy regression only.\n");
        configPath = base + "/config.toml";
        contextsPath = base + "/contexts.toml";
        write(configPath, "schema_version=3\n[ui]\nfont_size_pt=12\n");
        write(contextsPath, "schema_version=3\n");
        QVERIFY(server.listen(QHostAddress::LocalHost, 0));
        connect(&server, &QTcpServer::newConnection, this, [this] {
            while (auto socket = server.nextPendingConnection()) {
                connect(socket, &QTcpSocket::readyRead, socket, [socket] {
                    auto request = socket->property("requestBytes").toByteArray();
                    request += socket->readAll();
                    if (!request.contains("\r\n\r\n")) {
                        socket->setProperty("requestBytes", request);
                        return;
                    }
                    if (socket->property("responseSent").toBool())
                        return;
                    socket->setProperty("responseSent", true);
                    const QByteArray body =
                        "<!doctype html><title>Owned child secret</title><p>Owned fixture</p>";
                    socket->write("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: " +
                                  QByteArray::number(body.size()) +
                                  "\r\nConnection: close\r\n\r\n" + body);
                    socket->disconnectFromHost();
                });
                connect(socket, &QTcpSocket::disconnected, socket, &QObject::deleteLater);
            }
        });
        launch(R"JSON({"schema_version":3,"contexts":[],"routes":[]})JSON");
        QVERIFY(evaluate("!!testWindow.switcherRegistry && typeof "
                         "testWindow.switcherRegistry.switcherEntries==='function'")
                    .toBool());
    }
    void cleanup() {
        closeWindow();
        server.close();
        server.disconnect(this);
    }
    void heldEnterRequiresFreshConfirmation() {
        const auto origin = QString("http://127.0.0.1:%1").arg(server.serverPort());
        for (const auto &name : {QString("alpha"), QString("beta"), QString("gamma")}) {
            submit((name == "alpha" ? "open " : "tab-open ") + origin + "/" + name);
            QTRY_COMPARE(evaluate("testWindow.browserUi.current_url").toString(),
                         origin + "/" + name);
            QTRY_COMPARE(evaluate("testWindow.activeWebView().url.toString()").toString(),
                         origin + "/" + name);
        }
        submit("switcher --scope tabs alpha");
        QTRY_VERIFY(
            evaluate("testWindow.switcherResultsCurrent() && testWindow.switcherResults.length===1")
                .toBool());
        auto input = visual(window->contentItem(), "switcherSearchInput");
        QVERIFY(input);
        QTRY_VERIFY(input->hasActiveFocus());
        QVERIFY(input->setProperty("text", "beta"));
        QKeyEvent initialPress(QEvent::KeyPress, Qt::Key_Return, Qt::NoModifier, QString(), false,
                               1);
        QCoreApplication::sendEvent(window, &initialPress);
        QKeyEvent searchingRepeat(QEvent::KeyPress, Qt::Key_Return, Qt::NoModifier, QString(), true,
                                  1);
        QCoreApplication::sendEvent(window, &searchingRepeat);
        QCOMPARE(evaluate("testWindow.browserUi.current_url").toString(), origin + "/gamma");
        QTRY_VERIFY(evaluate("testWindow.switcherResultsCurrent() && "
                             "testWindow.switcherResultContext.query==='beta' && "
                             "testWindow.switcherResults.length===1")
                        .toBool());
        QKeyEvent readyRepeat(QEvent::KeyPress, Qt::Key_Return, Qt::NoModifier, QString(), true, 1);
        QCoreApplication::sendEvent(window, &readyRepeat);
        const QJsonObject facts{{"repeat", true},
                                {"before", origin + "/gamma"},
                                {"after", evaluate("testWindow.browserUi.current_url").toString()},
                                {"visible", evaluate("testWindow.switcherVisible").toBool()}};
        qInfo().noquote() << QJsonDocument(facts).toJson(QJsonDocument::Compact);
        const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
        if (!artifacts.isEmpty())
            write(artifacts + "/held-enter.json", QJsonDocument(facts).toJson());
        QCOMPARE(evaluate("testWindow.browserUi.current_url").toString(), origin + "/gamma");
        QVERIFY(evaluate("testWindow.switcherVisible").toBool());
        QKeyEvent heldRelease(QEvent::KeyRelease, Qt::Key_Return, Qt::NoModifier, QString(), false,
                              1);
        QCoreApplication::sendEvent(window, &heldRelease);
        QTest::keyClick(window, Qt::Key_Return);
        QTRY_COMPARE(evaluate("testWindow.browserUi.current_url").toString(), origin + "/beta");
        QTRY_VERIFY(!evaluate("testWindow.switcherVisible").toBool());
    }
    void savedLibraryPendingIsNotEmpty() {
        QTRY_COMPARE_WITH_TIMEOUT(evaluate("String(testWindow.browserUi.permission_decision('https:"
                                           "//owned-switcher.test','notifications'))")
                                      .toString(),
                                  QString("ask"), 15000);
        QVERIFY(evaluate("testWindow.browserUi.switcher_query('', 'marks')").toBool());
        QTRY_VERIFY(!evaluate("testWindow.browserUi.library_pending").toBool());
        QDirIterator files(QFileInfo(configPath).absolutePath() + "/data", {"browser.sqlite"},
                           QDir::Files, QDirIterator::Subdirectories);
        QVERIFY(files.hasNext());
        const auto metadata = files.next();
        QVERIFY(!files.hasNext());
        struct WriteLock {
            sqlite3 *db = nullptr;
            ~WriteLock() {
                if (db) {
                    sqlite3_exec(db, "ROLLBACK", nullptr, nullptr, nullptr);
                    sqlite3_close(db);
                }
            }
        } held;
        QCOMPARE(sqlite3_open_v2(QFile::encodeName(metadata).constData(), &held.db,
                                 SQLITE_OPEN_READWRITE, nullptr),
                 SQLITE_OK);
        QCOMPARE(sqlite3_exec(held.db, "BEGIN IMMEDIATE", nullptr, nullptr, nullptr), SQLITE_OK);
        QCOMPARE(sqlite3_exec(held.db,
                              "INSERT INTO bookmarks(id,url,title,created_at,updated_at) "
                              "VALUES('held-switcher-mark','https://owned-switcher.test/"
                              "heldmark','heldmark',1,1)",
                              nullptr, nullptr, nullptr),
                 SQLITE_OK);
        QVERIFY(evaluate("testWindow.browserUi.remember_permission('https://"
                         "owned-switcher.test','notifications','deny','site')")
                    .toBool());
        evaluate("testWindow.browserUi.refresh_library()");
        QTRY_VERIFY(evaluate("testWindow.browserUi.library_pending").toBool());
        submit("switcher --scope marks heldmark");
        QTRY_VERIFY(
            evaluate(
                "testWindow.switcherResultsCurrent() && testWindow.switcherSourceState.pending")
                .toBool());
        QCOMPARE(evaluate("testWindow.switcherResults.length").toInt(), 0);
        QVERIFY(evaluate("testWindow.browserUi.library_pending").toBool());
        auto status = visual(window->contentItem(), "switcherSourceStatus");
        QVERIFY(status);
        auto message = visual(status, "managerFeedbackMessage");
        QVERIFY(message);
        QCOMPARE(message->property("text").toString(),
                 QString("Loading bookmarks and quickmarks…"));
        QVERIFY(!visual(window->contentItem(), "switcherEmptyState"));
        QSignalSpy frame(window, &QQuickWindow::frameSwapped);
        window->requestUpdate();
        QTRY_VERIFY(!frame.isEmpty());
        const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
        if (!artifacts.isEmpty())
            QVERIFY(window->grabWindow().save(artifacts + "/switcher-library-pending.png"));
        QCOMPARE(sqlite3_exec(held.db, "COMMIT", nullptr, nullptr, nullptr), SQLITE_OK);
        sqlite3_close(held.db);
        held.db = nullptr;
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("!testWindow.switcherSourceState.pending && "
                                          "testWindow.switcherResults.some(function(row){return "
                                          "row.kind==='bookmark' && row.label==='heldmark'})")
                                     .toBool(),
                                 15000);
        QVERIFY(!evaluate("testWindow.switcherSourceState.failed").toBool());
    }

    void savedLibraryFailureWaitsForExplicitRetry() {
        QVERIFY(evaluate("testWindow.browserUi.switcher_query('', 'marks')").toBool());
        QTRY_VERIFY(!evaluate("testWindow.browserUi.library_pending").toBool());
        QDirIterator files(QFileInfo(configPath).absolutePath() + "/data", {"browser.sqlite"},
                           QDir::Files, QDirIterator::Subdirectories);
        QVERIFY(files.hasNext());
        const auto metadata = files.next();
        QVERIFY(!files.hasNext());
        struct Connection {
            sqlite3 *db = nullptr;
            ~Connection() {
                if (db)
                    sqlite3_close(db);
            }
        } database;
        QCOMPARE(sqlite3_open_v2(QFile::encodeName(metadata).constData(), &database.db,
                                 SQLITE_OPEN_READWRITE, nullptr),
                 SQLITE_OK);
        sqlite3_busy_timeout(database.db, 3000);
        // A real malformed stored value makes the actual catalog read fail;
        // the fixture never assigns the published readiness properties.
        QCOMPARE(sqlite3_exec(database.db,
                              "INSERT INTO bookmarks(id,url,title,created_at,updated_at) "
                              "VALUES('invalid-switcher-mark','https://owned-switcher.test/"
                              "recovered',zeroblob(4),1,1)",
                              nullptr, nullptr, nullptr),
                 SQLITE_OK);
        evaluate("testWindow.browserUi.refresh_library()");
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("!testWindow.browserUi.library_pending && "
                                          "testWindow.browserUi.library_error.length>0")
                                     .toBool(),
                                 15000);
        submit("switcher --scope marks recovered");
        QTRY_VERIFY(
            evaluate("testWindow.switcherResultsCurrent() && testWindow.switcherSourceState.failed "
                     "&& !testWindow.switcherSourceState.pending")
                .toBool());
        const auto detail = evaluate("testWindow.switcherSourceState.detail").toString();
        QVERIFY(!detail.isEmpty());
        QVERIFY(!evaluate("testWindow.switcherSourceState.notice").toString().contains(detail));
        auto input = visual(window->contentItem(), "switcherSearchInput");
        QVERIFY(input);
        QVERIFY(input->setProperty("text", "recovered bookmark"));
        QTRY_VERIFY(evaluate("testWindow.switcherResultsCurrent() && "
                             "testWindow.switcherResultContext.query==='recovered bookmark'")
                        .toBool());
        QVERIFY(
            evaluate(
                "testWindow.switcherSourceState.failed && !testWindow.switcherSourceState.pending")
                .toBool());
        QCOMPARE(evaluate("testWindow.switcherSourceState.detail").toString(), detail);
        // Repair the owned data. Typing still must not silently retry I/O.
        QCOMPARE(
            sqlite3_exec(
                database.db,
                "UPDATE bookmarks SET title='recovered bookmark' WHERE id='invalid-switcher-mark'",
                nullptr, nullptr, nullptr),
            SQLITE_OK);
        evaluate("testWindow.refreshSwitcher()");
        QTRY_VERIFY(
            evaluate("testWindow.switcherResultsCurrent() && "
                     "testWindow.switcherBatchIndex>=testWindow.switcherBatchEntries.length")
                .toBool());
        QVERIFY(evaluate("testWindow.switcherSourceState.failed").toBool());
        auto refresh = visual(window->contentItem(), "switcherSourceRefresh");
        QVERIFY(refresh);
        refresh->forceActiveFocus(Qt::TabFocusReason);
        QTRY_VERIFY(refresh->hasActiveFocus());
        QTest::keyClick(window, Qt::Key_Return);
        QTRY_VERIFY_WITH_TIMEOUT(
            evaluate(
                "!testWindow.switcherSourceState.pending && !testWindow.switcherSourceState.failed "
                "&& testWindow.switcherResults.some(function(row){return "
                "row.id==='invalid-switcher-mark'})")
                .toBool(),
            15000);
        QCOMPARE(evaluate("testWindow.switcherSourceState.detail").toString(), QString());
        // A failure in a saved source must not make a live-only scope fail.
        evaluate("testWindow.setSwitcherScope('tabs')");
        QTRY_VERIFY(
            evaluate(
                "testWindow.switcherResultsCurrent() && !testWindow.switcherSourceState.failed && "
                "!testWindow.switcherSourceState.pending")
                .toBool());
    }

    void invokingWindowOwnsTheCombinedLimit() {
        QCOMPARE(evaluate("testWindow.browserUi.feature_switcher_max_results").toInt(), 100);
        auto child = createChild("limit-owner", false);
        QVERIFY(child);
        const QString ui = "testWindow.browserWindowRegistry[1].ui";
        for (int i = 0; i < 10; ++i)
            submitAt(child, ui, "tab-open rb://blank");
        QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), 11);
        submitAt(child, ui, "set --temp switcher.max_results=10");
        QTRY_COMPARE(evaluate(ui + ".feature_switcher_max_results").toInt(), 10);
        QCOMPARE(evaluate("testWindow.browserUi.feature_switcher_max_results").toInt(), 100);
        submitAt(child, ui, "switcher --scope tabs");
        QTRY_VERIFY(
            evaluate(
                "testWindow.switcherResultsCurrent() && testWindow.switcherBatchEntries.length===2 "
                "&& testWindow.switcherBatchIndex===2")
                .toBool());
        QCOMPARE(evaluate("testWindow.switcherHost").value<QObject *>(),
                 static_cast<QObject *>(child));
        QCOMPARE(evaluate("testWindow.switcherMaxResults()").toInt(), 10);
        // Each source has its own backend cap. The invoking window's ten-item
        // cap must also constrain their combined results and visible count.
        QCOMPARE(evaluate("testWindow.switcherResults.length").toInt(), 10);
        auto count = visual(child->contentItem(), "switcherResultCount");
        QVERIFY(count);
        QCOMPARE(count->property("text").toString(), QString("10 results"));
    }
    void rootSwitcherHonorsPrivacy_data() {
        QTest::addColumn<QString>("kind");
        for (const auto &kind : {"regular", "private", "ephemeral"})
            QTest::newRow(kind) << QString::fromLatin1(kind);
    }
    void rootSwitcherHonorsPrivacy() {
        QFETCH(QString, kind);
        const bool privateChild = kind == "private";
        const bool ephemeralChild = kind == "ephemeral";
        const bool transientChild = privateChild || ephemeralChild;
        QVERIFY(!evaluate("testWindow.temporaryProfile || testWindow.browserProfile.offTheRecord")
                     .toBool());
        const auto url = QString("http://127.0.0.1:%1/child-secret").arg(server.serverPort());
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:" +
                 quoted(url) + ",windowProfileName:'privacy-fixture',windowPrivateProfile:" +
                 (privateChild ? "true" : "false") +
                 ",windowEphemeralProfile:" + (ephemeralChild ? "true" : "false") +
                 ",windowEphemeralInvocationToken:'owned-switcher-ephemeral'})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
        QTRY_VERIFY_WITH_TIMEOUT(
            evaluate("testWindow.browserWindowRegistry[1].host.profileBootstrapComplete && "
                     "!testWindow.browserWindowRegistry[1].ui.profile_bootstrap_pending")
                .toBool(),
            15000);
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry[1].ui.current_url").toString(),
                     url);
        QCOMPARE(evaluate("testWindow.browserWindowRegistry[1].profile.offTheRecord").toBool(),
                 transientChild);
        const auto child =
            evaluate("String(testWindow.browserWindowRegistry[1].ui.window_token)").toString();
        submit("switcher --scope tabs child-secret");
        QTRY_VERIFY(
            evaluate("testWindow.switcherVisible && testWindow.switcherBatchQuery==='child-secret' "
                     "&& testWindow.switcherBatchEntries.length>0 && "
                     "testWindow.switcherBatchIndex>=testWindow.switcherBatchEntries.length")
                .toBool());
        QCOMPARE(evaluate("testWindow.switcherBatchEntries.length").toInt(),
                 transientChild ? 1 : 2);
        const auto rows = evaluate("JSON.stringify(testWindow.switcherResults)").toString();
        const bool found =
            evaluate("testWindow.switcherResults.some(function(row){return row.owner_token===" +
                     quoted(child) + "})")
                .toBool();
        const QJsonObject facts{{"kind", kind},
                                {"childToken", child},
                                {"childReturned", found},
                                {"rows", QJsonDocument::fromJson(rows.toUtf8()).array()}};
        qInfo().noquote() << QJsonDocument(facts).toJson(QJsonDocument::Compact);
        const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
        if (!artifacts.isEmpty()) {
            QSignalSpy frame(window, &QQuickWindow::frameSwapped);
            window->requestUpdate();
            QTRY_VERIFY(!frame.isEmpty());
            QVERIFY(window->grabWindow().save(
                artifacts + "/" + QString::fromLatin1(QTest::currentDataTag()) + ".png"));
            write(artifacts + "/" + QString::fromLatin1(QTest::currentDataTag()) + ".json",
                  QJsonDocument(facts).toJson());
        }
        QCOMPARE(found, !transientChild);
    }
    void privateInvokerStaysInItsSessionAndHost() {
        const auto normal =
            QString("http://127.0.0.1:%1/child-secret-normal").arg(server.serverPort());
        submit("open " + normal);
        QTRY_COMPARE(evaluate("testWindow.browserUi.current_url").toString(), normal);
        auto privateHost = createChild("child-secret-a", true);
        QVERIFY(privateHost);
        auto independent = createChild("child-secret-b", true);
        QVERIFY(independent);
        QVERIFY(evaluate("testWindow.browserWindowRegistry[1].profile!==testWindow."
                         "browserWindowRegistry[2].profile")
                    .toBool());
        submitAt(privateHost, "testWindow.browserWindowRegistry[1].ui",
                 "set --temp ui.font_size_pt=20");
        QTRY_COMPARE(privateHost->property("font").value<QFont>().pointSize(), 20);
        privateHost->resize(640, 480);
        submitAt(privateHost, "testWindow.browserWindowRegistry[1].ui",
                 "switcher --scope tabs child-secret");
        QTRY_VERIFY(
            evaluate(
                "testWindow.switcherVisible && testWindow.switcherBatchQuery==='child-secret' && "
                "testWindow.switcherBatchEntries.length===1 && testWindow.switcherBatchIndex===1")
                .toBool());
        QCOMPARE(evaluate("testWindow.switcherHost").value<QObject *>(),
                 static_cast<QObject *>(privateHost));
        QVERIFY(evaluate("testWindow.switcherUi===testWindow.browserWindowRegistry[1].ui && "
                         "!testWindow.modalInputScope.activeSurface && "
                         "!!testWindow.switcherHost.modalInputScope.activeSurface")
                    .toBool());
        const auto rows = evaluate("JSON.stringify(testWindow.switcherResults)").toString();
        QVERIFY(evaluate("testWindow.switcherResults.length>0 && "
                         "testWindow.switcherResults.every(function(row){return "
                         "row.owner_token===testWindow.switcherOwnerToken})")
                    .toBool());
        auto field = visual(privateHost->contentItem(), "switcherSearchInput");
        QVERIFY(field);
        QTRY_VERIFY(field->hasActiveFocus());
        auto panel = visual(privateHost->contentItem(), "ferricModalPanel");
        QVERIFY(panel);
        QSignalSpy frame(privateHost, &QQuickWindow::frameSwapped);
        privateHost->requestUpdate();
        QTRY_VERIFY(!frame.isEmpty());
        QVERIFY(QRectF(0, 0, privateHost->width(), privateHost->height())
                    .adjusted(-1, -1, 1, 1)
                    .contains(panel->mapRectToScene(panel->boundingRect())));
        const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
        if (!artifacts.isEmpty()) {
            QVERIFY(privateHost->grabWindow().save(artifacts + "/private-invoker-large.png"));
            write(
                artifacts + "/private-invoker.json",
                QJsonDocument(QJsonObject{{"rows", QJsonDocument::fromJson(rows.toUtf8()).array()},
                                          {"hostedInInvoker", true},
                                          {"sourceCount", 1}})
                    .toJson());
        }
        window->requestActivate();
        QTRY_VERIFY(window->isActive());
        QTest::keyClick(window, Qt::Key_Escape);
        QVERIFY(evaluate("testWindow.switcherVisible").toBool());
        privateHost->requestActivate();
        QTRY_VERIFY(privateHost->isActive());
        QTest::keyClick(privateHost, Qt::Key_Escape);
        QTRY_VERIFY(!evaluate("testWindow.switcherVisible").toBool());
        QTRY_VERIFY(
            evaluate("testWindow.browserWindowRegistry[1].host.activeView.activeFocus").toBool());
        QCOMPARE(evaluate("testWindow.browserUi.current_url").toString(), normal);
    }
    void retiredResultDoesNotUsePrimaryOwner() {
        auto child = createChild("child-secret-retired", false);
        QVERIFY(child);
        submit("switcher --scope tabs child-secret-retired");
        QTRY_VERIFY(
            evaluate(
                "testWindow.switcherResults.some(function(row){return "
                "row.owner_token===String(testWindow.browserWindowRegistry[1].ui.window_token)})")
                .toBool());
        const auto captured =
            evaluate(
                "JSON.stringify(testWindow.switcherResults.find(function(row){return "
                "row.owner_token===String(testWindow.browserWindowRegistry[1].ui.window_token)}))")
                .toString();
        const auto before = evaluate("testWindow.browserUi.current_url").toString();
        evaluate("testWindow.browserWindowRegistry[1].host.windowShutdownApproved=true;testWindow."
                 "browserWindowRegistry[1].host.close()");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 1);
        // Retain the captured action across actual owner retirement, as an
        // already-pressed result can outlive its source window.
        evaluate("testWindow.switcherResults=[JSON.parse(" + quoted(captured) + ")]");
        evaluate("testWindow.activateSwitcher(0)");
        QCOMPARE(evaluate("testWindow.browserUi.current_url").toString(), before);
        QVERIFY(evaluate("testWindow.browserUi.status_text.indexOf('no longer available')>=0")
                    .toBool());
        QVERIFY(evaluate("testWindow.switcherVisible").toBool());
    }
};
int main(int argc, char **argv) {
    ferric_browser_enable_software_rendering();
    ferric_browser_register_internal_scheme();
    QQuickWindow::setGraphicsApi(QSGRendererInterface::Software);
    QtWebEngineQuick::initialize();
    cxx_qt_init_crate_ferric_browser_engine_qt();
    QVector<QByteArray> args;
    for (int i = 0; i < argc; ++i)
        args.append(QByteArray(argv[i]));
    auto app = ferric_browser_gui_application(args);
    if (!app || !ferric_browser_gui_preferences_ready())
        return 1;
    SwitcherTests tests;
    return QTest::qExec(&tests, argc, argv);
}
#include "tst_switcher.moc"
