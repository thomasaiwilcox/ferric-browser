#include "../../src/gui_application.h"
#include "../../src/startup_configuration.h"
#include "../../src/url_display.h"
#include "ferric-browser-engine-qt/src/bridge.cxxqt.h"
#include <QApplication>
#include <QFile>
#include <QDir>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQmlExpression>
#include <QQuickWindow>
#include <QTcpServer>
#include <QTcpSocket>
#include <QTemporaryDir>
#include <QtTest>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>

extern "C" bool cxx_qt_init_crate_ferric_browser_engine_qt();

class StartupConfigurationTests final : public QObject {
    Q_OBJECT
    bool sourceDocumentDone = false;
    QVariant sourceDocumentResult;
    static void write(const QString &path, const QByteArray &text) {
        QFile file(path);
        QVERIFY(file.open(QIODevice::WriteOnly | QIODevice::Truncate));
        QCOMPARE(file.write(text), text.size());
    }
    static QString prepare(QTemporaryDir &directory) {
        const auto path = directory.filePath("root.toml");
        write(path, "schema_version=3\ninclude=['included.toml']\nsite_rules=[]\n[hints.selectors.links]\nmode='replace'\nselectors=[]\n");
        write(directory.filePath("included.toml"), "[hints.selectors.links]\nmode='replace'\nselectors=['a[']\n");
        return path;
    }
public slots:
    void acceptSourceDocument(const QVariant &value) {
        sourceDocumentResult = value;
        sourceDocumentDone = true;
    }

private slots:
    void interactionSnapshotTracksRealPageRoutes() {
        QTemporaryDir directory;
        QVERIFY(directory.isValid());
        QTcpServer server;
        QVERIFY(server.listen(QHostAddress::LocalHost, 0));
        connect(&server, &QTcpServer::newConnection, &server, [&server] {
            while (auto *socket = server.nextPendingConnection()) {
                connect(socket, &QTcpSocket::readyRead, socket, [socket] {
                    if (socket->property("served").toBool()) return;
                    auto request = socket->property("request").toByteArray() + socket->readAll();
                    socket->setProperty("request", request);
                    if (!request.contains("\r\n\r\n")) return;
                    socket->setProperty("served", true);
                    const QByteArray body = "<!doctype html><title>Interaction cache</title><button id='main'>Main</button>";
                    socket->write("HTTP/1.1 200 OK\r\nContent-Length: " + QByteArray::number(body.size())
                        + "\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n" + body);
                    socket->disconnectFromHost();
                });
                connect(socket, &QTcpSocket::disconnected, socket, &QObject::deleteLater);
            }
        });
        const auto origin = QString("http://127.0.0.1:%1").arg(server.serverPort());
        const auto url = origin + "/ready";
        QVERIFY(QDir().mkpath(directory.filePath("configuration")));
        const auto path = directory.filePath("configuration/root.toml");
        write(path, (QString("schema_version=3\n[hints]\nauto_follow='never'\n"
            "[[site_rules]]\nid='ready'\npattern='%1/ready*'\n"
            "[[site_rules.interaction.anchors]]\nid='main'\ncode='m'\nfamilies=['buttons']\ntarget_selector='#main'\n"
            "[[site_rules]]\nid='disabled'\npattern='%1/disabled*'\n[site_rules.set]\n'content.zoom'=1.25\n[site_rules.interaction]\nenabled=false\n"
            "[[site_rules.interaction.anchors]]\nid='main'\ncode='m'\nfamilies=['buttons']\ntarget_selector='#main'\n"
            "[[site_rules]]\nid='conflict-anchor'\npattern='%1/conflict*'\n"
            "[[site_rules.interaction.anchors]]\nid='main'\ncode='m'\nfamilies=['buttons']\ntarget_selector='#main'\n"
            "[[site_rules]]\nid='conflict-list'\npattern='%1/conflict*'\n"
            "[[site_rules.interaction.collections]]\nid='list'\ncodes=['m']\nfamilies=['buttons']\n"
            "container_self=true\nitem_selector='button'\nprimary_self=true\norder='dom'\n").arg(origin)).toUtf8());
        const auto bundle = load_startup_configuration(path);
        const auto base = ferric_startup_base_json(bundle);
        QQmlApplicationEngine engine;
        engine.setInitialProperties({
            {"temporaryProfile", false}, {"ephemeralProfile", false},
            {"softwareRendering", true}, {"safeMode", true}, {"userscriptsOff", true},
            {"storageBasePath", directory.filePath("data")},
            {"startupConfigJson", base}, {"startupConfigBaseJson", base},
            {"startupConfigurationBundle", bundle}, {"startupConfigPath", path}, {"startupConfigSource", "user"},
            {"startupCliOverridesJson", "{\"schema_version\":3}"},
            {"startupProfileOverridesJson", "{\"schema_version\":3}"},
            {"startupContextsJson", "{\"schema_version\":3}"},
            {"startupUrl", url}, {"width", 640}, {"height", 480}
        });
        engine.load(QUrl("qrc:/qt/qml/io/github/ferricbrowser/qml/Main.qml"));
        QCOMPARE(engine.rootObjects().size(), 1);
        auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
        QVERIFY(window);
        engine.rootContext()->setContextProperty("testWindow", window);
        engine.rootContext()->setContextProperty("sourceDocumentProbe", this);
        auto evaluate = [&](const QString &script) {
            QQmlExpression expression(engine.rootContext(), window, script);
            const auto result = expression.evaluate();
            if (expression.hasError()) qFatal("QML Interaction cache probe: %s", qPrintable(expression.error().toString()));
            return result;
        };
        auto snapshot = [&] {
            return QJsonDocument::fromJson(evaluate("testWindow.browserUi.interaction_policy_snapshot()").toString().toUtf8()).object();
        };
        auto route = [&](const QString &path) {
            evaluate(QString("testWindow.activeWebView().runJavaScript(\"history.pushState({},'', '%1')\",0)").arg(path));
        };
        QTRY_VERIFY2_WITH_TIMEOUT(
            evaluate("testWindow.profileBootstrapComplete && !testWindow.browserUi.profile_bootstrap_pending").toBool(),
            qPrintable(evaluate("JSON.stringify({pending:testWindow.browserUi.profile_bootstrap_pending,complete:testWindow.profileBootstrapComplete,status:testWindow.browserUi.status_text,details:testWindow.browserUi.profile_startup_details(),selectors:testWindow.browserUi.selector_validation_request})").toString()),
            15000);
        QTRY_COMPARE_WITH_TIMEOUT(evaluate("testWindow.browserUi.page_title").toString(), QString("Interaction cache"), 15000);
        QTRY_COMPARE(snapshot()["state"].toString(), QString("ready"));
        const auto initial = snapshot();
        const auto initialSelectors = evaluate("testWindow.browserUi.hint_selector_policy('all')").toString();
        QVERIFY(!QJsonDocument::fromJson(initialSelectors.toUtf8()).object().contains("error"));
        QCOMPARE(initial["validation_stage"].toString(), QString("source-compilation"));
        QCOMPARE(initial["anchors"].toInt(), 1);
        QVERIFY(evaluate(QString("testWindow.browserUi.select_site_rule_settings('%1/disabled')").arg(origin)).toBool());
        QCOMPARE(snapshot(), initial);
        route("/ready?sort=new#hash");
        QTRY_COMPARE(evaluate("testWindow.browserUi.current_url").toString(), url + "?sort=new#hash");
        QTRY_COMPARE(snapshot()["state"].toString(), QString("ready"));
        QCOMPARE(snapshot(), initial);
        QCOMPARE(evaluate("testWindow.browserUi.hint_selector_policy('all')").toString(), initialSelectors);
        route("/disabled");
        QTRY_COMPARE(snapshot()["state"].toString(), QString("disabled"));
        QCOMPARE(snapshot()["anchors"].toInt(), 1);
        QVERIFY(!snapshot()["enabled"].toBool());
        QTRY_COMPARE(evaluate("testWindow.activeWebView().zoomFactor").toDouble(), 1.25);
        route("/ordinary");
        QTRY_COMPARE(snapshot()["state"].toString(), QString("no-op"));
        QCOMPARE(snapshot()["anchors"].toInt(), 0);
        QTRY_COMPARE(evaluate("testWindow.activeWebView().zoomFactor").toDouble(), 1.0);
        route("/conflict");
        QTRY_COMPARE(snapshot()["state"].toString(), QString("conflicted"));
        QVERIFY(snapshot()["diagnostic"].toString().contains("duplicate"));
        QVERIFY(snapshot()["anchors"].isNull());
        const auto conflicted = snapshot();
        QCOMPARE(snapshot(), conflicted);
        route("/ready");
        QTRY_COMPARE(snapshot()["state"].toString(), QString("ready"));
        // These are actual synchronous Rust key-handler entry points, not a
        // platform delivery qualification. Neither binding may compile policy
        // before returning; the existing runtime timer drains the dirty cache.
        QVERIFY(evaluate("testWindow.browserUi.execute_command('bind x set --temp hints.min_chars=2')").toBool());
        QTRY_VERIFY(evaluate("testWindow.browserUi.config_json.indexOf('set --temp hints.min_chars=2') >= 0").toBool());
        QTRY_COMPARE(snapshot()["state"].toString(), QString("ready"));
        const auto beforeBoundSnapshot = snapshot();
        const auto beforeBoundSetting = snapshot()["compilations"].toString().toULongLong();
        auto *boundUi = qobject_cast<BrowserUi *>(evaluate("testWindow.browserUi").value<QObject *>());
        QVERIFY(boundUi);
        QJsonObject dirtyBoundSetting;
        QObject publicationObserver;
        const auto publication = connect(boundUi, &BrowserUi::config_jsonChanged, &publicationObserver, [&] {
            const auto config = QJsonDocument::fromJson(boundUi->property("config_json").toString().toUtf8()).object();
            if (config.value("hints").toObject().value("min_chars").toInt() == 2)
                dirtyBoundSetting = snapshot();
        }, Qt::DirectConnection);
        QVERIFY(evaluate("testWindow.browserUi.handle_key('x')").toBool());
        QCOMPARE(snapshot(), beforeBoundSnapshot);
        QCOMPARE(snapshot()["compilations"].toString().toULongLong(), beforeBoundSetting);
        QTRY_VERIFY(!dirtyBoundSetting.isEmpty());
        disconnect(publication);
        QCOMPARE(dirtyBoundSetting["state"].toString(), QString("unavailable"));
        QCOMPARE(dirtyBoundSetting["reason"].toString(), QString("pending-refresh"));
        QCOMPARE(dirtyBoundSetting["compilations"].toString().toULongLong(), beforeBoundSetting);
        QTRY_COMPARE(snapshot()["state"].toString(), QString("ready"));
        QCOMPARE(snapshot()["compilations"].toString().toULongLong(), beforeBoundSetting + 1);
        const auto beforeReload = snapshot();
        window->show();
        window->requestActivate();
        QTRY_VERIFY(window->isActive());
        evaluate("testWindow.activeWebView().forceActiveFocus()");
        QTRY_VERIFY(evaluate("testWindow.hintWorkspace.hintInputOwnerActive && !testWindow.hintWorkspace.inputBlocked").toBool());
        QVERIFY(evaluate("testWindow.browserUi.execute_command('hint buttons')").toBool());
        QTRY_VERIFY(evaluate("testWindow.browserUi.hint_visible").toBool());
        QTRY_VERIFY(evaluate("JSON.parse(testWindow.browserUi.update_hint_interaction('clear','')).error === undefined").toBool());
        // config-reload is a Normal/Command action. Exercise the real watched
        // reload while retaining the live Hint owner by rewriting equal bytes.
        QFile reloadInput(path);
        QVERIFY(reloadInput.open(QIODevice::ReadOnly));
        const auto unchangedBytes = reloadInput.readAll();
        reloadInput.close();
        write(path, unchangedBytes);
        QTRY_VERIFY_WITH_TIMEOUT(snapshot()["state"].toString() == "ready"
            && snapshot()["configuration_revision"] != beforeReload["configuration_revision"], 15000);
        QCOMPARE(snapshot()["policy_revision"], beforeReload["policy_revision"]);
        QCOMPARE(snapshot()["anchor_source_revision"], snapshot()["reload_revision"]);
        QVERIFY(snapshot()["anchor_source_revision"] != beforeReload["anchor_source_revision"]);
        QCOMPARE(evaluate("testWindow.browserUi.mode").toString(), QString("hint"));
        QVERIFY(evaluate("JSON.parse(testWindow.browserUi.update_hint_interaction('clear','')).error === undefined").toBool());
        route("/disabled");
        QTRY_COMPARE(snapshot()["state"].toString(), QString("disabled"));
        QTRY_COMPARE(evaluate("testWindow.browserUi.mode").toString(), QString("normal"));
        QCOMPARE(evaluate("JSON.parse(testWindow.browserUi.update_hint_interaction('clear','')).error").toString(), QString("no-session"));
        const auto originalTab = evaluate("testWindow.browserUi.tab_id_for_index(0)").toString();
        QVERIFY(!originalTab.isEmpty());
        QVERIFY(evaluate(QString("testWindow.browserUi.execute_command('tab-open %1/ordinary')").arg(origin)).toBool());
        QTRY_COMPARE(evaluate("testWindow.browserUi.current_url").toString(), origin + "/ordinary");
        QTRY_COMPARE(snapshot()["state"].toString(), QString("no-op"));
        // Use a key without existing longer keychains (y is a yank prefix).
        QVERIFY(evaluate(QString("testWindow.browserUi.execute_command('bind F8 tab-focus %1')").arg(originalTab)).toBool());
        QTRY_VERIFY(evaluate(QString("testWindow.browserUi.config_json.indexOf('tab-focus %1') >= 0").arg(originalTab)).toBool());
        QTRY_COMPARE(snapshot()["state"].toString(), QString("no-op"));
        const auto beforeBoundTab = snapshot()["compilations"].toString().toULongLong();
        QVERIFY(evaluate("testWindow.browserUi.handle_key('F8')").toBool());
        QCOMPARE(snapshot()["state"].toString(), QString("unavailable"));
        QCOMPARE(snapshot()["compilations"].toString().toULongLong(), beforeBoundTab);
        QTRY_COMPARE(snapshot()["state"].toString(), QString("disabled"));
        QCOMPARE(evaluate("testWindow.browserUi.current_url").toString(), origin + "/disabled");
        QCOMPARE(snapshot()["compilations"].toString().toULongLong(), beforeBoundTab + 1);
        route("/ready");
        QTRY_COMPARE(snapshot()["state"].toString(), QString("ready"));
        const auto beforeRejectedRequest = snapshot();
        evaluate("testWindow.browserUi.navigate('https://[::1/ready')");
        QVERIFY2(evaluate("testWindow.browserUi.status_text.indexOf('Navigation rejected:') === 0").toBool(),
            qPrintable(evaluate("testWindow.browserUi.status_text").toString()));
        QCOMPARE(snapshot(), beforeRejectedRequest);
        QCOMPARE(evaluate("testWindow.activeWebView().url.toString()").toString(), url);

        evaluate("testWindow.activeWebView().forceActiveFocus()");
        QTRY_VERIFY(evaluate("testWindow.hintWorkspace.hintInputOwnerActive && !testWindow.hintWorkspace.inputBlocked").toBool());
        QVERIFY(evaluate("testWindow.browserUi.execute_command('hint buttons')").toBool());
        QTRY_VERIFY(evaluate("testWindow.browserUi.hint_visible").toBool());
        const auto beforeInvalidObservation = snapshot()["compilations"].toString();
        // This is a real WebEngine document, deliberately outside Core's URL
        // grammar. The reducer's old address must never become a cache fallback.
        evaluate("testWindow.activeWebView().url='data:text/html,%3Ctitle%3ERejected%20route%3C/title%3E%3Cbutton%3EData%3C/button%3E'");
        QTRY_COMPARE_WITH_TIMEOUT(evaluate("testWindow.activeWebView().title").toString(), QString("Rejected route"), 15000);
        QVERIFY(evaluate("testWindow.activeWebView().url.toString().indexOf('data:') === 0").toBool());
        QTRY_COMPARE(snapshot()["reason"].toString(), QString("rejected-engine-context"));
        QCOMPARE(snapshot()["state"].toString(), QString("unavailable"));
        QCOMPARE(evaluate("testWindow.browserUi.mode").toString(), QString("normal"));
        QCOMPARE(evaluate("JSON.parse(testWindow.browserUi.update_hint_interaction('clear','')).error").toString(), QString("no-session"));
        QCOMPARE(evaluate("testWindow.browserUi.current_url").toString(), url);
        QVERIFY(evaluate("testWindow.browserUi.execute_command('set --temp hints.min_chars=3')").toBool());
        QTRY_VERIFY(evaluate("JSON.parse(testWindow.browserUi.config_json).hints.min_chars === 3").toBool());
        evaluate("testWindow.browserUi.poll_interaction_policy_refresh()");
        QCOMPARE(snapshot()["reason"].toString(), QString("rejected-engine-context"));
        QCOMPARE(snapshot()["compilations"].toString(), beforeInvalidObservation);
        evaluate(QString("testWindow.activeWebView().url='%1'").arg(url));
        QTRY_COMPARE_WITH_TIMEOUT(evaluate("testWindow.activeWebView().title").toString(), QString("Interaction cache"), 15000);
        QTRY_COMPARE(snapshot()["state"].toString(), QString("ready"));

        // Constructed callback-boundary negatives complement the actual engine
        // document above; these do not qualify renderer document-proof delivery.
        const auto recovered = snapshot();
        evaluate("(function(){var u=testWindow.browserUi,i=u.active_tab_index,t=u.tab_id_for_index(i),e=u.input_epoch;"
            "u.page_document_identity_for(i,t,'policy-current',e);"
            "u.begin_same_document_url_change_for(i,t,'data:text/html,bad','policy-older',e);"
            "u.begin_same_document_url_change_for(i,t,'data:text/html,bad','policy-current',e-1);"
            "u.begin_same_document_url_change_for(i,'tabid-999999','data:text/html,bad','policy-current',e);"
            "u.complete_same_document_url_change_for(i,t,'data:text/html,bad','policy-current',e);"
            "u.navigation_url_changed_for(9999,'data:text/html,bad');})()");
        QCOMPARE(snapshot(), recovered);
        evaluate("(function(){var u=testWindow.browserUi,i=u.active_tab_index,t=u.tab_id_for_index(i);"
            "u.begin_same_document_url_change_for(i,t,'data:text/html,bad','policy-current',u.input_epoch);})()");
        QCOMPARE(snapshot()["reason"].toString(), QString("rejected-engine-context"));
        evaluate(QString("testWindow.browserUi.complete_same_document_url_change_for(0,'%1','%2','policy-current',testWindow.browserUi.input_epoch-1)").arg(originalTab, url));
        evaluate("testWindow.browserUi.poll_interaction_policy_refresh()");
        QCOMPARE(snapshot()["reason"].toString(), QString("rejected-engine-context"));
        evaluate(QString("testWindow.browserUi.navigation_url_changed_for(0,'%1')").arg(url));
        QTRY_COMPARE(snapshot()["state"].toString(), QString("ready"));

        evaluate("testWindow.activeWebView().forceActiveFocus()");
        QTRY_VERIFY(evaluate("testWindow.hintWorkspace.hintInputOwnerActive && !testWindow.hintWorkspace.inputBlocked").toBool());
        QVERIFY(evaluate("testWindow.browserUi.execute_command('hint buttons')").toBool());
        QTRY_VERIFY(evaluate("testWindow.browserUi.hint_visible").toBool());
        const auto beforeBackgroundObservation = snapshot();
        evaluate("testWindow.tabViewAt(1).url='data:text/html,%3Ctitle%3ERejected%20background%3C/title%3E'");
        QTRY_COMPARE_WITH_TIMEOUT(evaluate("testWindow.tabViewAt(1).title").toString(), QString("Rejected background"), 15000);
        QCOMPARE(snapshot(), beforeBackgroundObservation);
        QCOMPARE(evaluate("testWindow.browserUi.mode").toString(), QString("hint"));
        QVERIFY(evaluate("JSON.parse(testWindow.browserUi.update_hint_interaction('clear','')).error === undefined").toBool());
        evaluate("testWindow.browserUi.escape()");
        const auto backgroundTab = evaluate("testWindow.browserUi.tab_id_for_index(1)").toString();
        QVERIFY(evaluate(QString("testWindow.browserUi.execute_command('tab-focus %1')").arg(backgroundTab)).toBool());
        QTRY_COMPARE(snapshot()["reason"].toString(), QString("rejected-engine-context"));
        QCOMPARE(evaluate("testWindow.activeWebView().title").toString(), QString("Rejected background"));
        evaluate(QString("testWindow.activeWebView().url='%1/ordinary'").arg(origin));
        QTRY_COMPARE_WITH_TIMEOUT(evaluate("testWindow.activeWebView().title").toString(), QString("Interaction cache"), 15000);
        QTRY_COMPARE(snapshot()["state"].toString(), QString("no-op"));
        // A valid notification with no established document keeps rejection
        // closed until the subsequent current document probe proves recovery.
        evaluate(QString("(function(){var u=testWindow.browserUi,i=u.active_tab_index,t=u.tab_id_for_index(i);"
            "u.navigation_started_for(i,'%1/ordinary');u.navigation_url_changed_for(i,'data:text/html,bad');"
            "u.begin_same_document_url_change_for(i,t,'%1/ordinary','policy-probe',u.input_epoch);})()").arg(origin));
        evaluate("testWindow.browserUi.poll_interaction_policy_refresh()");
        QCOMPARE(snapshot()["reason"].toString(), QString("rejected-engine-context"));
        evaluate(QString("(function(){var u=testWindow.browserUi,i=u.active_tab_index,t=u.tab_id_for_index(i),e=u.input_epoch;"
            "u.page_document_identity_for(i,t,'policy-probe',e);"
            "u.complete_same_document_url_change_for(i,t,'%1/ordinary','policy-probe',e);})()").arg(origin));
        QTRY_COMPARE(snapshot()["state"].toString(), QString("no-op"));
        evaluate("testWindow.activeWebView().url='about:blank'");
        QTRY_COMPARE(evaluate("testWindow.browserUi.current_url").toString(), QString("about:blank"));
        QTRY_COMPARE(snapshot()["state"].toString(), QString("unavailable"));
        QTRY_COMPARE(snapshot()["reason"].toString(), QString("unsupported-context"));
        window->setProperty("windowShutdownApproved", true);
        window->close();
    }

