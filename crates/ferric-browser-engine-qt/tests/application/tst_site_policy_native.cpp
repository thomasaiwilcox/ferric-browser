#include "tst_application.h"
#include "../../src/browser_key_router.h"
#include "../../src/hint_request_adapter.h"
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJSEngine>
#include <QQmlEngine>
#include <QQmlContext>
#include <QKeyEvent>
#include <QInputMethodEvent>
#include <QQuickItem>
#include <QSignalSpy>
#include <QScopeGuard>
#include <QElapsedTimer>
#include <QMetaProperty>
#include <QClipboard>
#include <QGuiApplication>
#include <QDir>
#include <QFileInfo>
#include <QDBusArgument>
#include <QDBusInterface>
#include <QDBusReply>
#include <QTcpSocket>
#include <QtTest>
#include <algorithm>

namespace {
bool suspendHintRefreshForAdapterFixture(QObject *workspace)
{
    // These component fixtures drive native requests or raw collectors. Stop the
    // unrelated workspace poller so it cannot replace their batch mid-proof.
    // Competing ordinary collection has its own explicit rejection case below;
    // these positives do not qualify shipping refresh concurrency.
    int found=0;
    for(auto *child:workspace->children()) {
        if(child->property("interval").toInt()!=80||!child->property("repeat").toBool()
            ||child->metaObject()->indexOfMethod("stop()")<0)continue;
        ++found;
        if(!QMetaObject::invokeMethod(child,"stop",Qt::DirectConnection)
            ||child->property("running").toBool())return false;
    }
    return found==1;
}
}

void ApplicationTests::destroyHintRequestAdapter()
{
    if (!hintAdapterDestructionTarget) return;
    hintAdapterSawProbe = true;
    hintAdapterCompletedBeforeProbe = scriptDone;
    delete hintAdapterDestructionTarget.data();
}

void ApplicationTests::hintRequestCaptureClassification_data()
{
    QTest::addColumn<QString>("change");
    for(const auto *name:{"ordinary","disabled","no-op","labels-only","page-keys-only","anchor",
                         "non-http","stop","replacement","other-adapter","invalid-view"})
        QTest::newRow(name)<<QString::fromLatin1(name);
}

void ApplicationTests::hintRequestWorkspaceCollection_data()
{
    QTest::addColumn<bool>("foreign");
    QTest::newRow("top")<<false;
    QTest::newRow("foreign")<<true;
    QTest::newRow("same-origin-primary")<<false;
    QTest::newRow("shadow-slot")<<false;
    QTest::newRow("foreign-shadow")<<true;
    QTest::newRow("ordinary-timeout")<<false;
    QTest::newRow("ordinary-timeout-twice")<<false;
    QTest::newRow("ordinary-timeout-cancel")<<false;
    QTest::newRow("ordinary-timeout-reentry")<<false;
    QTest::newRow("ordinary-timeout-navigation")<<false;
    QTest::newRow("ordinary-evidence-limit")<<false;
    QTest::newRow("busy-top")<<false;
    QTest::newRow("busy-foreign")<<true;
}

void ApplicationTests::hintInputIntegration_data()
{
    QTest::addColumn<QString>("change");
    for(const auto *name:{"early-query","early-edit","early-confirm","early-autofollow","discard-loading","page-loading","page-query","page-ready",
        "zero-absent","zero-stale","zero-consumed","overflow","restart","first-ambiguous","first-capacity","namespace-error","first-stale","first-refresh","search-query","ready-selection"})
        QTest::newRow(name)<<QString::fromLatin1(name);
}

void ApplicationTests::hintPageKeyRouting_data()
{
    QTest::addColumn<QString>("change");
    for(const auto *name:{"mid-prefix","unchanged","numeric","modified","loading","refreshing","stale-refresh","early-query","ready-query",
        "editable","unknown","escape-noneditable","one-shot-held","one-shot-reentry","one-shot-overlap","one-shot-collision","one-shot-query-held","normal-browser","normal-mixed","normal-count","normal-chain","passthrough-once",
        "passthrough","blocked-once","omitted-context","named-output","uppercase-output","digit-output","punctuation-output","chord-output","native-prompt","two-windows",
        "raw-once","raw-persistent","raw-binding-once","raw-refusal","raw-recovery","raw-metadata","raw-overlap-once","raw-mode-invalid",
        "receiver-top","receiver-top-denies-frame","receiver-focused-top-denied","receiver-same",
        "receiver-foreign","receiver-foreign-denied","receiver-combined-top","receiver-combined-foreign",
        "receiver-open-shadow","receiver-closed-shadow","receiver-custom-editor","receiver-opaque",
        "receiver-media-normal","receiver-media-hint","receiver-media-pass","receiver-media-input",
        "receiver-media-canvas","receiver-media-editor","receiver-media-unknown",
        "receiver-frame-replacement","receiver-same-url-reload","receiver-focus-switch","receiver-tab","receiver-enter","receiver-stale-legacy","receiver-many-observations",
        "policy-publication","policy-publication-once","unqualified-output",
        "clipboard-map-hint","clipboard-map-normal","clipboard-map-once","clipboard-map-persistent","clipboard-map-raw","clipboard-map-ordinary"})
        QTest::newRow(name)<<QString::fromLatin1(name);
    for(const auto *mapping:{"chain","reciprocal"})
        for(const auto *context:{"hint","normal","once","persistent"}) {
            const auto name=QString("one-hop-%1-%2").arg(mapping,context);
            QTest::newRow(qPrintable(name))<<name;
        }
}

void ApplicationTests::hintPageKeyRouting()
{
    QFETCH(QString,change);
    auto *receiverRouter=window->findChild<FerricBrowserKeyRouter *>();QVERIFY(receiverRouter);
    QSignalSpy receiverFacts(receiverRouter,&FerricBrowserKeyRouter::pageReceiverObserved);
    const auto nativeFrameTrace=qScopeGuard([&]{if(QTest::currentTestFailed()) {
        auto *item=qobject_cast<QQuickItem *>(evaluate("testWindow.activeWebView()").value<QObject *>());
        const auto property=item->metaObject()->property(item->metaObject()->indexOfProperty("mainFrame"));
        const auto frame=property.read(item).value<QWebEngineFrame>();
        const int before=receiverFacts.size();
        receiverRouter->probePageReceiver(item,evaluate("testWindow.focusProbeScript()").toString(),evaluate("testWindow.browserUi.input_epoch").toInt());
        QElapsedTimer retry;retry.start();while(receiverFacts.size()==before&&retry.elapsed()<1500)QTest::qWait(5);
        qInfo()<<"PAGE_RECEIVER_LATE"<<receiverRouter->pageReceiverKey()<<receiverFacts.size()
            <<(receiverFacts.isEmpty()?QString():receiverFacts.last().at(3).toString());
        qInfo()<<"PAGE_RECEIVER_NATIVE"<<item->metaObject()->className()<<property.isFinal()<<frame.isValid()<<frame.isMainFrame()
            <<page("({focused:document.hasFocus(),epoch:window.__ferric_page_key_document,state:window.__ferric_browserReadFocusState?window.__ferric_browserReadFocusState():null})",1);
    }});
    const auto receiverTrace=qScopeGuard([&]{if(QTest::currentTestFailed())qInfo()<<"PAGE_RECEIVER_WIRE"
        <<receiverRouter->pageReceiverKey()<<"signals"<<receiverFacts.size()
        <<(receiverFacts.isEmpty()?QString():receiverFacts.last().at(3).toString())
        <<evaluate("(function(){var o=testWindow.browserWindowEntryForUi(testWindow.browserUi);return JSON.stringify({owner:!!o,same:!!o&&o.view===testWindow.activeWebView(),host:!!o&&o.host===testWindow,epoch:testWindow.browserUi.input_epoch,active:testWindow.active,focus:!!testWindow.activeWebView().activeFocus})})()").toString();});
    const auto trace=qScopeGuard([this]{if(QTest::currentTestFailed())qInfo()<<"PAGE_KEY_FAILURE"<<evaluate(
        "JSON.stringify({mode:testWindow.browserUi.mode,status:testWindow.browserUi.status_text,route:JSON.parse(testWindow.browserUi.page_key_route('p')),hint:testWindow.hintState})").toString();});
    QVERIFY(page("window.routeEvents=[];for(const type of ['keydown','keyup'])document.addEventListener(type,e=>window.routeEvents.push({type:e.type,key:e.key,code:e.code,trusted:e.isTrusted,repeat:e.repeat,ctrl:e.ctrlKey,shift:e.shiftKey,alt:e.altKey,meta:e.metaKey}));document.activeElement.blur();true").toBool());
    const auto probe=[this]{evaluate("testWindow.probeActiveFocus(testWindow.browserUi,testWindow.browserUi.active_tab_index,testWindow.activeWebView())");};
    const auto allowed=[this]{return QJsonDocument::fromJson(evaluate("testWindow.browserUi.page_key_route('p')").toString().toUtf8()).object().value("allowed").toBool();};
    if(change=="escape-noneditable") {
        QVERIFY(page("document.getElementById('button').focus();true").toBool());
        QVERIFY(command("hint buttons"));probe();QTRY_VERIFY(allowed());
        const auto sequence=page("window.__ferric_browserFocusState.sequence",1).toInt();
        QVERIFY(sequence>0);
        key(Qt::Key_Escape);QTRY_COMPARE(text("mode"),QString("normal"));
        QVERIFY(command("hint buttons"));probe();
        QTRY_VERIFY_WITH_TIMEOUT(allowed(),1500);
        QCOMPARE(page("window.__ferric_browserFocusState.sequence",1).toInt(),sequence);
        key(Qt::Key_P);
        QTRY_COMPARE(page("routeEvents.filter(e=>e.key==='k').length").toInt(),2);
        QVERIFY(page("routeEvents.filter(e=>e.key==='k').every(e=>e.trusted)").toBool());
        QTRY_COMPARE(text("mode"),QString("normal"));
        return;
    }
    if(change.startsWith("one-hop-")) {
        const bool once=change.endsWith("-once");
        const auto enter=[&] {
            if(change.endsWith("-hint"))return command("hint buttons");
            if(change.endsWith("-normal"))return true;
            return command(once?"mode-enter passthrough-once":"mode-enter passthrough");
        };
        QVERIFY(enter());probe();QTRY_VERIFY(allowed());
        QSignalSpy admissions(receiverRouter,&FerricBrowserKeyRouter::keyPressed);
        const auto returned=receiverRouter->feasibilityReturnedEvents();
        key(Qt::Key_P);
        QTRY_COMPARE(page("routeEvents.filter(e=>e.key==='k').length").toInt(),2);
        QTRY_VERIFY(receiverRouter->feasibilityReturnedEvents()>returned);
        QCOMPARE(page("routeEvents.length").toInt(),2);
        QCOMPARE(admissions.size(),1);
        if(once||change.endsWith("-hint")) {
            QTRY_COMPARE(text("mode"),QString("normal"));
            QVERIFY(enter());probe();QTRY_VERIFY(allowed());
        }
        key(Qt::Key_K);
        const auto expected=change.contains("reciprocal")?"p":"j";
        QTRY_COMPARE(page("routeEvents.filter(e=>e.key==="+quoted(expected)+").length").toInt(),2);
        QCOMPARE(page("routeEvents.length").toInt(),4);
        QCOMPARE(admissions.size(),2);
        QVERIFY(page("routeEvents.every(e=>e.trusted)").toBool());
        QCOMPARE(receiverRouter->feasibilitySourceCount(),0);
        QCOMPARE(receiverRouter->feasibilityOutputCount(),0);
        return;
    }
    if(change.startsWith("clipboard-map-")) {
        const auto savedClipboard=QGuiApplication::clipboard()->text();
        const auto restore=qScopeGuard([&]{QGuiApplication::clipboard()->setText(savedClipboard);});
        QGuiApplication::clipboard()->setText("clipboard-original-control");
        const bool unchanged=change.endsWith("raw")||change.endsWith("ordinary");
        QVERIFY(page("window.pasteCount=0;document.addEventListener('paste',()=>pasteCount++);true").toBool());
        if(change.endsWith("hint"))QVERIFY(command("hint buttons"));
        else if(change.endsWith("once"))QVERIFY(command("mode-enter passthrough-once"));
        else if(!change.endsWith("normal"))QVERIFY(command(change.endsWith("raw")?
            "mode-enter passthrough --raw":"mode-enter passthrough"));
        probe();QTRY_VERIFY(!receiverRouter->pageReceiverKey().isEmpty());
        if(!unchanged)QTRY_VERIFY(evaluate("JSON.parse(testWindow.browserUi.page_key_route('Ctrl+v')).allowed===true").toBool());
        else QVERIFY(page("document.getElementById('input').value='';document.getElementById('input').focus();true").toBool());
        key(Qt::Key_V,Qt::ControlModifier);
        if(unchanged) {
            QTRY_COMPARE(page("document.getElementById('input').value").toString(),QString("clipboard-original-control"));
            QCOMPARE(page("pasteCount").toInt(),1);
        } else {
            QTRY_COMPARE(page("routeEvents.filter(e=>e.key==='k').length").toInt(),2);
            QCOMPARE(page("routeEvents.filter(e=>e.key.toLowerCase()==='v').length").toInt(),0);
            QCOMPARE(page("routeEvents.filter(e=>e.key==='k').every(e=>!e.ctrl&&e.trusted)").toBool(),true);
            QCOMPARE(page("pasteCount").toInt(),0);
            if(change.endsWith("once")||change.endsWith("hint"))QTRY_COMPARE(text("mode"),QString("normal"));
        }
        QCOMPARE(receiverRouter->feasibilitySourceCount(),0);
        QCOMPARE(receiverRouter->feasibilityOutputCount(),0);
        return;
    }
    if(change=="unqualified-output") {
        QVERIFY(command("hint buttons"));probe();
        QTRY_VERIFY(!receiverRouter->pageReceiverKey().isEmpty());
        const auto route=QJsonDocument::fromJson(evaluate("testWindow.browserUi.page_key_route('p')").toString().toUtf8()).object();
        QVERIFY(route.value("claimed").toBool());QVERIFY(!route.value("allowed").toBool());
        QCOMPARE(route.value("status").toString(),QString("qualification-failed"));
        QCOMPARE(route.value("qualification").toObject().value("status").toString(),QString("unqualified"));
        QVERIFY(evaluate("JSON.parse(testWindow.browserUi.interaction_policy_snapshot()).native_page_keys.some(entry=>entry.syntax==='valid'&&entry.destination==='Ctrl+c'&&entry.qualification.status==='unqualified')").toBool());
        QVERIFY(evaluate("JSON.parse(testWindow.browserUi.refresh_interaction_status()).page_keys.schema===1").toBool());
        key(Qt::Key_P);
        QCOMPARE(page("window.keys.length").toInt(),0);
        QVERIFY(text("status_text").contains("syntax is valid"));
        return;
    }
    if(change.startsWith("policy-publication")) {
        QVERIFY(command(change.endsWith("once")?"mode-enter passthrough-once":"mode-enter passthrough"));
        probe();QTRY_VERIFY(allowed());
        QTest::keyPress(window,Qt::Key_P);
        bool sourceHeld=true;
        const auto held=qScopeGuard([&]{if(sourceHeld)QTest::keyRelease(window,Qt::Key_P);});
        QTRY_COMPARE(page("routeEvents.filter(e=>e.key==='k'&&e.type==='keydown').length").toInt(),1);
        QCOMPARE(receiverRouter->feasibilityOutputCount(),1);
        const auto previous=evaluate("testWindow.browserUi.binding_context()").toString();
        QFile policy(directory.filePath("held-policy-config.toml"));
        QVERIFY(policy.open(QIODevice::WriteOnly));
        policy.write(QString("schema_version = 3\n[[site_rules]]\nid = 'page-key-routing'\npattern = 'http://127.0.0.1:%1/*'\n"
            "[site_rules.interaction]\nenabled = false\n").arg(server.serverPort()).toUtf8());
        policy.close();
        evaluate("testWindow.browserUi.poll_config()");
        QTRY_VERIFY(evaluate("testWindow.browserUi.binding_context()").toString()!=previous);
        QTRY_COMPARE(receiverRouter->policyContext(),evaluate("testWindow.browserUi.binding_context()").toString());
        QVERIFY(evaluate("JSON.parse(testWindow.browserUi.page_key_route('p')).claimed!==true").toBool());
        QTRY_COMPARE(page("routeEvents.filter(e=>e.key==='k'&&e.type==='keyup').length").toInt(),1);
        QKeyEvent repeat(QEvent::KeyPress,Qt::Key_P,Qt::NoModifier,QString("p"),true);
        QCoreApplication::sendEvent(window,&repeat);
        QTest::keyRelease(window,Qt::Key_P);
        sourceHeld=false;
        QCOMPARE(receiverRouter->feasibilitySourceCount(),0);
        QCOMPARE(page("routeEvents.filter(e=>e.key==='k'||e.key==='p').length").toInt(),2);
        QTRY_COMPARE(text("mode"),change.endsWith("once")?QString("normal"):QString("pass-through"));
        return;
    }
    if(change.startsWith("receiver-")) {
        const bool top=change=="receiver-top"||change=="receiver-focused-top-denied"||change=="receiver-combined-top"
            ||change=="receiver-open-shadow"||change=="receiver-closed-shadow"||change=="receiver-custom-editor"||change=="receiver-many-observations"
            ||change.startsWith("receiver-media-");
        const bool blocked=change=="receiver-top-denies-frame"||change=="receiver-focused-top-denied"
            ||change=="receiver-foreign-denied"||change=="receiver-closed-shadow"||change=="receiver-custom-editor"||change=="receiver-opaque"
            ||change=="receiver-media-input"||change=="receiver-media-canvas"||change=="receiver-media-editor"||change=="receiver-media-unknown";
        QWebEngineFrame receiver;
        const auto read=[&](const QString &script) {
            if(top)return page(script);
            scriptDone=false;receiver.runJavaScript(script,0,[this](const QVariant &value){accept(value);});
            QElapsedTimer timer;timer.start();while(!scriptDone&&timer.elapsed()<5000)QTest::qWait(5);
            return scriptResult;
        };
        if(top) {
            QString setup="document.getElementById('button').focus();";
            if(change.contains("shadow")) setup="var host=document.createElement('div');host.id='shadow-receiver';document.body.append(host);var root=host.attachShadow({mode:"
                +quoted(change=="receiver-closed-shadow"?"closed":"open")+"});root.innerHTML='<button id=shadow-button>Shadow command</button>';root.firstChild.focus();";
            if(change=="receiver-custom-editor")setup="var host=document.createElement('div');host.tabIndex=0;host.setAttribute('role','textbox');document.body.append(host);host.focus();";
            if(change.startsWith("receiver-media-")) {
                const auto content=change=="receiver-media-unknown"?QString("<div>Unknown control</div>")
                    :QString("<div><video></video></div>")+(change=="receiver-media-input"?"<input>":change=="receiver-media-canvas"?"<canvas></canvas>":"");
                setup="var host=document.createElement('div');host.tabIndex=-1;host.innerHTML="+quoted(content)+";"
                    +(change=="receiver-media-editor"?QString("host.setAttribute('role','textbox');"):QString())
                    +"document.body.append(host);host.focus();";
            }
            QVERIFY(page(setup+"true").toBool());
        } else {
            QVERIFY(page("document.querySelectorAll('iframe').forEach(frame=>frame.remove());true").toBool());
            frameFixtures["/page-receiver.html"]=R"HTML(<!doctype html><button id=receiver onclick="document.getElementById('next').focus()">Command</button><button id=next>Next</button>
<script>window.events=[];for(const type of ['keydown','keyup'])document.addEventListener(type,e=>events.push({type:e.type,key:e.key,code:e.code,trusted:e.isTrusted,focus:document.activeElement.id}));</script>)HTML";
            const bool foreign=change=="receiver-foreign"||change=="receiver-foreign-denied"||change=="receiver-combined-foreign";
            const auto url=QStringLiteral("http://%1:%2/page-receiver.html").arg(foreign?"localhost":"127.0.0.1").arg(server.serverPort());
            const auto sandbox=change=="receiver-opaque"?"frame.setAttribute('sandbox','allow-scripts');":"";
            QVERIFY(page("var frame=document.createElement('iframe');frame.id='policy-receiver';frame.name='policy-receiver';"+QString(sandbox)
                +"frame.style='position:fixed;left:20px;top:20px;width:420px;height:180px';frame.src="+quoted(url)+";document.body.append(frame);true").toBool());
            QTRY_COMPARE(evaluate("testWindow.activeWebView().mainFrame.children.length").toInt(),1);
            receiver=evaluate("testWindow.activeWebView().mainFrame.children[0]").value<QWebEngineFrame>();QVERIFY(receiver.isValid());
            QTRY_VERIFY(read("document.readyState==='complete'&&!!document.getElementById('receiver')").toBool());
            QVERIFY(read("document.getElementById('receiver').focus();true").toBool());
        }
        auto *router=window->findChild<FerricBrowserKeyRouter *>();QVERIFY(router);
        QSignalSpy observed(router,&FerricBrowserKeyRouter::pageReceiverObserved);
        if(change=="receiver-frame-replacement"||change=="receiver-same-url-reload")
            QVERIFY(read("for(var count=0;count<5;count++){document.getElementById('next').focus();document.getElementById('receiver').focus();}true").toBool());
        QVERIFY(command(change=="receiver-media-normal"?"mode-enter normal":change=="receiver-media-hint"?"hint buttons":"mode-enter passthrough"));
        evaluate("testWindow.activeWebView().forceActiveFocus()");probe();
        const auto route=[this]{return QJsonDocument::fromJson(evaluate("testWindow.browserUi.page_key_route('p')").toString().toUtf8()).object();};
        if(blocked) {
            QTRY_VERIFY(!observed.isEmpty());
            if(change=="receiver-top-denies-frame"||change=="receiver-focused-top-denied"||change=="receiver-foreign-denied")
                QTRY_VERIFY(!route().value("receiver").toString().isEmpty());
            QTRY_VERIFY(!allowed());
            if(change=="receiver-top-denies-frame"||change=="receiver-focused-top-denied"||change=="receiver-foreign-denied")
                QCOMPARE(route().value("status").toString(),QString("authorization-denied"));
            if(change=="receiver-closed-shadow"||change=="receiver-custom-editor"||change.startsWith("receiver-media-"))
                QCOMPARE(route().value("status").toString(),QString("receiver-unavailable"));
            key(Qt::Key_P);
            QCOMPARE(read(top?"window.keys.filter(k=>k==='p'||k==='k').length":"events.filter(e=>e.key==='p'||e.key==='k').length").toInt(),0);
            return;
        }
        QTRY_VERIFY(allowed());
        QCOMPARE(route().value("status").toString(),QString("ready"));
        const auto previous=route().value("receiver").toString();QVERIFY(!previous.isEmpty());
        if(change=="receiver-many-observations") {
            evaluate("(function(){var ui=testWindow.browserUi;for(var n=0;n<1000;n++)ui.page_focus_observed_for(ui.active_tab_index,ui.current_url,'0.'+n,1,ui.input_epoch,0,false,false);})()");
            probe();QTRY_VERIFY(allowed());
            QCOMPARE(route().value("receiver").toString(),previous);
            // A cached lookup key must not retain authority after legacy revocation.
            evaluate("(function(){var ui=testWindow.browserUi;ui.page_key_focus_observed_for(ui.active_tab_index,ui.current_url,'0',ui.input_epoch,JSON.stringify({target:ui.navigation_target_for_index(ui.active_tab_index),state:{sequence:999999,context:0,editable:true,user_activated:false,kind:'editable',focused:true}}));})()");
            QVERIFY(!allowed());
            probe();QTRY_VERIFY(allowed());
            QCOMPARE(route().value("receiver").toString(),previous);
        }
        if(change=="receiver-stale-legacy") {
            evaluate("(function(){var ui=testWindow.browserUi;ui.page_key_focus_observed_for(ui.active_tab_index,ui.current_url,'0.0',ui.input_epoch,JSON.stringify({target:ui.navigation_target_for_index(ui.active_tab_index),state:{sequence:999,context:0,editable:false,user_activated:false,kind:'noneditable',focused:true}}));})()");
            QVERIFY(allowed());
            QCOMPARE(route().value("receiver").toString(),previous);
            evaluate("(function(){var ui=testWindow.browserUi;ui.page_key_focus_observed_for(ui.active_tab_index,ui.current_url,'0.0',ui.input_epoch,JSON.stringify({target:ui.navigation_target_for_index(ui.active_tab_index),state:{sequence:1000,context:1,editable:true,user_activated:true,kind:'input',focused:true}}));})()");
            QVERIFY(!allowed()); // uncertain legacy facts still revoke authorization
            probe();QTRY_VERIFY(allowed()); // the native document's lower counter remains valid
            QCOMPARE(route().value("receiver").toString(),previous);
        }
        if(change=="receiver-frame-replacement"||change=="receiver-same-url-reload"||change=="receiver-focus-switch") {
            QTest::keyPress(window,Qt::Key_P);
            bool sourceHeld=true;
            const auto held=qScopeGuard([&]{if(sourceHeld)QTest::keyRelease(window,Qt::Key_P);});
            QTRY_COMPARE(read("events.filter(e=>e.key==='k'&&e.type==='keydown').length").toInt(),1);
            if(change=="receiver-frame-replacement")
                QVERIFY(page("var old=document.getElementById('policy-receiver');var replacement=old.cloneNode();old.replaceWith(replacement);true").toBool());
            else if(change=="receiver-focus-switch")
                QVERIFY(page("var second=document.getElementById('policy-receiver').cloneNode();second.id='second-receiver';second.name='second-receiver';document.body.append(second);true").toBool());
            else QVERIFY(page("var old=document.getElementById('policy-receiver');old.src=old.src;true").toBool());
            if(change=="receiver-focus-switch")QTRY_COMPARE(evaluate("testWindow.activeWebView().mainFrame.children.length").toInt(),2);
            const auto refresh=[&] {
                receiver=evaluate(change=="receiver-focus-switch"?"testWindow.activeWebView().mainFrame.children[1]":"testWindow.activeWebView().mainFrame.children[0]").value<QWebEngineFrame>();
                return receiver.isValid()&&read("document.readyState==='complete'&&!!document.getElementById('receiver')&&events.length===0").toBool();
            };
            QTRY_VERIFY(refresh());
            QVERIFY(read("document.getElementById('receiver').focus();true").toBool());probe();
            QTRY_VERIFY(allowed()&&route().value("receiver").toString()!=previous);
            QTest::keyRelease(window,Qt::Key_P);sourceHeld=false;
            QCOMPARE(read("events.filter(e=>e.key==='k').length").toInt(),0);
        }
        const auto expected=change=="receiver-tab"?"Tab":change=="receiver-enter"?"Enter":"k";
        key(Qt::Key_P);
        const QString events=top?"window.routeEvents":"window.events";
        QTRY_COMPARE(read(events+".filter(e=>e.key==="+quoted(expected)+"&&e.type==='keyup').length").toInt(),1);
        QVERIFY(read(events+".filter(e=>e.key==="+quoted(expected)+").every(e=>e.trusted)").toBool());
        QCOMPARE(read(events+".filter(e=>e.key==='p').length").toInt(),0);
        if(change=="receiver-tab"||change=="receiver-enter") {
            QCOMPARE(read("events[0].focus").toString(),QString("receiver"));
            QCOMPARE(read("events[1].focus").toString(),QString("next"));
        }
        if(change=="receiver-media-hint")QTRY_COMPARE(text("mode"),QString("normal"));
        return;
    }
    if(change.startsWith("raw-")) {
        auto *router=window->findChild<FerricBrowserKeyRouter *>();QVERIFY(router);
        const auto raw=[this]{return evaluate("testWindow.browserUi.raw_passthrough").toBool();};
        const auto held=qScopeGuard([&]{QTest::keyRelease(window,Qt::Key_P);QTest::keyRelease(window,Qt::Key_J);});
        if(change=="raw-mode-invalid") {
            for(const auto *input:{"mode-enter insert --raw","mode-enter normal --raw","mode-enter passthrough --raw --raw","mode-enter passthrough --unknown"}) {
                QVERIFY(!command(input));QCOMPARE(text("mode"),QString("normal"));QVERIFY(!raw());
            }
            return;
        }
        if(change=="raw-refusal"||change=="raw-recovery") {
            QVERIFY(command("mode-enter passthrough"));probe();QTRY_VERIFY(allowed());
            QTest::keyPress(window,Qt::Key_P);
            QTRY_COMPARE(page("window.keys.filter(k=>k==='k').length").toInt(),1);
            QVERIFY(!command("mode-enter passthrough --raw"));
            QVERIFY(text("status_text").contains("Release transformed keys first"));QVERIFY(!raw());
            QCOMPARE(router->feasibilitySourceCount(),1);
            if(change=="raw-recovery") {
                QQuickWindow other;other.resize(200,100);other.show();other.requestActivate();
                QTRY_COMPARE(QGuiApplication::focusWindow(),&other);
                if (QGuiApplication::platformName() == "wayland"
                    && qEnvironmentVariable("FERRIC_PORTAL_TEST_BUS") == "isolated") {
                    // A compositor-owned focus transition models the user's return.
                    // requestActivate alone has no input serial in this Qt-event test.
                    QDBusInterface control("org.ferricbrowser.ParentFixture",
                        "/org/ferricbrowser/ParentFixture", "org.ferricbrowser.ParentFixture");
                    QDBusReply<QVariantList> surfaces = control.call("Windows");
                    QVERIFY(surfaces.isValid());
                    QStringList matches;
                    for (const auto &item : surfaces.value()) {
                        const auto surface = qdbus_cast<QVariantMap>(item);
                        if (surface.value("pid").toLongLong() == QCoreApplication::applicationPid()
                            && surface.value("title").toString() == window->title()
                            && surface.value("mapped").toBool())
                            matches.append(surface.value("id").toString());
                    }
                    QCOMPARE(matches.size(), 1);
                    QDBusReply<bool> focused = control.call("Focus", matches.first());
                    QVERIFY(focused.isValid() && focused.value());
                } else {
                    window->requestActivate();
                }
                QTRY_COMPARE(QGuiApplication::focusWindow(),window);
                evaluate("testWindow.activeWebView().forceActiveFocus()");
                QTRY_VERIFY(router->enabled());
                QCOMPARE(router->feasibilityOutputCount(),0);
                QVERIFY(!command("mode-enter passthrough --raw"));
                key(Qt::Key_Escape,Qt::ControlModifier|Qt::ShiftModifier);
                QVERIFY(text("status_text").startsWith("Press and release"));
                QKeyEvent repeat(QEvent::KeyPress,Qt::Key_P,Qt::NoModifier,QString("p"),true);
                QCoreApplication::sendEvent(window,&repeat);
                QVERIFY(!command("mode-enter passthrough --raw"));
                QTest::keyPress(window,Qt::Key_P);QTest::keyRelease(window,Qt::Key_P);
                QCOMPARE(router->feasibilitySourceCount(),0);
                QCOMPARE(page("window.keys.filter(k=>k==='p'||k==='k').join(',')").toString(),QString("k"));
                QTRY_COMPARE(text("mode"),QString("normal"));
            } else {
                QTest::keyRelease(window,Qt::Key_P);
                QTRY_COMPARE(text("mode"),QString("pass-through"));
            }
            QVERIFY(!raw()); // neither release nor recovery defers raw entry
        }
        const bool once=change=="raw-once"||change=="raw-binding-once"||change=="raw-overlap-once";
        if(change=="raw-binding-once") {
            QVERIFY(command("bind u mode-enter passthrough-once --raw"));
            QTest::keyPress(window,Qt::Key_U);QTest::keyRelease(window,Qt::Key_U);
        } else QVERIFY(command(once?"mode-enter passthrough-once --raw":"mode-enter passthrough --raw"));
        QTRY_VERIFY(raw());
        QTRY_COMPARE(text("mode"),once?QString("pass-through-once"):QString("pass-through"));
        QVERIFY(evaluate("JSON.stringify(JSON.parse(testWindow.browserUi.page_key_route('p')))==='{}'").toBool());
        QTest::keyPress(window,Qt::Key_Shift,Qt::ShiftModifier);QTest::keyRelease(window,Qt::Key_Shift);
        QVERIFY(raw());QCOMPARE(text("mode"),once?QString("pass-through-once"):QString("pass-through"));
        if(change=="raw-metadata") {
            QKeyEvent down(QEvent::KeyPress,Qt::Key_P,Qt::ShiftModifier,33,0x50,1,QString("P"));
            QKeyEvent repeat(QEvent::KeyPress,Qt::Key_P,Qt::ShiftModifier,33,0x50,1,QString("P"),true);
            QKeyEvent up(QEvent::KeyRelease,Qt::Key_P,Qt::ShiftModifier,33,0x50,1,QString("P"));
            QCoreApplication::sendEvent(window,&down);QCoreApplication::sendEvent(window,&repeat);QCoreApplication::sendEvent(window,&up);
            QTRY_COMPARE(page("window.routeEvents.filter(e=>e.key==='P').length").toInt(),3);
            QVERIFY(page("window.routeEvents.filter(e=>e.key==='P').every(e=>e.code==='KeyP'&&e.shift&&!e.ctrl&&!e.alt&&!e.meta&&e.trusted)").toBool());
            QCOMPARE(page("window.routeEvents.filter(e=>e.key==='P').map(e=>e.repeat).join(',')").toString(),QString("false,true,false"));return;
        }
        QTest::keyPress(window,Qt::Key_P);
        if(change=="raw-overlap-once")QTest::keyPress(window,Qt::Key_J);
        QTRY_VERIFY(page("window.keys.includes('p')").toBool());QVERIFY(raw());
        QTest::keyRelease(window,Qt::Key_P);
        if(once) {QTRY_COMPARE(text("mode"),QString("normal"));QVERIFY(!raw());}
        if(change=="raw-overlap-once") {
            QKeyEvent repeat(QEvent::KeyPress,Qt::Key_J,Qt::NoModifier,QString("j"),true);
            QCoreApplication::sendEvent(window,&repeat);QTest::keyRelease(window,Qt::Key_J);
            QTRY_COMPARE(page("window.routeEvents.filter(e=>e.key==='j').map(e=>e.type).join(',')").toString(),QString("keydown,keydown,keyup"));
        }
        if(!once) {
            key(Qt::Key_Escape);QCOMPARE(text("mode"),QString("pass-through"));QVERIFY(raw());
            key(Qt::Key_Escape,Qt::ControlModifier);QTRY_COMPARE(text("mode"),QString("normal"));QVERIFY(!raw());
        }
        QVERIFY(command("mode-enter passthrough"));QVERIFY(!raw());probe();QTRY_VERIFY(allowed());
        const int mapped=page("window.keys.filter(k=>k==='k').length").toInt();
        key(Qt::Key_P);QTRY_COMPARE(page("window.keys.filter(k=>k==='k').length").toInt(),mapped+1);
        return;
    }
    if(change.startsWith("normal-")) {
        QVERIFY(command(change=="normal-chain"?"bind gp mode-enter insert-locked":"bind p mode-enter insert-locked"));
        probe();
        if(change!="normal-browser")QTRY_VERIFY(allowed());
        if(change=="normal-count")key(Qt::Key_2);
        if(change=="normal-chain")key(Qt::Key_G);
        key(Qt::Key_P);
        if(change=="normal-mixed")QTRY_COMPARE(page("window.keys.filter(k=>k==='k').length").toInt(),1);
        else {
            QCOMPARE(page("window.keys.length").toInt(),0);
            if(change!="normal-count")QTRY_COMPARE(text("mode"),QString("insert-locked"));
        }
        return;
    }
    if(change=="passthrough-once"||change=="passthrough"||change=="blocked-once") {
        if(change=="blocked-once")QVERIFY(page("document.getElementById('input').focus();true").toBool());
        QVERIFY(command(change=="passthrough"?"mode-enter passthrough":"mode-enter passthrough-once"));probe();
        if(change!="blocked-once")QTRY_VERIFY(allowed());
        key(Qt::Key_P);
        if(change=="blocked-once")QCOMPARE(page("window.keys.length").toInt(),0);
        else QTRY_COMPARE(page("window.keys.filter(k=>k==='k').length").toInt(),1);
        QTRY_COMPARE(text("mode"),change=="passthrough"?QString("pass-through"):QString("normal"));
        return;
    }
    QVERIFY(command("hint buttons"));
    auto *workspace=evaluate("testWindow.hintWorkspace").value<QObject *>();QVERIFY(workspace);
    const bool loading=change=="loading"||change=="early-query";
    if(loading)QVERIFY(workspace->setProperty("hintRefreshPending",true));
    probe();QTRY_VERIFY(allowed());
    if(!loading)QTRY_VERIFY(!workspace->property("hintRefreshPending").toBool()&&evaluate("testWindow.hintResults.length>0").toBool());
    if(change.startsWith("one-shot-")) {
        const bool collision=change=="one-shot-collision";
        const auto source=collision?Qt::Key_K:Qt::Key_P;
        bool sourceHeld=true,extraHeld=false;
        const auto releaseHeld=qScopeGuard([&]{
            if(sourceHeld)QTest::keyRelease(window,source);
            if(extraHeld)QTest::keyRelease(window,Qt::Key_F);
        });
        QTest::keyPress(window,source);
        QTRY_COMPARE(page("routeEvents.filter(e=>e.key==='k'&&e.type==='keydown').length").toInt(),1);
        QCOMPARE(text("mode"),QString("hint"));
        if(change=="one-shot-reentry") {
            const auto old=evaluate("testWindow.browserUi.hint_input_context()").toString();
            key(Qt::Key_Escape);QCOMPARE(text("mode"),QString("normal"));
            QVERIFY(command("hint buttons"));probe();QTRY_VERIFY(allowed());
            QVERIFY(evaluate("testWindow.browserUi.hint_input_context()").toString()!=old);
        } else if(change=="one-shot-overlap") {
            QTest::keyPress(window,Qt::Key_F);
            extraHeld=true;
            QTRY_COMPARE(page("routeEvents.filter(e=>e.key==='f'&&e.type==='keydown').length").toInt(),1);
        } else if(collision) {
            key(Qt::Key_P);
            QCOMPARE(text("mode"),QString("hint"));
            QCOMPARE(page("routeEvents.filter(e=>e.key==='k').length").toInt(),1);
            QCOMPARE(page("routeEvents.filter(e=>e.key==='p').length").toInt(),0);
        } else if(change=="one-shot-query-held") {
            key(Qt::Key_Slash);typeText("q");
            QCOMPARE(evaluate("testWindow.hintState.query").toString(),QString("q"));
        } else {
            QKeyEvent repeat(QEvent::KeyPress,Qt::Key_P,Qt::NoModifier,QString("p"),true);
            QCoreApplication::sendEvent(window,&repeat);
            QKeyEvent repeatRelease(QEvent::KeyRelease,Qt::Key_P,Qt::NoModifier,QString("p"),true);
            QCoreApplication::sendEvent(window,&repeatRelease);
            QTRY_COMPARE(page("routeEvents.filter(e=>e.key==='k'&&e.type==='keydown').length").toInt(),2);
            QCOMPARE(text("mode"),QString("hint"));
        }
        QTest::keyRelease(window,source);sourceHeld=false;
        QTRY_COMPARE(page("routeEvents.filter(e=>e.key==='k'&&e.type==='keyup').length").toInt(),1);
        QTest::qWait(100);
        const bool retained=change=="one-shot-reentry"||change=="one-shot-query-held";
        QCOMPARE(text("mode"),retained?QString("hint"):QString("normal"));
        if(change=="one-shot-query-held")QCOMPARE(evaluate("testWindow.hintState.query").toString(),QString("q"));
        if(change=="one-shot-overlap") {
            QCOMPARE(page("routeEvents.filter(e=>e.key==='f'&&e.type==='keyup').length").toInt(),0);
            QKeyEvent repeat(QEvent::KeyPress,Qt::Key_F,Qt::NoModifier,QString("f"),true);
            QCoreApplication::sendEvent(window,&repeat);
            QTest::keyRelease(window,Qt::Key_F);extraHeld=false;
            QTRY_COMPARE(page("routeEvents.filter(e=>e.key==='f').map(e=>e.type).join(',')").toString(),QString("keydown,keydown,keyup"));
            QCOMPARE(text("mode"),QString("normal"));
        }
        QVERIFY(page("routeEvents.every(e=>e.trusted)").toBool());
        QCOMPARE(receiverRouter->feasibilitySourceCount(),0);
        QCOMPARE(receiverRouter->feasibilityOutputCount(),0);
        return;
    }
    if(change=="stale-refresh") {
        // Own the explicit stale-refresh cycle below; background polling must
        // not start overlapping collections while its handle is inspected.
        QVERIFY(suspendHintRefreshForAdapterFixture(workspace));
        QVERIFY(page(R"JS((()=>{
            const state=window.__ferric_browserHintState;let pending=state.pendingInteraction;
            window.staleRefreshes=0;
            Object.defineProperty(state,'pendingInteraction',{configurable:true,
                get(){return pending;},set(value){pending=value;if(value){++window.staleRefreshes;state.dirtyHandler();}}});
            return true;
        })())JS",1).toBool());
        QVERIFY(QMetaObject::invokeMethod(workspace,"startHintCollection",Qt::DirectConnection));
        QTRY_VERIFY(page("window.staleRefreshes",1).toInt()>=2&&!workspace->property("hintRefreshPending").toBool());
        QCOMPARE(text("mode"),QString("hint"));
        QVERIFY(evaluate("testWindow.hintResults.length>0").toBool());
        // Retained presentation must never authorize a stale policy action.
        typeText("mm");key(Qt::Key_Return);
        QTRY_VERIFY(!workspace->property("hintDomActivationPending").toBool());
        QTRY_VERIFY(text("status_text").contains("policy Hint changed"));
        QCOMPARE(page("window.clicks").toInt(),0);
        QCOMPARE(text("mode"),QString("hint"));
        probe();QTRY_VERIFY(allowed());key(Qt::Key_P);
        QTRY_COMPARE(page("window.keys.filter(k=>k==='k').length").toInt(),1);
        QTRY_COMPARE(text("mode"),QString("normal"));
        return;
    }
    if(change=="early-query"||change=="ready-query") {
        key(Qt::Key_Slash);typeText("pkf");
        QCOMPARE(evaluate("testWindow.hintState.query").toString(),QString("pkf"));
        QCOMPARE(page("window.keys.length").toInt(),0);QCOMPARE(text("mode"),QString("hint"));return;
    }
    typeText("a");QCOMPARE(evaluate("testWindow.hintState.prefix").toString(),QString("a"));
    if(loading)QCOMPARE(evaluate("testWindow.hintWorkspace.pendingHintInputs.length").toInt(),1);
    if(change=="refreshing") {
        QVERIFY(workspace->setProperty("hintRefreshPending",true));
        QVERIFY(workspace->setProperty("hintCollectionRetryCount",1));
    }
    if(change=="editable"||change=="unknown") {
        QVERIFY(page(change=="editable"?"document.getElementById('input').focus();true"
            :"document.body.insertAdjacentHTML('beforeend','<div id=unknown tabindex=0 style=\"position:fixed;top:10px;left:300px\">Custom editor</div>');document.getElementById('unknown').focus();true").toBool());
        probe();QTRY_VERIFY(!allowed());key(Qt::Key_P);
        QCOMPARE(evaluate("testWindow.hintState.prefix").toString(),QString("a"));
        QCOMPARE(page("window.keys.length").toInt(),0);QCOMPARE(text("mode"),QString("hint"));return;
    }
    if(change=="native-prompt") {
        QVERIFY(page("setTimeout(function(){window.routeAnswer=prompt('Enter a response','');},0);true").toBool());
        QTRY_VERIFY(evaluate("testWindow.pendingPageDialogRequest!==null").toBool());
        auto *input=window->findChild<QQuickItem *>("pageDialogInput");QVERIFY(input);QTRY_VERIFY(input->hasActiveFocus());
        typeText("p");key(Qt::Key_Return);
        QTRY_VERIFY(evaluate("testWindow.pendingPageDialogRequest===null").toBool());
        QCOMPARE(page("window.routeAnswer").toString(),QString("p"));
        QCOMPARE(page("window.keys.filter(k=>k==='k').length").toInt(),0);return;
    }
    if(change=="two-windows") {
        const auto prefix=evaluate("testWindow.hintState.prefix").toString();
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"+quoted(text("current_url"))+",windowProfileName:'page-key-owner',windowProfileLabel:'Page key owner',windowEphemeralProfile:true,windowEphemeralInvocationToken:'page-key-owner-fixture'})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
        const auto child=QString("testWindow.browserWindowRegistry[1]");
        QTRY_VERIFY(evaluate(child+".host.profileBootstrapComplete && !"+child+".ui.profile_bootstrap_pending").toBool());
        auto *childWindow=qobject_cast<QQuickWindow *>(evaluate(child+".host").value<QObject *>());QVERIFY(childWindow);
        pageReceiver=child+".host.activeView";
        QTRY_VERIFY(page("document.readyState==='complete'&&!!document.getElementById('button')").toBool());
        childWindow->requestActivate();QTRY_VERIFY(childWindow->isActive());evaluate(pageReceiver+".forceActiveFocus()");
        QVERIFY(evaluate(child+".ui.execute_secondary_interactive_command('hint buttons')").toBool());
        evaluate("testWindow.probeActiveFocus("+child+".ui,"+child+".ui.active_tab_index,"+pageReceiver+")");
        QTRY_VERIFY(evaluate("JSON.parse("+child+".ui.page_key_route('p')).allowed===true").toBool());
        QTest::keyClick(childWindow,Qt::Key_P);
        QTRY_COMPARE(page("window.keys.filter(k=>k==='k').length").toInt(),1);
        QTRY_COMPARE(evaluate(child+".ui.mode").toString(),QString("normal"));
        QCOMPARE(evaluate("testWindow.hintState.prefix").toString(),prefix);
        pageReceiver="testWindow.activeWebView()";QCOMPARE(page("window.keys.length").toInt(),0);return;
    }
    key(change=="unchanged"?Qt::Key_K:change=="numeric"?Qt::Key_1:change=="modified"?Qt::Key_J:Qt::Key_P,
        change=="modified"?Qt::ControlModifier:Qt::NoModifier);
    const auto output=change=="numeric"?"1":change=="omitted-context"?"p"
        :change=="named-output"?"Enter":change=="uppercase-output"?"A":change=="digit-output"?"7"
        :change=="punctuation-output"?"+":change=="chord-output"?"ArrowLeft":"k";
    QTRY_COMPARE(page("window.keys.filter(k=>k==="+quoted(output)+").length").toInt(),1);
    QCOMPARE(evaluate("testWindow.hintState.prefix").toString(),QString());
    QTRY_COMPARE(page("window.routeEvents.filter(e=>e.key==="+quoted(output)+"&&e.type==='keyup').length").toInt(),1);
    QVERIFY(page("window.routeEvents.filter(e=>e.key==="+quoted(output)+").every(e=>e.trusted)").toBool());
    QCOMPARE(page("window.clicks").toInt(),0);
    QTRY_COMPARE(text("mode"),QString("normal"));
    if(loading||change=="refreshing") {
        QCOMPARE(evaluate("testWindow.hintWorkspace.pendingHintInputs.length").toInt(),0);
        QCOMPARE(evaluate("testWindow.hintResults.length").toInt(),0);
        QTest::qWait(100);QCOMPARE(text("mode"),QString("normal"));
        QCOMPARE(evaluate("testWindow.hintState.prefix").toString(),QString());
        QCOMPARE(page("window.keys.filter(k=>k==='k').length").toInt(),1);
    }
}

