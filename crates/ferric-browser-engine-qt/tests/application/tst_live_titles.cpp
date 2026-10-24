#include "tst_application.h"
#include "owned_browser_keyboard.h"
#include <QQuickItem>
#include <QtTest>

void ApplicationTests::productPolishSwitcherLiveUpdates_data() {
    QTest::addColumn<QString>("hostKind");
    for (const auto &kind : {"primary","secondary","private","popup","many-windows"})
        QTest::newRow(kind)<<QString(kind);
}

void ApplicationTests::productPolishSwitcherLiveUpdates() {
    QFETCH(QString,hostKind);
    QString owner,ui;QQuickWindow *host=nullptr;
    preparePageSearchHost(hostKind=="many-windows"?"primary":hostKind,owner,ui,host);QVERIFY(host);
    if (hostKind=="many-windows") {
        const auto url=page("location.href").toString();
        for (int i=0;i<7;++i) {
            evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"+quoted(url)
                +",windowProfileName:"+quoted("busy-window-"+QString::number(i))+"})");
        }
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),8);
        QTRY_VERIFY(evaluate("testWindow.browserWindowRegistry.every(function(entry){return entry.host.profileBootstrapComplete && !entry.ui.profile_bootstrap_pending && entry.host.activeWebView() && !entry.host.activeWebView().loading && entry.ui.load_state==='complete'})").toBool());
        const auto activeReceiver=pageReceiver;
        pageReceiver="testWindow.browserWindowRegistry[7].host.activeWebView()";
        page("document.title='Tailstar reference';true");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry[7].ui.page_title").toString(),"Tailstar reference");
        pageReceiver=activeReceiver;
        // Establish the invoker through the owned seat after opening windows.
        // Untokened Wayland requestActivate() need not move compositor focus.
        if (QGuiApplication::platformName().startsWith("wayland")
                && qEnvironmentVariable("FERRIC_PORTAL_TEST_BUS")=="isolated")
            sendOwnedBrowserKey(host,Qt::Key_Escape);
        else host->requestActivate();
        QTRY_VERIFY(host->isActive());
        evaluate(pageReceiver+".forceActiveFocus()"); // Initial invoker focus after window creation.
    }
    page("document.title='Northstar initial';true");
    QTRY_COMPARE(evaluate(ui+".page_title").toString(),"Northstar initial");
    const auto original=page("location.href").toString();
    const auto target=evaluate("JSON.stringify("+ui+".navigation_target_for_index("+ui+".active_tab_index))").toString();
    auto view=qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());QVERIFY(view);
    QSignalSpy titles(view,SIGNAL(titleChanged()));QVERIFY(titles.isValid());
    sendOwnedBrowserKey(host,Qt::Key_T,Qt::ShiftModifier);
    QTRY_VERIFY(evaluate("testWindow.switcherVisible && testWindow.switcherUi==="+ui).toBool());
    for (const auto c : QString("northstar"))sendOwnedBrowserKey(host,c.toUpper().unicode());
    QTRY_VERIFY(evaluate("testWindow.switcherResultsCurrent() && testWindow.switcherResults.length>0").toBool());
    auto input=host->activeFocusItem();QVERIFY(input);
    page("window.livePulse=0;window.liveBusy=true;window.liveTimer=setInterval(()=>{document.title='Northstar pulse '+(++window.livePulse)},10);window.liveStop=setTimeout(()=>{clearInterval(window.liveTimer);window.liveBusy=false},10000);true");
    QTRY_VERIFY_WITH_TIMEOUT(titles.count()>=10,1000);
    sendOwnedBrowserKey(host,Qt::Key_A,Qt::ControlModifier);
    const auto query=hostKind=="many-windows"?QString("tailstar"):QString("pulse");
    for (const auto c : query)sendOwnedBrowserKey(host,c.toUpper().unicode());
    const auto predicate=hostKind=="many-windows"
        ? QString("function(row){return row.kind==='tab' && row.label==='Tailstar reference'}")
        : QString("function(row){return row.kind==='tab' && row.label.indexOf('Northstar pulse ')===0}");
    const auto before=titles.count();
    QElapsedTimer deadline;deadline.start();bool found=false;
    do {
        found=evaluate("testWindow.switcherResultsCurrent() && testWindow.switcherResults.some("+predicate+")").toBool();
        if (!found)QTest::qWait(10);
    } while (!found && deadline.elapsed()<700);
    const auto updates=titles.count()-before;
    qInfo()<<"busy Switcher facts"<<hostKind<<"found"<<found<<"wait ms"<<deadline.elapsed()
        <<"title updates"<<updates<<"query"<<evaluate("testWindow.runtime_switcherSurface.query")
        <<"input retained"<<(host->activeFocusItem()==input);
    if (!found)page("clearInterval(window.liveTimer);clearTimeout(window.liveStop);window.liveBusy=false;true");
    QVERIFY2(found,"A continuously updating page must not postpone the user's Switcher query");
    QCOMPARE(evaluate("testWindow.runtime_switcherSurface.query").toString(),query);
    QCOMPARE(host->activeFocusItem(),input);
    const auto firstLabel=evaluate("testWindow.switcherResults.find("+predicate+").label").toString();
    sendOwnedBrowserKey(host,Qt::Key_Tab);
    QTRY_VERIFY(host->activeFocusItem() && host->activeFocusItem()->objectName()=="switcherActionButton");
    auto action=host->activeFocusItem();
    if (hostKind=="many-windows") {
        const auto activeReceiver=pageReceiver;
        pageReceiver="testWindow.browserWindowRegistry[7].host.activeWebView()";
        page("document.title='Tailstar updated';true");
        pageReceiver=activeReceiver;
    }
    deadline.restart();bool refreshed=false;
    do {
        refreshed=hostKind=="many-windows"
            ? evaluate("testWindow.switcherResults.some(function(row){return row.kind==='tab' && row.label==='Tailstar updated'})").toBool()
            : evaluate("testWindow.switcherResults.some(function(row){return row.kind==='tab' && row.label.indexOf('Northstar pulse ')===0 && row.label!=="+quoted(firstLabel)+"})").toBool();
        if (!refreshed)QTest::qWait(10);
    } while (!refreshed && deadline.elapsed()<700);
    const auto activeStream=page("window.liveBusy").toBool();
    qInfo()<<"busy action refresh"<<hostKind<<refreshed<<"wait ms"<<deadline.elapsed()<<"source active"<<activeStream;
    page("clearInterval(window.liveTimer);clearTimeout(window.liveStop);window.liveBusy=false;true");
    QVERIFY2(activeStream,"The page must still be updating during the search and action checks");
    QVERIFY2(refreshed,"Every window must publish live results while another source stays busy");
    QCOMPARE(host->activeFocusItem(),action);
    QCOMPARE(page("location.href").toString(),original);
    QCOMPARE(evaluate("JSON.stringify("+ui+".navigation_target_for_index("+ui+".active_tab_index))").toString(),target);
    sendOwnedBrowserKey(host,Qt::Key_Escape);
    QTRY_VERIFY(host->activeFocusItem() && (host->activeFocusItem()==view || view->isAncestorOf(host->activeFocusItem())));
}

