#include "tst_application.h"
#include <QQuickItem>
#include <QQmlComponent>
#include <QQmlContext>
#include <QDirIterator>
#include <QFile>
#include <QJsonDocument>
#include <QJsonObject>
#include <sqlite3.h>
#include <QUrl>
#include <QWebEnginePermission>
#include <QtTest>

void ApplicationTests::productPolishPermissionRevocationScope_data() {
    QTest::addColumn<QString>("kind");
    for (const auto &kind : {QString("unrelated"), QString("other-root"),
                             QString("private-isolation"), QString("popup"), QString("background")})
        QTest::newRow(qPrintable(kind)) << kind;
}
void ApplicationTests::productPolishPermissionRevocationScope() {
    QFETCH(QString, kind);
    const auto url = text("current_url");
    const auto origin =
        QUrl(url).adjusted(QUrl::RemovePath | QUrl::RemoveQuery | QUrl::RemoveFragment).toString();
    if (kind == "private-isolation") {
        for (int i = 0; i < 2; ++i) {
            evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:" +
                     quoted(url) +
                     ",windowProfileName:'separate-private',windowProfileLabel:'Separate "
                     "private',windowPrivateProfile:true})");
            QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), i + 2);
        }
        for (int i = 1; i < 3; ++i)
            QTRY_VERIFY_WITH_TIMEOUT(
                evaluate(
                    QString("testWindow.browserWindowRegistry[%1].host.profileBootstrapComplete && "
                            "!testWindow.browserWindowRegistry[%1].ui.profile_bootstrap_pending && "
                            "!testWindow.browserWindowRegistry[%1].view.loading")
                        .arg(i))
                    .toBool(),
                15000);
        QVERIFY(!evaluate("testWindow.sharesPermissionSession(testWindow.browserWindowRegistry[1]."
                          "ui,testWindow.browserWindowRegistry[2].ui)")
                     .toBool());
        QCOMPARE(evaluate("testWindow.reloadViewsForPermission(" + quoted(origin) +
                          ",testWindow.browserWindowRegistry[1].ui)")
                     .toInt(),
                 1);
    } else if (kind == "unrelated" || kind == "other-root") {
        const auto options = kind == "other-root"
                                 ? ",windowProfileName:testWindow.profileName,"
                                   "windowStorageBasePath:testWindow.storageBasePath+'/other-root'"
                                 : ",windowProfileName:'unrelated-reset'";
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:" +
                 quoted(url) + ",windowProfileLabel:'Unrelated reset'" + options + "})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
        QTRY_VERIFY_WITH_TIMEOUT(
            evaluate("testWindow.browserWindowRegistry[1].host.profileBootstrapComplete && "
                     "!testWindow.browserWindowRegistry[1].ui.profile_bootstrap_pending && "
                     "!testWindow.browserWindowRegistry[1].view.loading")
                .toBool(),
            15000);
        QCOMPARE(evaluate("testWindow.reloadViewsForPermission(" + quoted(origin) +
                          ",testWindow.browserUi)")
                     .toInt(),
                 1);
    } else if (kind == "popup") {
        QVERIFY(page("document.readyState==='complete' && !!document.getElementById('blank')")
                    .toBool());
        auto view = qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());
        QVERIFY(view);
        window->requestActivate();
        QTRY_VERIFY(window->isActive());
        const auto point =
            page("(()=>{const r=document.getElementById('blank').getBoundingClientRect();return "
                 "[r.x+r.width/2,r.y+r.height/2]})()")
                .toList();
        QTest::mouseClick(
            window, Qt::LeftButton, Qt::NoModifier,
            view->mapToScene(QPointF(point[0].toDouble(), point[1].toDouble())).toPoint());
        QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(), 1);
        QTRY_VERIFY_WITH_TIMEOUT(
            evaluate("testWindow.popupWindowRegistry[0].host.profileBootstrapComplete && "
                     "!testWindow.popupWindowRegistry[0].ui.profile_bootstrap_pending && "
                     "!testWindow.popupWindowRegistry[0].view.loading")
                .toBool(),
            15000);
        QCOMPARE(evaluate("testWindow.reloadViewsForPermission(" + quoted(origin) +
                          ",testWindow.browserUi)")
                     .toInt(),
                 2);
    } else {
        const auto desired = url + "?background-reset=1";
        interactiveCommand("tab-open " + desired);
        QTRY_COMPARE(evaluate("testWindow.browserUi.tab_count").toInt(), 2);
        QTRY_VERIFY_WITH_TIMEOUT(
            evaluate(
                "!!testWindow.activeWebView() && testWindow.activeWebView().url.toString()===" +
                quoted(desired) +
                " && !testWindow.activeWebView().loading && "
                "testWindow.browserUi.load_state==='complete'")
                .toBool(),
            15000);
        QCOMPARE(evaluate("testWindow.reloadViewsForPermission(" + quoted(origin) +
                          ",testWindow.browserUi)")
                     .toInt(),
                 2);
    }
}

