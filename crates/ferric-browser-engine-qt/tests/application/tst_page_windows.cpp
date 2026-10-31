#include "tst_application.h"
#include "owned_browser_keyboard.h"
#include <QQuickItem>
#include <QQmlContext>
#include <QGuiApplication>
#include <QPointer>
#include <QtTest>
#include <QDirIterator>
#include <QFile>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonArray>
#include <QTemporaryDir>
#include <QFileInfo>
#include <QProcess>
#include <QLocalServer>
#include <QLocalSocket>
#include <QRegularExpression>
#include <QScopeGuard>
#include <QThread>

namespace {
QQuickItem *findVisible(QQuickItem *parent, const QString &name) {
    if (parent->isVisible() && parent->objectName() == name) return parent;
    for (auto *child : parent->childItems()) if (auto *match = findVisible(child, name)) return match;
    return nullptr;
}
bool clickCloseDecision(QQuickWindow *host, bool proceed) {
    QPointer<QQuickWindow> ownedHost(host);
    if (!ownedHost || !ownedHost->isVisible()) return true;
    QSignalSpy frame(host, &QQuickWindow::frameSwapped);
    host->requestUpdate();
    if (!QTest::qWaitFor([&] { return !ownedHost || !ownedHost->isVisible() || !frame.isEmpty(); })) return false;
    // A completed close can hide the host during the frame wait. It needs no
    // second decision; callers still verify final close/cancel outcomes.
    if (!ownedHost || !ownedHost->isVisible()) return true;
    auto *button = findVisible(host->contentItem(), proceed ? "shutdownProceedButton" : "shutdownKeepButton");
    if (!button) return false;
    QTest::mouseClick(host, Qt::LeftButton, Qt::NoModifier, button->mapToScene(button->boundingRect().center()).toPoint());
    return true;
}
}

void ApplicationTests::durablePersistenceFlushWaitsForPendingSave()
{
    // One direct QML invocation leaves the named-save receipt pending even if
    // the background write has finished. The flush must drain that older save
    // and capture the later mutation, without requiring a second close attempt.
    const auto result = evaluate(QStringLiteral(R"JS((function(){
        var ui=testWindow.browserUi;
        if(!ui.execute_command('session-save before-close'))return 'save not queued';
        if(!ui.set_tab_muted(0,true))return 'mutation not applied';
        return ui.flush_durable_state() ? 'flushed' : String(ui.status_text);
    })())JS")).toString();
    QCOMPARE(result, QStringLiteral("flushed"));
    QDirIterator checkpoints(evaluate("testWindow.storageBasePath").toString(), {"current*.json"}, QDir::Files, QDirIterator::Subdirectories);
    QVERIFY(checkpoints.hasNext());
    QFile saved(checkpoints.next());
    QVERIFY(saved.open(QIODevice::ReadOnly));
    const auto snapshot = QJsonDocument::fromJson(saved.readAll()).object();
    const auto windows = snapshot["windows"].toArray();
    QCOMPARE(windows.size(), 1);
    const auto pages = windows[0].toObject()["tabs"].toArray();
    QCOMPARE(pages.size(), 1);
    QVERIFY(pages[0].toObject()["muted"].toBool());
}

void ApplicationTests::pageWindowPresentation_data()
{
    QTest::addColumn<QString>("scenario");
    for (const auto *scenario : {"window-undo-peer", "window-undo-primary", "window-undo-private-expiry", "window-undo-private-root-last", "window-undo-cancel", "window-undo-context", "window-order", "window-order-fallback", "window-order-fallback-pin", "window-native-identity", "window-focus-lease", "window-focus-stale", "window-focus-expired", "window-focus-reentrant", "window-focus-route", "window-focus-publication", "window-native-grouping", "window-grouping-order", "window-grouping-focus", "window-grouping-order-stale", "window-grouping-unrelated-map", "window-grouping-private-map", "window-grouping-auto-off", "window-grouping-detach", "window-grouping-context", "window-grouping-stale", "window-grouping-profile", "window-grouping-delay", "window-grouping-stale-create", "window-regroup", "window-regroup-stale", "window-regroup-background", "window-regroup-pin", "window-mutations", "window-mutation-navigation", "window-mutation-discard", "window-chain-workflows", "window-chain-multi-hop", "window-chain-failure", "window-chain-discard", "popup-tab-group", "popup-background", "popup-window", "popup-dialog", "window-selection", "session-aggregate", "foreground", "background", "preparation-focus", "preparation-navigation", "preparation-shutdown", "activation-context", "standalone", "private", "first-host-close", "first-page-tab-close", "last-page-close", "last-page-close-cancel", "factory-failure", "expansion-cancel"})
        QTest::newRow(scenario) << QString::fromLatin1(scenario);
}

