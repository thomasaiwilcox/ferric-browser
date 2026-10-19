#include "tst_application.h"
#include <QJsonDocument>
#include <QJsonObject>
#include <QtTest>
#include <QClipboard>
#include <QGuiApplication>
#include <QQuickItem>

void ApplicationTests::navigationJourneys_data() {
    QTest::addColumn<QString>("method");
    for (const auto name : {"open", "link", "hint", "push-state", "same-url-state", "mouse-back"})
        QTest::newRow(name) << QString(name);
}
void ApplicationTests::navigationJourneys() {
    QFETCH(QString, method);
    QTest::failOnWarning(QRegularExpression(".*(TypeError|ReferenceError|invalid context).*"));
    const auto first = text("current_url");
    const auto second = first + "?navigation-second=1";
    const auto third = first + "?navigation-third=1";
    if (method == "open" || method == "mouse-back") {
        interactiveCommand("open " + second);
    } else if (method == "push-state" || method == "same-url-state") {
        page("history.pushState({position:1},''," + quoted(method == "same-url-state" ? first : second) + ");true");
    } else {
        page("document.getElementById('new').href=" + quoted(second) + ";true");
        if (method == "hint") {
            QVERIFY(command("bind x hint"));
            key(Qt::Key_X);
            QTRY_COMPARE(text("mode"),"hint");
            QCOMPARE(text("hint_family"),"actions");
            key(Qt::Key_Escape);
            QVERIFY(command("set --temp hints.auto_follow=never"));
            key(Qt::Key_F);
            const auto label = hintLabel("New document");
            QVERIFY(!label.isEmpty());
            typeText(label);
            key(Qt::Key_Return);
        } else page("document.getElementById('new').click();true");
    }
    const auto destination = method == "same-url-state" ? first : second;
    QTRY_VERIFY(page("location.href===" + quoted(destination) + " && document.readyState==='complete'").toBool());
    QTRY_VERIFY(evaluate("testWindow.activeWebView().contentCanGoBack").toBool());
    if (method == "push-state" || method == "same-url-state") {
        page("history.pushState({position:2},''," + quoted(third) + ");true");
    } else {
        page("document.getElementById('new').href=" + quoted(third) + ";document.getElementById('new').click();true");
    }
    QTRY_VERIFY(page("location.href===" + quoted(third) + " && document.readyState==='complete'").toBool());
    QTRY_VERIFY(evaluate("testWindow.activeWebView().contentCanGoBack").toBool());
    qInfo() << "navigation history" << method << evaluate("testWindow.browserUi.content_history_export(0)");
    if (method == "mouse-back") QTest::mouseClick(window, Qt::BackButton, Qt::NoModifier, QPoint(window->width()/2,window->height()/2));
    else key(Qt::Key_H, Qt::ShiftModifier);
    QTRY_COMPARE_WITH_TIMEOUT(page("location.href").toString(), destination, 5000);
    if (method == "same-url-state") QCOMPARE(page("history.state.position").toInt(),1);
    interactiveCommand("back");
    QTRY_COMPARE_WITH_TIMEOUT(page("location.href").toString(), first, 5000);
    if (method == "same-url-state") QVERIFY(page("history.state===null").toBool());
    key(Qt::Key_2);
    key(Qt::Key_L, Qt::ShiftModifier);
    QTRY_COMPARE_WITH_TIMEOUT(page("location.href").toString(), third, 5000);
    if (method == "open") {
        page("window.reloadMarker='must disappear';true");
        key(Qt::Key_R);
        QTRY_VERIFY(page("typeof window.reloadMarker==='undefined' && document.readyState==='complete'").toBool());
        QCOMPARE(page("location.href").toString(),third);
    }
}

void ApplicationTests::urlCopyStatusBar_data() {
    QTest::addColumn<bool>("secondary");
    QTest::newRow("primary-single-tab") << false;
    QTest::newRow("secondary-single-tab") << true;
}
void ApplicationTests::urlCopyStatusBar() {
    QFETCH(bool, secondary);
    const auto initial = text("current_url");
    QString ui = "testWindow.browserUi";
    QString owner = "testWindow";
    auto hostWindow = window;
    auto expected = initial;
    if (secondary) {
        expected += "?copy-owner=secondary";
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"
            + quoted(expected) + ",windowProfileName:'copy-status',windowProfileLabel:'Copy status'})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
        owner = "testWindow.browserWindowRegistry[1].host";
        ui = "testWindow.browserWindowRegistry[1].ui";
        QTRY_VERIFY(evaluate(owner + ".profileBootstrapComplete").toBool());
        hostWindow = qobject_cast<QQuickWindow *>(evaluate(owner).value<QObject *>());
        QVERIFY(hostWindow);
        QTRY_COMPARE(evaluate(ui + ".current_url").toString(),expected);
        QTRY_VERIFY(evaluate(owner + ".activeView && " + owner + ".activeView.ownsTab() && !" + owner + ".activeView.loading && String(" + owner + ".activeView.url)===" + quoted(expected)).toBool());
    }
    QCOMPARE(evaluate(ui + ".tab_count").toInt(),1);
    auto button = hostWindow->findChild<QQuickItem *>("copyCurrentUrlButton");
    QVERIFY(button);
    // The default in-mode policy deliberately hides all status chrome in
    // Normal mode. Showing a copy control must not override that preference.
    QVERIFY(evaluate(ui + ".set_runtime_setting('ui.statusbar','in-mode',true)").toBool());
    QTRY_VERIFY(!button->isVisible());
    QVERIFY(evaluate(ui + ".set_runtime_setting('ui.statusbar','always',true)").toBool());
    QTRY_VERIFY(!evaluate(owner + ".tabStripVisible").toBool());
    QTRY_VERIFY(button->isVisible() && button->isEnabled());
    hostWindow->requestActivate();
    QTRY_VERIFY(hostWindow->isActive());
    QGuiApplication::clipboard()->setText("before status copy");
    const auto point = button->mapToScene(QPointF(button->width()/2,button->height()/2)).toPoint();
    QTest::mouseClick(hostWindow,Qt::LeftButton,Qt::NoModifier,point);
    QTRY_COMPARE(QGuiApplication::clipboard()->text(),expected);
    QTRY_COMPARE(evaluate(ui + ".status_text").toString(),"URL copied");
    if (secondary) QCOMPARE(text("current_url"),initial);
}

