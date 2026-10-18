#include "../../src/gui_application.h"
#include "notification_test_service.h"
#include <QApplication>
#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QGuiApplication>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQmlExpression>
#include <QQuickItem>
#include <QQuickWindow>
#include <QTcpServer>
#include <QTcpSocket>
#include <QTemporaryDir>
#include <QtTest>
#include <QtWebEngineCore/QWebEngineProfile>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include <memory>
#include <sqlite3.h>

extern "C" bool cxx_qt_init_crate_ferric_browser_engine_qt();
#include "../../src/url_display.h"

class NotificationConsentTests final : public QObject {
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
        engine->rootContext()->setContextProperty("pageHarness", this);
        QTRY_VERIFY_WITH_TIMEOUT(
            evaluate(
                "testWindow.profileBootstrapComplete && "
                "!testWindow.browserUi.profile_bootstrap_pending && !!testWindow.activeWebView()")
                .toBool(),
            15000);
        QCOMPARE(evaluate("testWindow.browserProfile.persistentPermissionsPolicy").toInt(),
                 static_cast<int>(QWebEngineProfile::PersistentPermissionsPolicy::AskEveryTime));
        QVERIFY(!evaluate("testWindow.browserProfile.offTheRecord").toBool());
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
    QVariant scriptResult;
    bool scriptDone = false;
    QVariant page(const QString &script) {
        scriptDone = false;
        evaluate("testWindow.activeWebView().runJavaScript(" + quoted(script) +
                 ",0,function(value){pageHarness.accept(value)})");
        QElapsedTimer timer;
        timer.start();
        while (!scriptDone && timer.elapsed() < 5000)
            QTest::qWait(5);
        if (!scriptDone)
            qFatal("Owned page observation timed out");
        return scriptResult;
    }
    static QQuickItem *button(QQuickItem *parent, const QString &text) {
        if (parent->isVisible() && parent->property("down").isValid() &&
            parent->property("text").toString() == text)
            return parent;
        for (auto child : parent->childItems())
            if (auto result = button(child, text))
                return result;
        return nullptr;
    }

    static bool fullyVisible(QQuickItem *item) {
        if (!item || !item->isVisible() || !item->window() || item->width() <= 0 ||
            item->height() <= 0)
            return false;
        const auto bounds = item->mapRectToScene(item->boundingRect());
        if (!QRectF(0, 0, item->window()->width(), item->window()->height())
                 .adjusted(-1, -1, 1, 1)
                 .contains(bounds))
            return false;
        for (auto ancestor = item->parentItem(); ancestor; ancestor = ancestor->parentItem())
            if (ancestor->clip() && !ancestor->mapRectToScene(ancestor->boundingRect())
                                         .adjusted(-1, -1, 1, 1)
                                         .contains(bounds))
                return false;
        return true;
    }