void ApplicationTests::productPolishPermissionResetOwner_data() {
    QTest::addColumn<QString>("kind");
    QTest::addColumn<bool>("remembered");
    for (const auto &kind :
         {QString("primary"), QString("secondary"), QString("private"), QString("popup")})
        QTest::newRow(qPrintable(kind)) << kind << true;
    QTest::newRow("private-once-only") << QString("private") << false;
}
void ApplicationTests::productPolishPermissionResetOwner() {
    QFETCH(bool, remembered);
    QFETCH(QString, kind);
    QString owner = "testWindow.browserWindowRegistry[0]";
    if (kind != "primary") {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:" +
                 quoted(text("current_url")) +
                 ",windowProfileName:'link-owner',windowProfileLabel:'Link "
                 "owner',windowPrivateProfile:" +
                 (kind == "private" ? "true" : "false") + "})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
        owner = "testWindow.browserWindowRegistry[1]";
        QTRY_VERIFY(evaluate(owner + ".host.profileBootstrapComplete").toBool());
    }
    if (kind == "popup") {
        pageReceiver = owner + ".view";
        QTRY_VERIFY_WITH_TIMEOUT(evaluate(owner + ".ui.load_state==='complete' && !" + owner +
                                          ".view.loading && !" + owner +
                                          ".ui.profile_bootstrap_pending")
                                     .toBool(),
                                 15000);
        QVERIFY(page("document.readyState==='complete' && !!document.getElementById('blank')")
                    .toBool());
        auto host = qobject_cast<QQuickWindow *>(evaluate(owner + ".host").value<QObject *>());
        auto view = qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());
        QVERIFY(host && view);
        host->requestActivate();
        QTRY_VERIFY(host->isActive());
        const auto point =
            page("(()=>{const r=document.getElementById('blank').getBoundingClientRect();return "
                 "[r.x+r.width/2,r.y+r.height/2]})()")
                .toList();
        QTest::mouseClick(
            host, Qt::LeftButton, Qt::NoModifier,
            view->mapToScene(QPointF(point[0].toDouble(), point[1].toDouble())).toPoint());
        QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(), 1);
        owner = "testWindow.popupWindowRegistry[0]";
    }
    auto host = qobject_cast<QQuickWindow *>(evaluate(owner + ".host").value<QObject *>());
    QVERIFY(host);
    QTRY_VERIFY(evaluate(owner + ".host.profileBootstrapComplete && !" + owner +
                         ".ui.profile_bootstrap_pending && " + owner + ".view.ownsTab()")
                    .toBool());
    QTRY_VERIFY(evaluate(owner + ".host.activeWebView() === " + owner + ".view").toBool());
    const auto submit = [&](const QString &value) {
        host->requestActivate();
        QTRY_VERIFY(host->isActive());
        evaluate(owner + ".ui.enter_command()");
        auto input = host->findChild<QQuickItem *>("commandLineInput");
        QVERIFY(input);
        QVERIFY(input->setProperty("text", value));
        input->forceActiveFocus();
        QTRY_VERIFY(input->hasActiveFocus());
        QTest::keyClick(host, Qt::Key_Return);
    };

    pageReceiver = owner + ".view";
    QTRY_VERIFY(
        page("document.readyState==='complete' && !!document.getElementById('button')").toBool());
    const auto origin =
        evaluate("testWindow.permissionOriginForView(" + owner + ".view)").toString();
    QVERIFY(origin.startsWith("http://127.0.0.1:"));
    if (remembered)
        QVERIFY(evaluate(owner + ".ui.remember_permission(" + quoted(origin) +
                         ",'camera','allow','session')")
                    .toBool());
    QVERIFY(page("window.permissionResetProbe=9;true").toBool());
    submit("permission-reset " + origin + " camera");
    QTRY_COMPARE(page("window.permissionResetProbe || 0").toInt(), 0);
    QTRY_VERIFY(page("document.readyState==='complete'").toBool());
    QCOMPARE(
        evaluate(owner + ".ui.permission_decision(" + quoted(origin) + ",'camera')").toString(),
        QString("ask"));
    if (kind != "primary")
        QVERIFY(!text("status_text").startsWith("Permission reset; reloaded"));
}