    void sourceAwareHintDiscoveryPublication_data() {
        QTest::addColumn<bool>("temporary");
        QTest::newRow("temporary") << true;
        QTest::newRow("generated") << false;
    }

    void sourceAwareHintDiscoveryPublication() {
        QFETCH(bool, temporary);
        QTemporaryDir directory;
        QVERIFY(directory.isValid());
        QTcpServer server;
        QVERIFY(server.listen(QHostAddress::LocalHost, 0));
        connect(&server, &QTcpServer::newConnection, &server, [&server] {
            while (auto *socket = server.nextPendingConnection()) {
                connect(socket, &QTcpSocket::readyRead, socket, [socket] {
                    if (socket->property("served").toBool()) return;
                    auto request = socket->property("request").toByteArray();
                    request += socket->readAll();
                    socket->setProperty("request", request);
                    if (!request.contains("\r\n\r\n")) return;
                    socket->setProperty("served", true);
                    const QByteArray body = "<!doctype html><title>Source policy</title>"
                        "<button class='site'>Site button</button><button class='base'>Base button</button>";
                    socket->write("HTTP/1.1 200 OK\r\nContent-Length: " + QByteArray::number(body.size())
                        + "\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n" + body);
                    socket->disconnectFromHost();
                });
                connect(socket, &QTcpSocket::disconnected, socket, &QObject::deleteLater);
            }
        });
        const auto url = QString("http://127.0.0.1:%1/page").arg(server.serverPort());
        QVERIFY(QDir().mkpath(directory.filePath("configuration")));
        const auto path = directory.filePath("configuration/root.toml");
        write(path, (QString("schema_version=3\n[hints]\nauto_follow='never'\n"
            "[hints.selectors.buttons]\nmode='replace'\nselectors=['.base']\n"
            "[[site_rules]]\nid='site-hints'\npattern='http://127.0.0.1:%1/*'\n"
            "[site_rules.set]\n'hints.proxy_targets'='validated'\n"
            "'hints.selectors.buttons'={mode='replace',selectors=['.site']}\n")
            .arg(server.serverPort())).toUtf8());
        const auto bundle = load_startup_configuration(path);
        const auto base = ferric_startup_base_json(bundle);
        QQmlApplicationEngine engine;
        engine.setInitialProperties({
            {"temporaryProfile", false}, {"ephemeralProfile", false},
            {"softwareRendering", true}, {"safeMode", true}, {"userscriptsOff", true},
            {"storageBasePath", directory.filePath("data")},
            {"startupConfigJson", base}, {"startupConfigBaseJson", base},
            {"startupConfigurationBundle", bundle}, {"startupConfigPath", path},
            {"startupConfigSource", "user"},
            {"startupCliOverridesJson", "{\"schema_version\":3}"},
            {"startupProfileOverridesJson", "{\"schema_version\":3}"},
            {"startupContextsJson", "{\"schema_version\":3}"},
            {"startupUrl", url}, {"width", 640}, {"height", 480}
        });
        engine.load(QUrl("qrc:/qt/qml/io/github/ferricbrowser/qml/Main.qml"));
        QCOMPARE(engine.rootObjects().size(), 1);
        auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
        QVERIFY(window);
        engine.rootContext()->setContextProperty("testWindow", window);
        engine.rootContext()->setContextProperty("sourceDocumentProbe", this);
        auto evaluate = [&](const QString &script) {
            QQmlExpression expression(engine.rootContext(), window, script);
            const auto result = expression.evaluate();
            if (expression.hasError()) qFatal("QML source-policy probe: %s", qPrintable(expression.error().toString()));
            return result;
        };
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.profileBootstrapComplete && !testWindow.browserUi.profile_bootstrap_pending").toBool(), 15000);
        QTRY_COMPARE_WITH_TIMEOUT(evaluate("testWindow.browserUi.current_url").toString(), url, 15000);
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("!!testWindow.activeWebView() && !testWindow.activeWebView().loading").toBool(), 15000);
        // An initial loading=false can precede the queued startup navigation.
        // Observe the actual document before entering a session it could retire.
        QTRY_COMPARE_WITH_TIMEOUT(evaluate("testWindow.browserUi.page_title").toString(), QString("Source policy"), 15000);
        sourceDocumentDone = false;
        evaluate("testWindow.activeWebView().runJavaScript(\"JSON.stringify({ready:document.readyState,href:location.href,button:!!document.querySelector('button.site')})\",0,function(value){sourceDocumentProbe.acceptSourceDocument(value)})");
        QTRY_VERIFY(sourceDocumentDone);
        const auto document = QJsonDocument::fromJson(sourceDocumentResult.toString().toUtf8()).object();
        QCOMPARE(document["ready"].toString(), QString("complete"));
        QCOMPARE(document["href"].toString(), url);
        QVERIFY(document["button"].toBool());
        auto snapshot = [&] { return evaluate("testWindow.browserUi.hint_selector_policy('buttons')").toString(); };
        auto selected = [&] { return QJsonDocument::fromJson(snapshot().toUtf8()).object()["policies"].toObject()["buttons"].toObject()["policy"].toObject()["selectors"].toArray(); };
        QTRY_COMPARE(selected(), QJsonArray{QString(".site")});
        QTRY_VERIFY(evaluate("testWindow.browserUi.hint_proxy_targets_enabled").toBool());
        const auto beforeBackgroundQuery = snapshot();
        QVERIFY(evaluate("testWindow.browserUi.select_site_rule_settings('https://unmatched.test/')").toBool());
        QVERIFY(evaluate("testWindow.browserUi.hint_proxy_targets_enabled").toBool());
        QCOMPARE(snapshot(), beforeBackgroundQuery);
        const auto original = QJsonDocument::fromJson(evaluate("testWindow.browserUi.config_json").toString().toUtf8());
        window->show();
        window->requestActivate();
        QTRY_VERIFY(window->isActive());
        evaluate("testWindow.activeWebView().forceActiveFocus()");
        QTRY_VERIFY(evaluate("testWindow.hintWorkspace.hintInputOwnerActive && !testWindow.hintWorkspace.inputBlocked").toBool());
        QVERIFY(evaluate("testWindow.browserUi.execute_command('hint buttons')").toBool());
        QTRY_COMPARE(evaluate("testWindow.browserUi.mode").toString(), QString("hint"));
        QTRY_VERIFY2(evaluate("testWindow.browserUi.hint_visible").toBool(),
            qPrintable(evaluate("JSON.stringify({mode:testWindow.browserUi.mode,status:testWindow.browserUi.status_text,owner:testWindow.hintWorkspace.hintInputOwnerActive,blocked:testWindow.hintWorkspace.inputBlocked,rows:testWindow.hintWorkspace.hintResults.length,pending:testWindow.hintWorkspace.hintRefreshPending})").toString()));
        QTRY_VERIFY(evaluate("JSON.parse(testWindow.browserUi.update_hint_interaction('clear','')).error === undefined").toBool());
        const auto scope = temporary ? "true" : "false";
        QVERIFY(evaluate(QString("testWindow.browserUi.set_runtime_setting('hints.proxy_targets','off',%1)").arg(scope)).toBool());
        QTRY_COMPARE(evaluate("testWindow.browserUi.setting_action_state").toString(), QString("succeeded"));
        QVERIFY(!evaluate("testWindow.browserUi.hint_proxy_targets_enabled").toBool());
        QCOMPARE(QJsonDocument::fromJson(evaluate("testWindow.browserUi.config_json").toString().toUtf8()), original);
        QCOMPARE(evaluate("testWindow.browserUi.mode").toString(), QString("normal"));
        QVERIFY(!evaluate("testWindow.browserUi.hint_visible").toBool());
        QCOMPARE(evaluate("JSON.parse(testWindow.browserUi.update_hint_interaction('clear','')).error").toString(), QString("no-session"));
        const auto beforePending = snapshot();
        QVERIFY(evaluate(QString("testWindow.browserUi.set_runtime_setting('hints.selectors.buttons',\"{mode='replace',selectors=['.base']}\",%1)").arg(scope)).toBool());
        QTRY_COMPARE(evaluate("testWindow.browserUi.setting_action_state").toString(), QString("succeeded"));
        QCOMPARE(snapshot(), beforePending);
        QVERIFY(evaluate("testWindow.browserUi.execute_command('config-reload')").toBool());
        QTRY_COMPARE_WITH_TIMEOUT(selected(), QJsonArray{QString(".base")}, 15000);
        QVERIFY(snapshot() != beforePending);
        const auto committed = snapshot();
        // Re-reading identical sources must not revoke selector/Hint authority.
        QVERIFY(evaluate("testWindow.browserUi.execute_command('config-reload')").toBool());
        QTRY_VERIFY(evaluate("testWindow.browserUi.status_text.indexOf('Settings reloaded') === 0").toBool());
        QCOMPARE(snapshot(), committed);
        QVERIFY(evaluate(QString("testWindow.browserUi.set_runtime_setting('hints.selectors.buttons',\"{mode='replace',selectors=['button[']}\",%1)").arg(scope)).toBool());
        QTRY_COMPARE(evaluate("testWindow.browserUi.setting_action_state").toString(), QString("failed"));
        const auto rejection = evaluate("testWindow.browserUi.runtime_setting_error").toString();
        QVERIFY2(rejection.contains("buttons") && rejection.contains("button[")
            && rejection.contains("SyntaxError") && !rejection.contains("timed out"), qPrintable(rejection));
        QCOMPARE(snapshot(), committed);
        QVERIFY(!evaluate("testWindow.browserUi.hint_proxy_targets_enabled").toBool());
        window->setProperty("windowShutdownApproved", true);
        window->close();
    }

    void nativeBundleRejectsMissingWrongTypeMismatchedBaseAndReuse() {
        QTemporaryDir directory;
        QVERIFY(directory.isValid());
        const auto path = prepare(directory);
        const auto bundle = load_startup_configuration(path);
        const auto base = ferric_startup_base_json(bundle);
        const QString overrides = "{\"schema_version\":3}";
        const QString source = "user";
        BrowserUi ui;
        QVERIFY(!ui.set_startup_configuration(base, base, overrides, overrides, path, source, QVariant()));
        QVERIFY(!ui.set_startup_configuration(base, base, overrides, overrides, path, source, QVariant("forged")));
        auto changed = QJsonDocument::fromJson(base.toUtf8()).object();
        changed.insert("hints", QJsonObject{{"chars", "abcd"}});
        const QString mismatch = QString::fromUtf8(QJsonDocument(changed).toJson(QJsonDocument::Compact));
        QVERIFY(!ui.set_startup_configuration(mismatch, mismatch, overrides, overrides, path, source, bundle));
        QVERIFY(ui.set_startup_configuration(base, base, overrides, overrides, path, source, bundle));
        QVERIFY(!ui.set_startup_configuration(base, base, overrides, overrides, path, source, bundle));
    }

    void qmlStartupValidatesTheCapturedSourcesAfterFilesChange() {
        QTemporaryDir directory;
        QVERIFY(directory.isValid());
        const auto path = prepare(directory);
        auto bundle = load_startup_configuration(path);
        const auto base = ferric_startup_base_json(bundle);
        // A later worker read sees valid replacement data. Startup must still
        // validate the catalogue captured together with its original base.
        write(directory.filePath("included.toml"), "");
        QQmlApplicationEngine engine;
        engine.setInitialProperties({
            {"temporaryProfile", true}, {"ephemeralProfile", true},
            {"softwareRendering", true}, {"userscriptsOff", true},
            {"storageBasePath", directory.filePath("data")},
            {"startupConfigJson", base}, {"startupConfigBaseJson", base},
            {"startupConfigurationBundle", bundle}, {"startupConfigPath", path},
            {"startupConfigSource", "user"},
            {"startupCliOverridesJson", "{\"schema_version\":3}"},
            {"startupProfileOverridesJson", "{\"schema_version\":3}"},
            {"startupContextsJson", "{\"schema_version\":3}"},
            {"startupUrl", "rb://blank"}, {"width", 640}, {"height", 480}
        });
        engine.load(QUrl("qrc:/qt/qml/io/github/ferricbrowser/qml/Main.qml"));
        QCOMPARE(engine.rootObjects().size(), 1);
        auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
        QVERIFY(window);
        engine.rootContext()->setContextProperty("testWindow", window);
        auto evaluate = [&](const QString &script) {
            QQmlExpression expression(engine.rootContext(), window, script);
            const auto result = expression.evaluate();
            if (expression.hasError()) qFatal("QML probe: %s", qPrintable(expression.error().toString()));
            return result;
        };
        bundle.clear();
        QTRY_VERIFY2_WITH_TIMEOUT(
            evaluate("testWindow.profileBootstrapComplete && !testWindow.browserUi.profile_bootstrap_pending").toBool(),
            qPrintable(evaluate("JSON.stringify({pending:testWindow.browserUi.profile_bootstrap_pending,complete:testWindow.profileBootstrapComplete,status:testWindow.browserUi.status_text,details:testWindow.browserUi.profile_startup_details(),selectors:testWindow.browserUi.selector_validation_request})").toString()),
            15000);
        const auto detail = evaluate("testWindow.browserUi.profile_startup_details()").toString();
        QVERIFY2(detail.contains("base document 1"), qPrintable(detail));
        QVERIFY2(detail.contains("selector", Qt::CaseInsensitive), qPrintable(detail));
        QVERIFY(evaluate("Object.keys(JSON.parse(testWindow.browserUi.config_json).hints.selectors).length === 0").toBool());
        const auto recaptured = evaluate("testWindow.browserUi.capture_startup_configuration()");
        QCOMPARE(ferric_startup_base_json(recaptured), base);
        BrowserUi secondary;
        const auto effective = evaluate("testWindow.browserUi.config_json").toString();
        const QString overrides = "{\"schema_version\":3}";
        QVERIFY(secondary.set_startup_configuration(effective, base, overrides, overrides, path, "user", recaptured));
        QVERIFY(evaluate("!testWindow.browserUi.set_startup_configuration(testWindow.startupConfigJson, testWindow.startupConfigBaseJson, testWindow.startupCliOverridesJson, testWindow.startupProfileOverridesJson, testWindow.startupConfigPath, testWindow.startupConfigSource, testWindow.startupConfigurationBundle)").toBool());
        window->setProperty("windowShutdownApproved", true);
        window->close();
    }
};

int main(int argc, char **argv) {
    ferric_browser_enable_software_rendering();
    ferric_browser_register_internal_scheme();
    QQuickWindow::setGraphicsApi(QSGRendererInterface::Software);
    QtWebEngineQuick::initialize();
    cxx_qt_init_crate_ferric_browser_engine_qt();
    QVector<QByteArray> arguments;
    for (int index = 0; index < argc; ++index) arguments.append(QByteArray(argv[index]));
    auto app = ferric_browser_gui_application(arguments);
    if (!app || !ferric_browser_gui_preferences_ready()) return 1;
    StartupConfigurationTests tests;
    return QTest::qExec(&tests, argc, argv);
}
#include "tst_startup_configuration.moc"