void ApplicationTests::pageWindowPresentation()
{
    QFETCH(QString, scenario);
    QCOMPARE(text("feature_tabs_presentation_effective"), QStringLiteral("windows"));
    QPointer<QObject> sourceView(evaluate("testWindow.activeWebView()").value<QObject *>());
    QVERIFY(sourceView);
    const auto sourceId = evaluate("testWindow.browserUi.tab_id_for_index(0)").toString();
    const auto initial = page("location.href").toString();
    QVERIFY(!initial.isEmpty());
    page("window.pageWindowDraft = 'unfinished work'");
    QVERIFY(!evaluate("testWindow.tabStripVisible").toBool());
    QVERIFY(!evaluate("testWindow.pageWindowTag").toString().isEmpty());
    QVERIFY(evaluate("testWindow.pageWindowTagRequested").toBool());

    QObject ownedFocusTrace;
    if (scenario == "window-native-grouping") {
        QObject::connect(qGuiApp, &QGuiApplication::focusWindowChanged, &ownedFocusTrace, [&](QWindow *focused) {
            qInfo() << "owned focus" << (focused == window ? "source" : focused ? "peer-or-owned-popup" : "none")
                    << "prepared foreground" << (focused && focused->property("windowPreparedPageActivateRequested").toBool());
        });
        QTRY_COMPARE(QGuiApplication::focusWindow(), window);
    }

    if (scenario == "expansion-cancel") {
        evaluate("for(var i=0;i<12;++i)testWindow.browserUi.new_tab(); testWindow.syncTabModel()");
        QTRY_VERIFY(evaluate("!!testWindow.pageWindowCoordinator.expansionPrompt").toBool());
        QCOMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(), 1);
        QCOMPARE(evaluate("testWindow.tabViews.length").toInt(), 1);
        sendOwnedBrowserKey(window, Qt::Key_Escape);
        QTRY_VERIFY(!evaluate("testWindow.pageWindowCoordinator.pendingExpansion").toBool());
        QCOMPARE(evaluate("testWindow.browserUi.tab_count").toInt(), 1);
        QCOMPARE(evaluate("testWindow.tabViews.length").toInt(), 1);
        QCOMPARE(evaluate("testWindow.activeWebView()").value<QObject *>(), sourceView.data());
        QCOMPARE(page("window.pageWindowDraft").toString(), QStringLiteral("unfinished work"));
        return;
    }

    if (scenario == "factory-failure") evaluate("testWindow.browserWindowComponentObject = null");
    auto destination = initial + (scenario == "session-aggregate" || (scenario.startsWith("window-order") || scenario.startsWith("window-undo")) ? "?page-window=yes" : "?page-window=yes&token=preserve-exact-intent#section");
    const auto target = scenario == "background" ? QStringLiteral("tab-bg")
        : scenario == "standalone" ? QStringLiteral("window")
        : scenario == "private" ? QStringLiteral("private-window") : QStringLiteral("tab");
    savedSelection = QString();
    QPointer<QObject> observed(evaluate("Qt.createQmlObject('import QtQuick; Connections { target: testWindow.browserUi; function onTab_countChanged() { if (target.tab_count === 2) testProbe.savedSelection = target.tab_id_for_index(1) } }', testWindow)").value<QObject *>());
    QVERIFY(observed);
    const bool opened = command("open --target " + target + " " + destination);
    delete observed.data();
    const auto requestedId = savedSelection.toString();
    QVERIFY(opened);
    evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
    if (scenario == "activation-context") {
        auto *staged = evaluate("Object.values(testWindow.pageWindowCoordinator.pendingPages)[0].child").value<QObject *>();
        QVERIFY(staged);
        engine->rootContext()->setContextProperty("testPreparedPage", staged);
        QVERIFY(evaluate("Qt.createQmlObject('import QtQuick; Connections { target: testPreparedPage; function onWindowPreparedPageCommittedChanged() { if (target.windowPreparedPageCommitted) testProbe.savedSelection = target.windowUi.assign_window_context(\"changed\") } }', testWindow)").value<QObject *>());
    }
    if (scenario.startsWith("preparation-")) {
        const auto prepared = QStringLiteral("Object.values(testWindow.pageWindowCoordinator.pendingPages)[0]");
        QVERIFY(evaluate("!!(" + prepared + ")").toBool());
        // The dormant destination has neither a WebEngine document nor a
        // published application-window owner before source commit.
        QVERIFY(!evaluate(prepared + ".child.activeView").value<QObject *>());
        QCOMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(), 1);
        if (scenario == "preparation-focus") {
            QVERIFY(evaluate("testWindow.browserUi.select_tab(0)").toBool());
        } else if (scenario == "preparation-navigation") {
            destination = initial + "?newer-page-window-intent=yes#latest";
            evaluate("testWindow.browserUi.navigate_without_context_route(" + quoted(destination) + ")");
            QCOMPARE(evaluate("testWindow.browserUi.current_url").toString(), destination);
        } else {
            QPointer<QObject> staged(evaluate(prepared + ".child").value<QObject *>());
            QVERIFY(evaluate("testWindow.browserUi.request_shutdown()").toBool());
            QTRY_VERIFY(staged.isNull());
            QCOMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(), 1);
            QCOMPARE(evaluate("testWindow.browserUi.tab_index_for_id(" + quoted(requestedId) + ")").toInt(), 1);
            return;
        }
    }
    if (scenario == "factory-failure") {
        QTRY_COMPARE(evaluate("testWindow.browserUi.tab_count").toInt(), 2);
        QCOMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(), 1);
        QCOMPARE(evaluate("testWindow.tabViewAt(0)").value<QObject *>(), sourceView.data());
        QVERIFY(evaluate("testWindow.tabStripVisible").toBool());
        pageReceiver = "testWindow.tabViewAt(1)";
        QTRY_COMPARE(page("location.href").toString(), destination);
        pageReceiver = "testWindow.tabViewAt(0)";
        QCOMPARE(page("window.pageWindowDraft").toString(), QStringLiteral("unfinished work"));
        return;
    }

    const QString entry = "testWindow.liveWindowEntries().find(function(e){return e.ui!==testWindow.browserUi})";
    QTRY_COMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(), 2);
    const auto childHost = entry + ".host";
    auto *childWindow = qobject_cast<QQuickWindow *>(evaluate(childHost).value<QObject *>());
    QVERIFY(childWindow);
    QTRY_COMPARE(evaluate("testWindow.browserUi.tab_count").toInt(), 1);
    QCOMPARE(evaluate("testWindow.tabViews.length").toInt(), 1);
    QCOMPARE(evaluate("testWindow.browserUi.tab_id_for_index(0)").toString(), sourceId);
    QCOMPARE(evaluate("testWindow.tabViewAt(0)").value<QObject *>(), sourceView.data());
    QCOMPARE(evaluate(entry + ".ui.tab_count").toInt(), 1);
    QVERIFY(!evaluate(childHost + ".tabStripVisible").toBool());
    QVERIFY(evaluate(childHost + ".pageWindowTag !== testWindow.pageWindowTag").toBool());
    QCOMPARE(evaluate(childHost + ".pageGroupId === testWindow.pageGroupId").toBool(),
             target == "tab" || target == "tab-bg");
    QTRY_VERIFY(evaluate(childHost + ".profileBootstrapComplete && !!" + childHost + ".activeView").toBool());
    QCOMPARE(evaluate(childHost + ".activeView.profile === testWindow.browserProfile").toBool(), scenario != "private");
    if (target == "tab" || target == "tab-bg") {
        QVERIFY(!requestedId.isEmpty());
        QCOMPARE(evaluate(entry + ".ui.tab_id_for_index(0)").toString(), requestedId);
        QCOMPARE(evaluate("testWindow.browserUi.tab_index_for_id(" + quoted(requestedId) + ")").toInt(), -1);
    }
    if (scenario == "background" || scenario == "preparation-focus") {
        QVERIFY(!childWindow->isVisible());
        QCOMPARE(QGuiApplication::focusWindow(), window);
        QVERIFY(evaluate("testWindow.activateBrowserWindow(" + childHost + "," + entry + ".ui)").toBool());
    }
    QTRY_VERIFY(childWindow->isVisible());
    pageReceiver = childHost + ".activeView";
    if (scenario == "activation-context") {
        QTRY_COMPARE(evaluate(entry + ".ui.context_name").toString(), QStringLiteral("changed"));
        QVERIFY(savedSelection.toBool());
        QTRY_VERIFY(evaluate(childHost + ".commandNoticeVisible").toBool());
        QTRY_VERIFY2(evaluate(childHost + ".commandNoticeText").toString().startsWith("Page opening cancelled"),
                     qPrintable(evaluate(childHost + ".commandNoticeText").toString()));
        QCOMPARE(page("location.href").toString(), QStringLiteral("about:blank"));
    } else {
        QTRY_COMPARE(page("location.href").toString(), destination);
    }
    pageReceiver = "testWindow.tabViewAt(0)";
    QCOMPARE(page("window.pageWindowDraft").toString(), QStringLiteral("unfinished work"));
    if (scenario == "window-focus-publication") {
        // Exercise production pending-state publication/receipts using owned
        // inert hosts, so callbacks can supersede synchronously and exactly.
        const auto result=evaluate(QStringLiteral(R"JS((function(){
            var a=Qt.createQmlObject('import QtQml; QtObject { property bool active:false; function show(){} function raise(){} function requestActivate(){} }',testWindow);
            var b=Qt.createQmlObject('import QtQml; QtObject { property bool active:false; function show(){} function raise(){} function requestActivate(){} }',testWindow);
            var ui=Qt.createQmlObject('import QtQml; QtObject { property string context_workspace:""; property string status_text:""; property var receipts:({}); property var results:({}); function complete_window_focus(id,status){var counts=Object.assign({},receipts);counts[id]=(counts[id]||0)+1;receipts=counts;var values=Object.assign({},results);values[id]=status;results=values} function hyprland_route_workspace(workspace){return true} }',testWindow);
            var hook=Qt.createQmlObject('import QtQml; Connections { target:testWindow; property var newerHost:null; property var newerUi:null; property string watch:"publish"; property bool injected:false; function onPendingActivationChanged(){if(injected)return;var state=target.pendingActivation;if((watch==="publish" && state && state.operation==="old") || (watch==="clear" && !state)){injected=true;target.activateBrowserWindow(newerHost,newerUi,watch==="publish"?"new":"clear-new")}} }',testWindow);
            hook.newerHost=b;hook.newerUi=ui;
            testWindow.activateBrowserWindow(a,ui,"old");
            if(!hook.injected || !testWindow.pendingActivation || testWindow.pendingActivation.operation!=="new" || !testWindow.activationOutcomeTimerObject.running)return "new publication lost";
            if(ui.receipts.old!==1 || ui.results.old!=="unknown")return "old publication receipt lost or duplicated";
            b.active=true;testWindow.activationOutcomeTimerObject.triggered();
            if(testWindow.pendingActivation!==null || ui.receipts.new!==1 || ui.results.new!=="activated" || ui.receipts.old!==1)return "new publication receipt lost or duplicated";
            a.active=false;b.active=false;
            testWindow.activateBrowserWindow(a,ui,"clear-old");
            hook.watch="clear";hook.newerHost=a;hook.injected=false;
            testWindow.activateBrowserWindow(b,ui,"middle");
            if(!hook.injected || !testWindow.pendingActivation || testWindow.pendingActivation.operation!=="clear-new" || !testWindow.activationOutcomeTimerObject.running)return "new cancellation publication lost";
            a.active=true;testWindow.activationOutcomeTimerObject.triggered();
            if(testWindow.pendingActivation!==null || ui.receipts["clear-old"]!==1 || ui.receipts.middle!==1 || ui.receipts["clear-new"]!==1 || ui.results["clear-new"]!=="activated")return "cancellation receipt lost or duplicated";
            hook.destroy();
            var nested=Qt.createQmlObject('import QtQml; QtObject { property bool active:false; property var queuedHost:null; property var queuedUi:null; function show(){testWindow.activateBrowserWindow(queuedHost,queuedUi,"queued-expire");testWindow.activationOutcomeTimerObject.triggered()} function raise(){} function requestActivate(){} }',testWindow);
            var queued=Qt.createQmlObject('import QtQml; QtObject { property bool active:false; property int requests:0; function show(){} function raise(){} function requestActivate(){requests++} }',testWindow);
            nested.queuedHost=queued;nested.queuedUi=ui;
            testWindow.activateBrowserWindow(nested,ui,"outer-expire");
            if(testWindow.pendingActivation!==null || queued.requests!==0 || ui.receipts["queued-expire"]!==1 || ui.receipts["outer-expire"]!==1)return "queued expiry lost, duplicated or dispatched";
            a.destroy();b.destroy();ui.destroy();nested.destroy();queued.destroy();
            return "passed";
        })())JS")).toString();
        QCOMPARE(result,QStringLiteral("passed"));
        return;
    }
    if (scenario == "window-focus-route") {
        const auto result = evaluate(QStringLiteral(R"JS((function(){
            var hostA=Qt.createQmlObject('import QtQml; QtObject { property bool active:false; function show(){} function raise(){} function requestActivate(){} }',testWindow);
            var hostB=Qt.createQmlObject('import QtQml; QtObject { property bool active:false; function show(){} function raise(){} function requestActivate(){} }',testWindow);
            var ui=Qt.createQmlObject('import QtQml; QtObject { property string context_workspace:"test-workspace"; property string status_text:""; property int routed:0; function complete_window_focus(id,status){} function hyprland_route_workspace(workspace){routed++;return true} }',testWindow);
            testWindow.activateBrowserWindow(hostA,ui,"","test-workspace");
            if(testWindow.pendingWorkspaceRoute!=="test-workspace" || testWindow.pendingWorkspaceRouteIntent!==testWindow.pendingActivationIntent)return "new route lost";
            testWindow.activateBrowserWindow(hostB,ui);
            if(testWindow.pendingWorkspaceRoute.length!==0)return "superseded route retained";
            testWindow.activateBrowserWindow(hostA,ui);
            hostA.active=true;
            testWindow.activationOutcomeTimerObject.triggered();
            if(ui.routed!==0)return "old route resurrected";
            hostA.active=false;
            testWindow.activateBrowserWindow(hostA,ui,"","test-workspace");
            hostA.active=true;
            testWindow.activationOutcomeTimerObject.triggered();
            if(ui.routed!==1 || testWindow.pendingWorkspaceRoute.length!==0)return "new matching route failed";
            hostA.active=false;
            testWindow.activateBrowserWindow(hostA,ui,"","test-workspace");
            testWindow.supersedePageWindowFocus();
            hostA.active=true;
            testWindow.activationOutcomeTimerObject.triggered();
            if(ui.routed!==1 || testWindow.pendingWorkspaceRoute.length!==0)return "old generation routed";
            testWindow.activationOutcomeTimerObject.stop();
            hostA.destroy();hostB.destroy();ui.destroy();
            return "passed";
        })())JS")).toString();
        QCOMPARE(result,QStringLiteral("passed"));
        return;
    }
    if (scenario == "window-focus-lease" || scenario == "window-focus-stale" || scenario == "window-focus-expired" || scenario == "window-focus-reentrant") {
        QString newerHost, newerIssuer;
        QQuickWindow *newerWindow = nullptr;
        if (scenario == "window-focus-reentrant") {
            for (int index=0;index<2;++index) {
                QVERIFY(command("open --target window " + initial + "?focus-newer=" + QString::number(index)));
                evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
                QTRY_COMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(),3+index);
            }
            const auto extra = "testWindow.liveWindowEntries().filter(function(e){return e.host!==testWindow && e.host!==" + childHost + "})";
            QTRY_VERIFY(evaluate(extra + ".every(function(e){return e.host.profileBootstrapComplete && !!e.host.activeView})").toBool());
            newerIssuer = extra + "[0].ui";
            newerHost = extra + "[1].host";
            newerWindow = qobject_cast<QQuickWindow *>(evaluate(newerHost).value<QObject *>());
            QVERIFY(newerWindow);
            childWindow->hide();
            QTRY_VERIFY(!childWindow->isVisible());
        }
        QVERIFY(evaluate("testWindow.activateBrowserWindow(testWindow,testWindow.browserUi)").toBool());
        QTRY_COMPARE(QGuiApplication::focusWindow(),window);
        QVERIFY(command("tab-next"));
        QString focusAction;
        // Drive only queued Core callbacks. The QML maintenance timer must not
        // consume the private native action before this boundary regression.
        for (int turn=0;turn<20 && focusAction.isEmpty();++turn) {
            QCoreApplication::sendPostedEvents(nullptr,QEvent::MetaCall);
            const auto candidate = evaluate("testWindow.browserUi.take_engine_action()").toString();
            if (candidate.startsWith("window-focus\t")) focusAction=candidate;
            else QVERIFY2(candidate.isEmpty(),qPrintable(candidate));
        }
        QVERIFY2(!focusAction.isEmpty(),"queued focus lease action was not produced");
        const auto fields = focusAction.split('\t');
        QCOMPARE(fields.size(),5);
        const auto request = childHost + ".requestPageWindowFocus(testWindow.browserUi," + entry + ".ui," + quoted(fields[3]) + ")";
        const auto status = childHost + ".pageWindowFocusStatus(testWindow.browserUi," + entry + ".ui," + quoted(fields[3]) + ")";
        if (scenario == "window-focus-stale") {
            QVERIFY(evaluate(entry + ".ui.assign_window_context('changed')").toBool());
            QCOMPARE(evaluate(request).toString(),QStringLiteral("stale"));
            QCOMPARE(QGuiApplication::focusWindow(),window);
        } else if (scenario == "window-focus-expired") {
            QTest::qWait(2100);
            QCOMPARE(evaluate(request).toString(),QStringLiteral("stale"));
            QCOMPARE(QGuiApplication::focusWindow(),window);
        } else if (scenario == "window-focus-reentrant") {
            bool injected=false, newerAccepted=false;
            QObject guard;
            QObject::connect(childWindow,&QWindow::visibleChanged,&guard,[&](bool visible) {
                if (!visible || injected) return;
                injected=true;
                newerAccepted=evaluate("testWindow.activateBrowserWindow("+newerHost+","+newerIssuer+")").toBool();
            });
            // Exercise the production dispatcher so the newer request is
            // serialized after this older show/map call has unwound.
            evaluate("testWindow.executeWindowFocusAction(testWindow.browserUi," + quoted(focusAction) + ")");
            QVERIFY(injected && newerAccepted);
            QTRY_COMPARE(QGuiApplication::focusWindow(),newerWindow);
            QCOMPARE(evaluate(status).toString(),QStringLiteral("stale"));
        } else {
            QCOMPARE(evaluate("testWindow.requestPageWindowFocus(testWindow.browserUi," + entry + ".ui," + quoted(fields[3]) + ")").toString(),QStringLiteral("stale"));
            QCOMPARE(QGuiApplication::focusWindow(),window);
            QCOMPARE(evaluate(request).toString(),QStringLiteral("requested"));
            QTRY_COMPARE(QGuiApplication::focusWindow(),childWindow);
            QTRY_COMPARE(evaluate(status).toString(),QStringLiteral("activated"));
            // The same valid Core owners cannot replay a consumed capability.
            QCOMPARE(evaluate(request).toString(),QStringLiteral("stale"));
        }
        QCOMPARE(evaluate("testWindow.activeWebView()").value<QObject *>(),sourceView.data());
        QCOMPARE(page("window.pageWindowDraft").toString(),QStringLiteral("unfinished work"));
        return;
    }
    if (scenario == "window-grouping-stale" || scenario == "window-grouping-profile" || scenario == "window-grouping-delay" || scenario == "window-grouping-stale-create" || scenario == "window-grouping-order-stale") {
        // This owned mock exercises GUI admission/lifecycle and stalled IPC.
        // Native Lua effects are qualified separately on the real compositor.
        QTemporaryDir runtime;
        QVERIFY(runtime.isValid());
        const auto oldRuntime = qgetenv("XDG_RUNTIME_DIR");
        const auto oldInstance = qgetenv("HYPRLAND_INSTANCE_SIGNATURE");
        const auto restoreEnvironment = qScopeGuard([&] {
            qputenv("XDG_RUNTIME_DIR", oldRuntime);
            qputenv("HYPRLAND_INSTANCE_SIGNATURE", oldInstance);
        });
        const auto instance = QStringLiteral("ferric-owned-mock");
        const auto directory = runtime.path() + "/hypr/" + instance;
        QVERIFY(QDir().mkpath(directory));
        QLocalServer server;
        QVERIFY(server.listen(directory + "/.socket.sock"));
        qputenv("XDG_RUNTIME_DIR", runtime.path().toUtf8());
        qputenv("HYPRLAND_INSTANCE_SIGNATURE", instance.toUtf8());
        bool injected = false, fixtureOk = true;
        int writes = 0, emptyConnections = 0;
        QPointer<QLocalSocket> heldReply;
        QByteArray createdCap, createdScope;
        QObject::connect(&server, &QLocalServer::newConnection, &server, [&] {
            while (auto *socket = server.nextPendingConnection()) {
                QObject::connect(socket, &QLocalSocket::disconnected, &server, [&, socket] {
                    if (socket->property("ownedRequest").toByteArray().isEmpty()) ++emptyConnections;
                });
                QObject::connect(socket, &QLocalSocket::readyRead, &server, [&, socket] {
                    auto input = socket->property("ownedRequest").toByteArray() + socket->readAll();
                    socket->setProperty("ownedRequest", input);
                    if (socket->property("handled").toBool()) return;
                    if (input.size() > 65536) { fixtureOk = false; socket->close(); return; }
                    if (input != "j/version" && !input.endsWith("end)()")) return;
                    socket->setProperty("handled", true);
                    auto reply = [socket](const QByteArray &output) { socket->write(output); socket->disconnectFromServer(); };
                    if (input == "j/version") {
                        reply("{\"tag\":\"v0.56.2\",\"commit\":\"efb50993780079460b0cbed1363e2166a2de1d9f\"}");
                    } else if (input.contains("local ok,result=pcall")) {
                        ++writes;
                        if (scenario == "window-grouping-stale-create") {
                            if (!injected) {
                                const auto receiptFields = QRegularExpression(QStringLiteral("epoch=\"[a-f0-9-]{36}\",scope=\"([a-f0-9-]{36})\",fresh=\"([a-f0-9-]{36})\"")).match(QString::fromUtf8(input));
                                fixtureOk = receiptFields.hasMatch();
                                createdScope = receiptFields.captured(1).toUtf8();
                                createdCap = receiptFields.captured(2).toUtf8();
                                injected = true;
                                // The request has already crossed its GUI/native
                                // commit point. Revoke Auto before its first
                                // definite creation receipt is delivered.
                                evaluate("testWindow.liveWindowEntries().forEach(function(e){e.ui.execute_command('set --temp hyprland.page_grouping=off')})");
                                reply("created|" + createdCap + "|" + createdScope + "|101");
                            } else {
                                fixtureOk = fixtureOk && input.contains(createdCap) && input.contains("auto=false");
                                reply("retired|" + createdCap + "|" + createdScope + "|");
                            }
                        } else if (scenario == "window-grouping-profile" && !injected) {
                            injected = true;
                            fixtureOk = command("open --target private-window " + initial + "?during-owned-ipc=yes");
                            evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
                            heldReply = socket; // Deliberately stall the worker, never the GUI.
                        } else reply("defer-active");
                    } else {
                        if (scenario == "window-grouping-stale" && !injected) {
                            injected = true;
                            evaluate("testWindow.browserWindowComponentObject = null");
                            fixtureOk = command("open --target tab-bg " + initial + "?owned-stale-fallback=yes");
                            evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
                        }
                        QByteArray output;
                        auto tags = QRegularExpression(QStringLiteral("ferric-page-[a-f0-9-]{36}")).globalMatch(QString::fromUtf8(input));
                        int index = 0;
                        while (tags.hasNext()) {
                            tags.next();
                            if (index) output += '\n';
                            ++index;
                            output += QByteArray::number(index) + "|ok|" + QByteArray::number(100 + index)
                                + "|0x" + QByteArray::number(4096 + index, 16);
                        }
                        if (scenario == "window-grouping-order-stale") {
                            if (!injected) {
                                injected = true;
                                fixtureOk = command("tab-move " + sourceId + " right ;; jseval --world page window.nativeOrderAdmissionChanged=1");
                                evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
                                // Actual Core movement is already queued on
                                // this GUI thread. Queue the read-only reply
                                // behind it, within the socket's deadline.
                                QMetaObject::invokeMethod(&server, [socket, output] {
                                    socket->write(output); socket->disconnectFromServer();
                                }, Qt::QueuedConnection);
                            }
                            // Hold any fresh retry so the assertion below
                            // measures only the original stale admission.
                        } else if (scenario == "window-grouping-delay" && !injected) {
                            injected = true;
                            // Flush a complete resolution/EOF, then keep the GUI
                            // callback queued past the prepared send's deadline.
                            socket->write(output); socket->flush(); socket->close();
                            QThread::msleep(1200);
                        } else reply(output);
                    }
                });
            }
        });
        QVERIFY(command("set --temp hyprland.page_grouping=auto"));
        QVERIFY(evaluate(entry + ".ui.execute_command('set --temp hyprland.page_grouping=auto')").toBool());
        QTRY_VERIFY_WITH_TIMEOUT(injected, 10000);
        QVERIFY(fixtureOk);
        if (scenario == "window-grouping-order-stale") {
            QTRY_VERIFY_WITH_TIMEOUT(emptyConnections > 0, 10000);
            QTRY_COMPARE(page("window.nativeOrderAdmissionChanged").toInt(), 1);
            QCOMPARE(writes, 0);
        } else if (scenario == "window-grouping-stale") {
            QTRY_COMPARE(evaluate("testWindow.browserUi.tab_count").toInt(), 2);
            QTRY_COMPARE(text("feature_page_grouping_status"), QStringLiteral("unavailable"));
            QTRY_VERIFY(evaluate("testWindow.browserUi.maintenance_delay_ms()").toInt() != 50);
            QCOMPARE(writes, 0);
        } else if (scenario == "window-grouping-stale-create") {
            QTRY_VERIFY_WITH_TIMEOUT(writes >= 2, 10000);
            QVERIFY(fixtureOk);
            QTRY_VERIFY(evaluate("testWindow.browserUi.maintenance_delay_ms()").toInt() != 50);
        } else if (scenario == "window-grouping-delay") {
            QTRY_VERIFY(emptyConnections > 0);
            // A fresh retry succeeds after the expired callback sends zero
            // bytes, proving the coordinator did not become inhibited.
            QTRY_VERIFY_WITH_TIMEOUT(writes > 0, 10000);
            QTRY_COMPARE(text("feature_page_grouping_status"), QStringLiteral("waiting"));
        } else {
            QTRY_COMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(), 3);
            const auto originalPeerTag = evaluate(childHost + ".pageWindowTag").toString();
            const auto privateEntry = "testWindow.liveWindowEntries().find(function(e){return e.host!==testWindow && e.host.pageWindowTag!==" + quoted(originalPeerTag) + "})";
            QTRY_VERIFY(evaluate("!!(" + privateEntry + ")").toBool());
            QTRY_VERIFY(evaluate(privateEntry + ".host.profileBootstrapComplete && !!" + privateEntry + ".host.activeView").toBool());
            QVERIFY(evaluate(privateEntry + ".host.activeView.profile.offTheRecord").toBool());
            const auto profile = evaluate(privateEntry + ".ui.page_host_profile_id()").toString();
            QVERIFY(!profile.isEmpty());
            QVERIFY(profile != evaluate("testWindow.browserUi.page_host_profile_id()").toString());
            QVERIFY(writes > 0);
            if (heldReply && heldReply->state() == QLocalSocket::ConnectedState) { heldReply->write("defer-active"); heldReply->disconnectFromServer(); }
        }
        // Retire the mock's permission before restoring the real instance.
        evaluate("testWindow.liveWindowEntries().forEach(function(e){e.ui.execute_command('set --temp hyprland.page_grouping=off')})");
        return;
    }
    if (scenario == "window-native-grouping" || scenario == "window-grouping-order" || scenario == "window-grouping-focus" || scenario == "window-grouping-unrelated-map" || scenario == "window-grouping-private-map" || scenario == "window-grouping-auto-off" || scenario == "window-grouping-detach" || scenario == "window-grouping-context") {
        const auto sourceTag = evaluate("testWindow.pageWindowTag").toString();
        const auto peerTag = evaluate(childHost + ".pageWindowTag").toString();
        const auto separate = "testWindow.liveWindowEntries().find(function(e){return e.host!==testWindow && e.host.pageWindowTag!==" + quoted(peerTag) + "})";
        QQuickWindow *separateWindow = nullptr;
        auto mapSeparate = [&]() {
            if (!evaluate(entry + ".ui.execute_command(" + quoted("open --target " + QString(scenario == "window-grouping-private-map" ? "private-window" : "window") + " " + initial + "?independent-native-window=yes") + ")").toBool()) return false;
            evaluate("testWindow.completeBrowserKeyAction(" + entry + ".ui," + childHost + ")");
            if (!QTest::qWaitFor([&] { return evaluate("testWindow.liveWindowEntries().length").toInt() == 3
                    && evaluate("!!(" + separate + ") && " + separate + ".host.profileBootstrapComplete").toBool(); }, 10000)) return false;
            separateWindow = qobject_cast<QQuickWindow *>(evaluate(separate + ".host").value<QObject *>());
            return separateWindow && QTest::qWaitFor([&] { return QGuiApplication::focusWindow() == separateWindow; });
        };
        // Map the sentinel first to isolate existing-window placement from
        // compositor on-map policy. The other row exercises that policy.
        if (scenario != "window-grouping-unrelated-map" && scenario != "window-grouping-private-map") QVERIFY(mapSeparate());
        QVERIFY(evaluate("testWindow.activateBrowserWindow(" + childHost + "," + entry + ".ui)").toBool());
        QTRY_COMPARE(QGuiApplication::focusWindow(), childWindow);
        QVERIFY(command("set --temp hyprland.page_grouping=auto"));
        QVERIFY(evaluate(entry + ".ui.execute_command('set --temp hyprland.page_grouping=auto')").toBool());
        auto queryLua = [](const QString &code) {
            QProcess query;
            query.start(QStringLiteral("hyprctl"), {QStringLiteral("repl"), code});
            if (!query.waitForFinished(3000) || query.exitCode() != 0) return QStringLiteral("query failed");
            return QString::fromUtf8(query.readAllStandardOutput()).trimmed();
        };
        auto groupState = [&]() {
            return queryLua(QStringLiteral(
                "return (function() local s,t=nil,nil for _,w in ipairs(hl.get_windows()) do "
                "if w.pid==%1 and w.xdg_tag=='%2' then s=w end "
                "if w.pid==%1 and w.xdg_tag=='%3' then t=w end end "
                "if not s or not t then return 'missing' end local a=hl.get_active_window() "
                "if not s.group or s.group~=t.group then return 'waiting|'..(a==t and 'peer' or a==s and 'source' or 'other-or-none')..'|'..tostring(t.group and t.group.size or 0) end "
                "local c=s.group.current "
                "return tostring(s.group.size)..'|'..(a==t and 'peer' or a==s and 'source' or 'other-or-none')..'|'..(c==t and 'peer' or c==s and 'source' or 'other') end)()")
                .arg(QCoreApplication::applicationPid()).arg(sourceTag, peerTag));
        };
        QTRY_COMPARE_WITH_TIMEOUT(groupState(), QStringLiteral("waiting|peer|1"), 15000);
        QTRY_COMPARE(text("feature_page_grouping_status"), QStringLiteral("waiting"));
        QCOMPARE(QGuiApplication::focusWindow(), childWindow);
        QCOMPARE(evaluate("testWindow.activeWebView()").value<QObject *>(), sourceView.data());
        QCOMPARE(evaluate("testWindow.browserUi.tab_id_for_index(0)").toString(), sourceId);
        QCOMPARE(page("window.pageWindowDraft").toString(), QStringLiteral("unfinished work"));
        if (scenario == "window-grouping-unrelated-map" || scenario == "window-grouping-private-map") {
            QVERIFY(mapSeparate());
            const auto separateTag = evaluate(separate + ".host.pageWindowTag").toString();
            const auto separateState = QStringLiteral(
                "return (function() local w=nil for _,candidate in ipairs(hl.get_windows()) do "
                "if candidate.pid==%1 and candidate.xdg_tag=='%2' then w=candidate end end "
                "if not w then return 'missing' end return w.group and 'grouped' or 'separate' end)()")
                .arg(QCoreApplication::applicationPid()).arg(separateTag);
            // Default on-map insertion is repaired using the saved weak managed
            // capability and all actual current Core roles, without refocusing.
            QTRY_COMPARE_WITH_TIMEOUT(queryLua(separateState), QStringLiteral("separate"), 15000);
            QTRY_COMPARE_WITH_TIMEOUT(groupState(), QStringLiteral("2|other-or-none|peer"), 15000);
            QCOMPARE(QGuiApplication::focusWindow(), separateWindow);
            if (scenario == "window-grouping-private-map") {
                QVERIFY(evaluate(separate + ".host.activeView.profile.offTheRecord").toBool());
                QVERIFY(evaluate(separate + ".ui.page_host_profile_id()!==testWindow.browserUi.page_host_profile_id()").toBool());
            }
            QVERIFY(evaluate(separate + ".host.pageGroupId!==testWindow.pageGroupId").toBool());
            const auto independentPage = evaluate(separate + ".ui.tab_id_for_index(0)").toString();
            QVERIFY(!command("tab-pin " + independentPage + " on"));
            QCOMPARE(page("window.pageWindowDraft").toString(), QStringLiteral("unfinished work"));
            return;
        }
        QVERIFY(evaluate("testWindow.activateBrowserWindow(" + separate + ".host," + separate + ".ui)").toBool());
        QTRY_COMPARE(QGuiApplication::focusWindow(), separateWindow);
        QTRY_COMPARE_WITH_TIMEOUT(groupState(), QStringLiteral("2|other-or-none|peer"), 15000);
        QTRY_COMPARE(text("feature_page_grouping_status"), QStringLiteral("available"));
        QCOMPARE(QGuiApplication::focusWindow(), separateWindow);
        QCOMPARE(page("window.pageWindowDraft").toString(), QStringLiteral("unfinished work"));
        if (scenario == "window-grouping-focus") {
            const auto peerView = evaluate(childHost + ".activeView").value<QObject *>();
            const auto peerPage = evaluate(entry + ".ui.tab_id_for_index(0)").toString();
            QVERIFY(evaluate("testWindow.activateBrowserWindow(" + childHost + "," + entry + ".ui)").toBool());
            QTRY_COMPARE(QGuiApplication::focusWindow(), childWindow);
            QVERIFY(evaluate(entry + ".ui.execute_command('tab-next ;; jseval --world page window.nativeGroupFocus=1')").toBool());
            evaluate("testWindow.completeBrowserKeyAction(" + entry + ".ui," + childHost + ")");
            pageReceiver = "testWindow.activeWebView()";
            QTRY_COMPARE(page("window.nativeGroupFocus").toInt(), 1);
            QTRY_COMPARE(QGuiApplication::focusWindow(), window);
            QTRY_COMPARE(groupState(), QStringLiteral("2|source|source"));
            QCOMPARE(evaluate("testWindow.activeWebView()").value<QObject *>(), sourceView.data());
            QVERIFY(command("tab-next ;; jseval --world page window.nativeGroupFocus=2"));
            evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
            pageReceiver = childHost + ".activeView";
            QTRY_COMPARE(page("window.nativeGroupFocus").toInt(), 2);
            QTRY_COMPARE(QGuiApplication::focusWindow(), childWindow);
            QTRY_COMPARE(groupState(), QStringLiteral("2|peer|peer"));
            QCOMPARE(evaluate(childHost + ".activeView").value<QObject *>(), peerView);
            QCOMPARE(evaluate(entry + ".ui.tab_id_for_index(0)").toString(), peerPage);
            pageReceiver = "testWindow.activeWebView()";
            QCOMPARE(page("window.pageWindowDraft").toString(), QStringLiteral("unfinished work"));
        }
        if (scenario == "window-grouping-order") {
            const auto nativeOrder = QStringLiteral(
                "return (function() local s=nil for _,w in ipairs(hl.get_windows()) do "
                "if w.pid==%1 and w.xdg_tag=='%2' then s=w end end "
                "if not s or not s.group then return 'missing' end local out={} "
                "for _,w in ipairs(s.group.members) do out[#out+1]=w.xdg_tag=='%2' and 'source' or w.xdg_tag=='%3' and 'peer' or 'unknown' end "
                "return table.concat(out,',') end)()")
                .arg(QCoreApplication::applicationPid()).arg(sourceTag,peerTag);
            QTRY_COMPARE(queryLua(nativeOrder), QStringLiteral("source,peer"));
            QVERIFY(evaluate("testWindow.activateBrowserWindow(" + childHost + "," + entry + ".ui)").toBool());
            QTRY_COMPARE(QGuiApplication::focusWindow(), childWindow);
            const auto peerPage = evaluate(entry + ".ui.tab_id_for_index(0)").toString();
            QPointer<QObject> peerView(evaluate(childHost + ".activeView").value<QObject *>());
            QSignalSpy focusChanges(qGuiApp, &QGuiApplication::focusWindowChanged);
            for (const auto &direction : {QStringLiteral("left"),QStringLiteral("right")}) {
                QVERIFY(evaluate(entry + ".ui.execute_command(" + quoted("tab-move " + peerPage + " " + direction) + ")").toBool());
                evaluate("testWindow.completeBrowserKeyAction(" + entry + ".ui," + childHost + ")");
                QTRY_COMPARE_WITH_TIMEOUT(queryLua(nativeOrder), direction == "left" ? QStringLiteral("peer,source") : QStringLiteral("source,peer"), 15000);
                QCOMPARE(QGuiApplication::focusWindow(), childWindow);
                QCOMPARE(focusChanges.size(), 0);
            }
            QVERIFY(evaluate(entry + ".ui.set_tab_pinned(0,true)").toInt() >= 0);
            QTRY_COMPARE_WITH_TIMEOUT(queryLua(nativeOrder), QStringLiteral("peer,source"), 15000);
            QCOMPARE(QGuiApplication::focusWindow(), childWindow);
            QCOMPARE(focusChanges.size(), 0);
            QCOMPARE(evaluate(childHost + ".activeView").value<QObject *>(), peerView.data());
            QCOMPARE(page("window.pageWindowDraft").toString(), QStringLiteral("unfinished work"));
        }
        if (scenario == "window-grouping-auto-off" || scenario == "window-grouping-detach" || scenario == "window-grouping-context") {
            const auto peerView = evaluate(childHost + ".activeView").value<QObject *>();
            const auto peerPage = evaluate(entry + ".ui.tab_id_for_index(0)").toString();
            if (scenario == "window-grouping-auto-off") {
                QVERIFY(evaluate(entry + ".ui.execute_command('set --temp hyprland.page_grouping=off')").toBool());
            } else if (scenario == "window-grouping-detach") {
                QVERIFY(evaluate(entry + ".ui.execute_command('tab-detach')").toBool());
                evaluate("testWindow.completeBrowserKeyAction(" + entry + ".ui," + childHost + ")");
            } else {
                QVERIFY(evaluate(entry + ".ui.assign_window_context('changed')").toBool());
            }
            QVERIFY(evaluate("testWindow.activateBrowserWindow(" + childHost + "," + entry + ".ui)").toBool());
            QTRY_COMPARE(QGuiApplication::focusWindow(), childWindow);
            QTRY_COMPARE_WITH_TIMEOUT(groupState(), QStringLiteral("waiting|peer|0"), 15000);
            QCOMPARE(evaluate(childHost + ".activeView").value<QObject *>(), peerView);
            QCOMPARE(evaluate(entry + ".ui.tab_id_for_index(0)").toString(), peerPage);
            QCOMPARE(evaluate("testWindow.activeWebView()").value<QObject *>(), sourceView.data());
            QCOMPARE(page("window.pageWindowDraft").toString(), QStringLiteral("unfinished work"));
            QCOMPARE(QGuiApplication::focusWindow(), childWindow);
        }
        return;
    }
    if (scenario == "window-native-identity") {
        QTemporaryDir reportDirectory;
        QVERIFY(reportDirectory.isValid());
        int reportIndex = 0;
        for (const auto &ui : {QString("testWindow.browserUi"), entry + ".ui"}) {
            QVERIFY(evaluate(ui + ".request_diagnostics()").toBool());
            QTRY_VERIFY(!evaluate(ui + ".diagnostics_pending").toBool());
            QVERIFY2(evaluate(ui + ".diagnostics_available").toBool(), qPrintable(evaluate(ui + ".diagnostics_error").toString()));
            // The preview is bounded text and may deliberately end mid-JSON.
            const auto path = reportDirectory.filePath(QString("identity-%1.json").arg(++reportIndex));
            QVERIFY(evaluate(ui + ".export_diagnostics(" + quoted(path) + ")").toBool());
            QTRY_VERIFY(QFileInfo(path).size() > 0);
            QFile savedReport(path);
            QVERIFY(savedReport.open(QIODevice::ReadOnly));
            QJsonParseError parseError;
            const auto report = QJsonDocument::fromJson(savedReport.readAll(), &parseError).object();
            QVERIFY2(parseError.error == QJsonParseError::NoError, qPrintable(parseError.errorString()));
            const auto native = report["native_page_window"].toObject();
            const auto host = ui == "testWindow.browserUi" ? QString("testWindow") : childHost;
            QCOMPARE(native["tag"].toString(), evaluate(host + ".pageWindowTag").toString());
            QCOMPARE(native["pid"].toInt(), QCoreApplication::applicationPid());
            QVERIFY(!native["app_id"].toString().isEmpty());
            const auto binding = report["native_page_window_binding"].toObject();
            QVERIFY2(binding["status"].toString() == "available", QJsonDocument(binding).toJson(QJsonDocument::Compact).constData());
            QVERIFY(binding["native_owner_matched"].toBool());
            QVERIFY(binding["stable_id"].toInteger() > 0);
        }
        return;
    }
    if (scenario.startsWith("window-regroup")) {
        const auto childId = evaluate(entry + ".ui.tab_id_for_index(0)").toString();
        const auto peerWindowId = evaluate(entry + ".ui.core_window_id").toString();
        QPointer<QObject> peerView(evaluate(childHost + ".activeView").value<QObject *>());
        QVERIFY(evaluate("testWindow.activateBrowserWindow(testWindow,testWindow.browserUi)").toBool());
        QTRY_COMPARE(QGuiApplication::focusWindow(), window);
        QVERIFY(command("tab-detach ;; tab-next ;; jseval --world page window.detachedIssuer=1"));
        evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
        pageReceiver = "testWindow.activeWebView()";
        QTRY_COMPARE(page("window.detachedIssuer").toInt(), 1);
        QTRY_COMPARE(QGuiApplication::focusWindow(), window);
        QVERIFY(!command("tab-pin " + childId + " on"));
        QVERIFY(!evaluate(entry + ".ui.tab_pinned_at(0)").toBool());
        if (scenario == "window-regroup-pin") {
            QVERIFY(evaluate("testWindow.browserUi.set_tab_pinned(0,true)>=0").toBool());
            QVERIFY(command("tab-give " + peerWindowId + " ;; tab-select 1 ;; jseval --world page window.currentPinOrder=4"));
            QVERIFY(evaluate("testWindow.browserUi.set_tab_pinned(0,false)>=0").toBool());
            evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
            pageReceiver = childHost + ".activeView";
            QTRY_COMPARE(page("window.currentPinOrder").toInt(), 4);
            QTRY_COMPARE(QGuiApplication::focusWindow(), childWindow);
        } else if (scenario == "window-regroup-background") {
            evaluate("testWindow.commandNoticeVisible=false;testWindow.commandNoticeText='';testWindow.browserWindowComponentObject=null");
            QVERIFY(command("tab-give " + peerWindowId + " ;; jseval --world page window.backgroundRegroup=1"));
            QVERIFY(command("open --target tab-bg " + initial + "?regroup-background=yes"));
            evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
            QTRY_COMPARE(evaluate("testWindow.tabViews.length").toInt(), 2);
            QTRY_VERIFY(evaluate("testWindow.commandNoticeVisible && testWindow.commandNoticeError").toBool());
            QVERIFY(page("typeof window.backgroundRegroup === 'undefined'").toBool());
            QVERIFY(!command("tab-pin " + childId + " on"));
        } else if (scenario == "window-regroup-stale") {
            evaluate("testWindow.commandNoticeVisible=false;testWindow.commandNoticeText=''");
            QVERIFY(command("tab-give " + peerWindowId + " ;; jseval --world page window.staleRegroup=1"));
            QVERIFY(evaluate(entry + ".ui.assign_window_context('changed')").toBool());
            evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
            QTRY_VERIFY(evaluate("testWindow.commandNoticeVisible && testWindow.commandNoticeError").toBool());
            QVERIFY(page("typeof window.staleRegroup === 'undefined'").toBool());
        } else {
            QVERIFY(command("tab-give " + peerWindowId + " ;; tab-next ;; jseval --world page window.givenPeer=2"));
            evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
            pageReceiver = childHost + ".activeView";
            QTRY_COMPARE(page("window.givenPeer").toInt(), 2);
            QTRY_COMPARE(QGuiApplication::focusWindow(), childWindow);
            QVERIFY(command("tab-next ;; tab-prev ;; tab-detach ;; tab-next ;; jseval --world page window.regroupedRoot=3"));
            evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
            pageReceiver = "testWindow.activeWebView()";
            QTRY_COMPARE(page("window.regroupedRoot").toInt(), 3);
            QTRY_COMPARE(QGuiApplication::focusWindow(), window);
            QVERIFY(page("typeof window.givenPeer === 'undefined'").toBool());
        }
        pageReceiver = "testWindow.activeWebView()";
        QCOMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(), 2);
        QCOMPARE(evaluate("testWindow.activeWebView()").value<QObject *>(), sourceView.data());
        QCOMPARE(evaluate(childHost + ".activeView").value<QObject *>(), peerView.data());
        QCOMPARE(evaluate("testWindow.browserUi.tab_id_for_index(0)").toString(), sourceId);
        QCOMPARE(evaluate(entry + ".ui.tab_id_for_index(0)").toString(), childId);
        QCOMPARE(page("window.pageWindowDraft").toString(), QStringLiteral("unfinished work"));
        return;
    }
    if (scenario.startsWith("window-undo")) {
        const bool expiry = scenario == "window-undo-private-expiry" || scenario == "window-undo-private-root-last";
        const bool primary = scenario == "window-undo-primary" || scenario == "window-undo-private-expiry";
        const auto closingHost = primary ? QStringLiteral("testWindow") : childHost;
        const auto closingUi = primary ? QStringLiteral("testWindow.browserUi") : entry + ".ui";
        engine->rootContext()->setContextProperty("undoSurvivorHost", evaluate(primary ? childHost : QStringLiteral("testWindow")).value<QObject *>());
        engine->rootContext()->setContextProperty("undoSurvivorUi", evaluate(primary ? entry + ".ui" : QStringLiteral("testWindow.browserUi")).value<QObject *>());
        const auto survivorHost = QStringLiteral("undoSurvivorHost");
        const auto survivorUi = QStringLiteral("undoSurvivorUi");
        auto *survivorWindow = primary ? childWindow : window;
        QPointer<QObject> survivorView(evaluate(survivorHost + (primary ? ".activeView" : ".activeWebView()")).value<QObject *>());
        QPointer<QObject> retiredView(evaluate(closingHost + (primary ? ".activeWebView()" : ".activeView")).value<QObject *>());
        pageReceiver = survivorHost + (primary ? ".activeView" : ".activeWebView()");
        page("window.undoSurvivorDraft=71");
        QVERIFY(evaluate(closingUi + ".set_tab_pinned(0,true)>=0").toBool());
        QVERIFY(evaluate(closingUi + ".set_tab_muted(0,true)").toBool());
        QVERIFY(evaluate(closingUi + ".execute_command('zoom 1.4')").toBool());
        evaluate(closingHost + (primary ? ".completeBrowserKeyAction(testWindow.browserUi,testWindow)" : ".applySwitcherEngineAction()"));
        QTRY_COMPARE(evaluate(closingHost + (primary ? ".activeWebView()" : ".activeView") + ".zoomFactor").toDouble(), 1.4);
        if (scenario == "window-undo-context") QVERIFY(evaluate(closingUi + ".assign_window_context('changed')").toBool());
        if (scenario == "window-undo-context") {
            // Capture and queue a checkpoint with both live pages. Closing in
            // another context must replace it automatically after acceptance.
            QVERIFY(evaluate(survivorUi + ".flush_durable_state()").toBool());
            QDirIterator checkpoints(evaluate("testWindow.storageBasePath").toString(), {"current*.json"}, QDir::Files, QDirIterator::Subdirectories);
            QVERIFY(checkpoints.hasNext());
            savedSelection = checkpoints.next();
        }
        if (expiry) {
            QVERIFY(command("open --target private-window " + destination + "-foreign"));
            evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
            QTRY_COMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(), 3);
            QTRY_VERIFY(evaluate("testWindow.liveWindowEntries().every(function(e){return e.host===testWindow ? !!e.host.activeWebView() : e.host.profileBootstrapComplete && !!e.host.activeView})").toBool());
            QVERIFY(evaluate("testWindow.liveWindowEntries().some(function(e){return e.ui.page_host_profile_id()!==undoSurvivorUi.page_host_profile_id()})").toBool());
        }
        QVERIFY(evaluate("testWindow.activateBrowserWindow(" + closingHost + "," + closingUi + ")").toBool());
        QTRY_COMPARE(QGuiApplication::focusWindow(), primary ? window : childWindow);
        evaluate(closingHost + ".close()");
        QTRY_VERIFY(evaluate(closingHost + (primary ? ".shutdownConfirmationVisible" : ".windowCloseConfirmationVisible")).toBool());
        QVERIFY(clickCloseDecision(primary ? window : childWindow, scenario != "window-undo-cancel"));
        if (scenario == "window-undo-cancel") {
            QTRY_VERIFY(!evaluate(childHost + ".windowCloseConfirmationVisible").toBool());
            QVERIFY(retiredView && childWindow->isVisible());
            QVERIFY(!evaluate(survivorUi + ".execute_command('tab-undo')").toBool());
            QCOMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(), 2);
            return;
        }
        QPointer<QQuickWindow> closingGuard(primary ? window : childWindow);
        QTRY_VERIFY(!closingGuard || !closingGuard->isVisible() || evaluate(closingHost + (primary ? ".shutdownPagePromptVisible" : ".windowShutdownPagePromptVisible")).toBool());
        if (closingGuard && closingGuard->isVisible() && evaluate(closingHost + (primary ? ".shutdownPagePromptVisible" : ".windowShutdownPagePromptVisible")).toBool())
            QVERIFY(clickCloseDecision(closingGuard, true));
        QTRY_VERIFY(retiredView.isNull());
        QTRY_COMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(), expiry ? 2 : 1);
        if (expiry) {
            QVERIFY(evaluate("testWindow.activateBrowserWindow(" + survivorHost + "," + survivorUi + ")").toBool());
            QTRY_COMPARE(QGuiApplication::focusWindow(), survivorWindow);
            evaluate(survivorHost + ".close()");
            QTRY_VERIFY(evaluate(survivorHost + (primary ? ".windowCloseConfirmationVisible" : ".shutdownConfirmationVisible")).toBool());
            QVERIFY(clickCloseDecision(survivorWindow, true));
            QPointer<QQuickWindow> survivorGuard(survivorWindow);
            QTRY_VERIFY(!survivorGuard || !survivorGuard->isVisible() || evaluate(survivorHost + (primary ? ".windowShutdownPagePromptVisible" : ".shutdownPagePromptVisible")).toBool());
            if (survivorGuard && survivorGuard->isVisible() && evaluate(survivorHost + (primary ? ".windowShutdownPagePromptVisible" : ".shutdownPagePromptVisible")).toBool())
                QVERIFY(clickCloseDecision(survivorGuard, true));
            QTRY_VERIFY(survivorView.isNull());
            QTRY_COMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(), 1);
            QVERIFY(!command("tab-undo"));
            QVERIFY(!evaluate("testWindow.liveWindowEntries()[0].ui.execute_command('tab-undo')").toBool());
            return;
        }
        if (scenario == "window-undo-context") {
            const auto checkpoint = [&]() {
                QFile file(savedSelection.toString());
                if (!file.open(QIODevice::ReadOnly)) return QJsonObject();
                return QJsonDocument::fromJson(file.readAll()).object();
            };
            QTRY_VERIFY(checkpoint()["windows"].toArray().size() == 1 && checkpoint()["closed_tabs"].toArray().size() == 1);
            QCOMPARE(checkpoint()["closed_tabs"].toArray()[0].toObject()["context"].toString(), QStringLiteral("changed"));
            QVERIFY(!evaluate(survivorUi + ".execute_command('tab-undo')").toBool());
            QVERIFY(evaluate(survivorUi + ".assign_window_context('changed')").toBool());
        }
        if (scenario == "window-undo-peer" || scenario == "window-undo-context") {
            QVERIFY(evaluate(survivorUi + ".save_named_session('closed-page-window')").toBool());
            QTRY_COMPARE(evaluate(survivorUi + ".session_save_state").toString(), QStringLiteral("succeeded"));
            QDirIterator files(evaluate("testWindow.storageBasePath").toString(), {"closed-page-window.json"}, QDir::Files, QDirIterator::Subdirectories);
            QVERIFY(files.hasNext()); QFile saved(files.next()); QVERIFY(saved.open(QIODevice::ReadOnly));
            const auto snapshot = QJsonDocument::fromJson(saved.readAll()).object();
            QCOMPARE(snapshot["schema_version"].toInt(), 6);
            QCOMPARE(snapshot["windows"].toArray().size(), 1);
            const auto closed = snapshot["closed_tabs"].toArray();
            QCOMPARE(closed.size(), 1);
            QVERIFY(!closed[0].toObject()["group"].toString().isEmpty());
            QCOMPARE(closed[0].toObject()["context"].toString(), scenario == "window-undo-context" ? QStringLiteral("changed") : QString());
        }
        QPointer<QObject> reentrantUndo;
        if (scenario == "window-undo-peer") {
            savedSelection = QVariant();
            reentrantUndo = evaluate("Qt.createQmlObject('import QtQuick; Connections { property bool armed:true; target:undoSurvivorUi; function onTab_countChanged(){if(armed && target.tab_count>1){armed=false; testProbe.savedSelection=target.execute_command(\"tab-undo\");}} }',testWindow)").value<QObject *>();
            QVERIFY(reentrantUndo);
        }
        QVERIFY(evaluate(survivorUi + ".execute_command('tab-undo')").toBool());
        evaluate(primary ? survivorHost + ".applySwitcherEngineAction()" : "testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
        if (reentrantUndo) {
            QTRY_VERIFY(!reentrantUndo->property("armed").toBool());
            QVERIFY(!savedSelection.toBool());
            delete reentrantUndo.data();
        }
        QTRY_COMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(), 2);
        const auto reopened = QStringLiteral("testWindow.liveWindowEntries().filter(function(e){return e.host!==") + survivorHost + "})[0]";
        QTRY_VERIFY(evaluate("!!" + reopened + ".host.activeView").toBool());
        pageReceiver = reopened + ".host.activeView";
        QTRY_COMPARE(page("location.href").toString(), primary ? initial : destination);
        QVERIFY(evaluate(reopened + ".ui.tab_pinned_at(0)").toBool());
        QVERIFY(evaluate(reopened + ".ui.tab_muted_at(0)").toBool());
        QCOMPARE(evaluate("JSON.parse(" + reopened + ".ui.tab_transfer_payload(" + reopened + ".ui.tab_id_for_index(0))).zoom_hundredths").toInt(), 140);
        QTRY_VERIFY2(qAbs(evaluate(reopened + ".host.activeView.zoomFactor").toDouble() - 1.4) < 0.00001,
            qPrintable(evaluate("JSON.stringify({model:" + reopened + ".host.activeView.viewModel.zoom, settings:" + reopened + ".host.activeView.effectiveSiteSettings})").toString()));
        QVERIFY(page("typeof window.pageWindowDraft === 'undefined'").toBool());
        QVERIFY(!evaluate(survivorUi + ".execute_command('tab-undo')").toBool());
        QVERIFY(survivorView);
        pageReceiver = survivorHost + (primary ? ".activeView" : ".activeWebView()");
        QCOMPARE(page("window.undoSurvivorDraft").toInt(), 71);
        QVERIFY(survivorWindow->isVisible());
        return;
    }
    if (scenario == "session-aggregate") {
        QVERIFY(evaluate(entry + ".ui.assign_window_context('changed')").toBool());
        QVERIFY(evaluate(entry + ".ui.set_tab_muted(0,true)").toBool());
        QVERIFY(evaluate("testWindow.browserUi.set_tab_pinned(0,true)>=0").toBool());
        QVERIFY(evaluate("testWindow.browserUi.save_named_session('all-page-windows')").toBool());
        QTRY_COMPARE(text("session_save_state"), QStringLiteral("succeeded"));
        QDirIterator files(evaluate("testWindow.storageBasePath").toString(), {"all-page-windows.json"}, QDir::Files, QDirIterator::Subdirectories);
        QVERIFY(files.hasNext()); QFile saved(files.next()); QVERIFY(saved.open(QIODevice::ReadOnly));
        const auto snapshot = QJsonDocument::fromJson(saved.readAll()).object();
        QCOMPARE(snapshot["schema_version"].toInt(), 6);
        const auto groups = snapshot["windows"].toArray();
        QCOMPARE(groups.size(), 2);
        QSet<QString> addresses;
        bool pinned = false, muted = false, context = false;
        for (const auto &value : groups) {
            const auto group = value.toObject();
            QVERIFY(!group["group"].toString().isEmpty());
            const auto tabs = group["tabs"].toArray();
            QCOMPARE(tabs.size(), 1);
            const auto tab = tabs[0].toObject();
            addresses.insert(tab["safe_restore_url"].toString());
            pinned |= tab["pinned"].toBool(); muted |= tab["muted"].toBool();
            context |= group["context"].toString() == "changed";
        }
        QVERIFY(addresses.contains(initial) && addresses.contains(destination));
        QVERIFY(pinned && muted && context);
        QVERIFY(evaluate("testWindow.browserUi.request_named_session_load('all-page-windows',true)").toBool());
        QTRY_COMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(), 4);
        QTRY_VERIFY(evaluate("testWindow.liveWindowEntries().every(function(e){return !!(e.host===testWindow?testWindow.activeWebView():e.host.activeView) && e.ui.tab_count===1})").toBool());
        QCOMPARE(evaluate("testWindow.liveWindowEntries().filter(function(e){return e.ui.context_name==='changed'}).length").toInt(), 2);
        QCOMPARE(evaluate("testWindow.activeWebView()").value<QObject *>(), sourceView.data());
        QCOMPARE(page("window.pageWindowDraft").toString(), QStringLiteral("unfinished work"));
        // A retained restored page must subsequently persist its live context,
        // rather than the staging metadata from the file just loaded.
        QVERIFY(evaluate("testWindow.browserUi.request_named_session_load('all-page-windows',false)").toBool());
        QTRY_COMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(), 5);
        QTRY_COMPARE(text("context_name"), QString());
        QVERIFY(evaluate("testWindow.browserUi.assign_window_context('changed')").toBool());
        QVERIFY(evaluate("testWindow.browserUi.save_named_session('changed-keeper')").toBool());
        QTRY_COMPARE(text("session_save_state"), QStringLiteral("succeeded"));
        QDirIterator changed(evaluate("testWindow.storageBasePath").toString(), {"changed-keeper.json"}, QDir::Files, QDirIterator::Subdirectories);
        QVERIFY(changed.hasNext()); QFile keeper(changed.next()); QVERIFY(keeper.open(QIODevice::ReadOnly));
        const auto keeperGroups = QJsonDocument::fromJson(keeper.readAll()).object()["windows"].toArray();
        QCOMPARE(keeperGroups[0].toObject()["context"].toString(), QStringLiteral("changed"));
        return;
    }
    if (scenario.startsWith("popup-")) {
        // Keep two existing group members: profile-wide fallback would mask a
        // missing native popup group if the opener were the only member.
        QVERIFY(evaluate("testWindow.activateBrowserWindow(testWindow,testWindow.browserUi)").toBool());
        QTRY_COMPARE(QGuiApplication::focusWindow(), window);
        pageReceiver = "testWindow.activeWebView()";
        page("window.popupGroupMessage='';window.addEventListener('message',e=>window.popupGroupMessage=e.data);true");
        if (scenario == "popup-dialog")
            page("document.getElementById('blank').onclick=function(e){e.preventDefault();window.open('/popup.html','dialog','popup=1,width=400,height=300')};true");
        const auto point = page("(()=>{const r=document.getElementById('blank').getBoundingClientRect();return [r.x+r.width/2,r.y+r.height/2]})()").toList();
        QCOMPARE(point.size(), 2);
        auto *viewItem = qobject_cast<QQuickItem *>(sourceView.data());
        QVERIFY(viewItem);
        const auto modifiers = scenario == "popup-window" ? Qt::ShiftModifier
            : scenario == "popup-background" ? Qt::ControlModifier : Qt::NoModifier;
        QTest::mouseClick(window, Qt::LeftButton, modifiers,
            viewItem->mapToScene(QPointF(point[0].toDouble(), point[1].toDouble())).toPoint());
        QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(), 1);
        const auto popup = QStringLiteral("testWindow.popupWindowRegistry[0]");
        auto *popupWindow = qobject_cast<QQuickWindow *>(evaluate(popup + ".host").value<QObject *>());
        QVERIFY(popupWindow);
        QTRY_VERIFY(evaluate(popup + ".host.profileBootstrapComplete && !!" + popup + ".host.activeView").toBool());
        QCOMPARE(evaluate(popup + ".host.windowOrdinaryPage").toBool(), scenario != "popup-dialog");
        if (scenario == "popup-background") QVERIFY(!popupWindow->isVisible());
        pageReceiver = popup + ".host.activeView";
        QTRY_COMPARE(page("location.pathname").toString(), QStringLiteral("/popup.html"));
        const bool keepsOpener = scenario == "popup-tab-group" || scenario == "popup-dialog";
        QCOMPARE(page("!!window.opener").toBool(), keepsOpener);
        page("if(window.opener)window.opener.postMessage('popup-group-verified','*');window.popupGroupDraft=19");
        QVERIFY(evaluate("testWindow.activateBrowserWindow(testWindow,testWindow.browserUi)").toBool());
        QTRY_COMPARE(QGuiApplication::focusWindow(), window);
        QVERIFY(command("tab-select last"));
        evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
        QTRY_COMPARE(QGuiApplication::focusWindow(), scenario == "popup-dialog" || scenario == "popup-window" ? childWindow : popupWindow);
        pageReceiver = "testWindow.activeWebView()";
        if (keepsOpener) QTRY_COMPARE(page("window.popupGroupMessage").toString(), QStringLiteral("popup-group-verified"));
        QCOMPARE(page("window.pageWindowDraft").toString(), QStringLiteral("unfinished work"));
        QCOMPARE(evaluate("testWindow.activeWebView()").value<QObject *>(), sourceView.data());
        pageReceiver = popup + ".host.activeView";
        QCOMPARE(page("window.popupGroupDraft").toInt(), 19);
        return;
    }
    if (scenario.startsWith("window-order")) {
        const auto childId = evaluate(entry + ".ui.tab_id_for_index(0)").toString();
        QVERIFY(evaluate("testWindow.activateBrowserWindow(testWindow,testWindow.browserUi)").toBool());
        QTRY_COMPARE(QGuiApplication::focusWindow(), window);
        if (scenario.startsWith("window-order-fallback")) {
            evaluate("testWindow.browserWindowComponentObject=null");
            QVERIFY(command("tab-open " + initial + "?fallback-order=yes"));
            evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
            QTRY_COMPARE(evaluate("testWindow.tabViews.length").toInt(), 2);
            const auto fallbackId = evaluate("testWindow.browserUi.tab_id_for_index(1)").toString();
            QPointer<QObject> fallbackView(evaluate("testWindow.tabViewAt(1)").value<QObject *>());
            QVERIFY(fallbackView);
            pageReceiver = "testWindow.activeWebView()";
            QTRY_COMPARE(page("location.search").toString(), QStringLiteral("?fallback-order=yes"));
            QPointer<QObject> thirdView;
            if (scenario == "window-order-fallback-pin") {
                QVERIFY(command("tab-open " + initial + "?third-order=yes"));
                evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
                QTRY_COMPARE(evaluate("testWindow.tabViews.length").toInt(), 3);
                thirdView = evaluate("testWindow.tabViewAt(2)").value<QObject *>();
                QVERIFY(thirdView);
                QTRY_COMPARE(page("location.search").toString(), QStringLiteral("?third-order=yes"));
                QVERIFY(command("tab-pin " + fallbackId + " on ;; tab-pin " + fallbackId + " off ;; jseval --world page window.pinSync=1"));
                evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
                QTRY_COMPARE(page("window.pinSync").toInt(), 1);
                QCOMPARE(evaluate("testWindow.browserUi.tab_id_for_index(1)").toString(), fallbackId);
            }
            QVERIFY(command("tab-move " + fallbackId + " left ;; jseval --world page window.orderStep=1"));
            evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
            QTRY_COMPARE(page("window.orderStep").toInt(), 1);
            QVERIFY(command("tab-move " + fallbackId + " left ;; jseval --world page window.orderStep=2"));
            evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
            QTRY_COMPARE(evaluate("testWindow.browserUi.tab_id_for_index(0)").toString(), fallbackId);
            pageReceiver = "testWindow.activeWebView()";
            QTRY_COMPARE(page("window.orderStep").toInt(), 2);
            QCOMPARE(evaluate("testWindow.tabViewAt(0)").value<QObject *>(), fallbackView.data());
            QCOMPARE(evaluate("testWindow.tabViewAt(1)").value<QObject *>(), sourceView.data());
            if (thirdView) QCOMPARE(evaluate("testWindow.tabViewAt(2)").value<QObject *>(), thirdView.data());
            QVERIFY(evaluate(entry + ".ui.execute_command('tab-prev ;; jseval --world page window.fallbackOrderTarget=1')").toBool());
            evaluate(childHost + ".applySwitcherEngineAction()");
            pageReceiver = "testWindow.tabViewAt(1)";
            QTRY_COMPARE(page("window.fallbackOrderTarget").toInt(), 1);
            QCOMPARE(page("window.pageWindowDraft").toString(), QStringLiteral("unfinished work"));
            return;
        }
        for (int step = 1; step <= 3; ++step) {
            QVERIFY(command("tab-move " + childId + (step == 2 ? " right" : " left")
                + " ;; jseval --world page window.orderMove=" + QString::number(step)));
            evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
            pageReceiver = "testWindow.activeWebView()";
            QTRY_COMPARE(page("window.orderMove").toInt(), step);
            QVERIFY(command("tab-select 1 ;; jseval --world page window.selectedOrder=" + QString::number(step)));
            evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
            pageReceiver = step == 2 ? "testWindow.activeWebView()" : childHost + ".activeView";
            QTRY_COMPARE(page("window.selectedOrder").toInt(), step);
            QVERIFY(evaluate("testWindow.activateBrowserWindow(testWindow,testWindow.browserUi)").toBool());
            QTRY_COMPARE(QGuiApplication::focusWindow(), window);
        }
        QVERIFY(command("tab-pin " + sourceId + " on ;; tab-move " + childId + " left ;; tab-select 1 ;; jseval --world page window.pinOrderBoundary=1"));
        evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
        pageReceiver = "testWindow.activeWebView()";
        QTRY_COMPARE(page("window.pinOrderBoundary").toInt(), 1);
        QVERIFY(command("tab-pin " + sourceId + " off ;; jseval --world page window.orderMove=4"));
        evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
        QTRY_COMPARE(page("window.orderMove").toInt(), 4);
        QVERIFY(evaluate("testWindow.browserUi.save_named_session('ordered-pages')").toBool());
        QTRY_COMPARE(text("session_save_state"), QStringLiteral("succeeded"));
        QDirIterator files(evaluate("testWindow.storageBasePath").toString(), {"ordered-pages.json"}, QDir::Files, QDirIterator::Subdirectories);
        QVERIFY(files.hasNext()); QFile saved(files.next()); QVERIFY(saved.open(QIODevice::ReadOnly));
        const auto group = QJsonDocument::fromJson(saved.readAll()).object()["windows"].toArray()[0].toObject();
        const auto ordered = group["tabs"].toArray();
        QCOMPARE(ordered.size(), 2);
        QCOMPARE(ordered[0].toObject()["safe_restore_url"].toString(), destination);
        QCOMPARE(ordered[1].toObject()["safe_restore_url"].toString(), initial);
        QCOMPARE(group["selected_tab"].toString(), ordered[1].toObject()["id"].toString());
        engine->rootContext()->setContextProperty("originalOrderChildUi", evaluate(entry + ".ui").value<QObject *>());
        QVERIFY(evaluate("testWindow.browserUi.request_named_session_load('ordered-pages',true)").toBool());
        QTRY_COMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(), 4);
        const auto restored = QStringLiteral("testWindow.liveWindowEntries().filter(function(e){return e.ui!==testWindow.browserUi && e.ui!==originalOrderChildUi})");
        QTRY_VERIFY(evaluate(restored + ".every(function(e){return e.host.activeView && !e.host.activeView.loading})").toBool());
        const auto restoredFirst = restored + ".find(function(e){return e.host.activeView.url.toString()===" + quoted(destination) + "})";
        const auto restoredSelected = restored + ".find(function(e){return e.host.activeView.url.toString()===" + quoted(initial) + "})";
        QTRY_VERIFY(evaluate("!!(" + restoredFirst + ") && !!(" + restoredSelected + ")").toBool());
        auto *firstRestoredWindow = qobject_cast<QQuickWindow *>(evaluate(restoredFirst + ".host").value<QObject *>());
        QVERIFY(firstRestoredWindow);
        QVERIFY(evaluate(restoredSelected + ".ui.execute_command('tab-select 1')").toBool());
        evaluate(restoredSelected + ".host.applySwitcherEngineAction()");
        QTRY_COMPARE(QGuiApplication::focusWindow(), firstRestoredWindow);
        pageReceiver = "testWindow.activeWebView()";
        QCOMPARE(page("window.pageWindowDraft").toString(), QStringLiteral("unfinished work"));
        return;
    }
    if (scenario.startsWith("window-mutation")) {
        QVERIFY(evaluate("testWindow.activateBrowserWindow(testWindow,testWindow.browserUi)").toBool());
        QTRY_COMPARE(QGuiApplication::focusWindow(), window);
        const auto childId = evaluate(entry + ".ui.tab_id_for_index(0)").toString();
        if (scenario != "window-mutations") {
            evaluate("testWindow.commandNoticeVisible=false;testWindow.commandNoticeText=''");
            QVERIFY(command("tab-mute " + childId + " on ;; jseval --world page window.wrongMutationDoc=1"));
            if (scenario == "window-mutation-discard") delete childWindow;
            else {
                const auto replacement = initial + "?mutation-new-document=yes";
                evaluate("testWindow.browserUi.navigate_without_context_route(" + quoted(replacement) + ")");
                pageReceiver = "testWindow.activeWebView()";
                QTRY_COMPARE(page("location.href").toString(), replacement);
                QTRY_VERIFY(evaluate(entry + ".ui.tab_muted_at(0)").toBool());
            }
            evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
            QTRY_VERIFY(evaluate("testWindow.commandNoticeVisible && testWindow.commandNoticeError").toBool());
            pageReceiver = "testWindow.activeWebView()";
            QVERIFY(page("typeof window.wrongMutationDoc === 'undefined'").toBool());
            QTRY_COMPARE(QGuiApplication::focusWindow(), window);
            return;
        }
        QVERIFY(command("tab-pin " + childId + " on"));
        evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
        QTRY_VERIFY(evaluate(entry + ".ui.tab_pinned_at(0)").toBool());
        QCOMPARE(QGuiApplication::focusWindow(), window);
        QVERIFY(command("tab-mute " + childId + " on ;; jseval --world page window.mutationIssuer=99"));
        evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
        QTRY_VERIFY(evaluate(childHost + ".activeView.audioMuted").toBool());
        pageReceiver = "testWindow.activeWebView()";
        QTRY_COMPARE(page("window.mutationIssuer").toInt(), 99);
        pageReceiver = childHost + ".activeView";
        QVERIFY(page("typeof window.mutationIssuer === 'undefined'").toBool());
        QCOMPARE(QGuiApplication::focusWindow(), window);
        QVERIFY(command("tab-select 1"));
        evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
        QTRY_COMPARE(QGuiApplication::focusWindow(), childWindow);
        QVERIFY(command("tab-pin " + childId + " off"));
        evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
        QTRY_VERIFY(!evaluate(entry + ".ui.tab_pinned_at(0)").toBool());
        QCOMPARE(QGuiApplication::focusWindow(), childWindow);
        QVERIFY(command("tab-focus " + childId + " ;; tab-pin " + sourceId + " on ;; jseval --world page window.mutationChain=11"));
        evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
        pageReceiver = childHost + ".activeView";
        QTRY_COMPARE(page("window.mutationChain").toInt(), 11);
        QVERIFY(evaluate("testWindow.browserUi.tab_pinned_at(0)").toBool());
        QCOMPARE(QGuiApplication::focusWindow(), childWindow);
        pageReceiver = "testWindow.activeWebView()";
        QVERIFY(page("typeof window.mutationChain === 'undefined'").toBool());
        QCOMPARE(page("window.pageWindowDraft").toString(), QStringLiteral("unfinished work"));
        QVERIFY(command("tab-mute " + childId + " off"));
        evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
        QTRY_VERIFY(!evaluate(childHost + ".activeView.audioMuted").toBool());
        evaluate("testWindow.commandNoticeVisible=false;testWindow.commandNoticeText=''");
        QVERIFY(command("tab-mute " + childId + " on"));
        QVERIFY(evaluate(entry + ".ui.assign_window_context('changed')").toBool());
        evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
        QTRY_VERIFY(evaluate("testWindow.commandNoticeVisible && testWindow.commandNoticeError").toBool());
        QVERIFY(!evaluate(entry + ".ui.tab_muted_at(0)").toBool());
        return;
    }
    if (scenario.startsWith("window-chain-")) {
        QVERIFY(evaluate("testWindow.activateBrowserWindow(testWindow,testWindow.browserUi)").toBool());
        QTRY_COMPARE(QGuiApplication::focusWindow(), window);
        if (scenario == "window-chain-workflows") {
            QVERIFY(!command("tab-next ;; command-execute unknown ;; jseval --world page window.wrongWorkflowPage=1"));
            QVERIFY(command("macro-record a"));
            QVERIFY(command("tab-next"));
            evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
            QTRY_COMPARE(QGuiApplication::focusWindow(), childWindow);
            QVERIFY(evaluate(entry + ".ui.macro_status_text").toString().contains("recording macro @a (1)"));
            QVERIFY(evaluate(entry + ".ui.execute_command('tab-next')").toBool());
            evaluate("testWindow.completeBrowserKeyAction(" + entry + ".ui," + childHost + ")");
            QTRY_COMPARE(QGuiApplication::focusWindow(), window);
            QVERIFY(evaluate(entry + ".ui.execute_command('macro-stop')").toBool());
            QTRY_VERIFY(text("macro_status_text").contains("@a:2"));
            QVERIFY(evaluate(entry + ".ui.macro_status_text").toString().contains("@a:2"));
            QVERIFY(command("repeat --count 2"));
            evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
            QTest::qWait(250);
            QCOMPARE(QGuiApplication::focusWindow(), window);
            QVERIFY(command("macro-play a"));
            evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
            QTest::qWait(250);
            QCOMPARE(QGuiApplication::focusWindow(), window);
            QVERIFY(evaluate("testWindow.browserUi.switcher_query('tab-prev','commands')").toBool());
            const auto previousCommand = evaluate("(function(){var ui=testWindow.browserUi;for(var i=0;i<ui.switcher_result_labels.length;++i)if(ui.switcher_result_labels[i]==='tab-prev')return ui.switcher_result_ids[i];return ''})()").toString();
            QVERIFY(!previousCommand.isEmpty());
            int marker = 0;
            for (const auto &workflow : {QString("repeat"), QString("macro-play a"), QString("action tab previous"), QString("command-execute ") + previousCommand}) {
                QVERIFY(command(QString("tab-next ;; %1 ;; jseval --world page window.nestedWorkflow=%2").arg(workflow).arg(++marker)));
                evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
                const bool child = workflow == "macro-play a";
                pageReceiver = child ? childHost + ".activeView" : "testWindow.activeWebView()";
                QTRY_COMPARE(page("window.nestedWorkflow").toInt(), marker);
                QTRY_COMPARE(QGuiApplication::focusWindow(), child ? childWindow : window);
            }
            QVERIFY(evaluate(entry + ".ui.assign_window_context('changed')").toBool());
            QTRY_VERIFY(evaluate(entry + ".ui.macro_status_text").toString().isEmpty());
            QVERIFY(!evaluate(entry + ".ui.execute_command('macro-play a')").toBool());
            QVERIFY(text("macro_status_text").contains("@a:2"));
            for (const auto &receiver : {QString("testWindow.activeWebView()"), childHost + ".activeView"}) {
                pageReceiver = receiver;
                QVERIFY(page("typeof window.wrongWorkflowPage === 'undefined'").toBool());
            }
            QVERIFY(evaluate("testWindow.browserUi.switcher_query('target-search','commands')").toBool());
            const auto searchCommand = evaluate("(function(){var ui=testWindow.browserUi;for(var i=0;i<ui.switcher_result_labels.length;++i)if(ui.switcher_result_labels[i]==='target-search')return ui.switcher_result_ids[i];return ''})()").toString();
            QVERIFY(!searchCommand.isEmpty());
            QVERIFY(command("command-execute " + searchCommand));
        } else if (scenario == "window-chain-multi-hop") {
            QVERIFY(command("tab-next ;; tab-prev ;; jseval --world page window.multiHop=1 ;; jseval --world page window.multiHop+=1 ;; jseval --world page window.multiHop+=1"));
            evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
            pageReceiver = "testWindow.activeWebView()";
            QTRY_COMPARE(page("window.multiHop").toInt(), 3);
            // Allow both native selection receipts to drain; an older hop
            // must never reclaim focus after the final owner.
            QTest::qWait(250);
            QCOMPARE(QGuiApplication::focusWindow(), window);
            pageReceiver = childHost + ".activeView";
            QVERIFY(page("typeof window.multiHop === 'undefined'").toBool());
        } else {
            evaluate("testWindow.commandNoticeVisible=false;testWindow.commandNoticeText=''");
            QVERIFY(command(scenario == "window-chain-discard" ? "tab-next ;; jseval --world page window.discardedHop=1" : "tab-next ;; tab-select 999999"));
            if (scenario == "window-chain-discard") {
                // Destruction discards the queued CxxQtThread callback. Its
                // receipt still owes the original owner a terminal failure.
                delete childWindow;
            }
            evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
            QTRY_VERIFY(evaluate("testWindow.commandNoticeVisible && testWindow.commandNoticeError").toBool());
            QTest::qWait(250);
            QCOMPARE(QGuiApplication::focusWindow(), window);
            pageReceiver = "testWindow.activeWebView()";
            QVERIFY(page("typeof window.discardedHop === 'undefined'").toBool());
        }
        return;
    }
    if (scenario == "window-selection") {
        // Exercise the production command entry and queue consumer, including
        // a continuation that must execute on the selected native owner.
        pageReceiver = childHost + ".activeView";
        page("window.pageWindowDraft = 'child work'");
        QVERIFY(evaluate("testWindow.activateBrowserWindow(testWindow,testWindow.browserUi)").toBool());
        QTRY_COMPARE(QGuiApplication::focusWindow(), window);
        QTRY_VERIFY(evaluate("testWindow.browserKeyFocusActive").toBool());
        sendOwnedBrowserKey(window, Qt::Key_3);
        sendOwnedBrowserKey(window, Qt::Key_PageDown, Qt::ControlModifier);
        QTRY_COMPARE(QGuiApplication::focusWindow(), childWindow);
        QVERIFY(evaluate(entry + ".ui.execute_command('tab-prev')").toBool());
        evaluate(childHost + ".applySwitcherEngineAction()");
        QTRY_COMPARE(QGuiApplication::focusWindow(), window);
        QVERIFY(command("tab-select previous"));
        evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
        QTRY_COMPARE(QGuiApplication::focusWindow(), childWindow);
        QVERIFY(command("tab-select 1 ;; jseval --world page window.selectedPageCommand=7"));
        evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
        QTRY_COMPARE(QGuiApplication::focusWindow(), window);
        pageReceiver = "testWindow.activeWebView()";
        QTRY_COMPARE(page("window.selectedPageCommand").toInt(), 7);
        QCOMPARE(page("window.pageWindowDraft").toString(), QStringLiteral("unfinished work"));
        pageReceiver = childHost + ".activeView";
        QVERIFY(page("typeof window.selectedPageCommand === 'undefined'").toBool());
        QCOMPARE(page("window.pageWindowDraft").toString(), QStringLiteral("child work"));
        QVERIFY(command("tab-focus " + requestedId));
        evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
        QTRY_COMPARE(QGuiApplication::focusWindow(), childWindow);
        QVERIFY(!command("tab-select 0"));
        QVERIFY(!command("tab-focus tabid-999999999"));
        QVERIFY(evaluate("testWindow.activateBrowserWindow(testWindow,testWindow.browserUi)").toBool());
        QTRY_COMPARE(QGuiApplication::focusWindow(), window);
        evaluate("testWindow.commandNoticeVisible=false;testWindow.commandNoticeText=''");
        const auto replacement = initial + "?replacement-before-continuation=yes";
        QVERIFY(evaluate("(function(){var accepted=testWindow.browserUi.execute_command('tab-next ;; jseval --world page window.staleWindowCommand=9');" + entry + ".ui.navigate_without_context_route(" + quoted(replacement) + ");return accepted})()").toBool());
        evaluate(childHost + ".applySwitcherEngineAction()");
        pageReceiver = childHost + ".activeView";
        QTRY_COMPARE(page("location.href").toString(), replacement);
        QVERIFY(page("typeof window.staleWindowCommand === 'undefined'").toBool());
        QTRY_VERIFY(evaluate("testWindow.commandNoticeVisible && testWindow.commandNoticeError && testWindow.commandNoticeText.indexOf('changed')>=0").toBool());
        // Selection itself emits synchronous Qt signals. A navigation from
        // one of those callbacks must also revoke the document continuation.
        engine->rootContext()->setContextProperty("testSelectedPageUi", evaluate(entry + ".ui").value<QObject *>());
        const auto duringSelection = initial + "?replacement-during-selection=yes";
        const auto connectionQml = QString("import QtQuick; Connections { target: testSelectedPageUi; property bool armed: true; function onInput_epochChanged() { if(armed) { armed=false; testSelectedPageUi.navigate_without_context_route(%1) } } }").arg(quoted(duringSelection));
        QPointer<QObject> selectionObserver(evaluate("Qt.createQmlObject(" + quoted(connectionQml) + ",testWindow)").value<QObject *>());
        QVERIFY(selectionObserver);
        QVERIFY(command("tab-next ;; jseval --world page window.reentrantWindowCommand=9"));
        evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
        QTRY_COMPARE(page("location.href").toString(), duringSelection);
        QVERIFY(page("typeof window.reentrantWindowCommand === 'undefined'").toBool());
        delete selectionObserver.data();
        // A standalone page is excluded while this logical group has peers.
        QVERIFY(command("open --target window " + destination + "-standalone"));
        evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
        QTRY_COMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(), 3);
        QVERIFY(evaluate("testWindow.activateBrowserWindow(testWindow,testWindow.browserUi)").toBool());
        QTRY_COMPARE(QGuiApplication::focusWindow(), window);
        QVERIFY(command("tab-next --count 3"));
        evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
        QTRY_COMPARE(QGuiApplication::focusWindow(), childWindow);
        // Allocation fallback can retain two actual views in one owner. A
        // reentrant selection must not redirect a captured document command.
        evaluate("testWindow.browserWindowComponentObject=null");
        const auto fallback = initial + "?fallback-active-page=yes";
        QVERIFY(command("tab-open " + fallback));
        evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
        QTRY_COMPARE(evaluate("testWindow.browserUi.tab_count").toInt(), 2);
        QTRY_COMPARE(evaluate("testWindow.tabViews.length").toInt(), 2);
        pageReceiver = "testWindow.tabViewAt(1)";
        QTRY_COMPARE(page("location.href").toString(), fallback);
        evaluate("testWindow.commandNoticeVisible=false;testWindow.commandNoticeText=''");
        QTRY_COMPARE(evaluate("testWindow.browserUi.active_tab_index").toInt(), 1);
        const auto reselectionQml = QStringLiteral("import QtQuick; Connections { target: testWindow.browserUi; property bool armed: true; function onActive_tab_indexChanged() { if(armed && target.active_tab_index===0) { armed=false; target.select_tab(1) } } }");
        QPointer<QObject> reselectionObserver(evaluate("Qt.createQmlObject(" + quoted(reselectionQml) + ",testWindow)").value<QObject *>());
        QVERIFY(reselectionObserver);
        QVERIFY(command("tab-select 1 ;; jseval --world page window.redirectedWindowCommand=9"));
        evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
        QTRY_VERIFY(!reselectionObserver->property("armed").toBool());
        // Drain the queued receipt before inspecting both unchanged documents.
        QTRY_VERIFY2(evaluate("testWindow.commandNoticeError && testWindow.commandNoticeText.indexOf('changed')>=0").toBool(),
            qPrintable(evaluate("JSON.stringify({notice:testWindow.commandNoticeText,error:testWindow.commandNoticeError,status:testWindow.browserUi.status_text,active:testWindow.browserUi.active_tab_index,url:testWindow.browserUi.current_url})").toString()));
        QVERIFY(page("typeof window.redirectedWindowCommand === 'undefined'").toBool());
        pageReceiver = "testWindow.tabViewAt(0)";
        QVERIFY(page("typeof window.redirectedWindowCommand === 'undefined'").toBool());
        QCOMPARE(page("window.pageWindowDraft").toString(), QStringLiteral("unfinished work"));
        delete reselectionObserver.data();
        return;
    }
    if (scenario == "private") {
        QVERIFY(command("open --target private-window " + destination + "-second"));
        evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
        QTRY_COMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(), 3);
        QVERIFY(evaluate("(function(){var e=testWindow.liveWindowEntries().filter(function(e){return e.ui!==testWindow.browserUi});return e.length===2 && e[0].profile!==e[1].profile && e[0].ui.page_host_profile_id()!==e[1].ui.page_host_profile_id()})()").toBool());
    }
    if (scenario.startsWith("last-page-close")) {
        QPointer<QQuickWindow> closingWindow(childWindow);
        engine->rootContext()->setContextProperty("closingPageHost", childWindow);
        QPointer<QObject> closingView(evaluate(childHost + ".activeView").value<QObject *>());
        QVERIFY(evaluate(entry + ".ui.execute_command('tab-close')").toBool());
        evaluate(childHost + ".applySwitcherEngineAction()");
        QTRY_VERIFY(evaluate(childHost + ".windowCloseConfirmationVisible").toBool());
        QCOMPARE(evaluate(entry + ".ui.tab_count").toInt(), 1);
        QVERIFY(closingView);
        QVERIFY(clickCloseDecision(childWindow, scenario != "last-page-close-cancel"));
        if (scenario == "last-page-close-cancel") {
            QTRY_VERIFY(!evaluate(childHost + ".windowCloseConfirmationVisible").toBool());
            QVERIFY(childWindow->isVisible());
            QCOMPARE(evaluate(entry + ".ui.tab_count").toInt(), 1);
            pageReceiver = childHost + ".activeView";
            QCOMPARE(page("location.href").toString(), destination);
        } else {
            QTRY_VERIFY(!closingWindow || !closingWindow->isVisible()
                        || evaluate("closingPageHost && closingPageHost.windowShutdownPagePromptVisible").toBool());
            if (closingWindow && closingWindow->isVisible()
                    && evaluate("closingPageHost.windowShutdownPagePromptVisible").toBool())
                QVERIFY(clickCloseDecision(closingWindow, true));
            QTRY_VERIFY(!closingWindow || !closingWindow->isVisible());
            QTRY_COMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(), 1);
            QTRY_VERIFY(closingView.isNull());
        }
        QVERIFY(window->isVisible());
        pageReceiver = "testWindow.tabViewAt(0)";
        QCOMPARE(page("window.pageWindowDraft").toString(), QStringLiteral("unfinished work"));
        return;
    }
    if (scenario == "first-host-close" || scenario == "first-page-tab-close") {
        QVERIFY(evaluate("testWindow.activateBrowserWindow(testWindow,testWindow.browserUi)").toBool());
        QTRY_COMPARE(QGuiApplication::focusWindow(), window);
        if (scenario == "first-page-tab-close") {
            QVERIFY(command("tab-close"));
            evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
        } else evaluate("testWindow.close()");
        QTRY_VERIFY(evaluate("testWindow.shutdownConfirmationVisible").toBool());
        QVERIFY(clickCloseDecision(window, true));
        QTRY_VERIFY(!evaluate("testWindow.shutdownConfirmationVisible").toBool());
        QTRY_VERIFY(!window->isVisible() || evaluate("testWindow.shutdownPagePromptVisible").toBool());
        if (evaluate("testWindow.shutdownPagePromptVisible").toBool()) {
            QVERIFY(clickCloseDecision(window, true));
        }
        QTRY_VERIFY(!window->isVisible());
        QCOMPARE(evaluate("testWindow.browserUi.tab_count").toInt(), 0);
        QCOMPARE(evaluate("testWindow.tabViews.length").toInt(), 0);
        QTRY_VERIFY(sourceView.isNull());
        QVERIFY(childWindow->isVisible());
        pageReceiver = childHost + ".activeView";
        const auto next = initial + "?after-first-host-close=yes";
        evaluate(entry + ".ui.navigate_without_context_route(" + quoted(next) + ")");
        evaluate(childHost + ".applySwitcherEngineAction()");
        QTRY_COMPARE(page("location.href").toString(), next);
    }
}


