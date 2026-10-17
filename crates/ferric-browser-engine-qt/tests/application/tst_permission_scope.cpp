#include "tst_application.h"
#include <QQuickItem>
#include <QUrl>
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
    QVERIFY(evaluate("testWindow.pendingPermissionUi===" + child + ".ui").toBool());
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
    evaluate(QString(R"JS((() => {
        const make = () => ({origin: %1, closed: false,
            close: function() { this.closed = true }});
        const a = make(), b = make(), separator = String.fromCharCode(10);
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
                     "activeWebNotifications)[0]].closed")
                .toBool());
    QCOMPARE(evaluate("testWindow.pendingPortalNotifications.length").toInt(), 1);
    QVERIFY(
        evaluate(
            "testWindow.pendingPortalNotifications[0].ui===testWindow.browserWindowRegistry[1].ui")
            .toBool());
}