void ApplicationTests::hintInputIntegration()
{
    QFETCH(QString,change);
    auto *workspace=evaluate("testWindow.hintWorkspace").value<QObject *>();QVERIFY(workspace);
    const auto trace=qScopeGuard([this]{if(QTest::currentTestFailed())qInfo()<<"INPUT_FAILURE"<<evaluate(
        "JSON.stringify({mode:testWindow.browserUi.mode,status:testWindow.browserUi.status_text,state:testWindow.hintState,queue:testWindow.hintWorkspace.pendingHintInputs,refresh:testWindow.hintRefreshPending,activation:testWindow.hintWorkspace.hintDomActivationPending,rows:JSON.parse(testWindow.browserUi.hint_values||'[]')})").toString();});
    const bool first=change.startsWith("first-");
    const bool zero=change.startsWith("zero-");
    const auto clipboard=QGuiApplication::clipboard()->text();
    const auto restoreClipboard=qScopeGuard([&,clipboard]{if(change=="zero-consumed")QGuiApplication::clipboard()->setText(clipboard);});
    if(first||change=="namespace-error"||change=="ready-selection")QVERIFY(page(R"JS(document.body.innerHTML='<button id=button style="position:fixed;left:30px;top:20px" onclick="window.chosen=1;window.clicks++">First canonical</button><button id=button2 style="position:fixed;left:30px;top:100px" onclick="window.chosen=2;window.clicks++">Second canonical</button>';true)JS").toBool());
    if(zero) {
        const auto html=change=="zero-consumed"?QString("<a id=button href='/copied#fragment'>Only link</a>")
            :QString("<button id=button %1 onclick='window.clicks++'>Only button</button>").arg(change=="zero-absent"?"disabled":"");
        QVERIFY(page("document.body.innerHTML="+quoted(html)+";true").toBool());
    }
    QPointer<QObject> publication;
    if(change=="first-stale"||change=="first-refresh") {
        const auto action=change=="first-stale"
            ?QString::fromLatin1(R"JS(testWindow.activeWebView().runJavaScript("const old=document.getElementById('button2');old.replaceWith(old.cloneNode(true));true",0,function(value){mutated=value}))JS")
            :QString("Qt.callLater(testWindow.hintWorkspace.startHintCollection)");
        const auto source=QString("import QtQuick; Connections {property bool done:false;property bool mutated:false;property int publications:0;target:testWindow.hintWorkspace;function onHintPresentationSessionIdChanged(){if(!target.hintPresentationSessionId)return;++publications;if(done)return;done=true;%1;}}").arg(action);
        publication=evaluate("Qt.createQmlObject("+quoted(source)+",testWindow)").value<QObject *>();QVERIFY(publication);
    }
    QVERIFY(command(first?"hint --first --index 2 buttons":change=="zero-consumed"?"hint --rapid --target yank links":change=="search-query"?"target-search all":"hint buttons"));
    const bool delayed=change.startsWith("early-")||change=="discard-loading"||change=="page-loading"||change=="page-query"||change=="overflow"||change=="restart";
    // Hold the initial workspace collection while delivering real native keys;
    // release it below to run the actual collector and action pipeline.
    if(delayed)QVERIFY(workspace->setProperty("hintRefreshPending",true));
    const auto nativeKey=[this](int value,Qt::KeyboardModifiers modifiers=Qt::NoModifier){QTest::keyClick(window,Qt::Key(value),modifiers);};
    // Native delivery before the queued first collection gets an event-loop turn.
    if(change=="early-query"||change=="early-edit"||change=="page-query") {
        nativeKey(Qt::Key_Slash);nativeKey(Qt::Key_P);
        if(change!="page-query"){nativeKey(Qt::Key_K);if(change=="early-edit")nativeKey(Qt::Key_Backspace);nativeKey(Qt::Key_F);}
        QCOMPARE(evaluate("testWindow.hintState.mode").toString(),QStringLiteral("text"));
        QCOMPARE(evaluate("testWindow.hintState.query").toString(),change=="early-query"?QString("pkf"):change=="early-edit"?QString("pf"):QString("p"));
        if(change=="page-query") {
            QVERIFY(!evaluate("testWindow.hintWorkspace.commitHintPageCommand()").toBool());
            QVERIFY(!QJsonDocument::fromJson(evaluate("testWindow.browserUi.commit_hint_page_command()").toString().toUtf8()).object().value("ok").toBool());
        }
        if(change=="early-edit"){nativeKey(Qt::Key_U,Qt::ControlModifier);nativeKey(Qt::Key_Backspace);nativeKey(Qt::Key_M);}
    }
    if(change=="discard-loading") {
        nativeKey(Qt::Key_Slash);nativeKey(Qt::Key_P);
        QVERIFY(QMetaObject::invokeMethod(workspace,"clearPendingHintInput",Qt::DirectConnection));
        QCOMPARE(evaluate("testWindow.hintState.mode").toString(),QStringLiteral("label"));
        nativeKey(Qt::Key_Slash);nativeKey(Qt::Key_F);
        QCOMPARE(evaluate("testWindow.hintState.mode").toString(),QStringLiteral("text"));
        QCOMPARE(evaluate("testWindow.hintState.query").toString(),QStringLiteral("f"));
    }
    if(change=="early-confirm"||change=="early-autofollow") {
        nativeKey(Qt::Key_M);
        if(change=="early-confirm")nativeKey(Qt::Key_Return);
        nativeKey(Qt::Key_M);
        QCOMPARE(evaluate("testWindow.hintWorkspace.pendingHintInputs.length").toInt(),change=="early-confirm"?3:2);
    }
    if(change=="page-loading") {
        nativeKey(Qt::Key_M);QCOMPARE(evaluate("testWindow.hintWorkspace.pendingHintInputs.length").toInt(),1);
        QVERIFY(evaluate("testWindow.hintWorkspace.commitHintPageCommand()").toBool());
        QCOMPARE(evaluate("testWindow.hintWorkspace.pendingHintInputs.length").toInt(),0);
    }
    if(change=="overflow") {
        evaluate("(function(){for(var i=0;i<257;i++)testWindow.hintWorkspace.applyHintInteraction('character','q');return true})()");
        QCOMPARE(text("mode"),QStringLiteral("normal"));QTest::qWait(250);QCOMPARE(page("window.clicks").toInt(),0);return;
    }
    if(change=="restart") {
        const auto old=evaluate("testWindow.browserUi.hint_input_context()").toString();nativeKey(Qt::Key_Slash);nativeKey(Qt::Key_P);nativeKey(Qt::Key_Escape);
        QVERIFY(command("hint buttons"));QVERIFY(evaluate("testWindow.browserUi.hint_input_context()").toString()!=old);
        QCOMPARE(evaluate("testWindow.hintWorkspace.pendingHintInputs.length").toInt(),0);
    }
    if(delayed&&change!="restart") {
        QVERIFY(workspace->setProperty("hintRefreshPending",false));
        QVERIFY(QMetaObject::invokeMethod(workspace,"startHintCollection",Qt::DirectConnection));
    }
    if(change=="early-confirm"||change=="early-autofollow"||change=="early-edit"||first) {
        if(change=="first-stale") {
            QTRY_VERIFY_WITH_TIMEOUT(publication->property("mutated").toBool(),10000);
            QTRY_COMPARE_WITH_TIMEOUT(text("mode"),QStringLiteral("normal"),10000);QCOMPARE(page("window.clicks").toInt(),0);
        } else if(change=="first-refresh") {
            QTRY_VERIFY_WITH_TIMEOUT(publication->property("publications").toInt()>=2&&!workspace->property("hintRefreshPending").toBool(),10000);
            QTest::qWait(150);QCOMPARE(page("window.clicks").toInt(),0);
        } else {
            QTRY_COMPARE_WITH_TIMEOUT(page("window.clicks").toInt(),1,10000);
            if(first)QCOMPARE(page("window.chosen").toInt(),2);
            QTest::qWait(150);QCOMPARE(page("window.clicks").toInt(),1);
        }
        return;
    }
    if(change=="namespace-error") {
        QTRY_COMPARE_WITH_TIMEOUT(text("mode"),QStringLiteral("normal"),10000);
        QVERIFY(text("status_text").contains("capacity"));QCOMPARE(page("window.clicks").toInt(),0);return;
    }
    QTRY_VERIFY_WITH_TIMEOUT(!workspace->property("hintRefreshPending").toBool()&&(evaluate("testWindow.browserUi.hint_visible").toBool()||change=="search-query"),10000);
    QTRY_VERIFY(evaluate("Array.isArray(testWindow.hintState.reservations)&&testWindow.hintState.reservations.some(row=>row.code==='m')").toBool());
    if(change=="ready-selection") {
        const auto session=evaluate("testWindow.hintWorkspace.hintPresentationSessionId").toString();QVERIFY(!session.isEmpty());
        evaluate("testWindow.cancelFocusProbes();testWindow.runtime_scrollCaptureTimer.stop()");
        nativeKey(Qt::Key_Tab);
        QVERIFY(evaluate("testWindow.focusProbeRetries===0 && !testWindow.runtime_focusProbeTimer.running && !testWindow.runtime_scrollCaptureTimer.running").toBool());
        QTest::qWait(20); // Let the first selection's legitimate status wake settle.
        const auto initial=evaluate("testWindow.hintState.active_label").toString();QVERIFY(!initial.isEmpty());
        const QList<int> nextKeys{Qt::Key_Tab,Qt::Key_Right,Qt::Key_Down};
        const QList<int> previousKeys{Qt::Key_Backtab,Qt::Key_Left,Qt::Key_Up};
        for(int index=0;index<nextKeys.size();++index) {
            evaluate("testWindow.cancelFocusProbes();testWindow.runtime_scrollCaptureTimer.stop();testWindow.runtime_runtimeWorkTimer.stop()");
            nativeKey(nextKeys[index]);
            QVERIFY(evaluate("testWindow.focusProbeRetries===0 && !testWindow.runtime_focusProbeTimer.running && !testWindow.runtime_scrollCaptureTimer.running").toBool());
            QVERIFY(!evaluate("testWindow.runtime_runtimeWorkTimer.running").toBool());
            QVERIFY(evaluate("testWindow.hintState.active_label").toString()!=initial);
            nativeKey(previousKeys[index]);
            QVERIFY(evaluate("testWindow.focusProbeRetries===0 && !testWindow.runtime_focusProbeTimer.running && !testWindow.runtime_scrollCaptureTimer.running").toBool());
            QVERIFY(!evaluate("testWindow.runtime_runtimeWorkTimer.running").toBool());
            QCOMPARE(evaluate("testWindow.hintState.active_label").toString(),initial);
            QCOMPARE(text("mode"),QStringLiteral("hint"));
            QCOMPARE(evaluate("testWindow.hintWorkspace.hintPresentationSessionId").toString(),session);
        }
        evaluate("testWindow.runtime_runtimeWorkTimer.interval=60000;testWindow.runtime_runtimeWorkTimer.start()");
        nativeKey(Qt::Key_Tab);nativeKey(Qt::Key_Backtab);
        QVERIFY(evaluate("testWindow.runtime_runtimeWorkTimer.running && testWindow.runtime_runtimeWorkTimer.interval===60000").toBool());
        evaluate("testWindow.runtime_runtimeWorkTimer.stop()");
        QCOMPARE(page("window.keys.length").toInt(),0);QCOMPARE(page("window.clicks").toInt(),0);
        nativeKey(Qt::Key_Return);
        QTRY_COMPARE_WITH_TIMEOUT(page("window.clicks").toInt(),1,10000);
        return;
    }
    if(change=="early-query"||change=="page-query"||change=="discard-loading") {
        QCOMPARE(evaluate("testWindow.hintState.mode").toString(),QStringLiteral("text"));
        QCOMPARE(evaluate("testWindow.hintState.query").toString(),change=="early-query"?QString("pkf"):change=="discard-loading"?QString("f"):QString("p"));
        QCOMPARE(page("window.keys.length").toInt(),0);QCOMPARE(page("window.clicks").toInt(),0);return;
    }
    if(change=="search-query") {
        auto *searchInput=workspace->findChild<QQuickItem *>("targetSearchInput");QVERIFY(searchInput);
        QTRY_VERIFY(searchInput->hasActiveFocus());
        typeText("pkf");QTRY_COMPARE(text("target_search_query"),QStringLiteral("pkf"));
        QVERIFY(!evaluate("testWindow.hintWorkspace.commitHintPageCommand()").toBool());QCOMPARE(page("window.clicks").toInt(),0);return;
    }
    if(change=="zero-stale") {
        QVERIFY(suspendHintRefreshForAdapterFixture(workspace));QVERIFY(page("document.getElementById('button').disabled=true;true").toBool());typeText("m");
    }
    if(change=="zero-consumed")typeText("m");
    if(zero) {
        QTRY_COMPARE_WITH_TIMEOUT(evaluate("testWindow.hintState.kind").toString(),QStringLiteral("unavailable-only"),10000);
        QCOMPARE(text("mode"),QStringLiteral("hint"));QVERIFY(evaluate("testWindow.browserUi.hint_visible").toBool());
        QTRY_VERIFY(evaluate("testWindow.hintWorkspace.children.length>0").toBool());
        auto *notice=workspace->findChild<QObject *>("unavailablePolicyHints");QVERIFY(notice);QVERIFY(notice->property("visible").toBool());
        QVERIFY(notice->property("text").toString().contains("m:"));
        typeText("m");key(Qt::Key_Return);QCOMPARE(page("window.clicks").toInt(),0);QCOMPARE(text("mode"),QStringLiteral("hint"));
        key(Qt::Key_Escape);QCOMPARE(text("mode"),QStringLiteral("normal"));return;
    }
    if(change=="page-ready") {
        typeText("m");QCOMPARE(evaluate("testWindow.hintState.active_label").toString(),QStringLiteral("m"));
        QVERIFY(evaluate("testWindow.hintWorkspace.commitHintPageCommand()").toBool());key(Qt::Key_Return);QCOMPARE(page("window.clicks").toInt(),0);
        typeText("m");key(Qt::Key_Return);
    } else {typeText("m");}
    QTRY_COMPARE_WITH_TIMEOUT(page("window.clicks").toInt(),1,10000);
}

void ApplicationTests::hintFamilyPolicyIntegration_data()
{
    QTest::addColumn<QString>("family");
    for(const auto *name:{"actions","focus","links","all","inputs","buttons","images","media","scrollables","regions","surfaces"})QTest::newRow(name)<<QString::fromLatin1(name);
}

void ApplicationTests::hintFamilyPolicyIntegration()
{
    QFETCH(QString,family);
    QString target;
    if(family=="focus")target="<div id=family-anchor tabindex=0>Focus region</div>";
    else if(family=="links")target="<a id=family-anchor href='/family#fragment'>Link</a>";
    else if(family=="inputs")target="<input id=family-anchor aria-label='Policy input'>";
    else if(family=="images")target="<img id=family-anchor alt='Policy image' src='data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII='>";
    else if(family=="media")target="<video id=family-anchor controls src='/fixture-media.webm'></video>";
    else if(family=="scrollables"||family=="regions")target="<section id=family-anchor role=region aria-label='Policy region' style='width:280px;height:140px;overflow:auto'><div style='height:800px'>Scroll content</div></section>";
    else if(family=="surfaces")target="<canvas id=family-anchor tabindex=0 aria-label='Policy surface'></canvas>";
    else target="<button id=family-anchor>Policy button</button>";
    QVERIFY(page("document.body.innerHTML="+quoted(target)+";const item=document.getElementById('family-anchor');if(!item.matches('section'))item.style='width:280px;height:80px';item.addEventListener('click',()=>window.clicks++);true").toBool());
    if(family=="images")QTRY_VERIFY(page("document.getElementById('family-anchor').complete").toBool());
    const auto clipboard=QGuiApplication::clipboard()->text();const auto restore=qScopeGuard([&,clipboard]{if(family=="links")QGuiApplication::clipboard()->setText(clipboard);});
    if(family=="links")QGuiApplication::clipboard()->setText("policy-hint-copy-pending");
    QVERIFY(command("hint "+(family=="links"?QString("--target yank "):QString())+family));
    QTRY_VERIFY_WITH_TIMEOUT(evaluate("!testWindow.hintRefreshPending&&testWindow.browserUi.hint_visible").toBool(),10000);
    const auto code=family=="buttons"?QString("Aa"):QString("m");
    QCOMPARE(hintLabel(family=="inputs"?"Policy input":family=="images"?"Policy image":family=="focus"?"Focus region":family=="links"?"Link":family=="media"?"Media":family=="surfaces"?"Policy surface":family=="regions"||family=="scrollables"?"Policy region":"Policy button"),code);
    QCOMPARE(page("window.clicks").toInt(),0);
    if(family=="buttons") {typeText("aa");key(Qt::Key_Return);QCOMPARE(page("window.clicks").toInt(),0);key(Qt::Key_U,Qt::ControlModifier);}
    typeText(code);
    if(family=="regions"||family=="surfaces") {QTRY_VERIFY(evaluate("testWindow.pendingContextMenuRequest&&testWindow.pendingContextMenuRequest.hintAction").toBool());QCOMPARE(page("window.clicks").toInt(),0);}
    else if(family=="focus"||family=="inputs") {QTRY_COMPARE(page("document.activeElement.id").toString(),QStringLiteral("family-anchor"));QCOMPARE(page("window.clicks").toInt(),0);}
    else if(family=="scrollables") {QTRY_VERIFY(page("(()=>{const s=window.__ferric_browserScrollTargetState;return !!s&&s.policy==='element'&&s.elementRef.deref()===document.getElementById('family-anchor')})()",1).toBool());QCOMPARE(page("window.clicks").toInt(),0);}
    else if(family=="links")QTRY_COMPARE(QGuiApplication::clipboard()->text(),QStringLiteral("http://127.0.0.1:%1/family").arg(server.serverPort()));
    else QTRY_COMPARE_WITH_TIMEOUT(page("window.clicks").toInt(),1,10000);
}

void ApplicationTests::hintCollectionActivation_data()
{
    QTest::addColumn<QString>("change");
    for(const auto *name:{"dom","top-to-bottom","row-ltr","row-rtl","multi-code","anchor-hole","duplicate-anchors","overlap",
        "equal-urls","secondary","display-contents","tall-card","clip","shadow","per-item-shadow","same-origin","same-origin-mapping","foreign","slot","slot-reassigned",
        "insert","remove-fourth","reorder","recycle","peer-css","layout","missing-identity","ambiguous-primary","nested","missing-container","absent-before","rapid-yank","passive-burst"})
        QTest::newRow(name)<<QString::fromLatin1(name);
}

