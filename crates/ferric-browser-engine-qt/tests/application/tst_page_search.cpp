#include "tst_application.h"
#include "owned_browser_keyboard.h"
#include <QFile>
#include <QFileInfo>
#include <QDir>
#include <QInputMethodEvent>
#include <QFont>
#include <QImage>
#include <QtTest>
#include <QKeyEvent>
#include <QCoreApplication>
#include <QQuickItem>
#include <QtWebEngineCore/QWebEngineFindTextResult>

void ApplicationTests::preparePageSearchHost(const QString &kind, QString &owner,
                                            QString &ui, QQuickWindow *&host) {
    host=nullptr;
    QTest::failOnWarning(QRegularExpression(".*(TypeError|ReferenceError|Binding loop|invalid context).*"));
    const QDir sourceRoot(QDir(QFileInfo(QString::fromUtf8(__FILE__)).absolutePath()).absoluteFilePath("../.."));
    for (const auto &path : {QString("qml/components/FerricSearchBar.qml"),
            QString("qml/components/FerricPageSearch.qml"),
            QString("qml/components/FerricRuntimeController.qml"),
            QString("qml/components/FerricFocusOverlayController.qml"),
            QString("qml/components/FerricPrimaryBrowserWindow.qml"),
            QString("qml/components/FerricBrowserWindow.qml")}) {
        QFile source(sourceRoot.filePath(path)), embedded(":/qt/qml/io/github/ferricbrowser/"+path);
        QVERIFY(source.open(QIODevice::ReadOnly)); QVERIFY(embedded.open(QIODevice::ReadOnly));
        QVERIFY2(source.readAll()==embedded.readAll(),qPrintable("Rebuild the production engine: stale "+path));
    }
    const auto initial=text("current_url");
    owner="testWindow"; ui=owner+".browserUi";
    if (kind=="secondary" || kind=="private") {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"+quoted(initial)
            +",windowProfileName:'page-search',windowPrivateProfile:"+(kind=="private"?"true":"false")+"})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
        owner="testWindow.browserWindowRegistry[1].host"; ui="testWindow.browserWindowRegistry[1].ui";
    } else if (kind=="popup") {
        QVERIFY(command("bind p hint --target pointer links"));
        key(Qt::Key_P); typeText(hintLabel("Open popup")); key(Qt::Key_Return);
        QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(),1);
        owner="testWindow.popupWindowRegistry[0].host"; ui="testWindow.popupWindowRegistry[0].ui";
    }
    pageReceiver=owner+".activeWebView()";
    const auto expected=kind=="popup"?QUrl(initial).resolved(QUrl("/popup.html")).toString():initial;
    QTRY_VERIFY(evaluate(owner+".profileBootstrapComplete && !"+ui+".profile_bootstrap_pending && !!"
        +pageReceiver+" && !"+pageReceiver+".loading").toBool());
    QTRY_COMPARE(evaluate(pageReceiver+".url.toString()").toString(),expected);
    QTRY_COMPARE(page("location.href").toString(),expected);
    QTRY_VERIFY(page("document.readyState==='complete'").toBool());
    QTRY_COMPARE(evaluate(ui+".load_state").toString(),"complete");
    auto candidate=qobject_cast<QQuickWindow *>(evaluate(owner).value<QObject *>());
    QVERIFY(candidate);
    candidate->requestActivate(); QTRY_VERIFY(candidate->isActive());
    evaluate(pageReceiver+".forceActiveFocus()"); // Establish only the initial page focus.
    host=candidate;
}

