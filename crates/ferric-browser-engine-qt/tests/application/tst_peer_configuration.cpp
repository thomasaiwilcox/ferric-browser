#include "tst_application.h"
#include "ferric-browser-engine-qt/src/bridge.cxxqt.h"
#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QJsonDocument>
#include <QJsonArray>
#include <QJsonObject>
#include <QMetaObject>
#include <QQmlContext>
#include <QQmlEngine>
#include <QScopeGuard>
#include <QTest>
#ifdef Q_OS_UNIX
#include <sys/stat.h>
#endif

void ApplicationTests::mixedLoadedEnumNormalization_data() {
    QTest::addColumn<QString>("source");
    QTest::addColumn<QString>("value");
    QTest::newRow("profile-string") << QString("profile") << QString("\"mixed\"");
    QTest::newRow("profile-unit-enum") << QString("profile") << QString("{\"mixed\":{}}");
    QTest::newRow("cli-string") << QString("cli") << QString("\"mixed\"");
    QTest::newRow("cli-unit-enum") << QString("cli") << QString("{\"mixed\":{}}");
}

void ApplicationTests::mixedLoadedEnumNormalization() {
    QFETCH(QString, source);
    QFETCH(QString, value);
    const auto schema = QJsonDocument::fromJson(text("profile_overrides_json").toUtf8())
                            .object()["schema_version"].toInt();
    QVERIFY(schema > 0);
    const auto overrides = QString("{\"schema_version\":%1,\"settings\":{\"input.shortcut_policy\":%2,\"input.page_keys\":[\"k\"]}}")
                               .arg(schema).arg(value);
    evaluate("testWindow.browserUi.config_base_json=" + quoted(R"JSON({"schema_version":3,"site_rules":[{"id":"ordinary-site","pattern":"https://example.test/*","priority":0,"set":{"content.zoom":1.25,"content.javascript":false}}]})JSON"));
    evaluate("testWindow.browserUi." + source + "_overrides_json=" + quoted(overrides));
    evaluate("testWindow.browserUi.configure_profile(true,false,'Mixed normalization','mixed-normalization',testWindow.storageBasePath)");
    QTRY_VERIFY_WITH_TIMEOUT(!evaluate("testWindow.browserUi.profile_bootstrap_pending").toBool(), 10000);
    const auto selected = QJsonDocument::fromJson(text("config_json").toUtf8()).object();
    QCOMPARE(selected["input"].toObject()["shortcut_policy"].toString(), QString("mixed"));
    QTRY_COMPARE(evaluate("testWindow.browserUi.mixed_key_route('k')").toString(), QString("page"));
    auto status = QJsonDocument::fromJson(evaluate("testWindow.browserUi.refresh_interaction_status()")
                                            .toString().toUtf8()).object()["shortcut_policy"].toObject();
    QCOMPARE(status["status"].toString(), QString("ready"));
    QVERIFY(status["unavailable_reason"].isNull());
    QCOMPARE(status["value"].toString(), QString("mixed"));
    QCOMPARE(status["source"].toString(), source == "profile" ? QString("profile") : QString("runtime"));
    QCOMPARE(status["page_key_count"].toInt(), 1);
    // Exercise the navigation projection too, which must use the selected
    // application's canonical layers rather than reparsing presentation data.
    QVERIFY(evaluate("testWindow.browserUi.select_site_rule_settings('https://example.test/route')").toBool());
    QCOMPARE(evaluate("testWindow.browserUi.mixed_key_route('k')").toString(), QString("page"));
    QVERIFY(evaluate("testWindow.browserUi.site_rule_zoom_set").toBool());
    QCOMPARE(evaluate("testWindow.browserUi.site_rule_zoom").toDouble(), 1.25);
    QVERIFY(evaluate("testWindow.browserUi.site_rule_javascript_set").toBool());
    QVERIFY(!evaluate("testWindow.browserUi.site_rule_javascript_enabled").toBool());
    QVERIFY(evaluate("testWindow.browserUi.select_site_rule_settings('https://other.test/')").toBool());
    QVERIFY(!evaluate("testWindow.browserUi.site_rule_zoom_set").toBool());
    QVERIFY(!evaluate("testWindow.browserUi.site_rule_javascript_set").toBool());
}

void ApplicationTests::mixedPolicySelectedTabOwnership() {
    const auto origin = QString("http://127.0.0.1:%1").arg(server.serverPort());
    const auto a = origin + "/mixed-a/page";
    const auto b = origin + "/mixed-b/page";
    const auto rule = [&](const QString &id, const QString &path, const QString &key) {
        return QJsonObject{{"id", id}, {"pattern", origin + path + "/*"}, {"priority", 0},
            {"set", QJsonObject{{"input.shortcut_policy", "mixed"},
                                {"input.page_keys", QJsonArray{key}},
                                {"hints.proxy_targets", id == "background-b" ? "validated" : "off"}}}};
    };
    const auto base = QJsonDocument(QJsonObject{{"schema_version", 3},
        {"site_rules", QJsonArray{rule("selected-a", "/mixed-a", "k"), rule("background-b", "/mixed-b", "f")}}})
                          .toJson(QJsonDocument::Compact);
    evaluate("testWindow.browserUi.config_base_json=" + quoted(QString::fromUtf8(base)));
    evaluate("testWindow.browserUi.configure_profile(true,false,'Mixed ownership','mixed-ownership',testWindow.storageBasePath)");
    QTRY_VERIFY_WITH_TIMEOUT(!evaluate("testWindow.browserUi.profile_bootstrap_pending").toBool(), 10000);
    QVERIFY(command("open --target current " + a));
    QTRY_COMPARE(text("current_url"), a);
    QTRY_COMPARE(evaluate("testWindow.browserUi.mixed_key_route('k')").toString(), QString("page"));
    QTRY_VERIFY(!evaluate("testWindow.activeWebView().loading").toBool());
    QTRY_COMPARE(evaluate("testWindow.browserUi.mixed_key_route('k')").toString(), QString("page"));
    engine->rootContext()->setContextProperty("mixedSavedWindowFactory", evaluate("testWindow.browserWindowComponentObject").value<QObject *>());
    evaluate("testWindow.browserWindowComponentObject=null");
    const auto restoreFactory = qScopeGuard([&] { evaluate("testWindow.browserWindowComponentObject=mixedSavedWindowFactory"); });
    QVERIFY(command("open --target tab-bg " + b));
    evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
    QTRY_COMPARE(evaluate("testWindow.tabViews.length").toInt(), 2);
    QTRY_COMPARE(evaluate("testWindow.tabViewAt(1).url.toString()").toString(), b);
    QTRY_VERIFY(!evaluate("testWindow.tabViewAt(1).loading").toBool());
    pageReceiver = "testWindow.tabViewAt(1)";
    QTRY_VERIFY(page("document.readyState==='complete' && Array.isArray(window.keys)").toBool());
    pageReceiver = "testWindow.activeWebView()";
    QCOMPARE(text("current_url"), a);
    QCOMPARE(evaluate("testWindow.browserUi.mixed_key_route('k')").toString(), QString("page"));
    QCOMPARE(evaluate("testWindow.browserUi.mixed_key_route('f')").toString(), QString("browser"));
    QTRY_VERIFY(!evaluate("testWindow.browserUi.hint_proxy_targets_enabled").toBool());
    const auto reentrySource = QString::fromUtf8(R"QML(import QtQuick; Connections {
        property bool armed:false; property bool switched:false;
        target:testWindow.browserUi;
        function onBinding_overlayChanged(){
            if(armed && target.binding_overlay.length===0){
                armed=false; switched=true; target.select_tab(0);
            }
        }
    })QML");
    const QPointer<QObject> reentry(evaluate("Qt.createQmlObject(" + quoted(reentrySource) + ",testWindow)").value<QObject *>());
    QVERIFY(reentry);
    engine->rootContext()->setContextProperty("mixedReentryProbe", reentry.data());
    const auto reentryResult = evaluate(R"JS((function(){
        const ui=testWindow.browserUi;
        ui.select_tab(1);
        ui.binding_overlay="reentry fixture";
        mixedReentryProbe.armed=true;
        ui.poll_interaction_policy_refresh();
        return {switched:mixedReentryProbe.switched,index:ui.active_tab_index,
                proxy:ui.hint_proxy_targets_enabled,route:ui.mixed_key_route("k")};
    })())JS").toMap();
    QVERIFY(reentryResult.value("switched").toBool());
    QCOMPARE(reentryResult.value("index").toInt(), 0);
    QVERIFY(!reentryResult.value("proxy").toBool());
    QCOMPARE(reentryResult.value("route").toString(), QString("unavailable"));
    delete reentry.data();
    QTRY_COMPARE(evaluate("testWindow.browserUi.mixed_key_route('k')").toString(), QString("page"));
    const auto revision = QJsonDocument::fromJson(evaluate("testWindow.browserUi.refresh_interaction_status()")
        .toString().toUtf8()).object().value("shortcut_policy_revision");
    evaluate("testWindow.tabViewAt(1).refreshEffectiveSiteSettings()");
    QVERIFY(evaluate("testWindow.browserUi.select_site_rule_settings(" + quoted(b) + ")").toBool());
    QCOMPARE(evaluate("testWindow.browserUi.mixed_key_route('k')").toString(), QString("page"));
    QCOMPARE(QJsonDocument::fromJson(evaluate("testWindow.browserUi.refresh_interaction_status()")
        .toString().toUtf8()).object().value("shortcut_policy_revision"), revision);
    evaluate("testWindow.tabViewAt(1).url=" + quoted(b + "?next"));
    QTRY_COMPARE(evaluate("testWindow.tabViewAt(1).url.toString()").toString(), b + "?next");
    QTRY_VERIFY(!evaluate("testWindow.tabViewAt(1).loading").toBool());
    QCOMPARE(text("current_url"), a);
    QCOMPARE(evaluate("testWindow.browserUi.mixed_key_route('k')").toString(), QString("page"));
    QVERIFY(evaluate("testWindow.browserUi.select_tab(1)").toBool());
    QTRY_COMPARE(text("current_url"), b + "?next");
    QTRY_COMPARE(evaluate("testWindow.browserUi.mixed_key_route('f')").toString(), QString("page"));
    QCOMPARE(evaluate("testWindow.browserUi.mixed_key_route('k')").toString(), QString("browser"));
    // Returning to an already loaded tab needs no URL change signal.
    QVERIFY(evaluate("testWindow.browserUi.select_tab(0)").toBool());
    QTRY_COMPARE(text("current_url"), a);
    QTRY_COMPARE(evaluate("testWindow.browserUi.mixed_key_route('k')").toString(), QString("page"));
    QCOMPARE(evaluate("testWindow.browserUi.mixed_key_route('f')").toString(), QString("browser"));
    QCOMPARE(page("history.pushState({},'', '/mixed-b/spa');location.pathname").toString(), QString("/mixed-b/spa"));
    QTRY_COMPARE(text("current_url"), origin + "/mixed-b/spa");
    QTRY_COMPARE(evaluate("testWindow.browserUi.mixed_key_route('f')").toString(), QString("page"));
    QCOMPARE(evaluate("testWindow.browserUi.mixed_key_route('k')").toString(), QString("browser"));
    // Actual unsupported engine navigation closes the previous selected route.
    evaluate("testWindow.activeWebView().url='data:text/html,%3Ctitle%3EMixed%20rejected%3C/title%3E'");
    QTRY_COMPARE(evaluate("testWindow.activeWebView().title").toString(), QString("Mixed rejected"));
    QTRY_COMPARE(evaluate("testWindow.browserUi.mixed_key_route('f')").toString(), QString("unavailable"));
    const auto unavailable = QJsonDocument::fromJson(evaluate("testWindow.browserUi.refresh_interaction_status()")
        .toString().toUtf8()).object()["shortcut_policy"].toObject();
    QCOMPARE(unavailable["status"].toString(), QString("unavailable"));
    QVERIFY(unavailable["value"].isNull());
    QVERIFY(unavailable["source"].isNull());
    QCOMPARE(evaluate("testWindow.browserUi.mixed_key_route('Ctrl+Escape')").toString(), QString("reserved"));
    evaluate("testWindow.activeWebView().url=" + quoted(a));
    QTRY_COMPARE(evaluate("testWindow.activeWebView().url.toString()").toString(), a);
    QTRY_COMPARE(evaluate("testWindow.browserUi.mixed_key_route('k')").toString(), QString("page"));
    evaluate("testWindow.tabViewAt(1).url=" + quoted(a));
    QTRY_COMPARE(evaluate("testWindow.tabViewAt(1).url.toString()").toString(), a);
    QTRY_VERIFY(!evaluate("testWindow.tabViewAt(1).loading").toBool());
    QTRY_COMPARE(evaluate("testWindow.browserUi.mixed_key_route('k')").toString(), QString("page"));
    const auto incompleteSource = QString::fromUtf8(R"QML(import QtQuick; Connections {
        property bool parked:true; property bool armed:false; property string result:"";
        property bool urlArmed:false; property string urlResult:"";
        property bool displayArmed:false; property string displayResult:"";
        target:testWindow.browserUi;
        function configurationReentry(){
            const before=target.current_url;
            target.poll_config();
            target.poll_interaction_policy_refresh();
            return JSON.stringify({before:before,url:target.current_url,
                zoom:JSON.parse(target.config_json).content.zoom,route:target.mixed_key_route("k")});
        }
        function onRuntime_work_available(){
            if(parked)testWindow.runtime_runtimeWorkTimer.stop();
            if(urlArmed){urlArmed=false;urlResult=configurationReentry();}
        }
        function onDisplay_urlChanged(){
            if(displayArmed){displayArmed=false;displayResult=configurationReentry();}
        }
        function onInput_epochChanged(){
            if(!armed)return;
            armed=false;
            target.poll_config();
            target.poll_interaction_policy_refresh();
            result=JSON.stringify({index:target.active_tab_index,route:target.mixed_key_route("k"),
                zoom:JSON.parse(target.config_json).content.zoom});
        }
    })QML");
    const QPointer<QObject> incomplete(evaluate("Qt.createQmlObject(" + quoted(incompleteSource) + ",testWindow)").value<QObject *>());
    QVERIFY(incomplete);
    engine->rootContext()->setContextProperty("mixedIncompleteProbe", incomplete.data());
    evaluate("testWindow.runtime_runtimeWorkTimer.stop()");
    const auto restoreRuntime = qScopeGuard([&] {
        if (incomplete) incomplete->setProperty("parked", false);
        evaluate("testWindow.runtime_runtimeWorkTimer.start()");
    });
    const auto targetBefore = evaluate("testWindow.browserUi.navigation_target_for_index(1)").toMap();
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('content.zoom','1.25',true)").toBool());
    // This sentinel is queued after the actual CXX-Qt preparation-ready marker.
    const auto queued = std::make_shared<bool>(false);
    QMetaObject::invokeMethod(window, [queued] { *queued = true; }, Qt::QueuedConnection);
    QTRY_VERIFY(*queued);
    QVERIFY(incomplete->setProperty("armed", true));
    QVERIFY(evaluate("testWindow.browserUi.select_tab(1)").toBool());
    const auto interrupted = QJsonDocument::fromJson(incomplete->property("result").toString().toUtf8()).object();
    QCOMPARE(interrupted.value("zoom").toDouble(), 1.25);
    QCOMPARE(interrupted.value("index").toInt(), 0);
    QCOMPARE(interrupted.value("route").toString(), QString("unavailable"));
    QCOMPARE(evaluate("testWindow.browserUi.navigation_target_for_index(1)").toMap(), targetBefore);
    const auto retryPolicy = QJsonDocument::fromJson(evaluate("testWindow.browserUi.refresh_interaction_status()")
        .toString().toUtf8()).object().value("shortcut_policy").toObject();
    QCOMPARE(retryPolicy.value("unavailable_reason").toString(), QString("selected context preparation is pending"));
    // Same Target/address retry must still prepare after the interrupted work.
    QVERIFY(evaluate("testWindow.browserUi.select_tab(1)").toBool());
    QVERIFY(incomplete->setProperty("parked", false));
    evaluate("testWindow.runtime_runtimeWorkTimer.start()");
    QTRY_COMPARE(evaluate("testWindow.browserUi.mixed_key_route('k')").toString(), QString("page"));
    // The existing cold timer also repairs an interrupted selection without
    // an explicit second selection from the user.
    QVERIFY(incomplete->setProperty("parked", true));
    evaluate("testWindow.runtime_runtimeWorkTimer.stop()");
    const auto automaticTarget = evaluate("testWindow.browserUi.navigation_target_for_index(0)").toMap();
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('content.zoom','1.5',true)").toBool());
    const auto automaticQueued = std::make_shared<bool>(false);
    QMetaObject::invokeMethod(window, [automaticQueued] { *automaticQueued = true; }, Qt::QueuedConnection);
    QTRY_VERIFY(*automaticQueued);
    QVERIFY(incomplete->setProperty("armed", true));
    QVERIFY(evaluate("testWindow.browserUi.select_tab(0)").toBool());
    const auto automatic = QJsonDocument::fromJson(incomplete->property("result").toString().toUtf8()).object();
    QCOMPARE(automatic.value("zoom").toDouble(), 1.5);
    QCOMPARE(automatic.value("index").toInt(), 1);
    QCOMPARE(automatic.value("route").toString(), QString("unavailable"));
    QVERIFY(incomplete->setProperty("parked", false));
    evaluate("testWindow.runtime_runtimeWorkTimer.start()");
    QTRY_COMPARE(evaluate("testWindow.browserUi.active_tab_index").toInt(), 0);
    QTRY_COMPARE(evaluate("testWindow.browserUi.mixed_key_route('k')").toString(), QString("page"));
    QCOMPARE(evaluate("testWindow.browserUi.navigation_target_for_index(0)").toMap(), automaticTarget);
    QVERIFY(incomplete->setProperty("parked", true));
    evaluate("testWindow.runtime_runtimeWorkTimer.stop()");
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('content.zoom','1.75',true)").toBool());
    const auto urlQueued = std::make_shared<bool>(false);
    QMetaObject::invokeMethod(window, [urlQueued] { *urlQueued = true; }, Qt::QueuedConnection);
    QTRY_VERIFY(*urlQueued);
    const auto spaUrl = origin + "/mixed-b/reentry";
    QPointer<QObject> observedView = evaluate("testWindow.activeWebView()").value<QObject *>();
    QVERIFY(observedView);
    const bool oldSignals = observedView->blockSignals(true);
    const auto restoreSignals = qScopeGuard([&] { if (observedView) observedView->blockSignals(oldSignals); });
    QCOMPARE(page("history.pushState({},'', '/mixed-b/reentry');location.pathname").toString(), QString("/mixed-b/reentry"));
    QCOMPARE(evaluate("testWindow.activeWebView().url.toString()").toString(), spaUrl);
    QCOMPARE(text("current_url"), a);
    observedView->blockSignals(oldSignals);
    QVERIFY(incomplete->setProperty("urlArmed", true));
    // Replay the actual view's held URL observation through the production
    // service. No synthetic target, document, URL or eligibility is supplied.
    evaluate("testWindow.browserUi.navigation_url_changed_for(0,testWindow.activeWebView().url.toString())");
    const auto spa = QJsonDocument::fromJson(incomplete->property("urlResult").toString().toUtf8()).object();
    QCOMPARE(spa.value("zoom").toDouble(), 1.75);
    QCOMPARE(spa.value("before").toString(), a);
    QCOMPARE(spa.value("url").toString(), a);
    QCOMPARE(spa.value("route").toString(), QString("unavailable"));
    QCOMPARE(evaluate("testWindow.browserUi.navigation_target_for_index(0)").toMap(), automaticTarget);
    QVERIFY(incomplete->setProperty("parked", false));
    evaluate("testWindow.runtime_runtimeWorkTimer.start()");
    QTRY_COMPARE(text("current_url"), spaUrl);
    QTRY_COMPARE(evaluate("testWindow.browserUi.mixed_key_route('f')").toString(), QString("page"));
    // Reenter an actual provisional observation while the HTTP response is
    // delayed. Cold repair must retain the trusted URL rather than roll back.
    const auto delayed = origin + "/mixed-b/delayed";
    fixtureResponseDelays["/mixed-b/delayed"] = 3000;
    evaluate("testWindow.activeWebView().url=" + quoted(delayed));
    QTRY_VERIFY(evaluate("testWindow.activeWebView().loading").toBool());
    QTRY_COMPARE(text("load_state"), QString("provisional"));
    QTRY_COMPARE(text("current_url"), delayed);
    QVERIFY(incomplete->setProperty("parked", true));
    evaluate("testWindow.runtime_runtimeWorkTimer.stop()");
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('content.zoom','2.0',true)").toBool());
    const auto provisionalQueued = std::make_shared<bool>(false);
    QMetaObject::invokeMethod(window, [provisionalQueued] { *provisionalQueued = true; }, Qt::QueuedConnection);
    QTRY_VERIFY(*provisionalQueued);
    evaluate("testWindow.browserUi.display_url='provisional publication fixture'");
    QVERIFY(incomplete->setProperty("displayArmed", true));
    QVERIFY(evaluate("testWindow.activeWebView().loading").toBool());
    evaluate("testWindow.browserUi.navigation_started_for(0,testWindow.activeWebView().url.toString())");
    const auto provisional = QJsonDocument::fromJson(incomplete->property("displayResult").toString().toUtf8()).object();
    QCOMPARE(provisional.value("zoom").toDouble(), 2.0);
    QCOMPARE(provisional.value("url").toString(), delayed);
    QCOMPARE(provisional.value("route").toString(), QString("unavailable"));
    QVERIFY(incomplete->setProperty("parked", false));
    evaluate("testWindow.browserUi.poll_interaction_policy_refresh()");
    QVERIFY(evaluate("testWindow.activeWebView().loading").toBool());
    QCOMPARE(text("current_url"), delayed);
    QVERIFY(evaluate("testWindow.browserUi.mixed_key_route('k')").toString() != "page");
    evaluate("testWindow.runtime_runtimeWorkTimer.start()");
    QTRY_VERIFY(!evaluate("testWindow.activeWebView().loading").toBool());
    QTRY_COMPARE(evaluate("testWindow.browserUi.mixed_key_route('f')").toString(), QString("page"));
    evaluate("testWindow.activeWebView().url=" + quoted(a));
    QTRY_COMPARE(text("current_url"), a);
    QTRY_COMPARE(evaluate("testWindow.browserUi.mixed_key_route('k')").toString(), QString("page"));
    delete incomplete.data();
    QVERIFY(evaluate("testWindow.browserUi.select_tab(0)").toBool());
    QTRY_COMPARE(evaluate("testWindow.browserUi.mixed_key_route('k')").toString(), QString("page"));
    const auto replacement = evaluate("testWindow.browserUi.tab_id_for_index(1)").toString();
    const auto replacementResult = evaluate(R"JS((function(){
        const ui=testWindow.browserUi;
        const closed=ui.close_tab(0);
        return {closed:closed,index:ui.active_tab_index,tab:ui.tab_id_for_index(0),
                route:ui.mixed_key_route("k")};
    })())JS").toMap();
    QVERIFY(replacementResult.value("closed").toBool());
    QCOMPARE(replacementResult.value("index").toInt(), 0);
    QCOMPARE(replacementResult.value("tab").toString(), replacement);
    QCOMPARE(text("current_url"), a);
    QCOMPARE(replacementResult.value("route").toString(), QString("unavailable"));
    QTRY_COMPARE(evaluate("testWindow.browserUi.mixed_key_route('k')").toString(), QString("page"));
}