void ApplicationTests::productPolishPermissionResetQueue() {
    const auto url = text("current_url");
    const auto origin =
        QUrl(url).adjusted(QUrl::RemovePath | QUrl::RemoveQuery | QUrl::RemoveFragment).toString();
    evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:" + quoted(url) +
             ",windowProfileName:'queue-scope',windowProfileLabel:'Queue scope'})");
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
    const auto child = QString("testWindow.browserWindowRegistry[1]");
    QTRY_VERIFY(evaluate(child + ".host.profileBootstrapComplete && !" + child +
                         ".ui.profile_bootstrap_pending")
                    .toBool());
    const auto request = QString("navigator.geolocation.getCurrentPosition(()=>window.geoResult='"
                                 "allowed',()=>window.geoResult='denied');true");
    QVERIFY(page(request).toBool());
    QTRY_VERIFY(evaluate("testWindow.permissionPromptVisible").toBool());
    pageReceiver = child + ".view";
    QTRY_VERIFY(
        page("document.readyState==='complete' && !!document.getElementById('button')").toBool());
    QVERIFY(page(request).toBool());
    QTRY_COMPARE(evaluate("testWindow.permissionPromptQueue.length").toInt(), 2);
    QCOMPARE(
        evaluate("testWindow.reloadViewsForPermission(" + quoted(origin) + ",testWindow.browserUi)")
            .toInt(),
        1);
    QTRY_COMPARE(evaluate("testWindow.permissionPromptQueue.length").toInt(), 1);
    QTRY_VERIFY(evaluate("testWindow.pendingPermissionUi===" + child + ".ui && testWindow.permissionPromptVisible").toBool());
    QVERIFY(evaluate("testWindow.pendingPermissionHost===" + child + ".host").toBool());
    evaluate("testWindow.decidePermission(false,'')");
    QTRY_COMPARE(page("window.geoResult").toString(), QString("denied"));
}

