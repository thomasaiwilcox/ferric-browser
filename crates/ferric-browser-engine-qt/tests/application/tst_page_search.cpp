#include "tst_application.h"
#include <QtTest>
#include <QKeyEvent>
#include <QCoreApplication>
#include <QQuickItem>
#include <QtWebEngineCore/QWebEngineFindTextResult>

void ApplicationTests::pageSearchFollowing_data() {
    QTest::addColumn<bool>("secondary");
    QTest::addColumn<bool>("newTab");
    QTest::addColumn<bool>("fragment");
    QTest::addColumn<bool>("unselectable");
    for (bool secondary : {false,true}) {
        for (bool newTab : {false,true})
            QTest::newRow(qPrintable(QString(secondary?"secondary-":"primary-")+(newTab?"tab":"current"))) << secondary << newTab << false << false;
        QTest::newRow(secondary?"secondary-fragment":"primary-fragment") << secondary << false << true << false;
        for (bool newTab : {false,true})
            QTest::newRow(qPrintable(QString(secondary?"secondary-css-":"primary-css-")+(newTab?"tab":"current"))) << secondary << newTab << false << true;
    }
}
void ApplicationTests::pageSearchFollowing() {
    QFETCH(bool, secondary);
    QFETCH(bool, newTab);
    QFETCH(bool, fragment);
    QFETCH(bool, unselectable);
    QTest::failOnWarning(QRegularExpression(".*(TypeError|ReferenceError|Binding loop|invalid context).*"));
    const auto first=text("current_url"), destination=first+(fragment?"#search-match":"?search-first=1");
    QString owner="testWindow", ui=owner+".browserUi";
    auto ownerWindow=window;
    if (secondary) {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"+quoted(first)+",windowProfileName:'search-follow',windowProfileLabel:'Search follow'})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
        owner="testWindow.browserWindowRegistry[1].host";ui="testWindow.browserWindowRegistry[1].ui";
        QTRY_VERIFY(evaluate(owner+".profileBootstrapComplete && !"+ui+".profile_bootstrap_pending").toBool());
        ownerWindow=qobject_cast<QQuickWindow *>(evaluate(owner).value<QObject *>());
        QVERIFY(ownerWindow);
        pageReceiver=owner+".activeView";
        QTRY_VERIFY(evaluate(pageReceiver+" && !"+pageReceiver+".loading && String("+pageReceiver+".url)==="+quoted(first)).toBool());
        QTRY_VERIFY(page("location.href==="+quoted(first)+" && document.readyState==='complete'").toBool());
        QTRY_COMPARE(evaluate(ui+".load_state").toString(),"complete");
    }
    page("document.body.innerHTML='<div contenteditable=true>needle plain</div><a id=one href=\"'+"+quoted(destination)+"+'\"><h3><span>needle first</span></h3></a><a href=\"'+"+quoted(first+"?search-second=1")+"+'\">needle second</a>';true");
    if (unselectable) page("document.body.style.userSelect='none';true");
    ownerWindow->requestActivate();QTRY_VERIFY(ownerWindow->isActive());
    evaluate(pageReceiver+".forceActiveFocus()");
    QTest::keyClick(ownerWindow,Qt::Key_Slash);
    QTRY_COMPARE(evaluate(ui+".mode").toString(),"search");
    for (const auto c:QString("needle")) QTest::keyClick(ownerWindow,static_cast<Qt::Key>(c.toUpper().unicode()));
    QTRY_COMPARE(evaluate(ui+".search_text").toString(),"needle");
    const auto search=owner+".searchWorkspace";
    QTRY_COMPARE(evaluate(search+".query").toString(),"needle");
    QTRY_VERIFY(evaluate(search+".result.found && !"+search+".busy").toBool());
    QCOMPARE(evaluate(search+".result.activeMatch").toInt(),1);
    QCOMPARE(page("document.querySelector('[contenteditable]').textContent").toString(),"needle plain");
    auto searchBar=ownerWindow->findChild<QQuickItem *>("pageSearchBar");
    QVERIFY(searchBar);
    QVERIFY(QMetaObject::invokeMethod(searchBar,"nextRequested",Q_ARG(bool,false)));
    QTRY_COMPARE(evaluate(search+".result.activeMatch").toInt(),2);
    QCOMPARE(evaluate(ui+".mode").toString(),"search");
    QVERIFY(QMetaObject::invokeMethod(searchBar,"nextRequested",Q_ARG(bool,true)));
    QTRY_COMPARE(evaluate(search+".result.activeMatch").toInt(),1);
    QTest::keyClick(ownerWindow,Qt::Key_Return);
    QTRY_COMPARE(evaluate(ui+".mode").toString(),"normal");
    QCOMPARE(page("location.href").toString(),first);
    // Query acceptance and its held repeats cannot become a follow gesture.
    QKeyEvent held(QEvent::KeyPress,Qt::Key_Return,Qt::NoModifier,"\r",true,1);
    QCoreApplication::sendEvent(ownerWindow,&held);
    QCOMPARE(page("location.href").toString(),first);
    QTest::keyClick(ownerWindow,Qt::Key_N);
    QTRY_COMPARE(evaluate(search+".result.activeMatch").toInt(),2);
    QTRY_VERIFY(evaluate(search+".result.found && !"+search+".busy").toBool());
    QTest::keyClick(ownerWindow,Qt::Key_Return,newTab?Qt::ShiftModifier:Qt::NoModifier);
    QTRY_COMPARE(evaluate(ui+".current_url").toString(),destination);
    QTRY_VERIFY(page("location.href==="+quoted(destination)+" && document.readyState==='complete'").toBool());
    QTRY_COMPARE(evaluate(ui+".tab_count").toInt(),newTab?2:1);
    QTRY_VERIFY(!evaluate(search+".retained").toBool());
    QTRY_COMPARE(evaluate(ui+".mode").toString(),"normal");
    if (secondary) QCOMPARE(text("current_url"),first);
}
void ApplicationTests::pageSearchNativeText_data() {
    QTest::addColumn<bool>("secondary");
    QTest::newRow("primary") << false;
    QTest::newRow("secondary") << true;
}
void ApplicationTests::pageSearchNativeText() {
    QFETCH(bool,secondary);
    QString owner="testWindow",ui=owner+".browserUi";
    auto ownerWindow=window;
    const auto first=text("current_url");
    if (secondary) {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"+quoted(first)+",windowProfileName:'search-native',windowProfileLabel:'Search native'})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
        owner="testWindow.browserWindowRegistry[1].host";ui="testWindow.browserWindowRegistry[1].ui";
        QTRY_VERIFY(evaluate(owner+".profileBootstrapComplete && !"+ui+".profile_bootstrap_pending").toBool());
        ownerWindow=qobject_cast<QQuickWindow *>(evaluate(owner).value<QObject *>());
        QVERIFY(ownerWindow);
        pageReceiver=owner+".activeView";
        QTRY_VERIFY(evaluate(pageReceiver+" && !"+pageReceiver+".loading").toBool());
        QTRY_VERIFY(page("location.href==="+quoted(first)+" && document.readyState==='complete'").toBool());
        QTRY_COMPARE(evaluate(ui+".load_state").toString(),"complete");
    }
    page("document.body.innerHTML='<style>body{user-select:none}</style><a href=\"/first\">needle first</a><p>needle second</p>';true");
    auto receiver=evaluate(pageReceiver).value<QObject *>();
    QVERIFY(receiver);
    QSignalSpy results(receiver,SIGNAL(findTextFinished(QWebEngineFindTextResult)));
    QVERIFY(results.isValid());
    auto uiObject=evaluate(ui).value<QObject *>();
    QVERIFY(uiObject);
    QSignalSpy wakeups(uiObject,SIGNAL(runtime_work_available()));
    QVERIFY(wakeups.isValid());
    const auto activeMatch=[&results] {
        return results.isEmpty()?0:results.last().constFirst().value<QWebEngineFindTextResult>().activeMatch();
    };
    ownerWindow->requestActivate();QTRY_VERIFY(ownerWindow->isActive());
    evaluate(pageReceiver+".forceActiveFocus()");
    QTest::keyClick(ownerWindow,Qt::Key_Slash);
    QTRY_COMPARE(evaluate(ui+".mode").toString(),"search");
    const auto search=owner+".searchWorkspace";
    auto searchBar=ownerWindow->findChild<QQuickItem *>("pageSearchBar");
    QVERIFY(searchBar);
    QString typed;
    for (const auto c:QString("needle")) {
        const auto previousWakeups=wakeups.count();
        QTest::keyClick(ownerWindow,static_cast<Qt::Key>(c.toUpper().unicode()));
        typed+=c;
        QTRY_COMPARE(evaluate(ui+".search_text").toString(),typed);
        QVERIFY(wakeups.count()>previousWakeups);
        QTRY_COMPARE(evaluate(search+".query").toString(),typed);
        QTRY_VERIFY(evaluate(search+".result.found && !"+search+".busy").toBool());
        QTRY_VERIFY(searchBar->isAncestorOf(ownerWindow->activeFocusItem()));
        QCOMPARE(evaluate(ui+".mode").toString(),"search");
    }
    QTRY_COMPARE(evaluate(ui+".search_text").toString(),"needle");
    QTRY_VERIFY(evaluate(search+".result.found && !"+search+".busy").toBool());
    QTRY_COMPARE(activeMatch(),1);
    QTest::keyClick(ownerWindow,Qt::Key_Return);
    QTRY_COMPARE(evaluate(ui+".mode").toString(),"normal");
    QCOMPARE(activeMatch(),1);
    QTest::keyClick(ownerWindow,Qt::Key_N);
    QTRY_COMPARE(activeMatch(),2);
    QTest::keyClick(ownerWindow,Qt::Key_N,Qt::ShiftModifier);
    QTRY_COMPARE(activeMatch(),1);
    QVERIFY(evaluate(search+".result.found").toBool());
    QTest::keyClick(ownerWindow,Qt::Key_Escape);
    QTRY_VERIFY(!evaluate(search+".retained").toBool());
    QTRY_VERIFY(!results.isEmpty() && results.last().constFirst().value<QWebEngineFindTextResult>().numberOfMatches()==0);
    QCOMPARE(evaluate(ui+".search_text").toString(),QString());
}
void ApplicationTests::pageSearchCrossOriginFrames_data() {
    QTest::addColumn<bool>("top");
    QTest::newRow("frame-target") << false;
    QTest::newRow("top-target") << true;
}
void ApplicationTests::pageSearchCrossOriginFrames() {
    QFETCH(bool,top);
    QTest::failOnWarning(QRegularExpression(".*(TypeError|ReferenceError|invalid context).*"));
    const auto port=server.serverPort();
    const auto destination=QString("http://localhost:%1/search-destination").arg(port);
    frameFixtures["/search-child"]=("<!doctype html><a target='"+QString(top?"_top":"_self")+"' href='"+destination+"'><h3><span>needle child</span></h3></a>").toUtf8();
    fixtureHtml=QString("<!doctype html><p>needle main</p><iframe src='http://localhost:%1/search-child'></iframe>").arg(port).toUtf8();
    QVERIFY(command(QString("open http://127.0.0.1:%1/search-frames").arg(port)));
    // The starting fixture also has one iframe. Wait for the intended document,
    // rather than allowing the previous page to satisfy the frame-count check.
    QTRY_COMPARE(page("document.querySelector('p')?.textContent").toString(),"needle main");
    QTRY_VERIFY(evaluate("testWindow.activeWebView().mainFrame.children.length===1 && !testWindow.activeWebView().loading").toBool());
    QTest::qWait(100);
    QVERIFY(command("search needle"));
    QTRY_VERIFY(evaluate("testWindow.searchWorkspace.result.found && !testWindow.searchWorkspace.busy").toBool());
    QCOMPARE(evaluate("testWindow.searchWorkspace.result.activeMatch").toInt(),1);
    key(Qt::Key_N);
    QTRY_COMPARE(evaluate("testWindow.searchWorkspace.result.activeMatch").toInt(),2);
    QVERIFY(evaluate("testWindow.searchWorkspace.result.found").toBool());
    key(Qt::Key_Return);
    if (top) QTRY_COMPARE(text("current_url"),destination);
    else {
        QTRY_COMPARE(evaluate("testWindow.activeWebView().mainFrame.children[0].url.toString()").toString(),destination);
        QCOMPARE(text("current_url"),QString("http://127.0.0.1:%1/search-frames").arg(port));
    }
}
void ApplicationTests::pageSearchCancellationAndBindings() {
    const auto first=text("current_url");
    page("document.body.innerHTML='<a href=\"'+"+quoted(first+"?search=1")+"+'\">needle</a>';true");
    QVERIFY(command("search needle"));
    QTRY_VERIFY(evaluate("testWindow.searchWorkspace.result.found && !testWindow.searchWorkspace.busy").toBool());
    const auto rejected = evaluate(R"JS((function(){
        const ui=testWindow.browserUi, source=testWindow.searchWorkspace.captured;
        return [ui.search_activation_ready(source.tabId,source.generation,'stale-document'),
                ui.search_activation_ready(source.tabId,'stale-generation',source.documentId)];
    })())JS").toList();
    QCOMPARE(rejected.size(),2);
    QVERIFY(!rejected[0].toBool());
    QVERIFY(!rejected[1].toBool());
    QCOMPARE(text("current_url"),first);
    const auto pendingFollow = evaluate(R"JS((function(){
        const ui=testWindow.browserUi, source=testWindow.searchWorkspace.captured;
        ui.search_next(false);
        return ui.search_activation_ready(source.tabId,source.generation,source.documentId);
    })())JS").toBool();
    QVERIFY(!pendingFollow);
    QCOMPARE(text("current_url"),first);
    QVERIFY(command("bind Enter tab-new"));
    key(Qt::Key_Return);
    QTRY_COMPARE(evaluate("testWindow.browserUi.tab_count").toInt(),2);
    QCOMPARE(text("current_url"),QString("rb://start"));
    QVERIFY(command("tab-prev"));
    QTRY_COMPARE(text("current_url"),first);
    key(Qt::Key_N);
    QTRY_VERIFY(evaluate("testWindow.searchWorkspace.result.found && !testWindow.searchWorkspace.busy").toBool());
    key(Qt::Key_Escape);
    QTRY_VERIFY(!evaluate("testWindow.searchWorkspace.retained").toBool());
    QCOMPARE(text("search_text"),QString());
    QTRY_VERIFY(page("window.__ferric_pageSearch===undefined",1).toBool());
    const auto queue = evaluate(R"JS((function(){
        const ui=testWindow.browserUi;
        function drain(){const rows=[];for(let n=0;n<16;n++){const r=ui.take_engine_request();if(!r || !r.kind)break;rows.push(r)}return rows}
        ui.enter_search(false);ui.search_changed('n');ui.search_changed('needle');
        const queries=drain().filter(r=>r.kind==='find').map(r=>r.query);
        ui.search_changed('needle');ui.escape();
        const cancelled=drain();
        return [queries.join(','),cancelled.filter(r=>r.kind==='find').length,cancelled.some(r=>r.kind==='clear-find')];
    })())JS").toList();
    QCOMPARE(queue.size(),3);
    QCOMPARE(queue[0].toString(),"needle");
    QCOMPARE(queue[1].toInt(),0);
    QVERIFY(queue[2].toBool());
    const auto barriers = evaluate(R"JS((function(){
        const ui=testWindow.browserUi;
        const accepted=ui.execute_command("search first ;; jseval 'true' ;; search second");
        const order=[];
        for(let n=0;n<16;n++){
            const r=ui.take_engine_request();if(!r || !r.kind)break;
            if(r.kind==='find')order.push(r.query);else if(r.kind==='evaluate')order.push('evaluate');
        }
        ui.escape();
        return [accepted,order.join(',')];
    })())JS").toList();
    QCOMPARE(barriers.size(),2);
    QVERIFY(barriers[0].toBool());
    QCOMPARE(barriers[1].toString(),"first,evaluate,second");
}