void ApplicationTests::pageSearchEditorPreedit_data() {
    QTest::addColumn<QString>("hostKind");
    QTest::addColumn<int>("gesture");
    for (const auto &kind : {QString("primary"),QString("secondary"),QString("private"),QString("popup")}) {
        QTest::newRow(qPrintable(kind+"-return")) << kind << int(Qt::Key_Return);
        QTest::newRow(qPrintable(kind+"-escape")) << kind << int(Qt::Key_Escape);
    }
}
void ApplicationTests::pageSearchEditorPreedit() {
    QFETCH(QString,hostKind); QFETCH(int,gesture);
    QString owner,ui; QQuickWindow *host=nullptr;
    preparePageSearchHost(hostKind,owner,ui,host); QVERIFY(host);
    const auto original=page("location.href").toString();
    sendOwnedBrowserKey(host,Qt::Key_Slash,Qt::NoModifier,"AB10");
    QTRY_COMPARE(evaluate(ui+".mode").toString(),"search");
    auto bar=host->findChild<QQuickItem *>("pageSearchBar"); QVERIFY(bar);
    QTRY_VERIFY(host->activeFocusItem() && bar->isAncestorOf(host->activeFocusItem()));
    auto input=host->activeFocusItem();
    QSignalSpy accepted(bar,SIGNAL(accepted())), escaped(bar,SIGNAL(escapeRequested()));
    QVERIFY(accepted.isValid() && escaped.isValid());
    QInputMethodEvent preedit(QString::fromUtf8("にほ"),{});
    QCoreApplication::sendEvent(input,&preedit);
    QTRY_VERIFY(input->property("inputMethodComposing").toBool());
    sendOwnedBrowserKey(host,gesture);
    QCOMPARE(accepted.count(),0); QCOMPARE(escaped.count(),0);
    QCOMPARE(evaluate(ui+".mode").toString(),"search");
    QCOMPARE(page("location.href").toString(),original);
    QInputMethodEvent commit; commit.setCommitString(QString::fromUtf8("日本"));
    QCoreApplication::sendEvent(input,&commit);
    QTRY_VERIFY(!input->property("inputMethodComposing").toBool());
    QTRY_COMPARE(evaluate(ui+".search_text").toString(),QString::fromUtf8("日本"));
    sendOwnedBrowserKey(host,gesture);
    QCOMPARE(accepted.count(),gesture==Qt::Key_Return?1:0);
    QCOMPARE(escaped.count(),gesture==Qt::Key_Escape?1:0);
    QTRY_COMPARE(evaluate(ui+".mode").toString(),"normal");
    QCOMPARE(page("location.href").toString(),original);
}

void ApplicationTests::pageSearchFeedback_data() {
    QTest::addColumn<QString>("hostKind");
    for (const auto &kind : {"primary","secondary","private","popup"}) QTest::newRow(kind)<<QString(kind);
}
void ApplicationTests::pageSearchFeedback() {
    QFETCH(QString,hostKind);
    QString owner,ui; QQuickWindow *host=nullptr;
    preparePageSearchHost(hostKind,owner,ui,host); QVERIFY(host);
    page("document.body.innerHTML='<p>needle one</p><p>needle two</p><p>needle three</p>';true");
    const auto original=page("location.href").toString();
    const auto search=owner+".searchWorkspace";
    const auto pageHasFocus=[&] {
        auto view=qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());
        auto focus=host->activeFocusItem();
        return view && focus && (view==focus || view->isAncestorOf(focus));
    };
    const auto type=[host](const QString &word) {
        for (const auto c : word) sendOwnedBrowserKey(host,c.toUpper().unicode());
    };
    const auto viewportSize=page("JSON.stringify([innerWidth,innerHeight])").toString();
    for (const auto &policy : {QString("always"),QString("never"),QString("in-mode")}) {
        QVERIFY(evaluate(ui+".set_runtime_setting('ui.statusbar',"+quoted(policy)+",true)").toBool());
        sendOwnedBrowserKey(host,Qt::Key_Slash,Qt::NoModifier,"AB10");
        QTRY_COMPARE(evaluate(ui+".mode").toString(),"search");
        type("needle");
        QTRY_VERIFY(evaluate(search+".result.found && !"+search+".busy").toBool());
        QTRY_VERIFY(evaluate(search+".previewText.indexOf('Match 1 of 3')===0").toBool());
        auto feedback=host->findChild<QQuickItem *>("pageSearchFeedback"); QVERIFY(feedback);
        QTRY_VERIFY(feedback->isVisible());
        QCOMPARE(page("JSON.stringify([innerWidth,innerHeight])").toString(),viewportSize);
        auto bar=host->findChild<QQuickItem *>("pageSearchBar"); QVERIFY(bar);
        QVERIFY(feedback->mapToScene(QPointF(0,feedback->height())).y() <= bar->mapToScene(QPointF()).y());
        sendOwnedBrowserKey(host,Qt::Key_Return);
        QTRY_COMPARE(evaluate(ui+".mode").toString(),"normal");
        QTRY_VERIFY(evaluate(search+".retained && "+search+".accepted && "+search+".previewText.indexOf('Match 1 of 3')===0").toBool());
        QTRY_VERIFY(pageHasFocus());
        QTRY_COMPARE(feedback->isVisible(),!evaluate(owner+".statusBarVisible").toBool());
        for (const auto position : {2,3,1}) {
            sendOwnedBrowserKey(host,Qt::Key_N);
            QTRY_VERIFY(evaluate(search+".previewText.indexOf('Match "+QString::number(position)+" of 3')===0").toBool());
        }
        sendOwnedBrowserKey(host,Qt::Key_N,Qt::ShiftModifier);
        QTRY_VERIFY(evaluate(search+".previewText.indexOf('Match 3 of 3')===0").toBool());
        sendOwnedBrowserKey(host,Qt::Key_Escape);
        QTRY_VERIFY(!feedback->isVisible());
        QTRY_VERIFY(pageHasFocus());
        sendOwnedBrowserKey(host,Qt::Key_Slash,Qt::NoModifier,"AB10"); type("missing");
        QTRY_VERIFY(evaluate(search+".query==='missing' && !"+search+".busy").toBool());
        QCOMPARE(evaluate(search+".previewText").toString(),"No matches");
        QVERIFY(feedback->isVisible());
        sendOwnedBrowserKey(host,Qt::Key_Escape);
        QTRY_VERIFY(!feedback->isVisible());
        QTRY_VERIFY(pageHasFocus());
        QCOMPARE(page("location.href").toString(),original);
    }
    QVERIFY(evaluate(ui+".set_runtime_setting('ui.font_size_pt','26',true)").toBool());
    host->resize(360,640);
    QTRY_COMPARE(host->width(),360);
    sendOwnedBrowserKey(host,Qt::Key_Slash,Qt::NoModifier,"AB10"); type("needle");
    QTRY_VERIFY(evaluate(search+".previewText.indexOf('Match 1 of 3')===0 && !"+search+".busy").toBool());
    auto feedback=host->findChild<QQuickItem *>("pageSearchFeedback");
    auto label=host->findChild<QQuickItem *>("pageSearchFeedbackText");
    QVERIFY(feedback && label && feedback->isVisible());
    QSignalSpy frame(host,&QQuickWindow::frameSwapped);
    host->update(); QTRY_VERIFY(!frame.isEmpty());
    QTRY_COMPARE(evaluate(ui+".chrome_font_size_pt").toDouble(),26.0);
    QTRY_COMPARE(label->property("font").value<QFont>().pointSizeF(),
                 host->property("font").value<QFont>().pointSizeF());
    for (const auto &name : {QString("pageSearchPreviousButton"),QString("pageSearchNextButton")}) {
        auto button=host->findChild<QQuickItem *>(name); QVERIFY(button);
        auto symbol=button->property("contentItem").value<QQuickItem *>(); QVERIFY(symbol);
        QCOMPARE(symbol->property("font").value<QFont>().pointSizeF(),
                 host->property("font").value<QFont>().pointSizeF());
    }
    QVERIFY(feedback->width()<=host->width()-24);
    QVERIFY(label->property("lineCount").toInt()>0 && label->property("lineCount").toInt()<=2);
    const auto artifacts=qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
    if (!artifacts.isEmpty()) QVERIFY(host->grabWindow().save(artifacts+"/search-"+hostKind+".png"));
    sendOwnedBrowserKey(host,Qt::Key_Escape);
    QTRY_VERIFY(!feedback->isVisible());
    QCOMPARE(page("location.href").toString(),original);
}