void ApplicationTests::productPolishNotificationResetScope() {
    const auto url = text("current_url");
    const auto origin =
        QUrl(url).adjusted(QUrl::RemovePath | QUrl::RemoveQuery | QUrl::RemoveFragment).toString();
    evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:" + quoted(url) +
             ",windowProfileName:'notification-scope',windowProfileLabel:'Notification scope'})");
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
    QTRY_VERIFY(evaluate("testWindow.browserWindowRegistry[1].host.profileBootstrapComplete && "
                         "!testWindow.browserWindowRegistry[1].ui.profile_bootstrap_pending")
                    .toBool());
    QQmlComponent notificationFixture(engine.get());
    notificationFixture.setData(R"QML(import QtQml
        QtObject {
            property QtObject first: QtObject {
                property string origin: ""
                property bool wasClosed: false
                signal closed()
                function close() { wasClosed = true }
            }
            property QtObject second: QtObject {
                property string origin: ""
                property bool wasClosed: false
                signal closed()
                function close() { wasClosed = true }
            }
        })QML", QUrl());
    std::unique_ptr<QObject> notifications(notificationFixture.create());
    QVERIFY2(notifications, qPrintable(notificationFixture.errorString()));
    engine->rootContext()->setContextProperty("notificationScopeProbe", notifications.get());
    evaluate(QString(R"JS((() => {
        const a = notificationScopeProbe.first, b = notificationScopeProbe.second;
        a.origin = %1; b.origin = %1;
        const separator = String.fromCharCode(10);
        testWindow.rememberNotificationOwner(a, testWindow.browserUi);
        testWindow.rememberNotificationOwner(b, testWindow.browserWindowRegistry[1].ui);
        const active = {};
        active['one' + separator + a.origin + separator + '1'] = a;
        active['two' + separator + b.origin + separator + '1'] = b;
        testWindow.activeWebNotifications = active;
        testWindow.pendingPortalNotifications = [
            {ui: testWindow.browserUi, notification: a},
            {ui: testWindow.browserWindowRegistry[1].ui, notification: b}];
        return true;
    })())JS")
                 .arg(quoted(origin)));

    evaluate("testWindow.closeWebNotificationsForOrigin(" + quoted(origin) +
             ",testWindow.browserUi)");
    QCOMPARE(evaluate("Object.keys(testWindow.activeWebNotifications).length").toInt(), 1);
    QVERIFY(evaluate("!testWindow.activeWebNotifications[Object.keys(testWindow."
                     "activeWebNotifications)[0]].wasClosed")
                .toBool());
    QCOMPARE(evaluate("testWindow.pendingPortalNotifications.length").toInt(), 1);
    QVERIFY(
        evaluate(
            "testWindow.pendingPortalNotifications[0].ui===testWindow.browserWindowRegistry[1].ui")
            .toBool());
    evaluate("testWindow.closeWebNotifications()");
}

void ApplicationTests::productPolishPermissionReadiness_data() {
    QTest::addColumn<QString>("kind");
    QTest::newRow("unrelated-write") << QString("unrelated-write");
    QTest::newRow("shared-pending-deny") << QString("shared-pending-deny");
}
void ApplicationTests::productPolishPermissionReadiness() {
    QFETCH(QString,kind);
    const QString lookup="testWindow.browserUi.permission_decision('https://blocked.example.test','geolocation')";
    QTRY_COMPARE(evaluate("JSON.parse(testWindow.browserUi.config_json).permissions.geolocation").toString(),QString("allow"));
    if (kind=="unrelated-write") {
        QVERIFY(evaluate("testWindow.browserUi.remember_permission('https://blocked.example.test','geolocation','deny','site')").toBool());
        QTRY_COMPARE_WITH_TIMEOUT(evaluate(lookup).toString(),QString("deny"),15000);
    }
    // A second origin with no rule proves that the full preference snapshot is ready.
    QTRY_COMPARE_WITH_TIMEOUT(evaluate("testWindow.browserUi.permission_decision('https://unused.example.test','geolocation')").toString(),QString("allow"),15000);
    if (kind=="shared-pending-deny") {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:'rb://blank',windowSharedProfile:testWindow.browserProfile,windowInheritedUi:testWindow.browserUi,windowProfileName:testWindow.profileName})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.browserWindowRegistry[1].host.profileBootstrapComplete && !testWindow.browserWindowRegistry[1].ui.profile_bootstrap_pending").toBool(),15000);
        QTRY_COMPARE_WITH_TIMEOUT(evaluate("testWindow.browserWindowRegistry[1].ui.permission_decision('https://blocked.example.test','geolocation')").toString(),QString("allow"),15000);
    }
    QDirIterator files(evaluate("testWindow.storageBasePath").toString(), {"browser.sqlite"},QDir::Files,QDirIterator::Subdirectories);
    QVERIFY(files.hasNext());const auto path=QFile::encodeName(files.next());
    struct WriteLock {
        sqlite3 *database=nullptr;
        ~WriteLock(){if(database){sqlite3_exec(database,"ROLLBACK",nullptr,nullptr,nullptr);sqlite3_close(database);}}
    } held;
    QCOMPARE(sqlite3_open_v2(path.constData(),&held.database,SQLITE_OPEN_READWRITE,nullptr),SQLITE_OK);
    QCOMPARE(sqlite3_exec(held.database,"BEGIN IMMEDIATE",nullptr,nullptr,nullptr),SQLITE_OK);
    const auto mutation=kind=="unrelated-write"
        ? "testWindow.browserUi.remember_permission('https://other.example.test','notifications','deny','site')"
        : "testWindow.browserUi.remember_permission('https://blocked.example.test','geolocation','deny','site')";
    QVERIFY(evaluate(mutation).toBool());
    QCOMPARE(evaluate(lookup).toString(),QString("deny"));
    if (kind=="shared-pending-deny")
        QCOMPARE(evaluate("testWindow.browserWindowRegistry[1].ui.permission_decision('https://blocked.example.test','geolocation')").toString(),QString("deny"));
    QCOMPARE(sqlite3_exec(held.database,"ROLLBACK",nullptr,nullptr,nullptr),SQLITE_OK);
    sqlite3_close(held.database);held.database=nullptr;
    if (kind=="shared-pending-deny") {
        evaluate("(function(){const host=testWindow.browserWindowRegistry[1].host;host.windowShutdownApproved=true;host.close();host.destroy()})()");
        QCoreApplication::sendPostedEvents(nullptr,QEvent::DeferredDelete);
    }
}