void ApplicationTests::hintCollectionActivation()
{
    QFETCH(QString,change);
    const auto failureTrace=qScopeGuard([this]{if(QTest::currentTestFailed())qInfo()<<"COLLECTION_FAILURE"<<evaluate(
        "JSON.stringify({mode:testWindow.browserUi.mode,status:testWindow.browserUi.status_text,rows:JSON.parse(testWindow.browserUi.hint_values||'[]'),annotation:testWindow.hintWorkspace.hintCollectionAnnotation})").toString();});
    const bool links=change=="equal-urls"||change=="rapid-yank";
    const auto clipboard=QGuiApplication::clipboard()->text();
    const auto restoreClipboard=qScopeGuard([&,clipboard]{if(links)QGuiApplication::clipboard()->setText(clipboard);});
    QString cards="<style>html,body{margin:0}#policy-list{position:relative;margin:20px;width:650px;height:300px}.policy-item{position:absolute;left:0;width:600px;height:50px}.policy-primary,.policy-menu{display:inline-block;margin:0;width:120px;height:30px;box-sizing:border-box}</style><div id=policy-list>";
    for(int i=1;i<=4;++i) {
        const auto tag=links?"a":"button";
        const auto primary=QStringLiteral(R"HTML(<%1 id=card-%2 class=policy-primary data-index=%2 %3 onclick="top.postMessage({kind:'collection-click',index:Number(this.dataset.index)},'*')">Card %2</%1>)HTML")
            .arg(tag).arg(i).arg(links?"href='/shared#fragment'":"");
        cards+=QStringLiteral("<article class=policy-item data-id=item-%1 style='top:%2px'>%3<button class=policy-menu>Menu %1</button></article>")
            .arg(i).arg((i-1)*60).arg(change=="per-item-shadow"?"":primary);
    }
    cards+="</div>";
    QVERIFY(page("window.collectionClicks=[];addEventListener('message',e=>{if(e.data&&e.data.kind==='collection-click')collectionClicks.push(e.data.index)});true").toBool());
    if(change=="foreign") {
        frameFixtures["/policy-cards.html"]=cards.toUtf8();
        const auto url=QStringLiteral("http://localhost:%1/policy-cards.html").arg(server.serverPort());
        QVERIFY(page("document.body.innerHTML='<iframe id=policy-frame style=\"width:750px;height:400px;border:0\"></iframe>';window.cardsLoaded=false;const f=document.getElementById('policy-frame');f.onload=()=>cardsLoaded=true;f.src="+quoted(url)+";true").toBool());
        QTRY_VERIFY(page("window.cardsLoaded===true").toBool());
        QVERIFY(page("document.getElementById('policy-frame').focus();true").toBool());
    } else if(change.startsWith("same-origin")) {
        frameFixtures["/policy-cards.html"]=cards.toUtf8();
        const auto url=QStringLiteral("http://127.0.0.1:%1/policy-cards.html").arg(server.serverPort());
        QVERIFY(page("document.body.innerHTML='<iframe id=policy-frame style=\"width:750px;height:400px;border:0\"></iframe>';window.cardsLoaded=false;const f=document.getElementById('policy-frame');f.onload=()=>cardsLoaded=true;f.src="+quoted(url)+";true").toBool());
        QTRY_VERIFY(page("window.cardsLoaded===true").toBool());
    } else if(change=="shadow"||change.startsWith("slot")) {
        QVERIFY(page("document.body.innerHTML='<div id=policy-host></div>';const host=document.getElementById('policy-host'),root=host.attachShadow({mode:'open'});root.innerHTML="+quoted(change.startsWith("slot")?QString("<slot id=policy-slot name=cards></slot>"):cards)+";true").toBool());
        if(change.startsWith("slot"))QVERIFY(page("document.getElementById('policy-host').insertAdjacentHTML('beforeend',"+quoted(cards)+");document.getElementById('policy-list').slot='cards';true").toBool());
    } else {
        QVERIFY(page("document.body.innerHTML="+quoted(cards)+";true").toBool());
    }
    if(change=="per-item-shadow")QVERIFY(page("document.querySelectorAll('.policy-item').forEach((item,i)=>{const root=item.attachShadow({mode:'open'});root.innerHTML='<button class=policy-primary id=card-'+(i+1)+' style=\"width:120px;height:30px\">Card '+(i+1)+'</button>';root.firstElementChild.onclick=()=>top.postMessage({kind:'collection-click',index:i+1},'*')});true").toBool());
    if(change=="top-to-bottom")QVERIFY(page("const list=document.getElementById('policy-list');list.prepend(list.lastElementChild);true").toBool());
    if(change=="row-ltr"||change=="row-rtl")QVERIFY(page("const rows=document.querySelectorAll('.policy-item');[[400,0],[0,7],[200,14],[0,70]].forEach((p,i)=>{rows[i].style.left=p[0]+'px';rows[i].style.top=p[1]+'px'});true").toBool());
    if(change=="display-contents")QVERIFY(page("document.getElementById('policy-list').style.display='contents';true").toBool());
    if(change=="tall-card")QVERIFY(page("document.querySelector('.policy-item').style.height='2000px';true").toBool());
    if(change=="clip")QVERIFY(page("const list=document.getElementById('policy-list');list.style.height='140px';list.style.overflow='hidden';true").toBool());
    if(change=="peer-css")QVERIFY(page("window.peerAnimation=document.querySelectorAll('.policy-item')[3].animate([{visibility:'hidden'},{visibility:'visible'}],{duration:1000,fill:'both'});peerAnimation.pause();peerAnimation.currentTime=0;true").toBool());
    if(change=="missing-identity")QVERIFY(page("document.querySelectorAll('.policy-item')[3].removeAttribute('data-id');true").toBool());
    if(change=="ambiguous-primary")QVERIFY(page("const p=document.getElementById('card-1'),clone=p.cloneNode(true);clone.id='extra';p.after(clone);true").toBool());
    if(change=="nested")QVERIFY(page("document.querySelector('.policy-item').append(document.querySelectorAll('.policy-item')[1]);true").toBool());
    if(change=="missing-container"||change=="absent-before")QVERIFY(page("document.getElementById('policy-list').id='other-list';true").toBool());
    if(change=="passive-burst")QVERIFY(page(R"JS(
        window.burstClicks=0;document.addEventListener('click',()=>burstClicks++);
        const columns=30,cellWidth=(innerWidth-40)/columns,cellHeight=(innerHeight-270)/17;
        for(let index=0;index<492;index++) {
            const button=document.createElement('button');button.textContent='Ordinary '+index;
            button.className='policy-primary';
            button.style=`width:${cellWidth-2}px;height:${cellHeight-2}px;padding:0;border:0;font-size:8px`;
            const item=document.createElement('article');item.className='policy-item';item.dataset.id='ordinary-'+index;
            item.style=`left:${(index%columns)*cellWidth}px;top:${240+Math.floor(index/columns)*cellHeight}px;width:${cellWidth}px;height:${cellHeight}px`;
            item.append(button);document.getElementById('policy-list').append(item);
        }
        document.querySelectorAll('button').length===500&&document.querySelectorAll('#policy-list>.policy-item').length===496
    )JS").toBool());
    QVERIFY(command(links?(change=="rapid-yank"?"hint --rapid --target yank links":"hint --target yank links"):change=="passive-burst"?"hint buttons":"hint actions"));
    QTRY_VERIFY_WITH_TIMEOUT(evaluate("!testWindow.hintWorkspace.hintRefreshPending&&testWindow.browserUi.hint_visible").toBool(),10000);
    auto *workspace=evaluate("testWindow.hintWorkspace").value<QObject *>();QVERIFY(workspace);
    const auto row=[this](const QString &code){for(const auto &entry:QJsonDocument::fromJson(text("hint_values").toUtf8()).array())if(entry.toObject().value("label").toString()==code)return entry.toObject();return QJsonObject();};
    const auto code=change=="multi-code"?QStringLiteral("aa"):QStringLiteral("1");
    const bool unavailable=change=="missing-identity"||change=="ambiguous-primary"||change=="nested"||change=="missing-container"||change=="absent-before"||change=="overlap";
    QCOMPARE(!row(code).isEmpty(),!unavailable);
    if(unavailable) {
        QVERIFY(!QJsonDocument::fromJson(text("hint_values").toUtf8()).array().isEmpty());
        if(change!="absent-before")return;
        QVERIFY(page("document.getElementById('other-list').id='policy-list';true").toBool());
        QVERIFY(QMetaObject::invokeMethod(workspace,"startHintCollection",Qt::DirectConnection));
        QTRY_VERIFY_WITH_TIMEOUT(!workspace->property("hintRefreshPending").toBool(),10000);
        QVERIFY(row("1").isEmpty());
        key(Qt::Key_Escape);QVERIFY(command("hint actions"));
        QTRY_VERIFY_WITH_TIMEOUT(!workspace->property("hintRefreshPending").toBool()&&!row("1").isEmpty(),10000);
    }
    if(change=="anchor-hole"||change=="duplicate-anchors") {
        QVERIFY(row("2").isEmpty());QVERIFY(!row("3").isEmpty());
        QCOMPARE(!row("m").isEmpty(),change=="anchor-hole");
    }
    if(change=="secondary") {
        const auto rows=QJsonDocument::fromJson(text("hint_values").toUtf8()).array();int menus=0;
        for(const auto &entry:rows)if(entry.toObject().value("text").toString().startsWith("Menu")){++menus;const auto label=entry.toObject().value("label").toString();QVERIFY(label!="1"&&label!="2"&&label!="3");}
        QCOMPARE(menus,4);
    }
    if(links)QVERIFY(row("1").value("element_id")!=row("2").value("element_id"));
    if(change=="passive-burst") {
        const auto completeCoverage=[this] {
            QStringList actual;
            for(const auto &entry:QJsonDocument::fromJson(text("hint_values").toUtf8()).array())actual.append(entry.toObject().value("text").toString());
            auto expected=page("Array.from(document.querySelectorAll('button'),button=>button.textContent.trim())").toStringList();
            actual.sort();expected.sort();
            return expected.size()==500&&actual==expected;
        };
        QVERIFY(completeCoverage());
        const auto ordinaryOwners=[this] {
            QJsonObject owners;
            for(const auto &entry:QJsonDocument::fromJson(text("hint_values").toUtf8()).array()) {
                const auto hint=entry.toObject();
                if(hint.value("text").toString().startsWith("Ordinary "))owners.insert(hint.value("label").toString(),hint.value("element_id"));
            }
            return owners;
        };
        const auto initialOwners=ordinaryOwners();QCOMPARE(initialOwners.size(),492);
        const auto annotation=QJsonDocument::fromJson(workspace->property("hintCollectionAnnotation").toString().toUtf8()).object();
        const auto outcomes=annotation.value("outcomes").toArray();QCOMPARE(outcomes.size(),1);
        QCOMPARE(outcomes.first().toObject().value("result").toObject().value("collection-complete").toObject().value("items").toArray().size(),496);
        const auto session=workspace->property("hintPresentationSessionId").toString();QVERIFY(!session.isEmpty());
        QSignalSpy publications(workspace,SIGNAL(hintPresentationSessionIdChanged()));QVERIFY(publications.isValid());
        QVERIFY(page(R"JS(
            const buttons=Array.from(document.querySelectorAll('button')).filter(button=>button.textContent.startsWith('Ordinary '));
            let writes=0;
            for(let index=0;index<1000;index++) {
                const color=Math.floor(index/buttons.length)%2?'rgb(3, 2, 1)':'rgb(1, 2, 3)';
                const button=buttons[index%buttons.length];if(button.style.outlineColor!==color)writes++;
                button.style.outlineColor=color;
            }
            const list=document.getElementById('policy-list'),fourth=list.querySelector('[data-id="item-4"]'),replacement=fourth.cloneNode(true);
            replacement.dataset.id='burst-inserted';replacement.querySelector('.policy-primary').textContent='Inserted card';
            replacement.querySelector('.policy-menu').textContent='Inserted menu';
            fourth.remove();list.prepend(replacement);
            writes===1000
        )JS").toBool());
        QTRY_VERIFY_WITH_TIMEOUT(!workspace->property("hintRefreshPending").toBool()
            &&workspace->property("hintPresentationSessionId").toString()!=session,10000);
        QTRY_VERIFY_WITH_TIMEOUT(completeCoverage(),10000);
        QVERIFY(row("1").isEmpty());QVERIFY(row("2").isEmpty());QVERIFY(row("3").isEmpty());
        QCOMPARE(text("mode"),QStringLiteral("hint"));
        QTest::qWait(250);
        QCOMPARE(publications.count(),1); // One passive publication for the whole mutation burst.
        QVERIFY(completeCoverage());
        QCOMPARE(ordinaryOwners(),initialOwners);
        QCOMPARE(page("window.burstClicks").toInt(),0);QCOMPARE(page("window.collectionClicks.length").toInt(),0);
        key(Qt::Key_Escape);return;
    }
    QVERIFY(suspendHintRefreshForAdapterFixture(workspace));
    if(change=="insert")QVERIFY(page("const list=document.getElementById('policy-list'),clone=list.firstElementChild.cloneNode(true);clone.dataset.id='inserted';clone.firstElementChild.id='inserted';clone.firstElementChild.textContent='Inserted card';clone.style.top='250px';list.prepend(clone);true").toBool());
    if(change=="remove-fourth")QVERIFY(page("document.querySelectorAll('.policy-item')[3].remove();true").toBool());
    if(change=="reorder")QVERIFY(page("const list=document.getElementById('policy-list');list.append(list.firstElementChild);true").toBool());
    if(change=="recycle")QVERIFY(page("document.querySelectorAll('.policy-item')[3].dataset.id='recycled';true").toBool());
    if(change=="peer-css") {
        const auto revision=page("window.__ferric_browserHintState.revision",1);
        QVERIFY(page("peerAnimation.currentTime=1000;true").toBool());
        QTRY_COMPARE(page("getComputedStyle(document.querySelectorAll('.policy-item')[3]).visibility").toString(),QStringLiteral("visible"));
        QCOMPARE(page("window.__ferric_browserHintState.revision",1),revision);
    }
    if(change=="layout")QVERIFY(page("document.querySelectorAll('.policy-item')[3].style.top='190px';true").toBool());
    if(change=="same-origin-mapping")QVERIFY(page("document.getElementById('policy-frame').style.marginLeft='20px';true").toBool());
    if(change=="slot-reassigned")QVERIFY(page("document.getElementById('policy-list').slot='other';true").toBool());
    typeText(code);
    const bool stale=change=="same-origin-mapping"||change=="slot-reassigned"||change=="insert"||change=="remove-fourth"||change=="reorder"||change=="recycle"||change=="peer-css"||change=="layout";
    if(stale) {
        QTRY_VERIFY_WITH_TIMEOUT(!workspace->property("hintRefreshPending").toBool()&&row("1").isEmpty(),10000);
        QVERIFY(row("2").isEmpty());QVERIFY(row("3").isEmpty());
        QCOMPARE(page("window.collectionClicks.length").toInt(),0);QCOMPARE(text("mode"),QStringLiteral("hint"));
        QVERIFY(text("status_text").contains("again",Qt::CaseInsensitive));
        QVERIFY(QMetaObject::invokeMethod(workspace,"startHintCollection",Qt::DirectConnection));
        QTRY_VERIFY_WITH_TIMEOUT(!workspace->property("hintRefreshPending").toBool(),10000);
        QVERIFY(row("1").isEmpty());QVERIFY(row("2").isEmpty());QVERIFY(row("3").isEmpty());
        if(change!="slot-reassigned") {
            key(Qt::Key_Escape);QVERIFY(command("hint actions"));
            QTRY_VERIFY_WITH_TIMEOUT(!workspace->property("hintRefreshPending").toBool()&&!row("1").isEmpty(),10000);
            if(change=="insert")QCOMPARE(row("1").value("text").toString(),QStringLiteral("Inserted card"));
            if(change=="reorder")QCOMPARE(row("1").value("text").toString(),QStringLiteral("Card 2"));
            QCOMPARE(page("window.collectionClicks.length").toInt(),0);
        }
    } else if(links) {
        QTRY_COMPARE_WITH_TIMEOUT(QGuiApplication::clipboard()->text(),QStringLiteral("http://127.0.0.1:%1/shared").arg(server.serverPort()),10000);
        if(change=="rapid-yank") {
            QTRY_VERIFY_WITH_TIMEOUT(!workspace->property("hintRefreshPending").toBool()&&row("1").isEmpty(),10000);
            QVERIFY(!row("2").isEmpty());QVERIFY(!row("3").isEmpty());
            const auto third=row("3").value("element_id");typeText("2");
            QTRY_VERIFY_WITH_TIMEOUT(!workspace->property("hintRefreshPending").toBool()&&row("2").isEmpty(),10000);
            QVERIFY(row("1").isEmpty());QCOMPARE(row("3").value("element_id"),third);
        }
    } else {
        QTRY_COMPARE_WITH_TIMEOUT(page("window.collectionClicks.length").toInt(),1,10000);
        QCOMPARE(page("window.collectionClicks[0]").toInt(),change=="row-ltr"?2:1);
        QTest::qWait(100);QCOMPARE(page("window.collectionClicks.length").toInt(),1);
    }
}

void ApplicationTests::hintAnchorActivation_data()
{
    QTest::addColumn<QString>("change");
    for(const auto *name:{"activate","pointer","link-current","tab","tab-bg","yank","download","focus","scroll",
                         "foreign","shadow","same-origin","slot","unrelated","unrelated-during-fresh","refresh-unrelated","duplicate-after","replacement","meaning","disabled",
                         "duplicate-before","absent-before","duplicate-resolved","duplicate-claims"})
        QTest::newRow(name)<<QString::fromLatin1(name);
}

void ApplicationTests::hintAnchorActivation()
{
    QFETCH(QString,change);
    const auto failureTrace=qScopeGuard([this]{
        if(!QTest::currentTestFailed())return;
        qInfo()<<"ANCHOR_FAILURE"<<evaluate("JSON.stringify({mode:testWindow.browserUi.mode,status:testWindow.browserUi.status_text,"
            "state:testWindow.hintState,refresh:testWindow.hintRefreshPending,epoch:testWindow.hintCollectionEpoch,"
            "pointer:testWindow.browserUi.hint_pointer_state,rows:JSON.parse(testWindow.browserUi.hint_values||'[]').filter(r=>r.label==='m')})").toString();
    });
    if(change=="focus")QVERIFY(page("(()=>{const target=document.createElement('div');target.id='focus-anchor';target.tabIndex=0;target.textContent='Focus-only anchor';target.style='position:fixed;left:530px;top:420px;width:220px;height:40px';document.body.append(target);return true})()").toBool());
    const auto clipboard=QGuiApplication::clipboard()->text();
    const auto restoreClipboard=qScopeGuard([&,clipboard]{if(change=="yank")QGuiApplication::clipboard()->setText(clipboard);});
    if(change=="yank")QGuiApplication::clipboard()->setText("anchor-sentinel");
    QTcpServer heldServer;QPointer<QTcpSocket> held;
    if(change=="download") {
        QVERIFY(heldServer.listen(QHostAddress::LocalHost,0));
        connect(&heldServer,&QTcpServer::newConnection,&heldServer,[&]{
            while(auto *socket=heldServer.nextPendingConnection()) {
                held=socket;connect(socket,&QTcpSocket::readyRead,socket,[socket]{socket->readAll();});
            }
        });
        QVERIFY(page("document.getElementById('same').href="+quoted(QStringLiteral("http://127.0.0.1:%1/held.bin").arg(heldServer.serverPort()))+";true").toBool());
    }
    if(change=="foreign") {
        frameFixtures["/anchor-foreign.html"]="<!doctype html><button id=foreign-primary autofocus onclick=\"parent.postMessage({kind:'anchor-click',trusted:event.isTrusted},'*')\">Foreign anchor</button>";
        const auto source=QStringLiteral("http://localhost:%1/anchor-foreign.html").arg(server.serverPort());
        QVERIFY(page("(()=>{window.anchorLoaded=false;addEventListener('message',event=>{if(event.data&&event.data.kind==='anchor-click'){window.clicks++;window.trust=event.data.trusted;}});const frame=document.getElementById('frame');frame.addEventListener('load',()=>window.anchorLoaded=true,{once:true});frame.src="+quoted(source)+";return true})()").toBool());
        QTRY_VERIFY(page("window.anchorLoaded===true").toBool());
        QVERIFY(page("document.getElementById('frame').focus();true").toBool());
    }
    if(change=="shadow")QVERIFY(page("document.getElementById('shadow').shadowRoot.querySelector('button').onclick=event=>{window.clicks++;window.trust=event.isTrusted};true").toBool());
    if(change=="same-origin") {
        QTRY_VERIFY(page("!!document.getElementById('frame').contentDocument.querySelector('button')").toBool());
        QVERIFY(page("document.getElementById('frame').contentDocument.querySelector('button').onclick=function(event){parent.clicks++;parent.trust=event.isTrusted};true").toBool());
    }
    if(change=="slot")QVERIFY(page("(()=>{const host=document.getElementById('shadow');host.shadowRoot.innerHTML='<slot id=policy-slot name=target></slot>';const item=document.createElement('div');item.id='slot-item';item.slot='target';item.innerHTML='<button id=slot-button>Slot anchor</button>';item.firstElementChild.onclick=event=>{window.clicks++;window.trust=event.isTrusted};host.append(item);return true})()").toBool());
    const auto duplicate=[this]{return page("(()=>{const clone=document.getElementById('button').cloneNode(true);clone.style='position:absolute;left:530px;top:400px';window.extraAnchor=clone;document.body.append(clone);return true})()").toBool();};
    const bool initiallyUnavailable=change=="duplicate-before"||change=="absent-before"||change=="duplicate-resolved"||change=="duplicate-claims";
    if(change=="duplicate-before"||change=="duplicate-resolved")QVERIFY(duplicate());
    if(change=="absent-before")QVERIFY(page("window.savedAnchor=document.getElementById('button').outerHTML;document.getElementById('button').remove();true").toBool());
    const auto tabs=evaluate("testWindow.browserUi.tab_count").toInt();
    if(change=="pointer") {QVERIFY(command("bind p hint --target pointer all"));key(Qt::Key_P);}
    else if(change=="link-current")QVERIFY(command("hint links"));
    else if(change=="tab"||change=="tab-bg"||change=="yank"||change=="download")QVERIFY(command("hint --target "+change+" links"));
    else if(change=="focus")QVERIFY(command("hint focus"));
    else if(change=="scroll")QVERIFY(command("hint scrollables"));
    else QVERIFY(command("hint all"));
    QTRY_VERIFY_WITH_TIMEOUT(evaluate("!testWindow.hintWorkspace.hintRefreshPending && testWindow.browserUi.hint_visible").toBool(),10000);
    auto *workspace=evaluate("testWindow.hintWorkspace").value<QObject *>();QVERIFY(workspace);
    const auto hasCode=[this](const QString &code){
        const auto rows=QJsonDocument::fromJson(text("hint_values").toUtf8()).array();
        for(const auto &row:rows)if(row.toObject().value("label").toString()==code)return true;
        return false;
    };
    QCOMPARE(hasCode("m"),!initiallyUnavailable);
    QCOMPARE(page("window.clicks").toInt(),0);
    if(change=="absent-before"||change=="duplicate-resolved"||change=="refresh-unrelated") {
        if(change=="absent-before")QVERIFY(page("document.body.insertAdjacentHTML('afterbegin',window.savedAnchor);true").toBool());
        else if(change=="duplicate-resolved")QVERIFY(page("window.extraAnchor.remove();true").toBool());
        else QVERIFY(page("document.body.dataset.unrelated='changed';true").toBool());
        QVERIFY(QMetaObject::invokeMethod(workspace,"startHintCollection",Qt::DirectConnection));
        QTRY_VERIFY_WITH_TIMEOUT(!workspace->property("hintRefreshPending").toBool()
            &&!workspace->property("hintCollectionAnnotation").toString().isEmpty(),10000);
        const auto annotation=QJsonDocument::fromJson(workspace->property("hintCollectionAnnotation").toString().toUtf8()).object();
        QVERIFY(!annotation.isEmpty());
        QCOMPARE(annotation.value("outcomes").toArray().first().toObject().value("result").toObject().value("anchor-complete").toObject().value("matches").toArray().size(),1);
        QCOMPARE(hasCode("m"),change=="refresh-unrelated");
        if(change!="refresh-unrelated") {
            typeText("m");QCOMPARE(page("window.clicks").toInt(),0);
            key(Qt::Key_Escape);QVERIFY(command("hint all"));
            QTRY_VERIFY_WITH_TIMEOUT(!workspace->property("hintRefreshPending").toBool()&&hasCode("m"),10000);
        }
    }
    if(change=="duplicate-before"||change=="duplicate-claims") {
        if(change=="duplicate-claims")QVERIFY(!hasCode("n"));
        typeText("m");QCOMPARE(page("window.clicks").toInt(),0);
        evaluate("testWindow.hintWorkspace.applyHintInteraction('clear','')");
        const auto fallback=hintLabel("Activate exact button");
        QVERIFY(!fallback.isEmpty()&&fallback!="m"&&fallback!="n");
        typeText(fallback);
    } else {
        QVERIFY(suspendHintRefreshForAdapterFixture(workspace));
        if(change=="duplicate-after")QVERIFY(duplicate());
        if(change=="replacement")QVERIFY(page("(()=>{const old=document.getElementById('button');old.replaceWith(old.cloneNode(true));return true})()").toBool());
        if(change=="meaning")QVERIFY(page("document.getElementById('button').textContent='Changed meaning';true").toBool());
        if(change=="disabled")QVERIFY(page("document.getElementById('button').disabled=true;true").toBool());
        if(change=="unrelated")QVERIFY(page("document.body.dataset.unrelated='changed';true").toBool());
        if(change=="unrelated-during-fresh") {
            // Insert an unrelated mutation after the bounded uniqueness proof
            // and action freshness, exactly before the native final control.
            scriptDone=false;
            evaluate(R"JS((function(){
                const workspace=testWindow.hintWorkspace,view=testWindow.activeWebView();
                const selected=JSON.parse(testWindow.browserUi.hint_values).find(row=>row.label==='m');
                workspace.preparePolicyHint('m',view,function(request){
                    workspace.hintFrameProvider.fresh(selected,workspace.hintFreshScript(selected),function(value){
                        if(!value||!value.visible){testProbe.accept({error:'fresh-target-missing'});return;}
                        view.runJavaScript("document.body.dataset.unrelated='during-fresh';true",0,function(){
                            workspace.confirmPolicyHint(request,function(response){testProbe.accept(response);});
                        });
                    });
                });
            })())JS");
            QTRY_VERIFY_WITH_TIMEOUT(scriptDone,10000);
            QVERIFY2(scriptResult.toMap().value("ok").toBool(),qPrintable(scriptResult.toString()));
            QVERIFY(hasCode("m"));
            QCOMPARE(page("document.body.dataset.unrelated").toString(),QStringLiteral("during-fresh"));
            QCOMPARE(page("window.clicks").toInt(),0);
        }
        typeText("m");
        if(change=="pointer")key(Qt::Key_Return);
    }
    const bool rejected=change=="duplicate-after"||change=="replacement"||change=="meaning"||change=="disabled";
    if(change=="link-current") {
        QTRY_COMPARE_WITH_TIMEOUT(page("location.hash").toString(),QStringLiteral("#fragment"),10000);
    } else if(change=="tab"||change=="tab-bg") {
        QTRY_COMPARE_WITH_TIMEOUT(evaluate("testWindow.browserUi.tab_count").toInt(),tabs+1,10000);
        if(change=="tab-bg")QVERIFY(page("location.search").toString().isEmpty());
    } else if(change=="yank") {
        // Existing Hint yank sanitizes URLs, including removal of fragments.
        QTRY_COMPARE_WITH_TIMEOUT(QGuiApplication::clipboard()->text(),
            QStringLiteral("http://127.0.0.1:%1/fixture.html").arg(server.serverPort()),10000);
        QCOMPARE(page("window.clicks").toInt(),0);
    } else if(change=="download") {
        QTRY_VERIFY_WITH_TIMEOUT(!text("download_request_token").isEmpty()&&!held.isNull(),10000);
        evaluate("testWindow.browserUi.cancel_download_request(testWindow.browserUi.download_request_token)");
        QTRY_VERIFY(held.isNull()||held->state()==QAbstractSocket::UnconnectedState);
        QCOMPARE(evaluate("testWindow.browserUi.download_rows().length").toInt(),0);
    } else if(change=="focus") {
        QTRY_COMPARE_WITH_TIMEOUT(page("document.activeElement.id").toString(),QStringLiteral("focus-anchor"),10000);
        QCOMPARE(page("Number(window.allClicks||0)").toInt(),0);
    } else if(change=="scroll") {
        QTRY_VERIFY_WITH_TIMEOUT(page("(()=>{const s=window.__ferric_browserScrollTargetState;return !!s&&s.policy==='element'&&WeakRef.prototype.deref.call(s.elementRef)===document.querySelector('section')})()",1).toBool(),10000);
        QCOMPARE(page("window.clicks").toInt(),0);
    } else if(rejected) {
        QTRY_VERIFY_WITH_TIMEOUT(!workspace->property("hintRefreshPending").toBool()&&!hasCode("m"),10000);
        QCOMPARE(page("window.clicks").toInt(),0);
        QCOMPARE(text("mode"),QStringLiteral("hint"));
        QVERIFY(!QJsonDocument::fromJson(text("hint_values").toUtf8()).array().isEmpty());
        QVERIFY(text("status_text").contains("changed",Qt::CaseInsensitive));
    } else {
        QTRY_COMPARE_WITH_TIMEOUT(page("window.clicks").toInt(),1,10000);
        QCOMPARE(page("window.trust").toBool(),change=="pointer");
        QTest::qWait(150);QCOMPARE(page("window.clicks").toInt(),1);
    }
}

void ApplicationTests::hintRequestWorkspaceCollection()
{
    QFETCH(bool,foreign);
    const auto rootCase=QString::fromLatin1(QTest::currentDataTag());
    const bool collectionFailure=rootCase.startsWith("ordinary-");
    if(rootCase.startsWith("busy-")) {
        // Change the real DOM after every suspension, including a refreshed
        // ordinary pass. There is never a quiet async gap for sealing.
        QVERIFY(page(R"JS((()=>{
            window.busyUpdates=0;window.busyRequests=0;window.busyLastPlan=null;
            function watch(state){
                if(!state)return;
                let pending=state.pendingInteraction;
                Object.defineProperty(state,'pendingInteraction',{configurable:true,
                    get(){return pending;},set(value){pending=value;if(value){
                        if(value.plan!==window.busyLastPlan){window.busyLastPlan=value.plan;++window.busyRequests;}
                        queueMicrotask(()=>{
                            document.documentElement.setAttribute('data-clock',String(++window.busyUpdates));
                        });
                    }}});
            }
            let state=window.__ferric_browserHintState;watch(state);
            Object.defineProperty(window,'__ferric_browserHintState',{configurable:true,
                get(){return state;},set(value){state=value;watch(value);}});
            return true;
        })())JS",1).toBool());
    }
    if(collectionFailure) {
        // Delay the actual isolated collector at suspension so the native
        // deadline expires. The next attempt must acquire a fresh request.
        const auto limit=rootCase=="ordinary-timeout-twice"?2:1;
        QVERIFY(page(QString::fromLatin1(R"JS((()=>{
            window.collectionAttempts=0;
            function watch(state){
                if(!state)return;
                let pending=state.pendingInteraction;
                Object.defineProperty(state,'pendingInteraction',{configurable:true,
                    get(){return pending;},set(value){
                        pending=value;
                        if(!value)return;
                        ++window.collectionAttempts;
                        if(%1)throw {ferricRootReason:'inspection-limit'};
                        if(window.collectionAttempts<=%2){
                            const until=performance.now()+1150;
                            while(performance.now()<until){}
                        }
                    }});
            }
            let state=window.__ferric_browserHintState;watch(state);
            Object.defineProperty(window,'__ferric_browserHintState',{configurable:true,
                get(){return state;},set(value){state=value;watch(value);}});
            return true;
        })())JS").arg(rootCase=="ordinary-evidence-limit"?"true":"false").arg(limit),1).toBool());
    }
    if(qEnvironmentVariableIsSet("FERRIC_WORKSPACE_TRACE")) {
        QVERIFY(page(R"JS((()=>{
            const trace=[];window.__workspaceCollectionTrace=trace;
            function watch(state){
                if(!state)return;
                function note(kind,key,value){
                    if(trace.length===64)trace.shift();
                    trace.push({kind:kind,key:key,value:typeof value==='object'?!!value:value,
                        revision:state.revision,dirty:state.dirty,
                        stack:String(new Error().stack).split('\n').slice(0,3).join('\n')});
                }
                for(const key of ['dirty','revision','pendingInteraction','interactionProof']){
                    let held=state[key];
                    Object.defineProperty(state,key,{configurable:true,enumerable:true,
                        get(){if(key==='pendingInteraction')note('read',key,held);return held;},
                        set(value){held=value;note('write',key,value);}});
                }
            }
            let state=window.__ferric_browserHintState;watch(state);
            Object.defineProperty(window,'__ferric_browserHintState',{configurable:true,
                get(){return state;},set(value){state=value;watch(value);}});
            return true;
        })())JS",1).toBool());
    }
    if(foreign) {
        frameFixtures["/workspace-policy-child.html"]=rootCase==QStringLiteral("foreign-shadow")
            ?QByteArray("<!doctype html><div id='foreign-policy-host'></div><script>const root=document.getElementById('foreign-policy-host').attachShadow({mode:'open'});root.innerHTML='<button id=foreign-primary style=\"width:180px;height:32px\">Foreign receiver</button>';root.getElementById('foreign-primary').focus();</script>")
            :QByteArray("<!doctype html><button id='foreign-primary' autofocus>Foreign receiver</button>");
        const auto url=QStringLiteral("http://localhost:%1/workspace-policy-child.html").arg(server.serverPort());
        QVERIFY(page("(()=>{const frame=document.getElementById('frame');window.workspaceChildLoaded=false;"
            "frame.addEventListener('load',()=>window.workspaceChildLoaded=true,{once:true});frame.src="
            +quoted(url)+";return true})()",1).toBool());
        QTRY_VERIFY(page("window.workspaceChildLoaded===true",1).toBool());
        QVERIFY(page("document.getElementById('frame').focus();true",1).toBool());
    }
    if(rootCase==QStringLiteral("same-origin-primary"))
        QVERIFY(page("document.getElementById('frame').dataset.use='yes';true",1).toBool());
    if(rootCase==QStringLiteral("shadow-slot"))
        QVERIFY(page("(()=>{const host=document.createElement('div');host.id='policy-shadow';host.style='position:fixed;left:530px;top:420px;width:300px;height:80px';host.attachShadow({mode:'open'}).innerHTML='<slot id=policy-slot name=target></slot>';const item=document.createElement('div');item.id='policy-slot-item';item.slot='target';item.style='width:300px;height:80px';item.innerHTML='<button id=policy-slot-button>Slotted policy action</button>';host.append(item);document.body.append(host);return true})()",1).toBool());
    QElapsedTimer collectionClock;collectionClock.start();
    QVERIFY(command("hint all"));
    if (rootCase=="ordinary-timeout-cancel" || rootCase=="ordinary-timeout-reentry" || rootCase=="ordinary-timeout-navigation") {
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.hintWorkspace.hintCollectionRetryCount===1 && testWindow.hintRefreshPending").toBool(), 2000);
        key(Qt::Key_Escape);
        if (rootCase=="ordinary-timeout-navigation") {
            QVERIFY(command(QString("open http://127.0.0.1:%1/next").arg(server.serverPort())));
            QTest::qWait(500);
            QCOMPARE(text("mode"),QString("normal"));
        } else if (rootCase=="ordinary-timeout-reentry") {
            QVERIFY(command("hint all"));
            QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.hintWorkspace.hintPresentationSessionId.length>0 && !testWindow.hintRefreshPending").toBool(), 5000);
            QTest::qWait(200);
            QCOMPARE(text("mode"),QString("hint"));
        } else {
            QTest::qWait(600);
            QCOMPARE(text("mode"),QString("normal"));
            QCOMPARE(page("window.collectionAttempts",1).toInt(),1);
        }
        return;
    }
    if(rootCase=="ordinary-timeout-twice"||rootCase=="ordinary-evidence-limit") {
        const int attempts=rootCase=="ordinary-timeout-twice"?2:1;
        QTRY_COMPARE_WITH_TIMEOUT(page("window.collectionAttempts",1).toInt(),attempts,10000);
        QTRY_COMPARE_WITH_TIMEOUT(text("mode"),QStringLiteral("normal"),10000);
        QVERIFY(QJsonDocument::fromJson(text("hint_values").toUtf8()).array().isEmpty());
        QTest::qWait(100);
        QCOMPARE(page("window.collectionAttempts",1).toInt(),attempts);
        return;
    }
    QTRY_VERIFY2_WITH_TIMEOUT(evaluate("!testWindow.hintWorkspace.hintRefreshPending && testWindow.browserUi.hint_visible").toBool(),
        qPrintable(page("JSON.stringify(window.__workspaceCollectionTrace)",1).toString()),10000);
    auto *workspace=evaluate("testWindow.hintWorkspace").value<QObject *>();QVERIFY(workspace);
    // The busy cases prove initial collection and sealing without a quiet
    // gap. Freeze passive polling only after publication so the adapter-handle
    // assertions below cannot race a legitimate next refresh. Production
    // refresh concurrency remains exercised by passive-burst activation.
    if(rootCase.startsWith("busy-"))QVERIFY(suspendHintRefreshForAdapterFixture(workspace));
    auto *adapter=workspace->findChild<FerricHintRequestAdapter *>();QVERIFY(adapter);
    auto handle=workspace->property("hintCollectionRequest");
    QCOMPARE(adapter->collectionState(handle),QStringLiteral("proof-ready"));
    if(rootCase.startsWith("busy-")) {
        QVERIFY(page("window.busyUpdates>=2",1).toBool());
        QCOMPARE(page("window.busyRequests",1).toInt(),1);
    }
    qInfo().noquote()<<"HINT_COLLECTION_MEASUREMENT"<<QJsonDocument(QJsonObject{
        {"scenario",rootCase},{"publication_ms",double(collectionClock.elapsed())},
        {"requests",rootCase.startsWith("busy-")?QJsonValue(page("window.busyRequests",1).toInt()):QJsonValue(QJsonValue::Null)}}).toJson(QJsonDocument::Compact);
    if(rootCase=="ordinary-timeout")QCOMPARE(page("window.collectionAttempts",1).toInt(),2);
    QVERIFY(adapter->collectionWorkPlan(handle).isEmpty());
    const auto worlds=QJsonDocument::fromJson(evaluate("JSON.stringify(testWindow.hintWorkspace.hintCollectionWorlds)").toString().toUtf8()).array();
    QCOMPARE(worlds.size(),foreign?2:1);
    const auto annotation=QJsonDocument::fromJson(workspace->property("hintCollectionAnnotation").toString().toUtf8()).object();
    QVERIFY(!annotation.isEmpty());
    QSet<QString> matched;
    for(const auto &value:annotation.value("outcomes").toArray()) {
        const auto outcome=value.toObject();
        if(outcome.value("result").toObject().value("anchor-complete").toObject().value("matches").toArray().size()==1)
            matched.insert(outcome.value("rule").toObject().value("id").toString());
    }
    QVERIFY(matched.contains(QStringLiteral("primary")));
    if(foreign)QVERIFY(matched.contains(QStringLiteral("foreign-primary")));
    if(rootCase==QStringLiteral("shadow-slot"))QVERIFY(matched.contains(QStringLiteral("slotted")));
    if(rootCase==QStringLiteral("same-origin-primary")) {
        bool found=false;
        for(const auto &value:annotation.value("outcomes").toArray()) {
            const auto outcome=value.toObject();
            if(outcome.value("rule").toObject().value("id").toString()==QStringLiteral("excluded-child"))
                found=outcome.value("result").toObject().value("collection-complete").toObject().value("items").toArray().size()==1;
        }
        QVERIFY(found);
    }
    QVERIFY(adapter->finishCollection(handle).isEmpty());
    QCOMPARE(hintLabel("Activate exact button"),QStringLiteral("m"));
    QCOMPARE(QJsonDocument::fromJson(adapter->publishCollection(handle).toUtf8()).object().value("error").toString(),
        QStringLiteral("policy-collection-already-published"));
    QCOMPARE(QJsonDocument::fromJson(evaluate("testWindow.browserUi.begin_hint_session('{}')").toString().toUtf8()).object().value("error").toString(),
        QStringLiteral("native-policy-evidence-required"));
    for(const auto &entry:worlds) {
        const auto world=entry.toObject();
        const auto owner=world.value("owner").toString();
        QVERIFY(!adapter->collectionGeometry(handle,owner).isEmpty());
        const auto annotation=QJsonDocument::fromJson(world.value("annotation_json").toString().toUtf8()).object();
        QVERIFY(!annotation.isEmpty());
        QCOMPARE(annotation.value("stamp").toObject().value("family").toString(),QStringLiteral("all"));
        QVERIFY(annotation.value("outcomes").isArray());
    }
    if(foreign)QVERIFY(!hintLabel("Foreign receiver").isEmpty());
    const auto original=handle;
    QVERIFY(QMetaObject::invokeMethod(workspace,"startHintCollection",Qt::DirectConnection));
    QTRY_VERIFY_WITH_TIMEOUT(evaluate("!testWindow.hintWorkspace.hintRefreshPending && testWindow.browserUi.hint_visible").toBool(),10000);
    QCOMPARE(adapter->collectionState(original),QStringLiteral("closed"));
    handle=workspace->property("hintCollectionRequest");
    QCOMPARE(adapter->collectionState(handle),QStringLiteral("proof-ready"));
    evaluate("testWindow.browserUi.escape()");
    QCOMPARE(adapter->collectionState(handle),QStringLiteral("closed"));
    QCOMPARE(evaluate("testWindow.hintWorkspace.hintCollectionWorlds.length").toInt(),0);
    QVERIFY(workspace->property("hintCollectionAnnotation").toString().isEmpty());
}

