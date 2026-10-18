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
#include <sys/ioctl.h>
#include <sys/stat.h>
#include <unistd.h>

extern "C" bool cxx_qt_init_crate_ferric_browser_engine_qt();
#include "../../src/url_display.h"

class CloseRequestTests final : public QObject {
    Q_OBJECT
    QTemporaryDir directory;
    std::unique_ptr<QQmlApplicationEngine> engine;
    QQuickWindow *window = nullptr;
    QTcpServer server;
    QString configPath, contextsPath;
    QVariant scriptResult;
    bool scriptDone = false;
    QVariant evaluate(const QString &source) {
        QQmlExpression expression(engine->rootContext(), window, source);
        auto result = expression.evaluate();
        if (expression.hasError())
            qFatal("Probe QML: %s", qPrintable(expression.error().toString()));
        return result;
    }
    QVariant page(const QString &view, const QString &script) {
        scriptDone = false;
        evaluate(view + ".runJavaScript(" + quoted(script) +
                 ",0,function(value){closeHarness.pageReply(value)})");
        QElapsedTimer timer;
        timer.start();
        while (!scriptDone && timer.elapsed() < 5000)
            QTest::qWait(5);
        if (!scriptDone)
            qFatal("Owned close fixture page reply timed out");
        return scriptResult;
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
        engine->rootContext()->setContextProperty("closeHarness", this);
        QTRY_VERIFY_WITH_TIMEOUT(
            evaluate(
                "testWindow.profileBootstrapComplete && "
                "!testWindow.browserUi.profile_bootstrap_pending && !!testWindow.activeWebView()")
                .toBool(),
            15000);
        window->show();
        window->requestActivate();
        QTRY_VERIFY(window->isActive());
        // These cases start from a ready native tab, after the view has
        // acquired its model identity and completed its initial content setup.
        QTRY_VERIFY_WITH_TIMEOUT(evaluate(
            "testWindow.activeWebView().ownsTab() && "
            "testWindow.activeWebView().contentInitialized && "
            "testWindow.activeWebView().nativeUrl==='rb://blank'").toBool(), 15000);
        QSignalSpy frame(window, &QQuickWindow::frameSwapped);
        window->requestUpdate();
        QTRY_VERIFY(!frame.isEmpty());
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

  public:
    Q_INVOKABLE void pageReply(const QVariant &value) {
        scriptResult = value;
        scriptDone = true;
    }

  private slots:
    void init() {
        QVERIFY(directory.isValid());
        const auto base = directory.filePath(QString::fromLatin1(QTest::currentTestFunction()) +
                                             "/" + QString::fromLatin1(QTest::currentDataTag()));
        QVERIFY(QDir().mkpath(base));
        write(base + "/.ferric-owned-fixture", "Owned browser close checks only.\n");
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
                    const QByteArray body = "<!doctype html><title>Owned close "
                                            "fixture</title><input id=field value=initial>";
                    socket->write("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: " +
                                  QByteArray::number(body.size()) +
                                  "\r\nConnection: close\r\n\r\n" + body);
                    socket->disconnectFromHost();
                });
                connect(socket, &QTcpSocket::disconnected, socket, &QObject::deleteLater);
            }
        });
        launch(R"JSON({"schema_version":3,"contexts":[],"routes":[]})JSON");
        // Observe the real final Qt quit signal without ending this test's
        // shared application event loop between separate engine lifetimes.
        QObject::disconnect(engine.get(), SIGNAL(quit()), nullptr, nullptr);
    }
    void cleanup() {
        closeWindow();
        server.close();
        server.disconnect(this);
    }
    void cleanClose_data() {
        QTest::addColumn<bool>("secondary");
        QTest::addColumn<QString>("pageUrl");
        QTest::addColumn<bool>("newTab");
        for (bool secondary : {false, true})
            for (const auto &url : {QString("rb://blank"), QString("rb://start"), QString("web")})
                for (bool newTab : {false, true})
                    QTest::newRow(
                        qPrintable((secondary ? QString("secondary-") : QString("primary-")) + url +
                                   (newTab ? "-new-tab" : "-current")))
                        << secondary << url << newTab;
    }
    void cleanClose() {
        QFETCH(bool, secondary);
        QFETCH(QString, pageUrl);
        QFETCH(bool, newTab);
        QQuickWindow *host = window;
        QString ui = "testWindow.browserUi";
        QString view = "testWindow.activeWebView()";
        if (secondary) {
            host = createChild("clean-page", false);
            QVERIFY(host);
            ui = "testWindow.browserWindowRegistry[1].ui";
            view = "testWindow.browserWindowRegistry[1].host.activeView";
        }
        const auto url = pageUrl == "web"
                             ? QString("http://127.0.0.1:%1/clean-page").arg(server.serverPort())
                             : pageUrl;
        submitAt(host, ui, (newTab ? "tab-open " : "open ") + url);
        QTRY_COMPARE(evaluate(ui + ".current_url").toString(), url);
        if (pageUrl == "web") {
            QTRY_COMPARE_WITH_TIMEOUT(evaluate(ui + ".page_title").toString(),
                                      QString("Owned close fixture"), 15000);
            QTRY_VERIFY(!evaluate(view + ".loading").toBool());
        } else
            QTRY_COMPARE(evaluate(view + ".nativeUrl").toString(), url);
        QSignalSpy quit(engine.get(), &QQmlEngine::quit);
        submitAt(host, ui, secondary ? "window-close" : "quit");
        auto proceed = visual(host->contentItem(), "shutdownProceedButton");
        QVERIFY(proceed);
        QSignalSpy frame(host, &QQuickWindow::frameSwapped);
        host->requestUpdate();
        QTRY_VERIFY(!frame.isEmpty());
        QTest::mouseClick(host, Qt::LeftButton, Qt::NoModifier,
                          proceed->mapToScene(proceed->boundingRect().center()).toPoint());
        if (secondary) {
            QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 1);
            QCOMPARE(quit.count(), 0);
        } else {
            QTRY_COMPARE(quit.count(), 1);
            QVERIFY(evaluate("testWindow.shutdownApproved && !testWindow.shutdownPagePromptVisible "
                             "&& !testWindow.applicationShutdownForcePromptVisible")
                        .toBool());
        }
    }
    void dirtyBackgroundPage_data() {
        QTest::addColumn<bool>("secondary");
        QTest::newRow("primary") << false;
        QTest::newRow("secondary") << true;
    }
    void dirtyBackgroundPage() {
        QFETCH(bool, secondary);
        QQuickWindow *host = window;
        QString ui = "testWindow.browserUi";
        QString view = "testWindow.activeWebView()";
        if (secondary) {
            host = createChild("dirty-page", false);
            QVERIFY(host);
            ui = "testWindow.browserWindowRegistry[1].ui";
            view = "testWindow.browserWindowRegistry[1].host.activeView";
        }
        const auto url = QString("http://127.0.0.1:%1/dirty-page").arg(server.serverPort());
        submitAt(host, ui, "open " + url);
        QTRY_COMPARE(evaluate(ui + ".current_url").toString(), url);
        QTRY_COMPARE_WITH_TIMEOUT(evaluate(ui + ".page_title").toString(),
                                  QString("Owned close fixture"), 15000);
        QTRY_VERIFY(!evaluate(view + ".loading").toBool());
        QTRY_VERIFY(page(view, "!!document.getElementById('field')").toBool());
        QCOMPARE(page(view, "document.getElementById('field').value='unsaved'").toString(),
                 QString("unsaved"));
        submitAt(host, ui, "tab-open rb://blank");
        QTRY_COMPARE(evaluate(ui + ".current_url").toString(), QString("rb://blank"));
        QSignalSpy quit(engine.get(), &QQmlEngine::quit);
        submitAt(host, ui, secondary ? "window-close" : "quit");
        auto proceed = visual(host->contentItem(), "shutdownProceedButton");
        QVERIFY(proceed);
        QSignalSpy frame(host, &QQuickWindow::frameSwapped);
        host->requestUpdate();
        QTRY_VERIFY(!frame.isEmpty());
        QTest::mouseClick(host, Qt::LeftButton, Qt::NoModifier,
                          proceed->mapToScene(proceed->boundingRect().center()).toPoint());
        const auto visible = secondary ? "testWindow.browserWindowRegistry[1].host."
                                         "windowShutdownPagePromptVisible"
                                       : "testWindow.shutdownPagePromptVisible";
        const auto reason = secondary ? "testWindow.browserWindowRegistry[1].host."
                                        "windowShutdownPagePromptReason"
                                      : "testWindow.shutdownPagePromptReason";
        QTRY_VERIFY(evaluate(visible).toBool());
        QVERIFY(evaluate(reason).toString().contains("unsaved"));
        QCOMPARE(quit.count(), 0);
        QCOMPARE(evaluate(ui + ".tab_count").toInt(), 2);
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
    CloseRequestTests tests;
    return QTest::qExec(&tests, argc, argv);
}
#include "tst_close_requests.moc"