void ApplicationTests::productPolishPendingPermissionPolicy_data() {
    QTest::addColumn<QString>("outcome");
    for (const auto &outcome : {"allow", "deny", "ask", "timeout", "navigation", "transfer",
                                "tab-switch", "retired", "closed-host", "invalid-request"})
        QTest::newRow(outcome) << QString(outcome);
}

void ApplicationTests::productPolishPendingPermissionPolicy() {
    QFETCH(QString, outcome);
    QQmlComponent holder(engine.get());
    holder.setData("import QtQml; QtObject { property var state: null }", QUrl());
    std::unique_ptr<QObject> probe(holder.create());
    QVERIFY2(probe, qPrintable(holder.errorString()));
    engine->rootContext()->setContextProperty("policyProbe", probe.get());
    const auto mode = text("mode");
    QVERIFY(evaluate(R"JS((() => {
        const ui = {start_page_owner_token: 'policy-test-owner', active_tab_index: 0,
            decision: 'pending', status_text: '', leases: 0,
            permission_decision: function() { return this.decision },
            begin_native_prompt_lease: function() { this.leases++; return '' }};
        const view = {viewUi: ui, viewHost: testWindow, viewRetired: false,
            tabIndex: 0, stableTabId: 'policy-tab', documentRequestOwner: {},
            ownsTab: function() { return true }};
        let type = 0;
        while (type < 50 && testWindow.permissionTypeName(type) !== 'geolocation') type++;
        const request = {origin: 'https://policy.example.test', permissionType: type,
            isValid: true, grants: 0, denials: 0,
            grant: function() { this.grants++; this.isValid = false },
            deny: function() { this.denials++; this.isValid = false }};
        policyProbe.state = {ui: ui, view: view, request: request};
        testWindow.handlePermissionRequested(request, 0, ui, view, false, testWindow);
        return testWindow.permissionPromptQueue.length === 1;
    })())JS").toBool());
    QVERIFY(!evaluate("testWindow.permissionPromptVisible").toBool());
    QCOMPARE(evaluate("policyProbe.state.ui.leases").toInt(), 0);
    QCOMPARE(text("mode"), mode);
    if (outcome == "allow" || outcome == "deny" || outcome == "ask")
        evaluate("policyProbe.state.ui.decision=" + quoted(outcome));
    else if (outcome == "navigation")
        evaluate("policyProbe.state.view.documentRequestOwner=({})");
    else if (outcome == "transfer")
        evaluate("policyProbe.state.view.viewUi=({})");
    else if (outcome == "tab-switch")
        evaluate("policyProbe.state.ui.active_tab_index=1");
    else if (outcome == "retired")
        evaluate("policyProbe.state.view.viewRetired=true");
    else if (outcome == "closed-host")
        evaluate("(()=>{const host={runtimeReleased:true};policyProbe.state.view.viewHost=host;"
                 "testWindow.permissionPromptQueue[0].host=host})()");
    else if (outcome == "invalid-request")
        evaluate("policyProbe.state.request.isValid=false");
    if (outcome == "ask" || outcome == "timeout") {
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.permissionPromptVisible").toBool(), 3500);
        QCOMPARE(evaluate("policyProbe.state.ui.leases").toInt(), 1);
        QCOMPARE(evaluate("testWindow.pendingPermissionPolicyNotice.length>0").toBool(), outcome == "timeout");
        QCOMPARE(evaluate("policyProbe.state.request.grants").toInt(), 0);
        evaluate("testWindow.decidePermission(false,'')");
    }
    QTRY_COMPARE_WITH_TIMEOUT(evaluate("testWindow.permissionPromptQueue.length").toInt(), 0, 3500);
    QCOMPARE(evaluate("policyProbe.state.request.grants").toInt(), outcome == "allow" ? 1 : 0);
    QCOMPARE(evaluate("policyProbe.state.request.denials").toInt(),
             outcome == "allow" || outcome == "invalid-request" ? 0 : 1);
    evaluate("testWindow.refreshPendingPermissionPolicies()");
    QCOMPARE(evaluate("policyProbe.state.request.grants+policyProbe.state.request.denials").toInt(),
             outcome == "invalid-request" ? 0 : 1);
    engine->rootContext()->setContextProperty("policyProbe", static_cast<QObject *>(nullptr));
}

