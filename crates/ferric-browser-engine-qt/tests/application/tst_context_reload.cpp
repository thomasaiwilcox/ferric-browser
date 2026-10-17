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
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include <memory>
#include <fcntl.h>
#include <sys/ioctl.h>
#include <sys/stat.h>
#include <unistd.h>

extern "C" bool cxx_qt_init_crate_ferric_browser_engine_qt();
#include "../../src/url_display.h"

class ContextReloadTests final : public QObject {
    Q_OBJECT
    QTemporaryDir directory;
    std::unique_ptr<QQmlApplicationEngine> engine;
    QQuickWindow *window = nullptr;
    QTcpServer server;
    QString configPath, contextsPath;
    QVariant evaluate(const QString &source) {
        QQmlExpression expression(engine->rootContext(), window, source);
        auto result = expression.evaluate();
        if (expression.hasError()) qFatal("Probe QML: %s", qPrintable(expression.error().toString()));
        return result;
    }
    static QString quoted(const QString &value) {
        auto bytes = QJsonDocument(QJsonArray{value}).toJson(QJsonDocument::Compact);
        return QString::fromUtf8(bytes.mid(1, bytes.size() - 2));
    }
    static void write(const QString &path, const QByteArray &contents) {
        QFile file(path); QVERIFY(file.open(QIODevice::WriteOnly | QIODevice::Truncate));
        QCOMPARE(file.write(contents), contents.size()); file.close();
    }
    void submit(const QString &command) {
        window->requestActivate(); QTRY_VERIFY(window->isActive());
        evaluate("testWindow.browserUi.enter_command()");
        auto input = window->findChild<QQuickItem *>("commandLineInput"); QVERIFY(input);
        QVERIFY(input->setProperty("text", command)); input->forceActiveFocus();
        QTRY_VERIFY(input->hasActiveFocus()); QTest::keyClick(window, Qt::Key_Return);
    }
  private slots:
    void init() {
        QVERIFY(directory.isValid());
        const auto tag = QString::fromLatin1(QTest::currentDataTag());
        auto base = directory.filePath(tag.isEmpty() ? "busy-case" : tag);
        QVERIFY(QDir().mkpath(base)); configPath = base + "/config.toml";
        contextsPath = base + "/contexts.toml";
        write(configPath, "schema_version = 3\n[ui]\nfont_size_pt = 12\n");
        write(contextsPath, "schema_version = 3\n[[contexts]]\nname = 'work'\nlabel = 'Before work'\nprofile = 'default'\ndefault_target = 'reuse'\n");
        QVERIFY(server.listen(QHostAddress::LocalHost, 0));
        connect(&server, &QTcpServer::newConnection, this, [this] {
            while (auto socket = server.nextPendingConnection()) {
                connect(socket, &QTcpSocket::readyRead, socket, [socket] {
                    if (!socket->readAll().contains("\r\n\r\n")) return;
                    const QByteArray body = "<!doctype html><title>Reload probe</title>";
                    socket->write("HTTP/1.1 200 OK\r\nContent-Length: " + QByteArray::number(body.size()) + "\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n" + body);
                    socket->disconnectFromHost();
                });
                connect(socket, &QTcpSocket::disconnected, socket, &QObject::deleteLater);
            }
        });
        const QString config = R"JSON({"schema_version":3,"ui":{"font_size_pt":12}})JSON";
        const QString contexts = R"JSON({"schema_version":3,"contexts":[{"name":"work","label":"Before work","profile":"default","default_target":"reuse"}],"routes":[]})JSON";
        engine = std::make_unique<QQmlApplicationEngine>();
        engine->setInitialProperties({{"temporaryProfile", false}, {"storageBasePath", base + "/data"},
            {"softwareRendering", true}, {"safeMode", true}, {"userscriptsOff", true},
            {"startupConfigJson", config}, {"startupConfigBaseJson", config},
            {"startupConfigPath", configPath}, {"startupCliOverridesJson", "{\"schema_version\":3}"},
            {"startupProfileOverridesJson", "{\"schema_version\":3}"}, {"startupContextsJson", contexts},
            {"startupUrl", "rb://blank"}, {"width", 900}, {"height", 700}});
        engine->load(QUrl("qrc:/qt/qml/io/github/ferricbrowser/qml/Main.qml"));
        QCOMPARE(engine->rootObjects().size(), 1);
        window = qobject_cast<QQuickWindow *>(engine->rootObjects().first()); QVERIFY(window);
        engine->rootContext()->setContextProperty("testWindow", window);
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.profileBootstrapComplete && !testWindow.browserUi.profile_bootstrap_pending && !!testWindow.activeWebView()").toBool(), 15000);
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.browserUi.context_choice_names.indexOf('work')>=0").toBool(), 15000);
        QTRY_COMPARE(evaluate("testWindow.browserUi.context_choice_labels[0]").toString(), QString("Before work"));
        window->show(); window->requestActivate(); QTRY_VERIFY(window->isActive());
    }
    void cleanup() {
        if (engine && window) {
            evaluate("testWindow.browserWindowRegistry.slice(1).forEach(function(entry){entry.host.windowShutdownApproved=true; entry.host.close(); entry.host.destroy()})");
            QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
        }
        engine.reset(); window = nullptr; server.close(); server.disconnect(this);
        QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
        QCoreApplication::processEvents();
    }
    void reloadRetainsEditWhileBusy() {
        QVERIFY(QFile::remove(configPath));
        const auto encoded = QFile::encodeName(configPath);
        QTemporaryDir barrierDirectory(directory.filePath("barrier-XXXXXX"));
        QVERIFY(barrierDirectory.isValid());
        const auto pipePath = QFile::encodeName(barrierDirectory.filePath("held-config.toml"));
        QCOMPARE(::mkfifo(pipePath.constData(), 0600), 0);
        QCOMPARE(::symlink(pipePath.constData(), encoded.constData()), 0);
        struct HeldPipe {
            int fd;
            ~HeldPipe() { if (fd >= 0) ::close(fd); }
        } held{::open(pipePath.constData(), O_RDWR | O_NONBLOCK)};
        QVERIFY(held.fd >= 0);
        const QByteArray first = "schema_version = 3\n[ui]\nfont_size_pt = 14\n";
        QCOMPARE(::write(held.fd, first.constData(), first.size()), first.size());
        submit("config-reload");
        const auto available = [&]() {
            int bytes = -1;
            return ::ioctl(held.fd, FIONREAD, &bytes) == 0 ? bytes : -1;
        };
        // The loader has consumed A and is blocked until its writer closes.
        QTRY_COMPARE_WITH_TIMEOUT(available(), 0, 5000);
        const auto replacement = configPath + ".next";
        write(replacement, "schema_version = 3\n[ui]\nfont_size_pt = 22\n");
        QCOMPARE(::rename(QFile::encodeName(replacement).constData(), encoded.constData()), 0);
        QElapsedTimer busy; busy.start();
        while (busy.elapsed() < 500) {
            evaluate("testWindow.browserUi.poll_config()");
            QTest::qWait(20);
        }
        QCOMPARE(evaluate("JSON.parse(testWindow.browserUi.config_json).ui.font_size_pt").toInt(), 12);
        ::close(held.fd); held.fd = -1;
        QTRY_COMPARE_WITH_TIMEOUT(evaluate("JSON.parse(testWindow.browserUi.config_json).ui.font_size_pt").toInt(), 22, 10000);
    }
    void configurationReload_data() {
        QTest::addColumn<QString>("kind");
        for (const auto &kind : {QString("updated-definition"), QString("added-definition"), QString("removed-definition"), QString("new-route")})
            QTest::newRow(qPrintable(kind)) << kind;
    }
    void configurationReload() {
        QFETCH(QString, kind);
        QByteArray contexts = "schema_version = 3\n";
        if (kind != "removed-definition")
            contexts += "[[contexts]]\nname = 'work'\nlabel = 'After work'\nprofile = 'default'\ndefault_target = 'reuse'\n";
        if (kind == "added-definition")
            contexts += "[[contexts]]\nname = 'research'\nlabel = 'Research'\nprofile = 'default'\ndefault_target = 'reuse'\n";
        const auto url = QString("http://127.0.0.1:%1/route").arg(server.serverPort());
        if (kind == "new-route") contexts += QString("[[routes]]\nid = 'reload-work'\npattern = 'http://127.0.0.1:%1/*'\ncontext = 'work'\nbehavior = 'prompt'\nentry_points = ['explicit-open']\n").arg(server.serverPort()).toUtf8();
        write(contextsPath, contexts); write(configPath, "schema_version = 3\n[ui]\nfont_size_pt = 18\n");
        submit("config-reload");
        QTRY_COMPARE_WITH_TIMEOUT(evaluate("JSON.parse(testWindow.browserUi.config_json).ui.font_size_pt").toInt(), 18, 15000);
        if (kind == "updated-definition")
            QCOMPARE(evaluate("testWindow.browserUi.context_choice_labels[0]").toString(), QString("After work"));
        else if (kind == "added-definition")
            QVERIFY2(evaluate("testWindow.browserUi.context_choice_names.indexOf('research')>=0").toBool(), "Reload must publish the authored context choice");
        else if (kind == "removed-definition")
            QVERIFY2(evaluate("testWindow.browserUi.context_choice_names.indexOf('work')<0").toBool(), "A removed definition must not remain an offered context");
        else {
            submit("open " + url);
            QTRY_COMPARE_WITH_TIMEOUT(evaluate("testWindow.browserUi.context_route_id").toString(), QString("reload-work"), 5000);
        }
    }