void ApplicationTests::committedPeerSelectorRevisionAba() {
    QVERIFY(command("open --target window rb://blank"));
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
    const auto peer = QStringLiteral("testWindow.browserWindowRegistry[1]");
    QTRY_VERIFY(evaluate(peer + ".host.profileBootstrapComplete").toBool());
    QTRY_VERIFY(!evaluate(peer + ".ui.profile_bootstrap_pending").toBool());
    QPointer<QObject> host = evaluate(peer + ".host").value<QObject *>();
    QVERIFY(host);
    const auto closePeer = qScopeGuard([&] {
        if (host) evaluate(peer + ".host.windowShutdownApproved=true;" + peer + ".host.close()");
    });
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('content.zoom','1.25',false)").toBool());
    QTRY_COMPARE(text("setting_action_state"), QString("succeeded"));
    QTRY_COMPARE(QJsonDocument::fromJson(evaluate(peer + ".ui.config_json").toString().toUtf8())
                     .object()["content"].toObject()["zoom"].toDouble(), 1.25);
    auto *validator = host->findChild<QObject *>("selectorValidationSurface");
    QVERIFY(validator);
    QTRY_VERIFY(validator->property("documentReady").toBool());
    QVERIFY(validator->setProperty("documentReady", false));
    const auto oldPeerConfig = evaluate(peer + ".ui.config_json").toString();
    const auto oldStartupNotice = evaluate(peer + ".ui.profile_startup_notice").toString();
    const auto saveSelector = [&](const QString &selector) {
        return evaluate("testWindow.browserUi.set_runtime_setting('hints.selectors.links'," +
                        quoted("{mode='replace',selectors=['" + selector + "']}") + ",false)").toBool();
    };
    QVERIFY(saveSelector("#peer-a"));
    QTRY_COMPARE(text("setting_action_state"), QString("succeeded"));
    QTRY_VERIFY(!evaluate(peer + ".ui.selector_validation_request").toString().isEmpty());
    const auto generation = QJsonDocument::fromJson(
        evaluate(peer + ".ui.selector_validation_request").toString().toUtf8())
                                .object()["generation"].toString();
    QVERIFY(!generation.isEmpty());
    const auto firstReceipt = text("setting_action_id");
    QVERIFY(saveSelector("#peer-b"));
    QVERIFY(text("setting_action_id") != firstReceipt);
    QTRY_COMPARE(text("setting_action_state"), QString("succeeded"));
    const auto secondReceipt = text("setting_action_id");
    QVERIFY(saveSelector("#peer-a"));
    QVERIFY(text("setting_action_id") != secondReceipt);
    QTRY_COMPARE(text("setting_action_state"), QString("succeeded"));
    // Equal restored runtime bytes do not restore the revision captured by
    // the peer's outstanding parser request.
    QFile saved(evaluate("testWindow.storageBasePath").toString() + "/state/runtime-overrides.toml");
    QVERIFY(saved.open(QIODevice::ReadOnly));
    const auto savedBytes = saved.readAll();
#ifdef Q_OS_UNIX
    struct stat beforeAdoption {};
    QCOMPARE(::fstat(saved.handle(), &beforeAdoption), 0);
#endif
    saved.close();
    QVERIFY(savedBytes.contains("#peer-a"));
    QVERIFY(!savedBytes.contains("#peer-b"));
    QVERIFY(evaluate(peer + ".ui.complete_selector_validation(" + quoted(generation) +
                     ",'{\"ok\":true}')").toBool());
    QVERIFY(evaluate(peer + ".ui.status_text").toString().contains("reception postponed"));
    QCOMPARE(evaluate(peer + ".ui.config_json").toString(), oldPeerConfig);
    QCOMPARE(evaluate(peer + ".ui.profile_startup_notice").toString(), oldStartupNotice);
    QTRY_VERIFY(!evaluate(peer + ".ui.selector_validation_request").toString().isEmpty());
    const auto freshGeneration = QJsonDocument::fromJson(
        evaluate(peer + ".ui.selector_validation_request").toString().toUtf8())
                                     .object()["generation"].toString();
    QVERIFY(freshGeneration != generation);
    // The retry uses the real WebEngine parser, then the exact new capture.
    engine->rootContext()->setContextProperty("peerHeldValidator", validator);
    evaluate("peerHeldValidator.documentReady=true;peerHeldValidator.validateRequest(JSON.parse(" +
             peer + ".ui.selector_validation_request))");
    QTRY_VERIFY(evaluate(peer + ".ui.selector_validation_request").toString().isEmpty());
    QTRY_VERIFY(evaluate(peer + ".ui.status_text").toString().contains("Settings synchronized"));
    QCOMPARE(evaluate(peer + ".ui.profile_startup_notice").toString(), oldStartupNotice);
    QVERIFY(saved.open(QIODevice::ReadOnly));
    QCOMPARE(saved.readAll(), savedBytes);
#ifdef Q_OS_UNIX
    struct stat afterAdoption {};
    QCOMPARE(::fstat(saved.handle(), &afterAdoption), 0);
    QCOMPARE(afterAdoption.st_dev, beforeAdoption.st_dev);
    QCOMPARE(afterAdoption.st_ino, beforeAdoption.st_ino);
    QCOMPARE(afterAdoption.st_mtime, beforeAdoption.st_mtime);
#endif
    // Reload-scoped selectors remain pending; synchronization does not
    // silently activate them or rewrite the already committed file.
    QCOMPARE(evaluate(peer + ".ui.config_json").toString(), oldPeerConfig);
}

