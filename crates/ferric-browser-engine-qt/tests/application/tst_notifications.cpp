#include "tst_application.h"
#include "notification_test_service.h"
#include <QQuickItem>
#include <QRegularExpression>
#include <QUrl>
#include <QtTest>

void ApplicationTests::productPolishNativeNotifications_data()
{
    QTest::addColumn<QString>("kind");
    for (const auto &kind : {"primary", "secondary", "private", "required-portal",
                             "service-error", "page-close", "owner-close"})
        QTest::newRow(kind) << QString::fromLatin1(kind);
}

void ApplicationTests::productPolishNativeNotifications()
{
    if (qEnvironmentVariable("FERRIC_NOTIFICATION_TEST_BUS") != QStringLiteral("isolated"))
        QSKIP("Run application_native_notifications: requires the isolated test bus");
    QFETCH(QString, kind);
    QTest::failOnWarning(QRegularExpression("TypeError|ReferenceError|Binding loop"));
    NotificationTestService service(QStringLiteral("native"));
    QVERIFY(service.start());
    const auto origin = QString("http://127.0.0.1:%1").arg(server.serverPort());
    const auto url = origin + "/notification-polish";
    frameFixtures["/notification-polish"] = R"HTML(<!doctype html><meta charset="utf-8">
      <style>body{margin:20px}button{width:300px;height:60px}</style>
      <button id="notify" onclick="Notification.requestPermission().then(function(result){
        window.permissionResult=result;
        if(result==='granted') {
          window.notice=new Notification('Literal <b>title</b>',{body:'Literal <b>body</b> & details'});
          notice.onshow=function(){window.shown++};
          notice.onclose=function(){window.noticeClosed++};
          notice.onclick=function(){window.clicked++};
        }
      })">Enable notifications once</button>
      <script>window.shown=0;window.noticeClosed=0;window.clicked=0;</script>)HTML";
    QString owner = "testWindow.browserWindowRegistry[0]";
    const bool separate = kind == "secondary" || kind == "private" || kind == "owner-close";
    if (separate) {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:" + quoted(url)
            + ",windowProfileName:'notification-owner',windowProfileLabel:'Notification owner',windowPrivateProfile:"
            + (kind == "private" ? "true" : "false") + "})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
        owner = "testWindow.browserWindowRegistry[1]";
    } else {
        QVERIFY(command("open " + url));
    }
    pageReceiver = owner + ".view";
    const auto ui = owner + ".ui";
    QTRY_VERIFY_WITH_TIMEOUT(evaluate(owner + ".host.profileBootstrapComplete && !" + ui
        + ".profile_bootstrap_pending && !!" + pageReceiver + " && !" + pageReceiver + ".loading").toBool(), 15000);
    QTRY_VERIFY_WITH_TIMEOUT(page("document.readyState==='complete' && location.pathname==='/notification-polish' && !!document.getElementById('notify')").toBool(), 15000);
    QTRY_COMPARE(evaluate("String(" + ui + ".permission_decision(" + quoted(origin) + ",'notifications'))").toString(), QString("ask"));
    if (kind == "required-portal") {
        QCOMPARE(evaluate(ui + ".desktop_portal_mode").toString(), QString("required"));
        QVERIFY(evaluate(ui + ".probe_desktop_portals()").toBool());
        QTRY_COMPARE_WITH_TIMEOUT(evaluate(ui + ".portal_capability_status('notifications')").toString(), QString("available"), 10000);
    }
    auto host = qobject_cast<QQuickWindow *>(evaluate(owner + ".host").value<QObject *>());
    auto view = qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());
    QVERIFY(host && view);
    host->requestActivate();
    QTRY_VERIFY(host->isActive());
    view->forceActiveFocus();
    const auto point = page("(()=>{const r=document.getElementById('notify').getBoundingClientRect();return [r.x+r.width/2,r.y+r.height/2]})()").toList();
    QTest::qWait(100);
    QTest::mouseClick(host, Qt::LeftButton, Qt::NoModifier,
        view->mapToScene(QPointF(point[0].toDouble(), point[1].toDouble())).toPoint());
    QTRY_VERIFY(evaluate("testWindow.permissionPromptVisible && testWindow.pendingPermissionName==='notifications'").toBool());
    evaluate("testWindow.decidePermission(true,'')");
    QTRY_COMPARE(page("window.permissionResult").toString(), QString("granted"));
    // Allow once must remain Ask in Ferric's saved policy. Native WebEngine
    // authorization is the authority for this particular delivered message.
    QCOMPARE(evaluate("String(" + ui + ".permission_decision(" + quoted(origin) + ",'notifications'))").toString(), QString("ask"));
    QTRY_COMPARE(service.pendingCount(), 1);
    QCOMPARE(page("window.shown").toInt(), 0);
    QVERIFY(evaluate(ui + ".status_text").toString().startsWith("Sending desktop notification"));
    const auto request = service.delivery(0);
    const bool portal = kind == "required-portal";
    QCOMPARE(request["member"].toString(), portal ? QString("AddNotification") : QString("Notify"));
    if (kind == "owner-close") {
        evaluate(owner + ".host.windowShutdownApproved=true;" + owner + ".host.close()");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 1);
        service.acceptNext(77);
        QTRY_VERIFY(service.closedIds().contains(77));
        QTRY_COMPARE(evaluate("testWindow.notificationOwners.length").toInt(), 0);
        return;
    }
    if (kind == "service-error") {
        service.rejectNext("error");
        QTRY_COMPARE(evaluate("testWindow.notificationOwners.length").toInt(), 0);
        QCOMPARE(page("window.shown").toInt(), 0);
        QVERIFY(evaluate(ui + ".status_text").toString().contains("service unavailable"));
        return;
    }
    service.acceptNext(77);
    QTRY_COMPARE(page("window.shown").toInt(), 1);
    QVERIFY(evaluate(ui + ".status_text").toString().startsWith("Desktop notification sent"));
    const auto nativeScope = evaluate("Object.keys(testWindow.activeWebNotifications)[0].split('\\n')[0]").toString();
    QVERIFY(nativeScope.startsWith(kind == "private" ? "private:" : "normal:"));
    if (kind == "private")
        QVERIFY(nativeScope.endsWith(evaluate("encodeURIComponent(" + ui + ".start_page_owner_token)").toString()));
    if (kind == "page-close") {
        page("window.notice.close()");
        QTRY_VERIFY(service.closedIds().contains(77));
    } else {
        if (portal) {
            const auto id = request["arguments"].toArray()[0].toString();
            service.signal(true, "ActionInvoked", {id, QStringLiteral("default"), QVariantList{}});
        } else {
            service.signal(false, "ActionInvoked", {uint(77), QStringLiteral("default")});
        }
        QTRY_COMPARE(page("window.clicked").toInt(), 1);
        QTRY_COMPARE(evaluate("testWindow.activationStatus").toString(), QString("activated"));
        QTest::qWait(300); // Exercise deferred compositor activation as well.
        if (portal) {
            page("window.notice.close()");
            QTRY_COMPARE(service.removedIds().size(), 1);
        } else {
            service.signal(false, "NotificationClosed", {uint(77), uint(2)});
            QTRY_COMPARE(page("window.noticeClosed").toInt(), 1);
        }
    }
    QTRY_COMPARE(evaluate("testWindow.notificationOwners.length").toInt(), 0);
    QCOMPARE(evaluate("Object.keys(testWindow.activeWebNotifications).length").toInt(), 0);
}