void ApplicationTests::credentialViewAssociation_data()
{
    QTest::addColumn<QString>("reassignment");
    for (const auto *item : {"tab-identity", "ui", "host", "visual-parent"})
        QTest::newRow(item) << QString::fromLatin1(item);
}

void ApplicationTests::credentialViewAssociation()
{
    QFETCH(QString, reassignment);
    QPointer<QObject> view(evaluate("testWindow.activeWebView()").value<QObject *>());
    QVERIFY(view);
    engine->rootContext()->setContextProperty("associatedView", view);
    const auto bindingFor = [&]() -> QObject * {
        for (auto *child : view->children())
            if (QString::fromLatin1(child->metaObject()->className()) == "FerricQuickCredentialBinding") return child;
        return nullptr;
    };
    QTRY_VERIFY(bindingFor());
    QPointer<QObject> previous(bindingFor());
    QCOMPARE(view->property("webChannelWorld").toUInt(), 42U);
    const auto id = view->property("stableTabId").toString();
    const auto location = page("location.href").toString();
    page("window.credentialAssociationDraft = 'keep this document'");
    QVERIFY(page("String(navigator.credentials.get).includes('[native code]')").toBool());
    if (reassignment == "tab-identity") evaluate("associatedView.stableTabId='tabid-999999999'");
    else if (reassignment == "ui") evaluate("associatedView.viewUi=null");
    else if (reassignment == "host") evaluate("associatedView.viewHost=null");
    else {
        auto *item = qobject_cast<QQuickItem *>(view.data());
        QVERIFY(item);
        engine->rootContext()->setContextProperty("associatedParent", item->parentItem());
        evaluate("associatedView.parent=null");
    }
    QTRY_VERIFY(previous.isNull());
    QVERIFY(!bindingFor());
    if (reassignment == "tab-identity") evaluate("associatedView.stableTabId=" + quoted(id));
    else if (reassignment == "ui") evaluate("associatedView.viewUi=testWindow.browserUi");
    else if (reassignment == "host") evaluate("associatedView.viewHost=testWindow");
    else evaluate("associatedView.parent=associatedParent");
    QTRY_VERIFY(bindingFor());
    QCOMPARE(view->property("stableTabId").toString(), id);
    QCOMPARE(page("location.href").toString(), location);
    QCOMPARE(page("window.credentialAssociationDraft").toString(), QStringLiteral("keep this document"));
}
