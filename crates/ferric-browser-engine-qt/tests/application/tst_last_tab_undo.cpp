#include "tst_application.h"
#include "owned_browser_keyboard.h"
#include <QPointer>
#include <QQuickItem>
#include <QtTest>

void ApplicationTests::productPolishLastTabUndo_data() {
    QTest::addColumn<QString>("hostKind");
    QTest::addColumn<QString>("intervention");
    QTest::addColumn<bool>("modifiedShortcut");
    const auto add = [](const QString &host, const QString &intervention, bool modified) {
        const auto tag = host + "-" + intervention + (modified ? "-ctrl-shift-t" : "-u");
        QTest::newRow(qPrintable(tag)) << host << intervention << modified;
    };
    for (const auto &host : {QString("primary"), QString("secondary"), QString("private"),
                             QString("temporary"), QString("popup")})
        for (const bool modified : {false, true}) add(host, "untouched", modified);
    for (const auto &host : {QString("primary"), QString("secondary"), QString("private")}) {
        for (const auto &intervention : {QString("incoming"), QString("navigated-back"),
                                        QString("new-native"), QString("settings")})
            for (const bool modified : {false, true}) add(host, intervention, modified);
        add(host, "draft", false);
    }
    for (const bool modified : {false, true}) add("primary", "new-tab-then-close", modified);
}

