#include "tst_application.h"
#include "ferric-browser-engine-qt/src/bridge.cxxqt.h"
#include "owned_browser_keyboard.h"
#include <QtTest>
#include <QScopeGuard>
#include <QSignalSpy>
#include <QQuickItem>
#include <QJsonDocument>
#include <QJsonObject>

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
    QTRY_VERIFY(evaluate("testWindow.browserWindowRegistry.every(function(entry){return entry.host.profileBootstrapComplete && !entry.ui.profile_bootstrap_pending})").toBool());
    QVERIFY(text("macro_status_text").endsWith("(1)"));
    QVERIFY(command("macro-stop"));
    int expectedWindows = 2;
    for (const auto &replay : {QString("repeat"), QString("macro-play w")}) {
        QVERIFY(command(replay));
        evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
        ++expectedWindows;
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), expectedWindows);
        QTRY_VERIFY(evaluate("testWindow.browserWindowRegistry.every(function(entry){return entry.host.profileBootstrapComplete && !entry.ui.profile_bootstrap_pending})").toBool());
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

// Add beside the existing application repeat fixtures. These are constructed
// Qt QWindow events through the production router/QML/Rust path; they do not
// qualify physical seat or compositor delivery.
void ApplicationTests::unavailableBindingsKeepBrowserOwnership() {
    // Exercise a real supported resolver-clear lifecycle without accepting an
    // invalid profile just to create missing bindings.
    QTemporaryDir isolated;
    QVERIFY(isolated.isValid());
    BrowserUi owner;
    owner.configure_profile(true, false, QStringLiteral("Released input fixture"),
        QStringLiteral("released-input"), isolated.path());
    const auto ready = [&] { owner.poll_config(); return !owner.property("profile_bootstrap_pending").toBool(); };
    QTRY_VERIFY_WITH_TIMEOUT(ready(), 10000);
    QVERIFY(owner.release_transient_resources());
    QVERIFY(!owner.key_has_binding(QStringLiteral("j")));
    QCOMPARE(owner.property("mode").toString(), QStringLiteral("normal"));
    const auto status = owner.property("status_text");
    const auto overlay = owner.property("binding_overlay");
    const auto macroStatus = owner.property("macro_status_text");
    for (const auto &key : {QStringLiteral("q"), QStringLiteral("a"), QStringLiteral("z")}) {
        QVERIFY(owner.handle_key(key));
        QCOMPARE(owner.property("status_text"), status);
        QCOMPARE(owner.property("binding_overlay"), overlay);
        QCOMPARE(owner.property("macro_status_text"), macroStatus);
    }
}

void ApplicationTests::invalidProfileBindingsUseValidatedFallback() {
    auto *view = qobject_cast<QQuickItem *>(evaluate("testWindow.activeWebView()").value<QObject *>());
    QVERIFY(view);
    QVERIFY(evaluate("testWindow.browserUi.key_has_binding('j')").toBool());
    QVERIFY(evaluate("!JSON.stringify(JSON.parse(testWindow.browserUi.config_json)).includes('unknown-fixture-command')").toBool());
    QVERIFY(evaluate("testWindow.browserUi.profile_startup_details()").toString().contains("bindings", Qt::CaseInsensitive));
    QVERIFY(page("window.keys=[];document.getElementById('button').focus();true").toBool());
    view->forceActiveFocus();
    QTRY_COMPARE(text("mode"), QStringLiteral("normal"));
    key(Qt::Key_Z);
    QVERIFY(command("mode-enter pass-through"));
    QTRY_COMPARE(text("mode"), QStringLiteral("pass-through"));
    key(Qt::Key_X);
    QTRY_VERIFY(page("window.keys.includes('x')").toBool());
    QCOMPARE(page("window.keys.filter(k=>k==='z').length").toInt(), 0);
    key(Qt::Key_Escape, Qt::ControlModifier);
    QTRY_COMPARE(text("mode"), QStringLiteral("normal"));
}

void ApplicationTests::heldBrowserRepeatKeepsCommandAuthority_data() {
    QTest::addColumn<QString>("hostKind");
    QTest::addColumn<QString>("publication");
    for (const auto &host : {QString("primary"), QString("secondary")})
        for (const auto &publication : {QString("self"), QString("peer"), QString("peer-reentrant")})
            QTest::newRow(qPrintable(host + "-" + publication)) << host << publication;
}

