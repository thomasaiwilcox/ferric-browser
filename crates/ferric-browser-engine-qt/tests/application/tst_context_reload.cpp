#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QGuiApplication>
#include <QApplication>
#include "../../src/gui_application.h"
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
    void launch(const QString &contexts) {
        const auto base = QFileInfo(configPath).absolutePath();
        const QString config = R"JSON({"schema_version":3,"ui":{"font_size_pt":12}})JSON";
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
        window->show(); window->requestActivate(); QTRY_VERIFY(window->isActive());
    }
    void closeWindow() {
        if (engine && window) {
            evaluate("testWindow.browserWindowRegistry.slice(1).forEach(function(entry){entry.host.windowShutdownApproved=true; entry.host.close(); entry.host.destroy()})");
            QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
        }
        engine.reset(); window = nullptr;
        QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
        QCoreApplication::processEvents();
    }
    QJsonObject contextRecord(const QString &name) {
        QDirIterator files(QFileInfo(configPath).absolutePath() + "/data", {"contexts.json"}, QDir::Files, QDirIterator::Subdirectories);
        if (!files.hasNext()) return {};
        QFile file(files.next()); if (!file.open(QIODevice::ReadOnly)) return {};
        QJsonParseError error;
        const auto document = QJsonDocument::fromJson(file.readAll(), &error);
        if (error.error != QJsonParseError::NoError) return {};
        const auto records = document.isArray() ? document.array() : document.object()["contexts"].toArray();
        for (const auto &record : records) if (record.toObject()["name"].toString() == name) return record.toObject();
        return {};
    }
  private slots:
    void init() {
        QVERIFY(directory.isValid());
        const auto tag = QString::fromLatin1(QTest::currentDataTag());
        const auto name = QString::fromLatin1(QTest::currentTestFunction());
        auto base = directory.filePath(name + "/" + (tag.isEmpty() ? "default" : tag));
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
        launch(R"JSON({"schema_version":3,"contexts":[{"name":"work","label":"Before work","profile":"default","default_target":"reuse"}],"routes":[]})JSON");
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.browserUi.context_choice_names.indexOf('work')>=0").toBool(), 15000);
        QTRY_COMPARE(evaluate("testWindow.browserUi.context_choice_labels[0]").toString(), QString("Before work"));
    }
    void cleanup() {
        closeWindow(); server.close(); server.disconnect(this);
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


    void reloadPreservesEnteredContext() {
        submit("context-enter work");
        QTRY_COMPARE(evaluate("testWindow.browserUi.context_name").toString(), QString("work"));
        submit("context-save");
        QTRY_VERIFY2(!contextRecord("work")["members"].toArray().isEmpty(), qPrintable(evaluate("testWindow.browserUi.status_text").toString()));
        const auto before = contextRecord("work");
        QVERIFY(!before["id"].toString().isEmpty());
        const auto storage = evaluate("testWindow.browserProfile.persistentStoragePath").toString();
        const auto url = evaluate("testWindow.browserUi.current_url").toString();
        write(contextsPath, "schema_version = 3\n[[contexts]]\nname = 'work'\nlabel = 'After work'\nprofile = 'default'\ndefault_target = 'reuse'\n");
        submit("config-reload");
        QTRY_COMPARE_WITH_TIMEOUT(evaluate("testWindow.browserUi.context_label").toString(), QString("After work"),15000);
        QCOMPARE(evaluate("testWindow.browserUi.context_name").toString(), QString("work"));
        QCOMPARE(contextRecord("work")["id"], before["id"]);
        QCOMPARE(contextRecord("work")["members"], before["members"]);
        QCOMPARE(evaluate("testWindow.browserProfile.persistentStoragePath").toString(),storage);
        QCOMPARE(evaluate("testWindow.browserUi.current_url").toString(),url);
    }
    void removedContextStopsBeingCurrent_data() {
        QTest::addColumn<QString>("kind");
        QTest::newRow("reload") << QString("reload");
        QTest::newRow("command") << QString("command");
    }
    void removedContextStopsBeingCurrent() {
        QFETCH(QString,kind);
        const auto name = kind == "command" ? QString("manual") : QString("work");
        if (kind == "command") {
            submit("context-create manual --profile default");
            QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.browserUi.context_choice_names.indexOf('manual')>=0").toBool(),15000);
        }
        submit("context-enter " + name);
        QTRY_COMPARE(evaluate("testWindow.browserUi.context_name").toString(), name);
        const auto storage = evaluate("testWindow.browserProfile.persistentStoragePath").toString();
        const auto url = evaluate("testWindow.browserUi.current_url").toString();
        if (kind == "reload") {
            write(contextsPath, "schema_version = 3\n");
            submit("config-reload");
        } else submit("context-delete manual --confirm");
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.browserUi.context_choice_names.indexOf(" + quoted(name) + ")<0").toBool(),15000);
        QTRY_VERIFY(evaluate("testWindow.browserUi.context_name.length===0").toBool());
        submit("context-save");
        QVERIFY2(evaluate("testWindow.browserUi.status_text.indexOf('not in a context')>=0").toBool(), qPrintable(evaluate("testWindow.browserUi.status_text").toString()));
        QCOMPARE(evaluate("testWindow.browserProfile.persistentStoragePath").toString(),storage);
        QCOMPARE(evaluate("testWindow.browserUi.current_url").toString(),url);
    }
    void offlineRemovalPreservesCommandContext() {
        submit("context-create manual --profile default");
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.browserUi.context_choice_names.indexOf('manual')>=0").toBool(),15000);
        const auto manualId = contextRecord("manual")["id"].toString();
        QVERIFY(!manualId.isEmpty()); QVERIFY(!contextRecord("work").isEmpty());
        closeWindow();
        write(contextsPath, "schema_version = 3\n");
        launch(R"JSON({"schema_version":3,"contexts":[],"routes":[]})JSON");
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.browserUi.context_choice_names.indexOf('manual')>=0").toBool(),15000);
        QVERIFY(evaluate("testWindow.browserUi.context_choice_names.indexOf('work')<0").toBool());
        QCOMPARE(contextRecord("manual")["id"].toString(), manualId);
        QVERIFY(contextRecord("work").isEmpty());
    }


    void watchedReloadPreservesCommandFeedback() {
        submit("context-save");
        const auto feedback=evaluate("testWindow.browserUi.status_text").toString();
        QVERIFY2(feedback.contains("not in a context"),qPrintable(feedback));
        write(configPath,"schema_version = 3\n[ui]\nfont_size_pt = 18\n");
        QTRY_COMPARE_WITH_TIMEOUT(evaluate("JSON.parse(testWindow.browserUi.config_json).ui.font_size_pt").toInt(),18,15000);
        QCOMPARE(evaluate("testWindow.browserUi.status_text").toString(),feedback);
    }
    void invalidWatchedReloadRetainsFeedbackAndShowsNotice() {
        QTRY_VERIFY(evaluate("testWindow.browserUi.profile_startup_notice.length===0").toBool());
        submit("context-save");
        const auto feedback=evaluate("testWindow.browserUi.status_text").toString();
        QVERIFY2(feedback.contains("not in a context"),qPrintable(feedback));
        write(configPath,"schema_version = 3\n[ui]\nfont_size_pt = -1\n");
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.browserUi.profile_startup_notice.length>0").toBool(),15000);
        QCOMPARE(evaluate("JSON.parse(testWindow.browserUi.config_json).ui.font_size_pt").toInt(),12);
        QCOMPARE(evaluate("testWindow.browserUi.status_text").toString(),feedback);
        write(configPath,"schema_version = 3\n[ui]\nfont_size_pt = 18\n");
        QTRY_COMPARE_WITH_TIMEOUT(evaluate("JSON.parse(testWindow.browserUi.config_json).ui.font_size_pt").toInt(),18,15000);
        QTRY_VERIFY(evaluate("testWindow.browserUi.profile_startup_notice.length===0").toBool());
        QCOMPARE(evaluate("testWindow.browserUi.status_text").toString(),feedback);
    }


    void watchedRecoveryClearsItsOwnFailureFeedback() {
        write(configPath,"schema_version = 3\n[ui]\nfont_size_pt = -1\n");
        submit("config-reload");
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.browserUi.status_text.indexOf(\"couldn't be loaded\")>=0").toBool(),15000);
        write(configPath,"schema_version = 3\n[ui]\nfont_size_pt = 18\n");
        QTRY_COMPARE_WITH_TIMEOUT(evaluate("JSON.parse(testWindow.browserUi.config_json).ui.font_size_pt").toInt(),18,15000);
        QTRY_VERIFY(evaluate("testWindow.browserUi.profile_startup_notice.length===0").toBool());
        QCOMPARE(evaluate("testWindow.browserUi.status_text").toString(),QString("Settings reloaded"));
    }

    void contextCommandsDescribeTheirOutcome() {
        submit("context-enter work");
        QCOMPARE(evaluate("testWindow.browserUi.status_text").toString(),QString("Entered context: Before work"));
        submit("context-save");
        QCOMPARE(evaluate("testWindow.browserUi.status_text").toString(),QString("Context saved: Before work"));
        submit("context-list");
        QTRY_VERIFY(evaluate("testWindow.switcherVisible").toBool());
        QCOMPARE(evaluate("testWindow.switcherScope").toString(),QString("contexts"));
        const auto listed=evaluate("testWindow.browserUi.status_text").toString();
        QVERIFY2(listed.contains("Before work") && listed.contains("work"),qPrintable(listed));
        QTest::keyClick(window,Qt::Key_Escape);
        QTRY_VERIFY(!evaluate("testWindow.switcherVisible").toBool());
        submit("context-route add https://example.test/* work");
        const auto added=evaluate("testWindow.browserUi.status_text").toString();
        QVERIFY2(added.contains("Before work") && !added.contains("route-"),qPrintable(added));
        const auto id=evaluate("JSON.parse(testWindow.browserUi.contexts_json).routes[0].id").toString();
        QVERIFY(!id.isEmpty());
        submit("context-route remove " + id);
        const auto removed=evaluate("testWindow.browserUi.status_text").toString();
        QVERIFY2(removed.contains("removed") && !removed.contains(id),qPrintable(removed));
        submit("context-create manual --profile default");
        QTRY_VERIFY_WITH_TIMEOUT(!contextRecord("manual").isEmpty(),15000);
        submit("context-delete manual --confirm");
        QCOMPARE(evaluate("testWindow.browserUi.status_text").toString(),QString("Context deleted: manual"));
        QVERIFY(contextRecord("manual").isEmpty());
        QVERIFY(!contextRecord("work").isEmpty());
        submit("config-reload");
        QTRY_COMPARE(evaluate("testWindow.browserUi.status_text").toString(),QString("Settings reloaded"));
        QVERIFY(contextRecord("manual").isEmpty());
        closeWindow();
        launch(R"JSON({"schema_version":3,"contexts":[{"name":"work","label":"Before work","profile":"default","default_target":"reuse"}],"routes":[]})JSON");
        QVERIFY(contextRecord("manual").isEmpty());
    }

    void configuredDeletionPreservesSavedContext() {
        submit("context-enter work");
        QTRY_COMPARE(evaluate("testWindow.browserUi.context_name").toString(),QString("work"));
        submit("context-save");
        const auto before = contextRecord("work");
        QVERIFY(!before["members"].toArray().isEmpty());
        QFile source(contextsPath); QVERIFY(source.open(QIODevice::ReadOnly));
        const auto authored = source.readAll(); source.close();
        const auto storage = evaluate("testWindow.browserProfile.persistentStoragePath").toString();
        const auto url = evaluate("testWindow.browserUi.current_url").toString();
        submit("context-delete work --confirm");
        const auto status = evaluate("testWindow.browserUi.status_text").toString();
        QVERIFY2(status.contains("defined in contexts.toml") && status.contains(":config-reload"),qPrintable(status));
        QVERIFY(!status.contains("Context deleted"));
        QCOMPARE(contextRecord("work"),before);
        QCOMPARE(evaluate("testWindow.browserUi.context_name").toString(),QString("work"));
        QVERIFY(source.open(QIODevice::ReadOnly)); QCOMPARE(source.readAll(),authored); source.close();
        write(configPath,"schema_version = 3\n[ui]\nfont_size_pt = 18\n");
        submit("config-reload");
        QTRY_COMPARE_WITH_TIMEOUT(evaluate("JSON.parse(testWindow.browserUi.config_json).ui.font_size_pt").toInt(),18,15000);
        QCOMPARE(contextRecord("work"),before);
        QCOMPARE(evaluate("testWindow.browserUi.context_name").toString(),QString("work"));
        QCOMPARE(evaluate("testWindow.browserProfile.persistentStoragePath").toString(),storage);
        QCOMPARE(evaluate("testWindow.browserUi.current_url").toString(),url);
        closeWindow();
        launch(R"JSON({"schema_version":3,"contexts":[{"name":"work","label":"Before work","profile":"default","default_target":"reuse"}],"routes":[]})JSON");
        QCOMPARE(contextRecord("work"),before);
    }

    void guideTaskHelp_data() {
        QTest::addColumn<QString>("topic");
        QTest::addColumn<int>("pointSize");
        for (const auto &topic : {QString("context-delete"),QString("hint"),QString("target-search"),QString("grid")}) {
            QTest::newRow(qPrintable(topic + "-normal")) << topic << 12;
            QTest::newRow(qPrintable(topic + "-large")) << topic << 20;
        }
    }
    void guideTaskHelp() {
        QFETCH(QString,topic); QFETCH(int,pointSize);
        if (pointSize != 12) {
            submit("set --temp ui.font_size_pt=" + QString::number(pointSize));
            QTRY_COMPARE(evaluate("testWindow.font.pointSize").toInt(),pointSize);
            window->resize(640,480);
        }
        submit("help " + topic);
        QTRY_VERIFY(evaluate("testWindow.bindingHelpVisible").toBool());
        QSignalSpy openedFrames(window,&QQuickWindow::frameSwapped);
        window->requestUpdate(); QTRY_VERIFY_WITH_TIMEOUT(!openedFrames.isEmpty(),5000);
        QQuickItem *guide = nullptr;
        for (auto item : window->findChildren<QQuickItem *>("ferricGuide"))
            if (item->isVisible()) { guide=item; break; }
        QVERIFY(guide);
        auto search=qobject_cast<QQuickItem *>(guide->property("searchInput").value<QObject *>());
        QVERIFY(search); QTRY_VERIFY(search->hasActiveFocus());
        auto read = [&](const QString &source) {
            QQmlExpression expression(engine->rootContext(),guide,source);
            const auto value=expression.evaluate();
            if (expression.hasError()) qFatal("Guide probe: %s",qPrintable(expression.error().toString()));
            return value;
        };
        auto entries=qobject_cast<QQuickItem *>(guide->property("entriesList").value<QObject *>());
        QVERIFY(entries); entries->forceActiveFocus();
        QTRY_VERIFY(entries->hasActiveFocus());
        const auto count=read("rows.length").toInt();
        for (int index=0; index<count && read("currentRow ? currentRow.identity : ''").toString()!=topic; ++index)
            QTest::keyClick(window,Qt::Key_Down);
        QCOMPARE(read("currentRow ? currentRow.identity : ''").toString(),topic);
        QTest::keyClick(window,Qt::Key_Return);
        QTRY_VERIFY(read("readerActive && articleOpen").toBool());
        auto article=guide->findChild<QQuickItem *>("ferricGuideArticle");
        QVERIFY(article); QTRY_VERIFY(article->hasActiveFocus());
        QVERIFY(article->width()>0 && article->height()>0);
        // Opening the compact reader hides the index and relays out the
        // article on the next frame. Measure the presented reader, not its
        // preceding wide-window geometry.
        QSignalSpy presented(window,&QQuickWindow::frameSwapped);
        window->requestUpdate(); QTRY_VERIFY_WITH_TIMEOUT(!presented.isEmpty(),5000);
        QVERIFY(QRectF(0,0,window->width(),window->height()).adjusted(-1,-1,1,1)
            .contains(article->mapRectToScene(article->boundingRect())));
        const auto artifacts=qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
        if (!artifacts.isEmpty()) {
            QVERIFY(QDir().mkpath(artifacts));
            QVERIFY(window->grabWindow().save(artifacts + "/guide-" + QString::fromLatin1(QTest::currentDataTag()) + ".png"));
        }
        auto scroll=guide->findChild<QQuickItem *>("ferricGuideReaderScroll");
        QVERIFY(scroll);
        auto viewport=qobject_cast<QQuickItem *>(scroll->property("contentItem").value<QObject *>());
        QVERIFY(viewport);
        if (viewport->property("contentHeight").toReal()>viewport->height()+1) {
            QTest::keyClick(window,Qt::Key_PageDown);
            QTRY_VERIFY(viewport->property("contentY").toReal()>0);
        }
        QTest::keyClick(window,Qt::Key_Escape);
        QTRY_VERIFY(!read("readerActive").toBool());
        QVERIFY(evaluate("testWindow.bindingHelpVisible").toBool());
        QTest::keyClick(window,Qt::Key_Escape);
        QTRY_VERIFY(!evaluate("testWindow.bindingHelpVisible").toBool());
    }

    void reloadExplainsWhenRestartIsNeeded() {
        write(configPath,"schema_version = 3\n[ui]\nfont_size_pt = 18\n[desktop]\nportals = 'required'\n");
        submit("config-reload");
        QTRY_COMPARE_WITH_TIMEOUT(evaluate("JSON.parse(testWindow.browserUi.config_json).ui.font_size_pt").toInt(),18,15000);
        const auto status=evaluate("testWindow.browserUi.status_text").toString();
        QVERIFY2(status.contains("browser restart") && !status.contains("lifecycle"),qPrintable(status));
        QCOMPARE(evaluate("JSON.parse(testWindow.browserUi.config_json).desktop.portals").toString(),QString("auto"));
    }

    void switcherEnterWaitsForCurrentQuery() {
        const auto origin=QString("http://127.0.0.1:%1").arg(server.serverPort());
        for (const auto &name : {QString("alpha"),QString("beta"),QString("gamma")}) {
            submit((name=="alpha" ? "open " : "tab-open ") + origin + "/" + name);
            QTRY_COMPARE_WITH_TIMEOUT(evaluate("testWindow.browserUi.current_url").toString(),origin+"/"+name,15000);
            QTRY_VERIFY(!evaluate("testWindow.activeWebView().loading").toBool());
        }
        submit("switcher --scope tabs");
        QTRY_VERIFY(evaluate("testWindow.switcherVisible").toBool());
        auto input=window->findChild<QQuickItem *>("switcherSearchInput"); QVERIFY(input);
        QTRY_VERIFY(input->hasActiveFocus());
        QVERIFY(input->setProperty("text","alpha"));
        QTRY_VERIFY(evaluate("testWindow.switcherResults.length===1 && testWindow.switcherResults[0].secondary.endsWith('/alpha')").toBool());
        QVERIFY(input->setProperty("text","beta"));
        // Deliver Enter in the same event turn as the edit, before the query
        // debounce can replace Alpha. A previous result must never be opened.
        QTest::keyClick(window,Qt::Key_Return);
        QCOMPARE(evaluate("testWindow.browserUi.current_url").toString(),origin+"/gamma");
        QVERIFY(evaluate("testWindow.switcherVisible").toBool());
        QTRY_VERIFY(evaluate("testWindow.switcherResults.length===1 && testWindow.switcherResults[0].secondary.endsWith('/beta')").toBool());
        evaluate("testWindow.refreshSwitcher()");
        QVERIFY(evaluate("testWindow.switcherResults.length===1 && testWindow.switcherResultsCurrent()").toBool());
        QTest::keyClick(window,Qt::Key_Return);
        QTRY_COMPARE(evaluate("testWindow.browserUi.current_url").toString(),origin+"/beta");
        QTRY_VERIFY(!evaluate("testWindow.switcherVisible").toBool());
    }

    void switcherQueriesStayWithinTheirWindow_data() {
        QTest::addColumn<QString>("kind");
        for(const auto &kind:{QString("regular"),QString("private"),QString("temporary")})
            QTest::newRow(qPrintable(kind))<<kind;
    }
    void switcherQueriesStayWithinTheirWindow() {
        QFETCH(QString,kind);
        QString owner="testWindow.browserUi";
        const auto primary=evaluate(owner+".tab_id_for_index(0)").toString();
        if(kind!="regular") {
            evaluate(QString("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:'rb://blank',windowProfileName:'switcher-owner',windowEphemeralInvocationToken:'switcher-session',windowPrivateProfile:")
                +(kind=="private"?"true":"false")+",windowEphemeralProfile:"
                +(kind=="temporary"?"true":"false")+"})");
            QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
            owner="testWindow.browserWindowRegistry[1].ui";
            QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.browserWindowRegistry[1].host.profileBootstrapComplete && !"+owner+".profile_bootstrap_pending").toBool(),15000);
        }
        QVERIFY2(evaluate(owner+".switcher_query('','tabs')").toBool(),qPrintable(evaluate(owner+".status_text").toString()));
        QCOMPARE(evaluate(owner+".switcher_result_ids.length").toInt(),1);
        QCOMPARE(evaluate(owner+".switcher_result_ids[0]").toString(),evaluate(owner+".tab_id_for_index(0)").toString());
        if(kind!="regular") QVERIFY(evaluate(owner+".switcher_result_ids[0]").toString()!=primary);
    }

};

int main(int argc, char **argv) {
    ferric_browser_enable_software_rendering(); ferric_browser_register_internal_scheme();
    QQuickWindow::setGraphicsApi(QSGRendererInterface::Software); QtWebEngineQuick::initialize();
    cxx_qt_init_crate_ferric_browser_engine_qt();
    QVector<QByteArray> arguments;
    for (int index = 0; index < argc; ++index) arguments.append(QByteArray(argv[index]));
    auto app = ferric_browser_gui_application(arguments);
    if (!app || !ferric_browser_gui_preferences_ready()) return 1;
    ContextReloadTests tests; return QTest::qExec(&tests, argc, argv);
}
#include "tst_context_reload.moc"