void ApplicationTests::hintNavigationLifecycle_data() {
    QTest::addColumn<bool>("secondary");
    QTest::addColumn<QString>("method");
    for (const auto secondary : {false, true}) {
        for (const auto method : {"location", "busy-location", "form", "same-document", "replacement", "background"}) {
            const auto name = QString(secondary ? "secondary-" : "primary-") + method;
            QTest::newRow(qPrintable(name)) << secondary << QString(method);
        }
    }
}

void ApplicationTests::hintNavigationLifecycle() {
    QFETCH(bool, secondary);
    QFETCH(QString, method);
    QTest::failOnWarning(QRegularExpression(".*(invalid context|TypeError|ReferenceError|Binding loop).*"));
    const auto first = text("current_url");
    const auto destination = first + "?hint-destination=1";
    QString owner = "testWindow";
    QString ui = "testWindow.browserUi";
    auto ownerWindow = window;
    if (secondary) {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"
            + quoted(first) + ",windowProfileName:'hint-navigation',windowProfileLabel:'Hint navigation'})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
        owner = "testWindow.browserWindowRegistry[1].host";
        ui = "testWindow.browserWindowRegistry[1].ui";
        QTRY_VERIFY(evaluate(owner + ".profileBootstrapComplete && !" + ui + ".profile_bootstrap_pending").toBool());
        ownerWindow = qobject_cast<QQuickWindow *>(evaluate(owner).value<QObject *>());
        QVERIFY(ownerWindow);
        pageReceiver = owner + ".activeView";
        QTRY_VERIFY(evaluate(pageReceiver + ".ownsTab() && !" + pageReceiver
            + ".loading && " + pageReceiver + ".mainFrame.isValid && String("
            + pageReceiver + ".url)===" + quoted(first)).toBool());
    }
    if (method == "background") {
        QVERIFY(evaluate(ui + ".execute_command(" + quoted("tab-open --background " + destination) + ")").toBool());
        QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), 2);
        QTRY_VERIFY(evaluate(owner + ".tabViewAt(1) && !" + owner + ".tabViewAt(1).loading").toBool());
    }
    if (method == "form") {
        page("document.body.innerHTML='<form method=get><input type=hidden name=hint-destination value=1><button>Continue</button></form>';document.querySelector('form').action=" + quoted(first) + ";true");
    } else {
        auto handler = method == "same-document"
            ? "history.pushState({hint:true},''," + quoted(destination) + ")"
            : "location.href=" + quoted(destination);
        if (method == "busy-location")
            handler += ";const until=performance.now()+200;while(performance.now()<until){}";
        page("document.body.innerHTML='<button id=next>Continue</button>';document.getElementById('next').addEventListener('click',()=>{" + handler + "});true");
    }
    QVERIFY(evaluate(ui + ".execute_command('set --temp hints.auto_follow=never')").toBool());
    ownerWindow->requestActivate();
    QTRY_VERIFY(ownerWindow->isActive());
    evaluate(pageReceiver + ".forceActiveFocus()");
    QTest::keyClick(ownerWindow, Qt::Key_F);
    const auto label = hintLabel("Continue", ui);
    QVERIFY(!label.isEmpty());
    if (method == "background") {
        const auto backgroundDestination = first + "?background-finished=1";
        evaluate(owner + ".tabViewAt(1).url=" + quoted(backgroundDestination));
        const auto activeReceiver = pageReceiver;
        pageReceiver = owner + ".tabViewAt(1)";
        QTRY_VERIFY(page("location.href===" + quoted(backgroundDestination) + " && document.readyState==='complete'").toBool());
        pageReceiver = activeReceiver;
        QCOMPARE(evaluate(ui + ".mode").toString(), "hint");
        QVERIFY(evaluate(ui + ".hint_visible").toBool());
        QCOMPARE(hintLabel("Continue", ui), label);
    } else {
        if (method != "replacement") {
            for (const auto character : label)
                QTest::keyClick(ownerWindow, Qt::Key(Qt::Key_A + character.toLower().unicode() - 'a'));
        }
        if (method == "replacement") {
            // Document replacement can retire an activation callback before
            // it returns; navigation owns cancellation independently of it.
            evaluate(pageReceiver + ".url=" + quoted(destination));
        } else QTest::keyClick(ownerWindow, Qt::Key_Return);
        QTRY_VERIFY(page("location.href===" + quoted(destination) + " && document.readyState==='complete'").toBool());
        QTRY_COMPARE(evaluate(ui + ".mode").toString(), "normal");
        QVERIFY(!evaluate(ui + ".hint_visible").toBool());
        QVERIFY(!evaluate(owner + ".hintWorkspace.hintRefreshPending").toBool());
        QTest::keyClick(ownerWindow, Qt::Key_F);
        QVERIFY(!hintLabel(method == "same-document" ? "Continue" : "New document", ui).isEmpty());
    }
    QTest::keyClick(ownerWindow, Qt::Key_Escape);
    QTRY_COMPARE(evaluate(ui + ".mode").toString(), "normal");
    if (secondary) {
        QCOMPARE(text("mode"), "normal");
        QCOMPARE(text("current_url"), first);
    }
}