void ApplicationTests::heldBrowserRepeatKeepsCommandAuthority() {
    QFETCH(QString, hostKind);
    QFETCH(QString, publication);
    QString owner, ui;
    QQuickWindow *host = nullptr;
    preparePageSearchHost(hostKind, owner, ui, host);
    QVERIFY(host);
    const auto initial = page("location.href").toString();
    QVERIFY(evaluate(ui + ".execute_command(" + quoted("tab-open --background " + initial + "?held-spare") + ")").toBool());
    QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), 2);
    QVERIFY(evaluate(ui + ".execute_command('bind z scroll down')").toBool());
    QTRY_VERIFY(evaluate(ui + ".status_text.startsWith('Binding saved')").toBool());
    QString publisherUi = ui;
    if (publication != "self") {
        // Create from the actual recipient. The generic secondary-host helper
        // deliberately uses a different profile/privacy than the primary.
        const auto count = evaluate("testWindow.browserWindowRegistry.length").toInt();
        QVERIFY(evaluate(ui + ".execute_command('open --target window rb://blank')").toBool());
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), count + 1);
        const auto publisher = QString("testWindow.browserWindowRegistry[%1]").arg(count);
        publisherUi = publisher + ".ui";
        QTRY_VERIFY(evaluate(publisher + ".host.profileBootstrapComplete").toBool());
        QTRY_VERIFY(!evaluate(publisherUi + ".profile_bootstrap_pending").toBool());
    }
    page("document.body.style.height='12000px';window.keys=[];window.scrollTo(0,0)");
    host->requestActivate();
    QTRY_VERIFY(host->isActive());
    evaluate(pageReceiver + ".forceActiveFocus()");
    auto *router = evaluate(owner + ".browserKeyRouterObject").value<QObject *>();
    QVERIFY(router);
    QSignalSpy releases(router, SIGNAL(keyReleased(int,int)));
    QVERIFY(releases.isValid());
    bool held = false;
    QPointer<QQuickWindow> guardedHost(host);
    const auto releaseHeld = [&] {
        if (!held || !guardedHost) return;
        held = false;
        QKeyEvent release(QEvent::KeyRelease, Qt::Key_Z, Qt::NoModifier, "z");
        release.setAccepted(false);
        QCoreApplication::sendEvent(guardedHost.data(), &release);
        QVERIFY(release.isAccepted());
    };
    const auto cleanupHeld = qScopeGuard(releaseHeld);
    QKeyEvent press(QEvent::KeyPress, Qt::Key_Z, Qt::NoModifier, "z", false, 1);
    press.setAccepted(false);
    held = true;
    QCoreApplication::sendEvent(host, &press);
    QVERIFY(press.isAccepted());
    QTRY_VERIFY(page("window.scrollY").toDouble() > 0);
    const auto firstScroll = page("window.scrollY").toDouble();
    QKeyEvent repeatBefore(QEvent::KeyPress, Qt::Key_Z, Qt::NoModifier, "z", true, 1);
    repeatBefore.setAccepted(false);
    QCoreApplication::sendEvent(host, &repeatBefore);
    QVERIFY(repeatBefore.isAccepted());
    QTRY_VERIFY(page("window.scrollY").toDouble() > firstScroll);
    const auto inputEpoch = evaluate(ui + ".input_epoch");
    auto *recipient = qobject_cast<BrowserUi *>(evaluate(ui).value<QObject *>());
    QVERIFY(recipient);
    const auto bindingContext = recipient->binding_context();
    const auto publishedCommand = [&] {
        return QJsonDocument::fromJson(recipient->property("config_json").toString().toUtf8())
            .object()["bindings"].toObject()["normal"].toObject()["z"].toString();
    };
    bool reentered = false, reentrantAccepted = false;
    int reentrantBefore = -1, reentrantAfter = -1;
    const auto notification = connect(recipient, &BrowserUi::config_jsonChanged, recipient, [&] {
        if (publication != "peer-reentrant" || reentered || recipient->binding_context() == bindingContext
                || publishedCommand() != "tab-close")
            return;
        reentered = true;
        reentrantBefore = recipient->property("tab_count").toInt();
        QKeyEvent repeat(QEvent::KeyPress, Qt::Key_Z, Qt::NoModifier, "z", true, 1);
        repeat.setAccepted(false);
        QCoreApplication::sendEvent(host, &repeat);
        reentrantAccepted = repeat.isAccepted();
        reentrantAfter = recipient->property("tab_count").toInt();
    }, Qt::DirectConnection);
    const auto disconnectNotification = qScopeGuard([&] { disconnect(notification); });
    QVERIFY(evaluate(publisherUi + ".execute_command('bind z tab-close')").toBool());
    QTRY_VERIFY(evaluate(publisherUi + ".status_text.startsWith('Binding saved')").toBool());
    QTRY_VERIFY(recipient->binding_context() != bindingContext);
    QTRY_COMPARE(publishedCommand(), QString("tab-close"));
    if (publication == "peer-reentrant") {
        QVERIFY(reentered);
        QVERIFY(reentrantAccepted);
        QCOMPARE(reentrantBefore, 2);
        QCOMPARE(reentrantAfter, 2);
    }
    QCOMPARE(evaluate(ui + ".input_epoch"), inputEpoch);
    QCOMPARE(evaluate(ui + ".tab_count").toInt(), 2);
    QKeyEvent repeatAfter(QEvent::KeyPress, Qt::Key_Z, Qt::NoModifier, "z", true, 1);
    repeatAfter.setAccepted(false);
    QCoreApplication::sendEvent(host, &repeatAfter);
    QVERIFY(repeatAfter.isAccepted());
    QCOMPARE(evaluate(ui + ".tab_count").toInt(), 2);
    QCOMPARE(page("window.keys.filter(k=>k==='z').length").toInt(), 0);
    QCOMPARE(releases.count(), 0);
    releaseHeld();
    QCOMPARE(releases.count(), 1);
    QTest::keyClick(host, Qt::Key_Z);
    QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), 1);
    QCOMPARE(releases.count(), 2);
}
