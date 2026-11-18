#include "tst_application.h"
#include <QQuickItem>
#include <QtTest>
#include <algorithm>

void ApplicationTests::bindingAliases_data()
{
    QTest::addColumn<QString>("authored");
    QTest::addColumn<QString>("unset");
    QTest::newRow("canonical-plus") << QStringLiteral("Plus") << QStringLiteral("+");
    QTest::newRow("literal-plus") << QStringLiteral("+") << QStringLiteral("Plus");
}

void ApplicationTests::bindingAliases()
{
    QFETCH(QString, authored);
    QFETCH(QString, unset);
    auto *view = qobject_cast<QQuickItem *>(evaluate("testWindow.activeWebView()").value<QObject *>());
    QVERIFY(view);
    QVERIFY(page("document.getElementById('button').focus();true").toBool());
    view->forceActiveFocus();
    evaluate("testWindow.browserUi.escape()");
    QTRY_COMPARE(text("mode"), QStringLiteral("normal"));
    QVERIFY(command("bind --mode normal " + authored + " zoom 1.25"));
    QVERIFY(evaluate("testWindow.browserUi.key_has_binding('+')").toBool());
    // Production KeyboardPresentation emits the produced literal '+'.
    key(Qt::Key_Plus);
    QTRY_COMPARE(qRound(evaluate("testWindow.activeWebView().zoomFactor").toDouble() * 100), 125);
    QVERIFY(evaluate("testWindow.browserUi.refresh_binding_help('zoom')").toBool());
    const auto sources = evaluate("testWindow.browserUi.binding_help_row_sources").toStringList();
    QVERIFY(std::any_of(sources.cbegin(), sources.cend(), [](const QString &source) {
        return source.contains("user");
    }));
    QVERIFY(command("unbind " + unset));
    QVERIFY(!evaluate("testWindow.browserUi.key_has_binding('+')").toBool());
    QVERIFY(!evaluate("testWindow.browserUi.key_has_binding('Plus')").toBool());
    QVERIFY(command("bind --mode insert " + authored + " mode-enter normal"));
    QVERIFY(command("mode-enter insert"));
    QTRY_COMPARE(text("mode"), QStringLiteral("insert"));
    QVERIFY(evaluate("testWindow.browserUi.key_has_binding('+')").toBool());
    // Insert retains native printable editing ownership even when a binding
    // exists in its catalogue; this phase does not add a new Insert route.
    QVERIFY(page("window.keys=[];document.getElementById('button').focus();true").toBool());
    view->forceActiveFocus();
    key(Qt::Key_Plus);
    QTRY_VERIFY(page("window.keys.includes('+')").toBool());
    QCOMPARE(text("mode"), QStringLiteral("insert"));
    QVERIFY(command("mode-enter normal"));
    QVERIFY(command("unbind --mode insert " + unset));
    QVERIFY(command("mode-enter insert"));
    QTRY_COMPARE(text("mode"), QStringLiteral("insert"));
    QVERIFY(!evaluate("testWindow.browserUi.key_has_binding('Plus')").toBool());
    QVERIFY(command("mode-enter normal"));
}

void ApplicationTests::bindingAliasGuide()
{
    const auto before = text("config_json");
    QVERIFY(!command("bind --mode mode-enter insert"));
    QVERIFY(text("status_text").contains("bind accepts"));
    QCOMPARE(text("config_json"), before);
    QVERIFY(!command("unbind --mode"));
    QVERIFY(text("status_text").contains("unbind accepts"));
    QCOMPARE(text("config_json"), before);
    QVERIFY(command("bind Plus,g zoom out"));
    evaluate("testWindow.browserUi.refresh_guide('zoom')");
    const auto commands = evaluate("testWindow.browserUi.guide_commands").toStringList();
    const auto notes = evaluate("testWindow.browserUi.guide_binding_notes").toStringList();
    const int shorter = commands.indexOf("zoom in");
    const int longer = commands.indexOf("zoom out");
    QVERIFY(shorter >= 0 && shorter < notes.size());
    QVERIFY(longer >= 0 && longer < notes.size());
    QVERIFY(notes[shorter].contains("Prefix conflict"));
    QVERIFY(notes[longer].contains("runtime layer"));
    QVERIFY(evaluate("testWindow.browserUi.refresh_binding_help('zoom')").toBool());
    QVERIFY(evaluate("testWindow.browserUi.binding_help_row_kinds").toStringList().contains("conflict"));
}