void ApplicationTests::productPolishLastTabUndo() {
    QFETCH(QString, hostKind);
    QFETCH(QString, intervention);
    QFETCH(bool, modifiedShortcut);
    QString owner, ui;
    QQuickWindow *host = nullptr;
    preparePageSearchHost(hostKind == "temporary" ? "primary" : hostKind, owner, ui, host);
    QVERIFY(host);
    if (hostKind == "temporary") QVERIFY(evaluate(owner + ".ephemeralProfile").toBool());
    const auto drain = [&] {
        evaluate(owner + ((hostKind == "primary" || hostKind == "temporary")
            ? ".executePendingEngineAction()" : ".applySwitcherEngineAction()"));
    };
    const auto run = [&](const QString &value) {
        const auto accepted = evaluate(ui + ".execute_command(" + quoted(value) + ")").toBool();
        drain();
        return accepted;
    };
    const auto originalUrl = page("location.href").toString();
    const auto originalId = evaluate(ui + ".tab_id_for_index(0)").toString();
    const bool originallyPinned = modifiedShortcut;
    if (originallyPinned) QCOMPARE(evaluate(ui + ".set_tab_pinned(0,true)").toInt(), 0);
    QVERIFY(evaluate(ui + ".set_tab_muted(0,true)").toBool());
    QVERIFY(run("zoom 1.25"));
    QTRY_COMPARE(qRound(evaluate(pageReceiver + ".zoomFactor").toDouble() * 100), 125);
    if (originallyPinned) QVERIFY(run("tab-close --id " + originalId));
    else sendOwnedBrowserKey(host, Qt::Key_D);
    QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), 1);
    QTRY_COMPARE(evaluate(ui + ".current_url").toString(), QString("rb://start"));
    QTRY_VERIFY(evaluate("!!" + pageReceiver + " && " + pageReceiver + ".nativeUrl === 'rb://start'").toBool());
    const auto fallbackId = evaluate(ui + ".tab_id_for_index(0)").toString();
    QVERIFY(fallbackId != originalId);
    QPointer<QObject> fallbackView = evaluate(pageReceiver).value<QObject *>();
    QVERIFY(fallbackView);
    frameFixtures["/undo-current.html"] = "<!doctype html><title>Keep current work</title><input id='draft'>";
    const auto currentUrl = QUrl(originalUrl).resolved(QUrl("/undo-current.html")).toString();
    QPointer<QObject> draftField;
    int expectedCount = 1;
    if (intervention == "incoming" || intervention == "navigated-back") {
        QVERIFY(run("open " + currentUrl));
        QTRY_VERIFY(evaluate("!!" + pageReceiver).toBool());
        QTRY_COMPARE(page("location.href").toString(), currentUrl);
        QTRY_COMPARE(evaluate(ui + ".load_state").toString(), QString("complete"));
        if (intervention == "incoming")
            page("document.getElementById('draft').value='keep this draft';true");
        else {
            QVERIFY(run("open rb://start"));
            QTRY_COMPARE(evaluate(ui + ".current_url").toString(), QString("rb://start"));
            QTRY_VERIFY(evaluate(pageReceiver + ".nativeUrl === 'rb://start'").toBool());
        }
        expectedCount = 2;
    } else if (intervention == "new-native") {
        QVERIFY(run("tab-open rb://blank"));
        QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), 2);
        QTRY_COMPARE(evaluate(ui + ".current_url").toString(), QString("rb://blank"));
        expectedCount = 3;
    } else if (intervention == "settings") {
        QVERIFY(evaluate(ui + ".set_tab_muted(0,true)").toBool());
        QVERIFY(run("zoom 1.5"));
        expectedCount = 2;
    } else if (intervention == "draft") {
        QTRY_VERIFY(evaluate("!!" + pageReceiver + ".startPage && " + pageReceiver + ".startPage.pageReady").toBool());
        evaluate(pageReceiver + ".startPage.editSlot(1,'pin',null)");
        draftField = fallbackView->findChild<QObject *>("favouriteUrlInput");
        QVERIFY(draftField);
        QVERIFY(draftField->setProperty("text", "https://draft.example.test/unfinished"));
        QTRY_VERIFY(evaluate(pageReceiver + ".startPage.hasFavouriteDraft").toBool());
        expectedCount = 2;
    } else if (intervention == "new-tab-then-close") {
        QVERIFY(run("tab-open " + currentUrl));
        QTRY_VERIFY(evaluate("!!" + pageReceiver).toBool());
        QTRY_COMPARE(page("location.href").toString(), currentUrl);
        QTRY_COMPARE(evaluate(ui + ".load_state").toString(), QString("complete"));
        sendOwnedBrowserKey(host, Qt::Key_D);
        QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), 1);
        QTRY_COMPARE(evaluate(ui + ".current_url").toString(), QString("rb://start"));
    }
    // Native editing owns ordinary keys. An independently delivered command
    // must also preserve its unfinished draft when reopening a closed page.
    if (intervention == "draft") QVERIFY(run("tab-undo"));
    else if (modifiedShortcut)
        sendOwnedBrowserKey(host, Qt::Key_T, Qt::ControlModifier | Qt::ShiftModifier);
    else sendOwnedBrowserKey(host, Qt::Key_U);
    QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), expectedCount);
    QTRY_VERIFY(evaluate("!!" + pageReceiver).toBool());
    const auto restoredUrl = intervention == "new-tab-then-close" ? currentUrl : originalUrl;
    QTRY_COMPARE(page("location.href").toString(), restoredUrl);
    QTRY_COMPARE(evaluate(ui + ".load_state").toString(), QString("complete"));
    const auto restoredId = evaluate(ui + ".tab_id_for_index(" + ui + ".active_tab_index)").toString();
    QCOMPARE(restoredId == fallbackId, expectedCount == 1);
    if (expectedCount == 1) {
        const auto history = evaluate("JSON.parse(" + ui + ".content_history_export(" + ui + ".active_tab_index)).entries").toList();
        for (const auto &entry : history)
            QVERIFY(entry.toMap().value("url").toString() != "rb://start");
    }
    QVERIFY(fallbackView);
    if (expectedCount > 1) {
        const auto fallbackIndex = evaluate(ui + ".tab_index_for_id(" + quoted(fallbackId) + ")").toInt();
        QVERIFY(fallbackIndex >= 0);
        QCOMPARE(evaluate(owner + ".tabViewAt(" + QString::number(fallbackIndex) + ")").value<QObject *>(), fallbackView.data());
        if (intervention == "incoming") {
            const auto activeReceiver = pageReceiver;
            pageReceiver = owner + ".tabViewAt(" + QString::number(fallbackIndex) + ")";
            QCOMPARE(page("document.getElementById('draft').value").toString(), QString("keep this draft"));
            pageReceiver = activeReceiver;
        } else if (intervention == "draft") {
            QVERIFY(draftField);
            QCOMPARE(draftField->property("text").toString(), QString("https://draft.example.test/unfinished"));
        } else if (intervention == "settings") {
            const auto preserved = evaluate("JSON.parse(" + ui + ".tab_transfer_payload(" + quoted(fallbackId) + "))").toMap();
            QCOMPARE(preserved.value("muted").toBool(), true);
            QCOMPARE(preserved.value("zoom_hundredths").toInt(), 150);
        }
    }
    if (intervention == "new-tab-then-close") {
        sendOwnedBrowserKey(host, Qt::Key_U);
        QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), 2);
        QTRY_VERIFY(evaluate("!!" + pageReceiver).toBool());
        QTRY_COMPARE(page("location.href").toString(), originalUrl);
        QTRY_COMPARE(evaluate(ui + ".load_state").toString(), QString("complete"));
    }
    const auto settings = evaluate("JSON.parse(" + ui + ".tab_transfer_payload(" + ui + ".tab_id_for_index(" + ui + ".active_tab_index)))").toMap();
    QCOMPARE(settings.value("pinned").toBool(), originallyPinned);
    QCOMPARE(settings.value("muted").toBool(), true);
    QCOMPARE(settings.value("zoom_hundredths").toInt(), 125);
    QCOMPARE(evaluate(pageReceiver + ".audioMuted").toBool(), true);
    QCOMPARE(qRound(evaluate(pageReceiver + ".zoomFactor").toDouble() * 100), 125);
    if (intervention != "draft") {
        auto view = qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());
        QVERIFY(view);
        QTRY_VERIFY(host->activeFocusItem() && (host->activeFocusItem() == view || view->isAncestorOf(host->activeFocusItem())));
        QTRY_VERIFY(page("document.hasFocus()").toBool());
    }
}
