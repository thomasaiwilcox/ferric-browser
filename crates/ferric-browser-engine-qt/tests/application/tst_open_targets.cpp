#include "tst_application.h"
#include "owned_browser_keyboard.h"
#include <QDir>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QPointer>
#include <QQuickItem>
#include <QSet>
#include <QtTest>

void ApplicationTests::productPolishOpenTargets_data() {
    QTest::addColumn<QString>("hostKind");
    QTest::addColumn<QString>("scenario");
    for (const auto &host : {QString("primary"), QString("secondary")})
        for (const auto &scenario : {QString("current"), QString("tab"), QString("tab-bg"),
                QString("clean-tab"), QString("clean-cancel"), QString("clean-current"),
                QString("clean-unchanged"), QString("context-tab"), QString("context-bg"),
                QString("dismiss-tab"), QString("dismiss-bg"), QString("explicit-context")})
            QTest::newRow(qPrintable(host + "-" + scenario)) << host << scenario;
}

void ApplicationTests::productPolishOpenTargets() {
    QFETCH(QString, hostKind);
    QFETCH(QString, scenario);
    QString owner, ui;
    QQuickWindow *host = nullptr;
    preparePageSearchHost(hostKind, owner, ui, host);
    const auto initial = evaluate(ui + ".current_url").toString();
    const auto sourceId = evaluate(ui + ".tab_id_for_index(0)").toString();
    QPointer<QObject> sourceView(evaluate(pageReceiver).value<QObject *>());
    QVERIFY(sourceView);
    page("window.openTargetDraft = 'keep unfinished work'");
    const bool background = scenario.endsWith("bg");
    const bool current = scenario == "current" || scenario == "clean-current";
    const bool clean = scenario.startsWith("clean-");
    const bool automatic = scenario.startsWith("context-") || scenario.startsWith("dismiss-");
    const bool explicitContext = scenario == "explicit-context";
    const auto profile = hostKind == "secondary" ? QString("page-search") : QString("default");
    const auto target = current ? QString("current") : background ? QString("tab-bg") : QString("tab");
    const auto destination = initial + "?destination=yes";

    if (automatic || explicitContext) {
        QVERIFY(evaluate(ui + ".execute_command(" + quoted("context-create routed --profile " + profile) + ")").toBool());
        QTRY_VERIFY(evaluate(ui + ".status_text.indexOf('Context created: routed')===0").toBool());
        const auto configDirectory = directory.filePath("open-target-config-" + hostKind + "-" + scenario);
        QVERIFY(QDir().mkpath(configDirectory));
        QFile config(configDirectory + "/config.toml");
        QVERIFY(config.open(QIODevice::WriteOnly));
        config.write("schema_version=3\n");
        config.close();
        QFile contextsFile(configDirectory + "/contexts.toml");
        QVERIFY(contextsFile.open(QIODevice::WriteOnly));
        contextsFile.write(("schema_version=3\n[[contexts]]\nname=\"routed\"\nlabel=\"Routed\"\nprofile=\""
            + profile + "\"\n").toUtf8());
        contextsFile.close();
        evaluate(ui + ".config_path=" + quoted(config.fileName()));
        const QJsonObject contexts{{"schema_version", 3}, {"contexts", QJsonArray{QJsonObject{
            {"name", "routed"}, {"label", "Routed"}, {"profile", profile}}}}};
        QVERIFY(evaluate(ui + ".set_contexts_configuration(" + quoted(QString::fromUtf8(
            QJsonDocument(contexts).toJson(QJsonDocument::Compact))) + ")").toBool());
        if (automatic)
            QVERIFY(evaluate(ui + ".execute_command(" + quoted("context-route add " + initial
                + " routed --entry-point explicit-open") + ")").toBool());
    }
    if (explicitContext) {
        QVERIFY(evaluate(ui + ".execute_command(" + quoted("context-create other --profile " + profile) + ")").toBool());
        QTRY_VERIFY(evaluate(ui + ".status_text.startsWith('Context created: other')").toBool());
    }
    QVERIFY(evaluate(ui + ".execute_command('macro-record a')").toBool());
    QString open = "open --target " + target;
    if (clean) open += " --clean-link";
    if (explicitContext) open += " --profile " + profile + " --context routed";
    open += " " + destination;
    if (clean && scenario != "clean-unchanged") open += "&utm_source=owned-test";
    QVERIFY2(evaluate(ui + ".execute_command(" + quoted(open) + ")").toBool(),
        qPrintable(evaluate(ui + ".status_text").toString()));
    evaluate("testWindow.completeBrowserKeyAction(" + ui + "," + owner + ")");
    if (clean && scenario != "clean-unchanged") {
        QTRY_VERIFY(evaluate(ui + ".link_preview_visible").toBool());
        QCOMPARE(evaluate(ui + ".tab_count").toInt(), 1);
        QCOMPARE(page("location.href").toString(), initial);
        QVERIFY(evaluate(ui + ".macro_status_text.endsWith('(0)')").toBool());
        if (scenario == "clean-cancel") {
            evaluate(ui + ".clear_link_preview()");
            QCOMPARE(evaluate(ui + ".tab_count").toInt(), 1);
            QCOMPARE(page("window.openTargetDraft").toString(), QString("keep unfinished work"));
            QVERIFY(evaluate(ui + ".macro_status_text.endsWith('(0)')").toBool());
            QVERIFY(evaluate(ui + ".execute_command('macro-stop')").toBool());
            QVERIFY(!evaluate(ui + ".execute_command('macro-play a')").toBool());
            return;
        }
        QVERIFY(evaluate(ui + ".confirm_link_navigation()").toBool());
        evaluate("testWindow.completeBrowserKeyAction(" + ui + "," + owner + ")");
    }
    if (automatic) {
        QTRY_VERIFY(!evaluate(ui + ".context_route_id").toString().isEmpty());
        QCOMPARE(evaluate(ui + ".tab_count").toInt(), 1);
        QVERIFY(evaluate(ui + ".macro_status_text.endsWith('(0)')").toBool());
        QTRY_VERIFY(evaluate("testWindow.contextRouteVisible && testWindow.contextRouteUi === " + ui).toBool());
        sendOwnedBrowserKey(host, scenario.startsWith("dismiss-") ? Qt::Key_N : Qt::Key_Y);
        QTRY_VERIFY(!evaluate("testWindow.contextRouteVisible").toBool());
    }
    QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), current ? 1 : 2);
    const auto destinationIndex = current ? 0 : 1;
    const auto destinationView = owner + ".tabViewAt(" + QString::number(destinationIndex) + ")";
    QTRY_VERIFY(evaluate("!!" + destinationView).toBool());
    const auto activeId = evaluate(ui + ".tab_id_for_index(" + ui + ".active_tab_index)").toString();
    QCOMPARE(activeId == sourceId, current || background);
    pageReceiver = destinationView;
    QTRY_COMPARE(page("location.href").toString(), destination);
    QTRY_VERIFY(!evaluate(destinationView + ".loading").toBool());
    if (scenario == "tab" || scenario == "tab-bg") {
        // Export committed relationships from either the memory-only or
        // durable profile API. A native load can finish before Core observes
        // its commit, and an earlier durable preview can contain only source.
        const auto exportPath = directory.filePath("open-target-journey-" + hostKind + "-" + scenario + ".json");
        QJsonObject graph;
        QTRY_VERIFY2(([&] {
            const auto preview = evaluate(ui + ".journey_export_preview()").toString();
            if (preview.isEmpty() && evaluate(ui + ".journey_export_preview_text").toString().isEmpty())
                return false;
            QFile exported(exportPath);
            if (exported.exists() && !exported.remove()) return false;
            if (!evaluate(ui + ".export_journey(" + quoted(exportPath) + ")").toBool()
                    || !exported.open(QIODevice::ReadOnly)) return false;
            graph = QJsonDocument::fromJson(exported.readAll()).object();
            bool sourceReady = false, destinationReady = false;
            for (const auto &value : graph.value("nodes").toArray()) {
                const auto url = value.toObject().value("url").toString();
                sourceReady |= url == initial;
                destinationReady |= url == destination;
            }
            return sourceReady && destinationReady;
        })(), qPrintable(evaluate(ui + ".status_text").toString()));
        QSet<QString> sourceNodes;
        QString destinationNode;
        for (const auto &value : graph.value("nodes").toArray()) {
            const auto node = value.toObject();
            if (node.value("url").toString() == initial) sourceNodes.insert(node.value("id").toString());
            if (node.value("url").toString() == destination) destinationNode = node.value("id").toString();
        }
        QVERIFY(!sourceNodes.isEmpty());
        QVERIFY(!destinationNode.isEmpty());
        bool linked = false;
        for (const auto &value : graph.value("edges").toArray()) {
            const auto edge = value.toObject();
            linked |= sourceNodes.contains(edge.value("source").toString())
                && edge.value("target").toString() == destinationNode;
        }
        QVERIFY2(linked, "New-tab navigation must retain the originating journey node");
    }
    QVERIFY(sourceView);
    if (!current) {
        QCOMPARE(evaluate(owner + ".tabViewAt(0)").value<QObject *>(), sourceView.data());
        pageReceiver = owner + ".tabViewAt(0)";
        QCOMPARE(page("location.href").toString(), initial);
        QCOMPARE(page("window.openTargetDraft").toString(), QString("keep unfinished work"));
    }
    QVERIFY2(evaluate(ui + ".macro_status_text.endsWith('(1)')").toBool(),
        qPrintable(evaluate(ui + ".macro_status_text").toString()));
    QVERIFY(evaluate(ui + ".execute_command('macro-stop')").toBool());
    if (explicitContext) {
        QVERIFY(evaluate(ui + ".assign_window_context('other')").toBool());
        QTRY_COMPARE(evaluate(ui + ".context_name").toString(), QString("other"));
    }
    const auto beforeRepeat = evaluate(ui + ".tab_count").toInt();
    pageReceiver = owner + ".activeWebView()";
    page("window.repeatSentinel = true");
    sendOwnedBrowserKey(host, Qt::Key_Period);
    QTRY_VERIFY2(evaluate(ui + ".tab_count").toInt() == beforeRepeat + (current ? 0 : 1),
        qPrintable(evaluate("JSON.stringify({status:" + ui + ".status_text,mode:" + ui
            + ".mode,context:" + ui + ".context_name,prompt:testWindow.contextRouteVisible,route:"
            + ui + ".context_route_id,focus:" + owner + ".activeFocusItem?" + owner
            + ".activeFocusItem.objectName:''})").toString()));
    if (explicitContext) QTRY_COMPARE(evaluate(ui + ".context_name").toString(), QString("routed"));
    QTRY_VERIFY(evaluate(ui + ".context_route_id").toString().isEmpty());
    QVERIFY(!evaluate(ui + ".link_preview_visible").toBool());
    if (current) QTRY_VERIFY(page("typeof window.repeatSentinel === 'undefined'").toBool());
    const int repeatedIndex = current ? 0 : beforeRepeat;
    pageReceiver = owner + ".tabViewAt(" + QString::number(repeatedIndex) + ")";
    QTRY_VERIFY(evaluate("!!" + pageReceiver).toBool());
    QTRY_COMPARE(page("location.href").toString(), destination);
    QCOMPARE(evaluate(ui + ".active_tab_index").toInt(), background ? 0 : repeatedIndex);
    QVERIFY(evaluate(ui + ".execute_command('macro-play a')").toBool());
    evaluate("testWindow.completeBrowserKeyAction(" + ui + "," + owner + ")");
    QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), beforeRepeat + (current ? 0 : 2));
    QVERIFY(evaluate(ui + ".context_route_id").toString().isEmpty());
    QVERIFY(!evaluate(ui + ".link_preview_visible").toBool());
    pageReceiver = owner + ".tabViewAt(" + QString::number(current ? 0 : beforeRepeat + 1) + ")";
    QTRY_VERIFY(evaluate("!!" + pageReceiver).toBool());
    QTRY_COMPARE(page("location.href").toString(), destination);

}