void ApplicationTests::profileFallbackConfigurationMirrors_data() {
    QTest::addColumn<bool>("cssFailure");
    QTest::newRow("worker-static-fallback") << false;
    QTest::newRow("webengine-css-safe-fallback") << true;
}

void ApplicationTests::profileFallbackConfigurationMirrors() {
    QFETCH(bool, cssFailure);
    evaluate("testWindow.browserUi.config_base_json=" +
             quoted(R"JSON({"schema_version":3,"ui":{"font_size_pt":19}})JSON"));
    const auto rejectedCli = cssFailure
        ? R"JSON({"schema_version":3,"settings":{"hints.selectors.links":{"mode":"replace","selectors":["a["]}}})JSON"
        : R"JSON({"schema_version":3,"settings":{"content.zoom":99}})JSON";
    evaluate("testWindow.browserUi.cli_overrides_json=" + quoted(rejectedCli));
    evaluate("testWindow.browserUi.configure_profile(true,false,'Fallback mirrors','fallback-mirrors',testWindow.storageBasePath)");
    QTRY_VERIFY_WITH_TIMEOUT(!evaluate("testWindow.browserUi.profile_bootstrap_pending").toBool(), 10000);
    QVERIFY(!evaluate("testWindow.browserUi.profile_startup_details()").toString().isEmpty());
    const auto selected = QJsonDocument::fromJson(text("config_json").toUtf8()).object();
    QCOMPARE(selected["ui"].toObject()["font_size_pt"].toDouble(), cssFailure ? 10.0 : 19.0);
    QCOMPARE(selected["content"].toObject()["zoom"].toDouble(), 1.0);
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('content.zoom','1.25',true)").toBool());
    QTRY_COMPARE(text("setting_action_state"), QString("succeeded"));
    QTRY_COMPARE(QJsonDocument::fromJson(text("config_json").toUtf8())
                     .object()["content"].toObject()["zoom"].toDouble(), 1.25);
    QCOMPARE(QJsonDocument::fromJson(text("config_json").toUtf8())
                 .object()["ui"].toObject()["font_size_pt"].toDouble(), cssFailure ? 10.0 : 19.0);
}

void ApplicationTests::sharedPolicyPreparingOwner_data() {
    QTest::addColumn<bool>("closePreparingOwner");
    QTest::newRow("complete-switch") << false;
    QTest::newRow("retained-durable-close") << true;
}

void ApplicationTests::sharedPolicyPreparingOwner() {
    QFETCH(bool, closePreparingOwner);
    QVERIFY(command("open --target window rb://blank"));
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
    const auto peer = QStringLiteral("testWindow.browserWindowRegistry[1]");
    QTRY_VERIFY(evaluate(peer + ".host.profileBootstrapComplete").toBool());
    QTRY_VERIFY(!evaluate(peer + ".ui.profile_bootstrap_pending").toBool());
    QPointer<QObject> host = evaluate(peer + ".host").value<QObject *>();
    QVERIFY(host);
    QPointer<BrowserUi> peerUi = qobject_cast<BrowserUi *>(evaluate(peer + ".ui").value<QObject *>());
    QVERIFY(peerUi);
    engine->rootContext()->setContextProperty("preparingPeerHost", host.data());
    engine->rootContext()->setContextProperty("preparingPeerUi", peerUi.data());
    const auto closePeer = qScopeGuard([&] {
        if (host && host->property("visible").toBool())
            evaluate("preparingPeerHost.windowShutdownApproved=true;preparingPeerHost.close()");
    });
    auto *validator = host->findChild<QObject *>("selectorValidationSurface");
    QVERIFY(validator);
    QTRY_VERIFY(validator->property("documentReady").toBool());
    QVERIFY(validator->setProperty("documentReady", false));
    const auto fixture = QString::fromUtf8(R"JSON({"schema_version":3,"hints":{"selectors":{"links":{"mode":"replace","selectors":["#preparing-peer"]}}}})JSON");
    evaluate(peer + ".ui.config_base_json=" + quoted(fixture));
    evaluate(peer + ".ui.configure_profile(true,false,'Preparing peer','preparing-peer',testWindow.storageBasePath)");
    QTRY_VERIFY(!evaluate(peer + ".ui.selector_validation_request").toString().isEmpty());
    QVERIFY(evaluate(peer + ".ui.profile_bootstrap_pending").toBool());
    const auto before = text("config_json");
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('content.zoom','1.25',false)").toBool());
    QTRY_COMPARE(text("setting_action_state"), QString("failed"));
    QVERIFY(text("setting_action_detail").contains("sharing profile", Qt::CaseInsensitive));
    QCOMPARE(text("config_json"), before);
    const auto generation = QJsonDocument::fromJson(peerUi->property("selector_validation_request").toString().toUtf8())
                                .object()["generation"].toString();
    QVERIFY(!generation.isEmpty());
    if (closePreparingOwner) {
        evaluate("preparingPeerHost.windowShutdownApproved=true;preparingPeerHost.close()");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 1);
        QTRY_VERIFY(host && host->property("runtimeReleaseComplete").toBool());
        QVERIFY(peerUi);
        QVERIFY(!peerUi->property("view_alive").toBool());
        QVERIFY(!peerUi->property("core_window_id").toString().isEmpty());
        QVERIFY(!evaluate("preparingPeerUi.release_transient_resources()").toBool());
        QVERIFY(!peerUi->complete_selector_validation(generation, QString("{\"ok\":true}")));
    } else {
        engine->rootContext()->setContextProperty("preparingPeerValidator", validator);
        evaluate("preparingPeerValidator.documentReady=true;preparingPeerValidator.validateRequest(JSON.parse(" + peer + ".ui.selector_validation_request))");
        QTRY_VERIFY_WITH_TIMEOUT(!evaluate(peer + ".ui.profile_bootstrap_pending").toBool(), 10000);
    }
    // Completing the switch or actually retiring the outgoing view must both
    // remove its preparing registration before this root can publish again.
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('content.zoom','1.25',false)").toBool());
    QTRY_COMPARE(text("setting_action_state"), QString("succeeded"));
    QTRY_COMPARE(QJsonDocument::fromJson(text("config_json").toUtf8())
                     .object()["content"].toObject()["zoom"].toDouble(), 1.25);
    if (closePreparingOwner) {
        QVERIFY(peerUi);
        QVERIFY(!peerUi->complete_selector_validation(generation, QString("{\"ok\":true}")));
        QVERIFY(!peerUi->property("view_alive").toBool());
    }
}

void ApplicationTests::sharedPolicyPrivateErrorRecovery() {
    QVERIFY(command("open --target window rb://blank"));
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
    const auto peer = QStringLiteral("testWindow.browserWindowRegistry[1]");
    QTRY_VERIFY(evaluate(peer + ".host.profileBootstrapComplete").toBool());
    QTRY_VERIFY(!evaluate(peer + ".ui.profile_bootstrap_pending").toBool());
    QPointer<QObject> host = evaluate(peer + ".host").value<QObject *>();
    QVERIFY(host);
    const auto closePeer = qScopeGuard([&] {
        if (host) evaluate(peer + ".host.windowShutdownApproved=true;" + peer + ".host.close()");
    });
    // Private owners still watch the read-only userscript catalogue. Park the
    // actual secondary runtime timer through public QML object identity, so
    // recovery and peer reception cannot pass via its watch polling fallback.
    QTRY_COMPARE_WITH_TIMEOUT(evaluate(peer + ".ui.maintenance_delay_ms()").toInt(), 1000, 10000);
    QObject *runtimeTimer = nullptr;
    for (auto *candidate : host->findChildren<QObject *>()) {
        auto *context = QQmlEngine::contextForObject(candidate);
        if (context && context->nameForObject(candidate) == "secondaryRuntimeWorkTimer") {
            runtimeTimer = candidate;
            break;
        }
    }
    QVERIFY(runtimeTimer);
    QVERIFY(runtimeTimer->setProperty("running", false));
    const auto result = evaluate("(function(){var peer=" + peer + ";"
        "var accepted=peer.ui.execute_command('tab-select 999');"
        "return {accepted:accepted,delay:peer.ui.maintenance_delay_ms()};})()").toMap();
    QVERIFY(!result.value("accepted").toBool());
    QCOMPARE(result.value("delay").toInt(), 50);
    QTRY_COMPARE_WITH_TIMEOUT(evaluate(peer + ".ui.maintenance_delay_ms()").toInt(), 1000, 10000);
    QVERIFY(runtimeTimer->setProperty("running", false));
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('content.zoom','1.25',false)").toBool());
    QTRY_COMPARE(text("setting_action_state"), QString("succeeded"));
    // Receiving this memory-only publication must wake the otherwise idle peer.
    QTRY_COMPARE_WITH_TIMEOUT(QJsonDocument::fromJson(evaluate(peer + ".ui.config_json").toString().toUtf8())
                     .object()["content"].toObject()["zoom"].toDouble(), 1.25, 5000);
}

void ApplicationTests::savedRuntimeSelectorRepair_data() {
    QTest::addColumn<bool>("unset");
    QTest::newRow("typed-set") << false;
    QTest::newRow("typed-unset") << true;
}

void ApplicationTests::savedRuntimeSelectorRepair() {
    QFETCH(bool, unset);
    QTemporaryDir savedRoot;
    QVERIFY(savedRoot.isValid());
    QVERIFY(QDir().mkpath(savedRoot.filePath("state")));
    QFile saved(savedRoot.filePath("state/runtime-overrides.toml"));
    QVERIFY(saved.open(QIODevice::WriteOnly));
    const QByteArray original = R"TOML(schema_version=3
[settings]
'content.zoom'=1.25
'hints.selectors.links'={mode='replace',selectors=['a[']}
[bindings.normal]
x='scroll down'
[[site_rules]]
id='preserved-first'
pattern='https://elsewhere.test/*'
priority=7
[site_rules.set]
'content.images'=false
[site_rules.interaction]
enabled=false
hint_page_keys=['p']
[[site_rules]]
id='preserved-second'
pattern='https://second.test/*'
priority=-4
[site_rules.interaction]
enabled=false
)TOML";
    QCOMPARE(saved.write(original), original.size());
    saved.close();
    evaluate("testWindow.browserUi.configure_profile(false,false,'Saved repair','default'," + quoted(savedRoot.path()) + ")");
    QTRY_VERIFY_WITH_TIMEOUT(!evaluate("testWindow.browserUi.profile_bootstrap_pending").toBool(), 10000);
    QTRY_VERIFY2(evaluate("testWindow.browserUi.profile_startup_details()").toString().contains("Selector validation failed"),
                 qPrintable(evaluate("testWindow.browserUi.profile_startup_details()").toString()));
    QVERIFY(evaluate("testWindow.browserUi.profile_startup_details()").toString().contains("built-in safe configuration"));
    const auto before = text("config_json");
    const auto safe = QJsonDocument::fromJson(before.toUtf8()).object();
    QCOMPARE(safe.value("content").toObject().value("zoom").toDouble(), 1.0);
    QVERIFY(safe.value("site_rules").toArray().isEmpty());
    const auto readSaved = [&] {
        QFile file(saved.fileName());
        if (!file.open(QIODevice::ReadOnly)) return QByteArray();
        return file.readAll();
    };
    QCOMPARE(readSaved(), original);
    // A temporary edit cannot replace retained saved data with this local
    // fallback graph, nor repair a saved selector by silently discarding it.
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('content.zoom','1.5',true)").toBool());
    QTRY_COMPARE(text("setting_action_state"), QString("failed"));
    QCOMPARE(text("config_json"), before);
    QCOMPARE(readSaved(), original);
    const auto repair = unset
        ? QString("testWindow.browserUi.unset_runtime_setting('hints.selectors.links',false)")
        : QString("testWindow.browserUi.set_runtime_setting('hints.selectors.links',\"{mode='replace',selectors=['#repaired']}\",false)");
    QVERIFY(evaluate(repair).toBool());
    QTRY_VERIFY2_WITH_TIMEOUT(text("setting_action_state") == "succeeded",
                             qPrintable(text("setting_action_state") + ": " + text("setting_action_detail")), 10000);
    const auto repaired = readSaved();
    QVERIFY(!repaired.isEmpty());
    QVERIFY(!repaired.contains("a["));
    QVERIFY(repaired.contains("scroll down"));
    QVERIFY(repaired.contains("content.images"));
    QVERIFY(repaired.contains("hint_page_keys"));
    QVERIFY(repaired.indexOf("preserved-first") < repaired.indexOf("preserved-second"));
    QTRY_COMPARE(QJsonDocument::fromJson(text("config_json").toUtf8()).object()
                     .value("content").toObject().value("zoom").toDouble(), 1.25);
    // Reload-scoped selectors and the rule catalogue remain pending even
    // after actual CSS success and durable persistence.
    const auto active = QJsonDocument::fromJson(text("config_json").toUtf8()).object();
    QVERIFY(active.value("hints").toObject().value("selectors").toObject().isEmpty());
    QVERIFY(active.value("site_rules").toArray().isEmpty());
#ifdef Q_OS_UNIX
    struct stat beforeReload {};
    QCOMPARE(::stat(QFile::encodeName(saved.fileName()).constData(), &beforeReload), 0);
#endif
    QFile config(savedRoot.filePath("config.toml"));
    QVERIFY(config.open(QIODevice::WriteOnly));
    QCOMPARE(config.write("schema_version=3\n"), 17);
    config.close();
    evaluate("testWindow.browserUi.config_path=" + quoted(config.fileName()));
    QVERIFY(command("config-reload"));
    QTRY_COMPARE_WITH_TIMEOUT(QJsonDocument::fromJson(text("config_json").toUtf8()).object()
                                 .value("site_rules").toArray().size(), 2, 10000);
    const auto reloaded = QJsonDocument::fromJson(text("config_json").toUtf8()).object();
    const auto rules = reloaded.value("site_rules").toArray();
    QCOMPARE(rules.at(0).toObject().value("id").toString(), QString("preserved-first"));
    QCOMPARE(rules.at(0).toObject().value("set").toObject().value("content.images").toBool(), false);
    QCOMPARE(rules.at(0).toObject().value("interaction").toObject().value("hint_page_keys").toArray(), QJsonArray{"p"});
    QCOMPARE(rules.at(1).toObject().value("id").toString(), QString("preserved-second"));
    const auto selectors = reloaded.value("hints").toObject().value("selectors").toObject();
    if (unset) QVERIFY(!selectors.contains("links"));
    else QCOMPARE(selectors.value("links").toObject().value("selectors").toArray(), QJsonArray{"#repaired"});
    QCOMPARE(readSaved(), repaired);
#ifdef Q_OS_UNIX
    struct stat afterReload {};
    QCOMPARE(::stat(QFile::encodeName(saved.fileName()).constData(), &afterReload), 0);
    QCOMPARE(afterReload.st_dev, beforeReload.st_dev);
    QCOMPARE(afterReload.st_ino, beforeReload.st_ino);
    QCOMPARE(afterReload.st_mtime, beforeReload.st_mtime);
#endif
}