void ApplicationTests::hintRequestCaptureClassification()
{
    QFETCH(QString,change);
    if(change=="non-http") {
        const auto url=QUrl::fromLocalFile(directory.filePath("fixture.html"));
        QVERIFY(evaluate("testWindow.activeWebView().url="+quoted(url.toString())+";true").toBool());
        QTRY_COMPARE(page("location.href").toString(),url.toString());
        QTRY_VERIFY(page("!!document.getElementById('button')").toBool());
    }
    QVERIFY(command("hint all"));
    QTRY_COMPARE(text("mode"),QStringLiteral("hint"));
    QTRY_VERIFY(evaluate("!testWindow.hintWorkspace.hintRefreshPending && testWindow.browserUi.hint_visible").toBool());
    auto *view=qobject_cast<QQuickItem *>(evaluate("testWindow.activeWebView()").value<QObject *>());
    auto *provider=evaluate("testWindow.hintWorkspace.hintFrameProvider").value<QObject *>();
    auto *lifetime=qobject_cast<FerricProfileLifetime *>(evaluate(
        "testWindow.profileResourceRegistry.forProfile(testWindow.activeWebView().profile).lifetime").value<QObject *>());
    QVERIFY(view);QVERIFY(provider);QVERIFY(lifetime);
    const auto brokers=provider->findChildren<FerricHintFrameBroker *>();
    QCOMPARE(brokers.size(),1);
    const auto makeAdapter=[this,lifetime] {
        auto *value=qobject_cast<FerricHintRequestAdapter *>(evaluate("Qt.createQmlObject("+quoted(
            "import QtQuick; import io.github.ferricbrowser 1.0; FerricHintRequestAdapter {}")+",testWindow)").value<QObject *>());
        if(value)value->setLifetime(lifetime);
        return value;
    };
    auto *adapter=makeAdapter();QVERIFY(adapter);
    engine->globalObject().setProperty("__ferricClassificationAdapter",engine->newQObject(adapter));
    engine->globalObject().setProperty("__ferricClassificationView",engine->newQObject(view));
    engine->globalObject().setProperty("__ferricClassificationBroker",engine->newQObject(brokers.first()));
    engine->globalObject().setProperty("__ferricClassificationCarrier",engine->newObject());
    const auto clearCarrier=qScopeGuard([this] {
        for(const auto *name:{"__ferricClassificationAdapter","__ferricClassificationView",
                             "__ferricClassificationBroker","__ferricClassificationCarrier"})
            engine->globalObject().deleteProperty(QString::fromLatin1(name));
    });
    // Exercise the actual QML carrier round trip, including the opaque value
    // nested inside the native result map, before reading it from C++ too.
    const auto captured=evaluate("__ferricClassificationCarrier.capture=__ferricClassificationAdapter.beginCollection("
        "__ferricClassificationView,__ferricClassificationBroker);__ferricClassificationCarrier.capture").toMap();
    const auto expected=change=="labels-only"||change=="page-keys-only"||change=="anchor"
        ? QStringLiteral("proof-ready"):QStringLiteral("ordinary");
    QCOMPARE(captured.value("state").toString(),expected);
    const auto handle=captured.value("request");
    QVERIFY(handle.isValid());
    QCOMPARE(adapter->collectionState(handle),expected);
    QCOMPARE(evaluate("__ferricClassificationAdapter.collectionState(__ferricClassificationCarrier.capture.request)").toString(),expected);
    QCOMPARE(evaluate("__ferricClassificationAdapter.collectionState({state:'ordinary'})").toString(),QStringLiteral("closed"));
    QVERIFY(!adapter->collectorSelectors(handle).isEmpty());
    if(change=="non-http") {
        const auto selectors=QJsonDocument::fromJson(adapter->collectorSelectors(handle).toUtf8()).object();
        const auto policy=selectors.value("policies").toObject().value("all").toObject().value("policy").toObject();
        QCOMPARE(policy.value("mode").toString(),QStringLiteral("replace"));
        QCOMPARE(policy.value("selectors").toArray(),QJsonArray{"#button"});
    }
    QVERIFY(!adapter->observationObjectsChecked(handle));
    QCOMPARE(adapter->collectionState(QVariantMap{{"state","ordinary"}}),QStringLiteral("closed"));
    if(change=="stop") {
        evaluate("testWindow.browserUi.escape()");
        QTRY_COMPARE(text("mode"),QStringLiteral("normal"));
        QCOMPARE(adapter->collectionState(handle),QStringLiteral("closed"));
        QVERIFY(adapter->collectorSelectors(handle).isEmpty());
    } else if(change=="replacement") {
        const auto newer=adapter->beginCollection(view,brokers.first());
        QCOMPARE(newer.value("state").toString(),expected);
        QVERIFY(newer.value("request").isValid());
        QCOMPARE(adapter->collectionState(handle),QStringLiteral("closed"));
        QCOMPARE(adapter->collectionState(newer.value("request")),expected);
    } else if(change=="other-adapter") {
        auto *other=makeAdapter();QVERIFY(other);
        QCOMPARE(other->collectionState(handle),QStringLiteral("closed"));
        delete other;
    } else if(change=="invalid-view") {
        const auto invalid=adapter->beginCollection(nullptr,brokers.first());
        QCOMPARE(invalid.value("state").toString(),QStringLiteral("closed"));
        QVERIFY(!invalid.contains("request"));
    }
    QCOMPARE(page("window.clicks").toInt(),0);
    delete adapter;
}

void ApplicationTests::hintRequestCollectionBinding_data()
{
    QTest::addColumn<QString>("change");
    for(const auto *name:{"top-only","optional-focused","optional-foreign","optional-missing","optional-unfocused",
                         "optional-timeout","top-changed-during-timeout","required-foreign","required-unfocused",
                         "required-substitution","required-same-origin","invalid-frozen-owner","publication-before-bind",
                         "ordinary-non-http","required-omitted-scope","provider-epoch","collection-epoch"})
        QTest::newRow(name)<<QString::fromLatin1(name);
}

void ApplicationTests::hintRequestCollectionBinding()
{
    QFETCH(QString,change);
    const bool ordinary=change=="ordinary-non-http";
    const bool required=change.startsWith("required-");
    const bool timeout=change.contains("timeout");
    const bool hasChild=change!="top-only"&&change!="optional-missing"&&change!="invalid-frozen-owner"
        &&change!="publication-before-bind"&&!ordinary;
    const bool foreign=change.contains("foreign")||timeout||change=="required-unfocused"||change=="required-substitution";
    if(ordinary) {
        const auto url=QUrl::fromLocalFile(directory.filePath("fixture.html"));
        QVERIFY(evaluate("testWindow.activeWebView().url="+quoted(url.toString())+";true").toBool());
        QTRY_COMPARE(page("location.href").toString(),url.toString());
        QTRY_VERIFY(page("!!document.getElementById('button')").toBool());
    }
    QVERIFY(command("hint all"));
    QTRY_COMPARE(text("mode"),QStringLiteral("hint"));
    QTRY_VERIFY(evaluate("!testWindow.hintWorkspace.hintRefreshPending && testWindow.browserUi.hint_visible").toBool());
    auto *view=qobject_cast<QQuickItem *>(evaluate("testWindow.activeWebView()").value<QObject *>());
    auto *provider=evaluate("testWindow.hintWorkspace.hintFrameProvider").value<QObject *>();
    auto *lifetime=qobject_cast<FerricProfileLifetime *>(evaluate(
        "testWindow.profileResourceRegistry.forProfile(testWindow.activeWebView().profile).lifetime").value<QObject *>());
    QVERIFY(view);QVERIFY(provider);QVERIFY(lifetime);
    const auto brokers=provider->findChildren<FerricHintFrameBroker *>();QCOMPARE(brokers.size(),1);
    auto *broker=brokers.first();
    QQmlEngine::setObjectOwnership(this,QQmlEngine::CppOwnership);
    engine->globalObject().setProperty("__ferricCollectionBindingTest",engine->newQObject(this));
    const auto clearGlobal=qScopeGuard([this]{engine->globalObject().deleteProperty("__ferricCollectionBindingTest");});
    const auto completion=engine->evaluate("(function(value){__ferricCollectionBindingTest.accept(value)})");
    QVERIFY(completion.isCallable());
    view->forceActiveFocus();
    QVERIFY(page("document.getElementById('button').focus();true",1).toBool());
    QTRY_VERIFY(page("document.hasFocus()",1).toBool());
    const auto discover=[this] {
        scriptDone=false;
        evaluate("testWindow.hintWorkspace.hintFrameProvider.discover(testWindow.hintWorkspace.hintFrameProvider.rootOwner,0,false,false,"
                 "function(value){__ferricCollectionBindingTest.accept(value)})");
    };
    QString selectedOwner;
    const auto foreignUrl=QStringLiteral("http://localhost:%1/binding-foreign.html").arg(server.serverPort());
    if(hasChild) {
        if(foreign) {
            frameFixtures["/binding-foreign.html"]="<!doctype html><button autofocus>Binding receiver</button>";
            QVERIFY(page("(()=>{const frame=document.getElementById('frame');window.bindingForeignLoaded=false;frame.addEventListener('load',()=>window.bindingForeignLoaded=true,{once:true});frame.src="+quoted(foreignUrl)+";return true})()",1).toBool());
            QTRY_VERIFY(page("window.bindingForeignLoaded===true",1).toBool());
            QTRY_VERIFY(page("document.getElementById('frame').contentDocument===null",1).toBool());
            QVERIFY(page("document.getElementById('frame').focus();true",1).toBool());
        } else {
            QVERIFY(page("document.getElementById('frame').contentDocument.querySelector('button').focus();true",1).toBool());
            QTRY_VERIFY(page("document.getElementById('frame').contentDocument.hasFocus()",1).toBool());
        }
        if(!required) {
            discover();QTRY_VERIFY(scriptDone);QVERIFY(!scriptResult.toMap().contains("error"));
            selectedOwner=scriptResult.toMap().value("owner").toString();
            QVERIFY(!selectedOwner.isEmpty());QVERIFY(selectedOwner!=provider->property("rootOwner").toString());
        }
    }
    if(required) {
        QVERIFY(page("document.getElementById('frame').title='Entered binding subject';true",1).toBool());
        key(Qt::Key_Escape);
        QTRY_COMPARE(text("mode"),QStringLiteral("normal"));
        QVERIFY(command("hint --target choose all"));
        const auto label=hintLabel("Entered binding subject");QVERIFY(!label.isEmpty());
        const auto traceEntry=[this](const char *stage){
            if(!qEnvironmentVariableIsSet("FERRIC_BINDING_ENTRY_TRACE"))return;
            qInfo()<<stage<<evaluate("JSON.stringify({mode:testWindow.browserUi.mode,status:testWindow.browserUi.status_text,"
                "state:testWindow.hintState,pending:testWindow.pendingContextMenuRequest!==null,"
                "epoch:testWindow.hintCollectionEpoch,refresh:testWindow.hintRefreshPending})").toString();
        };
        const auto traceExit=qScopeGuard([&]{traceEntry("entry-exit");});
        traceEntry("before-label");
        // FullMatch opens the chooser on the last label character. Another
        // Enter would trigger its first action before we inspect the menu.
        typeText(label);
        traceEntry("after-label");
        QTRY_VERIFY(evaluate("testWindow.pendingContextMenuRequest!==null").toBool());
        QVERIFY(chooseCapability("hint-inside-frame"));
        QTRY_VERIFY(!provider->property("scopedOwner").toString().isEmpty());
        selectedOwner=provider->property("scopedOwner").toString();
        QVERIFY(!hintLabel(foreign?"Binding receiver":"Frame control").isEmpty());
        if(change=="required-same-origin") {
            QTRY_VERIFY(!evaluate("testWindow.hintWorkspace.hintCollectionAnnotation").toString().isEmpty());
            const auto rows=QJsonDocument::fromJson(text("hint_values").toUtf8()).array();
            QVERIFY(!rows.isEmpty());
            for(const auto &row:rows) {
                const auto candidate=row.toObject();
                QVERIFY(candidate.value("frame_owner").toString().isEmpty());
                QVERIFY(candidate.value("frame_path").toString()!=QStringLiteral("0"));
            }
        }
    }
    QTRY_VERIFY(!evaluate("testWindow.hintWorkspace.hintRefreshPending").toBool());
    auto *workspace=evaluate("testWindow.hintWorkspace").value<QObject *>();QVERIFY(workspace);
    QVERIFY(suspendHintRefreshForAdapterFixture(workspace));
    QTRY_VERIFY(!provider->property("pollPending").toBool()&&!workspace->property("hintRefreshPending").toBool());
    auto *adapter=qobject_cast<FerricHintRequestAdapter *>(evaluate("Qt.createQmlObject("+quoted(
        "import QtQuick; import io.github.ferricbrowser 1.0; FerricHintRequestAdapter {}")+",testWindow)").value<QObject *>());
    QVERIFY(adapter);adapter->setLifetime(lifetime);
    const auto clearAdapter=qScopeGuard([adapter]{delete adapter;});
    const auto frozenOwner=change=="invalid-frozen-owner"?QStringLiteral("FRAME-invalid")
        :(required&&change!="required-omitted-scope"?selectedOwner:QString());
    const auto capture=adapter->beginCollection(view,broker,frozenOwner);
    if(change=="invalid-frozen-owner"||change=="required-omitted-scope") {
        QCOMPARE(capture.value("state").toString(),QStringLiteral("closed"));
        QVERIFY(!capture.contains("request"));return;
    }
    const auto pending=capture.value("request");QVERIFY2(pending.isValid(),qPrintable(capture.value("state").toString()));
    const auto mode=ordinary?QStringLiteral("ordinary"):QStringLiteral("proof-ready");
    QCOMPARE(capture.value("state").toString(),mode);
    const auto originalSelectors=adapter->collectorSelectors(pending);QVERIFY(!originalSelectors.isEmpty());
    if(change=="optional-missing")selectedOwner=QStringLiteral("frame-11111111-1111-4111-8111-111111111111");
    if(change=="optional-unfocused"||change=="required-unfocused") {
        QVERIFY(page("document.getElementById('button').focus();true",1).toBool());
        QTRY_VERIFY(page("document.activeElement===document.getElementById('button')",1).toBool());
    }
    if(change=="required-substitution") {
        const auto original=selectedOwner;
        QVERIFY(page("(()=>{window.bindingOtherLoaded=false;const frame=document.createElement('iframe');frame.id='other-binding-frame';frame.style='position:absolute;left:530px;top:450px;width:300px;height:180px';frame.addEventListener('load',()=>window.bindingOtherLoaded=true,{once:true});frame.src="+quoted(foreignUrl)+";document.body.append(frame);return true})()",1).toBool());
        QTRY_VERIFY(page("window.bindingOtherLoaded===true",1).toBool());
        QVERIFY(page("document.getElementById('other-binding-frame').focus();true",1).toBool());
        discover();QTRY_VERIFY(scriptDone);QVERIFY(!scriptResult.toMap().contains("error"));
        selectedOwner=scriptResult.toMap().value("owner").toString();
        QVERIFY(!selectedOwner.isEmpty()&&selectedOwner!=original);
    }
    QPointer<QObject> observer;
    const auto clearObserver=qScopeGuard([&observer]{delete observer.data();});
    if(timeout) {
        const auto action=change=="top-changed-during-timeout"
            ? QStringLiteral("testWindow.activeWebView().runJavaScript(\"window.__ferric_hintFrameDocument='replaced';window.__ferric_hintFrameDocument==='replaced'\",1,function(value){nativeObserver.replaced=value===true});") : QString();
        const auto source=QStringLiteral("import QtQuick; Connections {id:nativeObserver; property bool sawProbe:false; property bool replaced:false; target:testWindow.activeWebView(); "
            "function onJavaScriptConsoleMessage(level,message,line,source){if(message==='collection-receiver-timeout'&&!sawProbe){sawProbe=true;%1}}}").arg(action);
        observer=evaluate("Qt.createQmlObject("+quoted(source)+",testWindow)").value<QObject *>();QVERIFY(observer);
        scriptDone=false;
        broker->run(selectedOwner,"(()=>{const origin=String(window.origin);Object.defineProperty(window,'origin',{configurable:true,get:function(){console.info('collection-receiver-timeout');const until=Date.now()+1300;while(Date.now()<until){}return origin}});return true})()",completion);
        QTRY_VERIFY(scriptDone);QCOMPARE(scriptResult.toBool(),true);
    }
    const auto topOwner=provider->property("rootOwner").toString();QVERIFY(!topOwner.isEmpty());
    if(change=="provider-epoch") {
        QVERIFY(provider->setProperty("epoch",provider->property("epoch").toInt()+1));
    } else if(change=="collection-epoch") {
        QVERIFY(workspace->setProperty("hintCollectionEpoch",workspace->property("hintCollectionEpoch").toInt()+1));
    } else if(change=="publication-before-bind") {
        const auto before=text("setting_action_id");
        QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('hints.marker_scale','1.1',true)").toBool());
        QTRY_VERIFY(text("setting_action_id")!=before&&text("setting_action_state")=="succeeded");
    }
    QElapsedTimer elapsed;elapsed.start();scriptDone=false;
    adapter->bindCollection(pending,topOwner,selectedOwner,completion);
    QTRY_VERIFY(scriptDone);
    const auto bound=scriptResult;
    // Entered same-origin succeeds only with the exact Document restriction;
    // the assertions below preserve Top's namespace rather than substituting it.
    const bool expected=change!="required-unfocused"&&change!="required-substitution"
        &&change!="top-changed-during-timeout"&&change!="publication-before-bind"
        &&change!="provider-epoch"&&change!="collection-epoch";
    if(timeout) {
        QVERIFY(observer->property("sawProbe").toBool());QVERIFY(elapsed.elapsed()>=900);
        if(change=="top-changed-during-timeout")QTRY_VERIFY(observer->property("replaced").toBool());
        QTest::qWait(400);
    }
    QCOMPARE(bound.isValid(),expected);
    QVERIFY(!adapter->observationObjectsChecked(pending));
    if(!expected) {QVERIFY(adapter->collectorPlan(pending).isEmpty());return;}
    QCOMPARE(adapter->collectionState(bound),mode);
    QCOMPARE(adapter->collectorSelectors(bound),originalSelectors);
    if(required) {
        if(change=="required-same-origin") {
            const auto context=QJsonDocument::fromJson(adapter->collectorPlan(bound).toUtf8()).object().value("context").toObject();
            QCOMPARE(context.value("owner").toString(),QString());
            QCOMPARE(context.value("entered_document").toObject().value("depth").toInt(),1);
            QVERIFY(adapter->collectorPlanForOwner(bound,selectedOwner).isEmpty());
        } else {
            QVERIFY(adapter->collectorPlan(bound).isEmpty());
            const auto foreignPlan=QJsonDocument::fromJson(adapter->collectorPlanForOwner(bound,selectedOwner).toUtf8()).object();
            QCOMPARE(foreignPlan.value("context").toObject().value("owner").toString(),frozenOwner);
        }
    } else if(ordinary) {
        QVERIFY(adapter->collectorPlan(bound).isEmpty());
        scriptDone=false;adapter->validate(bound,completion);QTRY_VERIFY(scriptDone);QCOMPARE(scriptResult.toBool(),true);
    } else {
        const auto plan=adapter->collectorPlan(bound);QVERIFY(!plan.isEmpty());
        const auto context=QJsonDocument::fromJson(plan.toUtf8()).object().value("context").toObject();
        QCOMPARE(context.contains("focused_document"),change=="optional-focused");
        const auto script=evaluate("testWindow.hintWorkspace.hintCollectorScript('all',JSON.parse("+quoted(originalSelectors)+"),'',false,JSON.parse("+quoted(plan)+"))").toString();
        const auto rows=QJsonDocument::fromJson(page("JSON.stringify("+script+")",1).toString().toUtf8()).object();
        QVERIFY(!rows.isEmpty()&&!rows.contains("error"));
        const auto candidates=QString::fromUtf8(QJsonDocument(rows.value("candidates").toArray()).toJson(QJsonDocument::Compact));
        scriptDone=false;
        adapter->verifyObservationObjects(bound,candidates,rows.value("annotation_json").toString(),rows.value("annotation_batch").toString(),
            QString::number(static_cast<qulonglong>(rows.value("revision").toDouble())),completion);
        QTRY_VERIFY(scriptDone);
        const bool sealed=scriptResult.toBool();
        if(!sealed)qWarning()<<"Binding object seal failed"<<change
            <<"original batch"<<rows.value("annotation_batch")
            <<"live"<<page("(()=>{const s=window.__ferric_browserHintState;return JSON.stringify({revision:s&&s.revision,dirty:s&&s.dirty,batch:s&&s.interactionProof&&s.interactionProof.batch})})()",1)
            <<"collection epoch"<<workspace->property("hintCollectionEpoch")<<"provider epoch"<<provider->property("epoch");
        QCOMPARE(sealed,true);QVERIFY(adapter->observationObjectsChecked(bound));
    }
    scriptDone=false;adapter->bindCollection(pending,topOwner,selectedOwner,completion);
    QTRY_VERIFY(scriptDone);QVERIFY(!scriptResult.isValid());
    QCOMPARE(page("window.clicks",1).toInt(),0);
}

void ApplicationTests::hintRequestNativeOwnership_data()
{
    QTest::addColumn<QString>("change");
    for (const auto *name : {"stable", "stop-restart", "same-url-reload", "policy-publication",
                             "lifetime-reset", "broker-clear", "destroy-during-probe",
                             "prebind-stop", "prebind-publication", "wrong-native-frame", "provisional-document"})
        QTest::newRow(name) << QString::fromLatin1(name);
}

void ApplicationTests::hintRequestNativeOwnership()
{
    QFETCH(QString, change);
    QVERIFY(command("hint all"));
    QTRY_COMPARE(text("mode"), QStringLiteral("hint"));
    QTRY_VERIFY(evaluate("!testWindow.hintWorkspace.hintRefreshPending && testWindow.browserUi.hint_visible").toBool());
    auto *workspace=evaluate("testWindow.hintWorkspace").value<QObject *>();QVERIFY(workspace);
    QVERIFY(suspendHintRefreshForAdapterFixture(workspace));
    auto *view = qobject_cast<QQuickItem *>(evaluate("testWindow.activeWebView()").value<QObject *>());
    auto *provider = evaluate("testWindow.hintWorkspace.hintFrameProvider").value<QObject *>();
    auto *lifetime = qobject_cast<FerricProfileLifetime *>(evaluate(
        "testWindow.profileResourceRegistry.forProfile(testWindow.activeWebView().profile).lifetime").value<QObject *>());
    QVERIFY(view); QVERIFY(provider); QVERIFY(lifetime);
    const auto brokers = provider->findChildren<FerricHintFrameBroker *>();
    QCOMPARE(brokers.size(), 1);
    auto *broker = brokers.first();
    const auto owner = provider->property("rootOwner").toString();
    QVERIFY(!owner.isEmpty());
    const auto createAdapter = [this] {
        return qobject_cast<FerricHintRequestAdapter *>(evaluate("Qt.createQmlObject(" + quoted(
            "import QtQuick; import io.github.ferricbrowser 1.0; FerricHintRequestAdapter {}") + ",testWindow)").value<QObject *>());
    };
    QPointer<FerricHintRequestAdapter> adapter = createAdapter();
    QVERIFY(adapter);
    QVERIFY2(qjsEngine(adapter), "Native fixture adapter must have a script engine");
    adapter->setLifetime(lifetime);
    QQmlEngine::setObjectOwnership(this, QQmlEngine::CppOwnership);
    engine->globalObject().setProperty("__ferricHintRequestTest", engine->newQObject(this));
    QCOMPARE(QQmlEngine::objectOwnership(this), QQmlEngine::CppOwnership);
    const auto completion = engine->evaluate("(function(value){__ferricHintRequestTest.accept(value)})");
    QVERIFY(completion.isCallable());
    scriptDone = false;
    if (change == "destroy-during-probe") {
        // Observe actual isolated receipt execution, then synchronously destroy
        // its adapter before the pending native completion may deliver.
        page("(()=>{const origin=String(window.origin);Object.defineProperty(window,'origin',"
             "{configurable:true,get:function(){console.info('hint-request-native-destruction');return origin;}});return true})()", 1);
        hintAdapterDestructionTarget = adapter;
        hintAdapterSawProbe = false;
        hintAdapterCompletedBeforeProbe = false;
        const auto observerSource = QString::fromLatin1(R"QML(import QtQuick; Connections {
            target:testWindow.activeWebView();
            function onJavaScriptConsoleMessage(level,message,line,source) {
                if(message==="hint-request-native-destruction") testProbe.destroyHintRequestAdapter();
            }
        })QML");
        const auto observer = evaluate("Qt.createQmlObject(" + quoted(observerSource) + ",testWindow)").value<QObject *>();
        QVERIFY(observer);
        scriptDone = false;
        adapter->capture(view, broker, owner, completion);
        QTRY_VERIFY(adapter.isNull());
        QVERIFY(hintAdapterSawProbe);
        QVERIFY(!hintAdapterCompletedBeforeProbe);
        QTest::qWait(1100);
        QVERIFY(!scriptDone);
        return;
    }
    if (change == "wrong-native-frame") {
        // Same URL is not the view→native-frame association. An unrelated real
        // WebEngine frame must fail even after its own broker captures it.
        auto *otherView = qobject_cast<QQuickItem *>(evaluate("Qt.createQmlObject(" + quoted(
            "import QtQuick; import QtWebEngine; WebEngineView {width:320;height:240}") + ",testWindow)").value<QObject *>());
        QVERIFY(otherView);
        QVERIFY(otherView->setProperty("profile", view->property("profile")));
        const auto url = view->property("url").toUrl();
        QVERIFY(otherView->setProperty("url", url));
        QTRY_COMPARE(otherView->property("mainFrame").value<QWebEngineFrame>().url(), url);
        QTRY_VERIFY(!otherView->property("loading").toBool());
        const QPointer<FerricHintFrameBroker> otherBroker(qobject_cast<FerricHintFrameBroker *>(evaluate("Qt.createQmlObject(" + quoted(
            "import QtQuick; import io.github.ferricbrowser 1.0; FerricHintFrameBroker {}") + ",testWindow)").value<QObject *>()));
        QVERIFY(otherBroker); QVERIFY(qjsEngine(otherBroker));
        QVERIFY(!adapter->begin(otherView, otherBroker.data()).isValid());
        const auto pending = adapter->begin(view, otherBroker.data());
        QVERIFY(pending.isValid());
        scriptDone = false;
        otherBroker->attach(otherView->property("mainFrame").value<QWebEngineFrame>(), completion);
        QTRY_VERIFY(scriptDone);
        const auto otherOwner = scriptResult.toMap().value("owner").toString();
        QVERIFY(!otherOwner.isEmpty());
        scriptDone = false;
        adapter->bindDocuments(pending, otherOwner, completion);
        QTRY_VERIFY(scriptDone);
        QVERIFY(adapter->collectorPlan(scriptResult).isEmpty());
        scriptDone = false; adapter->validate(pending, completion);
        QTRY_VERIFY(scriptDone); QCOMPARE(scriptResult.toBool(), false);
        delete adapter.data(); delete otherView;
        return;
    }
    if (change == "provisional-document") {
        const auto oldUrl = view->property("url").toUrl();
        auto nextUrl = oldUrl;
        nextUrl.setPath("/native-request-held.html");
        const QByteArray path("/native-request-held.html");
        fixtureHeldPaths.insert(path);
        frameFixtures.insert(path, fixtureHtml + "<script>document.documentElement.dataset.nativeRequestNew='B'</script>");
        page("document.documentElement.dataset.nativeRequestOld='A';true",1);
        const auto oldSelectors = evaluate("testWindow.browserUi.hint_selector_policy('all')").toString();
        QVERIFY(view->setProperty("url", nextUrl));
        QTRY_VERIFY(!fixtureHeldResponses.value(path).isEmpty());
        QTRY_COMPARE(text("current_url"), nextUrl.toString());
        QTRY_COMPARE(text("load_state"), QStringLiteral("provisional"));
        const auto document = page("({url:document.URL,old:document.documentElement.dataset.nativeRequestOld})",1).toMap();
        QCOMPARE(document.value("url").toString(), oldUrl.toString());
        QCOMPARE(document.value("old").toString(), QStringLiteral("A"));
        const auto nativeUrl = view->property("mainFrame").value<QWebEngineFrame>().url();
        QCOMPARE(nativeUrl,oldUrl);
        qInfo() << "P05 held response facts:" << "view" << view->property("url") << "Rust" << text("current_url")
                << "native frame" << nativeUrl << "document" << document;
        QVERIFY(command("hint all"));
        QTRY_COMPARE(text("mode"), QStringLiteral("hint"));
        QTRY_VERIFY(evaluate("(function(){const s=testWindow.browserUi.hint_selector_policy('all');if(!s)return false;try{return !JSON.parse(s).error}catch(e){return false}})()").toBool());
        const auto nextSelectors = evaluate("testWindow.browserUi.hint_selector_policy('all')").toString();
        QVERIFY(nextSelectors != oldSelectors);
        QTRY_VERIFY(!provider->property("rootOwner").toString().isEmpty());
        const auto pending = adapter->begin(view, broker);
        QVERIFY(pending.isValid());
        QCOMPARE(adapter->collectorSelectors(pending),nextSelectors);
        scriptDone = false;
        adapter->bindDocuments(pending, provider->property("rootOwner").toString(), completion);
        QTRY_VERIFY(scriptDone);
        QVERIFY2(adapter->collectorPlan(scriptResult).isEmpty(), "A provisional policy URL cannot label the old displayed document");
        fixtureHeldPaths.remove(path);
        const auto responses = fixtureHeldResponses.take(path);
        QVERIFY(!responses.isEmpty());
        for (const auto &response : responses) response();
        QTRY_VERIFY(!view->property("loading").toBool());
        QTRY_VERIFY(page("document.documentElement.dataset.nativeRequestNew==='B'",1).toBool());
        QTRY_COMPARE(text("load_state"), QStringLiteral("complete"));
        evaluate("testWindow.browserUi.escape()");
        QTRY_COMPARE(text("mode"),QStringLiteral("normal"));
        QVERIFY(command("hint all"));
        QTRY_COMPARE(text("mode"), QStringLiteral("hint"));
        QTRY_VERIFY(evaluate("!testWindow.hintWorkspace.hintRefreshPending && testWindow.browserUi.hint_visible").toBool());
        QVERIFY(suspendHintRefreshForAdapterFixture(workspace));
        const auto fresh = adapter->begin(view, broker);
        QVERIFY(fresh.isValid());
        scriptDone = false;
        adapter->bindDocuments(fresh, provider->property("rootOwner").toString(), completion);
        QTRY_VERIFY(scriptDone);
        const auto freshHandle = scriptResult;
        QVERIFY(!adapter->collectorPlan(freshHandle).isEmpty());
        scriptDone = false;
        adapter->validate(freshHandle, completion);
        QTRY_VERIFY(scriptDone); QCOMPARE(scriptResult.toBool(),true);
        delete adapter.data();
        return;
    }
    const auto pending = adapter->begin(view, broker);
    QVERIFY(pending.isValid());
    QVERIFY(adapter->collectorPlan(pending).isEmpty());
    const auto originalSelectors = adapter->collectorSelectors(pending);
    QVERIFY(!originalSelectors.isEmpty());
    QCOMPARE(QJsonDocument::fromJson(originalSelectors.toUtf8()).object().value("family").toString(), QStringLiteral("all"));
    if (change.startsWith("prebind-")) {
        if (change == "prebind-stop") {
            evaluate("testWindow.browserUi.escape()");
            QVERIFY(command("hint all"));
        } else {
            const auto before = text("setting_action_id");
            QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('hints.marker_scale','1.1',true)").toBool());
            QTRY_VERIFY(text("setting_action_id") != before && text("setting_action_state") == "succeeded");
            QTRY_COMPARE(QJsonDocument::fromJson(text("config_json").toUtf8()).object().value("hints").toObject().value("marker_scale").toDouble(), 1.1);
        }
        scriptDone = false;
        adapter->bindDocuments(pending, owner, completion);
        QTRY_VERIFY(scriptDone);
        QVERIFY(adapter->collectorPlan(scriptResult).isEmpty());
        scriptDone = false; adapter->validate(pending, completion);
        QTRY_VERIFY(scriptDone); QCOMPARE(scriptResult.toBool(), false);
        QVERIFY(adapter->collectorSelectors(pending).isEmpty());
        delete adapter.data();
        return;
    }
    adapter->bindDocuments(pending, owner, completion);
    QTRY_VERIFY(scriptDone);
    const auto handle = scriptResult;
    QCOMPARE(adapter->collectorSelectors(handle), originalSelectors);
    const auto plan = adapter->collectorPlan(handle);
    QVERIFY2(!plan.isEmpty(), qPrintable(text("status_text")));
    const auto parsed = QJsonDocument::fromJson(plan.toUtf8());
    QVERIFY(parsed.isObject());
    QCOMPARE(parsed.object().value("stamp").toObject().value("family").toString(), QStringLiteral("all"));
    // A renderer/QML map cannot mint this metatype; another adapter cannot use
    // the original object's opaque handle, even with the same lifetime.
    scriptDone = false;
    adapter->validate(QVariantMap{{"request_id", 1}}, completion);
    QVERIFY(scriptDone); QCOMPARE(scriptResult.toBool(), false);
    QVERIFY(adapter->collectorSelectors(QVariantMap{{"request_id", 1}}).isEmpty());
    const QPointer<FerricHintRequestAdapter> other(createAdapter());
    QVERIFY(other); QVERIFY(qjsEngine(other));
    other->setLifetime(lifetime);
    scriptDone = false; other->validate(handle, completion);
    QVERIFY(scriptDone); QCOMPARE(scriptResult.toBool(), false);
    QVERIFY(other->collectorSelectors(handle).isEmpty());
    if (change == "stop-restart") {
        evaluate("testWindow.browserUi.escape()");
        QVERIFY(command("hint all"));
    } else if (change == "same-url-reload") {
        page("window.__hintRequestReloadMarker=1;true");
        evaluate("testWindow.activeWebView().reload()");
        QTRY_VERIFY(page("typeof window.__hintRequestReloadMarker==='undefined'").toBool());
        QTRY_COMPARE(text("load_state"), QStringLiteral("complete"));
    } else if (change == "policy-publication") {
        const auto before = text("setting_action_id");
        QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('hints.marker_scale','1.1',true)").toBool());
        QTRY_VERIFY(text("setting_action_id") != before && text("setting_action_state") == "succeeded");
        QTRY_COMPARE(QJsonDocument::fromJson(text("config_json").toUtf8()).object().value("hints").toObject().value("marker_scale").toDouble(), 1.1);
    } else if (change == "lifetime-reset") {
        adapter->setLifetime(nullptr); adapter->setLifetime(lifetime);
    } else if (change == "broker-clear") {
        broker->clear();
    }
    scriptDone = false;
    adapter->validate(handle, completion);
    QTRY_VERIFY(scriptDone);
    QCOMPARE(scriptResult.toBool(), change == "stable");
    if (change == "stable") {
        QCOMPARE(adapter->collectorPlan(handle), plan);
        QCOMPARE(adapter->collectorSelectors(handle), originalSelectors);
        const auto outstanding=broker->pendingScripts();
        scriptDone=false;
        adapter->validate(handle,completion);
        QVERIFY2(scriptDone,"A repeated original-request validation must refuse before native dispatch");
        QCOMPARE(scriptResult.toBool(),false);
        QCOMPARE(broker->pendingScripts(),outstanding);
    } else {
        QVERIFY(adapter->collectorPlan(handle).isEmpty());
        QVERIFY(adapter->collectorSelectors(handle).isEmpty());
    }
    delete adapter.data();
}

void ApplicationTests::hintRequestObservationDocuments_data()
{
    QTest::addColumn<QString>("change");
    for (const auto *name : {"stable", "focused-top-plan", "selected-leaf-epoch", "wrong-focused-document", "missing-focused-name", "unavailable-focused-root", "bind-ancestor", "bind-focus-loss", "role-foreign", "role-return-origin", "one-shot", "bad-handle", "other-adapter", "bad-batch", "bad-stamp",
                             "excluded-child-epoch", "stop-before", "publication-before", "stop-during", "destroy-during",
                             "objects-stable", "objects-focused-top-plan", "objects-one-shot", "objects-bad-batch", "objects-bad-stamp",
                             "objects-excluded-child-epoch", "objects-changed-fact", "objects-stop-before", "objects-publication-before",
                             "objects-stop-during", "objects-destroy-during", "objects-scope-stable", "objects-scope-metadata", "objects-scope-stop",
                             "objects-competing-collection", "objects-raw-seal-staging", "objects-strict-nonmatch-staging",
                             "work-top", "work-foreign", "work-replaced", "work-over-budget", "work-policy-foreign", "work-policy-top-survives-focus-loss", "work-policy-forged-plan", "work-policy-cold-child", "work-policy-underreported-debit"})
        QTest::newRow(name) << QString::fromLatin1(name);
}

