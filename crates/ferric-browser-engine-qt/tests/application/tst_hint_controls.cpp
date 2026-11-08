#include "tst_application.h"
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QKeyEvent>
#include <QQuickWindow>
#include <QtTest>

void ApplicationTests::hintEarlyInput_data() {
    QTest::addColumn<bool>("secondary");
    QTest::addColumn<QString>("journey");
    for (const bool secondary : {false, true})
        for (const auto journey : {"label-enter", "pending-collection", "query-edits", "escape", "navigation", "view-change", "focus-loss", "shutdown", "auto-follow", "initial-auto", "pointer", "overflow", "refresh"}) {
            const auto tag = QString("%1-%2").arg(secondary ? "secondary" : "primary", journey).toUtf8();
            QTest::newRow(tag.constData()) << secondary << QString(journey);
        }
}

void ApplicationTests::hintEarlyInput() {
    QFETCH(bool, secondary);
    QFETCH(QString, journey);
    QTest::failOnWarning(QRegularExpression(".*(invalid context|TypeError|ReferenceError|Binding loop).*"));
    QString owner = "testWindow";
    QString ui = owner + ".browserUi";
    auto *host = window;
    if (secondary) {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"
            + quoted(text("current_url")) + ",windowProfileName:'early-hints',windowProfileLabel:'Early hints'})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
        owner = "testWindow.browserWindowRegistry[1].host";
        ui = "testWindow.browserWindowRegistry[1].ui";
        QTRY_VERIFY(evaluate(owner + ".profileBootstrapComplete && !" + ui + ".profile_bootstrap_pending").toBool());
        host = qobject_cast<QQuickWindow *>(evaluate(owner).value<QObject *>());
        QVERIFY(host);
        pageReceiver = owner + ".activeView";
        QTRY_VERIFY(evaluate(pageReceiver + ".ownsTab() && !" + pageReceiver + ".loading").toBool());
        QTRY_VERIFY(page("!!document.getElementById('button')").toBool());
    }
    host->requestActivate();
    QTRY_VERIFY(host->isActive());
    evaluate(pageReceiver + ".forceActiveFocus()");
    const auto workspace = owner + ".hintWorkspace";
    QVERIFY(evaluate(ui + ".execute_command('set --temp hints.min_chars=2')").toBool());
    QVERIFY(evaluate(ui + ".execute_command('set --temp hints.auto_follow=never')").toBool());
    if (journey == "view-change") {
        QVERIFY(evaluate(ui + ".execute_command('tab-open --background rb://start')").toBool());
        evaluate(owner + ".syncTabModel()");
        QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), 2);
    }
    if (journey == "initial-auto")
        page("document.body.replaceChildren(document.getElementById('button'));true");
    QVERIFY(evaluate(ui + ".execute_command(" + quoted(journey == "pointer" ? "hint all" : "hint") + ")").toBool());
    const auto label = hintLabel("Activate exact button", ui);
    QCOMPARE(label.size(), 2);
    QTest::keyClick(host, Qt::Key_Escape);
    QCOMPARE(evaluate(ui + ".mode").toString(), "normal");

    if (journey == "auto-follow")
        QVERIFY(evaluate(ui + ".execute_command('set --temp hints.auto_follow=full-match')").toBool());
    if (journey == "initial-auto")
        QVERIFY(evaluate(ui + ".execute_command('set --temp hints.auto_follow=always')").toBool());

    // Synchronous delivery uses the production window's key router without
    // pumping collection callbacks between gestures. Assert the missing Rust
    // session explicitly, rather than depending on a slow renderer or sleeps.
    auto press = [&](int key, const QString &text = {}, Qt::KeyboardModifiers modifiers = Qt::NoModifier) {
        QKeyEvent down(QEvent::KeyPress, key, modifiers, text);
        QCoreApplication::sendEvent(host, &down);
        QKeyEvent up(QEvent::KeyRelease, key, modifiers, text);
        QCoreApplication::sendEvent(host, &up);
    };
    auto type = [&](const QString &text) {
        for (const auto character : text)
            press(character.isLetter() ? Qt::Key_A + character.toLower().unicode() - 'a'
                  : character == '/' ? Qt::Key_Slash : Qt::Key_Space, QString(character));
    };
    if (journey == "pointer") {
        // Use the supported local UI entry so binding-prefix timing cannot
        // start collection before the early pointer inputs below are sent.
        QVERIFY(evaluate(ui + ".execute_command('hint --target pointer all')").toBool());
        QVERIFY(evaluate(ui + ".hint_pointer_mode").toBool());
    } else {
        press(Qt::Key_F, "f");
    }
    QCOMPARE(evaluate(ui + ".mode").toString(), "hint");
    if (journey == "pending-collection") {
        evaluate(workspace + ".startHintCollection()");
        QVERIFY(evaluate(workspace + ".hintRefreshPending").toBool());
    }
    QCOMPARE(evaluate("JSON.parse(" + ui + ".update_hint_interaction('clear','')).error").toString(), "no-session");

    if (journey == "initial-auto") {
        type("/activate exact");
    } else if (journey == "query-edits") {
        type("/bad");
        press(Qt::Key_U, "u", Qt::ControlModifier);
        type("activate / ");
        press(Qt::Key_Backspace);
        press(Qt::Key_Backspace);
        type("exact");
    } else if (journey == "overflow") {
        for (int i = 0; i < 257; ++i) type("a");
    } else {
        type(label.left(1));
        press(Qt::Key_Backspace);
        type(label);
        if (journey != "refresh") {
            press(Qt::Key_Return, "\r");
            press(Qt::Key_Return, "\r");
        }
    }
    if (journey == "overflow") {
        QCOMPARE(evaluate(ui + ".mode").toString(), "normal");
        QCOMPARE(evaluate(workspace + ".pendingHintInputs.length").toInt(), 0);
        QTest::qWait(150);
        QCOMPARE(page("window.clicks").toInt(), 0);
        return;
    }
    QVERIFY(evaluate(workspace + ".pendingHintInputs.length").toInt() > 0);
    if (journey == "shutdown") {
        evaluate("testWindow.applicationShutdownInProgress=true");
        QCOMPARE(evaluate(workspace + ".pendingHintInputs.length").toInt(), 0);
        evaluate("testWindow.applicationShutdownInProgress=false");
        QTRY_VERIFY(evaluate(ui + ".hint_visible && !" + workspace + ".hintRefreshPending").toBool());
        QCOMPARE(page("window.clicks").toInt(), 0);
        return;
    }
    if (journey == "focus-loss") {
        // Hold the scheduled initial collection at its existing pending gate
        // so activation timing cannot turn this into a renderer-speed race.
        evaluate(workspace + ".hintRefreshPending=true");
        QQuickWindow other;
        other.resize(320, 240);
        other.show();
        other.requestActivate();
        QTRY_VERIFY(other.isActive());
        QVERIFY(!host->isActive());
        QCOMPARE(evaluate(workspace + ".pendingHintInputs.length").toInt(), 0);
        evaluate(workspace + ".hintRefreshPending=false;" + workspace + ".startHintCollection()");
        QTRY_VERIFY(!evaluate(workspace + ".hintRefreshPending").toBool());
        QCOMPARE(page("window.clicks").toInt(), 0);
        host->requestActivate();
        QTRY_VERIFY(host->isActive());
        QTest::qWait(150);
        QCOMPARE(page("window.clicks").toInt(), 0);
        return;
    }
    if (journey == "navigation" || journey == "view-change") {
        if (journey == "navigation") {
            const auto destination = evaluate(ui + ".current_url").toString() + "?early-input-cancelled=1";
            evaluate(pageReceiver + ".url=" + quoted(destination));
            QTRY_VERIFY(page("location.href===" + quoted(destination) + " && document.readyState==='complete'").toBool());
            QCOMPARE(page("window.clicks").toInt(), 0);
        } else {
            QVERIFY(evaluate(ui + ".select_tab(1)").toBool());
            evaluate(owner + ".syncTabModel()");
        }
        QTRY_COMPARE(evaluate(ui + ".mode").toString(), "normal");
        QCOMPARE(evaluate(workspace + ".pendingHintInputs.length").toInt(), 0);
        QTest::qWait(150);
        QVERIFY(!evaluate(ui + ".hint_visible").toBool());
        return;
    }
    if (journey == "escape") {
        press(Qt::Key_Escape);
        QCOMPARE(evaluate(workspace + ".pendingHintInputs.length").toInt(), 0);
        press(Qt::Key_F, "f");
    }
    if (journey == "label-enter" || journey == "pending-collection" || journey == "auto-follow") {
        QTRY_COMPARE(page("window.clicks").toInt(), 1);
        QTest::qWait(150);
        QCOMPARE(page("window.clicks").toInt(), 1);
        QCOMPARE(evaluate(ui + ".mode").toString(), "normal");
    } else {
        QTRY_VERIFY(evaluate(ui + ".hint_visible && !" + workspace + ".hintRefreshPending").toBool());
        QCOMPARE(page("window.clicks").toInt(), 0);
        if (journey == "query-edits" || journey == "initial-auto") {
            QCOMPARE(evaluate(workspace + ".hintState.mode").toString(), "text");
            QCOMPARE(evaluate(workspace + ".hintState.query").toString(), "activate exact");
            QTest::keyClick(host, Qt::Key_Return);
            QTRY_COMPARE(page("window.clicks").toInt(), 1);
        } else if (journey == "escape") {
            QCOMPARE(evaluate(workspace + ".hintState.prefix").toString(), "");
        } else if (journey == "pointer") {
            QCOMPARE(evaluate(workspace + ".hintState.prefix").toString(), label);
            QCOMPARE(evaluate(ui + ".hint_pointer_state").toString(), "selecting");
            QTest::keyClick(host, Qt::Key_Return);
            QTRY_COMPARE(page("window.clicks").toInt(), 1);
            QVERIFY(page("window.trust").toBool());
        } else if (journey == "refresh") {
            QCOMPARE(evaluate(workspace + ".hintState.prefix").toString(), label);
            evaluate(workspace + ".startHintCollection()");
            QVERIFY(evaluate(workspace + ".hintRefreshPending").toBool());
            press(Qt::Key_Backspace);
            QCOMPARE(evaluate(workspace + ".hintState.prefix").toString(), label.left(1));
            QTRY_VERIFY(!evaluate(workspace + ".hintRefreshPending").toBool());
            QCOMPARE(evaluate(workspace + ".hintState.prefix").toString(), label.left(1));
        }
    }
    QCOMPARE(evaluate(workspace + ".pendingHintInputs.length").toInt(), 0);
    if (secondary) QCOMPARE(text("mode"), "normal");
}