void ApplicationTests::runtimeEditsBeforeProfile() {
    // A real production BrowserUi before configure_profile has local memory
    // configuration and no saved-source owner. It must retain that behavior.
    BrowserUi owner;
    QVERIFY(owner.set_runtime_setting(QString("content.zoom"), QString("1.5"), false));
    const auto drain = [&] {
        owner.poll_config();
        return owner.property("setting_action_state").toString();
    };
    QTRY_COMPARE(drain(), QString("succeeded"));
    QCOMPARE(QJsonDocument::fromJson(owner.property("config_json").toString().toUtf8()).object()
                 .value("content").toObject().value("zoom").toDouble(), 1.5);
    QVERIFY(owner.execute_command(QString("bind z scroll down")));
    const auto binding = [&] {
        owner.poll_config();
        return QJsonDocument::fromJson(owner.property("config_json").toString().toUtf8()).object()
            .value("bindings").toObject().value("normal").toObject().value("z").toString();
    };
    QTRY_COMPARE(binding(), QString("scroll down"));
    QCOMPARE(QJsonDocument::fromJson(owner.property("config_json").toString().toUtf8()).object()
                 .value("content").toObject().value("zoom").toDouble(), 1.5);
}

void ApplicationTests::transientRuntimeEditsUseMemoryOnly() {
    // This ordinary fixture opens a transient profile. A persistent-scope
    // command edits its initialized shared graph without requiring durability.
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('content.zoom','1.5',false)").toBool());
    QTRY_COMPARE(text("setting_action_state"), QString("succeeded"));
    QVERIFY(text("status_text").contains("memory-only"));
    QVERIFY(command("bind z scroll down"));
    const auto config = QJsonDocument::fromJson(text("config_json").toUtf8()).object();
    QCOMPARE(config.value("content").toObject().value("zoom").toDouble(), 1.5);
    QCOMPARE(config.value("bindings").toObject().value("normal").toObject().value("z").toString(), QString("scroll down"));
    QDirIterator overrides(evaluate("testWindow.storageBasePath").toString(), {"runtime-overrides.toml"}, QDir::Files, QDirIterator::Subdirectories);
    QVERIFY(!overrides.hasNext());
}