void ApplicationTests::hintRequestObservationDocuments()
{
    QFETCH(QString, change);
    const bool objects = change.startsWith("objects-");
    if (objects) change.remove(0, 8);
    const bool work = change.startsWith("work-");
    const bool policyWork = change.startsWith("work-policy-");
    const bool policyFocusLoss = change == "work-policy-top-survives-focus-loss";
    const bool forgedPolicyPlan = change == "work-policy-forged-plan";
    const bool coldChild = change == "work-policy-cold-child";
    const bool underreportedDebit = change == "work-policy-underreported-debit";
    const bool rejectPolicy = forgedPolicyPlan || underreportedDebit;
    const bool foreignRole = change == "role-foreign" || change == "role-return-origin" || change == "work-foreign" || policyWork;
    const bool needsFocusedFrame = change == "focused-top-plan" || change == "selected-leaf-epoch"
        || change == "wrong-focused-document" || change == "missing-focused-name" || change == "unavailable-focused-root"
        || change == "bind-ancestor" || change == "bind-focus-loss" || foreignRole;
    const bool scoped = change.startsWith("scope-");
    if(forgedPolicyPlan) {
        QVERIFY(page("(()=>{const button=document.createElement('button');button.id='policy-alternate';button.textContent='Alternate policy target';button.style.cssText='position:absolute;left:310px;top:300px;width:180px;height:32px';document.body.append(button);return true})()",1).toBool());
    }
    if (scoped) {
        QVERIFY(page("(()=>{const permit=document.querySelector('[aria-label=\"Scoped panel\"]'),button=document.getElementById('button'),frame=document.getElementById('frame');permit.style.cssText='position:absolute;left:20px;top:20px;width:800px;height:600px;overflow:visible';frame.style.cssText='position:absolute;left:400px;top:80px;width:300px;height:180px';permit.prepend(button,frame);return true})()",1).toBool());
        QTRY_VERIFY(page("!!document.getElementById('frame').contentDocument.querySelector('button')",1).toBool());
        QVERIFY(command("hint --target choose regions"));
        const auto region = hintLabel("Scoped panel");
        QVERIFY(!region.isEmpty());
        typeText(region);
        QTRY_VERIFY(evaluate("testWindow.contextMenuItems.length > 0").toBool());
        QVERIFY(chooseCapability("hint-within"));
        QTRY_VERIFY(!evaluate("testWindow.hintWorkspace.activeHintScopeLeaseId").toString().isEmpty());
    } else QVERIFY(command("hint all"));
    QTRY_COMPARE(text("mode"), QStringLiteral("hint"));
    QTRY_VERIFY(evaluate("!testWindow.hintWorkspace.hintRefreshPending && testWindow.browserUi.hint_visible").toBool());
    auto *view = qobject_cast<QQuickItem *>(evaluate("testWindow.activeWebView()").value<QObject *>());
    auto *provider = evaluate("testWindow.hintWorkspace.hintFrameProvider").value<QObject *>();
    auto *lifetime = qobject_cast<FerricProfileLifetime *>(evaluate(
        "testWindow.profileResourceRegistry.forProfile(testWindow.activeWebView().profile).lifetime").value<QObject *>());
    QVERIFY(view); QVERIFY(provider); QVERIFY(lifetime);
    const auto brokers = provider->findChildren<FerricHintFrameBroker *>();
    QCOMPARE(brokers.size(), 1);
    auto *broker = brokers.first();
    auto *workspace=evaluate("testWindow.hintWorkspace").value<QObject *>();QVERIFY(workspace);
    QVERIFY(suspendHintRefreshForAdapterFixture(workspace));
    QTRY_VERIFY(!provider->property("pollPending").toBool()&&!workspace->property("hintRefreshPending").toBool());
    const auto createAdapter = [this] {
        return qobject_cast<FerricHintRequestAdapter *>(evaluate("Qt.createQmlObject(" + quoted(
            "import QtQuick; import io.github.ferricbrowser 1.0; FerricHintRequestAdapter {}") + ",testWindow)").value<QObject *>());
    };
    QPointer<FerricHintRequestAdapter> adapter = createAdapter();
    QVERIFY(adapter); QVERIFY(qjsEngine(adapter));
    adapter->setLifetime(lifetime);
    QQmlEngine::setObjectOwnership(this, QQmlEngine::CppOwnership);
    engine->globalObject().setProperty("__ferricHintRequestTest", engine->newQObject(this));
    const auto completion = engine->evaluate("(function(value){__ferricHintRequestTest.accept(value)})");
    QVERIFY(completion.isCallable());
    const auto verifyObservation = [&](FerricHintRequestAdapter *receiver, const QVariant &request,
                                       const QString &rows, const QString &wire, const QString &batch,
                                       const QString &revision) {
        if (objects) receiver->verifyObservationObjects(request,rows,wire,batch,revision,completion);
        else receiver->verifyObservationDocuments(request,rows,wire,batch,revision,completion);
    };
    auto pending = work ? adapter->beginCollection(view, broker).value("request") : adapter->begin(view, broker);
    QVERIFY(pending.isValid());
    auto selectedOwner = provider->property("rootOwner").toString();
    const auto awaitInitialChildObservation=[&](quint64 revision, bool invalidated=false, const QString &nonce={}) {
        QElapsedTimer timer;timer.start();
        while(timer.elapsed()<1000) {
            scriptDone=false;
            // Initial equal-size reports have deliberately been non-dirty
            // since 066cf8b. Observe delivery itself for the stable positives;
            // require actual dirtiness and the same retained nonce for the
            // changed-child negative (9d39e43 retains weak pending facts).
            broker->run(selectedOwner,"(()=>{const state=window.__ferric_browserHintState,target=document.getElementById('foreign-primary');"
                "return !!state&&state.tracking===true&&!!target&&state.resizeInitialRects instanceof WeakMap"
                "&&state.resizeInitialRects.has(target)&&state.resizeInitialRects.get(target)===null&&state.revision>="
                +QString::number(revision)+(invalidated?"&&state.dirty===true&&state.revision>"+QString::number(revision)
                    +"&&!!state.pendingInteraction&&state.pendingInteraction.nonce==="+quoted(nonce):"")+"})()",completion);
            while(!scriptDone&&timer.elapsed()<1000)QTest::qWait(1);
            if(!scriptDone||scriptResult.metaType().id()!=QMetaType::Bool)return false;
            if(scriptResult.toBool())return true;
            QTest::qWait(10);
        }
        return false;
    };
    if (needsFocusedFrame) {
        view->forceActiveFocus();
        if (foreignRole) {
            const auto returned = QStringLiteral("http://127.0.0.1:%1/role-return.html").arg(server.serverPort());
            frameFixtures["/role-return.html"] = "<!doctype html><button autofocus>Returned-origin receiver</button>";
            frameFixtures["/role-child.html"] = change == "role-return-origin"
                ? ("<!doctype html><iframe id='return' src='" + returned + "'></iframe>").toUtf8()
                : QByteArray("<!doctype html><button id='foreign-primary' autofocus>Foreign receiver</button>");
            const auto foreign = QStringLiteral("http://localhost:%1/role-child.html").arg(server.serverPort());
            page("(()=>{const frame=document.getElementById('frame');window.__roleFixtureLoaded=false;frame.addEventListener('load',()=>window.__roleFixtureLoaded=true,{once:true});frame.src=" + quoted(foreign) + ";return true})()", 1);
            QTRY_VERIFY(page("window.__roleFixtureLoaded===true", 1).toBool());
            QTRY_VERIFY(page("document.getElementById('frame').contentDocument===null", 1).toBool());
            page("document.getElementById('frame').focus();true", 1);
        } else {
            page("document.getElementById('frame').contentDocument.querySelector('button').focus();true",1);
            QTRY_VERIFY(page("document.getElementById('frame').contentDocument.hasFocus()",1).toBool());
        }
        scriptDone = false;
        evaluate("testWindow.hintWorkspace.hintFrameProvider.discover(testWindow.hintWorkspace.hintFrameProvider.rootOwner,0,false,false,"
                 "function(value){__ferricHintRequestTest.accept(value)})");
        QTRY_VERIFY(scriptDone);
        QVERIFY(!scriptResult.toMap().contains("error"));
        selectedOwner = scriptResult.toMap().value("owner").toString();
        QVERIFY(selectedOwner != provider->property("rootOwner").toString());
        if (change == "role-return-origin") {
            const auto parent = selectedOwner;
            scriptDone = false;
            broker->run(parent, "(()=>{const frame=document.getElementById('return');if(!frame||!frame.contentWindow)return false;frame.focus();return true})()", completion);
            QTRY_VERIFY(scriptDone);
            QCOMPARE(scriptResult.metaType().id(), int(QMetaType::Bool));
            // If autofocus already chose the deeper receiver, discovery has
            // already traversed both worlds; otherwise focus it explicitly.
            if (scriptResult.toBool()) {
                scriptDone = false;
                evaluate("testWindow.hintWorkspace.hintFrameProvider.discover(testWindow.hintWorkspace.hintFrameProvider.rootOwner,0,false,false,"
                         "function(value){__ferricHintRequestTest.accept(value)})");
                QTRY_VERIFY(scriptDone);
                QVERIFY(!scriptResult.toMap().contains("error"));
                selectedOwner = scriptResult.toMap().value("owner").toString();
                QVERIFY(selectedOwner != parent);
            }
        }
    }
    if(policyWork&&!coldChild) {
        // Establish the child's initial ResizeObserver delivery before the
        // definitive request. Changed-child invalidation is a separate negative;
        // these component positives require a stable original inventory.
        const auto warmSelectors=adapter->collectorSelectors(pending);
        QVERIFY(!warmSelectors.isEmpty());
        const auto script=evaluate("testWindow.hintWorkspace.hintCollectorScript('all',JSON.parse("+quoted(warmSelectors)+"),'',false)").toString();
        scriptDone=false;
        broker->run(selectedOwner,"JSON.stringify("+script+")",completion);
        QTRY_VERIFY(scriptDone);
        const auto warm=QJsonDocument::fromJson(scriptResult.toString().toUtf8()).object();
        QVERIFY(!warm.contains("error")&&!warm.value("candidates").toArray().isEmpty());
        QVERIFY(awaitInitialChildObservation(warm.value("revision").toInteger()));
        pending=adapter->beginCollection(view,broker).value("request");
        QVERIFY(pending.isValid());
    }
    if (change == "bind-focus-loss") {
        page("document.getElementById('button').focus();true",1);
        QTRY_VERIFY(!page("document.getElementById('frame').contentDocument.hasFocus()",1).toBool());
    } else if (change == "bind-ancestor") {
        QVERIFY(page("(()=>{const doc=document.getElementById('frame').contentDocument,frame=doc.createElement('iframe');frame.id='deeper';frame.srcdoc='<button>Nested receiver</button>';doc.body.append(frame);return true})()",1).toBool());
        QTRY_VERIFY(page("(()=>{const doc=document.getElementById('frame').contentDocument.getElementById('deeper').contentDocument;return !!doc&&!!doc.querySelector('button')})()",1).toBool());
        page("document.getElementById('frame').contentDocument.getElementById('deeper').contentDocument.querySelector('button').focus();true",1);
        QTRY_VERIFY(page("document.getElementById('frame').contentDocument.getElementById('deeper').contentDocument.hasFocus()",1).toBool());
    }
    scriptDone = false;
    adapter->bindDocuments(pending, selectedOwner, completion);
    QTRY_VERIFY(scriptDone);
    const auto handle = scriptResult;
    if (change == "bind-ancestor" || change == "bind-focus-loss") {
        QVERIFY(!handle.isValid());
        QVERIFY(adapter->collectorPlan(handle).isEmpty());
        delete adapter.data();
        return;
    }
    const auto plan = adapter->collectorPlan(handle);
    const auto selectors = adapter->collectorSelectors(handle);
    QVERIFY(!plan.isEmpty()); QVERIFY(!selectors.isEmpty());
    const auto context = QJsonDocument::fromJson(plan.toUtf8()).object().value("context").toObject();
    QCOMPARE(context.value("start").toString(),QStringLiteral("top-document"));
    QVERIFY(adapter->collectorPlanForOwner(handle, provider->property("rootOwner").toString()).isEmpty());
    QVERIFY(adapter->collectorPlanForOwner(handle, QStringLiteral("frame-11111111-1111-4111-8111-111111111111")).isEmpty());
    if(work) {
        const auto originalValues=text("hint_values");
        // Alter only the compiled target selector in JS: parsing/re-encoding
        // through QJsonObject would also reorder the original stamp fields.
        const auto producerPlan=[this,adapter,&handle,forgedPolicyPlan](const QString &owner) {
            const auto original=adapter->collectorPlanForOwner(handle,owner);
            if(!forgedPolicyPlan||!owner.isEmpty())return original;
            return evaluate("(()=>{const plan=JSON.parse("+quoted(original)
                +");const rule=plan.rules.find(query=>query.rule.id==='primary');if(!rule)throw Error('missing primary rule');rule.target_selector.steps[0][1]='#policy-alternate';return JSON.stringify(plan)})()").toString();
        };
        QVERIFY(!adapter->beginCollectionWork(QVariant(QVariantMap{{"request_id",1}})));
        QVERIFY(adapter->beginCollectionWork(handle));
        QVERIFY(!adapter->beginCollectionWork(handle));
        const auto grant=QJsonDocument::fromJson(adapter->collectionWorkPlan(handle).toUtf8()).object();
        QCOMPARE(grant.value("world").toString(),QStringLiteral("top"));
        QCOMPARE(grant.value("phase").toString(),QStringLiteral("ordinary"));
        const auto makeRows=[this,&selectors,&producerPlan](const QJsonObject &grant) {
            const auto owner=grant.value("owner").toString();
            const auto plan=producerPlan(owner);
            const auto budget=QString::fromUtf8(QJsonDocument(grant.value("budget").toObject()).toJson(QJsonDocument::Compact));
            return evaluate("testWindow.hintWorkspace.hintCollectorScript('all',JSON.parse("+quoted(selectors)
                +"),'',false,JSON.parse("+quoted(plan)+"),true,JSON.parse("+quoted(budget)+"))").toString();
        };
        const auto localRows=[this](const QString &script) {
            return QJsonDocument::fromJson(page("JSON.stringify("+script+")",1).toString().toUtf8()).object();
        };
        const auto accept=[adapter,&handle](const QJsonObject &grant,const QJsonObject &rows,bool overspend) {
            const auto work=rows.value("interaction_work").toObject();
            const auto metadata=QJsonObject{{"nonce",rows.value("interaction_pending")},{"revision",rows.value("revision")},
                {"selector_revision",rows.value("selector_revision")},{"inspections",overspend?QJsonValue(50001):work.value("inspections")},
                {"predicate_steps",work.value("predicate_steps")},{"truncated",rows.value("truncated")},{"viewport",rows.value("viewport")}};
            return adapter->acceptCollectionOrdinary(handle,grant.value("stage").toString(),grant.value("owner").toString(),
                QString::fromUtf8(QJsonDocument(rows.value("candidates").toArray()).toJson(QJsonDocument::Compact)),
                QString::fromUtf8(QJsonDocument(metadata).toJson(QJsonDocument::Compact)));
        };
        auto rows=localRows(makeRows(grant));
        QVERIFY(!rows.isEmpty()&&!rows.contains("error"));
        QVERIFY(!rows.value("candidates").toArray().isEmpty());
        QVERIFY(rows.value("interaction_pending").isString());
        QVERIFY(!adapter->rejectCollectionWork(handle,QStringLiteral("99"),{}));
        if(change=="work-replaced") {
            const auto replacement=adapter->beginCollection(view,broker).value("request");
            QVERIFY(replacement.isValid());
            QVERIFY(!accept(grant,rows,false));
            QVERIFY(adapter->collectionWorkPlan(handle).isEmpty());
        } else if(change=="work-over-budget") {
            QVERIFY(!accept(grant,rows,true));
            QVERIFY(adapter->collectionWorkPlan(handle).isEmpty());
        } else {
            QVERIFY(accept(grant,rows,false));
            const auto next=adapter->collectionWorkPlan(handle);
            QVERIFY(!accept(grant,rows,false));
            QCOMPARE(adapter->collectionWorkPlan(handle),next);
            auto following=QJsonDocument::fromJson(next.toUtf8()).object();
            QJsonObject child;
            if(foreignRole) {
                QCOMPARE(following.value("world").toString(),QStringLiteral("selected"));
                QCOMPARE(following.value("phase").toString(),QStringLiteral("ordinary"));
                QCOMPARE(following.value("owner").toString(),selectedOwner);
                scriptDone=false;
                broker->run(selectedOwner,"JSON.stringify("+makeRows(following)+")",completion);
                QTRY_VERIFY(scriptDone);
                child=QJsonDocument::fromJson(scriptResult.toString().toUtf8()).object();
                QVERIFY(!child.isEmpty()&&!child.contains("error"));
                const auto childRows=child.value("candidates").toArray();
                const auto expected=std::find_if(childRows.cbegin(),childRows.cend(),[](const auto &value){
                    const auto row=value.toObject();
                    return row.value("text").toString()==QStringLiteral("Foreign receiver")
                        &&row.value("kind").toString()==QStringLiteral("button")
                        &&row.value("frame_path").toString()==QStringLiteral("0")
                        &&row.value("frame_owner").toString().isEmpty();
                });
                QVERIFY(expected!=childRows.cend());
                QVERIFY(accept(following,child,false));
                if(coldChild) {
                    // Equal initial geometry is now a coherent snapshot. Make
                    // a real child layout change to exercise stale rejection.
                    scriptDone=false;
                    broker->run(selectedOwner,"(()=>{const target=document.getElementById('foreign-primary');target.style.width='190px';return true})()",completion);
                    QTRY_VERIFY(scriptDone);QVERIFY(scriptResult.toBool());
                    QVERIFY(awaitInitialChildObservation(child.value("revision").toInteger(),true,child.value("interaction_pending").toString()));
                }
                following=QJsonDocument::fromJson(adapter->collectionWorkPlan(handle).toUtf8()).object();
                QCOMPARE(following.value("world").toString(),QStringLiteral("top"));
            }
            QCOMPARE(following.value("phase").toString(),QStringLiteral("policy"));
            QVERIFY(!following.value("budget").toObject().contains("candidates"));
            QVERIFY(!following.value("budget").toObject().contains("depth"));
            QCOMPARE(following.value("ordinary").toObject().value("metadata").toObject().value("nonce"),rows.value("interaction_pending"));
            if(policyWork) {
                QVERIFY(rows.value("interaction_pending")!=child.value("interaction_pending"));
                const auto topRows=rows.value("candidates").toArray();
                const auto childRows=child.value("candidates").toArray();
                // These are genuine IDs assigned by the two isolated worlds.
                // Never edit the inventories to manufacture an ID collision.
                const bool overlappingIds=std::any_of(topRows.cbegin(),topRows.cend(),[&childRows](const auto &top){
                    return std::any_of(childRows.cbegin(),childRows.cend(),[&top](const auto &local){
                        return top.toObject().value("element_id")==local.toObject().value("element_id");
                    });
                });
                QVERIFY(overlappingIds);
                const auto finishScript=[this,&producerPlan](const QJsonObject &grant) {
                    const auto plan=producerPlan(grant.value("owner").toString());
                    const auto ordinary=grant.value("ordinary").toObject().value("metadata").toObject();
                    const auto budget=QString::fromUtf8(QJsonDocument(grant.value("budget").toObject()).toJson(QJsonDocument::Compact));
                    return evaluate("testWindow.hintWorkspace.hintFinishInteractionScript(JSON.parse("+quoted(plan)
                        +"),"+quoted(ordinary.value("nonce").toString())+",JSON.parse("+quoted(budget)+"))").toString();
                };
                const auto topPolicy=localRows(finishScript(following));
                QVERIFY(!topPolicy.isEmpty()&&!topPolicy.contains("error"));
                QCOMPARE(topPolicy.value("annotation_batch"),rows.value("interaction_pending"));
                const auto rawTop=QJsonDocument::fromJson(topPolicy.value("annotation_json").toString().toUtf8()).object();
                QVERIFY(!rawTop.value("roots").toArray().isEmpty());
                QCOMPARE(rawTop.value("stamp"),QJsonDocument::fromJson(plan.toUtf8()).object().value("stamp"));
                const auto hasAnchorTarget=[](const QJsonObject &raw,const QString &rule,const QJsonArray &ordinary,const QString &owner,const QString &expectedText) {
                    for(const auto &entry:raw.value("outcomes").toArray()) {
                        const auto outcome=entry.toObject();
                        if(outcome.value("rule").toObject().value("id").toString()!=rule)continue;
                        const auto targets=outcome.value("result").toObject().value("anchor-observed").toObject().value("targets").toArray();
                        if(targets.size()!=1)return false;
                        const auto candidate=targets.first().toObject().value("candidate").toObject();
                        if(!candidate.value("index").isDouble()||!candidate.value("frame_owner").isString()
                            ||!candidate.value("frame_path").isString())return false;
                        const auto index=candidate.value("index").toInt(-1);
                        if(index<0||index>=ordinary.size())return false;
                        const auto row=ordinary[index].toObject();
                        return candidate.value("frame_owner").toString()==owner
                            &&candidate.value("frame_path")==row.value("frame_path")
                            &&candidate.value("element_id")==row.value("element_id")
                            &&row.value("kind").toString()==QStringLiteral("button")
                            &&row.value("text").toString()==expectedText;
                    }
                    return false;
                };
                QVERIFY2(hasAnchorTarget(rawTop,QStringLiteral("primary"),topRows,{},forgedPolicyPlan
                    ?QStringLiteral("Alternate policy target"):QStringLiteral("Activate exact button")),
                    "Top must retain the actual producer-selected ordinary button witness");
                const auto topOrdinaryWork=following.value("ordinary").toObject().value("metadata").toObject();
                const auto topPolicyWork=topPolicy.value("interaction_work").toObject();
                const auto topRawWork=rawTop.value("discovery_work").toObject();
                for(const auto &counter:{QStringLiteral("inspections"),QStringLiteral("predicate_steps")}) {
                    QVERIFY(topRawWork.value(counter).isDouble());
                    QCOMPARE(topRawWork.value(counter).toInteger(),
                        topOrdinaryWork.value(counter).toInteger()+topPolicyWork.value(counter).toInteger());
                }
                // This new component gate intentionally fails until native
                // per-world policy staging/sealing is implemented. No repeated
                // renderer rows, caller receipt or final normalized graph is
                // supplied to the new native continuation.
                QVERIFY2(engine->newQObject(adapter).property("verifyCollectionPolicy").isCallable(),
                    "P05 requires native policy staging and sealing for each retained ordinary world");
                if(policyFocusLoss) {
                    QVERIFY(page("document.getElementById('button').focus();document.activeElement===document.getElementById('button')",1).toBool());
                    scriptDone=false;
                    broker->run(selectedOwner,"document.hasFocus()",completion);
                    QTRY_VERIFY(scriptDone);QCOMPARE(scriptResult.metaType().id(),int(QMetaType::Bool));
                    QCOMPARE(scriptResult.toBool(),false);
                }
                if(underreportedDebit)QVERIFY(topPolicy.value("interaction_work").toObject().value("annotation_bytes").toInteger()
                    >topPolicy.value("annotation_json").toString().toUtf8().size());
                const auto verifyPolicy=[adapter,&handle,&completion,underreportedDebit](const QJsonObject &grant,const QJsonObject &result) {
                    const auto work=result.value("interaction_work").toObject();
                    const auto metadata=QJsonObject{{"nonce",result.value("annotation_batch")},{"revision",result.value("revision")},
                        {"selector_revision",result.value("selector_revision")},{"inspections",work.value("inspections")},
                        {"predicate_steps",work.value("predicate_steps")},{"annotation_bytes",
                            underreportedDebit&&grant.value("world").toString()==QStringLiteral("top")
                                ?QJsonValue(static_cast<int>(result.value("annotation_json").toString().toUtf8().size())):work.value("annotation_bytes")},
                        {"truncated",result.value("truncated")},{"viewport",result.value("viewport")}};
                    const auto stage=grant.value("stage").toString(),owner=grant.value("owner").toString();
                    const auto annotation=result.value("annotation_json").toString();
                    const auto encoded=QString::fromUtf8(QJsonDocument(metadata).toJson(QJsonDocument::Compact));
                    return QMetaObject::invokeMethod(adapter,"verifyCollectionPolicy",Qt::DirectConnection,
                        Q_ARG(QVariant,handle),Q_ARG(QString,stage),Q_ARG(QString,owner),
                        Q_ARG(QString,annotation),Q_ARG(QString,encoded),Q_ARG(QJSValue,completion));
                };
                scriptDone=false;
                QVERIFY(verifyPolicy(following,topPolicy));
                QTRY_VERIFY(scriptDone);QCOMPARE(scriptResult.metaType().id(),int(QMetaType::Bool));
                QCOMPARE(scriptResult.toBool(),!rejectPolicy);
                if(rejectPolicy) {
                    const auto exhausted=QJsonDocument::fromJson(adapter->collectionWorkPlan(handle).toUtf8()).object();
                    QCOMPARE(exhausted.value("world").toString(),QStringLiteral("selected"));
                    QCOMPARE(exhausted.value("phase").toString(),QStringLiteral("policy"));
                    QCOMPARE(exhausted.value("owner").toString(),selectedOwner);
                    QCOMPARE(exhausted.value("ordinary").toObject().value("metadata").toObject().value("nonce"),child.value("interaction_pending"));
                    for(const auto &counter:{QStringLiteral("inspections"),QStringLiteral("predicate_steps"),QStringLiteral("annotation_bytes")})
                        QCOMPARE(exhausted.value("budget").toObject().value(counter).toInteger(),0);
                    QVERIFY(adapter->rejectCollectionWork(handle,exhausted.value("stage").toString(),selectedOwner));
                    QVERIFY(adapter->collectionWorkPlan(handle).isEmpty());
                    QVERIFY(!adapter->observationDocumentsChecked(handle));
                    QVERIFY(!adapter->observationObjectsChecked(handle));
                    QCOMPARE(text("hint_values"),originalValues);
                    QCOMPARE(page("window.clicks",1).toInt(),0);
                    delete adapter.data();
                    return;
                }
                const auto selectedGrant=adapter->collectionWorkPlan(handle);
                const auto selectedPolicy=QJsonDocument::fromJson(selectedGrant.toUtf8()).object();
                QCOMPARE(selectedPolicy.value("world").toString(),QStringLiteral("selected"));
                QCOMPARE(selectedPolicy.value("phase").toString(),QStringLiteral("policy"));
                QCOMPARE(selectedPolicy.value("owner").toString(),selectedOwner);
                QCOMPARE(selectedPolicy.value("ordinary").toObject().value("metadata").toObject().value("nonce"),child.value("interaction_pending"));
                scriptDone=false;
                QVERIFY(verifyPolicy(following,topPolicy));
                QTRY_VERIFY(scriptDone);QCOMPARE(scriptResult.toBool(),false);
                QCOMPARE(adapter->collectionWorkPlan(handle),selectedGrant);
                auto wrongOwner=selectedPolicy;wrongOwner.insert("owner",QString());
                scriptDone=false;
                QVERIFY(verifyPolicy(wrongOwner,topPolicy));
                QTRY_VERIFY(scriptDone);QCOMPARE(scriptResult.toBool(),false);
                QCOMPARE(adapter->collectionWorkPlan(handle),selectedGrant);
                scriptDone=false;
                broker->run(selectedOwner,"JSON.stringify("+finishScript(selectedPolicy)+")",completion);
                QTRY_VERIFY(scriptDone);
                const auto childPolicy=QJsonDocument::fromJson(scriptResult.toString().toUtf8()).object();
                if(coldChild) {
                    QCOMPARE(childPolicy.value("error").toString(),QStringLiteral("stale-interaction-collection"));
                    QCOMPARE(adapter->collectionWorkPlan(handle),selectedGrant);
                    QVERIFY(adapter->rejectCollectionWork(handle,selectedPolicy.value("stage").toString(),selectedOwner));
                    QVERIFY(adapter->collectionWorkPlan(handle).isEmpty());
                    QVERIFY(!adapter->observationDocumentsChecked(handle));
                    QVERIFY(!adapter->observationObjectsChecked(handle));
                    QCOMPARE(text("hint_values"),originalValues);
                    QCOMPARE(page("window.clicks",1).toInt(),0);
                    delete adapter.data();
                    return;
                }
                QVERIFY2(!childPolicy.isEmpty()&&!childPolicy.contains("error"),qPrintable(QString::fromUtf8(QJsonDocument(childPolicy).toJson(QJsonDocument::Compact))));
                QCOMPARE(childPolicy.value("annotation_batch"),child.value("interaction_pending"));
                const auto rawChild=QJsonDocument::fromJson(childPolicy.value("annotation_json").toString().toUtf8()).object();
                QVERIFY(!rawChild.value("roots").toArray().isEmpty());
                QVERIFY2(hasAnchorTarget(rawChild,QStringLiteral("foreign-primary"),childRows,selectedOwner,QStringLiteral("Foreign receiver")),
                    "Selected must retain its actual original foreign button witness and canonical owner");
                const auto childOrdinaryWork=selectedPolicy.value("ordinary").toObject().value("metadata").toObject();
                const auto childPolicyWork=childPolicy.value("interaction_work").toObject();
                const auto childRawWork=rawChild.value("discovery_work").toObject();
                for(const auto &counter:{QStringLiteral("inspections"),QStringLiteral("predicate_steps")}) {
                    QVERIFY(childRawWork.value(counter).isDouble());
                    QCOMPARE(childRawWork.value(counter).toInteger(),
                        childOrdinaryWork.value(counter).toInteger()+childPolicyWork.value(counter).toInteger());
                }
                scriptDone=false;
                QVERIFY(verifyPolicy(selectedPolicy,childPolicy));
                QTRY_VERIFY(scriptDone);QCOMPARE(scriptResult.toBool(),!policyFocusLoss);
                QVERIFY(adapter->collectionWorkPlan(handle).isEmpty());
                scriptDone=false;
                QVERIFY(verifyPolicy(selectedPolicy,childPolicy));
                QTRY_VERIFY(scriptDone);QCOMPARE(scriptResult.toBool(),false);
                QVERIFY(adapter->collectionWorkPlan(handle).isEmpty());
            }
        }
        QVERIFY(!adapter->observationDocumentsChecked(handle));
        QVERIFY(!adapter->observationObjectsChecked(handle));
        QCOMPARE(text("hint_values"),originalValues);
        QCOMPARE(page("window.clicks",1).toInt(),0);
        delete adapter.data();
        return;
    }
    if (foreignRole) {
        QVERIFY(!context.contains("focused_document"));
        const auto selectedPlan = QJsonDocument::fromJson(adapter->collectorPlanForOwner(handle, selectedOwner).toUtf8()).object();
        QVERIFY(!selectedPlan.isEmpty());
        const auto selectedContext = selectedPlan.value("context").toObject();
        QCOMPARE(selectedContext.value("owner").toString(), selectedOwner);
        QCOMPARE(selectedContext.value("start").toString(), QStringLiteral("focused-frame"));
        QCOMPARE(selectedContext.value("base_depth").toInt(), change == "role-return-origin" ? 2 : 1);
        QCOMPARE(selectedContext.value("receiver_origin").toString(),
            QStringLiteral("http://%1:%2").arg(change == "role-return-origin" ? "127.0.0.1" : "localhost").arg(server.serverPort()));
        const auto ancestors = evaluate("Object.keys(testWindow.hintWorkspace.hintFrameProvider.sources)").toStringList();
        QCOMPARE(ancestors.size(), change == "role-return-origin" ? 2 : 1);
        for (const auto &ancestor : ancestors)
            if (ancestor != selectedOwner) QVERIFY(adapter->collectorPlanForOwner(handle, ancestor).isEmpty());
        delete adapter.data();
        return;
    }
    if (needsFocusedFrame) {
        QVERIFY(adapter->collectorPlanForOwner(handle, selectedOwner).isEmpty());
        QCOMPARE(context.value("focused_document").toObject().value("depth").toInt(),1);
        QVERIFY(!context.value("focused_document").toObject().value("epoch").toString().isEmpty());
    }
    const auto lease = evaluate("testWindow.hintWorkspace.activeHintScopeLeaseId").toString();
    QCOMPARE(!lease.isEmpty(),scoped);
    const auto family = text("hint_family");
    QCOMPARE(family,scoped ? QStringLiteral("actions") : QStringLiteral("all"));
    const auto retainedStamp = QJsonDocument::fromJson(plan.toUtf8()).object().value("stamp").toObject();
    QCOMPARE(retainedStamp.value("scope_lease").toString(),lease);
    QCOMPARE(retainedStamp.value("family").toString(),family);
    const auto script = evaluate("testWindow.hintWorkspace.hintCollectorScript(" + quoted(family) + ",JSON.parse(" + quoted(selectors)
        + ")," + quoted(lease) + ",false,JSON.parse(" + quoted(plan) + "))").toString();
    QVERIFY(!script.isEmpty());
    const auto result = QJsonDocument::fromJson(page("JSON.stringify(" + script + ")", 1).toString().toUtf8()).object();
    QVERIFY2(!result.isEmpty(), qPrintable(text("status_text")));
    QVERIFY(!result.contains("error"));
    const auto candidates = QString::fromUtf8(QJsonDocument(result.value("candidates").toArray()).toJson(QJsonDocument::Compact));
    auto annotation = QJsonDocument::fromJson(result.value("annotation_json").toString().toUtf8()).object();
    QCOMPARE(annotation.value("documents").toArray().size(), 2);
    QCOMPARE(annotation.value("outcomes").toArray().size(), needsFocusedFrame ? 3 : 2);
    if (needsFocusedFrame) {
        QJsonObject focusedOutcome;
        for (const auto &outcome : annotation.value("outcomes").toArray()) {
            const auto object = outcome.toObject();
            if (object.value("rule").toObject().value("id") == "focused-primary")
                focusedOutcome = object.value("result").toObject().value("anchor-observed").toObject();
        }
        QCOMPARE(focusedOutcome.value("targets").toArray().size(),1);
        const auto target = focusedOutcome.value("targets").toArray().first().toObject().value("candidate").toObject();
        QCOMPARE(target.value("frame_owner").toString(),QString());
        const auto ordinary = result.value("candidates").toArray().at(target.value("index").toInt()).toObject();
        QCOMPARE(ordinary.value("element_id"),target.value("element_id"));
        QCOMPARE(ordinary.value("text").toString(),QStringLiteral("Frame control"));
        QCOMPARE(ordinary.value("frame_path"),target.value("frame_path"));
    }
    QJsonObject rawCollection;
    for (const auto &outcome : annotation.value("outcomes").toArray()) {
        const auto object = outcome.toObject();
        if (object.value("rule").toObject().value("id") == "excluded-child")
            rawCollection = object.value("result").toObject().value("collection-observed").toObject();
    }
    QCOMPARE(rawCollection.value("items").toArray().size(), 1);
    const auto excluded = rawCollection.value("items").toArray().first().toObject();
    const auto attributes = excluded.value("attributes").toArray();
    QCOMPARE(attributes.size(), 1);
    QCOMPARE(attributes.first().toObject().value("name").toString(), QStringLiteral("data-use"));
    QVERIFY(attributes.first().toObject().value("fact").toObject().contains("complete-absent"));
    QCOMPARE(excluded.value("primary").toObject().value("targets-complete").toObject().value("targets").toArray().size(), 1);
    auto batch = result.value("annotation_batch").toString();
    QVERIFY(!batch.isEmpty());
    const auto revision = QString::number(static_cast<qulonglong>(result.value("revision").toDouble()));
    QVERIFY(!adapter->observationDocumentsChecked(handle));
    QVERIFY(!adapter->observationObjectsChecked(handle));
    if (change == "changed-fact") {
        auto outcomes = annotation.value("outcomes").toArray();
        for (int i=0;i<outcomes.size();i++) {
            auto outcome = outcomes.at(i).toObject();
            if (outcome.value("rule").toObject().value("id") != "primary") continue;
            auto observed = outcome.value("result").toObject().value("anchor-observed").toObject();
            auto targets = observed.value("targets").toArray();
            QCOMPARE(targets.size(),1);
            auto target = targets.first().toObject();
            target.insert("name",QJsonObject{{"missing",QJsonObject{}}});
            targets.replace(0,target); observed.insert("targets",targets);
            outcome.insert("result",QJsonObject{{"anchor-observed",observed}});
            outcomes.replace(i,outcome);
        }
        annotation.insert("outcomes",outcomes);
    }
    if(change=="wrong-focused-document"||change=="missing-focused-name"||change=="unavailable-focused-root") {
        auto outcomes=annotation.value("outcomes").toArray();
        QJsonObject top;
        int focusedIndex=-1;
        for(int i=0;i<outcomes.size();i++) {
            const auto outcome=outcomes.at(i).toObject();
            const auto id=outcome.value("rule").toObject().value("id").toString();
            if(id=="primary")top=outcome.value("result").toObject().value("anchor-observed").toObject();
            if(id=="focused-primary")focusedIndex=i;
        }
        QVERIFY(focusedIndex>=0);
        auto outcome=outcomes.at(focusedIndex).toObject();
        auto focused=outcome.value("result").toObject().value("anchor-observed").toObject();
        if(change!="missing-focused-name") {
            const auto id=focused.value("root");
            auto roots=annotation.value("roots").toArray();
            QJsonObject topRoot;
            for(const auto &value:roots)if(value.toObject().value("id")==top.value("root"))topRoot=value.toObject();
            QVERIFY(!topRoot.isEmpty());
            for(int i=0;i<roots.size();i++)if(roots.at(i).toObject().value("id")==id) {
                topRoot.insert("id",id);roots.replace(i,topRoot);
            }
            annotation.insert("roots",roots);
            focused.insert("scope",top.value("scope"));
            focused.insert("scope_kind",top.value("scope_kind"));
            focused.insert("targets",top.value("targets"));
        }
        if(change!="wrong-focused-document") {
            auto targets=focused.value("targets").toArray();
            QCOMPARE(targets.size(),1);
            auto target=targets.first().toObject();
            target.insert("name",QJsonObject{{"missing",QJsonObject{}}});
            targets.replace(0,target);focused.insert("targets",targets);
        }
        outcome.insert("result",QJsonObject{{"anchor-observed",focused}});
        outcomes.replace(focusedIndex,outcome);annotation.insert("outcomes",outcomes);
    }
    if (change == "selected-leaf-epoch") {
        // The local Document/RootRuntime epoch stays unchanged; the retained
        // selected native receipt must independently reject its replaced token.
        page("document.getElementById('frame').contentWindow.__ferric_hintFrameDocument='native-selected-replaced';true",1);
    } else if (change == "bad-stamp") {
        auto stamp = annotation.value("stamp").toObject();
        stamp.insert("request_id", "999999999"); annotation.insert("stamp", stamp);
    } else if (change == "excluded-child-epoch") {
        auto documents = annotation.value("documents").toArray();
        auto child = documents.last().toObject();
        QVERIFY(child.value("parent_host").isObject());
        child.insert("epoch", "11111111-1111-4111-8111-111111111111");
        documents.replace(documents.size() - 1, child); annotation.insert("documents", documents);
    } else if (change == "bad-batch") {
        batch = "11111111-1111-4111-8111-111111111111";
    } else if (change == "scope-metadata") {
        QVERIFY(page("window.__ferric_browserHintState.scopeLease.elementId++;true",1).toBool());
    } else if (change == "stop-before" || change == "scope-stop") {
        evaluate("testWindow.browserUi.escape()");
        QTRY_COMPARE(text("mode"),QStringLiteral("normal"));
    } else if (change == "competing-collection") {
        const auto ordinaryScript=evaluate("testWindow.hintWorkspace.hintCollectorScript("+quoted(family)+",JSON.parse("
            +quoted(selectors)+"),"+quoted(lease)+",false)").toString();
        const auto ordinary=page(ordinaryScript,1).toMap();
        QVERIFY(!ordinary.contains("error"));QVERIFY(ordinary.contains("candidates"));
        QVERIFY(page("window.__ferric_browserHintState.interactionProof===null",1).toBool());
        QCOMPARE(adapter->collectorPlan(handle),plan);
    } else if (change == "publication-before") {
        const auto before = text("setting_action_id");
        QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('hints.marker_scale','1.1',true)").toBool());
        QTRY_VERIFY(text("setting_action_id") != before && text("setting_action_state") == "succeeded");
    }
    if (change == "strict-nonmatch-staging") {
        auto outcomes = annotation.value("outcomes").toArray();
        bool replaced = false;
        for (int index = 0; index < outcomes.size(); ++index) {
            auto outcome = outcomes.at(index).toObject();
            if (outcome.value("rule").toObject().value("id") != "primary") continue;
            auto facts = outcome.value("result").toObject().value("anchor-observed").toObject();
            QCOMPARE(facts.value("targets").toArray().size(), 1);
            facts.insert("matches", facts.take("targets"));
            outcome.insert("result", QJsonObject{{"anchor-complete", facts}});
            outcomes.replace(index, outcome);
            replaced = true;
        }
        QVERIFY(replaced);
        annotation.insert("outcomes", outcomes);
    }
    const auto encoded = QString::fromUtf8(QJsonDocument(annotation).toJson(QJsonDocument::Compact));
    const bool staging = change == "raw-seal-staging" || change == "strict-nonmatch-staging";
    if (staging) {
        // Test-only original-graph substitution keeps the real weak objects,
        // native receipt and browser policy. A strict false predicate must
        // reach its first native seal before classification rejects it.
        QVERIFY(page("(()=>{const state=window.__ferric_browserHintState,batch=state.interactionProof;"
            "batch.annotation=" + quoted(encoded) + ";window.__rawStagingGraphChecks=0;"
            "Object.defineProperty(state,'interactionProof',{configurable:true,get:function(){"
            "window.__rawStagingGraphChecks++;return batch;}});return true})()", 1).toBool());
    }
    if (change == "bad-handle" || change == "other-adapter") {
        QPointer<FerricHintRequestAdapter> other;
        if (change == "other-adapter") {
            other = createAdapter(); QVERIFY(other); other->setLifetime(lifetime);
        }
        scriptDone = false;
        verifyObservation(other ? other.data() : adapter.data(),
            other ? handle : QVariant(QVariantMap{{"request_id",1}}), candidates, encoded, batch, revision);
        QTRY_VERIFY(scriptDone); QCOMPARE(scriptResult.toBool(),false);
        QVERIFY(!adapter->observationDocumentsChecked(handle));
        if (other) delete other.data();
    }
    if (change.endsWith("-during")) {
        page("(()=>{const state=window.__ferric_browserHintState,batch=state.interactionProof;Object.defineProperty(state,'interactionProof',"
             "{configurable:true,get:function(){console.info('hint-observation-document-probe');return batch;}});return true})()",1);
        if (change == "destroy-during") hintAdapterDestructionTarget = adapter;
        else hintAdapterDestructionTarget.clear();
        hintAdapterSawProbe = false; hintAdapterCompletedBeforeProbe = false;
        const auto action = change == "destroy-during" ? QStringLiteral("testProbe.destroyHintRequestAdapter();")
            : QStringLiteral("sawProbe=true;testWindow.browserUi.escape();");
        const auto source = QStringLiteral("import QtQuick; Connections { property bool sawProbe:false; target:testWindow.activeWebView(); "
            "function onJavaScriptConsoleMessage(level,message,line,source){if(message==='hint-observation-document-probe'){%1}}}").arg(action);
        auto *observer = evaluate("Qt.createQmlObject("+quoted(source)+",testWindow)").value<QObject *>();
        QVERIFY(observer);
        scriptDone = false;
        verifyObservation(adapter,handle,candidates,encoded,batch,revision);
        if (change == "destroy-during") {
            QTRY_VERIFY(adapter.isNull()); QVERIFY(hintAdapterSawProbe); QVERIFY(!hintAdapterCompletedBeforeProbe);
        } else {
            QTRY_VERIFY(observer->property("sawProbe").toBool()); QTRY_COMPARE(text("mode"),QStringLiteral("normal"));
            QVERIFY(!adapter->observationDocumentsChecked(handle));
        }
        QTest::qWait(1100);
        QVERIFY(!scriptDone);
        if (adapter) delete adapter.data();
        return;
    }
    scriptDone = false;
    verifyObservation(adapter,handle,candidates,encoded,batch,revision);
    QTRY_VERIFY(scriptDone);
    const bool expected = change == "stable" || change == "scope-stable" || change == "focused-top-plan" || change == "missing-focused-name" || change == "one-shot" || change == "bad-handle" || change == "other-adapter" || change == "raw-seal-staging";
    const bool associated = scriptResult.toBool();
    if (associated != expected) {
        // Keep the result before diagnostic page() calls replace scriptResult.
        qWarning() << "Observation result mismatch" << QTest::currentDataTag()
                   << "batch" << batch << "revision" << revision
                   << "live" << page("(()=>{const s=window.__ferric_browserHintState;return JSON.stringify({focus:document.hasFocus(),active:document.activeElement&&document.activeElement.localName,dirty:s&&s.dirty,revision:s&&s.revision,batch:s&&s.interactionProof&&s.interactionProof.batch})})()",1)
                   << "workspace" << evaluate("JSON.stringify({epoch:testWindow.hintWorkspace.hintCollectionEpoch,pending:testWindow.hintWorkspace.hintRefreshPending,provider:testWindow.hintWorkspace.hintFrameProvider.epoch})");
    }
    QCOMPARE(associated,expected);
    QCOMPARE(adapter->observationDocumentsChecked(handle),expected);
    QCOMPARE(adapter->observationObjectsChecked(handle),objects && expected);
    // Each broker validation probes the original graph twice, around native
    // receipt revalidation. Success runs that pair before classification and
    // again afterward; strict predicate rejection runs only the first pair.
    if (staging) QCOMPARE(page("window.__rawStagingGraphChecks", 1).toInt(), expected ? 4 : 2);
    if (change == "one-shot") {
        scriptDone = false;
        verifyObservation(adapter,handle,candidates,encoded,batch,revision);
        QTRY_VERIFY(scriptDone); QCOMPARE(scriptResult.toBool(),false);
        QVERIFY(adapter->observationDocumentsChecked(handle));
        QCOMPARE(adapter->observationObjectsChecked(handle),objects);
    }
    QCOMPARE(page("window.clicks",1).toInt(),0);
    delete adapter.data();
}