void ApplicationTests::hintNativeControls_data() {
    QTest::addColumn<bool>("secondary");
    QTest::addColumn<double>("zoom");
    QTest::newRow("primary-80") << false << 0.8;
    QTest::newRow("primary-100") << false << 1.0;
    QTest::newRow("primary-125") << false << 1.25;
    QTest::newRow("secondary-100") << true << 1.0;
}

void ApplicationTests::hintNativeControls() {
    QFETCH(bool, secondary);
    QFETCH(double, zoom);
    QTest::failOnWarning(QRegularExpression(".*(invalid context|TypeError|ReferenceError|Binding loop).*"));
    QFile fixture(QFINDTESTDATA("../fixtures/webapp-interaction/native-hint-controls.html"));
    QVERIFY(fixture.open(QIODevice::ReadOnly));
    frameFixtures.insert("/native-hint-controls.html", fixture.readAll());
    const auto primaryUrl = text("current_url");
    QString owner = "testWindow";
    QString ui = "testWindow.browserUi";
    auto ownerWindow = window;
    if (secondary) {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"
            + quoted(primaryUrl) + ",windowProfileName:'hint-controls',windowProfileLabel:'Hint controls'})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
        owner = "testWindow.browserWindowRegistry[1].host";
        ui = "testWindow.browserWindowRegistry[1].ui";
        QTRY_VERIFY(evaluate(owner + ".profileBootstrapComplete && !" + ui + ".profile_bootstrap_pending").toBool());
        ownerWindow = qobject_cast<QQuickWindow *>(evaluate(owner).value<QObject *>());
        QVERIFY(ownerWindow);
        pageReceiver = owner + ".activeView";
        QTRY_VERIFY(evaluate(pageReceiver + ".ownsTab() && !" + pageReceiver
            + ".loading && " + pageReceiver + ".mainFrame.isValid && String("
            + pageReceiver + ".url)===" + quoted(primaryUrl)).toBool());
    }
    const auto url = QString("http://127.0.0.1:%1/native-hint-controls.html").arg(server.serverPort());
    evaluate(pageReceiver + ".url=" + quoted(url));
    QTRY_VERIFY(page("document.body.dataset.ready==='true' && !!window.controlEvents").toBool());
    evaluate(pageReceiver + ".zoomFactor=" + QString::number(zoom));
    QVERIFY(evaluate(ui + ".execute_command('set --temp hints.auto_follow=never')").toBool());
    ownerWindow->requestActivate();
    QTRY_VERIFY(ownerWindow->isActive());
    evaluate(pageReceiver + ".forceActiveFocus()");

    auto select = [&](const QString &name, const QString &action = "Activate", const QString &command = "hint actions") {
        if (command == "binding-f") QTest::keyClick(ownerWindow, Qt::Key_F);
        else if (!evaluate(ui + ".execute_command(" + quoted(command) + ")").toBool()) return false;
        const auto label = hintLabel(name, ui);
        if (label.isEmpty()) return false;
        const auto values = QJsonDocument::fromJson(evaluate(ui + ".hint_values").toString().toUtf8()).array();
        int matches = 0;
        for (const auto &value : values) {
            const auto row = value.toObject();
            if ((command != "hint focus" && row.value("text").toString() == "Thread container")
                || ((command == "hint actions" || command == "binding-f")
                    && row.value("kind").toString() == "focusable")) {
                qInfo() << "Redundant disclosure focus hint" << row;
                return false;
            }
            if (row.value("text").toString() == name) {
                if (row.value("action").toString() != action) {
                    qInfo() << "Unexpected hint action" << row << "expected" << action;
                    return false;
                }
                ++matches;
            }
        }
        if (matches != 1) return false;
        for (const auto character : label)
            QTest::keyClick(ownerWindow, Qt::Key(Qt::Key_A + character.toLower().unicode() - 'a'));
        QTest::keyClick(ownerWindow, Qt::Key_Return);
        return true;
    };

    for (int cycle = 0; cycle < 3; ++cycle) {
        QVERIFY(select("Expand or collapse thread", "Activate", "binding-f"));
        QTRY_COMPARE(page("document.getElementById('thread').open").toBool(), cycle % 2 == 0);
        QTRY_COMPARE(evaluate(ui + ".mode").toString(), "normal");
        QCOMPARE(page("window.controlEvents.clicks['thread-toggle']").toInt(), cycle + 1);
    }
    QVERIFY(select("Enable feature"));
    QTRY_VERIFY(page("document.getElementById('checkbox').checked").toBool());
    QTRY_COMPARE(evaluate(ui + ".mode").toString(), "normal");
    QCOMPARE(page("window.controlEvents.clicks.checkbox").toInt(), 1);
    QCOMPARE(page("window.controlEvents.changes.checkbox").toInt(), 1);
    QVERIFY(select("Second option"));
    QTRY_VERIFY(page("document.getElementById('radio-b').checked && !document.getElementById('radio-a').checked").toBool());
    QTRY_COMPARE(evaluate(ui + ".mode").toString(), "normal");
    QCOMPARE(page("window.controlEvents.changes['radio-b']").toInt(), 1);
    for (const auto &name : {"Run action", "Send form", "Image submit"}) {
        QVERIFY2(select(name), name);
        QTRY_COMPARE(evaluate(ui + ".mode").toString(), "normal");
    }
    QCOMPARE(page("window.controlEvents.clicks.button").toInt(), 1);
    QCOMPARE(page("window.controlEvents.submits").toInt(), 2);
    for (const auto &name : {"Quote", "React", "Thanks", "Add post"}) {
        QVERIFY2(select(name), name);
        QTRY_COMPARE(evaluate(ui + ".mode").toString(), "normal");
    }
    for (const auto id : {"quote", "react", "thanks", "add-post"})
        QCOMPARE(page("window.controlEvents.clicks[" + quoted(id) + "]").toInt(), 1);
    page("document.getElementById('text').value='modified';true");
    QVERIFY(select("Reset form"));
    QTRY_COMPARE(page("document.getElementById('text').value").toString(), "private-text-value");
    QTRY_COMPARE(evaluate(ui + ".mode").toString(), "normal");
    QCOMPARE(page("window.controlEvents.resets").toInt(), 1);
    // Explicit comprehensive hints retain keyboard regions; defaults exclude them.
    QVERIFY(evaluate(ui + ".execute_command('hint all')").toBool());
    QVERIFY(!hintLabel("Keyboard region", ui).isEmpty());
    QTest::keyClick(ownerWindow, Qt::Key_Escape);
    QTRY_COMPARE(evaluate(ui + ".mode").toString(), "normal");
    QVERIFY(evaluate(ui + ".execute_command('hint')").toBool());
    QTRY_VERIFY(!hintLabel("Expand or collapse thread", ui).isEmpty());
    QCOMPARE(evaluate(ui + ".hint_family").toString(), "actions");
    const auto defaults = QJsonDocument::fromJson(evaluate(ui + ".hint_values").toString().toUtf8()).array();
    for (const auto &value : defaults) QVERIFY(value.toObject().value("kind").toString() != "focusable");
    QTest::keyClick(ownerWindow, Qt::Key_Escape);
    QTRY_COMPARE(evaluate(ui + ".mode").toString(), "normal");
    // A retained focus hint must not focus content after its disclosure closes.
    QVERIFY(evaluate(ui + ".execute_command('hint focus')").toBool());
    const auto hiddenLabel = hintLabel("Thread keyboard region", ui);
    QVERIFY(!hiddenLabel.isEmpty());
    page("document.getElementById('reply-focus').addEventListener('focus',()=>document.body.dataset.hiddenFocused='true');document.getElementById('thread').open=false;true");
    for (const auto character : hiddenLabel)
        QTest::keyClick(ownerWindow, Qt::Key(Qt::Key_A + character.toLower().unicode() - 'a'));
    QTest::keyClick(ownerWindow, Qt::Key_Return);
    QTest::qWait(250);
    QVERIFY(evaluate(ui + ".mode").toString() != "insert");
    QVERIFY(!page("document.body.dataset.hiddenFocused==='true'").toBool());
    QTest::keyClick(ownerWindow, Qt::Key_Escape);
    QTRY_COMPARE(evaluate(ui + ".mode").toString(), "normal");
    // Check focus-only intent independently, before introducing editable focus
    // and its asynchronous blur notifications into the fixture.
    QVERIFY(select("Keyboard region", "Focus", "hint focus"));
    QTRY_COMPARE(evaluate(ui + ".mode").toString(), "insert");
    QCOMPARE(page("document.activeElement.id").toString(), "focus-only");
    QTest::keyClick(ownerWindow, Qt::Key_Escape);
    QTRY_COMPARE(evaluate(ui + ".mode").toString(), "normal");
    QVERIFY(select("Text entry", "Edit"));
    QTRY_COMPARE(evaluate(ui + ".mode").toString(), "insert");
    QCOMPARE(page("document.activeElement.id").toString(), "text");
    if (secondary) {
        QCOMPARE(text("mode"), "normal");
        QCOMPARE(text("current_url"), primaryUrl);
    }
}