void ApplicationTests::durablePersistenceSelectorPreparationAba() {
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('content.zoom','1.25',false)").toBool());
    QTRY_COMPARE(text("setting_action_state"), QString("succeeded"));
    const auto path = evaluate("testWindow.storageBasePath").toString() + "/state/runtime-overrides.toml";
    QFile initialSaved(path);
    QVERIFY(initialSaved.open(QIODevice::ReadOnly));
    const auto initialBytes = initialSaved.readAll();
    initialSaved.close();
    QVERIFY(command("open --target window rb://blank"));
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
    const auto peer = QStringLiteral("testWindow.browserWindowRegistry[1]");
    QTRY_VERIFY(evaluate(peer + ".host.profileBootstrapComplete && !" + peer + ".ui.profile_bootstrap_pending").toBool());
    QPointer<QObject> host = evaluate(peer + ".host").value<QObject *>();
    QVERIFY(host);
    const auto closePeer = qScopeGuard([&] {
        if (host) evaluate(peer + ".host.windowShutdownApproved=true;" + peer + ".host.close()");
    });
    auto *validator = window->findChild<QObject *>("selectorValidationSurface");
    QVERIFY(validator);
    QTRY_VERIFY(validator->property("documentReady").toBool());
    QVERIFY(validator->setProperty("documentReady", false));
    const auto before = text("config_json");
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('hints.selectors.links',\"{mode='replace',selectors=['#held-fresh-candidate']}\",false)").toBool());
    QTRY_VERIFY(!text("selector_validation_request").isEmpty());
    const auto generation = QJsonDocument::fromJson(text("selector_validation_request").toUtf8())
                                .object().value("generation").toString();
    QVERIFY(!generation.isEmpty());
    QCOMPARE(text("setting_action_state"), QString("pending"));
    // Real peer writes restore equal Runtime values but advance the sharing
    // revision twice while the original CSS callback is still outstanding.
    for (const auto &zoom : {QString("1.5"), QString("1.25")}) {
        QVERIFY(evaluate(peer + ".ui.set_runtime_setting('content.zoom'," + quoted(zoom) + ",false)").toBool());
        QTRY_COMPARE(evaluate(peer + ".ui.setting_action_state").toString(), QString("succeeded"));
        QCOMPARE(QJsonDocument::fromJson(evaluate(peer + ".ui.config_json").toString().toUtf8()).object()
                     .value("content").toObject().value("zoom").toDouble(), zoom.toDouble());
    }
    QCOMPARE(text("config_json"), before);
    QCOMPARE(QJsonDocument::fromJson(text("selector_validation_request").toUtf8())
                 .object().value("generation").toString(), generation);
    QFile saved(path);
    QVERIFY(saved.open(QIODevice::ReadOnly));
    const auto restored = saved.readAll();
    QCOMPARE(restored, initialBytes);
    QVERIFY(!restored.contains("held-fresh-candidate"));