  public:
    Q_INVOKABLE void accept(const QVariant &value) {
        scriptResult = value;
        scriptDone = true;
    }
  private slots:
    void init() {
        QVERIFY(directory.isValid());
        const auto name = QString::fromLatin1(QTest::currentDataTag());
        auto base = directory.filePath(name);
        QVERIFY(QDir().mkpath(base));
        write(base + "/.ferric-owned-fixture", "Owned notification permission regression only.\n");
        configPath = base + "/config.toml";
        contextsPath = base + "/contexts.toml";
        write(configPath, "schema_version=3\n[ui]\nfont_size_pt=12\n");
        write(contextsPath, "schema_version=3\n");
        QVERIFY(server.listen(QHostAddress::LocalHost, 0));
        connect(&server, &QTcpServer::newConnection, this, [this] {
            while (auto socket = server.nextPendingConnection()) {
                connect(socket, &QTcpSocket::readyRead, socket, [socket] {
                    if (!socket->readAll().contains("\r\n\r\n"))
                        return;
                    const QByteArray body =
                        R"HTML(<!doctype html><meta charset=utf-8><style>body{margin:20px}button{width:300px;height:60px}</style>
<button id=notify onclick="Notification.requestPermission().then(function(value){window.permissionResult=value;if(value==='granted'){window.notice=new Notification('Explicit once notification');notice.onshow=function(){window.shown++};notice.onclose=function(){window.closed++}}})">Enable notifications once</button>
<script>window.shown=0;window.closed=0;</script>)HTML";
                    socket->write("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: " +
                                  QByteArray::number(body.size()) +
                                  "\r\nConnection: close\r\n\r\n" + body);
                    socket->disconnectFromHost();
                });
                connect(socket, &QTcpSocket::disconnected, socket, &QObject::deleteLater);
            }
        });
        launch(R"JSON({"schema_version":3,"contexts":[],"routes":[]})JSON");
    }
    void cleanup() {
        closeWindow();
        server.close();
        server.disconnect(this);
    }
    void explicitOnce_data() {
        QTest::addColumn<QString>("kind");
        for (const auto &kind :
             {"ready", "invalid-saved-policy", "stored-deny", "denied-before-acknowledgement"})
            QTest::newRow(kind) << QString::fromLatin1(kind);
    }
    void explicitOnce() {
        QFETCH(QString, kind);
        const bool invalid = kind == "invalid-saved-policy";
        const bool denied = kind == "stored-deny";
        const bool lateDeny = kind == "denied-before-acknowledgement";
        NotificationTestService service("once");
        QVERIFY(service.start());
        const auto origin = QString("http://127.0.0.1:%1").arg(server.serverPort());
        const auto lookup = "String(testWindow.browserUi.permission_decision(" + quoted(origin) +
                            ",'notifications'))";
        QTRY_COMPARE_WITH_TIMEOUT(evaluate(lookup).toString(), QString("ask"), 15000);
        closeWindow();
        QDirIterator files(QFileInfo(configPath).absolutePath() + "/data", {"browser.sqlite"},
                           QDir::Files, QDirIterator::Subdirectories);
        QVERIFY(files.hasNext());
        const auto metadata = files.next();
        QVERIFY(!files.hasNext());
        if (invalid || denied) {
            sqlite3 *database = nullptr;
            QCOMPARE(sqlite3_open_v2(QFile::encodeName(metadata).constData(), &database,
                                     SQLITE_OPEN_READWRITE, nullptr),
                     SQLITE_OK);
            sqlite3_stmt *statement = nullptr;
            QCOMPARE(sqlite3_prepare_v2(
                         database,
                         "INSERT INTO permission_rules(origin,permission,decision,updated_at) "
                         "VALUES(?1,'notifications',?2,1)",
                         -1, &statement, nullptr),
                     SQLITE_OK);
            const auto encoded = origin.toUtf8();
            QCOMPARE(sqlite3_bind_text(statement, 1, encoded.constData(), encoded.size(),
                                       SQLITE_TRANSIENT),
                     SQLITE_OK);
            QCOMPARE(sqlite3_bind_text(statement, 2, denied ? "deny" : "invalid-saved-decision", -1,
                                       SQLITE_STATIC),
                     SQLITE_OK);
            QCOMPARE(sqlite3_step(statement), SQLITE_DONE);
            sqlite3_finalize(statement);
            sqlite3_close(database);
        }
        launch(R"JSON({"schema_version":3,"contexts":[],"routes":[]})JSON");
        QVERIFY(!evaluate("testWindow.safeMode || testWindow.temporaryProfile").toBool());
        QTRY_COMPARE_WITH_TIMEOUT(
            evaluate(lookup).toString(),
            invalid ? QString("pending") : (denied ? QString("deny") : QString("ask")), 15000);
        submit("open " + origin + "/notify");
        QTRY_VERIFY_WITH_TIMEOUT(
            !evaluate("testWindow.activeWebView().loading").toBool() &&
                page("document.readyState==='complete' && !!document.getElementById('notify')")
                    .toBool(),
            15000);
        auto view =
            qobject_cast<QQuickItem *>(evaluate("testWindow.activeWebView()").value<QObject *>());
        QVERIFY(view);
        window->requestActivate();
        QTRY_VERIFY(window->isActive());
        view->forceActiveFocus();
        const auto point =
            page("(()=>{const r=document.getElementById('notify').getBoundingClientRect();return "
                 "[r.x+r.width/2,r.y+r.height/2]})()")
                .toList();
        QSignalSpy presented(window, &QQuickWindow::frameSwapped);
        window->requestUpdate();
        QTRY_VERIFY(!presented.isEmpty());
        QTest::mouseClick(
            window, Qt::LeftButton, Qt::NoModifier,
            view->mapToScene(QPointF(point[0].toDouble(), point[1].toDouble())).toPoint());
        QString policyNotice;
        const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
        if (!artifacts.isEmpty())
            QVERIFY(QDir().mkpath(artifacts));
        if (denied) {
            QTRY_COMPARE(page("window.permissionResult").toString(), QString("denied"));
            QVERIFY(!evaluate("testWindow.permissionPromptVisible").toBool());
        } else {
            QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.permissionPromptVisible && "
                                              "testWindow.pendingPermissionName==='notifications'")
                                         .toBool(),
                                     10000);
            policyNotice = evaluate("testWindow.pendingPermissionPolicyNotice").toString();
            if (invalid)
                QVERIFY2(policyNotice.contains("couldn't be checked"), qPrintable(policyNotice));
            // This ordinary window has room for all four choices; a partly
            // clipped final choice must not require an unnecessary scroll.
            for (const auto &label : {QString("Deny"), QString("Allow for session"),
                                      QString("Allow once"), QString("Always allow for this site")})
                QTRY_VERIFY2(fullyVisible(button(window->contentItem(), label)), qPrintable(label));
            QSignalSpy dialogFrames(window, &QQuickWindow::frameSwapped);
            window->requestUpdate();
            QTRY_VERIFY_WITH_TIMEOUT(!dialogFrames.isEmpty(), 5000);
            auto allow = button(window->contentItem(), "Allow once");
            QVERIFY(allow);
            QVERIFY(allow->isEnabled());
            if (!artifacts.isEmpty()) {
                QVERIFY(QDir().mkpath(artifacts));
                QVERIFY(
                    window->grabWindow().save(artifacts + "/notification-once-" + kind + ".png"));
            }
            QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier,
                              allow->mapToScene(allow->boundingRect().center()).toPoint());
            QTRY_COMPARE(page("window.permissionResult").toString(), QString("granted"));
            QElapsedTimer deadline;
            deadline.start();
            bool accepted = false;
            while (deadline.elapsed() < 4000 && page("window.shown").toInt() != 1) {
                if (!accepted && service.pendingCount() > 0) {
                    if (lateDeny) {
                        QVERIFY(evaluate("testWindow.browserUi.remember_permission(" +
                                         quoted(origin) + ",'notifications','deny','session')")
                                    .toBool());
                        QTRY_COMPARE(evaluate(lookup).toString(), QString("deny"));
                    }
                    service.acceptNext(77);
                    accepted = true;
                }
                QTest::qWait(20);
            }
        }
        const auto shown = page("window.shown").toInt();
        if (lateDeny)
            QTRY_VERIFY(service.closedIds().contains(77));
        const QJsonObject facts{{"case", QString::fromLatin1(QTest::currentDataTag())},
                                {"safeMode", false},
                                {"permissionResult", page("window.permissionResult").toString()},
                                {"policyNotice", policyNotice},
                                {"currentPolicy", evaluate(lookup).toString()},
                                {"shown", shown},
                                {"status", evaluate("testWindow.browserUi.status_text").toString()},
                                {"service", service.snapshot()}};
        qInfo().noquote() << QJsonDocument(facts).toJson(QJsonDocument::Compact);
        if (!artifacts.isEmpty()) {
            write(artifacts + "/notification-once-" + QString::fromLatin1(QTest::currentDataTag()) +
                      ".json",
                  QJsonDocument(facts).toJson());
        }
        closeWindow();
        if (!artifacts.isEmpty())
            QVERIFY(QFile::copy(metadata, artifacts + "/notification-once-" +
                                              QString::fromLatin1(QTest::currentDataTag()) +
                                              ".sqlite"));
        QCOMPARE(shown, denied || lateDeny ? 0 : 1);
        QCOMPARE(service.deliveryCount(), denied ? 0 : 1);
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
    NotificationConsentTests tests;
    return QTest::qExec(&tests, argc, argv);
}
#include "tst_notification_consent.moc"