void ApplicationTests::productPolishSwitcherNavigation_data() {
    QTest::addColumn<QString>("hostKind");
    QTest::addColumn<QString>("method");
    for (const auto &kind : {QString("primary"),QString("secondary"),QString("private"),QString("popup")})
        for (const auto &method : {QString("navigation"),QString("same-document")})
            QTest::newRow(qPrintable(kind+"-"+method))<<kind<<method;
}

void ApplicationTests::productPolishSwitcherNavigation() {
    QFETCH(QString,hostKind);QFETCH(QString,method);
    QString owner,ui;QQuickWindow *host=nullptr;
    preparePageSearchHost(hostKind,owner,ui,host);QVERIFY(host);
    const auto original=page("location.href").toString();
    const auto originalTarget=evaluate("JSON.stringify("+ui+".navigation_target_for_index("+ui+".active_tab_index))").toString();
    const auto destination=QUrl(original).resolved(QUrl("/northstar-"+method+".html")).toString();
    frameFixtures.insert(QUrl(destination).path().toUtf8(),"<!doctype html><title>Northstar Destination</title><input aria-label='Destination input'><p>Destination document</p>");
    page("document.title='Original document';window.switcherNavigationMarker='same document';true");
    QTRY_COMPARE(evaluate(ui+".page_title").toString(),"Original document");
    sendOwnedBrowserKey(host,Qt::Key_T,Qt::ShiftModifier);
    QTRY_VERIFY(evaluate("testWindow.switcherVisible && testWindow.switcherUi==="+ui).toBool());
    for (const auto c : QString("northstar"))sendOwnedBrowserKey(host,c.toUpper().unicode());
    QTRY_VERIFY(evaluate("testWindow.switcherResultsCurrent() && testWindow.switcherResults.length===0").toBool());
    auto input=host->activeFocusItem();QVERIFY(input);
    if (method=="navigation") {
        QVERIFY(evaluate(ui+".execute_command("+quoted("open "+destination)+")").toBool());
    } else {
        page("history.pushState({switcher:true},'',"+quoted(destination)+");true");
    }
    QTRY_COMPARE(page("location.href").toString(),destination);
    QTRY_COMPARE(evaluate(ui+".current_url").toString(),destination);
    QTRY_COMPARE(evaluate(ui+".load_state").toString(),"complete");
    const auto currentTarget=evaluate("JSON.stringify("+ui+".navigation_target_for_index("+ui+".active_tab_index))").toString();
    if (method=="same-document") {
        QCOMPARE(currentTarget,originalTarget);
        QCOMPARE(page("window.switcherNavigationMarker").toString(),"same document");
    } else {
        QVERIFY(currentTarget!=originalTarget);
        QTRY_COMPARE(evaluate(ui+".page_title").toString(),"Northstar Destination");
    }
    QElapsedTimer deadline;deadline.start();bool backend=false,visible=false;
    do {
        backend=evaluate(ui+".switcher_query('northstar','tabs') && "+ui+".switcher_result_secondaries.indexOf("+quoted(destination)+")>=0").toBool();
        visible=evaluate("testWindow.switcherResults.some(function(row){return row.kind==='tab' && row.secondary==="+quoted(destination)+" && row.generation===JSON.parse("+quoted(currentTarget)+").generation})").toBool();
        if (!backend || !visible)QTest::qWait(50);
    } while ((!backend || !visible) && deadline.elapsed()<2000);
    qInfo()<<"navigation Switcher facts"<<hostKind<<method<<"backend"<<backend<<"visible"<<visible
        <<evaluate(ui+".switcher_result_labels")<<evaluate(ui+".switcher_result_secondaries")
        <<"input retained"<<(host->activeFocusItem()==input);
    QVERIFY2(backend,"Repeated Switcher queries must use the current navigation metadata");
    QVERIFY2(visible,"An open Switcher must refresh after navigation without another edit");
    QCOMPARE(evaluate("testWindow.runtime_switcherSurface.query").toString(),"northstar");
    QCOMPARE(host->activeFocusItem(),input);
    sendOwnedBrowserKey(host,Qt::Key_Return);
    QTRY_VERIFY(!evaluate("testWindow.switcherVisible").toBool());
    auto view=qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());QVERIFY(view);
    QTRY_VERIFY(host->activeFocusItem() && (host->activeFocusItem()==view || view->isAncestorOf(host->activeFocusItem())));
    QCOMPARE(page("location.href").toString(),destination);
    if (hostKind!="popup") {
        const auto activeReceiver=pageReceiver;
        const auto backgroundSource=QUrl(original).resolved(QUrl("/background-source.html")).toString();
        const auto backgroundDestination=QUrl(original).resolved(QUrl("/compass-updates.html")).toString();
        frameFixtures.insert("/background-source.html","<!doctype html><title>Background source</title><p>Original background document</p>");
        frameFixtures.insert("/compass-updates.html","<!doctype html><title>Compass updates</title><p>Changed background document</p>");
        QVERIFY(evaluate(ui+".execute_command("+quoted("tab-open --background "+backgroundSource)+")").toBool());
        QTRY_COMPARE(evaluate(ui+".tab_count").toInt(),2);
        pageReceiver=owner+".tabViewAt(1)";
        QTRY_VERIFY(evaluate(pageReceiver+" && !"+pageReceiver+".loading").toBool());
        QTRY_COMPARE(page("location.href").toString(),backgroundSource);
        sendOwnedBrowserKey(host,Qt::Key_T,Qt::ShiftModifier);
        QTRY_VERIFY(evaluate("testWindow.switcherVisible").toBool());
        for (const auto c : QString("compass"))sendOwnedBrowserKey(host,c.toUpper().unicode());
        QTRY_VERIFY(evaluate("testWindow.switcherResultsCurrent() && testWindow.switcherResults.length===0").toBool());
        input=host->activeFocusItem();QVERIFY(input);
        evaluate(pageReceiver+".url="+quoted(backgroundDestination));
        QTRY_COMPARE(page("location.href").toString(),backgroundDestination);
        QTRY_VERIFY(evaluate("testWindow.switcherResults.some(function(row){return row.kind==='tab' && row.secondary==="+quoted(backgroundDestination)+" && row.label==='Compass updates'})").toBool());
        QCOMPARE(evaluate(ui+".active_tab_index").toInt(),0);
        QCOMPARE(evaluate(ui+".current_url").toString(),destination);
        QCOMPARE(evaluate("testWindow.runtime_switcherSurface.query").toString(),"compass");
        QCOMPARE(host->activeFocusItem(),input);
        sendOwnedBrowserKey(host,Qt::Key_Escape);
        pageReceiver=activeReceiver;
        QTRY_VERIFY(host->activeFocusItem() && (host->activeFocusItem()==view || view->isAncestorOf(host->activeFocusItem())));
        QCOMPARE(page("location.href").toString(),destination);
    }
}

