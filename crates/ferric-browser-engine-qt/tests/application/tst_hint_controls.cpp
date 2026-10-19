#include "tst_application.h"
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QQuickWindow>
#include <QtTest>

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
