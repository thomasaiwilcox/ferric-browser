#include "../../src/gui_application.h"
#include <QApplication>
#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QPointer>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQmlExpression>
#include <QQuickItem>
#include <QQuickWindow>
#include <QProcess>
#include <QSslServer>
#include <QSslKey>
#include <QSslSocket>
#include <QTcpServer>
#include <QTcpSocket>
#include <QTemporaryDir>
#include <QTimer>
#include <QtTest>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include <memory>
#include <sqlite3.h>

extern "C" bool cxx_qt_init_crate_ferric_browser_engine_qt();
#include "../../src/url_display.h"

class DownloadsTests final : public QObject {
    Q_OBJECT
    QTemporaryDir directory;
    std::unique_ptr<QQmlApplicationEngine> engine;
    QQuickWindow *window = nullptr;
    QTcpServer server;
    QString configPath;
    QVariant pageResult;
    bool pageDone = false;
    QVariant page(const QString &view, const QString &script) {
        pageDone = false;
        const auto accepted = evaluate("testWindow.runBrowserScript(" + view + "," + quoted(script)
                 + ",function(value){downloadInitiatorProbe.acceptPage(value)})").toBool();
        if (!accepted)
            qFatal("Owned page script rejected (%s): %s", qPrintable(view), qPrintable(evaluate(
                "JSON.stringify({url:String(" + view + ".url),nativeUrl:String(" + view
                + ".nativeUrl),loading:" + view + ".loading,frame:" + view + ".mainFrame.isValid})").toString()));
        QElapsedTimer timer;
        timer.start();
        while (!pageDone && timer.elapsed() < 10000) QTest::qWait(5);
        if (!pageDone) qFatal("Owned page script did not complete");
        return pageResult;
    }
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
    void launch() {
        const auto base = QFileInfo(configPath).absolutePath();
        const QJsonObject configuration{
            {"schema_version", 3},
            {"ui", QJsonObject{{"font_size_pt", 12}}},
            {"downloads",
             QJsonObject{{"directory", QJsonObject{{"path", base + "/owned-downloads"}}},
                         {"ask_destination", false}}}};
        const QString config =
            QString::fromUtf8(QJsonDocument(configuration).toJson(QJsonDocument::Compact));
        engine = std::make_unique<QQmlApplicationEngine>();
        engine->setInitialProperties(
            {{"temporaryProfile", false},
             {"storageBasePath", base + "/data"},
             {"softwareRendering", true},
             {"safeMode", false},
             {"userscriptsOff", true},
             {"startupConfigJson", config},
             {"startupConfigBaseJson", config},
             {"startupConfigPath", configPath},
             {"startupCliOverridesJson", "{\"schema_version\":3}"},
             {"startupProfileOverridesJson", "{\"schema_version\":3}"},
             {"startupContextsJson", R"JSON({"schema_version":3,"contexts":[],"routes":[]})JSON"},
             {"startupUrl", "rb://blank"},
             {"width", 900},
             {"height", 700}});
        engine->load(QUrl("qrc:/qt/qml/io/github/ferricbrowser/qml/Main.qml"));
        QCOMPARE(engine->rootObjects().size(), 1);
        window = qobject_cast<QQuickWindow *>(engine->rootObjects().first());
        QVERIFY(window);
        engine->rootContext()->setContextProperty("testWindow", window);
        engine->rootContext()->setContextProperty("downloadInitiatorProbe", this);
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.profileBootstrapComplete && "
                                          "!testWindow.browserUi.profile_bootstrap_"
                                          "pending && !!testWindow.activeWebView()")
                                     .toBool(),
                                 15000);
        window->show();
        window->requestActivate();
        QTRY_VERIFY(window->isActive());
    }
    void closeWindow() {
        if (engine && window) {
            evaluate("testWindow.browserWindowRegistry.slice(1).concat(testWindow.popupWindowRegistry).forEach(function("
                     "entry){entry.host."
                     "windowShutdownApproved=true; entry.host.close(); "
                     "entry.host.destroy()})");
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
        const QString registry = "testWindow.browserWindowRegistry";
        const auto before = evaluate(registry + ".length").toInt();
        const auto url = QString("http://127.0.0.1:%1/").arg(server.serverPort()) + path;
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:" +
                 quoted(url) + ",windowProfileName:" + quoted(path) + ",windowPrivateProfile:" +
                 (privateProfile ? "true" : "false") + ",windowEphemeralProfile:false})");
        QElapsedTimer timer;
        timer.start();
        while (evaluate(registry + ".length").toInt() != before + 1 &&
               timer.elapsed() < 10000)
            QTest::qWait(5);
        if (evaluate(registry + ".length").toInt() != before + 1)
            qFatal("Owned child did not register");
        const auto entry = registry + "[" + QString::number(before) + "]";
        timer.restart();
        while (!evaluate(entry + ".host.profileBootstrapComplete && !" + entry +
                         ".ui.profile_bootstrap_pending && " + entry +
                         ".ui.current_url===" + quoted(url))
                    .toBool() &&
               timer.elapsed() < 15000)
            QTest::qWait(5);
        if (!evaluate(entry + ".host.profileBootstrapComplete && !" + entry +
                      ".ui.profile_bootstrap_pending && " + entry + ".ui.current_url===" + quoted(url))
                 .toBool())
            qFatal("Owned child did not open");
        // Core URL publication precedes the actual renderer document. Wait
        // for this newly created view's fixture before issuing page-bound work.
        timer.restart();
        const auto documentReady = "(()=>{const v=" + entry + ".host.activeWebView();return !!v && !v.loading && v.mainFrame.isValid && String(v.url)===" + quoted(url) + " && v.title==='Owned child secret'})()";
        while (!evaluate(documentReady).toBool() && timer.elapsed() < 10000) QTest::qWait(5);
        if (!evaluate(documentReady).toBool()) qFatal("Owned child did not render its fixture");
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
    static QQuickItem *visibleButton(QQuickItem *item, const QString &text) {
        if (item->isVisible() && item->property("text").toString() == text)
            return item;
        for (auto child : item->childItems())
            if (auto found = visibleButton(child, text))
                return found;
        return nullptr;
    }

  public:
    Q_INVOKABLE void acceptPage(const QVariant &value) { pageResult = value; pageDone = true; }

  private slots:
    void init() {
        QVERIFY(directory.isValid());
        auto base = directory.filePath(QString::fromLatin1(QTest::currentTestFunction()) + "/" +
                                       QString::fromLatin1(QTest::currentDataTag()));
        QVERIFY(QDir().mkpath(base));
        write(base + "/.ferric-owned-fixture", "Owned Downloads integration fixture only.\n");
        configPath = base + "/config.toml";
        write(configPath, ("schema_version=3\n[ui]\nfont_size_pt=12\n[downloads]\ndirectory={"
                           "path=" +
                           quoted(QFileInfo(configPath).absolutePath() + "/owned-downloads") +
                           "}\nask_destination=false\n")
                              .toUtf8());
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
                    const QByteArray body = "<!doctype html><title>Owned child "
                                            "secret</title><p>Owned fixture</p>";
                    socket->write("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: " +
                                  QByteArray::number(body.size()) +
                                  "\r\nSet-Cookie: owned=download-initiator; Path=/; SameSite=Lax\r\nConnection: close\r\n\r\n" + body);
                    socket->disconnectFromHost();
                });
                connect(socket, &QTcpSocket::disconnected, socket, &QObject::deleteLater);
            }
        });
        launch();
    }
    void cleanup() {
        closeWindow();
        server.close();
        server.disconnect(this);
    }
    void downloadInitiation_data() {
        QTest::addColumn<QString>("behavior");
        for (const auto &name : {"direct", "native-start", "retry", "retry-redirect", "private", "secondary-redirect", "bodyless", "popup",
                                 "cancel-preparation", "retirement", "navigation", "destination-cancel", "authentication",
                                 "cancel-active", "pause-resume", "retire-active", "private-cancel-active", "private-pause-resume", "private-retire-active", "peer-retire-active"})
            QTest::newRow(name) << QString::fromLatin1(name);
    }
    void downloadInitiation() {
        QFETCH(QString, behavior);
        QString host = "testWindow";
        QString ui = "testWindow.browserUi";
        QQuickWindow *owner = window;
        auto original = QString("http://127.0.0.1:%1/user").arg(server.serverPort());
        if (behavior == "peer-retire-active") {
            submit("window-new");
            QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
            QTRY_VERIFY(evaluate("testWindow.browserWindowRegistry[1].host.profileBootstrapComplete && !testWindow.browserWindowRegistry[1].ui.profile_bootstrap_pending").toBool());
            QVERIFY(evaluate("testWindow.browserWindowRegistry[1].profile===testWindow.browserProfile").toBool());
            owner = qobject_cast<QQuickWindow *>(evaluate("testWindow.browserWindowRegistry[1].host").value<QObject *>());
            QVERIFY(owner);
        } else if (behavior.startsWith("private") || behavior == "secondary-redirect" || behavior == "retirement" || behavior == "retire-active") {
            owner = createChild("initiator-owner", behavior.startsWith("private"));
            original = QString("http://127.0.0.1:%1/initiator-owner").arg(server.serverPort());
        } else if (behavior == "popup") {
            submit("open " + original);
            QTRY_COMPARE(evaluate(ui + ".current_url").toString(), original);
            QTRY_VERIFY(evaluate("testWindow.activeWebView().mainFrame.isValid && !testWindow.activeWebView().loading").toBool());
            QTRY_VERIFY(page("testWindow.activeWebView()", "location.href===" + quoted(original)
                + " && document.readyState==='complete'").toBool());
            original = QString("http://127.0.0.1:%1/initiator-popup").arg(server.serverPort());
            QVERIFY(page("testWindow.activeWebView()", "(()=>{const b=document.createElement('button');"
                "b.style='position:fixed;left:30px;top:30px;width:200px;height:40px';"
                "b.textContent='Open owned popup';b.onclick=()=>window.open(" + quoted(original)
                + ",'_blank','popup,width=700,height=600');document.body.prepend(b);return true})()").toBool());
            auto opener = qobject_cast<QQuickItem *>(evaluate("testWindow.activeWebView()").value<QObject *>());
            QVERIFY(opener);
            opener->forceActiveFocus();
            QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier, opener->mapToScene(QPointF(80, 50)).toPoint());
            QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(), 1);
            owner = qobject_cast<QQuickWindow *>(evaluate("testWindow.popupWindowRegistry[0].host").value<QObject *>());
            QVERIFY(owner);
        } else if (behavior != "native-start") {
            submit("open " + original);
        } else {
            original = "rb://blank";
        }
        if (owner != window) {
            engine->rootContext()->setContextProperty("ownedInitiatorHost", owner);
            auto object = evaluate("ownedInitiatorHost.windowUi").value<QObject *>();
            QVERIFY(object);
            engine->rootContext()->setContextProperty("ownedInitiatorUi", object);
            host = "ownedInitiatorHost";
            ui = "ownedInitiatorUi";
        }
        if (behavior == "peer-retire-active") submitAt(owner, ui, "open " + original);
        const auto view = host + ".activeWebView()";
        QTRY_COMPARE(evaluate(ui + ".current_url").toString(), original);
        if (behavior != "native-start") {
            QTRY_VERIFY(evaluate("String(" + view + ".url)===" + quoted(original) + " && !" + view + ".loading && " + view + ".mainFrame.isValid").toBool());
            QTRY_COMPARE(page(view, "document.readyState").toString(), QString("complete"));
            QTRY_VERIFY(page(view, "document.cookie.includes('owned=download-initiator')").toBool());
            page(view, "window.ownedDownloadSentinel='unchanged';true");
        }
        const auto rootUrl = evaluate("testWindow.browserUi.current_url").toString();
        const auto tabCount = evaluate(ui + ".tab_count").toInt();
        if (behavior == "bodyless") QVERIFY(page(view, "document.body.remove();true").toBool());
        if (behavior == "destination-cancel") submitAt(owner, ui, "set --temp downloads.ask_destination=true");
        const auto destination = evaluate(ui + ".default_download_directory()").toString();
        const auto workspace = host + ".downloadWorkspaceObject";
        QPointer<QTcpSocket> transfer;
        bool cookieMatched = false;
        int finalRequests = 0;
        int redirectRequests = 0;
        QTcpServer payload;
        QTcpServer redirects;
        QVERIFY(payload.listen(QHostAddress::LocalHost, 0));
        QVERIFY(redirects.listen(QHostAddress::LocalHost, 0));
        const bool delayed = behavior == "cancel-preparation" || behavior == "retirement" || behavior == "navigation";
        connect(&payload, &QTcpServer::newConnection, &payload, [&] {
            while (auto socket = payload.nextPendingConnection()) {
                connect(socket, &QTcpSocket::readyRead, socket, [&, socket] {
                    auto request = socket->property("request").toByteArray() + socket->readAll();
                    socket->setProperty("request", request);
                    if (!request.contains("\r\n\r\n") || socket->property("sent").toBool()) return;
                    socket->setProperty("sent", true);
                    if (!request.startsWith("GET /report ")) {
                        socket->write("HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n");
                        socket->disconnectFromHost(); return;
                    }
                    if (behavior == "authentication" && !request.contains("Authorization: Basic b3duZWQ6Y3JlZGVudGlhbA==")) {
                        socket->write("HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"Owned download\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                        socket->disconnectFromHost(); return;
                    }
                    ++finalRequests;
                    cookieMatched = request.contains("owned=download-initiator");
                    transfer = socket;
                    if (delayed) return; // Authoritative before-headers producer barrier.
                    const auto attachment = behavior == "retry" && finalRequests == 1
                        ? QByteArray("Content-Disposition: attachment; filename=old-report\r\n") : QByteArray();
                    socket->write("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n" + attachment + "Content-Length: 1048576\r\nConnection: close\r\n\r\n");
                    socket->write(QByteArray(262144, 'x'));
                });
                connect(socket, &QTcpSocket::disconnected, socket, &QObject::deleteLater);
            }
        });
        const auto target = QString("http://127.0.0.1:%1/report").arg(payload.serverPort());
        connect(&redirects, &QTcpServer::newConnection, &redirects, [&] {
            while (auto socket = redirects.nextPendingConnection()) {
                connect(socket, &QTcpSocket::readyRead, socket, [socket, target, &redirectRequests] {
                    auto request = socket->property("request").toByteArray() + socket->readAll();
                    socket->setProperty("request", request);
                    if (!request.contains("\r\n\r\n") || socket->property("sent").toBool()) return;
                    socket->setProperty("sent", true);
                    ++redirectRequests;
                    socket->write("HTTP/1.1 302 Found\r\nLocation: " + target.toUtf8() + "\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    socket->disconnectFromHost();
                });
                connect(socket, &QTcpSocket::disconnected, socket, &QObject::deleteLater);
            }
        });
        QString retrySource;
        if (behavior == "retry" || behavior == "retry-redirect") {
            const auto firstUrl = behavior == "retry-redirect" ? QString("http://127.0.0.1:%1/redirect").arg(redirects.serverPort()) : target;
            submitAt(owner, ui, (behavior == "retry" ? "open " : "download ") + firstUrl);
            QTRY_VERIFY(transfer);
            QTRY_COMPARE(evaluate("Object.keys(" + host + ".activeDownloads).length").toInt(), 1);
            retrySource = evaluate("Object.keys(" + host + ".activeDownloads)[0]").toString();
            QVERIFY(evaluate(ui + ".execute_ui_action('browser.download.cancel'," + quoted(retrySource) + ")").toBool());
            evaluate(host + ".executePendingEngineAction()");
            QTRY_COMPARE(evaluate("Object.keys(" + host + ".activeDownloads).length").toInt(), 0);
            QTRY_VERIFY(!transfer || transfer->state() == QAbstractSocket::UnconnectedState);
            transfer = nullptr;
            submitAt(owner, ui, "open " + original);
            QTRY_VERIFY(evaluate("String(" + view + ".url)===" + quoted(original) + " && !" + view + ".loading").toBool());
            page(view, "window.ownedDownloadSentinel='unchanged';true");
            QVERIFY(evaluate(ui + ".execute_ui_action('browser.download.retry'," + quoted(retrySource) + ")").toBool());
        } else {
            const auto url = behavior == "secondary-redirect" ? QString("http://127.0.0.1:%1/redirect").arg(redirects.serverPort()) : target;
            if (behavior == "popup") {
                QVERIFY(evaluate(ui + ".execute_ui_action('browser.link.download'," + quoted(url) + ")").toBool());
                evaluate("testWindow.dispatchDownloadRequest(" + ui + ")");
            } else submitAt(owner, ui, "download " + url);
        }
        if (behavior == "authentication") {
            QTRY_VERIFY(evaluate("testWindow.pendingAuthenticationRequest!==null").toBool());
            QCOMPARE(evaluate("testWindow.pendingPageDialogUi").value<QObject *>(), evaluate(ui).value<QObject *>());
            auto dialog = window->findChild<QObject *>("pageDialogPopup");
            QVERIFY(dialog);
            QVERIFY(dialog->setProperty("usernameText", QStringLiteral("owned")));
            QVERIFY(dialog->setProperty("passwordText", QStringLiteral("credential")));
            evaluate("testWindow.acceptPageDialog()");
        }
        QTRY_VERIFY(transfer);
        if (behavior != "native-start") QVERIFY(cookieMatched);
        // Find the public component by its busy/helper properties rather than
        // relying on QObject parenting (the native profile adopts each view).
        const auto helperExpression = workspace + ".children.filter(function(item){return item.helpers!==undefined})[0]";
        if (delayed) {
            QPointer<QObject> helper = evaluate(helperExpression + ".activeHelper").value<QObject *>();
            QVERIFY(helper);
            const auto token = evaluate(ui + ".download_request_token").toString();
            QVERIFY(!token.isEmpty());
            if (behavior == "cancel-preparation") {
                submitAt(owner, ui, "downloads");
                QTRY_VERIFY(visibleButton(owner->contentItem(), "Cancel request"));
                auto button = visibleButton(owner->contentItem(), "Cancel request");
                QTest::mouseClick(owner, Qt::LeftButton, Qt::NoModifier, button->mapToScene(QPointF(button->width()/2,button->height()/2)).toPoint());
            } else if (behavior == "retirement") {
                evaluate(host + ".windowShutdownApproved=true;" + host + ".close()");
            } else {
                submitAt(owner, ui, QString("open http://127.0.0.1:%1/after").arg(server.serverPort()));
            }
            QTRY_VERIFY(helper.isNull());
            QTRY_VERIFY(!transfer || transfer->state() == QAbstractSocket::UnconnectedState);
            QCOMPARE(evaluate("testWindow.profileResourceRegistry.downloadOwners.length").toInt(), 0);
            QCOMPARE(evaluate("testWindow.browserUi.download_rows().length").toInt(), 0);
            if (behavior != "navigation") QCOMPARE(evaluate("testWindow.browserUi.current_url").toString(), rootUrl);
            return;
        }
        QTRY_COMPARE(evaluate(host + ".pendingDownloadId.length>0").toBool(), behavior == "destination-cancel");
        if (behavior == "destination-cancel") {
            QTRY_COMPARE(evaluate(helperExpression + ".helpers.length").toInt(), 1);
            evaluate(host + ".cancelPendingDownload()");
            QTRY_COMPARE(evaluate(helperExpression + ".helpers.length").toInt(), 0);
            QTRY_COMPARE(evaluate(ui + ".download_rows()[0].state").toString(), QString("cancelled"));
            QTRY_VERIFY(!transfer || transfer->state() == QAbstractSocket::UnconnectedState);
            QCOMPARE(evaluate(ui + ".current_url").toString(), original);
            return;
        }
        QTRY_COMPARE(evaluate("Object.keys(" + host + ".activeDownloads).length").toInt(), 1);
        QTRY_COMPARE(evaluate(helperExpression + ".helpers.length").toInt(), 0);
        QTRY_COMPARE(evaluate(helperExpression + ".ownedHelpers.length").toInt(), 1);
        const auto id = evaluate("Object.keys(" + host + ".activeDownloads)[0]").toString();
        QTRY_VERIFY(evaluate(host + ".activeDownloads[" + quoted(id) + "].receivedBytes>=262144").toBool());
        QCOMPARE(evaluate(ui + ".current_url").toString(), original);
        if (behavior != "native-start") QCOMPARE(page(view, "window.ownedDownloadSentinel").toString(), QString("unchanged"));
        QCOMPARE(evaluate(ui + ".tab_count").toInt(), tabCount);
        if (!retrySource.isEmpty()) {
            QVERIFY(!evaluate(ui + ".execute_ui_action('browser.download.retry'," + quoted(retrySource) + ")").toBool());
            QVERIFY(evaluate(ui + ".status_text").toString().contains("already being retried"));
        }
        QVERIFY(transfer);
        qint64 extra = 0;
        const auto live = host + ".activeDownloads[" + quoted(id) + "]";
        const auto pathBeforeCompletion = evaluate(ui + ".download_rows().filter(function(row){return row.id===" + quoted(id) + "})[0].destination").toString();
        if (behavior.endsWith("retire-active")) {
            QVERIFY(evaluate(host + ".hasActiveDownloads()").toBool());
            evaluate(host + ".continueQuitRequest()");
            QVERIFY(evaluate(host + ".windowShutdownPromptVisible").toBool());
            evaluate(host + ".windowShutdownPromptVisible=false;" + host + ".windowShutdownApproved=true;" + host + ".close()");
            QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 1);
            QTRY_VERIFY(!transfer || transfer->state() == QAbstractSocket::UnconnectedState);
            QVERIFY(!QFileInfo::exists(pathBeforeCompletion));
            return;
        }
        if (behavior.endsWith("pause-resume") || behavior.endsWith("cancel-active")) {
            submitAt(owner, ui, "downloads");
            const auto label = behavior.endsWith("cancel-active") ? QString("Cancel") : QString("Pause");
            QTRY_VERIFY(visibleButton(owner->contentItem(), label));
            auto control = visibleButton(owner->contentItem(), label);
            if (behavior.endsWith("pause-resume")) {
                auto list = visual(owner->contentItem(), "downloadList");
                QVERIFY(list);
                QTRY_VERIFY(list->hasActiveFocus());
                QTest::keyClick(owner, Qt::Key_Tab);
                QTRY_COMPARE(owner->activeFocusItem(), control);
                QTest::keyClick(owner, Qt::Key_Return);
            } else {
                QTest::mouseClick(owner, Qt::LeftButton, Qt::NoModifier, control->mapToScene(QPointF(control->width()/2, control->height()/2)).toPoint());
            }
            if (behavior.endsWith("cancel-active")) {
                QTRY_COMPARE(evaluate(ui + ".download_rows()[0].state").toString(), QString("cancelled"));
                QTRY_COMPARE(evaluate(helperExpression + ".ownedHelpers.length").toInt(), 0);
                QTRY_VERIFY(!transfer || transfer->state() == QAbstractSocket::UnconnectedState);
                QVERIFY(!QFileInfo::exists(pathBeforeCompletion));
                QCOMPARE(evaluate(ui + ".current_url").toString(), original);
                return;
            }
            QTRY_VERIFY(evaluate(live + ".isPaused").toBool());
            QTRY_COMPARE(evaluate(ui + ".download_rows()[0].state").toString(), QString("paused"));
            const auto pausedBytes = evaluate(live + ".receivedBytes").toLongLong();
            transfer->write(QByteArray(65536, 'x')); extra = 65536;
            QTRY_COMPARE(transfer->bytesToWrite(), 0);
            QVERIFY(page(view, "document.readyState==='complete'").toBool());
            QCOMPARE(evaluate(live + ".receivedBytes").toLongLong(), pausedBytes);
            QTRY_VERIFY(visibleButton(owner->contentItem(), "Resume"));
            control = visibleButton(owner->contentItem(), "Resume");
            QTRY_COMPARE(owner->activeFocusItem(), control);
            QTest::keyClick(owner, Qt::Key_Return);
            QTRY_VERIFY(!evaluate(live + ".isPaused").toBool());
            QTRY_COMPARE(evaluate(ui + ".download_rows()[0].state").toString(), QString("in-progress"));
        }
        transfer->write(QByteArray(1048576 - 262144 - extra, 'x'));
        transfer->disconnectFromHost();
        QTRY_COMPARE(evaluate("Object.keys(" + host + ".activeDownloads).length").toInt(), 0);
        QTRY_VERIFY(evaluate(ui + ".download_rows().some(function(row){return row.id===" + quoted(id) + " && row.fileReady})").toBool());
        if (behavior.endsWith("pause-resume"))
            QTRY_VERIFY(owner->activeFocusItem() && owner->activeFocusItem()->isVisible()
                && owner->activeFocusItem()->isEnabled()
                && owner->activeFocusItem()->objectName() == "downloadOpenButton");
        const auto path = evaluate(ui + ".download_rows().filter(function(row){return row.id===" + quoted(id) + "})[0].destination").toString();
        QVERIFY(path.startsWith(destination + "/"));
        QTRY_COMPARE(evaluate(helperExpression + ".ownedHelpers.length").toInt(), 0);
        QFile result(path);
        QVERIFY(result.open(QIODevice::ReadOnly));
        QCOMPARE(result.readAll(), QByteArray(1048576, 'x'));
        QCOMPARE(finalRequests, behavior == "retry" || behavior == "retry-redirect" ? 2 : 1);
        if (behavior == "retry-redirect") QCOMPARE(redirectRequests, 2);
        QCOMPARE(evaluate(ui + ".current_url").toString(), original);
        if (host != "testWindow") QCOMPARE(evaluate("testWindow.browserUi.current_url").toString(), rootUrl);
        qInfo().noquote() << "Owned page-preserving download" << behavior << path << id;
    }

    void downloadFinalization_data() {
        QTest::addColumn<QString>("behavior");
        for (const auto name : {"complete", "cancel", "retire", "private-complete", "collision", "peer-close", "peer-cancel"})
            QTest::newRow(name) << QString::fromLatin1(name);
    }
    void downloadFinalization() {
        QFETCH(QString, behavior);
        const auto barrier = qEnvironmentVariable("FERRIC_TEST_SAVE_BARRIER_ROOT");
        if (barrier.isEmpty()) QSKIP("Run application_download_finalization for the owned sync interposer/watchdog.");
        struct Release {
            QString path;
            ~Release() { QFile file(path + "/release"); if (file.open(QIODevice::WriteOnly)) file.close(); }
        } release{barrier};
        for (const auto file : {"entered", "release", "left"}) QFile::remove(barrier + "/" + file);
        QString host = "testWindow", ui = "testWindow.browserUi";
        QQuickWindow *owner = window;
        if (behavior == "retire" || behavior == "private-complete") {
            owner = createChild("finishing-owner", behavior == "private-complete");
            engine->rootContext()->setContextProperty("ownedSaveHost", owner);
            auto object = evaluate("ownedSaveHost.windowUi").value<QObject *>();
            QVERIFY(object);
            engine->rootContext()->setContextProperty("ownedSaveUi", object);
            host = "ownedSaveHost"; ui = "ownedSaveUi";
        }
        const auto destination = evaluate(ui + ".default_download_directory()").toString();
        write(barrier + "/prefix", (destination + "/").toUtf8());
        // Measure GUI event dispatch, independently of Qt Quick's animation
        // clock. A QML Timer can miss this budget when offscreen frames slow
        // down even though the GUI thread continues dispatching events.
        QTimer eventLoopHeartbeat;
        eventLoopHeartbeat.setTimerType(Qt::PreciseTimer);
        eventLoopHeartbeat.setInterval(10);
        int eventLoopTicks = 0;
        connect(&eventLoopHeartbeat, &QTimer::timeout, &eventLoopHeartbeat,
                [&] { ++eventLoopTicks; });
        eventLoopHeartbeat.start();
        QTcpServer payload;
        QVERIFY(payload.listen(QHostAddress::LocalHost, 0));
        connect(&payload, &QTcpServer::newConnection, &payload, [&] {
            while (auto socket = payload.nextPendingConnection()) {
                connect(socket, &QTcpSocket::readyRead, socket, [socket] {
                    auto request = socket->property("request").toByteArray() + socket->readAll();
                    socket->setProperty("request", request);
                    if (!request.contains("\r\n\r\n") || socket->property("sent").toBool()) return;
                    socket->setProperty("sent", true);
                    socket->write("HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=Report.bin\r\nContent-Length: 1048576\r\nConnection: close\r\n\r\n");
                    socket->write(QByteArray(1048576, 'x'));
                    socket->disconnectFromHost();
                });
                connect(socket, &QTcpSocket::disconnected, socket, &QObject::deleteLater);
            }
        });
        submitAt(owner, ui, QString("open http://127.0.0.1:%1/file").arg(payload.serverPort()));
        QTRY_VERIFY_WITH_TIMEOUT(QFileInfo(barrier + "/entered").size() > 0, 5000);
        QFile entered(barrier + "/entered");
        QVERIFY(entered.open(QIODevice::ReadOnly));
        QVERIFY2(entered.readAll().toLongLong() != QCoreApplication::applicationPid(), "File sync ran on the GUI thread");
        const auto nativeTicks = eventLoopTicks;
        QElapsedTimer heartbeatElapsed;
        heartbeatElapsed.start();
        // Match the command-response budget below. Counting frames in a fixed
        // 120 ms window also measures unrelated window/render scheduling and
        // made the retirement row intermittent. Require repeated GUI progress
        // while the external watchdog still holds the worker for up to 10 s.
        const auto responsive = QTest::qWaitFor(
            [&] { return eventLoopTicks >= nativeTicks + 5; }, 1000);
        qInfo() << "Finalization heartbeat" << behavior
                << "event loop" << eventLoopTicks - nativeTicks
                << "elapsed ms" << heartbeatElapsed.elapsed();
        QVERIFY2(responsive && heartbeatElapsed.elapsed() < 1000,
                 "GUI event dispatch stalled while file sync was blocked");
        QVERIFY(!QFileInfo::exists(barrier + "/release"));
        QVERIFY(!QFileInfo::exists(barrier + "/left"));
        QElapsedTimer interaction; interaction.start();
        submitAt(owner, ui, "downloads");
        QVERIFY2(interaction.elapsed() < 1000, "Command input stalled while saving a file");
        QTRY_VERIFY(visual(owner->contentItem(), "downloadList"));
        QTRY_VERIFY(evaluate(ui + ".download_rows().some(function(row){return row.finishing && row.active && !row.fileReady})").toBool());
        const auto id = evaluate(ui + ".download_rows().filter(function(row){return row.finishing})[0].id").toString();
        const auto path = evaluate(ui + ".download_rows().filter(function(row){return row.finishing})[0].destination").toString();
        QVERIFY(!QFileInfo::exists(path));
        QVERIFY(evaluate("testWindow.profileResourceRegistry.downloadFor(" + ui + "," + quoted(id) + ")===null").toBool());
        QTRY_COMPARE(evaluate(host + ".downloadsModel.get(0).bytes").toLongLong(), 1048576LL);
        QCOMPARE(evaluate(host + ".downloadsModel.get(0).total").toLongLong(), 1048576LL);
        QCOMPARE(evaluate(host + ".downloadsModel.get(0).speed").toLongLong(), 0LL);
        evaluate(host + ".continueQuitRequest()");
        const auto prompt = host == "testWindow" ? ".shutdownPromptVisible" : ".windowShutdownPromptVisible";
        QVERIFY(evaluate(host + prompt).toBool());
        evaluate(host + prompt + "=false");
        if (behavior == "peer-close" || behavior == "peer-cancel") {
            // Keep the ownership check separate from asynchronous page-load
            // confirmation in the newly created peer.
            submit("window-new");
            QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
            QTRY_VERIFY(evaluate("testWindow.browserWindowRegistry[1].host.profileBootstrapComplete && !testWindow.browserWindowRegistry[1].ui.profile_bootstrap_pending").toBool());
            QVERIFY(evaluate("testWindow.browserWindowRegistry[1].profile===testWindow.browserProfile").toBool());
            auto peer = qobject_cast<QQuickWindow *>(evaluate("testWindow.browserWindowRegistry[1].host").value<QObject *>());
            QVERIFY(peer);
            engine->rootContext()->setContextProperty("savePeer", peer);
            submitAt(peer, "savePeer.windowUi", "open rb://blank");
            QTRY_VERIFY(evaluate("savePeer.activeWebView() && savePeer.activeWebView().nativeUrl==='rb://blank' && savePeer.activeWebView().ownsTab()").toBool());
            QVERIFY(!evaluate("savePeer.windowUi.has_finalizing_downloads()").toBool());
            QVERIFY(!evaluate("savePeer.hasActiveDownloads()").toBool());
            QCOMPARE(evaluate("testWindow.profileResourceRegistry.finishingForHost(savePeer).count").toInt(), 0);
            QVERIFY(evaluate(ui + ".has_finalizing_downloads()").toBool());
            if (behavior == "peer-close") {
                evaluate("savePeer.continueQuitRequest()");
                const auto state = QString("(()=>{if(!savePeer)return 'closed';const v=savePeer.activeWebView();return JSON.stringify({released:savePeer.runtimeReleased,visible:savePeer.visible,status:String(savePeer.windowUi.status_text),shutdown:savePeer.windowShutdownPromptVisible,page:savePeer.windowShutdownPagePromptVisible,reason:savePeer.windowShutdownPagePromptReason,storage:savePeer.windowShutdownStoragePromptVisible,history:savePeer.windowUi.download_history_status(),native:v?String(v.nativeUrl):'',owns:!!v&&v.ownsTab(),frame:!!v&&v.mainFrame.isValid})})()");
                qInfo().noquote() << "Peer close initial state" << evaluate(state).toString();
                QTRY_VERIFY2(evaluate("testWindow.browserWindowRegistry.length").toInt()==1, qPrintable(evaluate(state).toString()));
                QVERIFY(evaluate(ui + ".download_finalization_pending(" + quoted(id) + ")").toBool());
            } else {
                QVERIFY(evaluate("testWindow.profileResourceRegistry.controlDownload(savePeer.windowUi," + quoted(id) + ",'cancel')").toBool());
                QTRY_COMPARE(evaluate(ui + ".download_rows()[0].state").toString(), QString("cancelled"));
                QVERIFY(!QFileInfo::exists(path));
            }
        }
        if (behavior == "cancel") {
            QTRY_VERIFY(visibleButton(owner->contentItem(), "Cancel"));
            auto button = visibleButton(owner->contentItem(), "Cancel");
            QTest::mouseClick(owner, Qt::LeftButton, Qt::NoModifier, button->mapToScene(QPointF(button->width()/2,button->height()/2)).toPoint());
            QTRY_COMPARE(evaluate(ui + ".download_rows()[0].state").toString(), QString("cancelled"));
            QVERIFY(!QFileInfo::exists(path));
        } else if (behavior == "retire") {
            interaction.restart();
            evaluate(host + ".windowShutdownApproved=true;" + host + ".close()");
            QVERIFY2(interaction.elapsed() < 1000, "Retiring owner joined its blocked file worker");
        } else if (behavior == "collision") {
            write(path, "existing file");
        }
        write(barrier + "/release", "released");
        QTRY_VERIFY(QFileInfo::exists(barrier + "/left"));
        if (behavior == "retire") {
            QTest::qWait(150);
            QVERIFY(!QFileInfo::exists(path));
        } else {
            QTRY_VERIFY(!evaluate(ui + ".download_finalization_pending(" + quoted(id) + ")").toBool());
            if (behavior == "cancel" || behavior == "peer-cancel") QVERIFY(!QFileInfo::exists(path));
            else if (behavior == "collision") {
                QTRY_COMPARE(evaluate(ui + ".download_rows()[0].state").toString(), QString("interrupted"));
                QFile file(path); QVERIFY(file.open(QIODevice::ReadOnly));
                QCOMPARE(file.readAll(), QByteArray("existing file"));
                QVERIFY(!evaluate(ui + ".download_rows()[0].fileReady").toBool());
                QVERIFY(evaluate(ui + ".download_rows()[0].reason").toString().contains("already exists"));
            } else {
                QTRY_VERIFY(evaluate(ui + ".download_rows()[0].fileReady").toBool());
                QFile file(path); QVERIFY(file.open(QIODevice::ReadOnly));
                QCOMPARE(file.readAll(), QByteArray(1048576, 'x'));
                QCOMPARE(evaluate(ui + ".download_rows()[0].total").toString(), QString("1048576"));
            }
        }
        QTRY_VERIFY(QDir(destination).entryList({".ferric-download-*.partial"}, QDir::Files | QDir::Hidden).isEmpty());
    }

    void downloadProgressTracksItsTransport_data() {
        QTest::addColumn<QString>("behavior");
        QTest::newRow("progress") << QString("progress");
        QTest::newRow("cancel") << QString("cancel");
        for (const auto &kind :
             {"pause-resume", "foreign-profile", "peer-profile", "owner-retirement",
              "decision-once", "private-cancel", "private-pause-resume", "private-complete",
              "history-busy", "runtime-folder", "script-history-busy", "private-script-allowed",
              "private-script-denied"})
            QTest::newRow(kind) << QString::fromLatin1(kind);
    }
    void downloadProgressTracksItsTransport() {
        QFETCH(QString, behavior);
        const auto initialDestination = QFileInfo(configPath).absolutePath() + "/owned-downloads";
        auto destination = initialDestination;
        // Assert a safe owned startup directory before any transfer. The
        // runtime-folder case then changes it through actual command entry.
        qInfo().noquote()
            << "Owned startup download directory control:"
            << evaluate("testWindow.browserUi.default_download_directory()").toString();
        QTRY_COMPARE(evaluate("testWindow.browserUi.default_download_directory()").toString(),
                     destination);
        QTRY_VERIFY(!evaluate("testWindow.downloadsAskDestination()").toBool());
        if (behavior == "runtime-folder") {
            destination =
                QFileInfo(configPath).absolutePath() + QString::fromUtf8("/saved reports/résumé λ");
            submit("set --temp downloads.directory=" + quoted(destination));
            QTRY_COMPARE(evaluate("testWindow.browserUi.default_download_directory()").toString(),
                         destination);
            submit("set --temp downloads.directory=relative/folder");
            QTRY_VERIFY(evaluate("testWindow.browserUi.status_text")
                            .toString()
                            .contains("absolute folder path"));
            QCOMPARE(evaluate("testWindow.browserUi.default_download_directory()").toString(),
                     destination);
        }
        const bool scripted = behavior.contains("script");
        const bool scriptAllowed = behavior != "private-script-denied";
        const auto scriptId = QString("userscript.download_fixture.download");
        const auto scriptOutput = QFileInfo(configPath).absolutePath() + "/script-context.json";
        if (scripted) {
            const auto manifestPath =
                QFileInfo(configPath).absolutePath() + "/download_fixture.toml";
            const QJsonArray argv{
                "-c",
                "cat > \"$1\"; printf '%s' '{\"type\":\"message\",\"text\":\"Fixture done\"}'",
                "fixture", scriptOutput};
            const auto manifest =
                QByteArray(
                    "schema_version=1\nname=\"download_fixture\"\nexecutable=\"/bin/sh\"\nargv=") +
                QJsonDocument(argv).toJson(QJsonDocument::Compact) +
                "\ncontext_fields=[\"download_id\",\"url\",\"private\"]\nallowed_results=["
                "\"message\"]\nallow_private=" +
                (scriptAllowed ? "true" : "false") +
                "\n[[actions]]\nid=\"userscript.download_fixture.download\"\nsubject="
                "\"download\"\nverb=\"inspect-download\"\nlabel=\"Inspect "
                "download\"\nrequired_fields=[\"download_id\"]\n";
            write(manifestPath, manifest);
            QTRY_VERIFY(!evaluate("testWindow.browserUi.userscript_inventory_pending").toBool());
            QVERIFY(evaluate("testWindow.browserUi.install_userscript_manifest(" +
                             quoted(manifestPath) + ")")
                        .toBool());
            QTRY_COMPARE(evaluate("testWindow.browserUi.userscript_install_state").toString(),
                         QString("installed"));
        }
        QQuickWindow *peer = nullptr;
        QString peerUi;
        QString allocatorHost = "testWindow";
        QString allocatorUi = "testWindow.browserUi";
        if (behavior == "peer-profile" || behavior == "owner-retirement") {
            submit("window-new");
            QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
            QTRY_VERIFY(evaluate("testWindow.browserWindowRegistry[1].host."
                                 "profileBootstrapComplete && "
                                 "!testWindow.browserWindowRegistry[1].ui.profile_"
                                 "bootstrap_pending")
                            .toBool());
            QVERIFY(evaluate("testWindow.browserWindowRegistry[1].profile==="
                             "testWindow.browserProfile")
                        .toBool());
            peer = qobject_cast<QQuickWindow *>(
                evaluate("testWindow.browserWindowRegistry[1].host").value<QObject *>());
            QVERIFY(peer);
            peerUi = "testWindow.browserWindowRegistry[1].ui";
        } else if (behavior == "foreign-profile" || behavior == "script-history-busy" ||
                   behavior.startsWith("private-")) {
            peer = createChild("foreign-download", true);
            QVERIFY(peer);
            QVERIFY(evaluate("testWindow.browserWindowRegistry[1].profile.offTheRecord").toBool());
            QVERIFY(evaluate("testWindow.browserWindowRegistry[1].profile!=="
                             "testWindow.browserProfile")
                        .toBool());
            peerUi = "testWindow.browserWindowRegistry[1].ui";
        }
        if (peer) {
            QVERIFY(peer != window);
            auto uiObject = evaluate(peerUi).value<QObject *>();
            QVERIFY(uiObject);
            QVERIFY(uiObject != evaluate("testWindow.browserUi").value<QObject *>());
            engine->rootContext()->setContextProperty("ownedDownloadPeer", peer);
            engine->rootContext()->setContextProperty("ownedDownloadPeerUi", uiObject);
            peerUi = "ownedDownloadPeerUi";
            QVERIFY(
                evaluate(peerUi + ".window_token!==testWindow.browserUi.window_token").toBool());
        }
        if (behavior == "owner-retirement" || behavior.startsWith("private-")) {
            allocatorHost = "ownedDownloadPeer";
            allocatorUi = peerUi;
            QTRY_COMPARE(evaluate(allocatorUi + ".default_download_directory()").toString(),
                         destination);
        }
        QPointer<QTcpSocket> transportPeer;
        QTcpServer transfers;
        QVERIFY(transfers.listen(QHostAddress::LocalHost, 0));
        connect(&transfers, &QTcpServer::newConnection, &transfers, [&transfers, &transportPeer] {
            while (auto socket = transfers.nextPendingConnection()) {
                connect(socket, &QTcpSocket::readyRead, socket, [socket, &transportPeer] {
                    auto request =
                        socket->property("ownedRequest").toByteArray() + socket->readAll();
                    socket->setProperty("ownedRequest", request);
                    if (!request.contains("\r\n\r\n") || socket->property("ownedSent").toBool())
                        return;
                    socket->setProperty("ownedSent", true);
                    if (!request.startsWith("GET /owned-progress ")) {
                        socket->write("HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n");
                        socket->disconnectFromHost();
                        return;
                    }
                    transportPeer = socket;
                    // Deliberately hold this owned transport before completion.
                    // Enough body bytes arrive to expose the live progress count.
                    socket->write("HTTP/1.1 200 OK\r\nContent-Type: "
                                  "application/octet-stream\r\n"
                                  "Content-Disposition: attachment; filename=\"Power user "
                                  "report.bin\"\r\n"
                                  "Content-Length: 1048576\r\nConnection: close\r\n\r\n");
                    socket->write(QByteArray(262144, 'x'));
                });
                connect(socket, &QTcpSocket::disconnected, socket, &QObject::deleteLater);
            }
        });
        const auto open =
            QString("open http://127.0.0.1:%1/owned-progress").arg(transfers.serverPort());
        if (allocatorHost != "testWindow")
            submitAt(peer, allocatorUi, open);
        else
            submit(open);
        const auto active = allocatorHost + ".activeDownloads";
        QTRY_VERIFY(evaluate("Object.keys(" + active + ").length===1").toBool());
        const auto id = evaluate("Object.keys(" + active + ")[0]").toString();
        const auto live = active + "[" + quoted(id) + "]";
        QTRY_VERIFY(evaluate(live + ".receivedBytes>=8192").toBool());
        const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
        if (behavior == "owner-retirement") {
            const auto primaryUrl = evaluate("testWindow.browserUi.current_url").toString();
            QPointer<QObject> request = evaluate(live).value<QObject *>();
            QVERIFY(request);
            evaluate(allocatorHost + ".windowShutdownApproved=true;" + allocatorHost + ".close();" +
                     allocatorHost + ".destroy()");
            QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 1);
            QElapsedTimer retire;
            retire.start();
            while (transportPeer && transportPeer->state() != QAbstractSocket::UnconnectedState &&
                   retire.elapsed() < 3000)
                QTest::qWait(10);
            const bool networkClosed =
                !transportPeer || transportPeer->state() == QAbstractSocket::UnconnectedState;
            const bool requestFinished = !request || request->property("isFinished").toBool();
            const QJsonObject facts{
                {"sameProfile", true},
                {"allocatorRetired", true},
                {"networkClosed", networkClosed},
                {"requestFinished", requestFinished},
                {"primaryReady", evaluate("testWindow.browserUi.profile_available()").toBool()}};
            qInfo().noquote() << QJsonDocument(facts).toJson(QJsonDocument::Compact);
            if (!artifacts.isEmpty())
                write(artifacts + "/owner-retirement.json", QJsonDocument(facts).toJson());
            QVERIFY(networkClosed);
            QVERIFY(requestFinished);
            QVERIFY(evaluate("testWindow.browserUi.profile_available()").toBool());
            QCOMPARE(evaluate("testWindow.browserUi.current_url").toString(), primaryUrl);
            return;
        }
        if (behavior == "foreign-profile" || behavior == "peer-profile") {
            QVERIFY(evaluate("typeof "
                             "testWindow.profileResourceRegistry.controlDownload==='function'")
                        .toBool());
            if (behavior == "foreign-profile") {
                QVERIFY(!evaluate("testWindow.profileResourceRegistry.controlDownload(" + peerUi +
                                  "," + quoted(id) + ",'cancel')")
                             .toBool());
                QVERIFY(!evaluate(live + ".isFinished").toBool());
                QCOMPARE(evaluate("Object.keys(testWindow.activeDownloads).length").toInt(), 1);
                QVERIFY(transportPeer && transportPeer->state() == QAbstractSocket::ConnectedState);
            } else {
                QVERIFY(evaluate("testWindow.profileResourceRegistry.downloadOwnerFor(" + peerUi +
                                 "," + quoted(id) + ").ui===testWindow.browserUi")
                            .toBool());
                QVERIFY(evaluate("testWindow.profileResourceRegistry.controlDownload(" + peerUi +
                                 "," + quoted(id) + ",'pause')")
                            .toBool());
                QTRY_VERIFY(evaluate(live + ".isPaused").toBool());
                QVERIFY(evaluate("testWindow.profileResourceRegistry.controlDownload(" + peerUi +
                                 "," + quoted(id) + ",'resume')")
                            .toBool());
                QTRY_VERIFY(!evaluate(live + ".isPaused").toBool());
            }
            QVERIFY(evaluate("testWindow.profileResourceRegistry.controlDownload("
                             "testWindow.browserUi," +
                             quoted(id) + ",'cancel')")
                        .toBool());
            QTRY_COMPARE(evaluate("Object.keys(testWindow.activeDownloads).length").toInt(), 0);
            const QJsonObject facts{
                {"case", behavior}, {"policyPassed", true}, {"ownerCancelPassed", true}};
            qInfo().noquote() << QJsonDocument(facts).toJson(QJsonDocument::Compact);
            if (!artifacts.isEmpty())
                write(artifacts + "/" + behavior + ".json", QJsonDocument(facts).toJson());
            return;
        }
        auto managerHost = allocatorHost == "testWindow" ? window : peer;
        std::unique_ptr<sqlite3, decltype(&sqlite3_close)> historyLock(nullptr, sqlite3_close);
        if (behavior == "history-busy" || behavior == "script-history-busy") {
            QTRY_COMPARE(evaluate("testWindow.browserUi.download_history_status()").toString(),
                         QString("ready"));
            QDirIterator databases(QFileInfo(configPath).absolutePath(), {"browser.sqlite"},
                                   QDir::Files, QDirIterator::Subdirectories);
            QVERIFY(databases.hasNext());
            const auto database = databases.next();
            QVERIFY(!databases.hasNext());
            sqlite3 *connection = nullptr;
            QCOMPARE(sqlite3_open_v2(database.toUtf8().constData(), &connection,
                                     SQLITE_OPEN_READWRITE, nullptr),
                     SQLITE_OK);
            historyLock.reset(connection);
            QCOMPARE(sqlite3_exec(connection, "BEGIN EXCLUSIVE", nullptr, nullptr, nullptr),
                     SQLITE_OK);
            // WAL readers can run during an exclusive writer. Hold the one
            // metadata worker in a captured native progress write, then queue a
            // history read behind it. UI controls must remain usable throughout.
            QVERIFY(evaluate("testWindow.browserUi.update_download(" + quoted(id) +
                             ", 'in-progress', " + live + ".receivedBytes, false)")
                        .toBool());
            evaluate("testWindow.browserUi.refresh_library()");
            QTRY_VERIFY(evaluate("testWindow.browserUi.library_pending").toBool());
        }
        if (managerHost == window)
            submit("downloads");
        else
            submitAt(managerHost, allocatorUi, "downloads");
        QTRY_VERIFY(visual(managerHost->contentItem(), "downloadList"));
        QTRY_COMPARE(evaluate(allocatorHost + ".downloadsModel.count").toInt(), 1);
        const auto transport = evaluate(live + ".receivedBytes").toLongLong();
        qint64 displayed = 0;
        QElapsedTimer progress;
        progress.start();
        do {
            displayed =
                evaluate("Number(" + allocatorHost + ".downloadsModel.get(0).bytes)").toLongLong();
            if (displayed >= transport)
                break;
            QTest::qWait(10);
        } while (progress.elapsed() < 3000);
        const QJsonObject facts{
            {"id", id},
            {"transportBytes", double(transport)},
            {"displayedBytes", double(displayed)},
            {"state", evaluate(allocatorHost + ".downloadsModel.get(0).state").toString()},
            {"destination",
             evaluate(allocatorHost + ".downloadsModel.get(0).destination").toString()}};
        qInfo().noquote() << QJsonDocument(facts).toJson(QJsonDocument::Compact);

        if (!artifacts.isEmpty()) {
            write(artifacts + "/" + behavior + "-progress.json", QJsonDocument(facts).toJson());
            QSignalSpy frame(managerHost, &QQuickWindow::frameSwapped);
            managerHost->update();
            QVERIFY(frame.wait(2000));
            QVERIFY(managerHost->grabWindow().save(artifacts + "/" + behavior + "-progress.png"));
        }
        if (scripted) {
            QVERIFY(
                evaluate(allocatorUi + ".select_userscript_action_subject('download')").toBool());
            QTRY_VERIFY(evaluate(allocatorUi + ".userscript_action_ids.indexOf(" +
                                 quoted(scriptId) + ")>=0")
                            .toBool());
            QCOMPARE(evaluate(allocatorUi + ".userscript_action_availability[" + allocatorUi +
                              ".userscript_action_ids.indexOf(" + quoted(scriptId) + ")]")
                         .toString(),
                     scriptAllowed ? QString("true") : QString("false"));
            if (behavior == "script-history-busy") {
                QVERIFY(evaluate("testWindow.browserUi.library_pending").toBool());
                QVERIFY(!evaluate(peerUi + ".execute_ui_action(" + quoted(scriptId) + "," +
                                  quoted(id) + ")")
                             .toBool());
                QCOMPARE(
                    evaluate(peerUi + ".status_text").toString(),
                    QString(
                        "This download is no longer available. Refresh Downloads and try again."));
                QVERIFY(!QFileInfo::exists(scriptOutput));
            }
            const auto accepted = evaluate(allocatorUi + ".execute_ui_action(" + quoted(scriptId) +
                                           "," + quoted(id) + ")")
                                      .toBool();
            QCOMPARE(accepted, scriptAllowed);
            if (scriptAllowed) {
                QJsonObject input;
                QTRY_VERIFY(([&] {
                    QFile result(scriptOutput);
                    if (!result.open(QIODevice::ReadOnly))
                        return false;
                    input = QJsonDocument::fromJson(result.readAll()).object();
                    return input.contains("context") && input.contains("invocation_id");
                })());
                const auto context = input.value("context").toObject();
                QCOMPARE(context.value("download_id").toString(), id);
                QCOMPARE(context.value("private").toBool(), behavior.startsWith("private-"));
                QCOMPARE(context.value("url").toString(), open.mid(QString("open ").size()));
            } else {
                QCOMPARE(
                    evaluate(allocatorUi + ".status_text").toString(),
                    QString("This userscript isn't enabled for private or temporary profiles."));
                QVERIFY(!QFileInfo::exists(scriptOutput));
            }
        }
        if (behavior == "progress") {
            QVERIFY2(displayed >= transport,
                     "Active download progress must reflect received transport bytes");
            QVERIFY(transportPeer);
            transportPeer->write(QByteArray(131072, 'x'));
            QTRY_VERIFY(evaluate(live + ".receivedBytes>=393216").toBool());
            QTRY_VERIFY(
                evaluate("Number(" + allocatorHost + ".downloadsModel.get(0).bytes)>=393216")
                    .toBool());
            // The backend session also receives native progress, independently
            // of the row's direct transport metrics and history acknowledgement.
            QTRY_VERIFY(
                evaluate("(function(){var rows=" + allocatorUi +
                         ".download_rows();for(var i=0;i<rows.length;i++){if(rows[i].id===" +
                         quoted(id) +
                         " && Number(rows[i].bytes)>=393216)return true;}return false;})()")
                    .toBool());
            return;
        }
        if (behavior == "pause-resume" || behavior == "private-pause-resume") {
            QTRY_VERIFY(visibleButton(managerHost->contentItem(), "Pause"));
            auto pause = visibleButton(managerHost->contentItem(), "Pause");
            QTest::mouseClick(
                managerHost, Qt::LeftButton, Qt::NoModifier,
                pause->mapToScene(QPointF(pause->width() / 2, pause->height() / 2)).toPoint());
            QTRY_VERIFY(evaluate(live + ".isPaused").toBool());
            QTRY_VERIFY(
                evaluate(allocatorHost + ".downloadsModel.get(0).state==='paused'").toBool());
            QTRY_VERIFY(visibleButton(managerHost->contentItem(), "Resume"));
            auto resume = visibleButton(managerHost->contentItem(), "Resume");
            QTest::mouseClick(
                managerHost, Qt::LeftButton, Qt::NoModifier,
                resume->mapToScene(QPointF(resume->width() / 2, resume->height() / 2)).toPoint());
            QTRY_VERIFY(!evaluate(live + ".isPaused").toBool());
            QTRY_VERIFY(
                evaluate(allocatorHost + ".downloadsModel.get(0).state==='in-progress'").toBool());
        }
        if (behavior == "decision-once") {
            QVERIFY(!evaluate("testWindow.resolveQtRequest(testWindow.browserUi," + live +
                              ",'download','accept',[])")
                         .toBool());
            QCOMPARE(evaluate("testWindow.resolvedQtRequests.filter(function(entry){return "
                              "entry.kind==='download'}).length")
                         .toInt(),
                     1);
            QVERIFY(!evaluate(live + ".isFinished").toBool());
        }
        if (behavior == "history-busy" || behavior == "script-history-busy") {
            QVERIFY(evaluate("testWindow.browserUi.library_pending").toBool());
        }
        if (behavior == "private-complete" || behavior == "runtime-folder") {
            QVERIFY(transportPeer);
            transportPeer->write(QByteArray(1048576 - 262144, 'x'));
            transportPeer->disconnectFromHost();
            QTRY_COMPARE(evaluate("Object.keys(" + active + ").length").toInt(), 0);
            QTRY_VERIFY(evaluate(allocatorHost + ".downloadsModel.count===1 && " + allocatorHost +
                                 ".downloadsModel.get(0).fileReady")
                            .toBool());
            const auto savedPath =
                evaluate(allocatorHost + ".downloadsModel.get(0).destination").toString();
            QCOMPARE(QFileInfo(savedPath).size(), qint64(1048576));
            QFile completed(savedPath);
            QVERIFY(completed.open(QIODevice::ReadOnly));
            QCOMPARE(completed.readAll(), QByteArray(1048576, 'x'));
            QTRY_COMPARE(evaluate(allocatorHost + ".downloadsModel.get(0).bytes").toString(),
                         QString("1048576"));
            if (!artifacts.isEmpty()) {
                QSignalSpy frame(managerHost, &QQuickWindow::frameSwapped);
                managerHost->update();
                QVERIFY(frame.wait(2000));
                QVERIFY(
                    managerHost->grabWindow().save(artifacts + "/" + behavior + "-completed.png"));
            }
            QVERIFY(evaluate(allocatorUi + ".download_desktop_action(" + quoted(id) + ",false)")
                        .toBool());
            // Resolve the real action through its capability/command path,
            // then consume its typed effect without launching a desktop app.
            const auto reveal =
                evaluate("(function(){var ui=" + allocatorUi +
                         ";var accepted=ui.execute_ui_action('browser.download.show'," +
                         quoted(id) + ");return [accepted,ui.take_engine_action()];})()")
                    .toList();
            QCOMPARE(reveal.size(), 2);
            QVERIFY(reveal[0].toBool());
            const auto effect = reveal[1].toString();
            QVERIFY(effect.startsWith("download-show\t"));
            QCOMPARE(QUrl(effect.mid(QString("download-show\t").size())).toLocalFile(),
                     QFileInfo(savedPath).absolutePath());
            if (behavior == "private-complete") {
                QVERIFY(
                    evaluate("testWindow.browserUi.list_downloads().indexOf(" + quoted(id) + ")<0")
                        .toBool());
                QVERIFY(!evaluate("testWindow.downloadManagerVisible").toBool());
            } else {
                QCOMPARE(QFileInfo(savedPath).absolutePath(), destination);
                QVERIFY(!QFileInfo::exists(initialDestination + "/Power user report.bin"));
                QVERIFY(
                    evaluate("testWindow.browserUi.list_downloads().indexOf(" + quoted(id) + ")>=0")
                        .toBool());
            }
            return;
        }
        QQuickItem *cancel = nullptr;
        QTRY_VERIFY((cancel = visibleButton(managerHost->contentItem(), "Cancel")) &&
                    cancel->isEnabled() && cancel->width() > 0 && cancel->height() > 0);
        QSignalSpy cancelFrame(managerHost, &QQuickWindow::frameSwapped);
        managerHost->requestUpdate();
        QTRY_VERIFY(!cancelFrame.isEmpty());
        QTest::mouseClick(
            managerHost, Qt::LeftButton, Qt::NoModifier,
            cancel->mapToScene(QPointF(cancel->width() / 2, cancel->height() / 2)).toPoint());
        QElapsedTimer cancellation;
        cancellation.start();
        while (evaluate("Object.keys(" + active + ").length>0").toBool() &&
               cancellation.elapsed() < 3000)
            QTest::qWait(10);
        const auto activeCount = evaluate("Object.keys(" + active + ").length").toInt();
        const QJsonObject cancelFacts{
            {"activeAfterClick", activeCount},
            {"status", evaluate(allocatorUi + ".status_text").toString()},
            {"qtFinished", activeCount > 0 ? evaluate(live + ".isFinished").toBool() : true},
            {"acceptedRequestWasTracked",
             evaluate("testWindow.resolvedQtRequests.some(function(entry){"
                      "return entry.request && entry.kind==='download'})")
                 .toBool()}};
        qInfo().noquote() << QJsonDocument(cancelFacts).toJson(QJsonDocument::Compact);
        if (!artifacts.isEmpty())
            write(artifacts + "/" + behavior + "-cancel.json", QJsonDocument(cancelFacts).toJson());
        QCOMPARE(activeCount, 0);
        if (historyLock) {
            QCOMPARE(sqlite3_exec(historyLock.get(), "ROLLBACK", nullptr, nullptr, nullptr),
                     SQLITE_OK);
            historyLock.reset();
            QTRY_VERIFY(!evaluate("testWindow.browserUi.library_pending").toBool());
        }
    }
    void downloadStreamPolicy_data() {
        QTest::addColumn<QString>("behavior");
        QTest::addColumn<bool>("privateProfile");
        for (bool transient : {false, true})
            for (const auto kind : {"service-worker", "policy-block", "redirect-policy-block", "http-error", "redirect-loop", "truncated", "unknown-total", "binary", "empty", "utf8-name"}) {
                const auto tag = QByteArray(transient ? "private-" : "regular-") + kind;
                QTest::newRow(tag.constData()) << QString::fromLatin1(kind) << transient;
            }
    }
    void downloadStreamPolicy() {
        QFETCH(QString, behavior);
        QFETCH(bool, privateProfile);
        QQuickWindow *owner = window;
        QString host = "testWindow", ui = "testWindow.browserUi";
        if (privateProfile) {
            owner = createChild("stream-policy", true);
            engine->rootContext()->setContextProperty("streamPolicyHost", owner);
            engine->rootContext()->setContextProperty("streamPolicyUi", evaluate("streamPolicyHost.windowUi").value<QObject *>());
            host = "streamPolicyHost"; ui = "streamPolicyUi";
        }
        QByteArray binary;
        for (int index = 0; index < 8192; ++index) binary.append(char(index % 256));
        int fileRequests = 0, redirectRequests = 0;
        QTcpServer payload;
        QVERIFY(payload.listen(QHostAddress::LocalHost, 0));
        const auto origin = QString("http://127.0.0.1:%1").arg(payload.serverPort());
        connect(&payload, &QTcpServer::newConnection, &payload, [&] {
            while (auto socket = payload.nextPendingConnection()) {
                connect(socket, &QTcpSocket::readyRead, socket, [&, socket] {
                    auto request = socket->property("request").toByteArray() + socket->readAll();
                    socket->setProperty("request", request);
                    if (!request.contains("\r\n\r\n") || socket->property("sent").toBool()) return;
                    socket->setProperty("sent", true);
                    const auto path = request.split(' ').value(1).split('?').value(0);
                    QByteArray body, mime = "application/octet-stream", status = "200 OK", extra;
                    if (path == "/site") {
                        mime = "text/html";
                        body = "<!doctype html><title>Owned stream site</title><body><p>Unchanged page</p>";
                        if (behavior == "service-worker") body +=
                            "<script>navigator.serviceWorker.register('/sw.js').then(()=>navigator.serviceWorker.ready).then(async()=>{"
                            "if(!navigator.serviceWorker.controller)await new Promise(resolve=>navigator.serviceWorker.addEventListener('controllerchange',resolve,{once:true}));"
                            "const text=await fetch('/file?preview').then(r=>r.text());document.body.dataset.worker=text;"
                            "document.title='Worker controls website'}).catch(e=>document.body.dataset.workerError=e.name)</script>";
                    } else if (path == "/sw.js") {
                        mime = "application/javascript";
                        body = "self.addEventListener('install',()=>self.skipWaiting());self.addEventListener('activate',e=>e.waitUntil(self.clients.claim()));"
                               "self.addEventListener('fetch',e=>{if(new URL(e.request.url).pathname==='/file')e.respondWith(new Response('site-worker-body',{status:200}))})";
                    } else if (path == "/redirect" || path == "/loop") {
                        ++redirectRequests;
                        status = "302 Found";
                        const auto next = behavior == "redirect-policy-block"
                            ? QString("http://localhost:%1/file").arg(payload.serverPort())
                            : origin + (path == "/redirect" ? "/loop" : "/redirect");
                        extra = "Location: " + next.toUtf8() + "\r\n";
                    } else if (path == "/file") {
                        ++fileRequests;
                        body = behavior == "empty" ? QByteArray() : binary;
                        if (behavior == "http-error") status = "404 Not Found";
                        if (behavior == "utf8-name") extra += "Content-Disposition: attachment; filename=old.bin; filename*=UTF-8'en'R%C3%A9sum%C3%A9.bin\r\n";
                    } else { status = "404 Not Found"; }
                    socket->write("HTTP/1.1 " + status + "\r\nContent-Type: " + mime + "\r\n" + extra);
                    if (behavior != "unknown-total" || path != "/file")
                        socket->write("Content-Length: " + QByteArray::number(body.size() + ((behavior == "truncated" && path == "/file") ? 500 : 0)) + "\r\n");
                    socket->write("Connection: close\r\n\r\n" + body);
                    socket->disconnectFromHost();
                });
                connect(socket, &QTcpSocket::disconnected, socket, &QObject::deleteLater);
            }
        });
        const auto original = origin + "/site";
        submitAt(owner, ui, "open " + original);
        const auto view = host + ".activeWebView()";
        QTRY_VERIFY(evaluate("String(" + view + ".url)===" + quoted(original) + " && !" + view + ".loading && " + view + ".mainFrame.isValid && !" + view + ".nativeUrl").toBool());
        QTRY_VERIFY(page(view, "location.href===" + quoted(original) + " && document.readyState==='complete'").toBool());
        if (behavior == "service-worker") {
            QTRY_COMPARE(evaluate(view + ".title").toString(), QString("Worker controls website"));
            QCOMPARE(page(view, "document.body.dataset.worker").toString(), QString("site-worker-body"));
            QCOMPARE(fileRequests, 0); // The site's own fetch was actually intercepted.
        }
        const auto title = evaluate(view + ".title").toString();
        const auto tabs = evaluate(ui + ".tab_count").toInt();
        if (behavior == "policy-block" || behavior == "redirect-policy-block") {
            const auto blocked = behavior == "policy-block" ? "127.0.0.1" : "localhost";
            evaluate("(()=>{const g=testWindow.profileResourceRegistry.forProfile(" + view + ".profile);g.interceptor.securityDenyHosts=[" + quoted(blocked) + "];g.interceptor.enabled=true})()");
        }
        const auto target = origin + ((behavior == "redirect-policy-block" || behavior == "redirect-loop") ? "/redirect" : "/file");
        submitAt(owner, ui, "download " + target);
        const auto helper = host + ".downloadWorkspaceObject.children.filter(function(item){return item.ownedHelpers!==undefined})[0]";
        QTRY_VERIFY(!evaluate(ui + ".download_request_token").toString().isEmpty());
        QTRY_VERIFY(evaluate(ui + ".download_request_source_index(" + ui + ".download_request_token)<0").toBool());
        QTRY_COMPARE(evaluate(helper + ".ownedHelpers.length").toInt(), 0);
        if (behavior == "policy-block" || behavior == "redirect-policy-block" || behavior == "http-error" || behavior == "redirect-loop") {
            QCOMPARE(evaluate(ui + ".download_rows().length").toInt(), 0);
            if (behavior.contains("policy-block")) {
                QCOMPARE(fileRequests, 0);
                QCOMPARE(redirectRequests, behavior == "policy-block" ? 0 : 1);
                QVERIFY(evaluate(ui + ".status_text").toString().contains("blocked"));
            } else if (behavior == "http-error") {
                QCOMPARE(fileRequests, 1);
                QVERIFY(evaluate(ui + ".status_text").toString().contains("404"));
            } else {
                QCOMPARE(redirectRequests, 2);
                QVERIFY(evaluate(ui + ".status_text").toString().contains("redirect"));
            }
        } else {
            QTRY_COMPARE(evaluate(ui + ".download_rows().length").toInt(), 1);
            QTRY_VERIFY(!evaluate(ui + ".download_rows()[0].active").toBool());
            const auto path = evaluate(ui + ".download_rows()[0].destination").toString();
            if (behavior == "truncated") {
                QCOMPARE(evaluate(ui + ".download_rows()[0].state").toString(), QString("interrupted"));
                QVERIFY(!evaluate(ui + ".download_rows()[0].fileReady").toBool());
                QVERIFY(!QFileInfo::exists(path));
            } else {
                QVERIFY(evaluate(ui + ".download_rows()[0].fileReady").toBool());
                QFile file(path); QVERIFY(file.open(QIODevice::ReadOnly)); QCOMPARE(file.readAll(), behavior == "empty" ? QByteArray() : binary);
                QCOMPARE(evaluate(ui + ".download_rows()[0].total").toLongLong(), behavior == "unknown-total" ? -1LL : behavior == "empty" ? 0LL : 8192LL);
                if (behavior == "utf8-name") QCOMPARE(QFileInfo(path).fileName(), QString::fromUtf8("Résumé.bin"));
            }
            QCOMPARE(fileRequests, 1);
        }
        QCOMPARE(evaluate(ui + ".current_url").toString(), original);
        QCOMPARE(evaluate(view + ".title").toString(), title);
        QCOMPARE(evaluate(ui + ".tab_count").toInt(), tabs);
    }

    void downloadStreamTls_data() {
        QTest::addColumn<QString>("behavior");
        QTest::addColumn<bool>("privateProfile");
        for (bool transient : {false, true})
            for (const auto kind : {"untrusted", "approved", "downgrade"}) {
                const auto tag = QByteArray(transient ? "private-" : "regular-") + kind;
                QTest::newRow(tag.constData()) << QString::fromLatin1(kind) << transient;
            }
    }
    void downloadStreamTls() {
        QFETCH(QString, behavior);
        QFETCH(bool, privateProfile);
        QQuickWindow *owner = window;
        QString host = "testWindow", ui = "testWindow.browserUi";
        if (privateProfile) {
            owner = createChild("stream-tls", true);
            engine->rootContext()->setContextProperty("streamTlsHost", owner);
            engine->rootContext()->setContextProperty("streamTlsUi", evaluate("streamTlsHost.windowUi").value<QObject *>());
            host = "streamTlsHost"; ui = "streamTlsUi";
        }
        const auto base = QFileInfo(configPath).absolutePath();
        const auto keyPath = base + "/tls-key.pem", certPath = base + "/tls-cert.pem";
        QProcess openssl;
        openssl.start("openssl", {"req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
                                 "-subj", "/CN=localhost", "-addext", "subjectAltName=DNS:localhost,IP:127.0.0.1",
                                 "-keyout", keyPath, "-out", certPath});
        QVERIFY(openssl.waitForFinished(10000)); QCOMPARE(openssl.exitCode(), 0);
        QFile key(keyPath), certificate(certPath);
        QVERIFY(key.open(QIODevice::ReadOnly)); QVERIFY(certificate.open(QIODevice::ReadOnly));
        auto config = QSslConfiguration::defaultConfiguration();
        config.setLocalCertificate(QSslCertificate(certificate.readAll()));
        config.setPrivateKey(QSslKey(key.readAll(), QSsl::Rsa));
        config.setPeerVerifyMode(QSslSocket::VerifyNone);
        QSslServer tls; tls.setSslConfiguration(config);
        QVERIFY(tls.listen(QHostAddress::LocalHost));
        QTcpServer insecure; QVERIFY(insecure.listen(QHostAddress::LocalHost));
        int insecureRequests = 0, fileRequests = 0;
        connect(&insecure, &QTcpServer::newConnection, &insecure, [&] {
            while (auto socket = insecure.nextPendingConnection()) {
                connect(socket, &QTcpSocket::readyRead, socket, [&, socket] {
                    socket->readAll(); ++insecureRequests;
                    socket->write("HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nfile"); socket->disconnectFromHost();
                });
                connect(socket, &QTcpSocket::disconnected, socket, &QObject::deleteLater);
            }
        });
        connect(&tls, &QTcpServer::pendingConnectionAvailable, &tls, [&] {
            while (auto socket = tls.nextPendingConnection()) {
                connect(socket, &QTcpSocket::readyRead, socket, [&, socket] {
                    auto request = socket->property("request").toByteArray() + socket->readAll(); socket->setProperty("request", request);
                    if (!request.contains("\r\n\r\n") || socket->property("sent").toBool()) return;
                    socket->setProperty("sent", true);
                    if (request.startsWith("GET /file ")) {
                        ++fileRequests;
                        if (behavior == "downgrade") socket->write("HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:" + QByteArray::number(insecure.serverPort()) + "/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                        else socket->write("HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: 4\r\nConnection: close\r\n\r\nfile");
                    } else {
                        const QByteArray body = "<!doctype html><title>Owned TLS site</title><p>Approved source</p>";
                        socket->write("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: " + QByteArray::number(body.size()) + "\r\nConnection: close\r\n\r\n" + body);
                    }
                    socket->disconnectFromHost();
                });
                connect(socket, &QTcpSocket::disconnected, socket, &QObject::deleteLater);
            }
        });
        const auto origin = QString("https://127.0.0.1:%1").arg(tls.serverPort());
        if (behavior != "untrusted") {
            submitAt(owner, ui, "open " + origin + "/site");
            QTRY_VERIFY(evaluate("testWindow.pendingCertificateError!==null").toBool());
            evaluate("testWindow.acceptCertificateError()");
            QTRY_COMPARE(evaluate(host + ".activeWebView().title").toString(), QString("Owned TLS site"));
        }
        const auto source = evaluate(ui + ".current_url").toString();
        const auto tabs = evaluate(ui + ".tab_count").toInt();
        submitAt(owner, ui, "download " + origin + "/file");
        const auto helper = host + ".downloadWorkspaceObject.children.filter(function(item){return item.ownedHelpers!==undefined})[0]";
        QTRY_VERIFY(!evaluate(ui + ".download_request_token").toString().isEmpty());
        QTRY_VERIFY(evaluate(ui + ".download_request_source_index(" + ui + ".download_request_token)<0").toBool());
        QTRY_COMPARE(evaluate(helper + ".ownedHelpers.length").toInt(), 0);
        if (behavior == "approved") {
            QTRY_COMPARE(evaluate(ui + ".download_rows().length").toInt(), 1);
            QTRY_VERIFY(evaluate(ui + ".download_rows()[0].fileReady").toBool());
            QFile file(evaluate(ui + ".download_rows()[0].destination").toString());
            QVERIFY(file.open(QIODevice::ReadOnly)); QCOMPARE(file.readAll(), QByteArray("file"));
            QCOMPARE(fileRequests, 1);
        } else {
            QCOMPARE(evaluate(ui + ".download_rows().length").toInt(), 0);
            QCOMPARE(fileRequests, behavior == "untrusted" ? 0 : 1);
            QCOMPARE(insecureRequests, 0);
            QVERIFY(!evaluate("testWindow.pendingCertificateError!==null").toBool());
            QVERIFY(!evaluate(ui + ".status_text").toString().isEmpty());
        }
        QCOMPARE(evaluate(ui + ".current_url").toString(), source);
        QCOMPARE(evaluate(ui + ".tab_count").toInt(), tabs);
    }

    void secondaryDownloadManagerIsAvailable() {
        submit("downloads");
        QTRY_VERIFY(visual(window->contentItem(), "downloadList"));
        QTest::keyClick(window, Qt::Key_Escape);
        QTRY_VERIFY(!evaluate("testWindow.downloadManagerVisible").toBool());
        auto child = createChild("download-owner", true);
        QVERIFY(child);
        QVERIFY(evaluate("testWindow.browserWindowRegistry[1].profile.offTheRecord").toBool());
        QTRY_VERIFY(evaluate("testWindow.browserWindowRegistry[1].view && "
                             "testWindow.browserWindowRegistry[1].view.title==='"
                             "Owned child secret' && "
                             "!testWindow.browserWindowRegistry[1].view.loading")
                        .toBool());
        submitAt(child, "testWindow.browserWindowRegistry[1].ui", "downloads");
        QElapsedTimer opening;
        opening.start();
        while (!visual(child->contentItem(), "downloadList") && opening.elapsed() < 3000)
            QTest::qWait(10);
        const QJsonObject facts{
            {"private",
             evaluate("testWindow.browserWindowRegistry[1].profile.offTheRecord").toBool()},
            {"status", evaluate("testWindow.browserWindowRegistry[1].ui.status_text").toString()},
            {"primaryVisible", visual(window->contentItem(), "downloadList") != nullptr},
            {"invokingVisible", visual(child->contentItem(), "downloadList") != nullptr}};
        qInfo().noquote() << QJsonDocument(facts).toJson(QJsonDocument::Compact);
        const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
        if (!artifacts.isEmpty())
            write(artifacts + "/downloads-owner.json", QJsonDocument(facts).toJson());
        // A manager requested in this private window must be usable there.
        // No content, normal profile or desktop input is involved in this
        // observation.
        QVERIFY2(
            visual(child->contentItem(), "downloadList"),
            qPrintable(evaluate("testWindow.browserWindowRegistry[1].ui.status_text").toString()));
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
    DownloadsTests tests;
    return QTest::qExec(&tests, argc, argv);
}
#include "tst_downloads.moc"