void ApplicationTests::productPolishLiveTitles_data() {
    QTest::addColumn<QString>("hostKind");
    for (const auto &kind : {"primary","secondary","private","popup"})
        QTest::newRow(kind)<<QString(kind);
}

void ApplicationTests::productPolishLiveTitles() {
    QFETCH(QString,hostKind);
    QString owner,ui; QQuickWindow *host=nullptr;
    preparePageSearchHost(hostKind,owner,ui,host);QVERIFY(host);
    const auto original=page("location.href").toString();
    const auto target=evaluate("JSON.stringify("+ui+".navigation_target_for_index("+ui+".active_tab_index))").toString();
    const auto history=evaluate("(function(){var h=JSON.parse("+ui+".content_history_export("+ui+".active_tab_index));return JSON.stringify([h.cursor,h.entries.length])})()").toString();
    QVERIFY(evaluate(ui+".switcher_query('northstar','tabs')").toBool());
    QVERIFY(evaluate(ui+".switcher_result_labels.length===0").toBool()); // Seed the old cache.
    const auto title=QString("Live Northstar Inbox");
    page("document.title="+quoted(title)+";true");
    QTRY_COMPARE(evaluate(pageReceiver+".title").toString(),title);
    QElapsedTimer deadline;deadline.start();
    bool found=false;
    do {
        QVERIFY(evaluate(ui+".switcher_query('northstar','tabs')").toBool());
        found=evaluate(ui+".switcher_result_labels.indexOf("+quoted(title)+")>=0").toBool();
        if (!found)QTest::qWait(50);
    } while (!found && deadline.elapsed()<2000);
    const auto copied=evaluate("(function(){if(!"+ui+".execute_command('yank title'))return 'rejected';return "+ui+".take_clipboard_request()})()").toString();
    qInfo()<<"live title facts"<<hostKind<<evaluate(pageReceiver+".title")<<evaluate(ui+".page_title")
        <<evaluate(ui+".switcher_result_labels")<<copied;
    QVERIFY2(found,"The keyboard Switcher must find the page's current title");
    QCOMPARE(evaluate(ui+".page_title").toString(),title);
    QCOMPARE(copied,title);
    for (const auto &invalid : {QString("['missing','1','missing']"),
            QString("(function(){var t=JSON.parse(")+quoted(target)+");return [t.tabId,'999999',t.documentId]})()",
            QString("(function(){var t=JSON.parse(")+quoted(target)+");return [t.tabId,t.generation,'missing']})()"}) {
        QVERIFY(!evaluate("(function(){var t="+invalid+";return "+ui+".document_title_changed(t[0],t[1],t[2],'Stale title')})()").toBool());
        QCOMPARE(evaluate(ui+".page_title").toString(),title);
    }
    QCOMPARE(page("location.href").toString(),original);
    QCOMPARE(evaluate("JSON.stringify("+ui+".navigation_target_for_index("+ui+".active_tab_index))").toString(),target);
    QCOMPARE(evaluate("(function(){var h=JSON.parse("+ui+".content_history_export("+ui+".active_tab_index));return JSON.stringify([h.cursor,h.entries.length])})()").toString(),history);
    sendOwnedBrowserKey(host,Qt::Key_T,Qt::ShiftModifier);
    QTRY_VERIFY(evaluate("testWindow.switcherVisible && testWindow.switcherUi==="+ui).toBool());
    for (const auto c : QString("northstar"))sendOwnedBrowserKey(host,c.toUpper().unicode());
    QTRY_VERIFY(evaluate("testWindow.switcherResults.some(function(row){return row.kind==='tab' && row.label==="+quoted(title)+"})").toBool());
    auto input=host->activeFocusItem();QVERIFY(input);
    QVERIFY(input->objectName()=="switcherSearchInput" || (input->parentItem() && input->parentItem()->objectName()=="switcherSearchInput"));
    page("document.title='Northstar Review (3)';true");
    QTRY_VERIFY(evaluate("testWindow.switcherResults.some(function(row){return row.kind==='tab' && row.label==='Northstar Review (3)'})").toBool());
    QCOMPARE(evaluate("testWindow.runtime_switcherSurface.query").toString(),"northstar");
    QCOMPARE(host->activeFocusItem(),input);
    page("document.title='Different workspace';true");
    QTRY_VERIFY(evaluate("!testWindow.switcherResults.some(function(row){return row.kind==='tab' && row.id===JSON.parse("+quoted(target)+").tabId})").toBool());
    QCOMPARE(host->activeFocusItem(),input);
    sendOwnedBrowserKey(host,Qt::Key_Escape);
    auto view=qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());QVERIFY(view);
    QTRY_VERIFY(host->activeFocusItem() && (host->activeFocusItem()==view || view->isAncestorOf(host->activeFocusItem())));
    page("document.title='Northstar\\u202e\\n'+ '界'.repeat(3000);true");
    QTRY_VERIFY(evaluate(pageReceiver+".title.indexOf('界')>=0").toBool());
    const auto safe=evaluate(ui+".display_page_title("+pageReceiver+".title)").toString();
    QVERIFY(safe.toUtf8().size()<=4096); QVERIFY(!safe.contains(QChar(0x202e)));
    QTRY_COMPARE(evaluate(ui+".page_title").toString(),safe);
    QCOMPARE(evaluate("(function(){if(!"+ui+".execute_command('yank title'))return 'rejected';return "+ui+".take_clipboard_request()})()").toString(),safe);
    QCOMPARE(page("location.href").toString(),original);
    QCOMPARE(evaluate("JSON.stringify("+ui+".navigation_target_for_index("+ui+".active_tab_index))").toString(),target);
    QCOMPARE(evaluate("(function(){var h=JSON.parse("+ui+".content_history_export("+ui+".active_tab_index));return JSON.stringify([h.cursor,h.entries.length])})()").toString(),history);

    if (hostKind!="popup") {
        const auto activeReceiver=pageReceiver;
        const auto backgroundUrl=original+"?background-title=1";
        QVERIFY(evaluate(ui+".execute_command("+quoted("tab-open --background "+backgroundUrl)+")").toBool());
        QTRY_COMPARE(evaluate(ui+".tab_count").toInt(),2);
        pageReceiver=owner+".tabViewAt(1)";
        QTRY_VERIFY(evaluate(pageReceiver+" && !"+pageReceiver+".loading").toBool());
        QTRY_COMPARE(page("location.href").toString(),backgroundUrl);
        QTRY_VERIFY(page("document.readyState==='complete'").toBool());
        page("document.title='Background Northstar Updates';true");
        QTRY_VERIFY(([&] {
            return evaluate(ui+".switcher_query('background northstar','tabs') && "+ui+".switcher_result_labels.indexOf('Background Northstar Updates')>=0").toBool();
        })());
        QCOMPARE(evaluate(ui+".active_tab_index").toInt(),0);
        QCOMPARE(evaluate(ui+".page_title").toString(),safe);
        QVERIFY(evaluate(ui+".select_tab(1)").toBool());
        QTRY_COMPARE(evaluate(ui+".page_title").toString(),"Background Northstar Updates");
        QVERIFY(evaluate(ui+".select_tab(0)").toBool());
        pageReceiver=activeReceiver;
        QTRY_COMPARE(evaluate(ui+".page_title").toString(),safe);
    }
    // An explicit fragment Open advances the core target without loading a
    // replacement document. Live titles must acquire that admitted target.
    const auto liveUi=ui;
    page("window.fragmentTitleMarker='document kept';true");
    const auto fragment=original+"#live-title";
    for (int visit=0;visit<2;++visit) {
        QVERIFY(evaluate(liveUi+".execute_command("+quoted("open "+fragment)+")").toBool());
        evaluate("testWindow.completeBrowserKeyAction("+ui+","+owner+")");
        QTRY_COMPARE(page("location.href").toString(),fragment);
        QTRY_COMPARE(evaluate(liveUi+".load_state").toString(),"complete");
        QCOMPARE(page("window.fragmentTitleMarker").toString(),"document kept");
        const auto nextTitle=QString("Northstar fragment %1").arg(visit);
        page("document.title="+quoted(nextTitle)+";true");
        QTRY_COMPARE(evaluate(liveUi+".page_title").toString(),nextTitle);
        QVERIFY(evaluate(liveUi+".switcher_query('northstar fragment','tabs') && "+liveUi+".switcher_result_labels.indexOf("+quoted(nextTitle)+")>=0").toBool());
        QCOMPARE(evaluate("(function(){if(!"+liveUi+".execute_command('yank title'))return 'rejected';return "+liveUi+".take_clipboard_request()})()").toString(),nextTitle);
    }
    if (hostKind=="primary") {
        page("window.liveTitleTransferMarker='document kept';true");
        QVERIFY(evaluate(ui+".execute_command('tab-detach')").toBool());
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
        const auto detachedUi=QString("testWindow.browserWindowRegistry[1].ui");
        pageReceiver="testWindow.browserWindowRegistry[1].host.activeWebView()";
        QTRY_VERIFY(evaluate(pageReceiver+" && "+pageReceiver+".ownsTab() && !"+pageReceiver+".loading").toBool());
        QTRY_COMPARE(page("window.liveTitleTransferMarker").toString(),"document kept");
        page("document.title='Detached Northstar Document';true");
        QTRY_COMPARE(evaluate(detachedUi+".page_title").toString(),"Detached Northstar Document");
        QVERIFY(evaluate(detachedUi+".switcher_query('detached northstar','tabs') && "+detachedUi+".switcher_result_labels.indexOf('Detached Northstar Document')>=0").toBool());
        QVERIFY(!evaluate("(function(){var t=JSON.parse("+quoted(target)+");return "+ui+".document_title_changed(t.tabId,t.generation,t.documentId,'Retired source')})()").toBool());
        QCOMPARE(page("location.href").toString(),fragment);
    }

}