namespace {
// A failed assertion must not strand this fixture's constructed down in the
// application-owned capture when generic application cleanup deletes a router.
class NativeProbeGesture {
    QPointer<QQuickWindow> window_;
    QPointer<FerricBrowserKeyRouter> router_;
    QSet<int> held_;
public:
    NativeProbeGesture(QQuickWindow *window, FerricBrowserKeyRouter *router)
        : window_(window), router_(router) {}
    ~NativeProbeGesture() {
        const auto held = held_;
        for (const auto key : held) send(key, false);
        if (router_) router_->setEnabled(false);
    }
    void send(int key, bool down, bool repeat = false,
              Qt::KeyboardModifiers modifiers = Qt::NoModifier) {
        if (!window_) return;
        if (!repeat) {
            if (down) held_.insert(key);
            else held_.remove(key);
        }
        const quint32 scan = key == Qt::Key_P ? 33 : key == Qt::Key_U ? 30
            : key == Qt::Key_J ? 44 : key == Qt::Key_L ? 46 : 50;
        const auto value = key >= Qt::Key_A && key <= Qt::Key_Z
            ? QString(QChar(key)).toLower() : QString();
        QKeyEvent event(down ? QEvent::KeyPress : QEvent::KeyRelease,
            key, modifiers, scan, value.isEmpty() ? 0 : value.front().unicode(), 0, value, repeat);
        QCoreApplication::sendEvent(window_, &event);
    }
};

FerricBrowserKeyRouter *nativeProbeRouter(QQuickWindow *window)
{
    FerricBrowserKeyRouter *router = nullptr;
    for (auto *candidate : window->findChildren<FerricBrowserKeyRouter *>()) {
        if (candidate->targetWindow() == window) {
            if (router) return nullptr;
            router = candidate;
        }
    }
    return router;
}
}

void ApplicationTests::sitePolicyNativeProductionGesture_data()
{
    QTest::addColumn<bool>("once");
    QTest::addColumn<bool>("bindingEntry");
    QTest::addColumn<bool>("overlap");
    QTest::newRow("persistent-command") << false << false << false;
    QTest::newRow("persistent-binding-entry-release") << false << true << false;
    QTest::newRow("one-shot-command") << true << false << false;
    QTest::newRow("one-shot-binding-entry-release") << true << true << false;
    QTest::newRow("one-shot-overlap-after-normal") << true << true << true;
}

void ApplicationTests::sitePolicyNativeProductionGesture()
{
    QFETCH(bool, once);
    QFETCH(bool, bindingEntry);
    QFETCH(bool, overlap);
    auto *receiver = qobject_cast<QQuickItem *>(evaluate("testWindow.activeWebView()").value<QObject *>());
    QVERIFY(receiver);
    page("document.getElementById('button').focus();window.nativePolicyEvents=[];"
         "for(const kind of ['keydown','keyup'])document.addEventListener(kind,e=>"
         "nativePolicyEvents.push({type:e.type,key:e.key,code:e.code,repeat:e.repeat,"
         "trusted:e.isTrusted,ctrl:e.ctrlKey,shift:e.shiftKey,alt:e.altKey,meta:e.metaKey}));true");
    receiver->forceActiveFocus();
    evaluate("testWindow.browserUi.escape()");
    QTRY_COMPARE(text("mode"), QStringLiteral("normal"));
    auto *router = nativeProbeRouter(window);
    QVERIFY(router);
    NativeProbeGesture source(window, router);
    QTRY_VERIFY(router->enabled());
    // The startup policy uses the same compiled admission as ordinary routing.
    QSignalSpy completions(router, &FerricBrowserKeyRouter::passNextKeyDispatched);
    const auto mode = once ? QStringLiteral("passthrough-once") : QStringLiteral("passthrough");
    if (bindingEntry) {
        QVERIFY(command("bind u mode-enter " + mode));
        source.send(Qt::Key_U, true);
    } else {
        QVERIFY(command("mode-enter " + mode));
    }
    const auto canonicalMode = once ? QStringLiteral("pass-through-once") : QStringLiteral("pass-through");
    QTRY_COMPARE(text("mode"), canonicalMode);
    if (bindingEntry) source.send(Qt::Key_U, false);
    QCOMPARE(completions.count(), 0);
    QCOMPARE(text("mode"), canonicalMode);
    // A modifier-only gesture and an entry release cannot select the one shot.
    source.send(Qt::Key_Shift, true, false, Qt::ShiftModifier);
    source.send(Qt::Key_Shift, false);
    QTRY_VERIFY(page("nativePolicyEvents.some(e=>e.key==='Shift'&&e.type==='keyup')").toBool());
    QCOMPARE(completions.count(), 0);
    QCOMPARE(text("mode"), canonicalMode);
    evaluate("testWindow.probeActiveFocus(testWindow.browserUi,testWindow.browserUi.active_tab_index,testWindow.activeWebView())");
    QTRY_VERIFY(evaluate("JSON.parse(testWindow.browserUi.page_key_route('p')).allowed===true").toBool());
    if(qEnvironmentVariableIsSet("FERRIC_P15_NATIVE_ROUTING")&&!once&&!bindingEntry) {
        const bool smoke=qEnvironmentVariableIsSet("FERRIC_SITE_POLICY_PERF_SMOKE");
        const int warmups=smoke?1:10,measured=smoke?2:1000;
        QJsonArray raw,mapped,rawRelease,mappedRelease;
        for(const bool rawMode:{true,false}) {
            QVERIFY(command(rawMode?"mode-enter passthrough --raw":"mode-enter passthrough"));
            evaluate("testWindow.probeActiveFocus(testWindow.browserUi,testWindow.browserUi.active_tab_index,testWindow.activeWebView())");
            if(!rawMode)QTRY_VERIFY(evaluate("JSON.parse(testWindow.browserUi.page_key_route('p')).allowed===true").toBool());
            QVERIFY(page("nativePolicyEvents=[];true").toBool());
            for(int index=-warmups;index<measured;++index) {
                if(index==0)evaluate("testWindow.cancelFocusProbes();testWindow.runtime_scrollCaptureTimer.stop()");
                QElapsedTimer timer;timer.start();source.send(Qt::Key_P,true);
                const double elapsed=timer.nsecsElapsed()/1e6;
                timer.restart();
                source.send(Qt::Key_P,false);
                const double released=timer.nsecsElapsed()/1e6;
                if(index==0)QVERIFY2(evaluate("testWindow.focusProbeRetries===0 && !testWindow.runtime_focusProbeTimer.running && !testWindow.runtime_scrollCaptureTimer.running").toBool(),
                    "A native page key must not restart browser-action focus/scroll polling");
                QTRY_COMPARE(page("nativePolicyEvents.length").toInt(),2*(index+warmups+1));
                if(index>=0){(rawMode?raw:mapped).append(elapsed);(rawMode?rawRelease:mappedRelease).append(released);}
            }
            QVERIFY(page(rawMode?"nativePolicyEvents.every(e=>e.key==='p'&&e.trusted)":"nativePolicyEvents.every(e=>e.key==='k'&&e.trusted)").toBool());
            QCOMPARE(router->feasibilitySourceCount(),0);QCOMPARE(router->feasibilityOutputCount(),0);
        }
        const auto percentile=[](const QJsonArray &values,int percent){QList<double> sorted;for(const auto value:values)sorted.append(value.toDouble());std::sort(sorted.begin(),sorted.end());return sorted[(sorted.size()*percent+99)/100-1];};
        QJsonArray added;for(int i=0;i<measured;++i)added.append(std::max({0.,mapped[i].toDouble()-raw[i].toDouble(),mappedRelease[i].toDouble()-rawRelease[i].toDouble()}));
        const double p99=percentile(added,99);
        qInfo().noquote()<<"FERRIC_SITE_POLICY_RENDER_JSON "+QJsonDocument(QJsonObject{
            {"journey","synchronous-native-routing"},{"run_kind",smoke?"functional-smoke-not-baseline":"performance-baseline"},
            {"warmups",warmups},{"requested_measured_samples",measured},{"completed_measured_samples",measured},
            {"samples_ms",added},{"raw_samples_ms",raw},{"mapped_samples_ms",mapped},
            {"raw_release_samples_ms",rawRelease},{"mapped_release_samples_ms",mappedRelease},
            {"raw_p99_ms",percentile(raw,99)},{"mapped_p99_ms",percentile(mapped,99)},{"p99_ms",p99},
            {"max_ms",percentile(added,100)},{"dpr",window->devicePixelRatio()},{"failures",smoke||p99<=1.?0:1},
            {"boundary","Constructed native Qt press/release to synchronous event-filter return, including Qt input dispatch; renderer receipt outside timing. Raw then mapped blocks, max nonnegative press/release difference by sample index; not OS physical-input qualification."}
        }).toJson(QJsonDocument::Compact);
        if(!smoke)QVERIFY2(p99<=1.,"Native routing p99 added time exceeds 1ms");
        return;
    }
    if (overlap) source.send(Qt::Key_J, true);
    source.send(Qt::Key_P, true);
    QTRY_VERIFY(page("nativePolicyEvents.some(e=>e.key==='k'&&e.type==='keydown')").toBool());
    QCOMPARE(text("mode"), canonicalMode);
    QCOMPARE(completions.count(), 0);
    if (overlap) {
        source.send(Qt::Key_J, false);
        QTRY_COMPARE(text("mode"), QStringLiteral("normal"));
        QCOMPARE(completions.count(), 1);
        QCOMPARE(router->feasibilitySourceCount(), 1);
    }
    source.send(Qt::Key_P, false, true);
    QCOMPARE(completions.count(), overlap ? 1 : 0);
    source.send(Qt::Key_P, true, true);
    source.send(Qt::Key_P, false);
    QTRY_VERIFY(page("nativePolicyEvents.some(e=>e.key==='k'&&e.type==='keyup')").toBool());
    QTRY_COMPARE(text("mode"), once ? QStringLiteral("normal") : canonicalMode);
    QCOMPARE(completions.count(), once ? 1 : 0);
    QCOMPARE(router->feasibilitySourceCount(), 0);
    QCOMPARE(router->feasibilityOutputCount(), 0);
    const auto trace = page("nativePolicyEvents");
    qInfo().noquote() << "P01 production BrowserUi/QML (constructed Qt source):"
                     << QJsonDocument::fromVariant(trace).toJson(QJsonDocument::Compact);
    QCOMPARE(page("nativePolicyEvents.filter(e=>['p','k','j','u'].includes(e.key)).map(e=>e.key).join(',')").toString(),
             overlap ? QStringLiteral("j,k,j,k,k") : QStringLiteral("k,k,k"));
    QCOMPARE(page("nativePolicyEvents.filter(e=>e.key==='k').map(e=>e.type).join(',')").toString(),
             QStringLiteral("keydown,keydown,keyup"));
    QCOMPARE(page("nativePolicyEvents.filter(e=>e.key==='k').map(e=>e.repeat).join(',')").toString(),
             QStringLiteral("false,true,false"));
    QVERIFY(page("nativePolicyEvents.filter(e=>e.key==='k').every(e=>e.code==='KeyK'&&e.trusted"
                 "&&!e.ctrl&&!e.shift&&!e.alt&&!e.meta)").toBool());
}

void ApplicationTests::sitePolicyNativeProductionPrompt()
{
    auto *receiver = qobject_cast<QQuickItem *>(evaluate("testWindow.activeWebView()").value<QObject *>());
    QVERIFY(receiver);
    page("document.getElementById('button').focus();window.nativePolicyEvents=[];"
         "for(const kind of ['keydown','keyup'])document.addEventListener(kind,e=>"
         "nativePolicyEvents.push({type:e.type,key:e.key,repeat:e.repeat}));true");
    receiver->forceActiveFocus();
    auto *router = nativeProbeRouter(window);
    QVERIFY(router);
    NativeProbeGesture source(window, router);
    QTRY_VERIFY(router->enabled());
    QVERIFY(command("mode-enter passthrough"));
    evaluate("testWindow.probeActiveFocus(testWindow.browserUi,testWindow.browserUi.active_tab_index,testWindow.activeWebView())");
    QTRY_VERIFY(evaluate("JSON.parse(testWindow.browserUi.page_key_route('p')).allowed===true").toBool());
    source.send(Qt::Key_P, true);
    QTRY_COMPARE(page("nativePolicyEvents.length").toInt(), 1);
    QCOMPARE(page("nativePolicyEvents[0].key").toString(), QStringLiteral("k"));
    page("setTimeout(()=>{window.nativePolicyAnswer=confirm('P01 held key prompt')},0);true");
    QTRY_VERIFY(evaluate("testWindow.pendingPageDialogRequest!==null").toBool());
    QTRY_COMPARE(text("mode"), QStringLiteral("normal"));
    QCOMPARE(router->feasibilitySourceCount(), 1);
    QCOMPARE(router->feasibilityOutputCount(), 0);
    source.send(Qt::Key_P, true, true);
    source.send(Qt::Key_P, false, true);
    QCOMPARE(router->feasibilitySourceCount(), 1);
    // Reject through the real request owner; do not fake a fixture mode flag.
    evaluate("testWindow.clearPageDialog(true)");
    QTRY_VERIFY(evaluate("testWindow.pendingPageDialogRequest===null").toBool());
    QTRY_COMPARE(text("mode"), QStringLiteral("pass-through"));
    QVERIFY(page("nativePolicyAnswer === false").toBool());
    receiver->forceActiveFocus();
    QTRY_VERIFY(router->enabled());
    QVERIFY(!router->canEnterRawPassThrough());
    const auto beforeRelease = page("nativePolicyEvents").toList();
    QVERIFY(beforeRelease.size() == 1 || beforeRelease.size() == 2);
    QCOMPARE(beforeRelease.first().toMap().value("key").toString(), QStringLiteral("k"));
    QCOMPARE(beforeRelease.first().toMap().value("type").toString(), QStringLiteral("keydown"));
    QCOMPARE(beforeRelease.first().toMap().value("repeat").toBool(), false);
    if (beforeRelease.size() == 2) {
        QCOMPARE(beforeRelease.last().toMap().value("key").toString(), QStringLiteral("k"));
        QCOMPARE(beforeRelease.last().toMap().value("type").toString(), QStringLiteral("keyup"));
        QCOMPARE(beforeRelease.last().toMap().value("repeat").toBool(), false);
    }
    source.send(Qt::Key_P, true, true);
    source.send(Qt::Key_P, false);
    source.send(Qt::Key_L, true); source.send(Qt::Key_L, false);
    QTRY_VERIFY(page("nativePolicyEvents.some(e=>e.key==='l'&&e.type==='keyup')").toBool());
    const auto afterRelease = page("nativePolicyEvents").toList();
    qInfo().noquote() << "P01 production prompt:" << QJsonDocument::fromVariant(afterRelease).toJson(QJsonDocument::Compact);
    QCOMPARE(afterRelease.size(), beforeRelease.size() + 2);
    QCOMPARE(afterRelease.mid(0, beforeRelease.size()), beforeRelease);
    QVERIFY(page("nativePolicyEvents.every(e=>e.key==='k'||e.key==='l')").toBool());
    QCOMPARE(router->feasibilitySourceCount(), 0);
    QCOMPARE(router->feasibilityOutputCount(), 0);
    QVERIFY(router->canEnterRawPassThrough());
}

void ApplicationTests::sitePolicyNativeProductionViewDestruction()
{
    const QPointer<QQuickItem> original(qobject_cast<QQuickItem *>(evaluate("testWindow.activeWebView()").value<QObject *>()));
    QVERIFY(original);
    QVERIFY(command("open --target tab-bg " + text("current_url") + "?native-replacement=1"));
    evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
    QTRY_COMPARE(evaluate("testWindow.tabViews.length").toInt(), 2);
    pageReceiver = "testWindow.tabViewAt(1)";
    QTRY_VERIFY(!evaluate(pageReceiver + ".loading").toBool());
    QTRY_VERIFY(page("document.readyState==='complete'&&!!document.getElementById('button')").toBool());
    page("window.nativePolicyEvents=[];for(const kind of ['keydown','keyup'])"
         "document.addEventListener(kind,e=>nativePolicyEvents.push({type:e.type,key:e.key,repeat:e.repeat}));true");
    pageReceiver = "testWindow.activeWebView()";
    page("document.getElementById('button').focus();window.nativePolicyEvents=[];"
         "for(const kind of ['keydown','keyup'])document.addEventListener(kind,e=>"
         "nativePolicyEvents.push({type:e.type,key:e.key,repeat:e.repeat}));true");
    original->forceActiveFocus();
    auto *router = nativeProbeRouter(window);
    QVERIFY(router);
    NativeProbeGesture source(window, router);
    QTRY_VERIFY(router->enabled());
    QVERIFY(command("mode-enter passthrough"));
    evaluate("testWindow.probeActiveFocus(testWindow.browserUi,testWindow.browserUi.active_tab_index,testWindow.activeWebView())");
    QTRY_VERIFY(evaluate("JSON.parse(testWindow.browserUi.page_key_route('p')).allowed===true").toBool());
    source.send(Qt::Key_P, true);
    QTRY_COMPARE(page("nativePolicyEvents.length").toInt(), 1);
    QCOMPARE(page("nativePolicyEvents[0].key").toString(), QStringLiteral("k"));
    // Use the tab-strip close route, which remains available in passthrough.
    // The tab-close command itself is restricted to Normal/Command modes.
    QVERIFY(evaluate("testWindow.closeTabAtIndex(0)").toBool());
    QTRY_VERIFY(original.isNull());
    QTRY_VERIFY(evaluate("!!testWindow.activeWebView()&&!testWindow.activeWebView().loading").toBool());
    auto *replacement = qobject_cast<QQuickItem *>(evaluate("testWindow.activeWebView()").value<QObject *>());
    QVERIFY(replacement);
    QTRY_VERIFY(replacement->isVisible());
    QVERIFY(page("document.getElementById('button').focus();true").toBool());
    replacement->forceActiveFocus();
    QTRY_VERIFY(page("document.hasFocus()").toBool());
    QTRY_VERIFY(window->activeFocusItem() == replacement
                || replacement->isAncestorOf(window->activeFocusItem()));
    QVERIFY(command("mode-enter passthrough"));
    QTRY_COMPARE(text("mode"), QStringLiteral("pass-through"));
    QTRY_VERIFY(router->enabled());
    QCOMPARE(router->feasibilitySourceCount(), 1);
    QCOMPARE(router->feasibilityOutputCount(), 0);
    source.send(Qt::Key_P, true, true);
    source.send(Qt::Key_P, false, true);
    source.send(Qt::Key_P, false);
    source.send(Qt::Key_L, true); source.send(Qt::Key_L, false);
    QTRY_VERIFY2(page("nativePolicyEvents.some(e=>e.key==='l'&&e.type==='keyup')").toBool(),
        qPrintable(QStringLiteral("Replacement state: %1; mode=%2 source=%3 output=%4 enabled=%5")
            .arg(page("JSON.stringify({events:nativePolicyEvents,focus:document.hasFocus(),"
                      "active:document.activeElement&&document.activeElement.id,url:location.href})").toString())
            .arg(text("mode")).arg(router->feasibilitySourceCount())
            .arg(router->feasibilityOutputCount()).arg(router->enabled())));
    qInfo().noquote() << "P01 production view replacement:"
        << QJsonDocument::fromVariant(page("nativePolicyEvents")).toJson(QJsonDocument::Compact);
    QCOMPARE(page("nativePolicyEvents.map(e=>e.key).join(',')").toString(), QStringLiteral("l,l"));
    QCOMPARE(router->feasibilitySourceCount(), 0);
    QCOMPARE(router->feasibilityOutputCount(), 0);
    QVERIFY(router->canEnterRawPassThrough());
}

void ApplicationTests::sitePolicyDraftWorkflow_data()
{
    QTest::addColumn<QString>("journey");
    for(const auto *journey:{"temporary","temporary-activate","generated","generated-activate","stale-revision","rebase"})
        QTest::newRow(journey)<<QString::fromLatin1(journey);
}

void ApplicationTests::sitePolicyWorkspaceEditor()
{
    QVERIFY(page("window.sitePolicyDocument='editor-document';true").toBool());
    QVERIFY(command("site-policy"));
    QTRY_VERIFY(evaluate("testWindow.sitePoliciesVisible").toBool());
    const auto workspaceName=QStringLiteral("testWindow.runtime_policyWorkspace");
    auto *workspace=evaluate(workspaceName).value<QObject *>();
    QVERIFY(workspace);
    const auto pattern=QString("http://127.0.0.1:%1/*").arg(server.serverPort());
    QVERIFY(evaluate(workspaceName+".openRule('temporary','native-editor',true,"+quoted(pattern)+");true").toBool());
    QTRY_VERIFY(evaluate(workspaceName+".selected !== null").toBool());
    auto *editorTab=workspace->findChild<QObject *>("policyEditorTab");
    QVERIFY(editorTab);
    QVERIFY(QMetaObject::invokeMethod(editorTab,"click",Qt::DirectConnection));
    QTRY_VERIFY(editorTab->property("checked").toBool());
    const std::function<QQuickItem *(QQuickItem *)> findField=[&](QQuickItem *item)->QQuickItem * {
        if (item->objectName()=="policyField:rules.0.pattern") return item;
        for (auto *child:item->childItems()) if (auto *found=findField(child)) return found;
        return nullptr;
    };
    QTRY_VERIFY(findField(qobject_cast<QQuickItem *>(workspace)));
    auto *field=findField(qobject_cast<QQuickItem *>(workspace));
    const auto edited=QString("http://127.0.0.1:%1/editor/*").arg(server.serverPort());
    field->forceActiveFocus();
    QTRY_VERIFY(field->hasActiveFocus());
    key(Qt::Key_A,Qt::ControlModifier);
    typeText(edited);
    QCOMPARE(field->property("text").toString(),edited);
    QCOMPARE(evaluate(workspaceName+".selected.proposals[0].pattern").toString(),edited);
    const auto id=evaluate(workspaceName+".selected.view.id").toString();
    QVERIFY(!id.isEmpty());
    QVERIFY(evaluate(workspaceName+".review("+workspaceName+".selected);"+workspaceName+
        ".submit("+workspaceName+".selected,{action:'apply',activate:true});true").toBool());
    const auto draft=[&]{return QJsonDocument::fromJson(evaluate("testWindow.browserUi.site_policy_draft_snapshot("+quoted(id)+")").toString().toUtf8()).object();};
    QTRY_COMPARE(draft().value("operation").toObject().value("state").toString(),QString("applied"));
    QTRY_COMPARE(evaluate("("+workspaceName+".selected.view.operation || {}).state").toString(),QString("applied"));
    QCOMPARE(draft().value("rules").toArray()[0].toObject().value("proposed").toObject().value("pattern").toString(),edited);
    QCOMPARE(page("window.sitePolicyDocument").toString(),QString("editor-document"));
    QCOMPARE(evaluate(workspaceName+".selected.proposals[0].pattern").toString(),edited);
    const auto artifacts=qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
    if (!artifacts.isEmpty()) {
        QVERIFY(QDir().mkpath(artifacts));
        QVERIFY(window->grabWindow().save(artifacts+"/site-policy-editor.png"));
    }
    QVERIFY(evaluate(workspaceName+".close();true").toBool());
    QVERIFY(!evaluate("testWindow.sitePoliciesVisible").toBool());
    QCOMPARE(draft().value("id").toString(),id);
}

void ApplicationTests::sitePolicyWorkspaceSharing()
{
    const auto active=evaluate("testWindow.browserUi.interaction_policy_snapshot()").toString();
    const auto source=directory.filePath("shared \"policy\";quit.toml");
    QFile input(source);QVERIFY(input.open(QIODevice::WriteOnly));
    input.write("format='ferric-site-interaction'\nformat_version=1\n[[rules]]\noperation='upsert-interaction'\nid='shared'\npattern='https://example.test/*'\npriority=0\n[rules.interaction]\nenabled=true\n");input.close();
    QVERIFY(command("site-policy"));
    const auto name=QStringLiteral("testWindow.runtime_policyWorkspace");
    auto *surface=qobject_cast<QQuickItem *>(evaluate(name).value<QObject *>());QVERIFY(surface);
    const std::function<QQuickItem *(QQuickItem *,const char *,const QString &)> find=
        [&](QQuickItem *item,const char *property,const QString &value)->QQuickItem * {
            if(item->property(property).toString()==value)return item;
            for(auto *child:item->childItems())if(auto *found=find(child,property,value))return found;
            return nullptr;
        };
    const auto click=[&](const QString &label){auto *button=find(surface,"text",label);return button&&QMetaObject::invokeMethod(button,"click",Qt::DirectConnection);};
    const auto result=[&]{return QJsonDocument::fromJson(text("site_policy_result").toUtf8()).object();};
    QVERIFY(click("Import / export"));
    auto *path=find(surface,"placeholderText","Absolute local path");QVERIFY(path);QVERIFY(path->setProperty("text",source));
    QVERIFY(click("Import draft"));
    QTRY_VERIFY_WITH_TIMEOUT(evaluate(name+".selected !== null").toBool(),10000);
    QCOMPARE(evaluate(name+".selected.view.source").toString(),QString("temporary"));
    QCOMPARE(evaluate(name+".selected.view.rules[0].id").toString(),QString("shared"));
    QCOMPARE(evaluate("testWindow.browserUi.interaction_policy_snapshot()").toString(),active);
    auto *exportButton=find(surface,"text","Export selected draft");QVERIFY(exportButton);
    QVERIFY(!exportButton->isEnabled());
    QVERIFY(click("I understand export persists this draft to disk"));
    const auto target=directory.filePath("exported.toml");QVERIFY(path->setProperty("text",target));
    QVERIFY(exportButton->isEnabled());QVERIFY(click("Export selected draft"));
    QTRY_COMPARE_WITH_TIMEOUT(result().value("status").toString(),QString("exported"),10000);
    QVERIFY(!result().value("persistence_warning").toString().isEmpty());
    QFile output(target);QVERIFY(output.open(QIODevice::ReadOnly));const auto bytes=output.readAll();output.close();
    QVERIFY(bytes.contains("ferric-site-interaction"));QVERIFY(!bytes.contains("capture"));
    QVERIFY(click("Export selected draft"));
    QTRY_VERIFY_WITH_TIMEOUT(result().contains("error"),10000);
    QVERIFY(output.open(QIODevice::ReadOnly));QCOMPARE(output.readAll(),bytes);output.close();
    QVERIFY(click("Allow replacing the export file"));QVERIFY(click("Export selected draft"));
    QTRY_COMPARE_WITH_TIMEOUT(result().value("status").toString(),QString("exported"),10000);
    const auto link=directory.filePath("link.toml");QVERIFY(QFile::link(target,link));QVERIFY(path->setProperty("text",link));
    QVERIFY(click("Export selected draft"));QTRY_VERIFY_WITH_TIMEOUT(result().contains("error"),10000);
    QVERIFY(QFileInfo(link).isSymLink());
    for(const auto &invalid:{QByteArray(bytes).replace("format_version = 1","format_version = 2"),QByteArray("format='ferric-site-interaction'\nformat_version=1\ninclude=['other.toml']\n")}) {
        QVERIFY(input.open(QIODevice::WriteOnly|QIODevice::Truncate));input.write(invalid);input.close();
        QVERIFY(path->setProperty("text",source));QVERIFY(click("Import draft"));
        QTRY_VERIFY_WITH_TIMEOUT(result().contains("error"),10000);
        QCOMPARE(evaluate(name+".sessions.length").toInt(),1);
    }
    QCOMPARE(evaluate("testWindow.browserUi.interaction_policy_snapshot()").toString(),active);
}

void ApplicationTests::sitePolicyBuilderCapture_data()
{
    QTest::addColumn<QString>("journey");
    for(const auto *journey:{"anchor","input-close","collection","shadow","slot","slot-collection","same-origin-frame","focused-frame","samples","mutated","zoom","closed","window-close","navigation","history","prompt","modal","seal-stale-once","seal-stale-twice"})
        QTest::newRow(journey)<<QString::fromLatin1(journey);
}

