#include "tst_application.h"
#include "owned_browser_keyboard.h"
#include <QtTest>

void ApplicationTests::repeatAdmission_data() {
    QTest::addColumn<QString>("hostKind");
    QTest::addColumn<QString>("rejection");
    for (const auto &host : {QString("primary"), QString("secondary")})
        for (const auto &failure : {QString("profile"), QString("input"),
                QString("local"), QString("partial-chain"), QString("clean-cancel")})
            QTest::newRow(qPrintable(host + "-" + failure)) << host << failure;
}

void ApplicationTests::repeatAdmission() {
    QFETCH(QString, hostKind);
    QFETCH(QString, rejection);
    QString owner, ui;
    QQuickWindow *host = nullptr;
    preparePageSearchHost(hostKind, owner, ui, host);
    QVERIFY(host);
    const auto initial = page("location.href").toString();
    const auto destination = initial + "?accepted=yes";
    QVERIFY(evaluate(ui + ".execute_command('macro-record a')").toBool());
    const auto accepted = "open --target tab-bg " + destination;
    const bool partial = rejection == "partial-chain";
    if (!partial) {
        QVERIFY(evaluate(ui + ".execute_command(" + quoted(accepted) + ")").toBool());
        evaluate("testWindow.completeBrowserKeyAction(" + ui + "," + owner + ")");
    }
    const auto failed = rejection == "profile" ? "open --target tab --profile wrong " + initial
        : rejection == "input" ? QString("open --target tab http://[invalid")
        : rejection == "local" ? QString("tab-open http://[invalid")
        : accepted + " ;; open --target tab http://[invalid";
    if (rejection == "clean-cancel") {
        QVERIFY(evaluate(ui + ".execute_command(" + quoted("open --clean-link --target tab "
            + initial + "?utm_source=cancel-test") + ")").toBool());
        QTRY_VERIFY(evaluate(ui + ".link_preview_visible").toBool());
        evaluate(ui + ".clear_link_preview()");
    } else {
        QVERIFY2(!evaluate(ui + ".execute_command(" + quoted(failed) + ")").toBool(), qPrintable(failed));
    }
    evaluate("testWindow.completeBrowserKeyAction(" + ui + "," + owner + ")");
    QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), 2);
    QCOMPARE(evaluate(ui + ".active_tab_index").toInt(), 0);
    QVERIFY2(evaluate(ui + ".macro_status_text.endsWith('(1)')").toBool(),
        qPrintable(evaluate(ui + ".macro_status_text").toString()));
    QVERIFY(evaluate(ui + ".execute_command('macro-stop')").toBool());
    sendOwnedBrowserKey(host, Qt::Key_Period);
    QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), 3);
    QCOMPARE(evaluate(ui + ".active_tab_index").toInt(), 0);
    pageReceiver = owner + ".tabViewAt(2)";
    QTRY_VERIFY(evaluate("!!" + pageReceiver).toBool());
    QTRY_COMPARE(page("location.href").toString(), destination);
    QVERIFY(evaluate(ui + ".execute_command('macro-play a')").toBool());
    evaluate("testWindow.completeBrowserKeyAction(" + ui + "," + owner + ")");
    QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), 4);
    QCOMPARE(evaluate(ui + ".active_tab_index").toInt(), 0);
    pageReceiver = owner + ".tabViewAt(3)";
    QTRY_VERIFY(evaluate("!!" + pageReceiver).toBool());
    QTRY_COMPARE(page("location.href").toString(), destination);
}

void ApplicationTests::repeatCommonCommands_data() {
    QTest::addColumn<QString>("hostKind");
    QTest::addColumn<QString>("action");
    for (const auto &host : {QString("primary"), QString("secondary")})
        for (const auto &action : {QString("zoom"), QString("scroll"), QString("stale-clean")})
            QTest::newRow(qPrintable(host + "-" + action)) << host << action;
}