void ApplicationTests::pageSearchDirection_data() {
    QTest::addColumn<QString>("hostKind");
    QTest::addColumn<bool>("backward");
    for (const auto &kind : {QString("primary"),QString("secondary"),QString("private"),QString("popup")}) {
        for (const bool backward : {false,true})
            QTest::newRow(qPrintable(kind+(backward?"-backward":"-forward"))) << kind << backward;
    }
}
void ApplicationTests::pageSearchDirection() {
    QFETCH(QString,hostKind); QFETCH(bool,backward);
    QString owner,ui; QQuickWindow *host=nullptr;
    preparePageSearchHost(hostKind,owner,ui,host); QVERIFY(host);
    page("document.body.innerHTML='<p>needle one</p><p>needle two</p><p>needle three</p>';true");
    const auto original=page("location.href").toString();
    const auto search=owner+".searchWorkspace";
    sendOwnedBrowserKey(host,backward?Qt::Key_Question:Qt::Key_Slash,
        backward?Qt::ShiftModifier:Qt::NoModifier,"AB10");
    QTRY_COMPARE(evaluate(ui+".mode").toString(),"search");
    QCOMPARE(evaluate(ui+".search_backward").toBool(),backward);
    for (const auto c : QString("needle")) sendOwnedBrowserKey(host,c.toUpper().unicode());
    QTRY_VERIFY(evaluate(search+".query==='needle' && !"+search+".busy && "+search+".result.count===3").toBool());
    int position=evaluate(search+".result.activeMatch").toInt();
    QVERIFY(position>=1 && position<=3);
    sendOwnedBrowserKey(host,Qt::Key_Return);
    QTRY_COMPARE(evaluate(ui+".mode").toString(),"normal");
    // Reversing is a single step. Neither n, N nor an explicit direction
    // changes the direction of the query that the next n will continue.
    for (const bool reverse : {false,true,false,false,true,true,false}) {
        const bool stepBackward=backward!=reverse;
        position=(position-1+(stepBackward?2:1))%3+1;
        sendOwnedBrowserKey(host,Qt::Key_N,reverse?Qt::ShiftModifier:Qt::NoModifier);
        QTRY_VERIFY(evaluate("!"+search+".busy && !"+search+".pending.length").toBool());
        QTRY_COMPARE(evaluate(search+".result.activeMatch").toInt(),position);
        QCOMPARE(evaluate(ui+".search_backward").toBool(),backward);
    }
    const auto drain=owner+"."+(hostKind=="primary"?"executePendingEngineAction()":"applySwitcherEngineAction()");
    for (const auto &invocation : {QString("search-next --forward --count 2"),
            QString("search-next --backward --count 2"),QString("search-next --count 2"),
            QString("action tab search-next forward 2")}) {
        const bool stepBackward=invocation.contains("--backward")
            || (invocation=="search-next --count 2" && backward);
        position=(position-1+(stepBackward?1:2))%3+1;
        QVERIFY2(evaluate(ui+".execute_command("+quoted(invocation)+")").toBool(),qPrintable(invocation));
        evaluate(drain);
        QTRY_VERIFY(evaluate("!"+search+".busy && !"+search+".pending.length").toBool());
        QTRY_COMPARE(evaluate(search+".result.activeMatch").toInt(),position);
        QCOMPARE(evaluate(ui+".search_backward").toBool(),backward);
    }
    // A command that changes direction for this step must not change n.
    position=(position-1+(backward?2:1))%3+1;
    sendOwnedBrowserKey(host,Qt::Key_N);
    QTRY_COMPARE(evaluate(search+".result.activeMatch").toInt(),position);
    evaluate(ui+".refresh_guide('search-next')");
    QCOMPARE(evaluate(ui+".guide_commands["+ui+".guide_keys.indexOf('N')]").toString(),
        backward?QString("search-next --forward"):QString("search-next --backward"));
    auto view=qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());
    QVERIFY(view && host->activeFocusItem());
    QVERIFY(view==host->activeFocusItem() || view->isAncestorOf(host->activeFocusItem()));
    QCOMPARE(page("location.href").toString(),original);
    sendOwnedBrowserKey(host,Qt::Key_Escape);
    QTRY_VERIFY(!evaluate(search+".retained").toBool());
    const bool nextBackward=!backward;
    sendOwnedBrowserKey(host,nextBackward?Qt::Key_Question:Qt::Key_Slash,
        nextBackward?Qt::ShiftModifier:Qt::NoModifier,"AB10");
    for (const auto c : QString("needle")) sendOwnedBrowserKey(host,c.toUpper().unicode());
    QTRY_VERIFY(evaluate(search+".query==='needle' && !"+search+".busy && "+search+".result.count===3").toBool());
    for (const auto &name : {QString("pageSearchPreviousButton"),QString("pageSearchNextButton")}) {
        auto button=host->findChild<QQuickItem *>(name); QVERIFY(button);
        QTest::mouseClick(host,Qt::LeftButton,Qt::NoModifier,
            button->mapToScene(QPointF(button->width()/2,button->height()/2)).toPoint());
        QTRY_VERIFY(evaluate("!"+search+".busy && !"+search+".pending.length").toBool());
        QCOMPARE(evaluate(ui+".search_backward").toBool(),nextBackward);
    }
    sendOwnedBrowserKey(host,Qt::Key_Space);
    QTRY_COMPARE(evaluate(ui+".search_text").toString(),"needle ");
    QCOMPARE(evaluate(ui+".search_backward").toBool(),nextBackward);
    sendOwnedBrowserKey(host,Qt::Key_Escape);
    QTRY_VERIFY(!evaluate(search+".retained").toBool());
    QCOMPARE(page("location.href").toString(),original);
}

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
    sendOwnedBrowserKey(ownerWindow,Qt::Key_Slash,Qt::NoModifier,"AB10");
    QTRY_COMPARE(evaluate(ui+".mode").toString(),"search");
    for (const auto c:QString("needle")) sendOwnedBrowserKey(ownerWindow,static_cast<Qt::Key>(c.toUpper().unicode()));
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
    sendOwnedBrowserKey(ownerWindow,Qt::Key_Return);
    QTRY_COMPARE(evaluate(ui+".mode").toString(),"normal");
    QCOMPARE(page("location.href").toString(),first);
    // Query acceptance and its held repeats cannot become a follow gesture.
    QKeyEvent held(QEvent::KeyPress,Qt::Key_Return,Qt::NoModifier,"\r",true,1);
    QCoreApplication::sendEvent(ownerWindow,&held);
    QCOMPARE(page("location.href").toString(),first);
    sendOwnedBrowserKey(ownerWindow,Qt::Key_N);
    QTRY_COMPARE(evaluate(search+".result.activeMatch").toInt(),2);
    QTRY_VERIFY(evaluate(search+".result.found && !"+search+".busy").toBool());
    sendOwnedBrowserKey(ownerWindow,Qt::Key_Return,newTab?Qt::ShiftModifier:Qt::NoModifier);
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