void ApplicationTests::productPolishPermissionQueueBound() {
    QVERIFY(evaluate(R"JS((() => {
        const ui = {start_page_owner_token: 'budget-owner', active_tab_index: 0,
            status_text: '', permission_decision: function() { return 'pending' }};
        const view = {viewUi: ui, viewHost: testWindow, viewRetired: false,
            tabIndex: 0, stableTabId: 'budget-tab', documentRequestOwner: {},
            ownsTab: function() { return true }};
        let type = 0;
        while (type < 50 && testWindow.permissionTypeName(type) !== 'geolocation') type++;
        let denied = 0;
        for (let i = 0; i < testWindow.permissionPromptQueueLimit + 2; ++i) {
            testWindow.handlePermissionRequested({origin: 'https://budget.example.test',
                permissionType: type, isValid: true,
                deny: function() { denied++; this.isValid = false }}, 0, ui, view, false, testWindow);
        }
        const key = testWindow.permissionQueueKey('https://budget.example.test','geolocation',false,ui,view,0);
        const unrelated = testWindow.permissionQueueKey('https://budget.example.test','geolocation',false,
            {start_page_owner_token:'other-owner'},view,0);
        const bounded = testWindow.permissionQueueRequestCount(testWindow.permissionPromptQueue)
            === testWindow.permissionPromptQueueLimit && denied === 2 && key !== unrelated;
        testWindow.cancelPermissionForUi(ui);
        return bounded && testWindow.permissionPromptQueue.length === 0
            && denied === testWindow.permissionPromptQueueLimit + 2;
    })())JS").toBool());
}