void ApplicationTests::sitePolicyBuilderCapture()
{
    QFETCH(QString,journey);
    const bool frameCase=journey=="same-origin-frame"||journey=="focused-frame";
    QWebEngineFrame capturedFrame;
    const auto frameRead=[&](const QString &script) {
        scriptDone=false;capturedFrame.runJavaScript(script,0,[this](const QVariant &value){accept(value);});
        QElapsedTimer timer;timer.start();while(!scriptDone&&timer.elapsed()<5000)QTest::qWait(5);
        return scriptDone?scriptResult:QVariant();
    };
    QVERIFY(page(R"JS((()=>{
        document.body.innerHTML='<section id="list"><article class="card"><a id="first" href="/never">First anchor</a></article><article class="card"><a href="/never">Second anchor</a></article></section><section id="shadow-list"><x-card id="shadow-one"></x-card><x-card id="shadow-two"></x-card></section><x-slot id="slots"><a id="slotted" slot="entry" href="/never">Slot primary</a></x-slot><input type=\"password\" value=\"private-password\"><input value=\"private-text\">';
        for(const id of ['shadow-one','shadow-two'])document.getElementById(id).attachShadow({mode:'open'}).innerHTML='<button>Shadow primary</button>';
        document.getElementById('slots').attachShadow({mode:'open'}).innerHTML='<slot name="entry"></slot>';
        window.captureEvents={click:0,focus:0,key:0};window.captureDocument='same';
        document.addEventListener('click',()=>captureEvents.click++);document.addEventListener('focusin',()=>captureEvents.focus++);document.addEventListener('keydown',()=>captureEvents.key++);document.addEventListener('keyup',()=>captureEvents.key++);return true;
    })())JS").toBool());
    if(journey=="slot-collection")QVERIFY(page("document.getElementById('slots').innerHTML='<section id=assigned-cards slot=entry><article class=card><a href=/never>Slot primary</a></article><article class=card><a href=/never>Second slot primary</a></article></section>';true").toBool());
    if(frameCase) {
        frameFixtures["/capture-frame.html"]="<!doctype html><button id=frame-primary>Frame primary</button>";
        const auto url=QString("http://%1:%2/capture-frame.html").arg(journey=="focused-frame"?"localhost":"127.0.0.1").arg(server.serverPort());
        QVERIFY(page("var frame=document.createElement('iframe');frame.id='capture-frame';frame.style='position:fixed;left:20px;top:420px;width:420px;height:140px';frame.src="+quoted(url)+";document.body.append(frame);true").toBool());
        QTRY_COMPARE(evaluate("testWindow.activeWebView().mainFrame.children.length").toInt(),1);
        capturedFrame=evaluate("testWindow.activeWebView().mainFrame.children[0]").value<QWebEngineFrame>();QVERIFY(capturedFrame.isValid());
        QTRY_VERIFY(frameRead("document.readyState==='complete'&&!!document.getElementById('frame-primary')").toBool());
        QVERIFY(frameRead("document.getElementById('frame-primary').focus();window.captureEvents={click:0,focus:0,key:0};for(const type of ['click','focusin','keydown','keyup'])document.addEventListener(type,()=>captureEvents[type==='focusin'?'focus':type==='click'?'click':'key']++);true").toBool());
        QVERIFY(page("document.getElementById('capture-frame').focus();captureEvents={click:0,focus:0,key:0};true").toBool());
    }
    if(journey=="anchor"||journey=="input-close")QVERIFY(page("document.querySelector('input:not([type=password])').focus();window.captureOriginalFocus=document.activeElement;captureEvents={click:0,focus:0,key:0};true").toBool());
    const auto originalUrl=page("location.href").toString();
    if(journey=="history") {
        evaluate("testWindow.activeWebView().settings.backForwardCacheEnabled=true");
        QVERIFY(page("window.captureRestored=false;window.captureKeyDetails=[];for(const type of ['keydown','keyup'])addEventListener(type,event=>captureKeyDetails.push({type:event.type,key:event.key}));addEventListener('pageshow',event=>{captureRestored=event.persisted});true").toBool());
    }
    QVERIFY(command("site-policy"));QTRY_VERIFY(evaluate("testWindow.sitePoliciesVisible").toBool());
    const auto workspace=QStringLiteral("testWindow.runtime_policyWorkspace"),builder=workspace+".guidedBuilder",runtime=QStringLiteral("testWindow.sitePolicyPreviewWorkspace");
    evaluate(workspace+".openRule('temporary','capture',true,"+quoted(QString("http://127.0.0.1:%1/*").arg(server.serverPort()))+")");
    QTRY_VERIFY(evaluate(workspace+".selected !== null").toBool());
    const auto draft=evaluate(workspace+".selected.view.id").toString();
    const bool collection=journey=="collection"||journey=="shadow"||journey=="slot-collection";
    evaluate(builder+".begin("+quoted(collection?"collection":"anchor")+")");
    auto *surface=evaluate(workspace).value<QObject *>();QVERIFY(surface);
    auto *tab=surface->findChild<QObject *>("policyBuilderTab");QVERIFY(tab);
    QVERIFY(QMetaObject::invokeMethod(tab,"click",Qt::DirectConnection));
    window->requestActivate();QTRY_VERIFY(window->isActive());
    const bool sealRace=journey.startsWith("seal-stale-");
    if(sealRace) {
        // Deliver a controlled tracking change after policy extraction, while
        // the real native seal must still reject that original proof.
        QVERIFY(page(QString(R"JS((()=>{
            window.p15SealDirtyCount=0;window.ResizeObserver=class{observe(){}unobserve(){}disconnect(){}};
            let state=window.__ferric_browserHintState;
            function instrument(value){if(!value)return;let proof=value.interactionProof;
                Object.defineProperty(value,'interactionProof',{configurable:true,get(){return proof},set(next){proof=next;
                    if(next&&p15SealDirtyCount<%1){p15SealDirtyCount++;queueMicrotask(()=>value.dirtyHandler());}}});}
            instrument(state);Object.defineProperty(window,'__ferric_browserHintState',{configurable:true,
                get(){return state},set(value){state=value;instrument(value)}});return true;
        })())JS").arg(journey=="seal-stale-once"?1:2),1).toBool());
    }
    evaluate(workspace+".startCapture('all')");
    auto *capture=window->findChild<QObject *>("sitePolicyCapture");QVERIFY(capture);
    engine->rootContext()->setContextProperty("testCapture",capture);
    if(journey=="seal-stale-twice") {
        QTRY_COMPARE_WITH_TIMEOUT(capture->property("notice").toString(),QString("interaction-collection-stale"),20000);
        QCOMPARE(page("p15SealDirtyCount",1).toInt(),2);
        QVERIFY(!evaluate(runtime+".captureReady").toBool());
        QCOMPARE(evaluate(runtime+".requestId").toString(),QString());
        QVERIFY(!evaluate("!!"+builder+".capturedFacts").toBool());
        QCOMPARE(page("captureEvents.click+captureEvents.key").toInt(),0);return;
    }
    QTRY_VERIFY2_WITH_TIMEOUT(evaluate(runtime+".captureReady && testCapture.rows.length>0").toBool(),qPrintable(capture->property("notice").toString()+" / "+text("site_policy_result")),20000);
    if(journey=="seal-stale-once") {
        QCOMPARE(page("p15SealDirtyCount",1).toInt(),1);
        QVERIFY(!evaluate("!!"+builder+".capturedFacts").toBool());
        QCOMPARE(page("captureEvents.click+captureEvents.key").toInt(),0);
        QVERIFY(QMetaObject::invokeMethod(capture,"close",Qt::DirectConnection));return;
    }
    const auto originalCapture=evaluate(runtime+".nativeRequest");
    if(journey=="modal") {
        QVERIFY(command("settings"));QTRY_VERIFY(evaluate("testWindow.settingsVisible").toBool());
        QTRY_VERIFY(!capture->property("opened").toBool());
        auto *focused=window->activeFocusItem();QVERIFY(focused);
        QVERIFY(QMetaObject::invokeMethod(capture,"begin",Qt::DirectConnection));
        QVERIFY(!capture->property("opened").toBool());QCOMPARE(window->activeFocusItem(),focused);
        QCOMPARE(evaluate(runtime+".requestId").toString(),QString());
        QVERIFY(!evaluate("!!"+builder+".capturedFacts").toBool());return;
    }
    if(journey=="prompt") {
        QVERIFY(page("setTimeout(()=>{window.capturePromptAnswer=prompt('Capture prompt','')},0);true").toBool());
        QTRY_VERIFY(evaluate("testWindow.pendingPageDialogRequest!==null").toBool());
        QTRY_VERIFY(!capture->property("opened").toBool());
        QTRY_COMPARE(evaluate(runtime+".requestId").toString(),QString());
        auto *input=window->findChild<QQuickItem *>("pageDialogInput");QVERIFY(input);QTRY_VERIFY(input->hasActiveFocus());
        QVERIFY(QMetaObject::invokeMethod(capture,"begin",Qt::DirectConnection));
        QVERIFY(!capture->property("opened").toBool());QVERIFY(input->hasActiveFocus());
        typeText("native prompt");QCOMPARE(input->property("text").toString(),QString("native prompt"));
        auto *accept=window->findChild<QObject *>("pageDialogAccept");QVERIFY(accept);
        QVERIFY(QMetaObject::invokeMethod(accept,"click",Qt::DirectConnection));
        QTRY_VERIFY(evaluate("testWindow.pendingPageDialogRequest===null").toBool());
        QTRY_COMPARE(page("capturePromptAnswer").toString(),QString("native prompt"));
        QVERIFY(!evaluate("testWindow.sitePoliciesVisible").toBool());
        QVERIFY(!evaluate("!!"+builder+".capturedFacts").toBool());return;
    }
    QVERIFY(!evaluate("testWindow.sitePoliciesVisible").toBool());
    QVERIFY(capture->property("visible").toBool());
    QVERIFY(!evaluate("testWindow.browserUi.hint_visible").toBool());
    if(journey=="window-close") {
        QPointer<QObject> owner=evaluate(runtime).value<QObject *>();
        engine.reset();window=nullptr;QVERIFY(owner.isNull());
        QCoreApplication::sendPostedEvents(nullptr,QEvent::DeferredDelete);QTest::qWait(100);return;
    }
    const auto focusBaseline=page("captureEvents.focus").toInt();
    if(journey=="anchor"||journey=="input-close") {
        const auto point=page("(()=>{const r=document.getElementById('first').getBoundingClientRect();return {x:r.right-2,y:r.top+r.height/2}})()").toMap();
        auto *view=qobject_cast<QQuickItem *>(evaluate("testWindow.activeWebView()").value<QObject *>());QVERIFY(view);
        QTest::mouseClick(window,Qt::LeftButton,Qt::NoModifier,view->mapToScene(QPointF(point.value("x").toDouble(),point.value("y").toDouble())).toPoint());
        QInputMethodEvent commit;commit.setCommitString("unwanted-commit");
        QCoreApplication::sendEvent(QGuiApplication::focusObject(),&commit);
        QCOMPARE(page("document.querySelector('input:not([type=password])').value").toString(),QString("private-text"));
        key(Qt::Key_C,Qt::ControlModifier);key(Qt::Key_X,Qt::ControlModifier);key(Qt::Key_V,Qt::ControlModifier);key(Qt::Key_V,Qt::ControlModifier|Qt::ShiftModifier);
        QCOMPARE(page("captureEvents.click+captureEvents.key").toInt(),0);
        QCOMPARE(page("captureEvents.focus").toInt(),focusBaseline);
        QVERIFY(page("document.activeElement===captureOriginalFocus").toBool());
        if(journey=="input-close"){
            QPointer<QObject> owner=evaluate(runtime).value<QObject *>();engine.reset();window=nullptr;QVERIFY(owner.isNull());
            QCoreApplication::sendPostedEvents(nullptr,QEvent::DeferredDelete);QTest::qWait(100);return;
        }
        const auto firstLabel=evaluate("testCapture.rows[0].label").toString();
        key(Qt::Key_Tab);
        QCOMPARE(evaluate("testCapture.labelText").toString(),evaluate("testCapture.rows[1].label").toString());
        key(Qt::Key_Backtab,Qt::ShiftModifier);
        QCOMPARE(evaluate("testCapture.labelText").toString(),firstLabel);
        for(int index=0;index<firstLabel.size();++index)key(Qt::Key_Backspace);
        QCOMPARE(evaluate("testCapture.labelText").toString(),QString());
        const auto artifacts=qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
        if(!artifacts.isEmpty()){QVERIFY(QDir().mkpath(artifacts));QVERIFY(window->grabWindow().save(artifacts+"/p12-capture-overlay.png"));}
    }
    QVERIFY(!evaluate("JSON.stringify(testCapture.rows)").toString().contains("private-password"));
    QVERIFY(!evaluate("JSON.stringify(testCapture.rows)").toString().contains("private-text"));
    const auto title=frameCase?"Frame primary":journey=="shadow"?"Shadow primary":journey=="slot"||journey=="slot-collection"?"Slot primary":"First anchor";
    const auto label=evaluate("(testCapture.rows.find(function(row){return row.candidate.text==="+quoted(title)+"})||{}).label||''").toString();QVERIFY2(!label.isEmpty(),qPrintable(evaluate("JSON.stringify(testCapture.rows)").toString()));
    QVERIFY(evaluate("JSON.parse(testWindow.browserUi.site_policy_draft_snapshot("+quoted(draft)+")).preview === null").toBool());
    if(journey=="closed")evaluate("testCapture.close()");
    else if(journey=="mutated")QVERIFY(page("document.getElementById('first').remove();true").toBool());
    else if(journey=="zoom")evaluate("testWindow.activeWebView().zoomFactor=1.25");
    else if(journey=="navigation"||journey=="history")QVERIFY(command("open "+quoted(QString("http://127.0.0.1:%1/replacement").arg(server.serverPort()))));
    else {
        QVERIFY(evaluate("testWindow.activeWebView().activeFocus").toBool());typeText(label);key(Qt::Key_Return);
    }
    if(journey=="closed"||journey=="mutated"||journey=="zoom"||journey=="navigation"||journey=="history") {
        QTRY_COMPARE_WITH_TIMEOUT(evaluate(runtime+".requestId").toString(),QString(),10000);
        QVERIFY(!evaluate("!!"+builder+".capturedFacts").toBool());
        if(journey!="closed")QVERIFY(!capture->property("notice").toString().isEmpty());
        if(journey=="history") {
            QTRY_VERIFY(evaluate("!testWindow.activeWebView().loading && testWindow.activeWebView().canGoBack").toBool());
            evaluate("testWindow.activeWebView().goBack()");
            QTRY_COMPARE(page("location.href").toString(),originalUrl);
            QTRY_VERIFY(page("window.captureRestored===true && window.captureDocument==='same'").toBool());
            QVERIFY(!evaluate(runtime+".captureReady").toBool());
            auto *adapter=evaluate(runtime).value<QObject *>()->findChild<FerricHintRequestAdapter *>();QVERIFY(adapter);
            QVERIFY(QJsonDocument::fromJson(adapter->builderCandidate(originalCapture,label).toUtf8()).object().contains("error"));
            QVERIFY(evaluate("(()=>{var result=null;"+runtime+".chooseCapture("+quoted(label)+",function(value){result=value});return !!result && !!result.error})()").toBool());
            if(capture->property("opened").toBool()) {
                QVERIFY(window->findChild<FerricBrowserKeyRouter *>()->property("enabled").toBool());
                typeText(label);key(Qt::Key_Return);
            } else QVERIFY(!capture->property("inputAvailable").toBool());
            QVERIFY(!evaluate("!!"+builder+".capturedFacts").toBool());
            QCOMPARE(page("captureEvents.click+captureEvents.key").toInt(),0);
            QCOMPARE(page("JSON.stringify(captureKeyDetails)").toString(),QString("[]"));
        }
    } else {
        QTRY_VERIFY2_WITH_TIMEOUT(evaluate("!!"+builder+".capturedFacts").toBool(),qPrintable(capture->property("notice").toString()+" / "+evaluate(workspace+".notice").toString()),8000);
        QTRY_VERIFY(evaluate("testWindow.sitePoliciesVisible").toBool());
        QVERIFY(!capture->property("visible").toBool());
        QVERIFY(!evaluate(builder+".capturedFacts.activation_permitted").toBool());
        if(journey=="samples") {
            auto *keep=surface->findChild<QObject *>("policyKeepSample");QVERIFY(keep);QVERIFY(QMetaObject::invokeMethod(keep,"click",Qt::DirectConnection));
            QCOMPARE(evaluate(builder+".keptSamples.length").toInt(),1);
            evaluate(workspace+".close()");
            frameFixtures["/samples-second.html?token=secret"]="<!doctype html><a id=first href=/never>First anchor</a><a id=first href=/never>Another anchor</a><script>window.captureEvents={click:0,focus:0,key:0};for(const type of ['click','focusin','keydown','keyup'])document.addEventListener(type,()=>captureEvents[type==='click'?'click':type==='focusin'?'focus':'key']++);</script>";
            const auto next=QString("http://127.0.0.1:%1/samples-second.html?token=secret#fragment").arg(server.serverPort());
            QVERIFY(command("open "+quoted(next)));QTRY_VERIFY(evaluate("testWindow.activeWebView().url.toString() === "+quoted(next)+" && !testWindow.activeWebView().loading").toBool());
            QCOMPARE(evaluate(builder+".keptSamples.length").toInt(),1);
            QVERIFY(command("site-policy"));QTRY_VERIFY(evaluate("testWindow.sitePoliciesVisible").toBool());
            evaluate(workspace+".startCapture('all')");QTRY_VERIFY_WITH_TIMEOUT(evaluate(runtime+".captureReady").toBool(),20000);
            const auto second=evaluate("(testCapture.rows.find(function(row){return row.candidate.text==='First anchor'})||{}).label||''").toString();QVERIFY2(!second.isEmpty(),qPrintable(evaluate("JSON.stringify(testCapture.rows)").toString()));
            typeText(second);key(Qt::Key_Return);
            QTRY_VERIFY(evaluate("testWindow.sitePoliciesVisible && "+builder+".capturedFacts.location.indexOf('/samples-second.html')>=0").toBool());
            const auto comparison=surface->findChild<QObject *>("policySampleComparison");QVERIFY(comparison);
            QVERIFY(comparison->property("text").toString().contains("kept 1; current 2"));
            QCOMPARE(evaluate(builder+".keptSamples.length").toInt(),1);
            QVERIFY(!evaluate("JSON.stringify("+builder+".capturedFacts)").toString().contains("secret"));
            QVERIFY(evaluate("!"+workspace+".selected.proposals[0].interaction.anchors").toBool());
            QCOMPARE(page("JSON.stringify(captureEvents)").toString(),QString("{\"click\":0,\"focus\":0,\"key\":0}"));
            QCOMPARE(evaluate(runtime+".requestId").toString(),QString());return;
        }
        if(frameCase) {
            QCOMPARE(evaluate(builder+".capturedFacts.root.start").toString(),journey=="focused-frame"?QString("focused-frame"):QString("top-document"));
            if(journey=="focused-frame")QCOMPARE(evaluate(builder+".capturedFacts.root.receiver_origin").toString(),QString("http://localhost:%1").arg(server.serverPort()));
            else QCOMPARE(evaluate(builder+".capturedFacts.root.steps[0].kind").toString(),QString("same-origin-frame"));
        }
        QVERIFY(evaluate("JSON.parse(testWindow.browserUi.site_policy_draft_snapshot("+quoted(draft)+")).preview === null").toBool());
        evaluate(builder+".seed.id='captured';"+builder+".seed.families=['all'];"+(collection?"":builder+".seed.code='m';"));
        auto *chooser=surface->findChild<QObject *>("policyUseCapturedFacts");QVERIFY(chooser);
        const auto select=[&](const char *name,int index){auto *control=surface->findChild<QObject *>(QString::fromLatin1(name));return control&&control->setProperty("currentIndex",index);};
        if(collection) {
            const bool slotted=journey=="slot-collection";
            if(slotted){QVERIFY(select("policyCapturedRoot",1));auto *self=surface->findChild<QObject *>("policyCapturedRootContainer");QVERIFY(self);QVERIFY(self->setProperty("checked",true));}
            const auto itemTag=journey=="shadow"?"x-card":"article",containerId=journey=="shadow"?"#shadow-list":"#list";
            const auto itemIndex=evaluate(builder+".capturedFacts.ancestors.findIndex(function(value){return value.tag==="+quoted(itemTag)+"})").toInt();
            const auto containerIndex=evaluate(builder+".capturedFacts.ancestors.findIndex(function(value){return value.selectors.some(function(s){return s.selector==="+quoted(containerId)+"})})").toInt();
            QVERIFY(itemIndex>=0);QVERIFY(select("policyCapturedItem",itemIndex));
            if(!slotted){QVERIFY(containerIndex>itemIndex);QVERIFY(select("policyCapturedContainer",containerIndex));}
            const auto itemSelector=journey=="shadow"?"x-card":"article.card",primary=journey=="shadow"?"button":"a";
            QVERIFY(select("policyCapturedItemSelector",evaluate(builder+".capturedFacts.ancestors["+QString::number(itemIndex)+"].selectors.findIndex(function(s){return s.selector==="+quoted(itemSelector)+"})").toInt()));
            if(!slotted)QVERIFY(select("policyCapturedContainerSelector",evaluate(builder+".capturedFacts.ancestors["+QString::number(containerIndex)+"].selectors.findIndex(function(s){return s.selector==="+quoted(containerId)+"})").toInt()));
            QVERIFY(select("policyCapturedPrimary",evaluate(builder+".capturedFacts.ancestors["+QString::number(itemIndex)+"].primary.suggestions.findIndex(function(s){return s.selector==="+quoted(primary)+"})").toInt()));
        } else {
            if(journey=="slot")QVERIFY(select("policyCapturedRoot",1));
            QVERIFY(select("policyCapturedSelector",0));
        }
        QVERIFY(QMetaObject::invokeMethod(chooser,"click",Qt::DirectConnection));
        if(journey=="slot-collection"){QVERIFY(evaluate(builder+".seed.container_self").toBool());QCOMPARE(evaluate(builder+".seed.root.steps[1].kind").toString(),QString("assigned-slot"));}
        if(collection)QCOMPARE(evaluate(builder+".seed.primary_root_steps.length").toInt(),journey=="shadow"?1:0);
        evaluate(builder+".add()");
        evaluate(workspace+".selected.proposals[0].interaction.enabled=true;"+workspace+".selected.proposals[0].interaction.labels={auto_follow:'never',fallback_prefix:'z'};"+workspace+".selected.editEpoch++");
        const auto activeTab=evaluate("testWindow.activeWebView().stableTabId").toString();
        evaluate(workspace+".submit("+workspace+".selected,{action:'preview',tab:"+quoted(activeTab)+",pageFocus:"+(journey=="focused-frame"?"true":"false")+"})");
        const auto receipt=[&]{return QJsonDocument::fromJson(evaluate("testWindow.browserUi.site_policy_draft_snapshot("+quoted(draft)+")").toString().toUtf8()).object().value("preview").toObject();};
        QTRY_VERIFY_WITH_TIMEOUT(receipt().value("observations").toObject().value("complete").toBool(),20000);
        const auto invocations=receipt().value("observations").toObject().value("invocations").toArray();
        const auto reservations=invocations[0].toObject().value("result").toObject().value("reservations").toArray();
        QCOMPARE(reservations.size(),collection?2:1);for(const auto slot:reservations)QCOMPARE(slot.toObject().value("state").toString(),QString("Live"));
    }
    if(journey!="navigation") {
        QCOMPARE(page("captureEvents.click+captureEvents.key").toInt(),0);
        QCOMPARE(page("captureEvents.focus").toInt(),focusBaseline);
        if(journey=="anchor")QVERIFY(page("document.activeElement===captureOriginalFocus").toBool());
        QCOMPARE(page("captureDocument").toString(),QString("same"));
        QTRY_VERIFY(page("!window.__ferric_browserHintState || !window.__ferric_browserHintState.tracking",1).toBool());
        if(frameCase)QCOMPARE(frameRead("JSON.stringify(captureEvents)").toString(),QString("{\"click\":0,\"focus\":0,\"key\":0}"));
    }
    QCOMPARE(text("mode"),QString("normal"));
}

void ApplicationTests::sitePolicyBuilderStart()
{
    QVERIFY(page("window.builderEvents={key:0,click:0};document.addEventListener('keydown',()=>++builderEvents.key);document.addEventListener('click',()=>++builderEvents.click);true").toBool());
    QVERIFY(command("site-policy"));
    QTRY_VERIFY(evaluate("testWindow.sitePoliciesVisible").toBool());
    const auto workspace=QStringLiteral("testWindow.runtime_policyWorkspace"),builder=workspace+".guidedBuilder";
    auto *surface=evaluate(workspace).value<QObject *>();QVERIFY(surface);
    auto *tab=surface->findChild<QObject *>("policyBuilderTab");QVERIFY(tab);
    QVERIFY(QMetaObject::invokeMethod(tab,"click",Qt::DirectConnection));
    evaluate(builder+".begin('policy')");
    QTRY_VERIFY(!evaluate(builder+".scope.origin").toString().isEmpty());
    const auto origin=QString("http://127.0.0.1:%1").arg(server.serverPort());
    QCOMPARE(evaluate(builder+".scope.origin").toString(),origin);
    QCOMPARE(evaluate(builder+".scope.exact_pattern").toString(),origin+"/fixture.html");
    const QJsonObject scopeRequest{{"action","scope"},{"url","HTTPS://user:secret@EXAMPLE.test.:443/a?token=secret#hidden"},
        {"pattern","https://example.test/a"},{"samples",QJsonArray{"https://example.test/a?other=1","https://sub.example.test/a","https://example.test:8443/a"}}};
    const auto token=evaluate("testWindow.browserUi.site_policy_editor_request("+quoted(QString::fromUtf8(QJsonDocument(scopeRequest).toJson(QJsonDocument::Compact)))+")").toString();
    QVERIFY(!token.isEmpty());
    QTRY_COMPARE(QJsonDocument::fromJson(text("site_policy_result").toUtf8()).object().value("request").toString(),token);
    const auto checkedScope=QJsonDocument::fromJson(text("site_policy_result").toUtf8()).object().value("result").toObject();
    QCOMPARE(checkedScope.value("origin").toString(),QString("https://example.test"));
    QVERIFY(checkedScope.value("samples").toArray()[0].toObject().value("matches").toBool());
    QVERIFY(!checkedScope.value("samples").toArray()[1].toObject().value("matches").toBool());
    QVERIFY(!checkedScope.value("samples").toArray()[2].toObject().value("matches").toBool());
    QVERIFY(!QJsonDocument(checkedScope).toJson().contains("secret"));
    evaluate(builder+".create()");
    QVERIFY(!evaluate(workspace+".selected !== null").toBool());
    const auto click=[&](const QString &name) {
        auto *control=surface->findChild<QObject *>(name);
        return control&&QMetaObject::invokeMethod(control,"click",Qt::DirectConnection);
    };
    auto *scope=surface->findChild<QObject *>("policyBuilderScope");QVERIFY(scope);
    QVERIFY(scope->setProperty("currentIndex",0));
    QVERIFY(QMetaObject::invokeMethod(scope,"activated",Q_ARG(int,0)));
    auto *id=qobject_cast<QQuickItem *>(surface->findChild<QObject *>("policyBuilderId"));QVERIFY(id);
    id->forceActiveFocus();typeText("builder-native");
    QVERIFY(click("policyBuilderCreate"));
    QTRY_VERIFY(evaluate(workspace+".selected !== null").toBool());
    QVERIFY(!evaluate(workspace+".selected.proposals[0].interaction.enabled").toBool());
    const auto draft=evaluate(workspace+".selected.view.id").toString();QVERIFY(!draft.isEmpty());
    // Capture consumes the actual native shortcut path, including Ctrl+Q.
    evaluate(builder+".begin('remap')");
    QVERIFY(QMetaObject::invokeMethod(tab,"click",Qt::DirectConnection));
    QTRY_VERIFY(!evaluate(builder+".scope.origin").toString().isEmpty());
    QVERIFY(click("policyCaptureSourceKey"));key(Qt::Key_Q,Qt::ControlModifier);
    QTRY_COMPARE(evaluate(builder+".canonicalFrom").toString(),QString("Ctrl+q"));
    QVERIFY(evaluate("testWindow.sitePoliciesVisible").toBool());
    QVERIFY(click("policyCaptureSourceKey"));key(Qt::Key_P);
    QTRY_COMPARE(evaluate(builder+".canonicalFrom").toString(),QString("p"));
    QVERIFY(click("policyCaptureDestinationKey"));key(Qt::Key_K);
    QTRY_COMPARE(evaluate(builder+".canonicalTo").toString(),QString("k"));
    const auto report=QJsonDocument::fromJson(evaluate(builder+".keyReport").toString().toUtf8()).object();
    QCOMPARE(report.value("qualifications").toArray().size(),12);
    const auto artifacts=qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
    if(!artifacts.isEmpty()) {
        QVERIFY(QDir().mkpath(artifacts));
        QVERIFY(window->grabWindow().save(artifacts+"/p12-key-builder.png"));
    }
    QVERIFY(!report.value("activation_permitted").toBool());
    auto *mapping=qobject_cast<QQuickItem *>(surface->findChild<QObject *>("policyBuilderMappingId"));QVERIFY(mapping);
    mapping->forceActiveFocus();typeText("play");
    QVERIFY(click("policyBuilderAdmitHint"));
    QVERIFY(click("policyBuilderAdd"));
    QCOMPARE(evaluate(workspace+".selected.proposals[0].interaction.remaps[0].from").toString(),QString("p"));
    QCOMPARE(evaluate(workspace+".selected.proposals[0].interaction.remaps[0].to").toString(),QString("k"));
    QVERIFY(!evaluate(workspace+".selected.proposals[0].interaction.enabled").toBool());
    evaluate(workspace+".submit("+workspace+".selected,{action:'validate'})");
    QTRY_VERIFY(!evaluate(workspace+".selected.pending").toBool());
    QTRY_VERIFY(evaluate(workspace+".selected.view.validation !== null").toBool());
    QVERIFY2(evaluate(workspace+".selected.view.validation.valid").toBool(),qPrintable(evaluate(workspace+".selected.view.validation.message").toString()));
    QCOMPARE(page("JSON.stringify(window.builderEvents)").toString(),QString("{\"key\":0,\"click\":0}"));
    QVERIFY(evaluate("JSON.parse(testWindow.browserUi.site_policy_draft_snapshot("+quoted(draft)+")).operation === null").toBool());
}

void ApplicationTests::sitePolicyBuilderPageTest_data()
{
    QTest::addColumn<QString>("context");
    for(const auto *context:{"passthrough-once","passthrough","normal-page-key","hint-page-key","unapplied","editable","stale"})
        QTest::newRow(context)<<QString::fromLatin1(context);
}

void ApplicationTests::sitePolicyBuilderKeyboard_data()
{
    QTest::addColumn<QString>("journey");
    QTest::addColumn<int>("viewportWidth");
    QTest::addColumn<int>("fontPointSize");
    QTest::addColumn<bool>("reducedMotion");
    for(const auto *name:{"navigation","list","grid","sidebar","shadow-card","p-to-k","failed-save"})
        QTest::newRow(name)<<QString::fromLatin1(name)<<1000<<0<<false;
    QTest::newRow("navigation-narrow-large-reduced-motion")<<QString("navigation")<<520<<18<<true;
}

void ApplicationTests::sitePolicyBuilderKeyboard()
{
    QFETCH(QString,journey);
    QFETCH(int,viewportWidth);QFETCH(int,fontPointSize);QFETCH(bool,reducedMotion);
    if(fontPointSize>0){QVERIFY(command(QString("set --temp ui.font_size_pt=%1").arg(fontPointSize)));QTRY_COMPARE(evaluate("testWindow.chromeFontPointSize").toInt(),fontPointSize);}
    if(reducedMotion){QVERIFY(command("set --temp ui.reduced_motion=on"));QTRY_VERIFY(evaluate("testWindow.reducedMotionActive").toBool());}
    window->setWidth(viewportWidth);window->setHeight(820);
    const bool mapping=journey=="p-to-k",collection=journey!="navigation"&&journey!="failed-save"&&!mapping,shadow=journey=="shadow-card";
    QVERIFY(page(R"JS((()=>{
        document.body.innerHTML='<nav id="navigation"><a id="home" href="/never">Home</a><a href="/never">Store</a></nav><section id="cards"><article class="card" data-id="one"><a href="/never">First primary</a></article><article class="card" data-id="two"><a href="/never">Second primary</a></article></section><section id="shadow-cards"><x-card></x-card><x-card></x-card></section>';
        for(const host of document.querySelectorAll('x-card'))host.attachShadow({mode:'open'}).innerHTML='<button>Shadow primary</button>';
        window.authorEvents={click:0,key:0};document.addEventListener('click',()=>authorEvents.click++);document.addEventListener('keydown',()=>authorEvents.key++);window.authorKeys=[];document.addEventListener('keydown',e=>authorKeys.push({key:e.key,trusted:e.isTrusted}));return true;
    })())JS").toBool());
    if(journey=="grid")QVERIFY(page("document.getElementById('cards').style='display:grid;grid-template-columns:200px 200px;gap:20px';true").toBool());
    if(journey=="sidebar")QVERIFY(page("document.getElementById('cards').style='position:fixed;right:10px;top:30px;width:220px';true").toBool());
    key(Qt::Key_Colon);QTRY_COMPARE(text("mode"),QString("command"));typeText("site-policy");key(Qt::Key_Return);
    QTRY_VERIFY(evaluate("testWindow.sitePoliciesVisible").toBool());
    const auto workspace=QStringLiteral("testWindow.runtime_policyWorkspace"),builder=workspace+".guidedBuilder";
    auto *surface=evaluate(workspace).value<QObject *>();QVERIFY(surface);
    const auto within=[&](QQuickItem *target){for(auto *item=window->activeFocusItem();item;item=item->parentItem())if(item==target)return true;return false;};
    const std::function<QQuickItem *(QQuickItem *,const QString &)> find=[&](QQuickItem *item,const QString &name)->QQuickItem * {
        if(item->objectName()==name)return item;
        for(auto *child:item->childItems())if(auto *found=find(child,name))return found;
        return nullptr;
    };
    const auto tabTo=[&](const QString &name) {
        QQuickItem *target=nullptr;QElapsedTimer ready;ready.start();
        do{target=find(qobject_cast<QQuickItem *>(surface),name);if(!target)QTest::qWait(5);}while(!target&&ready.elapsed()<1000);
        if(!target){qWarning()<<"Missing keyboard control"<<name;return false;}
        QStringList visited;
        for(int step=0;step<500;++step){if(within(target))return true;if(step<45){auto *focus=window->activeFocusItem();visited<<(!focus?QString("null"):focus->objectName()+":"+focus->metaObject()->className());}QTest::keyClick(window,Qt::Key_Tab);QCoreApplication::processEvents(QEventLoop::AllEvents,5);}
        qWarning()<<"Keyboard control unreachable"<<name<<window->activeFocusItem()<<"target"<<target->isVisible()<<target->isEnabled()<<target->property("focusPolicy")<<target->property("activeFocusOnTab");
        qWarning()<<"TAB_VISITED"<<visited;
        QStringList chain;auto *item=target;for(int i=0;i<35;++i){chain<<item->objectName()+":"+item->metaObject()->className();item=item->nextItemInFocusChain(true);if(!item||item==target)break;}qWarning()<<"TAB_CHAIN"<<chain<<target->width()<<target->height();
        qWarning()<<evaluate("JSON.stringify({visible:testWindow.sitePoliciesVisible,builder:testWindow.runtime_policyWorkspace.guidedBuilder.kind,interactive:testWindow.runtime_policyWorkspace.interactive,capture:testWindow.sitePolicyPreviewWorkspace.captureReady,tab:testWindow.runtime_policyWorkspace.guidedBuilder.visible})").toString();return false;
    };
    const auto click=[&](const QString &name){if(!tabTo(name))return false;key(Qt::Key_Space);return true;};
    const auto enterText=[&](const QString &name,const QString &value){if(!tabTo(name))return false;key(Qt::Key_A,Qt::ControlModifier);typeText(value);return true;};
    const auto select=[&](const QString &name,int index){if(index<0||!tabTo(name))return false;key(Qt::Key_Space);key(Qt::Key_Home);for(int i=0;i<index;++i)key(Qt::Key_Down);key(Qt::Key_Return);return true;};
    const auto section=[&](int index){
        for(int step=0;step<500;++step){auto *item=window->activeFocusItem();if(item&&item->inherits("QQuickTabButton")){key(Qt::Key_Home);for(int i=0;i<index;++i)key(Qt::Key_Right);return true;}QTest::keyClick(window,Qt::Key_Tab);QCoreApplication::processEvents(QEventLoop::AllEvents,5);}return false;
    };
    QVERIFY(section(4));QVERIFY(click("policyBuildNew"));
    QTRY_VERIFY(!evaluate(builder+".scope.origin").toString().isEmpty());
    QVERIFY(select("policyBuilderScope",0));QVERIFY(enterText("policyBuilderId","keyboard-"+journey));
    if(journey=="failed-save")QVERIFY(select("policyBuilderDestination",1));
    QVERIFY(click("policyBuilderCreate"));QTRY_VERIFY(evaluate(workspace+".selected !== null").toBool());
    const auto draft=evaluate(workspace+".selected.view.id").toString();
    QVERIFY(section(4));QVERIFY(click(mapping?"policyBuildMapping":collection?"policyBuildCollection":"policyBuildAnchor"));
    if(mapping) {
        QVERIFY(enterText("policyBuilderMappingId","play"));QVERIFY(click("policyCaptureSourceKey"));key(Qt::Key_P);
        QTRY_COMPARE(evaluate(builder+".canonicalFrom").toString(),QString("p"));
        QVERIFY(click("policyCaptureDestinationKey"));key(Qt::Key_K);QTRY_COMPARE(evaluate(builder+".canonicalTo").toString(),QString("k"));
        QVERIFY(click("policyBuilderAdmitHint"));
    } else {
        QVERIFY(enterText("policyField:builder.id","primary"));
        if(!collection)QVERIFY(enterText("policyField:builder.code","m"));
        QVERIFY(click("policyExpand:builder.families"));QVERIFY(select("policyField:builder.families.0",0));
        QVERIFY(click("policyBuilderCapture"));
        auto *capture=window->findChild<QObject *>("sitePolicyCapture");QVERIFY(capture);
        engine->rootContext()->setContextProperty("testCapture",capture);
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.sitePolicyPreviewWorkspace.captureReady").toBool(),20000);
        const auto name=shadow?"Shadow primary":collection?"First primary":"Home";
        const auto label=evaluate("(testCapture.rows.find(function(row){return row.candidate.text==="+quoted(name)+"})||{}).label||''").toString();QVERIFY(!label.isEmpty());
        typeText(label);key(Qt::Key_Return);QTRY_VERIFY(evaluate("testWindow.sitePoliciesVisible").toBool());
        QTRY_VERIFY(evaluate("!!"+builder+".capturedFacts").toBool());
        if(collection) {
            const auto item=evaluate(builder+".capturedFacts.ancestors.findIndex(function(value){return value.tag==="+quoted(shadow?"x-card":"article")+"})").toInt();
            const auto container=evaluate(builder+".capturedFacts.ancestors.findIndex(function(value){return value.selectors.some(function(s){return s.selector==="+quoted(shadow?"#shadow-cards":"#cards")+"})})").toInt();
            QVERIFY(select("policyCapturedItem",item));QVERIFY(select("policyCapturedContainer",container));
            const auto itemSelector=evaluate(builder+".capturedFacts.ancestors["+QString::number(item)+"].selectors.findIndex(function(s){return s.selector==="+quoted(shadow?"x-card":"article.card")+"})").toInt();
            QVERIFY(select("policyCapturedItemSelector",itemSelector));QVERIFY(select("policyCapturedContainerSelector",0));
            const auto primary=evaluate(builder+".capturedFacts.ancestors["+QString::number(item)+"].primary.suggestions.findIndex(function(s){return s.selector==="+quoted(shadow?"button":"a")+"})").toInt();
            QVERIFY(select("policyCapturedPrimary",primary));
        } else QVERIFY(select("policyCapturedSelector",0));
        QVERIFY(click("policyUseCapturedFacts"));
        if(collection)QVERIFY(click("policyGenerateCodes"));
    }
    QVERIFY(click("policyBuilderAdd"));
    QVERIFY(click("policyExpand:rules.0.interaction"));
    QVERIFY(click("policyField:rules.0.interaction.enabled"));
    QVERIFY(click("policyInclude:rules.0.interaction.labels"));QVERIFY(click("policyExpand:rules.0.interaction.labels"));
    QVERIFY(click("policyInclude:rules.0.interaction.labels.auto_follow"));QVERIFY(select("policyField:rules.0.interaction.labels.auto_follow",3));
    QVERIFY(click("policyInclude:rules.0.interaction.labels.fallback_prefix"));QVERIFY(enterText("policyField:rules.0.interaction.labels.fallback_prefix","z"));
    QVERIFY(click("policyValidate"));QTRY_VERIFY(!evaluate(workspace+".selected.pending").toBool());
    QTRY_VERIFY(evaluate(workspace+".selected.view.validation !== null").toBool());
    QVERIFY2(evaluate(workspace+".selected.view.validation.valid").toBool(),qPrintable(evaluate(workspace+".selected.view.validation.message").toString()));
    QVERIFY(click("policyLivePreview"));
    QTRY_VERIFY2_WITH_TIMEOUT(evaluate("!!"+workspace+".selected.view.preview && !!"+workspace+".selected.view.preview.observations && "+workspace+".selected.view.preview.observations.complete").toBool(),qPrintable(evaluate("JSON.stringify({notice:"+workspace+".notice,preview:"+workspace+".selected.view.preview})").toString()),20000);
    const auto reservations=QJsonDocument::fromJson(evaluate("JSON.stringify("+workspace+".selected.view.preview.observations.invocations[0].result.reservations)").toString().toUtf8()).array();
    QCOMPARE(reservations.size(),mapping?0:collection?3:1);
    for(int i=0;i<(mapping?0:collection?2:1);++i)QCOMPARE(reservations[i].toObject().value("state").toString(),QString("Live"));
    if(collection)QVERIFY(reservations[2].toObject().value("state").toString()!="Live");
    QCOMPARE(page("JSON.stringify(authorEvents)").toString(),QString("{\"click\":0,\"key\":0}"));
    QVERIFY(click("policyReview"));QVERIFY(click("policyActivate"));
    if(journey=="failed-save")QVERIFY(QDir().mkpath(evaluate("testWindow.storageBasePath").toString()+"/state/runtime-overrides.toml"));
    QVERIFY(click("policyApply"));
    const auto saved=[&]{return QJsonDocument::fromJson(evaluate("testWindow.browserUi.site_policy_draft_snapshot("+quoted(draft)+")").toString().toUtf8()).object();};
    QTRY_COMPARE_WITH_TIMEOUT(saved().value("operation").toObject().value("state").toString(),journey=="failed-save"?QString("failed"):QString("applied"),15000);
    QTRY_VERIFY(!evaluate(workspace+".busy("+workspace+".selected)").toBool());
    QCOMPARE(saved().value("dirty").toBool(),journey=="failed-save");
    QCOMPARE(evaluate(workspace+".selected.view.id").toString(),draft);
    if(mapping) {
        QVERIFY(select("policyTestContext",3));QVERIFY(click("policyTestOnPage"));
        QTRY_VERIFY_WITH_TIMEOUT(evaluate(workspace+".notice").toString().startsWith("Ready for a physical"),8000);
        key(Qt::Key_P);QTRY_COMPARE(page("JSON.stringify(authorKeys)").toString(),QString("[{\"key\":\"k\",\"trusted\":true}]"));
    } else {key(Qt::Key_Escape);QTRY_VERIFY(!evaluate("testWindow.sitePoliciesVisible").toBool());}
}