void ApplicationTests::repeatCommonCommands() {
    QFETCH(QString, hostKind);
    QFETCH(QString, action);
    QString owner, ui;
    QQuickWindow *host = nullptr;
    preparePageSearchHost(hostKind, owner, ui, host);
    QVERIFY(host);
    if (action == "stale-clean") {
        const auto initial = page("location.href").toString();
        const auto accepted = initial + "?newer=yes";
        QVERIFY(evaluate(ui + ".execute_command('macro-record a')").toBool());
        QVERIFY(evaluate(ui + ".execute_command(" + quoted("open --clean-link --target current "
            + initial + "?utm_source=stale-test") + ")").toBool());
        QTRY_VERIFY(evaluate(ui + ".link_preview_visible").toBool());
        QVERIFY(evaluate(ui + ".execute_command(" + quoted("open --target current " + accepted) + ")").toBool());
        evaluate("testWindow.completeBrowserKeyAction(" + ui + "," + owner + ")");
        QTRY_COMPARE(page("location.href").toString(), accepted);
        QVERIFY2(!evaluate(ui + ".confirm_link_navigation()").toBool(), "A stale clean-link target must reject confirmation");
        QVERIFY(evaluate(ui + ".macro_status_text.endsWith('(1)')").toBool());
        evaluate(ui + ".clear_link_preview()");
        QVERIFY(evaluate(ui + ".execute_command('macro-stop')").toBool());
        page("window.repeatSentinel=true");
        sendOwnedBrowserKey(host, Qt::Key_Period);
        QTRY_VERIFY(page("typeof window.repeatSentinel==='undefined'").toBool());
        QTRY_COMPARE(page("location.href").toString(), accepted);
        QCOMPARE(evaluate(ui + ".tab_count").toInt(), 1);
        QVERIFY(evaluate(ui + ".execute_command('macro-play a')").toBool());
        evaluate("testWindow.completeBrowserKeyAction(" + ui + "," + owner + ")");
        QTRY_COMPARE(page("location.href").toString(), accepted);
        QCOMPARE(evaluate(ui + ".tab_count").toInt(), 1);
        return;
    }
    page("document.body.style.height='12000px';window.scrollTo(0,0)");
    QVERIFY(evaluate(ui + ".execute_command('macro-record a')").toBool());
    const auto command = action == "zoom" ? QString("zoom in") : QString("scroll down");
    QVERIFY(evaluate(ui + ".execute_command(" + quoted(command) + ")").toBool());
    evaluate("testWindow.completeBrowserKeyAction(" + ui + "," + owner + ")");
    if (action == "zoom") QTRY_VERIFY(evaluate(pageReceiver + ".zoomFactor").toDouble() > 1.05);
    else QTRY_VERIFY(page("window.scrollY").toDouble() > 0);
    QVERIFY(evaluate(ui + ".macro_status_text.endsWith('(1)')").toBool());
    QVERIFY(!evaluate(ui + ".execute_command(" + quoted(action + " invalid") + ")").toBool());
    QVERIFY(evaluate(ui + ".macro_status_text.endsWith('(1)')").toBool());
    QVERIFY(evaluate(ui + ".execute_command('macro-stop')").toBool());
    const auto before = action == "zoom" ? evaluate(pageReceiver + ".zoomFactor").toDouble()
        : page("window.scrollY").toDouble();
    sendOwnedBrowserKey(host, Qt::Key_Period);
    if (action == "zoom") QTRY_VERIFY(evaluate(pageReceiver + ".zoomFactor").toDouble() > before + 0.05);
    else QTRY_VERIFY(page("window.scrollY").toDouble() > before);
    QVERIFY(evaluate(ui + ".execute_command('macro-play a')").toBool());
    evaluate("testWindow.completeBrowserKeyAction(" + ui + "," + owner + ")");
    if (action == "zoom") QTRY_VERIFY(evaluate(pageReceiver + ".zoomFactor").toDouble() > 1.3);
    else QTRY_VERIFY(page("window.scrollY").toDouble() > before * 1.5);
}

void ApplicationTests::repeatKeyboardCount_data() {
    QTest::addColumn<QString>("hostKind");
    QTest::addColumn<int>("count");
    for (const auto &host : {QString("primary"), QString("secondary")})
        for (const auto count : {2, 101})
            QTest::newRow(qPrintable(host + "-" + QString::number(count))) << host << count;
}

void ApplicationTests::repeatKeyboardCount() {
    QFETCH(QString, hostKind);
    QFETCH(int, count);
    const int acceptedCount = qMin(count, 100);
    QString owner, ui;
    QQuickWindow *host = nullptr;
    preparePageSearchHost(hostKind, owner, ui, host);
    QVERIFY(host);
    const auto initial = page("location.href").toString();
    for (int index = 1; index < 5; ++index) {
        QVERIFY(evaluate(ui + ".execute_command(" + quoted("tab-open --background " + initial
            + "?tab=" + QString::number(index)) + ")").toBool());
        evaluate("testWindow.completeBrowserKeyAction(" + ui + "," + owner + ")");
    }
    QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), 5);
    QVERIFY(evaluate(ui + ".execute_command('bind x tab-next')").toBool());
    QTRY_VERIFY(evaluate(ui + ".status_text.startsWith('Binding saved')").toBool());
    QVERIFY(evaluate(ui + ".execute_command('macro-record a')").toBool());
    for (const auto digit : QString::number(count))
        sendOwnedBrowserKey(host, Qt::Key_0 + digit.digitValue());
    sendOwnedBrowserKey(host, Qt::Key_X);
    QTRY_COMPARE(evaluate(ui + ".active_tab_index").toInt(), acceptedCount % 5);
    QVERIFY(evaluate(ui + ".macro_status_text.endsWith('(1)')").toBool());
    QVERIFY(evaluate(ui + ".execute_command('macro-stop')").toBool());
    sendOwnedBrowserKey(host, Qt::Key_Period);
    QTRY_COMPARE(evaluate(ui + ".active_tab_index").toInt(), (acceptedCount * 2) % 5);
    QVERIFY(evaluate(ui + ".execute_command('macro-play a')").toBool());
    evaluate("testWindow.completeBrowserKeyAction(" + ui + "," + owner + ")");
    QTRY_COMPARE(evaluate(ui + ".active_tab_index").toInt(), (acceptedCount * 3) % 5);
}

void ApplicationTests::repeatWindowRouting() {
    const auto initial = text("current_url");
    QVERIFY(command("macro-record w"));
    QVERIFY(command("open --target window --profile repeat-work " + initial + "?repeat-window=yes"));
    evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
    QVERIFY(text("macro_status_text").endsWith("(1)"));
    QVERIFY(command("macro-stop"));
    for (const auto &replay : {QString("repeat"), QString("macro-play w")}) {
        QVERIFY(command(replay));
        evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
    }
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 4);
    QCOMPARE(evaluate("testWindow.browserUi.tab_count").toInt(), 1);
    for (int index = 1; index < 4; ++index) {
        const auto entry = "testWindow.browserWindowRegistry[" + QString::number(index) + "]";
        QTRY_VERIFY(evaluate(entry + ".host.profileBootstrapComplete && !" + entry + ".ui.profile_bootstrap_pending").toBool());
        QCOMPARE(evaluate(entry + ".profileName").toString(), QString("repeat-work"));
        pageReceiver = entry + ".host.activeWebView()";
        QTRY_VERIFY(evaluate("!!" + pageReceiver).toBool());
        QTRY_COMPARE(page("location.href").toString(), initial + "?repeat-window=yes");
    }
}