void ApplicationTests::productPolishPermissionCapacity_data() {
    QTest::addColumn<bool>("savedDenial");
    QTest::newRow("saved-denial") << true;
    QTest::newRow("exact-absence") << false;
}
void ApplicationTests::productPolishPermissionCapacity() {
    QFETCH(bool, savedDenial);
    const auto url = text("current_url");
    const auto origin = QUrl(url).adjusted(QUrl::RemovePath | QUrl::RemoveQuery | QUrl::RemoveFragment).toString();
    const QString create = "testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:" + quoted(url)
        + ",windowProfileName:'default',windowStorageBasePath:testWindow.storageBasePath+'/capacity-profile'})";
    evaluate(create);
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
    const QString owner = "testWindow.browserWindowRegistry[1]";
    QTRY_VERIFY_WITH_TIMEOUT(evaluate(owner + ".host.profileBootstrapComplete && !" + owner + ".ui.profile_bootstrap_pending").toBool(), 15000);
    QDirIterator files(evaluate("testWindow.storageBasePath+'/capacity-profile'").toString(), {"browser.sqlite"}, QDir::Files, QDirIterator::Subdirectories);
    QVERIFY(files.hasNext());
    const auto path = QFile::encodeName(files.next());
    evaluate("(()=>{const host=" + owner + ".host;host.windowShutdownApproved=true;host.close();host.destroy()})()");
    QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 1);
    struct Database {
        sqlite3 *value = nullptr;
        ~Database() { if (value) sqlite3_close(value); }
    } database;
    QCOMPARE(sqlite3_open_v2(path.constData(), &database.value, SQLITE_OPEN_READWRITE, nullptr), SQLITE_OK);
    QCOMPARE(sqlite3_exec(database.value, "BEGIN IMMEDIATE", nullptr, nullptr, nullptr), SQLITE_OK);
    sqlite3_stmt *insert = nullptr;
    QCOMPARE(sqlite3_prepare_v2(database.value,
        "INSERT INTO permission_rules(origin,permission,decision,expires_at,updated_at) VALUES(?1,'geolocation','deny',NULL,1)",
        -1, &insert, nullptr), SQLITE_OK);
    const auto encoded = (savedDenial ? origin : QString("https://other-denial.example.test")).toUtf8();
    QCOMPARE(sqlite3_bind_text(insert, 1, encoded.constData(), encoded.size(), SQLITE_TRANSIENT), SQLITE_OK);
    QCOMPARE(sqlite3_step(insert), SQLITE_DONE);
    sqlite3_finalize(insert);
    QCOMPARE(sqlite3_exec(database.value,
        "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<1000) "
        "INSERT INTO permission_rules(origin,permission,decision,expires_at,updated_at) "
        "SELECT 'https://noise-'||x||'.example.test','notifications','deny',NULL,x+10 FROM n",
        nullptr, nullptr, nullptr), SQLITE_OK);
    QCOMPARE(sqlite3_exec(database.value, "COMMIT", nullptr, nullptr, nullptr), SQLITE_OK);
    sqlite3_close(database.value); database.value = nullptr;
    evaluate(create);
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
    QTRY_VERIFY_WITH_TIMEOUT(evaluate(owner + ".host.profileBootstrapComplete && !" + owner + ".view.loading && !" + owner + ".ui.profile_bootstrap_pending").toBool(), 15000);
    QTRY_COMPARE_WITH_TIMEOUT(evaluate("JSON.parse("+owner+".ui.config_json).permissions.geolocation").toString(), QString("allow"),5000);
    pageReceiver = owner + ".view";
    QTRY_VERIFY_WITH_TIMEOUT(evaluate(owner + ".view.url.toString()===" + quoted(url)
        + " && " + owner + ".ui.load_state==='complete'").toBool(),15000);
    QTRY_VERIFY_WITH_TIMEOUT(page("document.readyState==='complete' && !!document.getElementById('button')").toBool(),15000);
    QQmlComponent holder(engine.get());
    holder.setData("import QtQml; QtObject { property var state: ({requests:0,flashes:0}) }", QUrl());
    std::unique_ptr<QObject> probe(holder.create());
    QVERIFY2(probe, qPrintable(holder.errorString()));
    engine->rootContext()->setContextProperty("nativePolicyProbe", probe.get());
    evaluate("(()=>{const state=nativePolicyProbe.state;state.observePermission=function(request){state.requests++;state.request=request};"
        + owner + ".view.permissionRequested.connect(state.observePermission);"
        "state.observeVisibility=function(){if(testWindow.permissionPromptVisible)state.flashes++};"
        "testWindow.permissionPromptVisibleChanged.connect(state.observeVisibility)})()");
    QVERIFY(page("navigator.geolocation.getCurrentPosition(()=>window.geoError=0,error=>window.geoError=error.code);true").toBool());
    QTRY_VERIFY_WITH_TIMEOUT(evaluate("nativePolicyProbe.state.requests>0").toBool(),5000);
    const auto permission = evaluate("nativePolicyProbe.state.request").value<QWebEnginePermission>();
    QVERIFY(permission.isValid());
    qInfo() << "Native policy outcome" << int(permission.state()) << "origin" << permission.origin()
            << "page error" << page("window.geoError")
            << "application choice" << evaluate(owner+".ui.permission_decision("+quoted(origin)+",'geolocation')");
    QTRY_COMPARE_WITH_TIMEOUT(permission.state(), savedDenial ? QWebEnginePermission::State::Denied : QWebEnginePermission::State::Granted, 5000);
    QCOMPARE(evaluate("nativePolicyProbe.state.flashes").toInt(),0);
    evaluate(owner+".view.permissionRequested.disconnect(nativePolicyProbe.state.observePermission)");
    evaluate("testWindow.permissionPromptVisibleChanged.disconnect(nativePolicyProbe.state.observeVisibility)");
    QVERIFY(!evaluate("testWindow.permissionPromptVisible").toBool());
    QTRY_COMPARE_WITH_TIMEOUT(evaluate(owner+".ui.permission_decision("+quoted(origin)+",'geolocation')").toString(), savedDenial ? QString("deny") : QString("allow"),5000);
    const QString expected = savedDenial
        ? "value.durable_rules.some(r=>r.permission==='geolocation' && r.decision==='deny')"
        : "value.durable_rules.length===0";
    QTRY_VERIFY_WITH_TIMEOUT(evaluate("(()=>{const value=JSON.parse("+owner+".ui.refresh_site_status()).facts.find(f=>f.id==='permissions').value;return value.ready && ("+expected+")})()").toBool(),5000);
}