void ApplicationTests::sitePolicyBuilderPageTest()
{
    QFETCH(QString,context);
    QVERIFY(command("set --temp input.shortcut_policy=mixed"));
    QTRY_COMPARE(QJsonDocument::fromJson(text("config_json").toUtf8()).object().value("input").toObject().value("shortcut_policy").toString(),QString("mixed"));
    QVERIFY(command("set --temp \"input.page_keys=['p']\""));
    QTRY_COMPARE(evaluate("testWindow.browserUi.mixed_key_route('p')").toString(),QString("page"));
    QVERIFY(page("window.pageTestKeys=[];document.addEventListener('keydown',e=>pageTestKeys.push({key:e.key,trusted:e.isTrusted}));document.getElementById('button').focus();true").toBool());
    QVERIFY(command("site-policy"));QTRY_VERIFY(evaluate("testWindow.sitePoliciesVisible").toBool());
    const auto workspace=QStringLiteral("testWindow.runtime_policyWorkspace");
    evaluate(workspace+".openRule('temporary','page-test',true,"+quoted(QString("http://127.0.0.1:%1/*").arg(server.serverPort()))+")");
    QTRY_VERIFY(evaluate(workspace+".selected !== null").toBool());
    const auto target=(context=="unapplied"||context=="editable"||context=="stale")?QString("passthrough"):context;
    evaluate(workspace+".selected.proposals[0].interaction={enabled:true,hint_page_keys:['p'],labels:{auto_follow:'never'},remaps:[{id:'play',from:'p',to:'k',contexts:["+quoted(target)+"]}]};"+workspace+".selected.editEpoch++");
    evaluate(workspace+".submit("+workspace+".selected,null)");
    QTRY_VERIFY(!evaluate(workspace+".selected.pending").toBool());
    const auto draft=evaluate(workspace+".selected.view.id").toString();
    if(context!="unapplied") {
        evaluate(workspace+".review("+workspace+".selected);"+workspace+".submit("+workspace+".selected,{action:'apply',activate:true})");
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("(JSON.parse(testWindow.browserUi.site_policy_draft_snapshot("+quoted(draft)+")).operation||{}).active===true").toBool(),15000);
        QTRY_VERIFY(!evaluate(workspace+".busy("+workspace+".selected)").toBool());
    }
    if(context=="stale") {
        QVERIFY(command("set --temp content.zoom=1.1"));
        QTRY_COMPARE(QJsonDocument::fromJson(text("config_json").toUtf8()).object().value("content").toObject().value("zoom").toDouble(),1.1);
    }
    if(context=="editable")QVERIFY(page("document.getElementById('input').focus();true").toBool());
    auto *router=window->findChild<FerricBrowserKeyRouter *>();QVERIFY(router);
    QSignalSpy receiverFacts(router,&FerricBrowserKeyRouter::pageReceiverObserved);
    const auto trace=qScopeGuard([&]{if(QTest::currentTestFailed()){
        qInfo()<<"BUILDER_PAGE_TEST"<<evaluate("JSON.stringify({epoch:testWindow.browserUi.input_epoch,mode:testWindow.browserUi.mode,route:JSON.parse(testWindow.browserUi.page_key_route('p')),focus:String(testWindow.activeFocusItem),viewFocus:testWindow.activeWebView().activeFocus})").toString();
        for(int index=0;index<qMin(receiverFacts.size(),qsizetype(8));++index)qInfo()<<"BUILDER_RECEIVER"<<receiverFacts[index];
    }});
    evaluate(workspace+".startPageTest("+workspace+".selected,'play',"+quoted(target)+")");
    const bool refused=context=="unapplied"||context=="stale"||context=="editable";
    if(refused) {
        QTRY_VERIFY_WITH_TIMEOUT(evaluate(workspace+".notice").toString().contains(context=="editable"?"Page test unavailable":context=="unapplied"?"Apply the current":"active revision"),8000);
        QVERIFY(evaluate("testWindow.sitePoliciesVisible").toBool());
        QCOMPARE(page("pageTestKeys.length").toInt(),0);
        QCOMPARE(evaluate(workspace+".selected.view.id").toString(),draft);
        return;
    }
    QTRY_VERIFY2_WITH_TIMEOUT(evaluate(workspace+".notice").toString().startsWith("Ready for a physical"),qPrintable(evaluate(workspace+".notice").toString()),8000);
    QVERIFY(!evaluate("testWindow.sitePoliciesVisible").toBool());
    QCOMPARE(page("pageTestKeys.length").toInt(),0);
    key(Qt::Key_P);
    QTRY_COMPARE(page("JSON.stringify(pageTestKeys)").toString(),QString("[{\"key\":\"k\",\"trusted\":true}]"));
    if(context=="passthrough-once")QTRY_COMPARE(text("mode"),QString("normal"));
}

void ApplicationTests::sitePolicySourceCopies()
{
    QVERIFY(command("site-policy"));
    const auto workspace=QStringLiteral("testWindow.runtime_policyWorkspace");
    QTRY_VERIFY(evaluate("testWindow.sitePoliciesVisible").toBool());
    evaluate(workspace+".openRule('authored:0','mixed-copy',false,'')");
    QTRY_VERIFY2(evaluate(workspace+".selected !== null").toBool(),qPrintable(evaluate(workspace+".notice").toString()));
    QVERIFY(evaluate(workspace+".selected.view.read_only").toBool());
    const auto authoredId=evaluate(workspace+".selected.view.id").toString();
    evaluate(workspace+".submit("+workspace+".selected,{action:'copy',destination:'generated',new_id:null})");
    QTRY_COMPARE(evaluate(workspace+".selected.view.source").toString(),QString("generated"));
    QVERIFY(!evaluate(workspace+".selected.view.read_only").toBool());
    const auto generated=evaluate(workspace+".selected.view.id").toString();
    QVERIFY(generated!=authoredId);
    const auto copied=QJsonDocument::fromJson(evaluate(workspace+".selected.diff").toString().toUtf8()).object().value("rules").toArray()[0].toObject();
    QCOMPARE(copied.value("copied_from").toObject().value("set").toObject().value("content.zoom").toDouble(),1.25);
    QCOMPARE(copied.value("proposed").toObject().value("set").toObject().value("content.zoom").toDouble(),1.25);
    QCOMPARE(evaluate(workspace+".selected.reviewedEpoch").toInt(),-1);
    evaluate(workspace+".review("+workspace+".selected);"+workspace+".submit("+workspace+".selected,{action:'apply',activate:false})");
    QTRY_COMPARE(evaluate("(JSON.parse(testWindow.browserUi.site_policy_draft_snapshot("+quoted(generated)+")).operation || {}).state").toString(),QString("reload-required"));
    evaluate(workspace+".submit("+workspace+".selected,{action:'copy',destination:'temporary',new_id:null})");
    QTRY_COMPARE(evaluate(workspace+".selected.view.source").toString(),QString("temporary"));
    QCOMPARE(evaluate(workspace+".selected.view.rules[0].proposed.set['content.zoom']").toDouble(),1.25);
    // A deliberate new ID copies Interaction only; it cannot import ordinary settings.
    evaluate(workspace+".submit("+workspace+".selected,{action:'copy',destination:'temporary',new_id:'interaction-only-clone'})");
    QTRY_COMPARE(evaluate(workspace+".selected.view.rules[0].id").toString(),QString("interaction-only-clone"));
    QCOMPARE(evaluate("Object.keys("+workspace+".selected.view.rules[0].proposed.set).length").toInt(),0);
    QVERIFY(!evaluate(workspace+".selected.view.rules[0].proposed.interaction.enabled").toBool());
    QVERIFY(evaluate(workspace+".selected.view.operation === null").toBool());
    QVERIFY(evaluate(workspace+".selected.diff.indexOf('prospective_resolution') >= 0").toBool());
}

void ApplicationTests::sitePolicyIndependentWindows()
{
    QVERIFY(command("site-policy"));
    const auto primary=QStringLiteral("testWindow.runtime_policyWorkspace");
    QTRY_VERIFY(evaluate("testWindow.sitePoliciesVisible").toBool());
    QVERIFY(evaluate(primary+".openRule('temporary','primary-dirty',true,'https://primary.test/*');true").toBool());
    QTRY_VERIFY(evaluate(primary+".selected !== null").toBool());
    QVERIFY(evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:'rb://blank',windowProfileName:'policy-peer',windowProfileLabel:'Policy peer'}) !== null").toBool());
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
    const auto peer=QStringLiteral("testWindow.browserWindowRegistry[1]");
    QTRY_VERIFY(evaluate(peer+".host.profileBootstrapComplete && !"+peer+".ui.profile_bootstrap_pending").toBool());
    QVERIFY(evaluate(peer+".ui.execute_secondary_interactive_command('site-policy')").toBool());
    QTRY_VERIFY(evaluate(peer+".host.sitePoliciesVisible").toBool());
    QVERIFY(!evaluate(peer+".host.browserKeyFocusActive").toBool());
    const auto secondary=peer+".host.runtime_policyWorkspace";
    QVERIFY(evaluate(secondary+".openRule('temporary','secondary-dirty',true,'https://secondary.test/*');true").toBool());
    QTRY_VERIFY(evaluate(secondary+".selected !== null").toBool());
    const std::function<QQuickItem *(QQuickItem *,const QString &)> find=[&](QQuickItem *item,const QString &name)->QQuickItem * {
        if(item->objectName()==name)return item;
        for(auto *child:item->childItems())if(auto *found=find(child,name))return found;
        return nullptr;
    };
    auto *peerWindow=qobject_cast<QQuickWindow *>(evaluate(peer+".host").value<QObject *>());QVERIFY(peerWindow);
    QQuickItem *fields[2] {};
    const QString workspaces[2]={primary,secondary};
    QQuickWindow *hosts[2]={window,peerWindow};
    const QString patterns[2]={"https://primary-edited.test/*","https://secondary-edited.test/*"};
    for(int i=0;i<2;++i) {
        auto *workspace=evaluate(workspaces[i]).value<QObject *>();QVERIFY(workspace);
        auto *tab=workspace->findChild<QObject *>("policyEditorTab");QVERIFY(tab);
        QVERIFY(QMetaObject::invokeMethod(tab,"click",Qt::DirectConnection));
        QTRY_VERIFY(find(qobject_cast<QQuickItem *>(workspace),"policyField:rules.0.pattern"));
        fields[i]=find(qobject_cast<QQuickItem *>(workspace),"policyField:rules.0.pattern");
        hosts[i]->requestActivate();QTRY_VERIFY(hosts[i]->isActive());
        fields[i]->forceActiveFocus();QTRY_VERIFY(fields[i]->hasActiveFocus());
        QTest::keyClick(hosts[i],Qt::Key_A,Qt::ControlModifier);
        for(const auto character:patterns[i]) QTest::keyClick(hosts[i],Qt::Key(character.toUpper().unicode()),character.isUpper()?Qt::ShiftModifier:Qt::NoModifier);
        QCOMPARE(fields[i]->property("text").toString(),patterns[i]);
        QVERIFY(QMetaObject::invokeMethod(fields[i],"select",Q_ARG(int,8),Q_ARG(int,14)));
    }
    const auto primarySelection=fields[0]->property("selectedText").toString();
    const auto secondarySelection=fields[1]->property("selectedText").toString();
    QVERIFY(!primarySelection.isEmpty());QVERIFY(!secondarySelection.isEmpty());
    evaluate(primary+".catalogue=Object.assign({},"+primary+".catalogue,{catalogue:false});"
        +secondary+".catalogue=Object.assign({},"+secondary+".catalogue,{catalogue:false});"
        +primary+".refresh();"+secondary+".refresh()");
    QTRY_VERIFY(evaluate(primary+".catalogue.catalogue && "+secondary+".catalogue.catalogue").toBool());
    for(int i=0;i<2;++i) {
        QCOMPARE(fields[i]->property("text").toString(),patterns[i]);
        QCOMPARE(evaluate(workspaces[i]+".selected.proposals[0].pattern").toString(),patterns[i]);
        QVERIFY(fields[i]->property("canUndo").toBool());
        QVERIFY(evaluate(workspaces[i]+".selected.editEpoch !== "+workspaces[i]+".selected.storedEpoch").toBool());
    }
    QCOMPARE(fields[0]->property("selectedText").toString(),primarySelection);
    QCOMPARE(fields[1]->property("selectedText").toString(),secondarySelection);
    // Secondary command, native workspace and preview all use the same owner.
    evaluate(secondary+".submit("+secondary+".selected,{action:'preview',tab:null})");
    QTRY_VERIFY(evaluate("!!"+secondary+".selected.view.preview").toBool());
    QCOMPARE(evaluate(secondary+".selected.view.preview.kind").toString(),QString("static-only"));
    QCOMPARE(fields[0]->property("text").toString(),patterns[0]);
    evaluate(secondary+".close()");
    QVERIFY(!evaluate(peer+".host.sitePoliciesVisible").toBool());
    QVERIFY(evaluate("testWindow.sitePoliciesVisible").toBool());
    QCOMPARE(fields[0]->property("selectedText").toString(),primarySelection);
}

void ApplicationTests::sitePolicyEditorReceipts_data()
{
    QTest::addColumn<QString>("outcome");
    for(const auto *outcome:{"write-rejected","uncertain","closed","profile-switch"})
        QTest::newRow(outcome)<<QString::fromLatin1(outcome);
}

void ApplicationTests::sitePolicyEditorReceipts()
{
    QFETCH(QString,outcome);
    QVERIFY(qEnvironmentVariableIntValue("FERRIC_SETTINGS_SYNC_FIXTURE")==1);
    QVERIFY(page("window.sitePolicyDocument='receipt-document';true").toBool());
    QVERIFY(command("site-policy"));
    const auto workspace=QStringLiteral("testWindow.runtime_policyWorkspace");
    QTRY_VERIFY(evaluate("testWindow.sitePoliciesVisible").toBool());
    evaluate(workspace+".openRule('generated','receipt',true,'https://receipt.test/*')");
    QTRY_VERIFY(evaluate(workspace+".selected !== null").toBool());
    const auto id=evaluate(workspace+".selected.view.id").toString();
    const auto draft=[&]{return QJsonDocument::fromJson(evaluate("testWindow.browserUi.site_policy_draft_snapshot("+quoted(id)+")").toString().toUtf8()).object();};
    const auto statePath=evaluate("testWindow.storageBasePath").toString()+"/state";
    const auto savedPath=statePath+"/runtime-overrides.toml";
    QTemporaryDir hold;QVERIFY(hold.isValid());
    const auto ready=hold.filePath("ready"),release=hold.filePath("release");
    const auto releaseWriter=[&]{QFile file(release);if(file.open(QIODevice::WriteOnly))file.close();};
    const auto cleanup=qScopeGuard([&]{
        releaseWriter();
        for(const auto *key:{"FERRIC_SETTINGS_SYNC_HOLD_PATH","FERRIC_SETTINGS_SYNC_HOLD_FILE","FERRIC_SETTINGS_SYNC_READY_FILE","FERRIC_SETTINGS_SYNC_RELEASE_FILE","FERRIC_SETTINGS_SYNC_FAULT_PATH"})qunsetenv(key);
    });
    if(outcome=="write-rejected")QVERIFY(QDir().mkpath(savedPath));
    else {
        qputenv("FERRIC_SETTINGS_SYNC_HOLD_PATH",statePath.toUtf8());
        qputenv("FERRIC_SETTINGS_SYNC_HOLD_FILE",savedPath.toUtf8());
        qputenv("FERRIC_SETTINGS_SYNC_READY_FILE",ready.toUtf8());
        qputenv("FERRIC_SETTINGS_SYNC_RELEASE_FILE",release.toUtf8());
        if(outcome=="uncertain")qputenv("FERRIC_SETTINGS_SYNC_FAULT_PATH",statePath.toUtf8());
    }
    evaluate(workspace+".review("+workspace+".selected);"+workspace+".submit("+workspace+".selected,{action:'apply',activate:false})");
    if(outcome!="write-rejected") {
        QTRY_VERIFY_WITH_TIMEOUT(QFile::exists(ready),10000);
        QCOMPARE(draft().value("operation").toObject().value("state").toString(),QString("writing"));
        QVERIFY(evaluate(workspace+".busy("+workspace+".selected)").toBool());
        if(outcome=="closed")evaluate(workspace+".close()");
        if(outcome=="profile-switch") {
            evaluate("testWindow.browserUi.configure_profile(true,false,'New owner','receipt-new-owner',testWindow.storageBasePath)");
            QTRY_VERIFY(!evaluate("testWindow.browserUi.profile_bootstrap_pending").toBool());
            QVERIFY(!evaluate("testWindow.sitePoliciesVisible").toBool());
        }
        releaseWriter();
    }
    if(outcome=="profile-switch") {
        QTRY_VERIFY_WITH_TIMEOUT(QFile::exists(savedPath),10000);
        QVERIFY(draft().contains("error"));
        QVERIFY(command("site-policy"));
        QTRY_VERIFY(evaluate(workspace+".sessions.length === 0").toBool());
    } else {
        const auto expected=outcome=="write-rejected"?"failed":"reload-required";
        QTRY_COMPARE(draft().value("operation").toObject().value("state").toString(),QString(expected));
        const auto receipt=draft().value("operation").toObject();
        QCOMPARE(receipt.value("saved").toBool(),outcome!="write-rejected");
        QVERIFY(!receipt.value("active").toBool());
        QCOMPARE(receipt.value("durability_uncertain").toBool(),outcome=="uncertain");
        QCOMPARE(draft().value("dirty").toBool(),outcome=="write-rejected");
        QVERIFY(command("site-policy"));
        QTRY_VERIFY(evaluate("testWindow.sitePoliciesVisible").toBool());
        QTRY_COMPARE(evaluate("("+workspace+".selected.view.operation || {}).state").toString(),QString(expected));
    }
    if(outcome!="profile-switch")QCOMPARE(page("window.sitePolicyDocument").toString(),QString("receipt-document"));
}

void ApplicationTests::sitePolicyLivePreview_data()
{
    QTest::addColumn<QString>("journey");
    for (const auto *name:{"mixed-families","background","disabled","edited","closed","navigation"}) QTest::newRow(name)<<QString::fromLatin1(name);
}

void ApplicationTests::sitePolicyLivePreview()
{
    QFETCH(QString,journey);
    const auto fixture=QString("http://127.0.0.1:%1/fixture.html").arg(server.serverPort());
    int targetIndex=evaluate("testWindow.browserUi.active_tab_index").toInt();
    if (journey=="background") {
        QVERIFY(command("open --target tab "+quoted(fixture+"?preview=background")));
        QTRY_VERIFY(evaluate("testWindow.activeWebView() && testWindow.activeWebView().url.toString() === "+quoted(fixture+"?preview=background")+" && testWindow.activeWebView().loading === false").toBool());
        targetIndex=evaluate("testWindow.browserUi.active_tab_index").toInt();
    }
    const auto receiver=QString("testWindow.tabViewAt(%1)").arg(targetIndex);
    pageReceiver=receiver;
    QVERIFY(page(R"JS(document.body.innerHTML='<a id="preview-one" href="#never">Navigation</a><div id="preview-list"><div class="preview-row"><a href="#one">One</a></div><div class="preview-row"><a href="#two">Two</a></div></div>';window.previewEvents={click:0,focus:0,key:0};document.addEventListener('click',()=>previewEvents.click++);document.addEventListener('focusin',()=>previewEvents.focus++);document.addEventListener('keydown',()=>previewEvents.key++);window.previewDocument='same-document';true)JS").toBool());
    const auto tab=evaluate(receiver+".stableTabId").toString();
    QVERIFY(!tab.isEmpty());
    if(journey=="background") QVERIFY(evaluate("testWindow.browserUi.select_tab(0)").toBool());
    const auto selected=evaluate("testWindow.browserUi.active_tab_index").toInt();
    const auto active=[&]{return QJsonDocument::fromJson(evaluate("testWindow.browserUi.interaction_policy_snapshot()").toString().toUtf8()).object();};
    QTRY_VERIFY(active().contains("reload_revision"));
    const auto before=active().value("reload_revision");
    const auto result=[&]{return QJsonDocument::fromJson(text("site_policy_result").toUtf8()).object();};
    QVERIFY(command("site-policy-edit --source temporary --id preview --new --pattern "+quoted(QString("http://127.0.0.1:%1/*").arg(server.serverPort()))));
    QTRY_VERIFY(!result().value("id").toString().isEmpty());
    const auto id=result().value("id").toString();
    const auto revision=result().value("revision").toInt();
    QJsonObject interaction{{"enabled",journey!="disabled"},{"labels",QJsonObject{{"auto_follow","full-match"},{"fallback_prefix","x"}}},
        {"anchors",QJsonArray{QJsonObject{{"id","navigation"},{"code","na"},{"families",QJsonArray{"all","links"}},{"target_selector","#preview-one"}}}},
        {"collections",QJsonArray{QJsonObject{{"id","rows"},{"families",QJsonArray{"links"}},{"codes",QJsonArray{"ma","mb"}},
            {"container_selector","#preview-list"},{"item_selector",".preview-row"},{"primary_selector","a"},{"order","dom"}}}}};
    const auto payload=QJsonDocument(QJsonObject{{"action","patch"},{"draft",id},{"revision",revision},
        {"proposals",QJsonArray{QJsonObject{{"pattern",QString("http://127.0.0.1:%1/*").arg(server.serverPort())},{"priority",0},{"interaction",interaction}}}}}).toJson(QJsonDocument::Compact);
    const auto token=evaluate("testWindow.browserUi.site_policy_editor_request("+quoted(QString::fromUtf8(payload))+")").toString();
    QTRY_COMPARE(result().value("request").toString(),token);
    QVERIFY2(!result().contains("error"),qPrintable(QString::fromUtf8(QJsonDocument(result()).toJson())));
    const auto reviewedRevision=result().value("result").toObject().value("revision").toInt();
    QVERIFY(command("site-policy-preview "+id));
    QTRY_COMPARE(result().value("kind").toString(),QString("static-only"));
    QVERIFY(result().value("valid").toBool());
    QVERIFY(!result().value("activation_permitted").toBool());
    QVERIFY(command("site-policy-preview "+id+" --tab "+tab));
    const bool invalidated=journey=="edited"||journey=="closed"||journey=="navigation";
    if(invalidated) {
        QTRY_COMPARE(result().value("status").toString(),QString("pending-live"));
        if(journey=="edited") {
            auto newer=QJsonDocument::fromJson(payload).object();newer["revision"]=reviewedRevision;
            auto proposal=newer.value("proposals").toArray()[0].toObject();proposal["priority"]=1;
            newer["proposals"]=QJsonArray{proposal};
            const auto newerToken=evaluate("testWindow.browserUi.site_policy_editor_request("+quoted(QString::fromUtf8(QJsonDocument(newer).toJson(QJsonDocument::Compact)))+")").toString();
            QTRY_COMPARE(result().value("request").toString(),newerToken);
            QVERIFY(!result().contains("error"));
        } else if(journey=="closed") {
            QVERIFY(evaluate("testWindow.sitePolicyPreviewWorkspace.stop();true").toBool());
        } else {
            QVERIFY(page("location.replace('/fixture.html?preview=navigated');true").toBool());
            QTRY_COMPARE(text("current_url"),fixture+"?preview=navigated");
        }
    }
    const auto receipt=[&]{return QJsonDocument::fromJson(evaluate("testWindow.browserUi.site_policy_draft_snapshot("+quoted(id)+")").toString().toUtf8()).object().value("preview").toObject();};
    QTRY_VERIFY_WITH_TIMEOUT(receipt().value("kind").toString().startsWith("live"),30000);
    const auto report=receipt();
    const auto tracking=page("(()=>{const s=window.__ferric_browserHintState;return JSON.stringify(s?{revision:s.revision,dirty:s.dirty,tracking:s.tracking,pending:!!s.pendingInteraction}:null)})()",1).toString();
    QVERIFY2(report.value("kind").toString()==(journey=="disabled"||invalidated?"live-unavailable":"live"),qPrintable(QString::fromUtf8(QJsonDocument(report).toJson())+tracking));
    const auto invocations=report.value("observations").toObject().value("invocations").toArray();
    if(!invalidated)QCOMPARE(invocations.size(),11);
    if (journey!="disabled"&&!invalidated) {
        QJsonArray all,links;
        for(const auto invocation:invocations) {
            const auto row=invocation.toObject();
            if(row.value("family")=="all"||row.value("family")=="links") {
                const auto diagnostics=row.value("result").toObject().value("diagnostics").toObject();
                QVERIFY(diagnostics.contains("unavailable_rules"));
                QVERIFY(diagnostics.value("unavailable_rules").toArray().isEmpty());
                const auto work=diagnostics.value("discovery_work").toObject();
                QVERIFY(work.value("inspections").isDouble());
                QVERIFY(work.value("predicate_steps").isDouble());
            }
            if(row.value("family")=="all")all=row.value("result").toObject().value("reservations").toArray();
            if(row.value("family")=="links")links=row.value("result").toObject().value("reservations").toArray();
        }
        QCOMPARE(all.size(),1);QCOMPARE(links.size(),3);
        for(const auto slot:links)QCOMPARE(slot.toObject().value("state").toString(),QString("Live"));
    }
    QCOMPARE(evaluate("testWindow.browserUi.active_tab_index").toInt(),selected);
    QCOMPARE(text("mode"),QString("normal"));
    if(journey!="navigation") {
        QCOMPARE(page("JSON.stringify(window.previewEvents)").toString(),QString("{\"click\":0,\"focus\":0,\"key\":0}"));
        QCOMPARE(page("window.previewDocument").toString(),QString("same-document"));
    }
    QTRY_VERIFY(page("!window.__ferric_browserHintState || !window.__ferric_browserHintState.tracking",1).toBool());
    if(journey=="edited") {
        const auto current=QJsonDocument::fromJson(evaluate("testWindow.browserUi.site_policy_draft_snapshot("+quoted(id)+")").toString().toUtf8()).object();
        QVERIFY(current.value("revision").toInt()>reviewedRevision);
        QCOMPARE(current.value("rules").toArray()[0].toObject().value("proposed").toObject().value("priority").toInt(),1);
        QCOMPARE(report.value("revision").toInt(),reviewedRevision);
    }
    QCOMPARE(QJsonDocument::fromJson(evaluate("testWindow.browserUi.interaction_policy_snapshot()").toString().toUtf8()).object().value("reload_revision"),before);
    pageReceiver="testWindow.activeWebView()";
}

void ApplicationTests::sitePolicyDraftWorkflow()
{
    QFETCH(QString,journey);
    const bool generated=journey.startsWith("generated");
    const bool activate=journey.endsWith("activate");
    const auto active=[&]{return QJsonDocument::fromJson(evaluate("testWindow.browserUi.interaction_policy_snapshot()").toString().toUtf8()).object();};
    QTRY_VERIFY(active().contains("reload_revision"));
    const auto originalReload=active().value("reload_revision").toString();
    QVERIFY(page("window.sitePolicyDocument='original-document';true").toBool());
    const auto result=[&]{return QJsonDocument::fromJson(text("site_policy_result").toUtf8()).object();};
    QVERIFY(command(QString("site-policy-edit --source %1 --id workflow --new --pattern %2 --enabled true")
        .arg(generated?"generated":"temporary",quoted(QString("http://127.0.0.1:%1/*").arg(server.serverPort())))));
    QTRY_VERIFY(!result().value("id").toString().isEmpty());
    const auto id=result().value("id").toString();
    auto revision=result().value("revision").toVariant().toULongLong();
    const auto draft=[&]{return QJsonDocument::fromJson(evaluate("testWindow.browserUi.site_policy_draft_snapshot("+quoted(id)+")").toString().toUtf8()).object();};
    QCOMPARE(active().value("reload_revision").toString(),originalReload);
    if(journey=="rebase") {
        QVERIFY(command(QString("site-policy-edit --draft %1 --revision %2 --rebase").arg(id).arg(revision)));
        QTRY_VERIFY(result().value("revision").toVariant().toULongLong()>revision);
        revision=result().value("revision").toVariant().toULongLong();
        QCOMPARE(result().value("id").toString(),id);
        QVERIFY(result().value("rebased_from").isArray());
    }
    if(journey=="stale-revision") {
        QVERIFY(command(QString("site-policy-apply %1 --revision %2").arg(id).arg(revision+1)));
        QTRY_VERIFY(text("status_text").contains("revision"));
        QVERIFY(draft().value("operation").isNull());
        QCOMPARE(active().value("reload_revision").toString(),originalReload);
    }
    QVERIFY(command(QString("site-policy-apply %1 --revision %2%3").arg(id).arg(revision).arg(activate?" --activate":"")));
    QTRY_COMPARE(draft().value("operation").toObject().value("state").toString(),QString(activate?"applied":"reload-required"));
    const auto operation=draft().value("operation").toObject();
    QCOMPARE(operation.value("saved").toBool(),generated);
    QCOMPARE(operation.value("active").toBool(),activate);
    QVERIFY(!draft().value("dirty").toBool());
    if(activate)QTRY_VERIFY(active().value("reload_revision").toString()!=originalReload);
    else QCOMPARE(active().value("reload_revision").toString(),originalReload);
    QCOMPARE(page("window.sitePolicyDocument").toString(),QString("original-document"));
}

void ApplicationTests::hintCollectionTimeoutSettlement_data()
{
    QTest::addColumn<QString>("scenario");
    for (const auto *name : {"settles", "wait-bound", "clear", "new-invocation"})
        QTest::newRow(name) << QString::fromLatin1(name);
}

void ApplicationTests::hintCollectionTimeoutSettlement()
{
    QFETCH(QString, scenario);
    QVERIFY(page("document.body.innerHTML='<button>Collection target</button>';true").toBool());
    QVERIFY(command("hint all"));
    QVERIFY(!hintLabel("Collection target").isEmpty());
    auto *workspace = evaluate("testWindow.hintWorkspace").value<QObject *>();
    QVERIFY(workspace);
    QVERIFY(suspendHintRefreshForAdapterFixture(workspace));
    const auto owner = evaluate("testWindow.hintFrameProvider.rootOwner").toString();
    QVERIFY(!owner.isEmpty());
    frameCallbackCount = 0;
    const int delay = scenario == "wait-bound" ? 2400 : 1400;
    const auto script = QString("(()=>{const end=performance.now()+%1;while(performance.now()<end){};return true})()").arg(delay);
    // The delivery expires at 1s. The recovery callback must follow actual
    // engine settlement (or its own bound), rather than the delivery timeout.
    evaluate("testWindow.hintFrameProvider.run(" + quoted(owner) + "," + quoted(script)
        + ",function(v){testProbe.frameOperationComplete(v);testWindow.hintFrameProvider.nativeBroker.waitForPendingScripts(function(ok){testProbe.frameOperationComplete(ok)})})");
    QTRY_COMPARE_WITH_TIMEOUT(frameCallbackCount, 1, 1250);
    QCOMPARE(frameCallbackResult.toMap().value("error").toString(), QString("frame-timeout"));
    if (scenario == "clear" || scenario == "new-invocation") key(Qt::Key_Escape);
    QTRY_COMPARE_WITH_TIMEOUT(frameCallbackCount, 2, 1500);
    QCOMPARE(frameCallbackResult.toBool(), scenario == "settles");
    if (scenario == "new-invocation") {
        QVERIFY(command("hint all"));
        QVERIFY(!hintLabel("Collection target").isEmpty());
        QTest::qWait(200);
        QCOMPARE(text("mode"), QString("hint"));
    }
    QTest::qWait(500);
    QCOMPARE(frameCallbackCount, 2);
}