    void rejectedReload_data() {
        QTest::addColumn<QString>("kind");
        QTest::newRow("unknown-profile") << QString("unknown-profile");
        QTest::newRow("invalid-selector") << QString("invalid-selector");
        QTest::newRow("invalid-contexts") << QString("invalid-contexts");
    }
    void rejectedReload() {
        QFETCH(QString, kind);
        QDirIterator metadata(QFileInfo(configPath).absolutePath(), {"contexts.json"}, QDir::Files, QDirIterator::Subdirectories);
        QVERIFY(metadata.hasNext());
        const auto metadataPath = metadata.next();
        QFile beforeFile(metadataPath); QVERIFY(beforeFile.open(QIODevice::ReadOnly));
        const auto before = beforeFile.readAll(); beforeFile.close();
        QByteArray contexts = "schema_version = 3\n[[contexts]]\nname = 'work'\nlabel = 'Rejected work'\nprofile = 'default'\ndefault_target = 'reuse'\n";
        QByteArray config = "schema_version = 3\n[ui]\nfont_size_pt = 18\n";
        if (kind == "unknown-profile") contexts.replace("profile = 'default'", "profile = 'missing'");
        if (kind == "invalid-contexts") contexts += "[[routes]]\nid = 'broken'\npattern = '*'\ncontext = 'missing'\n";
        if (kind == "invalid-selector") config += "[hints.selectors.links]\nmode = 'extend'\nselectors = ['[']\n";
        write(contextsPath, contexts); write(configPath, config);
        submit("config-reload");
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.browserUi.status_text.indexOf(\"couldn't be loaded\")>=0").toBool(), 15000);
        QCOMPARE(evaluate("JSON.parse(testWindow.browserUi.config_json).ui.font_size_pt").toInt(), 12);
        QCOMPARE(evaluate("testWindow.browserUi.context_choice_labels[0]").toString(), QString("Before work"));
        QCOMPARE(evaluate("JSON.parse(testWindow.browserUi.contexts_json).contexts[0].label").toString(), QString("Before work"));
        QFile afterFile(metadataPath); QVERIFY(afterFile.open(QIODevice::ReadOnly));
        QCOMPARE(afterFile.readAll(), before); afterFile.close();
        // A corrected document must recover without restarting the window.
        write(contextsPath, "schema_version = 3\n[[contexts]]\nname = 'work'\nlabel = 'Recovered work'\nprofile = 'default'\ndefault_target = 'reuse'\n");
        write(configPath, "schema_version = 3\n[ui]\nfont_size_pt = 20\n");
        submit("config-reload");
        QTRY_COMPARE_WITH_TIMEOUT(evaluate("JSON.parse(testWindow.browserUi.config_json).ui.font_size_pt").toInt(), 20, 15000);
        QCOMPARE(evaluate("testWindow.browserUi.context_choice_labels[0]").toString(), QString("Recovered work"));
    }
    void existingWindowReceivesReloadedCatalog() {
        submit("window-new");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.browserWindowRegistry[1].host.profileBootstrapComplete && !testWindow.browserWindowRegistry[1].ui.profile_bootstrap_pending").toBool(),15000);
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry[1].ui.context_choice_labels[0]").toString(),QString("Before work"));
        write(contextsPath, "schema_version = 3\n[[contexts]]\nname = 'work'\nlabel = 'Shared updated work'\nprofile = 'default'\ndefault_target = 'reuse'\n");
        submit("config-reload");
        QTRY_COMPARE_WITH_TIMEOUT(evaluate("testWindow.browserUi.context_choice_labels[0]").toString(),QString("Shared updated work"),15000);
        QTRY_COMPARE_WITH_TIMEOUT(evaluate("testWindow.browserWindowRegistry[1].ui.context_choice_labels[0]").toString(),QString("Shared updated work"),15000);
    }

};

int main(int argc, char **argv) {
    ferric_browser_enable_software_rendering(); ferric_browser_register_internal_scheme();
    QQuickWindow::setGraphicsApi(QSGRendererInterface::Software); QtWebEngineQuick::initialize();
    cxx_qt_init_crate_ferric_browser_engine_qt(); QGuiApplication app(argc, argv);
    ContextReloadTests tests; return QTest::qExec(&tests, argc, argv);
}
#include "tst_context_reload.moc"