void ApplicationTests::productPolishPopupDurableWork() {
    QVERIFY(command("bind p hint --target pointer links"));
    key(Qt::Key_P);
    const auto label = hintLabel("Open popup");
    QVERIFY(!label.isEmpty());
    typeText(label);
    key(Qt::Key_Return);
    QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(), 1);
    const auto owner = QString("testWindow.popupWindowRegistry[0]");
    QTRY_VERIFY(evaluate(owner + ".host.profileBootstrapComplete && !" + owner + ".ui.profile_bootstrap_pending").toBool());
    QTRY_VERIFY(!evaluate(owner + ".view.loading").toBool());
    const auto tab = evaluate(owner + ".ui.tab_id_for_index(0)").toString();
    QVERIFY(!tab.isEmpty());
    QDirIterator files(evaluate("testWindow.storageBasePath").toString(), {"browser.sqlite"}, QDir::Files, QDirIterator::Subdirectories);
    QVERIFY(files.hasNext());
    struct Database {
        sqlite3 *value = nullptr;
        ~Database() { if (value) sqlite3_close(value); }
    } database;
    const auto path = QFile::encodeName(files.next());
    QCOMPARE(sqlite3_open_v2(path.constData(), &database.value, SQLITE_OPEN_READONLY, nullptr), SQLITE_OK);
    const auto count = [&](const QString &query) {
        sqlite3_stmt *statement = nullptr;
        if (sqlite3_prepare_v2(database.value, query.toUtf8().constData(), -1, &statement, nullptr) != SQLITE_OK) return -1;
        const int result = sqlite3_step(statement) == SQLITE_ROW ? sqlite3_column_int(statement, 0) : -1;
        sqlite3_finalize(statement);
        return result;
    };
    // Observe worker output directly: opening a manager must not be needed to
    // drain the popup's queue or make its initial navigation durable.
    QTRY_COMPARE_WITH_TIMEOUT(count("SELECT count(*) FROM journey_nodes WHERE tab_id='" + tab + "' AND url LIKE '%/popup.html'"), 1, 15000);
    evaluate("testWindow.browserUi.status_text='Primary feedback'");
    for (const auto &origin : {QString("https://popup-first.test"), QString("https://popup-second.test")}) {
        QVERIFY(evaluate(owner + ".ui.remember_permission(" + quoted(origin) + ",'geolocation','deny','site')").toBool());
    }
    QTRY_COMPARE_WITH_TIMEOUT(count("SELECT count(*) FROM permission_rules WHERE origin IN ('https://popup-first.test','https://popup-second.test') AND permission='geolocation' AND decision='deny'"), 2, 15000);
    QCOMPARE(text("status_text"), QString("Primary feedback"));
}