#ifdef Q_OS_UNIX
    struct stat beforeCallback {};
    QCOMPARE(::fstat(saved.handle(), &beforeCallback), 0);
#endif
    saved.close();
    // Resume the actual WebEngine syntax parser, not a forged success payload.
    QCOMPARE(text("setting_action_state"), QString("pending"));
    QCOMPARE(QJsonDocument::fromJson(text("selector_validation_request").toUtf8())
                 .object().value("generation").toString(), generation);
    engine->rootContext()->setContextProperty("heldRuntimeValidator", validator);
    evaluate("heldRuntimeValidator.documentReady=true;heldRuntimeValidator.validateRequest(JSON.parse(testWindow.browserUi.selector_validation_request))");
    QTRY_COMPARE(text("setting_action_state"), QString("failed"));
    QVERIFY2(text("setting_action_detail").contains("prepared configuration became stale"),
             qPrintable(text("setting_action_detail")));
    QCOMPARE(text("config_json"), before);
    QVERIFY(saved.open(QIODevice::ReadOnly));
    QCOMPARE(saved.readAll(), restored);
#ifdef Q_OS_UNIX
    struct stat afterCallback {};
    QCOMPARE(::fstat(saved.handle(), &afterCallback), 0);
    QCOMPARE(afterCallback.st_dev, beforeCallback.st_dev);
    QCOMPARE(afterCallback.st_ino, beforeCallback.st_ino);
    QCOMPARE(afterCallback.st_mtime, beforeCallback.st_mtime);
#endif
}
