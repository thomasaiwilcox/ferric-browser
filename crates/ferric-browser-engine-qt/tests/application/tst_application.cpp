#include "tst_application.h"
#include "../../src/url_display.h"
#include <QClipboard>
#include <QMimeData>
#include <QProcess>
#include <QCryptographicHash>
#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QFileInfo>
#include <QGuiApplication>
#include <QImage>
#include <QHostAddress>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQmlExpression>
#include <QQuickItem>
#include <QQuickWindow>
#include <QTcpSocket>
#include <QSslServer>
#include <QSslKey>
#include <QSslSocket>
#include <QtWebEngineCore/QWebEngineFrame>
#include <QtWebEngineCore/QWebEngineWebAuthUxRequest>
#include <QTemporaryDir>
#include <QtTest>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include <csignal>
#include <sqlite3.h>
#include <unistd.h>

static QQuickItem *findVisualItem(QQuickItem *parent, const QString &name) {
    if (parent->objectName() == name) return parent;
    for (auto child : parent->childItems()) {
        if (auto found = findVisualItem(child, name)) return found;
    }
    return nullptr;
}

// Only observation and fixture orchestration live here. Policy, BrowserUi,
// chrome, key routing and pointer dispatch all come from the production module.
QVariant ApplicationTests::evaluate(const QString &source) {
    QQmlExpression expression(engine->rootContext(), window, source);
    const auto value = expression.evaluate();
    if (expression.hasError())
        qFatal("QML expression: %s", qPrintable(expression.error().toString()));
    return value;
}
QString ApplicationTests::text(const QString &property) {
    return evaluate("testWindow.browserUi." + property).toString();
}
bool ApplicationTests::command(const QString &value) {
    const auto accepted =
        evaluate("testWindow.browserUi.execute_command(" + quoted(value) + ")")
            .toBool();
    if (!accepted)
        qInfo() << "command rejected" << value << text("status_text");
    return accepted;
}
void ApplicationTests::interactiveCommand(const QString &value) {
    evaluate("testWindow.browserUi.enter_command()");
    auto input = window->findChild<QQuickItem *>("commandLineInput");
    QVERIFY(input);
    QVERIFY(input->setProperty("text", value));
    evaluate("testWindow.submitInteractiveCommand(" + quoted(value) + ")");
}
QString ApplicationTests::quoted(const QString &value) {
    QJsonArray array;
    array.append(value);
    auto bytes = QJsonDocument(array).toJson(QJsonDocument::Compact);
    return QString::fromUtf8(bytes.mid(1, bytes.size() - 2));
}
QVariant ApplicationTests::page(const QString &source, int world) {
    scriptDone = false;
    evaluate(pageReceiver + ".runJavaScript(" + quoted(source) + ", " +
             QString::number(world) +
             ", function(value) { testProbe.accept(value); })");
    QElapsedTimer deadline;
    deadline.start();
    while (!scriptDone && deadline.elapsed() < 5000)
        QTest::qWait(10);
    if (!scriptDone)
        qFatal("WebEngine script callback timed out");
    return scriptResult;
}
void ApplicationTests::key(int value, Qt::KeyboardModifiers modifiers) {
    QTest::keyClick(window, Qt::Key(value), modifiers);
    QTest::qWait(30);
}

void ApplicationTests::accept(const QVariant &value) {
    scriptResult = value;
    scriptDone = true;
}

void ApplicationTests::frameOperationComplete(const QVariant &value) {
    ++frameCallbackCount;scriptResult=value;
}

void ApplicationTests::initTestCase() {
    if (QGuiApplication::platformName() != "wayland")
        return;
    QQuickWindow probe;
    probe.resize(320, 240);
    probe.show();
    probe.requestActivate();
    QTRY_VERIFY2_WITH_TIMEOUT(probe.isActive(),
                              "Wayland input qualification unavailable: "
                              "compositor cannot activate a test window/seat",
                              3000);
}

void ApplicationTests::init() {
    // Qt can skip cleanup after an init assertion fails (for example an
    // unavailable compositor seat). Always retire the previous fixture.
    cleanup();
    pageReceiver = "testWindow.activeWebView()";
    QVERIFY(directory.isValid());
    QFile fixture(directory.filePath("fixture.html"));
    QVERIFY(fixture.open(QIODevice::WriteOnly));
    fixture.write(
        R"HTML(<!doctype html><meta charset="utf-8"><style>body{margin:30px}button,input,a,[contenteditable]{display:block;margin:15px;width:260px;height:32px}</style>
<button id="button" onclick="window.clicks++;window.trust=event.isTrusted">Activate exact button</button>
<input id="input" aria-label="Editable field"><div id="editor" aria-label="Editable document" contenteditable="true">Editable content</div>
<a id="same" href="#fragment">Same document</a><a id="new" href="fixture.html?new">New document</a>
<iframe id="frame" src="/child.html" style="position:absolute;left:530px;top:30px;border:9px solid green;width:300px;height:180px"></iframe><div id="shadow" style="position:absolute;left:530px;top:260px"></div><a id="blank" href="/popup.html" target="_blank" style="position:absolute;left:530px;top:330px">Open popup</a>
<section role="region" aria-label="Scoped panel" style="margin-left:70px;border:7px solid black;width:350px;height:220px;overflow:auto"><button id="scoped" onclick="window.scopedClicks=(window.scopedClicks||0)+1">Scoped control</button><div style="height:800px"></div></section><div style="height:1500px"></div><script>window.clicks=0;window.trust=false;window.shadowClicks=0;document.getElementById('shadow').attachShadow({mode:'open'}).innerHTML='<button style="width:220px;height:32px" onclick="window.shadowClicks++;window.shadowTrusted=event.isTrusted">Open shadow action</button>';window.keys=[];document.addEventListener('keydown',e=>window.keys.push(e.key));document.addEventListener('click',e=>{window.lastClicked=e.target.id;window.lastTrusted=e.isTrusted;window.allClicks=(window.allClicks||0)+1;window.clientPoint=[e.clientX,e.clientY]},true);</script>)HTML");
    fixture.close();
    QVERIFY(fixture.open(QIODevice::ReadOnly));
    fixtureHtml = fixture.readAll();
    fixture.close();
    QVERIFY(server.listen(QHostAddress::LocalHost, 0));
    connect(&server, &QTcpServer::newConnection, this, [this] {
        while (auto socket = server.nextPendingConnection()) {
            connect(socket, &QTcpSocket::readyRead, socket, [this, socket] {
                const auto request = socket->readAll();
                if (!request.contains("\r\n\r\n"))
                    return;
                const auto path = request.split(' ').value(1);
                if (path == "/polish-auth" && !request.contains(
                        "Authorization: Basic " + QByteArray("test-user:test-pass").toBase64())) {
                    socket->write("HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"Polish fixture\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                    socket->disconnectFromHost();
                    return;
                }
                const QByteArray body = path == "/polish-auth"
                    ? QByteArray("<!doctype html><script>window.authenticated=true</script>")
                    : frameFixtures.contains(path) ? frameFixtures.value(path) :
                    request.startsWith("GET /child.html ")
                        ? QByteArray(
                              "<!doctype "
                              "html><style>body{margin:10px}</style><button "
                              "id='child' style='width:240px;height:32px' "
                              "onclick='window.clicks=(window.clicks||0)+1;"
                              "window."
                              "trust=event.isTrusted'>Frame control</button>")
                        : fixtureHtml;
                socket->write(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; "
                    "charset=utf-8\r\nConnection: close\r\nContent-Length: " +
                    QByteArray::number(body.size()) + "\r\n\r\n" + body);
                socket->disconnectFromHost();
            });
            connect(socket, &QTcpSocket::disconnected, socket,
                    &QObject::deleteLater);
        }
    });
    engine = std::make_unique<QQmlApplicationEngine>();
    const auto testName = QString::fromLatin1(QTest::currentTestFunction());
    const bool durable = testName.startsWith("durablePersistence")
                         || (testName.startsWith("productPolish") && testName != "productPolishPrivateLibrary"
                             && !(testName == "productPolishDiagnosticLifecycle" && QString::fromLatin1(QTest::currentDataTag()) == "private"));
    QString startupConfig = "{\"schema_version\":3}";
    if (testName == "productPolishSiteLedger")
        startupConfig = R"JSON({"schema_version":3,"blocking":{"enabled":true,"bypass_sites":["other-private.test"]}})JSON";
    if (QString::fromLatin1(QTest::currentTestFunction()) ==
        "startupSelectorValidation") {
        const bool valid =
            QString::fromLatin1(QTest::currentDataTag()) == "accepted";
        const QJsonObject selector{
            {"mode", "extend"},
            {"selectors", QJsonArray{valid ? "button" : "["}}};
        const QJsonObject config{
            {"schema_version", 3},
            {"ui", QJsonObject{{"font_size_pt", 15}}},
            {"hints",
             QJsonObject{{"selectors", QJsonObject{{"all", selector}}}}}};
        startupConfig = QString::fromUtf8(
            QJsonDocument(config).toJson(QJsonDocument::Compact));
    }
    if (QString::fromLatin1(QTest::currentTestFunction()) == "selectorDiscoveryUsesProductionPolicy") {
        const auto tag = QString::fromLatin1(QTest::currentDataTag());
        QJsonObject policies;
        policies.insert("scrollables", QJsonObject{{"mode", "replace"}, {"selectors", QJsonArray{}}});
        if (tag != "inherit") policies.insert("all", QJsonObject{
            {"mode", tag == "extend" ? "extend" : "replace"},
            {"selectors", tag == "disabled" ? QJsonArray{} : tag == "extend" ? QJsonArray{".custom"} : QJsonArray{"#button"}}});
        startupConfig = QString::fromUtf8(QJsonDocument(QJsonObject{
            {"schema_version",3}, {"hints",QJsonObject{{"selectors",policies}}}}).toJson(QJsonDocument::Compact));
    }
    const auto storageBase = QDir::cleanPath(testName.startsWith("productPolish")
        ? directory.filePath("durable-test/" + testName + "/" + QString::fromLatin1(QTest::currentDataTag()))
        : directory.filePath("durable-test"));
    if (testName == "productPolishUnavailableStorage") {
        QVERIFY(QDir().mkpath(QFileInfo(storageBase).absolutePath()));
        QFile blockedRoot(storageBase);
        QVERIFY(blockedRoot.open(QIODevice::WriteOnly));
        blockedRoot.write("fixture blocks profile directory");
    }
    engine->setInitialProperties(
        {{"temporaryProfile", !durable},
         {"storageBasePath", storageBase},
         {"softwareRendering", true},
         {"nativeWayland", QGuiApplication::platformName() == "wayland"},
         {"startupConfigJson", startupConfig},
         {"startupConfigBaseJson", startupConfig},
         {"startupCliOverridesJson", "{\"schema_version\":3}"},
         {"startupProfileOverridesJson", "{\"schema_version\":3}"},
         {"safeMode", true},
         {"userscriptsOff", true},
         {"width", 1000},
         {"height", 800},
         {"startupUrl", QString("http://127.0.0.1:%1/fixture.html")
                            .arg(server.serverPort())}});
    qInfo() << "loading production QML";
    engine->load(QUrl("qrc:/qt/qml/io/github/ferricbrowser/qml/Main.qml"));
    qInfo() << "production QML created";
    QCOMPARE(engine->rootObjects().size(), 1);
    window = qobject_cast<QQuickWindow *>(engine->rootObjects().first());
    QVERIFY(window);
    engine->rootContext()->setContextProperty("testWindow", window);
    engine->rootContext()->setContextProperty("testProbe", this);
    QTRY_VERIFY_WITH_TIMEOUT(
        evaluate("testWindow.activeWebView() !== null").toBool(), 10000);
    QTRY_VERIFY_WITH_TIMEOUT(
        !evaluate("testWindow.activeWebView().loading").toBool(), 15000);
    qInfo() << "waiting for URL" << text("current_url");
    QTRY_VERIFY_WITH_TIMEOUT(
        text("current_url").startsWith("http://127.0.0.1:"), 15000);
    QTRY_VERIFY_WITH_TIMEOUT(
        !evaluate("testWindow.activeWebView().loading").toBool(), 15000);
    qInfo() << "fixture loaded" << text("current_url") << text("status_text");
    QTRY_VERIFY_WITH_TIMEOUT(
        page("document.getElementById('button') !== null").toBool(), 15000);
    window->show();
    window->requestActivate();
    QTest::qWait(150);
    evaluate("testWindow.activeWebView().forceActiveFocus()");
    QTRY_VERIFY_WITH_TIMEOUT(window->isActive(), 3000);
    QTRY_VERIFY_WITH_TIMEOUT(
        evaluate("testWindow.browserKeyFocusActive").toBool(), 3000);
}
void ApplicationTests::cleanup() {
    engine.reset();
    frameFixtures.clear();
    server.close();
    server.disconnect(this);
    window = nullptr;
    QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
    QTest::qWait(20);
}

void ApplicationTests::crossOriginConsentHints_data() {
    QTest::addColumn<double>("zoom");
    QTest::addColumn<QString>("choice");
    QTest::addColumn<QString>("entry");
    for (const auto zoom : {0.8,1.0,1.25}) for (const auto &choice : {"Accept","Reject All","Manage My Choices"}) {
        const auto tag=QString("explicit-%1-%2").arg(zoom).arg(choice).toUtf8();
        QTest::newRow(tag.constData()) << zoom << QString(choice) << QString("explicit");
    }
    QTest::newRow("already-focused") << 1.0 << QString("Reject All") << QString("focused");
    QTest::newRow("nested") << 1.0 << QString("Manage My Choices") << QString("nested");
    QTest::newRow("duplicate-left") << 1.0 << QString("Accept") << QString("left");
    QTest::newRow("duplicate-right") << 1.0 << QString("Accept") << QString("right");
    QTest::newRow("opaque-script-allowed") << 1.0 << QString("Accept") << QString("opaque");
    QTest::newRow("positive-scaling") << 1.0 << QString("Reject All") << QString("scaled");
}
void ApplicationTests::crossOriginConsentHints() {
    QFETCH(double,zoom);QFETCH(QString,choice);QFETCH(QString,entry);
    const auto port=server.serverPort();
    const auto child=QString("http://localhost:%1/consent").arg(port);
    frameFixtures["/consent"]=R"HTML(<!doctype html><style>body{margin:10px}button{display:block;width:260px;height:38px;margin:10px}</style><button onclick="window.choice='Accept';window.count=(window.count||0)+1">Accept</button><button onclick="window.choice='Reject All';window.count=(window.count||0)+1">Reject All</button><button onclick="window.choice='Manage My Choices';window.count=(window.count||0)+1;document.body.dataset.preferences='open'">Manage My Choices</button><script>window.count=0;window.keys=[];document.addEventListener('keydown',e=>window.keys.push(e.key))</script>)HTML";
    QString source=child;
    if(entry=="nested") {
        frameFixtures["/outer"]=QString("<!doctype html><iframe title='Nested consent' src='http://127.0.0.1:%1/consent' style='position:absolute;left:15px;top:20px;width:300px;height:200px;border:3px solid'></iframe>").arg(port).toUtf8();
        source=QString("http://localhost:%1/outer").arg(port);
    }
    const bool duplicate=entry=="left"||entry=="right";
    fixtureHtml=QString("<!doctype html><input aria-label='Covered background'><div role='dialog'><iframe id='left' name='duplicate' title='Left consent' src='%1' style='position:absolute;left:30px;top:80px;width:340px;height:260px;border:9px solid;padding:4px'></iframe>%2</div>").arg(source,duplicate?QString("<iframe id='right' name='duplicate' title='Right consent' src='%1' style='position:absolute;left:430px;top:80px;width:340px;height:260px;border:9px solid;padding:4px'></iframe>").arg(source):QString()).toUtf8();
    if(entry=="opaque")fixtureHtml.replace("id='left'", "id='left' sandbox='allow-scripts'");
    if(entry=="scaled")fixtureHtml.replace("border:9px solid;padding:4px", "border:9px solid;padding:4px;transform:scale(1.25);transform-origin:left top");
    QVERIFY(command(QString("open http://127.0.0.1:%1/frames").arg(port)));
    QTRY_VERIFY_WITH_TIMEOUT(page("!!document.getElementById('left')").toBool(),10000);
    QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.activeWebView().mainFrame.children.length>0").toBool(),10000);
    QTest::qWait(150);
    QVERIFY(page("document.getElementById('left').contentDocument===null").toBool());
    evaluate(QString("testWindow.activeWebView().zoomFactor=%1").arg(zoom));
    QVERIFY(command("set --temp hints.auto_follow=never"));
    if(entry=="focused") page("document.getElementById('left').focus();true");
    QVERIFY(command("hint all"));
    if(entry!="focused") {
        const auto frameName=entry=="right"?"Right consent":"Left consent";
        const auto label=hintLabel(frameName);QVERIFY2(!label.isEmpty(),qPrintable(text("status_text")));
        typeText(label);key(Qt::Key_Return);
        QTRY_VERIFY(evaluate("testWindow.pendingContextMenuRequest!==null").toBool());
        QVERIFY(chooseCapability("hint-inside-frame"));
        if(entry=="nested") {
            const auto nested=hintLabel("Nested consent");QVERIFY2(!nested.isEmpty(),qPrintable(text("status_text")));
            typeText(nested);key(Qt::Key_Return);
            QTRY_VERIFY(evaluate("testWindow.pendingContextMenuRequest!==null").toBool());
            QVERIFY(chooseCapability("hint-inside-frame"));
        }
    }
    const auto label=hintLabel(choice);QVERIFY2(!label.isEmpty(),qPrintable(text("status_text")));
    QTRY_VERIFY(!evaluate("testWindow.hintRefreshPending").toBool());
    const auto values=QJsonDocument::fromJson(text("hint_values").toUtf8()).array();
    QString owner;
    for(const auto &value:values) {
        const auto row=value.toObject();
        QVERIFY(row["text"].toString()!="Covered background");
        if(row["text"].toString()==choice) {
            owner=row["frame_owner"].toString();
            QVERIFY(owner.startsWith("frame-"));
            const auto geometry=row;
            QVERIFY(geometry["x"].toDouble()>(entry=="right"?430:30));
            QVERIFY(geometry["y"].toDouble()>80);
        }
    }
    QVERIFY(!owner.isEmpty());
    // Reading through the browser-owned lease verifies the frame selected by production,
    // independently of native frame enumeration order or duplicate metadata.
    auto childRead=[this,owner](const QString &script){
        scriptDone=false;
        evaluate("testWindow.hintFrameProvider.run("+quoted(owner)+","+quoted(script)+",function(v){testProbe.accept(v)})");
        QElapsedTimer timer;timer.start();while(!scriptDone&&timer.elapsed()<5000)QTest::qWait(10);
        return scriptResult;
    };
    QCOMPARE(childRead("document.querySelectorAll('button').length").toInt(),3);
    // Preserve a native handle for read-only post-activation observation, because
    // successful activation must release the production lease.
    QList<QWebEngineFrame> observed;
    std::function<void(QWebEngineFrame,int)> retain=[&](QWebEngineFrame frame,int depth){
        if(depth>8)return;
        observed.append(frame);
        for(const auto &childFrame:frame.children())retain(childFrame,depth+1);
    };
    retain(evaluate("testWindow.activeWebView().mainFrame").value<QWebEngineFrame>(),0);
    QVERIFY(observed.size()>1);
    typeText(label);key(Qt::Key_Return);
    QTRY_COMPARE(text("mode"),QString("normal"));
    QVariantList choices;
    for(auto frame:observed){
        scriptDone=false;
        frame.runJavaScript("({choice:window.choice,count:window.count,keys:window.keys})",0,[this](const QVariant &value){accept(value);});
        QTRY_VERIFY_WITH_TIMEOUT(scriptDone,5000);
        if(!scriptResult.toMap()["choice"].toString().isEmpty())choices.append(scriptResult);
    }
    QCOMPARE(choices.size(),1);
    QCOMPARE(choices[0].toMap()["choice"].toString(),choice);
    QCOMPARE(choices[0].toMap()["count"].toInt(),1);
    QCOMPARE(choices[0].toMap()["keys"].toList().size(),0);
    QCOMPARE(evaluate("testWindow.hintFrameProvider.leaseCount").toInt(),0);
}
void ApplicationTests::crossOriginFrameInvalidation_data() {
    QTest::addColumn<QString>("change");
    for(const auto &change:{"navigate","remove","replace","covered","focus","rotate","sandbox","redirect","zoom","resize"})QTest::newRow(change)<<QString(change);
}
void ApplicationTests::crossOriginFrameInvalidation() {
    QFETCH(QString,change);
    const auto port=server.serverPort();
    frameFixtures["/consent"]="<!doctype html><button onclick='window.count=(window.count||0)+1'>Reject All</button><script>window.count=0</script>";
    fixtureHtml=QString("<!doctype html><iframe id='left' title='Consent' src='http://localhost:%1/consent' style='position:absolute;left:40px;top:80px;width:300px;height:180px'></iframe><input id='other'>").arg(port).toUtf8();
    QVERIFY(command(QString("open http://127.0.0.1:%1/frames").arg(port)));
    QTRY_VERIFY(page("!!document.getElementById('left')").toBool());QTest::qWait(180);
    QVERIFY(command("set --temp hints.auto_follow=never"));
    if(change=="sandbox")page("document.getElementById('left').setAttribute('sandbox','');true");
    if(change=="redirect")page("document.getElementById('left').addEventListener('focus',()=>document.getElementById('other').focus());true");
    QVERIFY(command("hint all"));
    auto label=hintLabel("Consent");QVERIFY(!label.isEmpty());typeText(label);key(Qt::Key_Return);
    QTRY_VERIFY(evaluate("testWindow.pendingContextMenuRequest!==null").toBool());QVERIFY(chooseCapability("hint-inside-frame"));
    if(change=="sandbox"||change=="redirect") {
        QTRY_COMPARE(text("mode"),QString("normal"));
        QVERIFY(text("status_text").contains("Cannot hint inside"));return;
    }
    label=hintLabel("Reject All");QVERIFY2(!label.isEmpty(),qPrintable(text("status_text")));
    const auto rows=QJsonDocument::fromJson(text("hint_values").toUtf8()).array();QJsonObject selected;
    for(const auto &row:rows)if(row.toObject()["label"].toString()==label)selected=row.toObject();
    QVERIFY(!selected.isEmpty());
    const QHash<QString,QString> scripts{
        {"navigate","document.getElementById('left').src+='/new';true"},
        {"remove","document.getElementById('left').remove();true"},
        {"replace","let old=document.getElementById('left');old.replaceWith(old.cloneNode(true));true"},
        {"covered","let cover=document.createElement('div');cover.style='position:fixed;inset:0;background:black;z-index:9999';document.body.append(cover);true"},
        {"focus","document.getElementById('other').focus();true"},
        {"rotate","document.getElementById('left').style.transform='rotate(15deg)';true"}};
    if(change=="zoom")evaluate("testWindow.activeWebView().zoomFactor=1.25");
    else if(change=="resize")evaluate("testWindow.width+=80");
    else page(scripts.value(change));
    if(change!="zoom"&&change!="resize")QTest::qWait(120);
    scriptDone=false;
    evaluate("testWindow.hintFrameProvider.fresh("+QString::fromUtf8(QJsonDocument(selected).toJson(QJsonDocument::Compact))+",testWindow.hintFreshScript("+QString::fromUtf8(QJsonDocument(selected).toJson(QJsonDocument::Compact))+"),function(v){testProbe.accept(v)})");
    QTRY_VERIFY_WITH_TIMEOUT(scriptDone,5000);
    QVERIFY(!scriptResult.toMap()["visible"].toBool());
    key(Qt::Key_Escape);
    QTRY_COMPARE(evaluate("testWindow.hintFrameProvider.leaseCount").toInt(),0);
}

void ApplicationTests::crossOriginFrameOperationLifetimes() {
    const auto port=server.serverPort();
    frameFixtures["/consent"]="<!doctype html><button>Accept</button>";
    fixtureHtml=QString("<!doctype html><iframe id='left' src='http://localhost:%1/consent'></iframe>").arg(port).toUtf8();
    QVERIFY(command(QString("open http://127.0.0.1:%1/frames").arg(port)));
    QTRY_VERIFY(page("!!document.getElementById('left')").toBool());QTest::qWait(150);
    page("document.getElementById('left').focus();true");
    QVERIFY(command("set --temp hints.auto_follow=never"));QVERIFY(command("hint all"));
    QVERIFY(!hintLabel("Accept").isEmpty());
    QString owner;
    for(const auto &value:QJsonDocument::fromJson(text("hint_values").toUtf8()).array())if(value.toObject()["text"].toString()=="Accept")owner=value.toObject()["frame_owner"].toString();
    QVERIFY(!owner.isEmpty());frameCallbackCount=0;
    const auto delayed=[](int millis){return QString("(()=>{const end=performance.now()+%1;while(performance.now()<end){};return true})()").arg(millis);};
    evaluate("testWindow.hintFrameProvider.run("+quoted(owner)+","+quoted(delayed(1200))+",function(v){testProbe.frameOperationComplete(v)})");
    QTRY_COMPARE_WITH_TIMEOUT(frameCallbackCount,1,2500);
    QCOMPARE(scriptResult.toMap()["error"].toString(),QString("frame-timeout"));
    QTest::qWait(500);QCOMPARE(frameCallbackCount,1);
    key(Qt::Key_Escape);page("document.getElementById('left').focus();true");
    QVERIFY(command("hint all"));QVERIFY(!hintLabel("Accept").isEmpty());
    owner.clear();
    for(const auto &value:QJsonDocument::fromJson(text("hint_values").toUtf8()).array())if(value.toObject()["text"].toString()=="Accept")owner=value.toObject()["frame_owner"].toString();
    QVERIFY(!owner.isEmpty());frameCallbackCount=0;
    evaluate("testWindow.hintFrameProvider.run("+quoted(owner)+","+quoted(delayed(150))+",function(v){testProbe.frameOperationComplete(v)})");
    key(Qt::Key_Escape);QTest::qWait(300);QCOMPARE(frameCallbackCount,0);
    QCOMPARE(evaluate("testWindow.hintFrameProvider.leaseCount").toInt(),0);
}
void ApplicationTests::crossOriginFrameLifetimeCycles() {
    if(!qEnvironmentVariableIsSet("FERRIC_APP_STRESS"))QSKIP("Opt-in 1000-cycle frame lifetime check");
    const auto port=server.serverPort();
    frameFixtures["/consent"]="<!doctype html><button>Accept</button>";
    fixtureHtml=QString("<!doctype html><iframe id='left' src='http://localhost:%1/consent'></iframe>").arg(port).toUtf8();
    QVERIFY(command(QString("open http://127.0.0.1:%1/frames").arg(port)));
    QTRY_VERIFY(page("!!document.getElementById('left')").toBool());QTest::qWait(150);
    QVERIFY(command("set --temp hints.auto_follow=never"));
    for(int cycle=0;cycle<1000;++cycle){
        page("document.getElementById('left').focus();true");QVERIFY(command("hint all"));
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.hintResults.some(r=>r.text==='Accept'&&r.frame_owner)").toBool(),5000);
        key(Qt::Key_Escape);QTRY_COMPARE(evaluate("testWindow.hintFrameProvider.leaseCount").toInt(),0);
        if((cycle+1)%100==0)qInfo()<<"Frame lifetime cycles completed"<<cycle+1;
    }
    page("gc&&gc();true",1);QTest::qWait(1200);
    QCOMPARE(evaluate("testWindow.hintFrameProvider.leaseCount").toInt(),0);
}

void ApplicationTests::startupAndConfigPublication() {
    const auto oldConfig = text("config_json");
    QVERIFY(command("set --temp content.zoom=1.25"));
    QTRY_VERIFY(text("config_json") != oldConfig);
    const auto accepted = text("config_json");
    QVERIFY(!command("set --temp content.zoom=-2"));
    QCOMPARE(text("config_json"), accepted);
}
void ApplicationTests::embeddedResourceParity() {
    const QString prefix=":/qt/qml/io/github/ferricbrowser/";
    QFile manifest(prefix+"browser-script-manifest.json");
    QVERIFY(manifest.open(QIODevice::ReadOnly));
    const auto resources=QJsonDocument::fromJson(manifest.readAll()).object()["resources"].toArray();
    QVERIFY(!resources.isEmpty());
    const QDir sourceRoot(QDir(QFileInfo(QString::fromUtf8(__FILE__)).absolutePath()).absoluteFilePath("../../../.."));
    for(const auto &entry:resources) {
        const auto resource=entry.toObject();
        const auto path=resource["path"].toString();
        QFile embedded(prefix+path),source(sourceRoot.filePath("crates/ferric-browser-engine-qt/"+path));
        QVERIFY2(embedded.open(QIODevice::ReadOnly),qPrintable(path));
        QVERIFY2(source.open(QIODevice::ReadOnly),qPrintable(path));
        const auto bytes=embedded.readAll();
        QVERIFY2(bytes==source.readAll(),qPrintable(path));
        QCOMPARE(QString::fromLatin1(QCryptographicHash::hash(bytes,QCryptographicHash::Sha256).toHex()),resource["sha256"].toString());
    }
    qInfo()<<"Embedded resource bytes and manifest hashes match"<<resources.size()<<"source files";
}
void ApplicationTests::reservedCommandRecovery() {
    key(Qt::Key_Colon);
    QTRY_COMPARE(text("mode"), QString("command"));
    key(Qt::Key_Escape);
    QTRY_COMPARE(text("mode"), QString("normal"));
}
void ApplicationTests::realHintKeyboardActivation() {
    key(Qt::Key_F);
    QTest::qWait(100);
    QTRY_VERIFY_WITH_TIMEOUT(
        evaluate("testWindow.browserUi.hint_visible").toBool(), 5000);
    const auto values =
        QJsonDocument::fromJson(text("hint_values").toUtf8()).array();
    QVERIFY2(!values.isEmpty(), qPrintable(text("status_text")));
    const auto label = hintLabel("Activate exact button");
    QVERIFY(!label.isEmpty());
    typeText(label);
    QTRY_VERIFY(!evaluate("testWindow.hintRefreshPending").toBool());
    key(Qt::Key_Return);
    QTRY_COMPARE(page("window.clicks").toInt(), 1);
    QTest::qWait(100);
    QCOMPARE(page("window.clicks").toInt(), 1);
}
void ApplicationTests::stretchedPostTitleKeyboardNavigation_data() {
    QTest::addColumn<double>("zoom");
    QTest::newRow("zoom80") << 0.8;
    QTest::newRow("zoom100") << 1.0;
    QTest::newRow("zoom125") << 1.25;
}
void ApplicationTests::stretchedPostTitleKeyboardNavigation() {
    QFETCH(double, zoom);
    QVERIFY(command("set --temp hints.auto_follow=never"));
    QVERIFY(command("set --temp content.zoom=" + QString::number(zoom)));
    page(R"JS(document.body.innerHTML='<article id=card><a id=cover slot=cover href=/posts/example/comments aria-label="Open whole post"></a><a id=community slot=credit href=/community>Community</a><a id=title slot=title href=/posts/example/comments>Article title</a></article>';
        const style=document.createElement('style');style.textContent='body{margin:0;height:1800px}#card{position:relative;margin:450px 20px 0;width:600px;height:180px}#card a{margin:0}#cover{position:absolute;inset:0;width:100%;height:100%;z-index:2}#community{position:absolute;left:16px;top:8px;width:140px;height:24px;z-index:3}#title{position:absolute;left:16px;top:40px;width:568px;height:48px}';document.head.append(style);
        document.getElementById('card').attachShadow({mode:'open'}).innerHTML='<slot name=cover></slot><slot name=credit></slot><h2><slot name=title></slot></h2>';scrollTo(0,400);true)JS");
    QTest::qWait(150);
    key(Qt::Key_F);
    const auto label=hintLabel("Article title");
    QVERIFY2(!label.isEmpty(), "Visible title needs its own URL hint");
    auto repeater=window->findChild<QQuickItem*>("hintMarkerRepeater");
    QVERIFY(repeater);
    engine->rootContext()->setContextProperty("testHintOverlay",repeater->parentItem());
    QTRY_VERIFY(evaluate("testHintOverlay.laidOutHints.some(r=>r.text==='Article title')").toBool());
    QVERIFY(evaluate(R"JS((function(){const rows=testHintOverlay.laidOutHints, title=rows.find(r=>r.text==='Article title'), community=rows.find(r=>r.text==='Community');return !!community && !(community.marker_x<title.x+title.width&&community.marker_x+community.marker_width>title.x&&community.marker_y<title.y+title.height&&community.marker_y+community.marker_height>title.y);})())JS").toBool());
    typeText(label);
    QTRY_VERIFY(!evaluate("testWindow.hintRefreshPending").toBool());
    key(Qt::Key_Return);
    QTRY_VERIFY_WITH_TIMEOUT(text("current_url").endsWith("/posts/example/comments"),10000);
    QTRY_COMPARE_WITH_TIMEOUT(page("location.pathname").toString(),QString("/posts/example/comments"),10000);
    QTRY_VERIFY_WITH_TIMEOUT(!evaluate("testWindow.activeWebView().loading").toBool(),10000);
    QVERIFY(!text("current_url").endsWith("/community"));
}
void ApplicationTests::mixedReservedKeysAndUnmappedDelivery() {
    QVERIFY(command("set --temp input.shortcut_policy=mixed"));
    QVERIFY(command("set --temp \"input.page_keys=['z']\""));
    page("document.getElementById('input').focus(); window.keys=[]");
    evaluate("testWindow.activeWebView().forceActiveFocus()");
    key(Qt::Key_Z);
    QTRY_VERIFY(page("window.keys.includes('z')").toBool());
    key(Qt::Key_Colon);
    QTRY_COMPARE(text("mode"), QString("command"));
    key(Qt::Key_Escape);
}
void ApplicationTests::typeText(const QString &text) {
    for (const auto character : text) {
        key(character.toUpper().unicode(),
            character.isUpper() ? Qt::ShiftModifier : Qt::NoModifier);
    }
}
QString ApplicationTests::hintLabel(const QString &name) {
    // Wait for the mode footer's first viewport update and the corresponding
    // dirty-snapshot refresh before aiming at a label.
    QTest::qWait(180);
    QElapsedTimer deadline;
    deadline.start();
    do {
        const auto values =
            QJsonDocument::fromJson(text("hint_values").toUtf8()).array();
        for (const auto &value : values) {
            const auto object = value.toObject();
            if (object.value("text").toString() == name)
                return object.value("label").toString();
        }
        QTest::qWait(20);
    } while (deadline.elapsed() < 5000);
    return {};
}
bool ApplicationTests::chooseCapability(const QString &name) {
    const auto action = "hint.capability." + name;
    const auto index =
        evaluate("testWindow.contextMenuItems.findIndex(function("
                 "item) { return item.actionId === " +
                 quoted(action) + " && item.enabled; })")
            .toInt();
    if (index < 0)
        return false;
    QTest::qWait(80);
    for (int i = 0; i < index; ++i)
        key(Qt::Key_Down);
    key(Qt::Key_Return);
    return true;
}
void ApplicationTests::nativePointerRequiresLocalEntryAndClicksOnce() {
    QVERIFY(!command("open about:blank ;; hint --target pointer all"));
    QVERIFY(!evaluate("testWindow.browserUi.hint_pointer_mode").toBool());
    QVERIFY(command("bind p hint --target pointer all"));
    key(Qt::Key_P);
    QTRY_VERIFY(evaluate("testWindow.browserUi.hint_pointer_mode").toBool());
    const auto label = hintLabel("Activate exact button");
    QVERIFY(!label.isEmpty());
    typeText(label);
    QCOMPARE(page("window.clicks").toInt(), 0);
    key(Qt::Key_Return);
    QTRY_COMPARE(page("window.clicks").toInt(), 1);
    QVERIFY(page("window.trust").toBool());
    QTest::qWait(150);
    QCOMPARE(page("window.clicks").toInt(), 1);
    QVERIFY(!evaluate("testWindow.browserUi.hint_pointer_pending").toBool());
}
void ApplicationTests::pointerConfigurationInvalidatesHeldConfirmation() {
    QVERIFY(command("bind p hint --target pointer all"));
    key(Qt::Key_P);
    const auto label = hintLabel("Activate exact button");
    QVERIFY(!label.isEmpty());
    typeText(label);
    QTest::keyPress(window, Qt::Key_Return);
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('hints.marker_"
                     "scale','1.2',true)")
                .toBool());
    QTest::keyRelease(window, Qt::Key_Return);
    QTest::qWait(300);
    QCOMPARE(page("window.clicks").toInt(), 0);
    QVERIFY(!evaluate("testWindow.browserUi.hint_pointer_mode").toBool());
}
void ApplicationTests::targetSearchNeedsConfirmation() {
    QVERIFY(command("bind t target-search controls"));
    key(Qt::Key_T);
    QTRY_VERIFY(evaluate("testWindow.browserUi.target_search_active").toBool());
    typeText("Activate exact button");
    QCOMPARE(page("window.clicks").toInt(), 0);
    key(Qt::Key_Return);
    QTRY_COMPARE(page("window.clicks").toInt(), 1);
    QVERIFY(!evaluate("testWindow.browserUi.target_search_active").toBool());
}
void ApplicationTests::promptAcceptCancelAndNestedRestoration() {
    QVERIFY(command("mode-enter pass-through"));
    page("setTimeout(function(){window.answer=confirm('Integration "
         "confirmation');},0); true");
    QTRY_VERIFY(
        evaluate("testWindow.pendingPageDialogRequest !== null").toBool());
    QCOMPARE(text("mode"), QString("normal"));
    QTest::qWait(100);
    key(Qt::Key_Escape);
    QTRY_VERIFY(
        evaluate("testWindow.pendingPageDialogRequest === null").toBool());
    QTRY_COMPARE(text("mode"), QString("pass-through"));
    QCOMPARE(page("window.answer").toBool(), false);
    page("setTimeout(function(){window.answer=confirm('Accept "
         "confirmation');},0); true");
    QTRY_VERIFY(
        evaluate("testWindow.pendingPageDialogRequest !== null").toBool());
    auto accept = findVisualItem(window->contentItem(), "pageDialogAccept");
    QVERIFY(accept);
    accept->forceActiveFocus();
    QTRY_VERIFY(accept->hasActiveFocus());
    key(Qt::Key_Return);
    QTRY_VERIFY(
        evaluate("testWindow.pendingPageDialogRequest === null").toBool());
    QTRY_COMPARE(text("mode"), QString("pass-through"));
    QCOMPARE(page("window.answer").toBool(), true);
    const auto outer =
        evaluate(
            "testWindow.browserUi.begin_native_prompt_lease(0, 'file-chooser')")
            .toString();
    const auto inner =
        evaluate(
            "testWindow.browserUi.begin_native_prompt_lease(0, 'page-dialog')")
            .toString();
    QVERIFY(!outer.isEmpty() && !inner.isEmpty());
    evaluate("testWindow.browserUi.complete_native_prompt_lease(" +
             quoted(inner) + ")");
    QCOMPARE(text("mode"), QString("normal"));
    evaluate("testWindow.browserUi.complete_native_prompt_lease(" +
             quoted(outer) + ")");
    QTRY_COMPARE(text("mode"), QString("pass-through"));
}
void ApplicationTests::stalePromptCannotRestoreAfterConfigChange() {
    QVERIFY(command("mode-enter pass-through"));
    const auto lease =
        evaluate(
            "testWindow.browserUi.begin_native_prompt_lease(0, 'page-dialog')")
            .toString();
    QVERIFY(!lease.isEmpty());
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('hints.marker_"
                     "scale','1.2',true)")
                .toBool());
    evaluate("testWindow.browserUi.complete_native_prompt_lease(" +
             quoted(lease) + ")");
    QCOMPARE(text("mode"), QString("normal"));
    evaluate("testWindow.browserUi.complete_native_prompt_lease(" +
             quoted(lease) + ")");
    QCOMPARE(text("mode"), QString("normal"));
}
void ApplicationTests::passThroughDocumentBoundaries() {
    QVERIFY(command("mode-enter pass-through"));
    page("location.hash='same-document';true");
    QTRY_VERIFY(text("current_url").endsWith("#same-document"));
    QTRY_COMPARE(text("mode"), QString("pass-through"));
    page("setTimeout(function(){location.href='/fixture.html?new';},0);true");
    QTRY_VERIFY(text("current_url").contains("?new"));
    QTRY_VERIFY(!evaluate("testWindow.activeWebView().loading").toBool());
    QTRY_COMPARE(text("mode"), QString("normal"));
}
void ApplicationTests::scopedGridResetAndInvalidation() {
    QVERIFY(command("bind r hint --target choose regions"));
    key(Qt::Key_R);
    const auto label = hintLabel("Scoped panel");
    QVERIFY(!label.isEmpty());
    typeText(label);
    QTRY_VERIFY(evaluate("testWindow.contextMenuItems.length > 0").toBool());
    QVERIFY(chooseCapability("grid-within"));
    QTRY_VERIFY(evaluate("testWindow.browserUi.spatial_visible").toBool());
    const auto x = evaluate("testWindow.browserUi.spatial_root_x").toDouble();
    const auto width =
        evaluate("testWindow.browserUi.spatial_root_width").toDouble();
    QVERIFY(x > 0 && width > 0);
    key(Qt::Key_5);
    QTRY_COMPARE(evaluate("testWindow.browserUi.spatial_depth").toInt(), 1);
    key(Qt::Key_Backspace);
    QTRY_COMPARE(evaluate("testWindow.browserUi.spatial_depth").toInt(), 0);
    key(Qt::Key_5);
    key(Qt::Key_0);
    QTRY_COMPARE(evaluate("testWindow.browserUi.spatial_depth").toInt(), 0);
    QCOMPARE(evaluate("testWindow.browserUi.spatial_root_x").toDouble(), x);
    QCOMPARE(evaluate("testWindow.browserUi.spatial_root_width").toDouble(),
             width);
    window->update();
    QTest::qWait(80);
    QVERIFY(evaluate("testWindow.browserUi.spatial_visible").toBool());
    page("document.querySelector('section').style.marginLeft='140px';true");
    key(Qt::Key_Return);
    QTRY_VERIFY(!evaluate("testWindow.browserUi.spatial_visible").toBool());
    QCOMPARE(page("window.scopedClicks || 0").toInt(), 0);
}
void ApplicationTests::scopedHintAndSearchJourneys_data() {
    QTest::addColumn<QString>("capability");
    QTest::newRow("hint-within") << QString("hint-within");
    QTest::newRow("search-within") << QString("search-within");
}
void ApplicationTests::scopedHintAndSearchJourneys() {
    QFETCH(QString, capability);
    QVERIFY(command("bind r hint --target choose regions"));
    key(Qt::Key_R);
    const auto label = hintLabel("Scoped panel");
    QVERIFY(!label.isEmpty());
    typeText(label);
    QTRY_VERIFY(evaluate("testWindow.contextMenuItems.length > 0").toBool());
    QVERIFY(chooseCapability(capability));
    if (capability == "search-within") {
        QTRY_VERIFY(
            evaluate("testWindow.browserUi.target_search_active").toBool());
        typeText("Scoped control");
        key(Qt::Key_Return);
    } else {
        QTRY_VERIFY(evaluate("testWindow.browserUi.hint_visible").toBool());
        const auto inner = hintLabel("Scoped control");
        QVERIFY(!inner.isEmpty());
        QVERIFY(hintLabel("Activate exact button").isEmpty());
        typeText(inner);
        QTRY_VERIFY(!evaluate("testWindow.hintRefreshPending").toBool());
        key(Qt::Key_Return);
    }
    QTRY_COMPARE(page("window.scopedClicks || 0").toInt(), 1);
    QCOMPARE(page("window.clicks").toInt(), 0);
}
void ApplicationTests::durablePersistenceFailurePreservesConfiguration() {
    QVERIFY(command("set content.zoom=1.1"));
    QVERIFY(command("bind p hint --target pointer all"));
    const auto accepted = text("config_json");
    const auto path =
        directory.filePath("durable-test/state/runtime-overrides.toml");
    QFile file(path);
    QVERIFY(file.open(QIODevice::ReadOnly));
    const auto bytes = file.readAll();
    file.close();
    QVERIFY(QFile::remove(path));
    QVERIFY(QDir().mkdir(path));
    const auto beforeFailure = text("config_json");
    key(Qt::Key_P);
    const auto label = hintLabel("Activate exact button");
    QVERIFY(!label.isEmpty());
    typeText(label);
    const auto selector =
        evaluate("testWindow.browserUi.hint_selector_policy('all')").toString();
    QTest::keyPress(window, Qt::Key_Return);
    QVERIFY(!evaluate("testWindow.browserUi.set_runtime_setting('content.zoom',"
                      "'1.25',false)")
                 .toBool());
    QCOMPARE(text("config_json"), beforeFailure);
    QCOMPARE(
        evaluate("testWindow.browserUi.hint_selector_policy('all')").toString(),
        selector);
    QTest::keyRelease(window, Qt::Key_Return);
    QTest::qWait(120);
    qInfo() << "rejected-write pointer state" << text("mode")
            << text("hint_pointer_state") << text("status_text");
    QTRY_COMPARE(page("window.clicks").toInt(), 1);
    QCOMPARE(text("config_json"), beforeFailure);
    QVERIFY(QFileInfo(path).isDir());
    QVERIFY(QDir().rmdir(path));
    QVERIFY(file.open(QIODevice::WriteOnly));
    QCOMPARE(file.write(bytes), qint64(bytes.size()));
    file.close();
    QVERIFY(command("set content.zoom=1.25"));
    QVERIFY(text("config_json") != accepted);
    QCOMPARE(QJsonDocument::fromJson(text("config_json").toUtf8())
                 .object()
                 .value("content")
                 .toObject()
                 .value("zoom")
                 .toDouble(),
             1.25);
}
void ApplicationTests::selectorValidationRejectsWholeCandidate() {
    const auto accepted = text("config_json");
    QVERIFY(
        evaluate(
            "testWindow.browserUi.set_runtime_setting('hints.selectors.all', " +
            quoted("{mode='extend',selectors=['[']}") + ",true)")
            .toBool());
    QTRY_VERIFY_WITH_TIMEOUT(
        text("status_text").contains("selector", Qt::CaseInsensitive), 6000);
    QTRY_VERIFY_WITH_TIMEOUT(
        text("status_text").contains("failed", Qt::CaseInsensitive), 6000);
    QCOMPARE(text("config_json"), accepted);
}

void ApplicationTests::nativePointerControlFamilies_data() {
    QTest::addColumn<QString>("name");
    QTest::addColumn<QString>("id");
    QTest::newRow("input") << QString("Editable field") << QString("input");
    QTest::newRow("contenteditable")
        << QString("Editable document") << QString("editor");
    QTest::newRow("same-document-link")
        << QString("Same document") << QString("same");
}
void ApplicationTests::nativePointerControlFamilies() {
    QFETCH(QString, name);
    QFETCH(QString, id);
    QVERIFY(command("bind p hint --target pointer all"));
    key(Qt::Key_P);
    const auto label = hintLabel(name);
    QVERIFY(!label.isEmpty());
    typeText(label);
    key(Qt::Key_Return);
    QTRY_COMPARE(page("window.lastClicked").toString(), id);
    QVERIFY(page("window.lastTrusted").toBool());
    QCOMPARE(page("window.allClicks").toInt(), 1);
    if (id == "input" || id == "editor") {
        QCOMPARE(page("document.activeElement.id").toString(), id);
    } else {
        QTRY_VERIFY(text("current_url").endsWith("#fragment"));
    }
}
void ApplicationTests::scopedGridNativeCoordinates_data() {
    QTest::addColumn<double>("zoom");
    QTest::newRow("80-percent") << 0.8;
    QTest::newRow("100-percent") << 1.0;
    QTest::newRow("125-percent") << 1.25;
}
void ApplicationTests::scopedGridNativeCoordinates() {
    QFETCH(double, zoom);
    evaluate("testWindow.activeWebView().zoomFactor=" + QString::number(zoom));
    window->setPosition(120, 90);
    QTest::qWait(100);
    QVERIFY(command("bind r hint --target choose regions"));
    key(Qt::Key_R);
    const auto label = hintLabel("Scoped panel");
    QVERIFY(!label.isEmpty());
    typeText(label);
    QTRY_VERIFY(evaluate("testWindow.contextMenuItems.length > 0").toBool());
    QVERIFY(chooseCapability("grid-within"));
    QTRY_VERIFY(evaluate("testWindow.browserUi.spatial_visible").toBool());
    key(Qt::Key_1);
    key(Qt::Key_Space);
    QTRY_VERIFY(evaluate("testWindow.browserUi.spatial_visible").toBool());
    QCOMPARE(evaluate("testWindow.browserUi.spatial_depth").toInt(), 0);
    key(Qt::Key_1);
    key(Qt::Key_Return);
    QTRY_COMPARE(page("window.scopedClicks || 0").toInt(), 1);
    QVERIFY(page("window.lastTrusted").toBool());
    QCOMPARE(page("window.allClicks").toInt(), 1);
    QVERIFY(page("(function(){const "
                 "r=document.getElementById('scoped').getBoundingClientRect(); "
                 "const p=window.clientPoint; return p[0]>=r.left && "
                 "p[0]<=r.right && p[1]>=r.top && p[1]<=r.bottom})()")
                .toBool());
    qInfo() << "coordinate qualification" << zoom << window->devicePixelRatio()
            << window->position();
}
void ApplicationTests::mixedPageKeysAndBrowserSequence_data() {
    QTest::addColumn<int>("value");
    QTest::addColumn<int>("modifiers");
    QTest::addColumn<QString>("domKey");
    QTest::newRow("slash") << int(Qt::Key_Slash) << 0 << QString("/");
    QTest::newRow("question") << int(Qt::Key_Question) << 0 << QString("?");
    QTest::newRow("hint-f") << int(Qt::Key_F) << 0 << QString("f");
    QTest::newRow("control-f")
        << int(Qt::Key_F) << int(Qt::ControlModifier) << QString("f");
}
void ApplicationTests::mixedPageKeysAndBrowserSequence() {
    QFETCH(int, value);
    QFETCH(int, modifiers);
    QFETCH(QString, domKey);
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('input.shortcut_"
                     "policy','mixed',true)")
                .toBool());
    QVERIFY(evaluate(
                "testWindow.browserUi.set_runtime_setting('input.page_keys', " +
                quoted("['/','?','f','Ctrl+f','g']") + ",true)")
                .toBool());
    key(value, Qt::KeyboardModifiers(modifiers));
    QTRY_COMPARE(
        page("window.keys.filter(k=>k===" + quoted(domKey) + ").length")
            .toInt(),
        1);
    QCOMPARE(text("mode"), QString("normal"));
    page("window.keys=[];true");
    key(Qt::Key_2);
    key(Qt::Key_G);
    key(Qt::Key_G);
    QCOMPARE(page("window.keys.filter(k=>k==='g').length").toInt(), 0);
    key(Qt::Key_Colon);
    QTRY_COMPARE(text("mode"), QString("command"));
    key(Qt::Key_Escape);
    QTRY_COMPARE(text("mode"), QString("normal"));
}

void ApplicationTests::pointerInvalidationBeforeRelease_data() {
    QTest::addColumn<QString>("invalidation");
    for (const auto name : {"resize", "remove-target", "tab-switch",
                            "new-document", "deactivate", "preflight-expiry"}) {
        QTest::newRow(name) << QString(name);
    }
}
void ApplicationTests::pointerInvalidationBeforeRelease() {
    QFETCH(QString, invalidation);
    QVERIFY(command("bind p hint --target pointer all"));
    key(Qt::Key_P);
    const auto label = hintLabel("Activate exact button");
    QVERIFY(!label.isEmpty());
    typeText(label);
    QTest::keyPress(window, Qt::Key_Return);
    if (invalidation == "resize")
        window->resize(window->width() + 20, window->height());
    else if (invalidation == "remove-target")
        page("document.getElementById('button').remove();true");
    else if (invalidation == "deactivate") {
        window->hide();
        QTest::qWait(80);
    } else if (invalidation == "tab-switch") {
        auto oldView =
            evaluate("testWindow.activeWebView()").value<QObject *>();
        QVERIFY(oldView);
        engine->rootContext()->setContextProperty("testOwnerView", oldView);
        evaluate("testWindow.browserUi.new_tab();testWindow.syncTabModel()");
        pageReceiver = "testOwnerView";
    } else if (invalidation == "new-document") {
        page("setTimeout(function(){location.href='/"
             "fixture.html?replacement'},0);true");
        QTRY_VERIFY(text("current_url").contains("?replacement"));
        QTRY_VERIFY(!evaluate("testWindow.activeWebView().loading").toBool());
    } else if (invalidation == "preflight-expiry") {
        evaluate("testWindow.activeWebView().runJavaScript('const "
                 "start=Date.now();while(Date.now()-start<500){};true',1)");
    }
    QTest::keyRelease(window, Qt::Key_Return);
    QTest::qWait(600);
    QCOMPARE(page("window.clicks || 0").toInt(), 0);
    QVERIFY(!evaluate("testWindow.browserUi.hint_pointer_pending").toBool());
}

void ApplicationTests::nativePointerFrameAndShadow_data() {
    QTest::addColumn<QString>("name");
    QTest::addColumn<QString>("result");
    QTest::newRow("same-origin-frame")
        << QString("Frame control")
        << QString(
               "({clicks:document.getElementById('frame').contentWindow.clicks,"
               "trust:document.getElementById('frame').contentWindow.trust})");
    QTest::newRow("open-shadow-root")
        << QString("Open shadow action")
        << QString("({clicks:window.shadowClicks,trust:window.shadowTrusted})");
}
void ApplicationTests::nativePointerFrameAndShadow() {
    QFETCH(QString, name);
    QFETCH(QString, result);
    QVERIFY(command("bind p hint --target pointer all"));
    key(Qt::Key_P);
    const auto label = hintLabel(name);
    QVERIFY(!label.isEmpty());
    typeText(label);
    key(Qt::Key_Return);
    QTRY_COMPARE(page(result).toMap().value("clicks").toInt(), 1);
    QVERIFY(page(result).toMap().value("trust").toBool());
    QCOMPARE(page("window.clicks").toInt(), 0);
}
void ApplicationTests::nativePopupActionIsOneTransport() {
    QVERIFY(command("bind p hint --target pointer links"));
    key(Qt::Key_P);
    const auto label = hintLabel("Open popup");
    QVERIFY(!label.isEmpty());
    typeText(label);
    key(Qt::Key_Return);
    QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(), 1);
    QCOMPARE(page("window.allClicks").toInt(), 1);
    QVERIFY(page("window.lastTrusted").toBool());
    evaluate("testWindow.popupWindowRegistry[0].host.destroy()");
}
void ApplicationTests::selectorReloadActivatesAtBoundary_data() {
    QTest::addColumn<QString>("selectors");
    QTest::newRow("immediate-empty") << QString("[]");
    QTest::newRow("parsed-nonempty") << QString("['#button']");
}
void ApplicationTests::selectorReloadActivatesAtBoundary() {
    QFETCH(QString, selectors);
    const auto path = directory.filePath("config.toml");
    auto write = [&](const QString &selectorSection, int font) {
        QFile file(path); QVERIFY(file.open(QIODevice::WriteOnly));
        file.write((QString("schema_version=3\n[ui]\nfont_size_pt=%1\n[navigation]\nstart_pages=['rb://blank']\n[ipc]\nenabled=false\n").arg(font) + selectorSection).toUtf8());
    };
    write("[hints.selectors.all]\nmode='replace'\nselectors=" + selectors + "\n",17);
    evaluate("testWindow.browserUi.config_path=" + quoted(path));
    QVERIFY(command("config-reload"));
    QTRY_VERIFY_WITH_TIMEOUT(text("config_json").contains("17.0"),6000);
    auto config = QJsonDocument::fromJson(text("config_json").toUtf8()).object();
    QCOMPARE(config["hints"].toObject()["selectors"].toObject()["all"].toObject()["mode"].toString(),QString("replace"));
    QVERIFY(config["ipc"].toObject()["enabled"].toBool());
    QCOMPARE(config["navigation"].toObject()["start_pages"].toArray(),QJsonArray{"rb://start"});
    QVERIFY(text("status_text").contains("2 lifecycle change(s) pending"));
    QVERIFY(command("hint all"));
    QTRY_VERIFY(text("status_text").contains("No visible hint targets") || evaluate("testWindow.hintResults.length").toInt()>0);
    QCOMPARE(evaluate("testWindow.hintResults.length").toInt(),selectors=="[]"?0:1);
    key(Qt::Key_Escape);
    // Removing the family at the same boundary restores built-in discovery.
    write("",18);
    QVERIFY(command("config-reload"));
    QTRY_VERIFY_WITH_TIMEOUT(text("config_json").contains("18.0"),6000);
    config = QJsonDocument::fromJson(text("config_json").toUtf8()).object();
    QVERIFY(config["hints"].toObject()["selectors"].toObject().isEmpty());
    QVERIFY(command("hint all"));
    QTRY_VERIFY(evaluate("testWindow.hintResults.length").toInt()>1);
}
void ApplicationTests::selectorReloadRejectsInvalidAndStaleCandidates() {
    const auto path = directory.filePath("config.toml");
    auto write = [&](const QString &selector) {
        QFile file(path); QVERIFY(file.open(QIODevice::WriteOnly));
        file.write(("schema_version=3\n[ui]\nfont_size_pt=17\n[hints.selectors.all]\nmode='replace'\nselectors=['" + selector + "']\n").toUtf8());
    };
    const auto before = text("config_json");
    const auto fingerprint = evaluate("testWindow.browserUi.hint_selector_policy('all')").toString();
    write("[");
    evaluate("testWindow.browserUi.config_path=" + quoted(path));
    QVERIFY(command("config-reload"));
    QTRY_VERIFY_WITH_TIMEOUT(text("status_text").contains("Selector validation failed"),6000);
    QCOMPARE(text("config_json"),before);
    QCOMPARE(evaluate("testWindow.browserUi.hint_selector_policy('all')").toString(),fingerprint);
    auto validator = window->findChild<QObject *>("selectorValidationSurface");
    QVERIFY(validator); QTRY_VERIFY(validator->property("documentReady").toBool());
    QVERIFY(validator->setProperty("documentReady",false));
    write("button[data-later]"); QVERIFY(command("config-reload"));
    QTRY_VERIFY_WITH_TIMEOUT(!text("selector_validation_request").isEmpty(),6000);
    const auto generation = QJsonDocument::fromJson(text("selector_validation_request").toUtf8()).object()["generation"].toString();
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('hints.marker_scale','1.2',true)").toBool());
    const auto newer = text("config_json");
    evaluate("testWindow.browserUi.complete_selector_validation(" + quoted(generation) + ",'{\"ok\":true}')");
    QCOMPARE(text("config_json"),newer);
    QVERIFY(!newer.contains("replace"));
}
void ApplicationTests::selectorReloadTimeoutPreservesConfiguration() {
    auto validator = window->findChild<QObject *>("selectorValidationSurface");
    QVERIFY(validator); QTRY_VERIFY(validator->property("documentReady").toBool());
    QVERIFY(validator->setProperty("documentReady",false));
    const auto before = text("config_json");
    const auto policy = evaluate("testWindow.browserUi.hint_selector_policy('all')").toString();
    const auto path = directory.filePath("timeout.toml");
    QFile file(path); QVERIFY(file.open(QIODevice::WriteOnly));
    file.write("schema_version=3\n[ui]\nfont_size_pt=23\n[hints.selectors.all]\nmode='replace'\nselectors=['#button']\n");file.close();
    evaluate("testWindow.browserUi.config_path=" + quoted(path));
    QVERIFY(command("config-reload"));
    QTRY_VERIFY_WITH_TIMEOUT(!text("selector_validation_request").isEmpty(),6000);
    QTRY_VERIFY_WITH_TIMEOUT(text("status_text").contains("timed out"),7000);
    QCOMPARE(text("config_json"),before);
    QCOMPARE(evaluate("testWindow.browserUi.hint_selector_policy('all')").toString(),policy);
}
void ApplicationTests::selectorDiscoveryUsesProductionPolicy_data() {
    QTest::addColumn<QString>("policy");
    for (const auto &name : {"inherit", "extend", "replace", "disabled"})
        QTest::newRow(name) << QString(name);
}
void ApplicationTests::selectorDiscoveryUsesProductionPolicy() {
    QFETCH(QString, policy);
    QVERIFY(page("var custom=document.createElement('div');custom.className='custom';custom.textContent='Custom discovery';custom.style='position:fixed;left:530px;top:410px;width:220px;height:32px';document.body.append(custom);true").toBool());
    QTRY_VERIFY(text("selector_validation_request").isEmpty());
    QVERIFY(command("hint all"));
    QTRY_VERIFY(!evaluate("testWindow.hintRefreshPending").toBool());
    // Wait for Rust to publish the collection, including an explicitly empty one.
    QTRY_VERIFY(text("status_text").contains("No visible hint targets") || evaluate("testWindow.hintResults.length").toInt() > 0);
    const auto rows = QJsonDocument::fromJson(evaluate("JSON.stringify(testWindow.hintResults)").toString().toUtf8()).array();
    bool customFound = false, buttonFound = false;
    for (const auto &row : rows) {
        const auto text = row.toObject().value("text").toString();
        customFound |= text == "Custom discovery";
        buttonFound |= text == "Activate exact button";
    }
    QCOMPARE(customFound, policy == "extend");
    QCOMPARE(buttonFound, policy != "disabled");
    if (policy == "disabled") QCOMPARE(rows.size(), 0);
    if (policy == "replace") QCOMPARE(rows.size(), 1);
}
void ApplicationTests::startupSelectorValidation_data() {
    QTest::addColumn<bool>("valid");
    QTest::newRow("accepted") << true;
    QTest::newRow("rejected") << false;
}
void ApplicationTests::startupSelectorValidation() {
    QFETCH(bool, valid);
    const auto config =
        QJsonDocument::fromJson(text("config_json").toUtf8()).object();
    QCOMPARE(config.value("ui").toObject().value("font_size_pt").toDouble(),
             valid ? 15.0 : 10.0);
    QCOMPARE(config.value("hints")
                 .toObject()
                 .value("selectors")
                 .toObject()
                 .isEmpty(),
             !valid);
}
void ApplicationTests::staleSelectorCallbackCannotPublish() {
    auto validator = window->findChild<QObject *>("selectorValidationSurface");
    QVERIFY(validator);
    QTRY_VERIFY(validator->property("documentReady").toBool());
    QVERIFY(validator->setProperty("documentReady", false));
    QVERIFY(
        evaluate(
            "testWindow.browserUi.set_runtime_setting('hints.selectors.all'," +
            quoted("{mode='extend',selectors=['button[data-queued]']}") +
            ",true)")
            .toBool());
    const auto pending =
        QJsonDocument::fromJson(text("selector_validation_request").toUtf8())
            .object();
    const auto generation = pending.value("generation").toString();
    QVERIFY(!generation.isEmpty());
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('content.zoom','"
                     "1.2',true)")
                .toBool());
    const auto accepted = text("config_json");
    evaluate("testWindow.browserUi.complete_selector_validation(" +
             quoted(generation) + ",'{\"ok\":true}')");
    QCOMPARE(text("config_json"), accepted);
    QVERIFY(text("status_text").contains("stale", Qt::CaseInsensitive));
}
void ApplicationTests::unavailableSelectorDocumentTimesOut() {
    auto validator = window->findChild<QObject *>("selectorValidationSurface");
    QVERIFY(validator);
    QTRY_VERIFY(validator->property("documentReady").toBool());
    QVERIFY(validator->setProperty("documentReady", false));
    const auto old = text("config_json");
    QVERIFY(
        evaluate(
            "testWindow.browserUi.set_runtime_setting('hints.selectors.all'," +
            quoted("{mode='extend',selectors=['button[data-timeout]']}") +
            ",true)")
            .toBool());
    QTRY_VERIFY_WITH_TIMEOUT(text("selector_validation_request").isEmpty(),
                             6500);
    QCOMPARE(text("config_json"), old);
    QVERIFY(text("status_text").contains("timed out", Qt::CaseInsensitive));
}

void ApplicationTests::productionHintLifetimeCycles() {
    if (!qEnvironmentVariableIsSet("FERRIC_APP_STRESS"))
        QSKIP("separate opt-in application_lifetime gate");
    auto cycle = [this] {
        QTest::keyClick(window, Qt::Key_F);
        QElapsedTimer timer;
        timer.start();
        while (!evaluate("testWindow.browserUi.hint_visible").toBool() &&
               timer.elapsed() < 5000)
            QTest::qWait(1);
        if (!evaluate("testWindow.browserUi.hint_visible").toBool())
            return false;
        QTest::keyClick(window, Qt::Key_Escape);
        QTest::qWait(1);
        QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
        return text("mode") == "normal";
    };
    for (int warm = 0; warm < 10; ++warm)
        QVERIFY(cycle());
    QTest::qWait(100);
    const auto baseline = window->findChildren<QObject *>().size();
    for (int index = 0; index < 1000; ++index)
        QVERIFY2(cycle(), qPrintable(QString("cycle %1").arg(index)));
    QTest::qWait(100);
    QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
    const auto snapshot =
        page("(function(){const s=window.__ferric_browserHintState; const "
             "hasGc=typeof gc==='function'; if(hasGc)gc();return "
             "{gc:hasGc,elements:s.elements.size,observers:s.observers.length,"
             "windows:s.windows.length,scope:!!s.scopeLease,tracking:s."
             "tracking};})()",
             1)
            .toMap();
    QVERIFY(snapshot.value("gc").toBool());
    QCOMPARE(snapshot.value("elements").toInt(), 0);
    QCOMPARE(snapshot.value("observers").toInt(), 0);
    QCOMPARE(snapshot.value("windows").toInt(), 0);
    QVERIFY(!snapshot.value("scope").toBool());
    QVERIFY(!snapshot.value("tracking").toBool());
    const auto finalCount = window->findChildren<QObject *>().size();
    qInfo() << "1000 production Hint cycles, Qt objects" << baseline
            << finalCount << snapshot;
    QCOMPARE(finalCount, baseline);
}

void ApplicationTests::rendererLossCancelsHeldConfirmation() {
    QVERIFY(command("bind p hint --target pointer all"));
    key(Qt::Key_P);
    const auto label = hintLabel("Activate exact button");
    QVERIFY(!label.isEmpty());
    typeText(label);
    QTest::keyPress(window, Qt::Key_Return);
    QTRY_COMPARE(text("hint_pointer_state"),
                 QString("awaiting-confirmation-release"));
    const auto pid =
        evaluate("testWindow.activeWebView().renderProcessPid").toLongLong();
    QVERIFY(pid > 1 && pid != getpid());
    // The public PID belongs to this disposable view's renderer, never a
    // desktop browser or external application.
    QFile commandLine(QString("/proc/%1/cmdline").arg(pid));
    QVERIFY(commandLine.open(QIODevice::ReadOnly));
    QVERIFY(commandLine.readAll().contains("--type=renderer"));
    QCOMPARE(::kill(static_cast<pid_t>(pid), SIGTERM), 0);
    QTRY_VERIFY_WITH_TIMEOUT(
        evaluate("testWindow.rendererFailureVisible").toBool(), 10000);
    QVERIFY(!evaluate("testWindow.browserUi.hint_pointer_mode").toBool());
    QTest::keyRelease(window, Qt::Key_Return);
    evaluate("testWindow.reloadRendererFailure()");
    QTRY_VERIFY_WITH_TIMEOUT(
        evaluate("testWindow.activeWebView().renderProcessPid").toLongLong() >
                1 &&
            evaluate("testWindow.activeWebView().renderProcessPid")
                    .toLongLong() != pid,
        15000);
    QTRY_VERIFY_WITH_TIMEOUT(
        !evaluate("testWindow.activeWebView().loading").toBool(), 15000);
    QTRY_VERIFY_WITH_TIMEOUT(
        page("document.getElementById('button') !== null").toBool(), 15000);
    QCOMPARE(page("window.clicks").toInt(), 0);
    QCOMPARE(page("window.allClicks || 0").toInt(), 0);
    QCOMPARE(text("mode"), QString("normal"));
}

void ApplicationTests::heldPageKeyRetainsOwnerAcrossPolicyChange() {
    QVERIFY(command("set --temp input.shortcut_policy=mixed"));
    QVERIFY(command("set --temp \"input.page_keys=['f']\""));
    page("window.heldEvents=[];for(const kind of "
         "['keydown','keyup'])document.addEventListener(kind,e=>window."
         "heldEvents.push({kind:kind,key:e.key,trusted:e.isTrusted}));true");
    QTest::keyPress(window, Qt::Key_F);
    QTRY_COMPARE(page("window.heldEvents.filter(e=>e.kind==='keydown'&&e.key==="
                      "'f').length")
                     .toInt(),
                 1);
    QVERIFY(command("set --temp input.page_keys=[]"));
    QTest::keyRelease(window, Qt::Key_F);
    QTRY_COMPARE(
        page(
            "window.heldEvents.filter(e=>e.kind==='keyup'&&e.key==='f').length")
            .toInt(),
        1);
    QCOMPARE(text("mode"), QString("normal"));
    QVERIFY(page("window.heldEvents.every(e=>e.trusted)").toBool());
    key(Qt::Key_F);
    QTRY_COMPARE(text("mode"), QString("hint"));
    QCOMPARE(page("window.heldEvents.filter(e=>e.key==='f').length").toInt(),
             2);
    key(Qt::Key_Escape);
    QTRY_COMPARE(text("mode"), QString("normal"));
}

void ApplicationTests::pointerObservationIntervalExperiment() {
    QVERIFY(command("bind p hint --target pointer all"));
    QJsonArray observations;
    for (int run = 0; run < 20; ++run) {
        QVERIFY(
            page(QString(
                     "(function(){document.querySelectorAll('.race-target')."
                     "forEach(e=>e."
                     "remove());"
                     "window.raceReceiver='';window.raceClicks=0;window."
                     "allClicks=0;window.raceOver=0;"
                     "const "
                     "b=document.createElement('button');b.className='race-"
                     "target';b.textContent='Race target';"
                     "b.style='position:fixed;left:%1px;top:350px;width:220px;"
                     "height:32px';"
                     "b.onclick=e=>{window.raceReceiver='original';window."
                     "raceClicks++;window.raceTrusted=e.isTrusted};"
                     "b.onpointerover=()=>{window.raceOver++;const "
                     "c=b.cloneNode(true);"
                     "c.onclick=e=>{window.raceReceiver='replacement';window."
                     "raceClicks++;window.raceTrusted=e.isTrusted};"
                     "b.replaceWith(c)};document.body.append(b);return "
                     "true;})()")
                     .arg(run % 2 ? 600 : 300))
                .toBool());
        key(Qt::Key_P);
        const auto label = hintLabel("Race target");
        QVERIFY(!label.isEmpty());
        typeText(label);
        key(Qt::Key_Return);
        QTRY_VERIFY(
            !evaluate("testWindow.browserUi.hint_pointer_mode").toBool());
        QTest::qWait(150);
        const auto fact = page("JSON.stringify({receiver:window.raceReceiver,"
                               "clicks:window.raceClicks,"
                               "allClicks:window.allClicks,over:window."
                               "raceOver,trusted:!!window.raceTrusted})")
                              .toString();
        const auto row = QJsonDocument::fromJson(fact.toUtf8()).object();
        qInfo() << "Interval trial" << run << fact;
        QVERIFY(row.value("clicks").toInt() <= 1);
        QVERIFY(row.value("allClicks").toInt() <= 1);
        if (row.value("clicks").toInt() == 1)
            QVERIFY(row.value("trusted").toBool());
        observations.append(row);
    }
    qInfo().noquote()
        << "FERRIC_POINTER_INTERVAL_JSON"
        << QJsonDocument(observations).toJson(QJsonDocument::Compact);
    // This deliberately measures the admitted DOM/native interval. A changed
    // receiver is residual uncertainty, never proof of an atomic transaction.
}

void ApplicationTests::popupWithoutUserActivationIsDenied() {
    page("window.open('/popup.html','_blank');true");
    QTest::qWait(200);
    QCOMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(), 0);
    QCOMPARE(page("window.allClicks || 0").toInt(), 0);
}
void ApplicationTests::fullscreenApiOutcomes_data() {
    QTest::addColumn<bool>("enabled");
    QTest::newRow("Qt-feature-disabled") << false;
    QTest::newRow("Qt-feature-enabled") << true;
}
void ApplicationTests::fullscreenApiOutcomes() {
    QFETCH(bool, enabled);
    evaluate(
        QString(
            "testWindow.activeWebView().settings.fullScreenSupportEnabled=") +
        (enabled ? "true" : "false"));
    page("const b=document.createElement('button');b.textContent='Fullscreen "
         "trigger';b.id='api';b.style='position:fixed;left:530px;top:410px;"
         "width:220px;height:32px';b.onclick=()=>document.documentElement."
         "requestFullscreen().then(()=>window.apiOutcome='allowed').catch(e=>{"
         "window.apiError=String(e);window.apiOutcome='denied'});document.body."
         "append(b);true");
    QVERIFY(command("bind p hint --target pointer all"));
    key(Qt::Key_P);
    const auto label = hintLabel("Fullscreen trigger");
    QVERIFY(!label.isEmpty());
    typeText(label);
    key(Qt::Key_Return);
    qInfo() << "API dispatch" << text("mode") << text("hint_pointer_state")
            << text("status_text")
            << page("JSON.stringify({clicks:window.allClicks,outcome:window."
                    "apiOutcome,error:window.apiError,focus:document.hasFocus()"
                    ",activation:navigator.userActivation.isActive})");
    QTRY_COMPARE(page("window.allClicks").toInt(), 1);
    qInfo() << "page API focus" << page("document.hasFocus()").toBool()
            << text("status_text");
    QTRY_COMPARE(page("window.apiOutcome").toString(),
                 enabled ? QString("allowed") : QString("denied"));
    QVERIFY(page("window.lastTrusted").toBool());
    QCOMPARE(page("window.allClicks").toInt(), 1);
    if (enabled) {
        QTRY_COMPARE(window->visibility(), QWindow::FullScreen);
        page("document.exitFullscreen();true");
        QTRY_VERIFY(window->visibility() != QWindow::FullScreen);
    }
}
void ApplicationTests::clipboardPermissionOutcomes_data() {
    QTest::addColumn<QString>("permission");
    QTest::newRow("denied") << QString("deny");
    QTest::newRow("allowed") << QString("allow");
}
void ApplicationTests::clipboardPermissionOutcomes() {
    if (QGuiApplication::platformName() != "offscreen")
        QSKIP("clipboard test uses the offscreen process-local clipboard");
    QFETCH(QString, permission);
    QGuiApplication::clipboard()->setText("fixture-clipboard");
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('permissions."
                     "clipboard'," +
                     quoted(permission) + ",true)")
                .toBool());
    page("const b=document.createElement('button');b.textContent='Clipboard "
         "trigger';b.id='api';b.style='position:fixed;left:530px;top:410px;"
         "width:220px;height:32px';b.onclick=()=>navigator.clipboard.readText()"
         ".then(v=>{window.clipValue=v;window.apiOutcome='allowed'}).catch(e=>{"
         "window.apiError=String(e);window.apiOutcome='denied'});document.body."
         "append(b);true");
    QVERIFY(command("bind p hint --target pointer all"));
    key(Qt::Key_P);
    const auto label = hintLabel("Clipboard trigger");
    QVERIFY(!label.isEmpty());
    typeText(label);
    key(Qt::Key_Return);
    qInfo() << "API dispatch" << text("mode") << text("hint_pointer_state")
            << text("status_text")
            << page("JSON.stringify({clicks:window.allClicks,outcome:window."
                    "apiOutcome,error:window.apiError,focus:document.hasFocus()"
                    ",activation:navigator.userActivation.isActive})");
    QTRY_COMPARE(page("window.allClicks").toInt(), 1);
    qInfo() << "page API focus" << page("document.hasFocus()").toBool()
            << text("status_text");
    QTRY_COMPARE(page("window.apiOutcome").toString(), permission == "allow"
                                                           ? QString("allowed")
                                                           : QString("denied"));
    QVERIFY(page("window.lastTrusted").toBool());
    QCOMPARE(page("window.allClicks").toInt(), 1);
    if (permission == "allow")
        QCOMPARE(page("window.clipValue").toString(),
                 QString("fixture-clipboard"));
}

extern "C" bool cxx_qt_init_crate_ferric_browser_engine_qt();

int main(int argc, char **argv) {
    ferric_browser_enable_software_rendering();
    ferric_browser_register_internal_scheme();
    QQuickWindow::setGraphicsApi(QSGRendererInterface::Software);
    QtWebEngineQuick::initialize();
    cxx_qt_init_crate_ferric_browser_engine_qt();
    QGuiApplication application(argc, argv);
    ApplicationTests tests;
    return QTest::qExec(&tests, argc, argv);
}

void ApplicationTests::hintRefreshRetainsInput() {
    QVERIFY(command("set --temp hints.min_chars=2"));
    QVERIFY(command("set --temp hints.auto_follow=never"));
    QVERIFY(command("hint buttons"));
    const auto label=hintLabel("Activate exact button");
    QCOMPARE(label.size(),2);
    QTRY_VERIFY(!evaluate("testWindow.hintRefreshPending").toBool());
    typeText(label.left(1));
    QCOMPARE(evaluate("testWindow.hintState.prefix").toString(),label.left(1));
    const auto revision=evaluate("testWindow.hintDirtyRevision").toInt();
    page("document.body.setAttribute('data-unrelated','refresh');true");
    QTRY_VERIFY(evaluate("testWindow.hintDirtyRevision").toInt()>revision);
    QTRY_VERIFY(!evaluate("testWindow.hintRefreshPending").toBool());
    qInfo().noquote()<<"REFRESH_PREFIX before="<<label.left(1)<<"after="<<evaluate("testWindow.hintState.prefix").toString();
    QCOMPARE(evaluate("testWindow.hintState.prefix").toString(),label.left(1));
    QCOMPARE(hintLabel("Activate exact button"),label);
    key(Qt::Key_Slash); typeText("activate");
    QCOMPARE(evaluate("testWindow.hintState.mode").toString(),QString("text"));
    QCOMPARE(evaluate("testWindow.hintState.query").toString(),QString("activate"));
    const auto textRevision=evaluate("testWindow.hintDirtyRevision").toInt();
    page("document.body.setAttribute('data-unrelated','refresh-again');true");
    QTRY_VERIFY(evaluate("testWindow.hintDirtyRevision").toInt()>textRevision);
    QTRY_VERIFY(!evaluate("testWindow.hintRefreshPending").toBool());
    qInfo().noquote()<<"REFRESH_TEXT after="<<evaluate("JSON.stringify(testWindow.hintState)").toString();
    QCOMPARE(evaluate("testWindow.hintState.mode").toString(),QString("text"));
    QCOMPARE(evaluate("testWindow.hintState.query").toString(),QString("activate"));
}
void ApplicationTests::hintRefreshDoesNotAutoFollow() {
    QVERIFY(command("set --temp hints.auto_follow=always"));
    // Exactly two top-level buttons; changing the DOM must not count as typing.
    page("document.body.innerHTML='<button id=remaining onclick=\"window.clicks++\">Remaining</button><button id=removed>Removed</button>';window.clicks=0;true");
    QVERIFY(command("hint buttons"));
    QVERIFY(!hintLabel("Remaining").isEmpty());
    QTRY_COMPARE(evaluate("testWindow.hintResults.length").toInt(),2);
    QCOMPARE(page("window.clicks").toInt(),0);
    page("document.getElementById('removed').remove();true");
    QTest::qWait(350);
    QCOMPARE(page("window.clicks").toInt(),0);
    key(Qt::Key_Return);
    QTRY_COMPARE(page("window.clicks").toInt(),1);
    qInfo()<<"EXPLICIT_AUTO_FOLLOW clicks_after_enter="<<page("window.clicks").toInt();
}

void ApplicationTests::rapidHintRejectsConsumedReplay() {
    QVERIFY(command("hint --rapid --target yank links"));
    const auto label=hintLabel("Same document");
    QVERIFY(!label.isEmpty());
    const auto candidate=evaluate("JSON.stringify(testWindow.hintResults.find(r=>r.label==="+quoted(label)+"))").toString();
    const auto fresh=page("("+evaluate("testWindow.hintFreshScript(JSON.parse("+quoted(candidate)+"))").toString()+")",1);
    const auto raw=QString::fromUtf8(QJsonDocument::fromVariant(fresh).toJson(QJsonDocument::Compact));
    const auto results=evaluate("JSON.stringify([JSON.parse(testWindow.browserUi.select_hint("+quoted(label)+","+quoted(raw)+")),JSON.parse(testWindow.browserUi.select_hint("+quoted(label)+","+quoted(raw)+"))])").toString();
    const auto rows=QJsonDocument::fromJson(results.toUtf8()).array();
    QCOMPARE(rows.size(),2);
    QCOMPARE(rows[0].toObject().value("action").toString(),QString("yank"));
    QCOMPARE(rows[1].toObject().value("error").toString(),QString("consumed-target"));
}

void ApplicationTests::hintConfirmationFreezesPendingRefresh() {
    QVERIFY(command("bind p hint --target pointer all"));
    key(Qt::Key_P);
    const auto label=hintLabel("Activate exact button");
    QVERIFY(!label.isEmpty());
    typeText(label);
    QTRY_VERIFY(!evaluate("testWindow.hintRefreshPending").toBool());
    QVERIFY(evaluate("testWindow.startHintCollection();testWindow.hintRefreshPending").toBool());
    QTest::keyPress(window,Qt::Key_Return);
    QCOMPARE(text("hint_pointer_state"),QString("awaiting-confirmation-release"));
    // Complete the older callback while the user's explicit gesture is held.
    QTest::qWait(180);
    QCOMPARE(text("hint_pointer_state"),QString("awaiting-confirmation-release"));
    QCOMPARE(evaluate("testWindow.hintState.prefix").toString(),label);
    QCOMPARE(page("window.clicks").toInt(),0);
    QTest::keyRelease(window,Qt::Key_Return);
    QTRY_COMPARE(page("window.clicks").toInt(),1);
    QVERIFY(page("window.trust").toBool());
    QCOMPARE(page("window.allClicks").toInt(),1);
}

void ApplicationTests::hintCollisionCyclesActualMarkers() {
    QVERIFY(command("set --temp hints.auto_follow=never"));
    page("document.body.innerHTML='<div role=button aria-label=\"Overlapping control\" style=\"position:absolute;left:40px;top:0;width:16px;height:16px\" onclick=\"window.clicks++\"><button aria-label=\"Overlapping control\" style=\"position:absolute;left:0;top:0;width:100%;height:100%;padding:0;margin:0;border:0;box-sizing:border-box;font-size:10px\" onclick=\"event.stopPropagation();window.clicks++\"></button></div>';window.clicks=0;true");
    QVERIFY(command("hint buttons"));
    QVERIFY(!hintLabel("Overlapping control").isEmpty());
    QTRY_VERIFY(!evaluate("testWindow.hintRefreshPending").toBool());
    const auto before=evaluate("testWindow.hintState.active_label").toString();
    QCOMPARE(evaluate("testWindow.hintCollisionLabels("+quoted(before)+").length").toInt(),2);
    key(Qt::Key_Space);
    QVERIFY(evaluate("testWindow.hintState.active_label").toString()!=before);
    auto panel=window->findChild<QObject*>("hintStackPanel");
    QVERIFY(panel);
    QTRY_VERIFY(panel->property("visible").toBool());
    const auto revision=evaluate("testWindow.hintDirtyRevision").toInt();
    page("document.body.dataset.unrelated='changed';true");
    QTRY_VERIFY(evaluate("testWindow.hintDirtyRevision").toInt()>revision);
    QTRY_VERIFY(!evaluate("testWindow.hintRefreshPending").toBool());
    QVERIFY(panel->property("visible").toBool());
    QCOMPARE(page("window.clicks").toInt(),0);
    key(Qt::Key_Return);
    QTRY_COMPARE(page("window.clicks").toInt(),1);
}

void ApplicationTests::clipboardChrome() {
    QGuiApplication::clipboard()->setText("fixture-browser-input");
    key(Qt::Key_O);
    QTRY_VERIFY(text("mode")=="command");
    auto input=window->findChild<QQuickItem *>("commandLineInput");QVERIFY(input);
    QTRY_VERIFY(input->hasActiveFocus());
    input->setProperty("text","open ");input->setProperty("cursorPosition",5);
    key(Qt::Key_V, Qt::ControlModifier);
    QTest::qWait(100);
    QCOMPARE(input->property("text").toString(),
             QString("open fixture-browser-input"));
    QCOMPARE(text("mode"), QString("command"));
    QMetaObject::invokeMethod(input, "selectAll");
    QGuiApplication::clipboard()->setText("fixture-before");
    key(Qt::Key_C, Qt::ControlModifier);
    QTest::qWait(100);
    QCOMPARE(QGuiApplication::clipboard()->text(),
             QString("open fixture-browser-input"));
    input->setProperty("text", "open ");
    input->setProperty("cursorPosition", 5);
    key(Qt::Key_V, Qt::ControlModifier | Qt::ShiftModifier);
    QCOMPARE(input->property("text").toString(),
             QString("open open fixture-browser-input"));
}

void ApplicationTests::clipboardMenus() {
  page("document.body.innerHTML='<p id=parent tabindex=0>Parent remembered "
       "selection</p><input id=field value=Field-copy><div id=rich "
       "tabindex=0><b>Rich</b> selection</div><iframe id=child src=/child.html "
       "style=\"width:420px;height:150px\"></iframe>';true");
  QTRY_VERIFY(page("!!document.getElementById('child').contentDocument?.body")
                  .toBool());
  const QStringList targets{"field", "rich", "same-origin-child",
                            "cross-origin-child"};
  QTcpServer foreign;
  QVERIFY(foreign.listen(QHostAddress::LocalHost, 0));
  connect(&foreign, &QTcpServer::newConnection, this, [&foreign] {
    while (auto socket = foreign.nextPendingConnection()) {
      QObject::connect(socket, &QTcpSocket::readyRead, socket, [socket] {
        socket->readAll();
        const QByteArray html =
            "<!doctype html><p id=childText tabindex=0 "
            "style='margin:8px;font:16px sans-serif'>Child selected "
            "text</p><script>onload=()=>{let "
            "e=document.getElementById('childText');e.focus();let "
            "r=document.createRange();r.selectNodeContents(e);let "
            "s=getSelection();s.removeAllRanges();s.addRange(r);}</script>";
        socket->write("HTTP/1.1 200 OK\r\nContent-Type: "
                      "text/html\r\nConnection: close\r\nContent-Length: " +
                      QByteArray::number(html.size()) + "\r\n\r\n" + html);
        socket->disconnectFromHost();
      });
      QObject::connect(socket, &QTcpSocket::disconnected, socket,
                       &QObject::deleteLater);
    }
  });
  for (const auto &target : targets) {
    evaluate("testWindow.browserUi.escape();testWindow.activeWebView()."
             "forceActiveFocus()");
    QString pointScript;
    if (target.contains("child")) {
      page("(()=>{let e=document.getElementById('parent');e.focus();let "
           "r=document.createRange();r.selectNodeContents(e);let "
           "s=getSelection();s.removeAllRanges();s.addRange(r);return "
           "true;})()");
      if (target == "cross-origin-child") {
        page("document.getElementById('child').src=" +
             quoted(QString("http://127.0.0.1:%1/foreign")
                        .arg(foreign.serverPort())) +
             ";true");
        QTest::qWait(500);
      } else {
        page("(()=>{let "
             "f=document.getElementById('child'),d=f.contentDocument;d.body."
             "innerHTML='<p id=childText tabindex=0 "
             "style=\"margin:8px;font:16px sans-serif\">Child selected "
             "text</p>';let e=d.getElementById('childText');e.focus();let "
             "r=d.createRange();r.selectNodeContents(e);let "
             "s=f.contentWindow.getSelection();s.removeAllRanges();s.addRange("
             "r);return true;})()");
      }
      pointScript = "(()=>{let "
                    "r=document.getElementById('child').getBoundingClientRect()"
                    ";return {x:r.left+40,y:r.top+20};})()";
    } else {
      page("(()=>{let e=document.getElementById(" + quoted(target) +
           ");e.focus();if(e.tagName==='INPUT')e.select();else{let "
           "r=document.createRange();r.selectNodeContents(e);let "
           "s=getSelection();s.removeAllRanges();s.addRange(r);}return "
           "true;})()");
      pointScript =
          "(()=>{let r=document.getElementById(" + quoted(target) +
          ").getBoundingClientRect();return {x:r.left+25,y:r.top+16};})()";
    }
    QTest::qWait(100);
    auto view = qobject_cast<QQuickItem *>(
        evaluate("testWindow.activeWebView()").value<QObject *>());
    QVERIFY(view);
    const auto local = page(pointScript).toMap();
    const auto point =
        view->mapToScene(QPointF(local["x"].toDouble(), local["y"].toDouble()));
    QGuiApplication::clipboard()->setText("fixture-before");
    QTest::mouseClick(window, Qt::RightButton, Qt::NoModifier, point.toPoint());
    QTRY_VERIFY_WITH_TIMEOUT(
        evaluate("testWindow.pendingContextMenuRequest!==null").toBool(), 3000);
    QTRY_VERIFY_WITH_TIMEOUT(
        evaluate("testWindow.pendingContextMenuSelection !== null").toBool(),
        3000);
    const auto selected =
        evaluate(
            "String(testWindow.pendingContextMenuRequest.selectedText||'')")
            .toString();
    const bool copyOffered =
        evaluate("testWindow.contextMenuItems.some(function(i){return "
                 "i.action==='web-copy'})")
            .toBool();
    if (copyOffered)
      evaluate("testWindow.activateContextMenuItem(testWindow.contextMenuItems."
               "filter(function(i){return i.action==='web-copy'})[0])");
    else
      evaluate("testWindow.clearContextMenuRequest()");
    QTest::qWait(300);
    QVERIFY(copyOffered);
    QCOMPARE(QGuiApplication::clipboard()->text(), selected);
    QCOMPARE(QGuiApplication::clipboard()->mimeData()->hasHtml(),
             target != "field");
    QVERIFY(text("status_text") == "Copy requested" ||
            (target == "field" && text("status_text") == "Insert mode"));
  }
}

void ApplicationTests::clipboardNativeEditing_data() {
  QTest::addColumn<QString>("context");
  QTest::addColumn<QString>("mode");
  QTest::addColumn<QString>("operation");
  for (const auto &context :
       {QString("input"), QString("area"), QString("rich"), QString("plain")})
    for (const auto &mode :
         {QString("normal"), QString("insert"), QString("insert-locked"),
          QString("passthrough"), QString("passthrough-once")})
      for (const auto &operation : {QString("copy"), QString("cut"),
                                    QString("paste"), QString("paste-plain")}) {
        if (context == "plain" && operation != "copy")
          continue;
        const auto name = context + "-" + mode + "-" + operation;
        QTest::newRow(qPrintable(name)) << context << mode << operation;
      }
}
void ApplicationTests::clipboardNativeEditing() {
  QFETCH(QString, context);
  QFETCH(QString, mode);
  QFETCH(QString, operation);
  page("document.body.innerHTML='<input id=input><textarea "
       "id=area></textarea><div id=rich contenteditable=true></div><p id=plain "
       "tabindex=0></p>';window.edits=[];for(const kind of "
       "['copy','cut','paste'])document.addEventListener(kind,e=>window.edits."
       "push(kind),true);true");
  evaluate("testWindow.activeWebView().forceActiveFocus()");
  const bool selected = operation == "copy" || operation == "cut";
  page(
      "(()=>{let e=document.getElementById(" + quoted(context) +
      ");e.focus();if('value' in e){e.value='Alpha beta';e.setSelectionRange(" +
      (selected ? QString("0,10") : QString("10,10")) +
      ");}else{e.innerHTML='<b>Alpha</b> beta';let "
      "s=getSelection(),r=document.createRange();r.selectNodeContents(e);" +
      (selected ? QString() : QString("r.collapse(false);")) +
      "s.removeAllRanges();s.addRange(r);}return true;})()");
  evaluate("testWindow.browserUi.escape()");
  if (mode != "normal")
    QVERIFY(command("mode-enter " + mode));
  const auto before = text("mode");
  auto mime = new QMimeData;
  mime->setText("fixture-paste");
  mime->setHtml("<b>fixture-paste</b>");
  QGuiApplication::clipboard()->setMimeData(mime);
  key(operation == "copy"  ? Qt::Key_C
      : operation == "cut" ? Qt::Key_X
                           : Qt::Key_V,
      operation == "paste-plain" ? (Qt::ControlModifier | Qt::ShiftModifier)
                                 : Qt::ControlModifier);
  const auto expected =
      selected ? (operation == "cut" ? QString("") : QString("Alpha beta"))
               : QString("Alpha betafixture-paste");
  QTRY_COMPARE(page("(()=>{let e=document.getElementById(" + quoted(context) +
                    ");return 'value' in e?e.value:e.textContent;})()")
                   .toString(),
               expected);
  if (selected)
    QTRY_COMPARE(QGuiApplication::clipboard()->text(), QString("Alpha beta"));
  if (mode == "passthrough-once")
    QTRY_COMPARE(text("mode"), QString("normal"));
  else
    QCOMPARE(text("mode"), before);
  QCOMPARE(page("window.edits.length").toInt(), 1);
  if (context == "rich" && operation == "paste-plain")
    QVERIFY(!page("document.getElementById('rich').innerHTML")
                 .toString()
                 .contains("<b>fixture-paste"));
  if (selected && (context == "rich" || context == "plain"))
    QVERIFY(QGuiApplication::clipboard()->mimeData()->hasHtml());
}
void ApplicationTests::clipboardSelectionOwnership() {
  page("document.body.innerHTML='<p id=parent tabindex=0>Parent "
       "selection</p><iframe id=child src=/child.html></iframe>';true");
  QTRY_VERIFY(page("!!document.getElementById('child').contentDocument?.body")
                  .toBool());
  evaluate("testWindow.activeWebView().forceActiveFocus()");
  page("(()=>{let p=document.getElementById('parent');p.focus();let "
       "r=document.createRange();r.selectNodeContents(p);getSelection()."
       "removeAllRanges();getSelection().addRange(r);let "
       "f=document.getElementById('child'),d=f.contentDocument;d.body."
       "innerHTML='<p id=text tabindex=0>Child selection</p>';let "
       "e=d.getElementById('text');e.focus();let "
       "c=d.createRange();c.selectNodeContents(e);f.contentWindow.getSelection("
       ").removeAllRanges();f.contentWindow.getSelection().addRange(c);return "
       "true;})()");
  QTRY_VERIFY(evaluate("testWindow.selectionProviderFor(testWindow."
                       "activeWebView()).snapshot() !== null && "
                       "testWindow.selectionProviderFor(testWindow."
                       "activeWebView()).snapshot().chain.length===2")
                  .toBool());
  // Browser chrome can take focus without changing the retained source.
  key(Qt::Key_Colon);
  QTRY_COMPARE(text("mode"), QString("command"));
  QGuiApplication::clipboard()->setText("sentinel");
  QVERIFY(command("yank selection"));
  evaluate("testWindow.scheduleRuntimeWork(0)");
  QTRY_COMPARE(QGuiApplication::clipboard()->text(),
               QString("Child selection"));
  QVERIFY(!QGuiApplication::clipboard()->mimeData()->hasHtml());
  QCOMPARE(text("status_text"), QString("Selection copied"));
  // A detached frame invalidates its lease and cannot fall back to the parent.
  evaluate("testProbe.savedSelection=testWindow.selectionProviderFor("
           "testWindow.activeWebView()).snapshot()");
  page("document.getElementById('child').remove();true");
  scriptDone = false;
  evaluate(
      "testWindow.selectionProviderFor(testWindow.activeWebView()).extract("
      "function(result){testProbe.accept(result)},testProbe.savedSelection)");
  QTRY_VERIFY(scriptDone);
  QVERIFY(scriptResult.toMap().contains("error"));
  QCOMPARE(QGuiApplication::clipboard()->text(), QString("Child selection"));
}
void ApplicationTests::clipboardShortcutOverride() {
  QVERIFY(command("bind Ctrl+v mode-enter passthrough"));
  page("let "
       "e=document.getElementById('input');e.value='unchanged';e.focus();e."
       "setSelectionRange(9,9);true");
  evaluate("testWindow.browserUi.escape();testWindow.activeWebView()."
           "forceActiveFocus()");
  QGuiApplication::clipboard()->setText("must-not-paste");
  key(Qt::Key_V, Qt::ControlModifier);
  QTRY_COMPARE(text("mode"), QString("pass-through"));
  QCOMPARE(page("document.getElementById('input').value").toString(),
           QString("unchanged"));
  evaluate("testWindow.browserUi.escape()");
  QVERIFY(command("bind Ctrl+Shift+v mode-enter passthrough-once"));
  key(Qt::Key_V, Qt::ControlModifier | Qt::ShiftModifier);
  QTRY_COMPARE(text("mode"), QString("pass-through-once"));
  QCOMPARE(page("document.getElementById('input').value").toString(),
           QString("unchanged"));
}

// Cross-process data-device transfer in a private compositor. The injector
// verifies the parent window PID before sending compositor-delivered input.
void ApplicationTests::clipboardWaylandTransport() {
  if (QGuiApplication::platformName() != "wayland" ||
      qEnvironmentVariable("FERRIC_CLIPBOARD_ISOLATED") != "1")
    QSKIP("requires the isolated Wayland clipboard runner");
  auto inject = [this](const QString &action) {
    if (!window->isActive())
      return false;
    const auto helper = qEnvironmentVariable("FERRIC_CLIPBOARD_INJECTOR");
    if (helper.isEmpty())
      return false;
    QProcess process;
    process.start("python3", {helper, "--inject", action});
    if (!process.waitForStarted(1000))
      return false;
    QElapsedTimer deadline;
    deadline.start();
    while (process.state() != QProcess::NotRunning && deadline.elapsed() < 8000)
      QTest::qWait(10);
    if (process.state() != QProcess::NotRunning) {
      process.kill();
      process.waitForFinished(1000);
      return false;
    }
    if (process.exitCode() != 0)
      qWarning().noquote() << process.readAllStandardError();
    return process.exitCode() == 0;
  };
  auto readClipboard = []() {
    QProcess reader;
    reader.start("wl-paste", {"--no-newline"});
    if (!reader.waitForStarted(1000))
      return QByteArray();
    QElapsedTimer deadline;
    deadline.start();
    while (reader.state() != QProcess::NotRunning && deadline.elapsed() < 3000)
      QTest::qWait(10);
    if (reader.state() != QProcess::NotRunning) {
      reader.kill();
      reader.waitForFinished(1000);
      return QByteArray();
    }
    return reader.exitCode() == 0 ? reader.readAllStandardOutput()
                                  : QByteArray();
  };
  page("let "
       "e=document.getElementById('input');e.value='native-wayland-copy';e."
       "focus();e.select();true");
  evaluate("testWindow.browserUi.escape();testWindow.activeWebView()."
           "forceActiveFocus()");
  QVERIFY(inject("copy"));
  QTRY_COMPARE(readClipboard(), QByteArray("native-wayland-copy"));
  QVERIFY(inject("yank"));
  QTRY_COMPARE(readClipboard(), text("current_url").toUtf8());
  QProcess writer;
  writer.start("wl-copy", {"--foreground", "--type", "text/plain"});
  QVERIFY(writer.waitForStarted(1000));
  writer.write("external-wayland-paste");
  writer.closeWriteChannel();
  QTRY_COMPARE(QGuiApplication::clipboard()->text(),
               QString("external-wayland-paste"));
  page("let e=document.getElementById('input');e.value='';e.focus();true");
  evaluate("testWindow.browserUi.escape();testWindow.activeWebView()."
           "forceActiveFocus()");
  QVERIFY(inject("paste"));
  QTRY_COMPARE(page("document.getElementById('input').value").toString(),
               QString("external-wayland-paste"));
  QCOMPARE(text("mode"), QString("normal"));
  QVERIFY(inject("command"));
  QTRY_COMPARE(text("mode"), QString("command"));
  auto input = window->findChild<QQuickItem *>("commandLineInput");
  QVERIFY(input);
  QTRY_VERIFY(input->hasActiveFocus());
  input->setProperty("text", "open ");
  input->setProperty("cursorPosition", 5);
  QVERIFY(inject("paste"));
  QTRY_COMPARE(input->property("text").toString(),
               QString("open external-wayland-paste"));
  writer.terminate();
  if (!writer.waitForFinished(1000)) {
    writer.kill();
    writer.waitForFinished(1000);
  }
}

void ApplicationTests::clipboardSelectionControls_data() {
  QTest::addColumn<QString>("fixture");
  QTest::addColumn<QString>("expected");
  QTest::addColumn<bool>("allowed");
  QTest::newRow("unicode-input-substring")
      << QString("<input id=text value='aé漢字z'>")
      << QString::fromUtf8("é漢字") << true;
  QTest::newRow("multiline-textarea")
      << QString("<textarea id=text>aé漢字z\nnext</textarea>")
      << QString::fromUtf8("é漢字") << true;
  QTest::newRow("open-shadow-input")
      << QString("<div id=host></div>") << QString::fromUtf8("é漢字") << true;
  QTest::newRow("closed-shadow-input")
      << QString(
             "<p id=prior>Parent remembered selection</p><div id=host></div>")
      << QString("sentinel") << false;
  QTest::newRow("password")
      << QString("<input id=text type=password value='aé漢字z'>")
      << QString("sentinel") << false;
}
void ApplicationTests::clipboardSelectionControls() {
  QFETCH(QString, fixture);
  QFETCH(QString, expected);
  QFETCH(bool, allowed);
  const bool closed =
      QString::fromLatin1(QTest::currentDataTag()) == "closed-shadow-input";
  page("document.body.innerHTML=" + quoted(fixture) +
       ";if(document.getElementById('host')){let "
       "root=document.getElementById('host').attachShadow({mode:" +
       quoted(closed ? "closed" : "open") +
       "});root.innerHTML=" + quoted("<input id=text value='aé漢字z'>") +
       ";window.selectionControl=root.getElementById('text');}let "
       "prior=document.getElementById('prior');if(prior){let "
       "r=document.createRange();r.selectNodeContents(prior);getSelection()."
       "removeAllRanges();getSelection().addRange(r);}true");
  evaluate("testWindow.activeWebView().forceActiveFocus()");
  page("let "
       "e=document.getElementById('text')||window.selectionControl;e.focus();e."
       "setSelectionRange(1,4);true");
  QTRY_VERIFY(evaluate("testWindow.selectionProviderFor(testWindow."
                       "activeWebView()).snapshot() !== null")
                  .toBool());
  QTest::qWait(50);
  QGuiApplication::clipboard()->setText("sentinel");
  QVERIFY(command("yank selection"));
  evaluate("testWindow.scheduleRuntimeWork(0)");
  if (allowed)
    QTRY_COMPARE(QGuiApplication::clipboard()->text(), expected);
  else {
    QTRY_VERIFY(
        text("status_text").contains(closed ? "unsupported" : "password"));
    QCOMPARE(QGuiApplication::clipboard()->text(), expected);
  }
}
void ApplicationTests::clipboardSelectionChangedRejects() {
  evaluate("testWindow.activeWebView().forceActiveFocus()");
  page("let e=document.getElementById('input');e.value='Alpha "
       "beta';e.focus();e.select();true");
  QTRY_VERIFY(evaluate("testWindow.selectionProviderFor(testWindow."
                       "activeWebView()).snapshot() !== null")
                  .toBool());
  QTest::qWait(50);
  evaluate("testProbe.savedSelection=testWindow.selectionProviderFor("
           "testWindow.activeWebView()).snapshot()");
  page("document.getElementById('input').setSelectionRange(0,5);true");
  scriptDone = false;
  evaluate(
      "testWindow.selectionProviderFor(testWindow.activeWebView()).extract("
      "function(result){testProbe.accept(result)},testProbe.savedSelection)");
  QTRY_VERIFY(scriptDone);
  QVERIFY(scriptResult.toMap().contains("error"));
  // Broker cancellation completes exactly once through the bounded adapter.
  scriptDone = false;
  evaluate("testWindow.selectionProviderFor(testWindow.activeWebView())."
           "extract(function(result){testProbe.accept(result)},testProbe."
           "savedSelection);testWindow.selectionProviderFor(testWindow."
           "activeWebView()).invalidate()");
  QTRY_VERIFY_WITH_TIMEOUT(scriptDone, 2500);
  QVERIFY(scriptResult.toMap().contains("error"));
}

void ApplicationTests::clipboardSelectionLifetimeCycles() {
  evaluate("testWindow.activeWebView().forceActiveFocus()");
  page("document.getElementById('input').focus();true");
  for (int cycle = 0; cycle < 200; ++cycle) {
    evaluate("testWindow.selectionProviderFor(testWindow.activeWebView())."
             "capture();testWindow.selectionProviderFor(testWindow."
             "activeWebView()).invalidate()");
    QCOMPARE(evaluate("testWindow.selectionProviderFor(testWindow."
                      "activeWebView()).leaseCount")
                 .toInt(),
             0);
    if (cycle % 16 == 0)
      QTest::qWait(10);
  }
  QTest::qWait(2100);
  scriptDone = false;
  evaluate("testWindow.selectionProviderFor(testWindow.activeWebView())."
           "capture(function(source){testProbe.accept(source!==null)})");
  QTRY_VERIFY(scriptDone);
  QVERIFY(scriptResult.toBool());
  evaluate("testWindow.selectionProviderFor(testWindow.activeWebView())."
           "invalidate()");
  QTest::qWait(100);
  for (auto object : window->findChildren<QObject *>())
    if (QByteArray(object->metaObject()->className()) ==
        "FerricSelectionFrameBroker") {
      QCOMPARE(object->property("leaseCount").toInt(), 0);
      QCOMPARE(object->findChildren<QTimer *>().size(), 0);
    }
}

void ApplicationTests::clipboardHeldBrowserGesture() {
  page("let "
       "e=document.getElementById('input');e.value='unchanged';e.focus();e."
       "setSelectionRange(9,9);true");
  evaluate("testWindow.browserUi.escape();testWindow.activeWebView()."
           "forceActiveFocus()");
  QGuiApplication::clipboard()->setText("must-not-paste");
  key(Qt::Key_2);
  QTest::keyPress(window, Qt::Key_V, Qt::ControlModifier);
  QKeyEvent repeat(QEvent::KeyPress, Qt::Key_V, Qt::ControlModifier, QString(),
                   true, 1);
  QCoreApplication::sendEvent(window, &repeat);
  QTest::keyRelease(window, Qt::Key_V, Qt::ControlModifier);
  QTest::qWait(100);
  QCOMPARE(page("document.getElementById('input').value").toString(),
           QString("unchanged"));
  QCOMPARE(text("mode"), QString("normal"));
  key(Qt::Key_Q);
  key(Qt::Key_V, Qt::ControlModifier);
  QCOMPARE(page("document.getElementById('input').value").toString(),
           QString("unchanged"));
}

void ApplicationTests::productPolishPagePrompts() {
    window->resize(390, 700);
    evaluate("testWindow.font.pointSize = 16");
    QVERIFY(command("mode-enter pass-through"));
    page("setTimeout(function(){window.polishAnswer=confirm('<b>Literal confirmation</b> '+ 'Long message '.repeat(300));},0);true");
    QTRY_VERIFY(evaluate("testWindow.pendingPageDialogRequest !== null").toBool());
    QVERIFY(evaluate("testWindow.pendingPageDialogRequest.accepted").toBool());
    QCOMPARE(evaluate("testWindow.pageDialogOrigin").toString(),
             QString("http://127.0.0.1:%1").arg(server.serverPort()));
    auto cancel = findVisualItem(window->contentItem(), "pageDialogCancel");
    auto accept = findVisualItem(window->contentItem(), "pageDialogAccept");
    QVERIFY(cancel && accept);
    QTRY_VERIFY(cancel->hasActiveFocus());
    const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
    if (!artifacts.isEmpty()) {
        QTest::qWait(100);
        QVERIFY(window->grabWindow().save(artifacts + "/production-page-confirm-large.png"));
    }
    key(Qt::Key_Return);
    QTRY_VERIFY(evaluate("testWindow.pendingPageDialogRequest === null").toBool());
    QCOMPARE(page("window.polishAnswer").toBool(), false);
    QTRY_COMPARE(text("mode"), QString("pass-through"));

    page("setTimeout(function(){window.polishAnswer=prompt('Response, not HTML <b>','draft');},0);true");
    QTRY_VERIFY(evaluate("testWindow.pendingPageDialogRequest !== null").toBool());
    auto input = findVisualItem(window->contentItem(), "pageDialogInput");
    QVERIFY(input);
    QTRY_VERIFY(input->hasActiveFocus());
    key(Qt::Key_A, Qt::ControlModifier);
    typeText("reviewed response");
    key(Qt::Key_Return);
    QTRY_VERIFY(evaluate("testWindow.pendingPageDialogRequest === null").toBool());
    QCOMPARE(page("window.polishAnswer").toString(), QString("reviewed response"));
    QTRY_COMPARE(text("mode"), QString("pass-through"));

    page("setTimeout(function(){window.polishAnswer=confirm('Explicit acceptance');},0);true");
    QTRY_VERIFY(evaluate("testWindow.pendingPageDialogRequest !== null").toBool());
    QTRY_VERIFY(cancel->hasActiveFocus());
    accept->forceActiveFocus();
    QTRY_VERIFY(accept->hasActiveFocus());
    key(Qt::Key_Return);
    QTRY_VERIFY(evaluate("testWindow.pendingPageDialogRequest === null").toBool());
    QCOMPARE(page("window.polishAnswer").toBool(), true);
}

void ApplicationTests::productPolishWindowPrompts_data() {
    QTest::addColumn<bool>("popup");
    QTest::addColumn<bool>("authentication");
    QTest::newRow("secondary-confirm") << false << false;
    QTest::newRow("popup-confirm") << true << false;
    QTest::newRow("secondary-authentication") << false << true;
    QTest::newRow("popup-authentication") << true << true;
}

void ApplicationTests::productPolishWindowPrompts() {
    QTest::failOnWarning(QRegularExpression(".*(invalid context|TypeError|ReferenceError).*"));
    QFETCH(bool, popup);
    QFETCH(bool, authentication);
    QString entry;
    if (popup) {
        QVERIFY(command("bind p hint --target pointer links"));
        key(Qt::Key_P);
        const auto label = hintLabel("Open popup");
        QVERIFY(!label.isEmpty());
        typeText(label);
        key(Qt::Key_Return);
        QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(), 1);
        entry = "testWindow.popupWindowRegistry[0]";
        pageReceiver = entry + ".view";
    } else {
        QVERIFY(evaluate("testWindow.openEphemeralWindow(" + quoted(text("current_url")) + ")").toBool());
        QTRY_VERIFY(evaluate("testWindow.browserWindowRegistry.length > 1").toBool());
        entry = "testWindow.browserWindowRegistry[testWindow.browserWindowRegistry.length-1]";
        pageReceiver = entry + ".host.activeView";
    }
    auto child = qobject_cast<QQuickWindow *>(evaluate(entry + ".host").value<QObject *>());
    QVERIFY(child);
    QTRY_VERIFY(evaluate(pageReceiver + " !== null && !" + pageReceiver + ".loading").toBool());
    QTRY_VERIFY(page("document.getElementById('button') !== null").toBool());
    child->requestActivate();
    QTRY_VERIFY(child->isActive());
    evaluate(pageReceiver + ".forceActiveFocus()");
    // Mode restoration requires a known committed document, beyond the DOM
    // fixture becoming readable. Unknown ownership deliberately retains Normal.
    QTRY_VERIFY(QJsonDocument::fromJson(evaluate(pageReceiver + ".viewUi.refresh_interaction_status()")
                    .toString().toUtf8()).object().value("target").toObject().value("document_known").toBool());
    QVERIFY(evaluate(pageReceiver + ".viewUi.execute_command('mode-enter pass-through')").toBool());
    if (authentication) {
        page("setTimeout(function(){location.href='/polish-auth'},0);true");
        QTRY_VERIFY(evaluate("testWindow.pendingAuthenticationRequest !== null").toBool());
    } else {
        page("setTimeout(function(){window.promptAnswer=confirm('From the child window')},0);true");
        QTRY_VERIFY(evaluate("testWindow.pendingPageDialogRequest !== null").toBool());
    }
    auto dialog = findVisualItem(child->contentItem(), "pageDialogPopup");
    QVERIFY2(dialog, "The requesting window must contain its page dialog");
    QVERIFY(dialog->isVisible());
    QCOMPARE(dialog->window(), child);
    QCOMPARE(dialog->property("hostWindow").value<QObject *>(), child);
    const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
    if (!artifacts.isEmpty()) {
        QTest::qWait(50);
        QVERIFY(child->grabWindow().save(artifacts + "/production-" + QString::fromLatin1(QTest::currentDataTag()) + ".png"));
    }
    // A close confirmation temporarily owns input; keeping the window must
    // return to the existing page request rather than also dismissing it.
    QVERIFY(!child->close());
    QTRY_VERIFY(evaluate(entry + ".host.windowCloseConfirmationVisible").toBool());
    QTest::keyClick(child, Qt::Key_Escape);
    QTRY_VERIFY(!evaluate(entry + ".host.windowCloseConfirmationVisible").toBool());
    QVERIFY(evaluate(authentication ? "testWindow.pendingAuthenticationRequest !== null"
                                    : "testWindow.pendingPageDialogRequest !== null").toBool());
    if (authentication) {
        auto username = findVisualItem(child->contentItem(), "pageDialogUsername");
        auto password = findVisualItem(child->contentItem(), "pageDialogPassword");
        QVERIFY(username && password);
        QTRY_VERIFY(username->hasActiveFocus());
        QVERIFY(username->setProperty("text", QString("test-user")));
        QVERIFY(password->setProperty("text", QString("test-pass")));
        password->forceActiveFocus();
        QVERIFY(!child->close());
        QTest::keyClick(child, Qt::Key_Escape);
        QTRY_VERIFY(!evaluate(entry + ".host.windowCloseConfirmationVisible").toBool());
        QTRY_VERIFY(password->hasActiveFocus());
        QCOMPARE(password->property("text").toString(), QString("test-pass"));
        QTest::keyClick(child, Qt::Key_Return);
        QTRY_VERIFY(evaluate("testWindow.pendingAuthenticationRequest === null").toBool());
        QTRY_VERIFY(!evaluate(pageReceiver + ".loading").toBool());
        QCOMPARE(page("window.authenticated").toBool(), true);
        QCOMPARE(password->property("text").toString(), QString());
    } else {
        auto cancel = findVisualItem(child->contentItem(), "pageDialogCancel");
        QVERIFY(cancel);
        QTRY_VERIFY(cancel->hasActiveFocus());
        QTest::keyClick(child, Qt::Key_Return);
        QTRY_VERIFY(evaluate("testWindow.pendingPageDialogRequest === null").toBool());
        QCOMPARE(page("window.promptAnswer").toBool(), false);
    }
    QTRY_VERIFY(page("document.hasFocus()").toBool());
    // Same-document confirmation preserves pass-through. Navigation into an
    // authentication challenge can legitimately reset the document's mode.
    QTRY_VERIFY(!evaluate("testWindow.nativePromptActiveFor(" + pageReceiver + ".viewUi)").toBool());
    if (!authentication)
        QVERIFY2(evaluate(pageReceiver + ".viewUi.mode").toString() == "pass-through",
                 qPrintable(evaluate(pageReceiver + ".viewUi.refresh_interaction_status()").toString()));
    QVERIFY(!dialog->isVisible());
    evaluate(entry + ".host.destroy()");
    pageReceiver = "testWindow.activeWebView()";
}

void ApplicationTests::productPolishTlsPrompt_data() {
    QTest::addColumn<int>("hostKind");
    QTest::addColumn<bool>("accept");
    for (int host = 0; host < 3; ++host) {
        const QByteArray name = host == 0 ? "primary" : host == 1 ? "secondary" : "popup";
        QTest::newRow((name + "-cancel").constData()) << host << false;
        QTest::newRow((name + "-continue-session").constData()) << host << true;
    }
}

void ApplicationTests::productPolishTlsPrompt() {
    QFETCH(int, hostKind);
    QFETCH(bool, accept);
    QTest::failOnWarning(QRegularExpression(".*(invalid context|TypeError|ReferenceError).*"));
    // A disposable key and self-signed certificate never enter a trust store.
    const auto keyPath = directory.filePath("tls-key.pem");
    const auto certPath = directory.filePath("tls-cert.pem");
    QProcess openssl;
    openssl.start("openssl", {"req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
                             "-subj", "/CN=localhost", "-addext", "subjectAltName=DNS:localhost,IP:127.0.0.1",
                             "-keyout", keyPath, "-out", certPath});
    QVERIFY(openssl.waitForFinished(10000));
    QCOMPARE(openssl.exitCode(), 0);
    QFile keyFile(keyPath), certFile(certPath);
    QVERIFY(keyFile.open(QIODevice::ReadOnly));
    QVERIFY(certFile.open(QIODevice::ReadOnly));
    auto config = QSslConfiguration::defaultConfiguration();
    config.setLocalCertificate(QSslCertificate(certFile.readAll()));
    config.setPrivateKey(QSslKey(keyFile.readAll(), QSsl::Rsa));
    config.setPeerVerifyMode(QSslSocket::VerifyNone);
    QSslServer tls;
    tls.setSslConfiguration(config);
    QVERIFY(tls.listen(QHostAddress::LocalHost));
    int received = 0;
    connect(&tls, &QTcpServer::pendingConnectionAvailable, &tls, [&] {
        while (auto socket = tls.nextPendingConnection()) {
            connect(socket, &QTcpSocket::readyRead, socket, [&, socket] {
                auto request = socket->property("request").toByteArray() + socket->readAll();
                socket->setProperty("request", request);
                if (!request.contains("\r\n\r\n")) return;
                ++received;
                const QByteArray body = "<!doctype html><title>TLS fixture</title><script>window.tlsLoaded=true</script>";
                socket->write("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: "
                              + QByteArray::number(body.size()) + "\r\nConnection: close\r\n\r\n" + body);
                socket->disconnectFromHost();
            });
        }
    });
    QString entry = "testWindow";
    if (hostKind == 1) {
        QVERIFY(evaluate("testWindow.openEphemeralWindow(" + quoted(text("current_url")) + ")").toBool());
        entry = "testWindow.browserWindowRegistry[testWindow.browserWindowRegistry.length-1].host";
        pageReceiver = entry + ".activeView";
    } else if (hostKind == 2) {
        QVERIFY(command("bind p hint --target pointer links"));
        key(Qt::Key_P);
        typeText(hintLabel("Open popup"));
        key(Qt::Key_Return);
        QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(), 1);
        entry = "testWindow.popupWindowRegistry[0].host";
        pageReceiver = "testWindow.popupWindowRegistry[0].view";
    }
    auto host = qobject_cast<QQuickWindow *>(evaluate(entry).value<QObject *>());
    QVERIFY(host);
    QTRY_VERIFY(page("document.getElementById('button') !== null").toBool());
    host->requestActivate();
    QTRY_VERIFY(host->isActive());
    const auto origin = QString("https://127.0.0.1:%1").arg(tls.serverPort());
    evaluate(pageReceiver + ".url=" + quoted(origin + "/tls"));
    QTRY_VERIFY(evaluate("testWindow.pendingCertificateError !== null").toBool());
    QCOMPARE(evaluate("testWindow.pendingCertificateErrorHost").toString(), origin);
    auto back = findVisualItem(host->contentItem(), "commandDialogAction-back");
    auto proceed = findVisualItem(host->contentItem(), "commandDialogAction-accept");
    QVERIFY(back && proceed);
    QTRY_VERIFY(back->hasActiveFocus());
    const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
    if (!artifacts.isEmpty()) {
        QTest::qWait(50);
        QVERIFY(host->grabWindow().save(artifacts + "/production-tls-" + QString::fromLatin1(QTest::currentDataTag()) + ".png"));
    }
    // Let the signal handler return and prove the actual request is paused.
    QTest::qWait(100);
    QCOMPARE(received, 0);
    QVERIFY(evaluate("testWindow.pendingCertificateError !== null").toBool());
    if (accept) proceed->forceActiveFocus();
    QTest::keyClick(host, Qt::Key_Return);
    QTRY_VERIFY(evaluate("testWindow.pendingCertificateError === null").toBool());
    QTRY_VERIFY(!evaluate(pageReceiver + ".loading").toBool());
    if (accept) {
        QTRY_VERIFY(received > 0);
        QVERIFY(page("window.tlsLoaded === true").toBool());
        const auto previous = received;
        evaluate(pageReceiver + ".url=" + quoted(origin + "/tls-again"));
        QTRY_VERIFY(received > previous);
        QTRY_VERIFY(!evaluate(pageReceiver + ".loading").toBool());
        QVERIFY(evaluate("testWindow.pendingCertificateError === null").toBool());
        QVERIFY(page("window.tlsLoaded === true").toBool());
    } else {
        QCOMPARE(received, 0);
    }
}

void ApplicationTests::productPolishIdentityWindows_data() {
    QTest::addColumn<bool>("popup");
    QTest::addColumn<bool>("clientCertificate");
    QTest::newRow("secondary-client-certificate") << false << true;
    QTest::newRow("popup-client-certificate") << true << true;
    QTest::newRow("secondary-security-key") << false << false;
    QTest::newRow("popup-security-key") << true << false;
}

void ApplicationTests::productPolishCertificateGuards() {
    for (const auto &input : {QString("url:'https://user:secret@ACCOUNTS.GOOGLE.COM:443/signin?token=private',overridable:true,isMainFrame:true"),
                              QString("url:'https://example.test/',overridable:false,isMainFrame:true"),
                              QString("url:'https://example.test/',overridable:true,isMainFrame:false"),
                              QString("url:'data:text/plain,unknown',overridable:true,isMainFrame:true")}) {
        savedSelection = 0;
        evaluate("testWindow.handleCertificateError(testWindow.browserUi,testWindow.activeWebView(),({"
                 + input + ",rejectCertificate:function(){testProbe.savedSelection=Number(testProbe.savedSelection)+1},"
                 "defer:function(){testProbe.savedSelection=-10},acceptCertificate:function(){testProbe.savedSelection=-20}}),testWindow)");
        QCOMPARE(savedSelection.toInt(), 1);
        QVERIFY(evaluate("testWindow.pendingCertificateError === null").toBool());
    }
    QUrl authority;
    authority.setHost("EXAMPLE.test");
    authority.setPort(8443);
    QCOMPARE(evaluate("testWindow.browserUi.client_certificate_origin(" + quoted(authority.toString()) + ")").toString(),
             QString("https://example.test:8443"));
    QCOMPARE(evaluate("testWindow.browserUi.prompt_host('https://name:secret@EXAMPLE.test:8443/path?token=private')").toString(),
             QString("example.test"));
}

void ApplicationTests::productPolishIdentityWindows() {
    QFETCH(bool, popup);
    QFETCH(bool, clientCertificate);
    QTest::failOnWarning(QRegularExpression(".*(invalid context|TypeError|ReferenceError).*"));
    QString entry;
    if (popup) {
        QVERIFY(command("bind p hint --target pointer links")); key(Qt::Key_P);
        typeText(hintLabel("Open popup")); key(Qt::Key_Return);
        QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(), 1);
        entry = "testWindow.popupWindowRegistry[0].host";
        pageReceiver = "testWindow.popupWindowRegistry[0].view";
    } else {
        QVERIFY(evaluate("testWindow.openEphemeralWindow(" + quoted(text("current_url")) + ")").toBool());
        entry = "testWindow.browserWindowRegistry[testWindow.browserWindowRegistry.length-1].host";
        pageReceiver = entry + ".activeView";
    }
    auto host = qobject_cast<QQuickWindow *>(evaluate(entry).value<QObject *>());
    QVERIFY(host);
    QTRY_VERIFY(page("document.getElementById('button') !== null").toBool());
    host->requestActivate(); QTRY_VERIFY(host->isActive());
    evaluate(pageReceiver + ".forceActiveFocus()");
    // In-memory requests qualify chrome ownership and exactly-once decisions,
    // independently of physical credentials and the OS certificate store.
    const auto request = clientCertificate
        ? "({host:'//EXAMPLE.test:8443',certificates:[{subject:'Test identity',issuer:'Test issuer'}],selectNone:function(){testProbe.savedSelection=Number(testProbe.savedSelection)+1}})"
        : "({relyingPartyId:'example.test',userNames:[],state:0,pinRequest:{remainingAttempts:0,minPinLength:0},stateChanged:{connect:function(){}},cancel:function(){testProbe.savedSelection=Number(testProbe.savedSelection)+1}})";
    savedSelection = 0;
    evaluate("testWindow." + QString(clientCertificate ? "handleClientCertificateRequested(" : "handleWebAuthRequested(")
             + pageReceiver + ".viewUi," + pageReceiver + "," + request
             + (clientCertificate ? ",true," : ",") + entry + ")");
    if (clientCertificate)
        QCOMPARE(evaluate("testWindow.pendingClientCertificateHost").toString(), QString("https://example.test:8443"));
    auto cancel = findVisualItem(host->contentItem(), clientCertificate ? "clientCertificateCancel" : "webAuthCancel");
    QVERIFY(cancel);
    QTRY_VERIFY(cancel->hasActiveFocus());
    QTest::keyClick(host, Qt::Key_Escape);
    QTRY_COMPARE(savedSelection.toInt(), 1);
    QTRY_VERIFY(!evaluate("testWindow.nativePromptActiveFor(" + pageReceiver + ".viewUi)").toBool());
    QTRY_VERIFY(page("document.hasFocus()").toBool());
}

void ApplicationTests::productPolishPromptTeardown_data() {
    QTest::addColumn<bool>("popup");
    QTest::addColumn<bool>("disposeEngine");
    QTest::newRow("secondary-destroy") << false << false;
    QTest::newRow("popup-destroy") << true << false;
    QTest::newRow("secondary-engine-disposal") << false << true;
    QTest::newRow("popup-engine-disposal") << true << true;
}

void ApplicationTests::productPolishPromptTeardown() {
    QFETCH(bool, popup);
    QFETCH(bool, disposeEngine);
    QTest::failOnWarning(QRegularExpression(".*(invalid context|TypeError|ReferenceError).*"));
    QString entry;
    if (popup) {
        QVERIFY(command("bind p hint --target pointer links")); key(Qt::Key_P);
        typeText(hintLabel("Open popup")); key(Qt::Key_Return);
        QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(), 1);
        entry = "testWindow.popupWindowRegistry[0].host";
        pageReceiver = "testWindow.popupWindowRegistry[0].view";
    } else {
        QVERIFY(evaluate("testWindow.openEphemeralWindow(" + quoted(text("current_url")) + ")").toBool());
        entry = "testWindow.browserWindowRegistry[testWindow.browserWindowRegistry.length-1].host";
        pageReceiver = entry + ".activeView";
    }
    QTRY_VERIFY(page("document.getElementById('button') !== null").toBool());
    page("setTimeout(function(){confirm('Dispose while pending')},0);true");
    QTRY_VERIFY(evaluate("testWindow.pendingPageDialogRequest !== null").toBool());
    if (disposeEngine) {
        savedSelection = 0;
        QVERIFY(evaluate("testWindow.runBrowserScript(" + pageReceiver
                         + ", '42', function(){testProbe.savedSelection=1})").toBool());
        cleanup();
        QCOMPARE(savedSelection.toInt(), 0);
    } else {
        evaluate(entry + ".destroy()");
        QTRY_VERIFY(evaluate("testWindow.pendingPageDialogRequest === null").toBool());
        pageReceiver = "testWindow.activeWebView()";
    }
    QTest::qWait(20);
}

void ApplicationTests::productPolishHttpAuthentication() {
    QCOMPARE(evaluate("testWindow.browserUi.prompt_origin('https://name:private@EXAMPLE.test:8443/path?token=private#fragment')").toString(),
             QString("https://example.test:8443"));
    QCOMPARE(evaluate("testWindow.browserUi.prompt_origin('data:text/html,opaque')").toString(), QString());
    const auto origin = QString("http://127.0.0.1:%1").arg(server.serverPort());
    QVERIFY(command("open " + origin + "/polish-auth"));
    QTRY_VERIFY(evaluate("testWindow.pendingAuthenticationRequest !== null").toBool());
    QVERIFY(evaluate("testWindow.pendingAuthenticationRequest.accepted").toBool());
    QCOMPARE(evaluate("testWindow.pageDialogOrigin").toString(), origin);
    auto username = findVisualItem(window->contentItem(), "pageDialogUsername");
    auto password = findVisualItem(window->contentItem(), "pageDialogPassword");
    QVERIFY(username && password);
    QTRY_VERIFY(username->hasActiveFocus());
    QVERIFY(username->setProperty("text", QString("test-user")));
    QVERIFY(password->setProperty("text", QString("test-pass")));
    password->forceActiveFocus();
    key(Qt::Key_Return);
    QTRY_VERIFY(evaluate("testWindow.pendingAuthenticationRequest === null").toBool());
    QTRY_VERIFY(!evaluate("testWindow.activeWebView().loading").toBool());
    QCOMPARE(page("window.authenticated").toBool(), true);
    QCOMPARE(username->property("text").toString(), QString());
    QCOMPARE(password->property("text").toString(), QString());
}

void ApplicationTests::productPolishFileChooser() {
    auto chooser = window->findChild<QObject *>("engineFileChooser");
    QVERIFY(chooser);
    QVERIFY(chooser->setProperty("currentFolder", QUrl::fromLocalFile(directory.path())));
    QVERIFY(command("mode-enter pass-through"));
    page("var input=document.createElement('input');input.type='file';input.id='polish-file';document.body.prepend(input);input.focus();true");
    key(Qt::Key_Return);
    QTRY_VERIFY(evaluate("testWindow.pendingFileDialogRequest !== null").toBool());
    QVERIFY(evaluate("testWindow.pendingFileDialogRequest.accepted").toBool());
    evaluate("testWindow.clearEngineFileDialog(true, 'File selection cancelled')");
    QTRY_VERIFY(evaluate("testWindow.pendingFileDialogRequest === null").toBool());
    QTRY_COMPARE(text("mode"), QString("pass-through"));
    QCOMPARE(page("document.getElementById('polish-file').files.length").toInt(), 0);
    const auto path = directory.filePath(QString::fromUtf8("résumé + 100%.txt"));
    QFile upload(path);
    QVERIFY(upload.open(QIODevice::WriteOnly));
    QCOMPARE(upload.write("upload fixture"), qint64(14));
    upload.close();
    // Configure the initial selection before the new chooser is opened.
    // Closing an active chooser would cancel the WebEngine request itself.
    QVERIFY(chooser->setProperty("selectedFile", QUrl::fromLocalFile(path)));
    window->requestActivate();
    QTRY_VERIFY(window->isActive());
    evaluate("testWindow.activeWebView().forceActiveFocus()");
    page("document.getElementById('polish-file').focus();true");
    QTRY_VERIFY(page("document.hasFocus()").toBool());
    key(Qt::Key_Return);
    QTRY_VERIFY(evaluate("testWindow.pendingFileDialogRequest !== null").toBool());
    QTRY_VERIFY(chooser->property("visible").toBool());
    QCOMPARE(chooser->property("selectedFile").toUrl(), QUrl::fromLocalFile(path));
    QVERIFY(QMetaObject::invokeMethod(chooser, "accept"));
    QTRY_VERIFY(evaluate("testWindow.pendingFileDialogRequest === null").toBool());
    QTRY_COMPARE(page("document.getElementById('polish-file').files.length").toInt(), 1);
    QCOMPARE(page("document.getElementById('polish-file').files[0].name").toString(), QFileInfo(path).fileName());
}

void ApplicationTests::productPolishWebAuthStates() {
    using State = QWebEngineWebAuthUxRequest::WebAuthUxState;
    const auto select = QString::number(static_cast<int>(State::SelectAccount));
    const auto pin = QString::number(static_cast<int>(State::CollectPin));
    const auto failed = QString::number(static_cast<int>(State::RequestFailed));
    QCOMPARE(evaluate("testWindow.webAuthStateLabel(" + select + ")").toString(), QString("Select an account"));
    QCOMPARE(evaluate("testWindow.webAuthStateLabel(" + pin + ")").toString(), QString("Enter your security-key PIN"));
    QCOMPARE(evaluate("testWindow.webAuthStateLabel(" + failed + ")").toString(), QString("Security-key request failed"));
    // Exercise the production request callbacks using an in-memory request.
    // Hardware authentication itself is qualified separately with a real key.
    evaluate("testWindow.pendingWebAuthRequest = ({selected:'',pinMatched:false,retries:0,"
             "pinRequest:{remainingAttempts:3,minPinLength:4},"
             "setSelectedAccount:function(value){this.selected=value},"
             "setPin:function(value){this.pinMatched=value==='1234'},"
             "retry:function(){this.retries++}})");
    evaluate("testWindow.pendingWebAuthState=" + select + ";testWindow.selectWebAuthAccount('offered account')");
    QCOMPARE(evaluate("testWindow.pendingWebAuthRequest.selected").toString(), QString("offered account"));
    evaluate("testWindow.pendingWebAuthState=" + pin);
    auto input = findVisualItem(window->contentItem(), "webAuthPin");
    QVERIFY(input);
    QVERIFY(input->setProperty("text", QString("1234")));
    evaluate("testWindow.submitWebAuthPin()");
    QVERIFY(evaluate("testWindow.pendingWebAuthRequest.pinMatched").toBool());
    QCOMPARE(input->property("text").toString(), QString());
    evaluate("testWindow.pendingWebAuthState=" + failed + ";testWindow.retryWebAuth()");
    QCOMPARE(evaluate("testWindow.pendingWebAuthRequest.retries").toInt(), 1);
    evaluate("testWindow.pendingWebAuthRequest=null;testWindow.pendingWebAuthState=null");
}

void ApplicationTests::productPolishNavigationFailure() {
    evaluate("testWindow.browserUi.navigation_failed_with_details("
             "testWindow.browserUi.active_tab_index,"
             "'https://unreachable.example/','dns','net::ERR_NAME_NOT_RESOLVED')");
    QTRY_VERIFY(evaluate("testWindow.browserUi.navigation_failure_visible").toBool());
    QCOMPARE(text("status_text"), QString("Couldn't load this page"));
    auto button = window->findChild<QQuickItem *>("navigationFailureDetailsButton");
    auto details = window->findChild<QQuickItem *>("navigationFailureDetails");
    QVERIFY(button);
    QVERIFY(details);
    QVERIFY(!details->isVisible());
    QSignalSpy rendered(window, &QQuickWindow::frameSwapped);
    window->update();
    QVERIFY(rendered.wait(2000));
    QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier,
                      button->mapToScene(QPointF(button->width() / 2, button->height() / 2)).toPoint());
    QTRY_VERIFY(details->isVisible());
    QVERIFY(details->property("text").toString().contains("ERR_NAME_NOT_RESOLVED"));
    const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
    if (!artifacts.isEmpty())
        QVERIFY(window->grabWindow().save(artifacts + "/production-navigation-details.png"));
    key(Qt::Key_Escape);
    QTRY_VERIFY(!evaluate("testWindow.browserUi.navigation_failure_visible").toBool());
}

void ApplicationTests::productPolishManagementSurfaces_data() {
    QTest::addColumn<QString>("entry");
    QTest::addColumn<QString>("visibleProperty");
    QTest::newRow("downloads") << QString("downloads") << QString("downloadManagerVisible");
    QTest::newRow("profiles") << QString("profiles") << QString("profileManagerVisible");
    QTest::newRow("sessions") << QString("sessions") << QString("sessionManagerVisible");
    QTest::newRow("settings") << QString("settings") << QString("settingsVisible");
}

void ApplicationTests::productPolishManagementSurfaces() {
    QFETCH(QString, entry);
    QFETCH(QString, visibleProperty);
    if (entry != "downloads") {
        QVERIFY(!command(entry + " unexpected"));
        QVERIFY(!evaluate("testWindow." + visibleProperty).toBool());
    }
    QVERIFY(command(entry));
    QTRY_VERIFY(evaluate("testWindow." + visibleProperty).toBool());
    if (entry == "settings") {
        auto input = window->findChild<QQuickItem *>("settingsSearchInput");
        QVERIFY(input);
        QTRY_VERIFY(input->hasActiveFocus());
        key(Qt::Key_Z);
        QTRY_COMPARE(evaluate("testWindow.settingsSearch").toString(), QString("z"));
    }
    const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
    if (!artifacts.isEmpty()) {
        QTest::qWait(100);
        QVERIFY(window->grabWindow().save(artifacts + "/production-" +
                                          QString::fromLatin1(QTest::currentDataTag()) + ".png"));
    }
    key(Qt::Key_Escape);
    QTRY_VERIFY(!evaluate("testWindow." + visibleProperty).toBool());
}

void ApplicationTests::productPolishWindowClose_data() {
    QTest::addColumn<bool>("popup");
    QTest::newRow("secondary") << false;
    QTest::newRow("popup") << true;
}

void ApplicationTests::productPolishWindowClose() {
    QFETCH(bool, popup);
    QString entry;
    if (popup) {
        QVERIFY(command("bind p hint --target pointer links"));
        key(Qt::Key_P);
        const auto label = hintLabel("Open popup");
        QVERIFY(!label.isEmpty());
        typeText(label);
        key(Qt::Key_Return);
        QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(), 1);
        entry = "testWindow.popupWindowRegistry[0]";
    } else {
        QVERIFY(evaluate("testWindow.openEphemeralWindow('rb://blank')").toBool());
        QTRY_VERIFY(evaluate("testWindow.browserWindowRegistry.length > 1").toBool());
        entry = "testWindow.browserWindowRegistry[testWindow.browserWindowRegistry.length-1]";
    }
    auto child = qobject_cast<QQuickWindow *>(evaluate(entry + ".host").value<QObject *>());
    QVERIFY(child);
    child->requestActivate();
    QTRY_VERIFY(child->isActive());
    // A compositor close request must display the safe choice in the actual
    // receiving window, using the same theme as the primary window.
    QVERIFY(!child->close());
    QTRY_VERIFY(evaluate(entry + ".host.windowCloseConfirmationVisible").toBool());
    const auto dialogs = child->findChildren<QQuickItem *>("shutdownDecisionDialog");
    QQuickItem *dialog = nullptr;
    for (auto candidate : dialogs)
        if (candidate->isVisible()) dialog = candidate;
    QVERIFY(dialog);
    auto panel = dialog->findChild<QQuickItem *>("shutdownDecisionPanel");
    auto keep = dialog->findChild<QQuickItem *>("shutdownKeepButton");
    QVERIFY(panel);
    QVERIFY(keep);
    QCOMPARE(panel->property("color"), evaluate("testWindow.panelColor"));
    QTRY_VERIFY(keep->hasActiveFocus());
    const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
    if (!artifacts.isEmpty())
        QVERIFY(child->grabWindow().save(artifacts + "/production-close-" +
                                         QString::fromLatin1(QTest::currentDataTag()) + ".png"));
    QTest::keyClick(child, Qt::Key_Escape);
    QTRY_VERIFY(!evaluate(entry + ".host.windowCloseConfirmationVisible").toBool());
    QVERIFY(child->isVisible());
    QVERIFY(!child->close());
    QTRY_VERIFY(evaluate(entry + ".host.windowCloseConfirmationVisible").toBool());
    QTest::mouseClick(child, Qt::LeftButton, Qt::NoModifier,
                     keep->mapToScene(QPointF(keep->width()/2, keep->height()/2)).toPoint());
    QTRY_VERIFY(!evaluate(entry + ".host.windowCloseConfirmationVisible").toBool());
    QVERIFY(child->isVisible());
    evaluate(entry + ".host.destroy()");
}

void ApplicationTests::productPolishPopupScripts() {
    // Install ordinary top-frame page scripts through the real manager. Both
    // phases must run after a popup's navigation has committed successfully.
    for (const auto &phase : {QString("document_end"), QString("document_idle")}) {
        const auto name = "popup_" + phase;
        QFile source(directory.filePath(name + ".js"));
        QVERIFY(source.open(QIODevice::WriteOnly));
        source.write(("window." + name + "=(window." + name + "||0)+1;").toUtf8());
        source.close();
        QFile manifest(directory.filePath(name + ".toml"));
        QVERIFY(manifest.open(QIODevice::WriteOnly));
        manifest.write(("schema_version = 1\nname = \"" + name + "\"\nexecutable = \"/bin/true\"\n"
                        "page_world = true\nsource = \"" + name + ".js\"\n"
                        "matches = [\"http://127.0.0.1:*/*\"]\nrun_at = \"" + phase + "\"\n").toUtf8());
        manifest.close();
        QVERIFY(evaluate("testWindow.browserUi.install_userscript_manifest(" +
                         quoted(manifest.fileName()) + ")").toBool());
        QTRY_COMPARE(text("userscript_install_state"), QString("installed"));
    }
    evaluate("testWindow.safeMode=false;testWindow.userscriptsOff=false");
    QVERIFY(command("bind p hint --target pointer links"));
    key(Qt::Key_P);
    const auto label = hintLabel("Open popup");
    QVERIFY(!label.isEmpty());
    typeText(label);
    key(Qt::Key_Return);
    QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(), 1);
    pageReceiver = "testWindow.popupWindowRegistry[0].view";
    QTRY_VERIFY(evaluate(pageReceiver + " !== null && !" + pageReceiver + ".loading").toBool());
    QTRY_COMPARE(page("window.popup_document_end || 0").toInt(), 1);
    QTRY_COMPARE(page("window.popup_document_idle || 0").toInt(), 1);
    evaluate("testWindow.popupWindowRegistry[0].host.destroy()");
    pageReceiver = "testWindow.activeWebView()";
}

void ApplicationTests::productPolishSiteLedger() {
    QVERIFY(text("config_json").contains("other-private.test"));
    page("history.replaceState(null,'','/private-path?token=secret');true");
    QTRY_VERIFY(text("current_url").contains("token=secret"));
    evaluate("testWindow.showSiteLedger()");
    const auto oldSnapshot = evaluate("JSON.stringify(testWindow.siteLedgerData)").toString();
    auto currentCopy = window->findChild<QQuickItem *>("siteReportCopyButton");
    QVERIFY(currentCopy);
    evaluate("testWindow.browserUi.status_text='Copied https://account.test/private?token=secret'");
    QGuiApplication::clipboard()->setText("sentinel");
    QVERIFY(QMetaObject::invokeMethod(currentCopy, "click"));
    QTRY_VERIFY(QGuiApplication::clipboard()->text().startsWith("{"));
    QVERIFY(!QGuiApplication::clipboard()->text().contains("127.0.0.1"));
    QVERIFY(!QGuiApplication::clipboard()->text().contains("token=secret"));
    const auto reports = evaluate(R"JS((function() {
        var ui = testWindow.browserUi;
        var started = JSON.parse(ui.begin_site_doctor_experiment('blocking-bypass'));
        var active = JSON.parse(ui.site_report(false));
        var withHost = JSON.parse(ui.site_report(true));
        var finished = ui.finish_site_doctor_experiment(started.id, true);
        return {active: active, withHost: withHost, finished: finished,
            proposal: JSON.parse(ui.site_doctor_proposal()),
            completed: JSON.parse(ui.site_report(false))};
    })())JS").toMap();
    QVERIFY(reports.value("finished").toBool());
    const auto active = reports.value("active").toMap();
    QCOMPARE(active.value("experiment").toMap().value("active").toMap().value("kind").toString(),
             QString("blocking-bypass"));
    const auto proposal = reports.value("proposal").toMap();
    QCOMPARE(proposal.value("state").toString(), QString("pending-confirmation"));
    QVERIFY(QJsonDocument::fromVariant(proposal).toJson().contains("other-private.test"));
    for (const auto &field : {QString("active"), QString("completed"), QString("withHost")}) {
        const auto bytes = QJsonDocument::fromVariant(reports.value(field)).toJson();
        QVERIFY(!bytes.contains("private-path"));
        QVERIFY(!bytes.contains("token=secret"));
        QVERIFY(!bytes.contains("other-private.test"));
        if (field != "withHost") QVERIFY(!bytes.contains("127.0.0.1"));
    }
    QCOMPARE(reports.value("withHost").toMap().value("site").toMap().value("host").toString(),
             QString("127.0.0.1"));
    // A background navigation must not reuse consent from an old site panel.
    evaluate("testWindow.browserUi.navigate('rb://blank')");
    evaluate("testWindow.executePendingEngineAction()");
    QTRY_COMPARE(evaluate("testWindow.activeWebView().nativeUrl").toString(), QString("rb://blank"));
    QTRY_VERIFY(evaluate("JSON.parse(testWindow.browserUi.refresh_site_status()).origin === null").toBool());
    QVERIFY(!evaluate("testWindow.browserUi.apply_site_doctor_proposal(" +
                     quoted(proposal.value("id").toString()) + ",true)").toBool());
    evaluate("testWindow.siteLedgerData=JSON.parse(" + quoted(oldSnapshot) + ");testWindow.siteLedgerVisible=true");
    auto consent = window->findChild<QQuickItem *>("siteReportHost");
    auto copy = window->findChild<QQuickItem *>("siteReportCopyButton");
    QVERIFY(consent); QVERIFY(copy);
    consent->setProperty("checked", true);
    QGuiApplication::clipboard()->setText("sentinel");
    QVERIFY(QMetaObject::invokeMethod(copy, "click"));
    QTest::qWait(100);
    QCOMPARE(QGuiApplication::clipboard()->text(), QString("sentinel"));
    auto notice = window->findChild<QQuickItem *>("siteLedgerNotice");
    QVERIFY(notice);
    QVERIFY(notice->property("text").toString().contains("page changed"));
}

void ApplicationTests::productPolishSessionPreview() {
    QVERIFY(evaluate("testWindow.browserUi.save_named_session('polish-preview')").toBool());
    QTRY_VERIFY(evaluate("testWindow.browserUi.list_named_sessions().indexOf('polish-preview')>=0").toBool());
    evaluate("testWindow.showSessionPreview('polish-preview',false)");
    QTRY_VERIFY(!evaluate("testWindow.sessionPreviewLoading").toBool());
    QVERIFY(!evaluate("testWindow.sessionPreviewError").toBool());
    auto tabs = window->findChild<QQuickItem *>("sessionPreviewTabs");
    QVERIFY(tabs);
    QTRY_COMPARE(tabs->property("count").toInt(), 1);
    QCOMPARE(evaluate("testWindow.browserUi.session_preview_urls[0]").toString(), text("current_url"));
    key(Qt::Key_Escape);
    QTRY_VERIFY(!evaluate("testWindow.sessionPreviewVisible").toBool());
    evaluate("testWindow.showSessionPreview('missing-session',false)");
    QTRY_VERIFY(!evaluate("testWindow.sessionPreviewLoading").toBool());
    QVERIFY(evaluate("testWindow.sessionPreviewError").toBool());
    auto append = findVisualItem(window->contentItem(), "commandDialogAction-append");
    QVERIFY(append);
    QVERIFY(!append->isEnabled());
    const auto before = text("current_url");
    key(Qt::Key_A);
    QCOMPARE(text("current_url"), before);
    key(Qt::Key_Escape);
    QTRY_VERIFY(!evaluate("testWindow.sessionPreviewVisible").toBool());
}

void ApplicationTests::productPolishManagerFeedback() {
    QVERIFY(command("profiles"));
    QTRY_VERIFY(evaluate("testWindow.profileManagerVisible").toBool());
    QTRY_VERIFY(window->findChild<QQuickItem *>("profileNameInput"));
    auto profileName = window->findChild<QQuickItem *>("profileNameInput");
    auto profileLabel = window->findChild<QQuickItem *>("profileLabelInput");
    auto create = window->findChild<QQuickItem *>("profileCreateButton");
    QVERIFY(profileLabel && create);
    QVERIFY(profileName->setProperty("text", "polish-worker"));
    QVERIFY(profileLabel->setProperty("text", "Polish worker"));
    // Include a concurrent catalog read: completion must publish the new entry.
    QVERIFY(evaluate("testWindow.browserUi.request_profile_list()").toBool());
    QVERIFY(QMetaObject::invokeMethod(create, "click"));
    QCOMPARE(text("profile_action_state"), QString("pending"));
    QCOMPARE(profileName->property("text").toString(), QString("polish-worker"));
    QTRY_COMPARE(text("profile_action_state"), QString("succeeded"));
    QTRY_COMPARE(profileName->property("text").toString(), QString());
    QTRY_VERIFY(text("profile_values").contains("polish-worker"));
    const auto firstReceipt = text("profile_action_id");
    profileName->setProperty("text", "polish-worker");
    profileLabel->setProperty("text", "Duplicate draft");
    QVERIFY(QMetaObject::invokeMethod(create, "click"));
    QVERIFY(text("profile_action_id") != firstReceipt);
    QTRY_COMPARE(text("profile_action_state"), QString("failed"));
    QCOMPARE(profileName->property("text").toString(), QString("polish-worker"));
    QCOMPARE(profileLabel->property("text").toString(), QString("Duplicate draft"));
    QVERIFY(!text("profile_action_detail").isEmpty());
    auto profileFeedback = window->findChild<QQuickItem *>("profileFeedback");
    QVERIFY(profileFeedback && profileFeedback->isVisible());
    QCOMPARE(profileFeedback->property("message").toString(), text("profile_action_message"));
    // Mutations from the command boundary also update the visible catalog.
    QVERIFY(evaluate("testWindow.browserUi.rename_profile('polish-worker','Research')").toBool());
    QTRY_COMPARE(text("profile_action_state"), QString("succeeded"));
    QTRY_VERIFY(text("profile_values").contains("polish-worker\tResearch"));
    QVERIFY(evaluate("testWindow.browserUi.delete_profile('polish-worker',true)").toBool());
    QTRY_COMPARE(text("profile_action_state"), QString("succeeded"));
    QTRY_VERIFY(!text("profile_values").contains("polish-worker"));
    key(Qt::Key_Escape);

    QVERIFY(command("sessions"));
    QTRY_VERIFY(evaluate("testWindow.sessionManagerVisible").toBool());
    QTRY_VERIFY(window->findChild<QQuickItem *>("sessionNameInput"));
    auto sessionName = window->findChild<QQuickItem *>("sessionNameInput");
    auto save = window->findChild<QQuickItem *>("sessionSaveButton");
    QVERIFY(save);
    sessionName->setProperty("text", "polish-worker");
    QVERIFY(QMetaObject::invokeMethod(save, "click"));
    QCOMPARE(text("session_save_state"), QString("pending"));
    QTRY_COMPARE(text("session_save_state"), QString("succeeded"));
    QTRY_COMPARE(sessionName->property("text").toString(), QString());
    QTRY_VERIFY(evaluate("testWindow.browserUi.session_names_values.indexOf('polish-worker')>=0").toBool());
    auto list = window->findChild<QQuickItem *>("sessionList");
    QVERIFY(list);
    QTRY_COMPARE(list->property("count").toInt(), 1);
    QDirIterator files(evaluate("testWindow.storageBasePath").toString() + "/state/sessions",
                       QStringList{"polish-worker.json"}, QDir::Files,
                       QDirIterator::Subdirectories);
    QVERIFY(files.hasNext());
    const auto savedPath = files.next();
    // Block rotation of the retained generation, preserving the current file.
    QFile savedFile(savedPath);
    QVERIFY(savedFile.open(QIODevice::ReadOnly));
    const auto savedBytes = savedFile.readAll();
    savedFile.close();
    const auto previousPath = savedPath.left(savedPath.size() - 5) + ".previous";
    QVERIFY(QDir().mkdir(previousPath));
    sessionName->setProperty("text", "polish-worker");
    QVERIFY(QMetaObject::invokeMethod(save, "click"));
    QCOMPARE(text("session_save_state"), QString("pending"));
    QTRY_COMPARE(text("session_save_state"), QString("failed"));
    QCOMPARE(sessionName->property("text").toString(), QString("polish-worker"));
    QVERIFY(!text("session_save_detail").isEmpty());
    auto sessionFeedback = window->findChild<QQuickItem *>("sessionSaveFeedback");
    QVERIFY(sessionFeedback && sessionFeedback->isVisible());
    QCOMPARE(sessionFeedback->property("message").toString(), text("session_save_message"));
    QVERIFY(savedFile.open(QIODevice::ReadOnly));
    QCOMPARE(savedFile.readAll(), savedBytes);
    savedFile.close();
    QCOMPARE(list->property("count").toInt(), 1);
    QVERIFY(QDir().rmdir(previousPath));
    QVERIFY(QMetaObject::invokeMethod(save, "click"));
    QTRY_COMPARE(text("session_save_state"), QString("succeeded"));
    QTRY_COMPARE(sessionName->property("text").toString(), QString());
    QVERIFY(QFileInfo::exists(savedPath));
    QVERIFY(evaluate("testWindow.browserUi.delete_named_session('polish-worker',true)").toBool());
    QTRY_COMPARE(text("session_delete_state"), QString("succeeded"));
    QTRY_COMPARE(list->property("count").toInt(), 0);
    QTRY_VERIFY(!evaluate("testWindow.browserUi.session_names_pending").toBool());
    QTest::qWait(250);
    QVERIFY(!evaluate("testWindow.browserUi.session_names_pending").toBool());
    const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
    if (!artifacts.isEmpty()) QVERIFY(window->grabWindow().save(artifacts + "/production-manager-feedback.png"));
    key(Qt::Key_Escape);
}

void ApplicationTests::productPolishUnavailableStorage() {
    QTRY_VERIFY(!evaluate("testWindow.browserUi.profile_bootstrap_pending").toBool());
    QVERIFY(!evaluate("testWindow.temporaryProfile").toBool());
    QVERIFY(!command("history"));
    QVERIFY(text("status_text").contains("Profile storage is unavailable"));
    QVERIFY(!text("status_text").contains("Private"));
    QVERIFY(!evaluate("testWindow.libraryManagerVisible").toBool());
    QVERIFY(!evaluate("testWindow.browserUi.queue_bookmark_transfer('https://example.com/page','Retained draft')").toBool());
    QCOMPARE(text("bookmark_transfer_state"), QString("failed"));
    QVERIFY(!text("bookmark_transfer_detail").isEmpty());
}

void ApplicationTests::productPolishLocalFilePaths() {
    const auto path = directory.filePath(QString::fromUtf8("résumé + 100% #?.txt"));
    const auto encoded = QUrl::fromLocalFile(path).toString(QUrl::FullyEncoded);
    QCOMPARE(ferric_browser_local_file_path(QUrl::fromEncoded(encoded.toUtf8(), QUrl::StrictMode)), path);
    QCOMPARE(evaluate("testWindow.browserUi.local_file_path(" + quoted(encoded) + ")").toString(), path);
    QCOMPARE(ferric_browser_local_file_path(QUrl("file://localhost/tmp/hello%20world")), QString("/tmp/hello world"));
    for (const auto &invalid : QStringList{"https://example.com/file", "file:relative.txt", "file://server/tmp/file", "file:///tmp/file?query", "file:///tmp/file#fragment", "file:///tmp/bad%00name", "file:///tmp/bad%xy", "file://user@localhost/tmp/file", ""}) {
        QVERIFY2(ferric_browser_local_file_path(QUrl::fromEncoded(invalid.toUtf8(), QUrl::StrictMode)).isEmpty(), qPrintable(invalid));
    }
}

void ApplicationTests::productPolishDiagnosticLifecycle_data() {
    QTest::addColumn<bool>("privateRoot");
    QTest::newRow("regular") << false;
    QTest::newRow("private") << true;
}

void ApplicationTests::productPolishDiagnosticLifecycle() {
    QFETCH(bool, privateRoot);
    QCOMPARE(evaluate("testWindow.temporaryProfile").toBool(), privateRoot);
    const auto path = directory.filePath("pending-report.json");
    evaluate("testWindow.browserUi.record_request_resolution('page-dialog','resolved')");
    // This method returns while the worker is collecting, without pumping an
    // event loop or granting export authorization before the report is visible.
    evaluate("testWindow.showDiagnostics()");
    QVERIFY(evaluate("testWindow.browserUi.diagnostics_pending").toBool());
    QVERIFY(!evaluate("testWindow.browserUi.diagnostics_available").toBool());
    QVERIFY(!evaluate("testWindow.browserUi.export_diagnostics(" + quoted(path) + ")").toBool());
    auto close = findVisualItem(window->contentItem(), "diagnosticsClose");
    QVERIFY(close && close->isEnabled());
    QVERIFY(!findVisualItem(window->contentItem(), "diagnosticsCopy")->isEnabled());
    QVERIFY(!findVisualItem(window->contentItem(), "diagnosticsSave")->isEnabled());
    QVERIFY(QMetaObject::invokeMethod(close, "click"));
    QVERIFY(!evaluate("testWindow.diagnosticsVisible").toBool());
    QVERIFY(!evaluate("testWindow.browserUi.diagnostics_pending").toBool());
    evaluate("testWindow.browserUi.record_request_resolution('page-dialog','resolved')");
    evaluate("testWindow.showDiagnostics()");
    QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.browserUi.diagnostics_available").toBool(), 16000);
    const auto report = QJsonDocument::fromJson(evaluate("testWindow.diagnosticsText").toString().toUtf8()).object();
    bool latest = false;
    for (const auto &value : report["request_resolutions"].toObject()["counts"].toArray()) {
        const auto row = value.toObject();
        if (row["kind"] == "page-dialog" && row["outcome"] == "resolved") latest = row["count"].toInt() == 2;
    }
    QVERIFY2(latest, qPrintable(QString::fromUtf8(QJsonDocument(report["request_resolutions"].toObject()).toJson())));
    QVERIFY(!QFile::exists(path));
    if (privateRoot) QVERIFY(evaluate("testWindow.browserUi.diagnostics_summary.join(' ').indexOf('kept in memory')>=0").toBool());
    evaluate("testWindow.refreshDiagnostics()");
    QVERIFY(evaluate("testWindow.browserUi.diagnostics_pending").toBool());
    const auto nextFontSize = evaluate("testWindow.browserUi.chrome_font_size_pt + 1").toDouble();
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('ui.font_size_pt'," + quoted(QString::number(nextFontSize)) + ",true)").toBool());
    QTRY_VERIFY(!evaluate("testWindow.browserUi.diagnostics_pending").toBool());
    QVERIFY(!evaluate("testWindow.browserUi.diagnostics_available").toBool());
    QVERIFY(!text("diagnostics_error").isEmpty());
    QVERIFY(!evaluate("testWindow.browserUi.export_diagnostics(" + quoted(path) + ")").toBool());
    evaluate("testWindow.refreshDiagnostics()");
    QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.browserUi.diagnostics_available").toBool(), 16000);
    QCOMPARE(text("diagnostics_error"), QString());
    evaluate("testWindow.closeDiagnostics()");
}

void ApplicationTests::productPolishDiagnosticReport() {
    const auto path = directory.filePath(QString::fromUtf8("reviewed résumé + 100%.json"));
    // Startup compatibility checks must not authorize export of a report that
    // has never been opened. They also must not replace an already reviewed one.
    evaluate("testWindow.refreshEngineUpdateNotice()");
    QVERIFY(!evaluate("testWindow.browserUi.export_diagnostics(" + quoted(path) + ")").toBool());
    QVERIFY(!QFile::exists(path));
    QElapsedTimer reportCreation;
    reportCreation.start();
    QVERIFY(command("diagnostics"));
    QTRY_VERIFY(evaluate("testWindow.diagnosticsVisible").toBool());
    qInfo() << "diagnostics panel opened elapsed ms" << reportCreation.elapsed();
    QTRY_VERIFY(evaluate("testWindow.browserUi.diagnostics_available").toBool());
    qInfo() << "diagnostics report ready elapsed ms" << reportCreation.elapsed();
    QVERIFY(evaluate("testWindow.browserUi.diagnostics_summary.length >= 3").toBool());
    const auto preview = evaluate("testWindow.diagnosticsText").toString();
    QVERIFY(QJsonDocument::fromJson(preview.toUtf8()).isObject());
    auto report = findVisualItem(window->contentItem(), "diagnosticsReportText");
    QVERIFY(report && !report->isVisible());
    evaluate("testWindow.browserUi.record_request_resolution('page-dialog','resolved')");
    evaluate("testWindow.refreshEngineUpdateNotice()");
    QCOMPARE(evaluate("testWindow.diagnosticsText").toString(), preview);
    auto chooser = window->findChild<QObject *>("diagnosticsExportChooser");
    QVERIFY(chooser);
    QFile existing(directory.filePath("fixture.html"));
    QVERIFY(existing.open(QIODevice::ReadOnly));
    const auto original = existing.readAll();
    existing.close();
    auto save = findVisualItem(window->contentItem(), "diagnosticsSave");
    QVERIFY(save);
    QVERIFY(QMetaObject::invokeMethod(save, "click"));
    QTRY_VERIFY(chooser->property("visible").toBool());
    QCOMPARE(evaluate("testWindow.diagnosticsText").toString(), preview);
    QVERIFY(QMetaObject::invokeMethod(chooser, "close"));
    QVERIFY(chooser->setProperty("selectedFile", QUrl::fromLocalFile(existing.fileName())));
    QVERIFY(QMetaObject::invokeMethod(chooser, "open"));
    QTRY_VERIFY(chooser->property("visible").toBool());
    QVERIFY(QMetaObject::invokeMethod(chooser, "accept"));
    QVERIFY(evaluate("testWindow.diagnosticsExportNotice.indexOf(\"The report couldn't be saved\") === 0").toBool());
    QVERIFY(evaluate("testWindow.browserUi.diagnostics_available").toBool());
    QCOMPARE(evaluate("testWindow.diagnosticsText").toString(), preview);
    QVERIFY(existing.open(QIODevice::ReadOnly));
    QCOMPARE(existing.readAll(), original);
    existing.close();
    // Qt takes its final selection from the platform dialog when accepted.
    // Set the initial selection before opening, as required by that API.
    QVERIFY(QMetaObject::invokeMethod(chooser, "close"));
    QVERIFY(chooser->setProperty("selectedFile", QUrl::fromLocalFile(path)));
    QVERIFY(QMetaObject::invokeMethod(chooser, "open"));
    QTRY_VERIFY(chooser->property("visible").toBool());
    QVERIFY(QMetaObject::invokeMethod(chooser, "accept"));
    QTRY_VERIFY(QFile::exists(path));
    QVERIFY(evaluate("testWindow.diagnosticsExportNotice.indexOf('Report saved') === 0").toBool());
    QVERIFY(!evaluate("testWindow.browserUi.diagnostics_available").toBool());
    QFile saved(path);
    QVERIFY(saved.open(QIODevice::ReadOnly));
    QCOMPARE(saved.readAll(), preview.toUtf8());
    key(Qt::Key_Escape);
    QTRY_VERIFY(!evaluate("testWindow.diagnosticsVisible").toBool());
}

void ApplicationTests::productPolishBookmarkTransfer() {
    QTRY_VERIFY(!evaluate("testWindow.browserUi.profile_bootstrap_pending").toBool());
    QDirIterator files(evaluate("testWindow.storageBasePath").toString(), QStringList{"browser.sqlite"}, QDir::Files, QDirIterator::Subdirectories);
    QVERIFY(files.hasNext());
    sqlite3 *raw = nullptr;
    const auto databasePath = files.next().toUtf8();
    QCOMPARE(sqlite3_open_v2(databasePath.constData(), &raw, SQLITE_OPEN_READWRITE, nullptr), SQLITE_OK);
    const std::unique_ptr<sqlite3, decltype(&sqlite3_close)> database(raw, sqlite3_close);
    sqlite3_busy_timeout(database.get(), 5000);
    QCOMPARE(sqlite3_exec(database.get(), "CREATE TRIGGER polish_reject_transfer BEFORE INSERT ON bookmarks WHEN NEW.title='Fail transfer' BEGIN SELECT RAISE(ABORT, 'fixture rejected transfer'); END", nullptr, nullptr, nullptr), SQLITE_OK);
    QVERIFY(evaluate("testWindow.browserUi.queue_bookmark_transfer('https://transfer.example/page','Fail transfer')").toBool());
    const auto receipt = text("bookmark_transfer_id");
    QVERIFY(!receipt.isEmpty());
    QVERIFY(!evaluate("testWindow.browserUi.queue_bookmark_transfer('https://transfer.example/duplicate','Duplicate')").toBool());
    QVERIFY(command("bookmark-add --title 'Independent bookmark'"));
    const auto markReceipt = text("mark_action_id");
    QVERIFY(markReceipt != receipt);
    QTRY_COMPARE(text("bookmark_transfer_state"), QString("failed"));
    QTRY_COMPARE(text("mark_action_state"), QString("succeeded"));
    QCOMPARE(text("bookmark_transfer_id"), receipt);
    QVERIFY(text("bookmark_transfer_detail").contains("fixture rejected transfer"));
    QCOMPARE(sqlite3_exec(database.get(), "DROP TRIGGER polish_reject_transfer", nullptr, nullptr, nullptr), SQLITE_OK);
    // Exercise the actual presentation controller with a real destination and
    // worker, independently of private-history retention policy.
    evaluate("testWindow.privateHistoryTransferVisible=true;testWindow.privateHistoryTransferTitle='Reviewed page';testWindow.privateHistoryTransferUrl='https://transfer.example/retry'");
    evaluate("testWindow.confirmPrivateHistoryBookmark(testWindow.profileName, 'Destination')");
    QVERIFY(evaluate("testWindow.privateBookmarkTransferPending").toBool());
    QVERIFY(evaluate("testWindow.privateHistoryTransferVisible").toBool());
    QTRY_VERIFY(!evaluate("testWindow.privateBookmarkTransferPending").toBool());
    QVERIFY(!evaluate("testWindow.privateBookmarkTransferFailed").toBool());
    QCOMPARE(evaluate("testWindow.privateBookmarkTransferMessage").toString(), QString("Bookmark saved in Destination"));
    QCOMPARE(evaluate("testWindow.privateHistoryTransferUrl").toString(), QString("https://transfer.example/retry"));
    evaluate("testWindow.cancelPrivateHistoryTransfer()");
}

void ApplicationTests::productPolishLibrary() {
    QVERIFY(command("bookmark-add --title " + quoted("Original bookmark")));
    QTRY_COMPARE(text("mark_action_state"), QString("succeeded"));
    QTRY_VERIFY(!evaluate("testWindow.browserUi.library_pending").toBool());
    interactiveCommand("bookmark-list");
    QTRY_VERIFY(evaluate("testWindow.libraryManagerVisible").toBool());
    QTRY_VERIFY(text("library_values").contains("Original bookmark"));
    QTRY_VERIFY(findVisualItem(window->contentItem(), "libraryEntryEditButton"));
    auto edit = findVisualItem(window->contentItem(), "libraryEntryEditButton");
    edit->forceActiveFocus();
    key(Qt::Key_Return);
    auto input = findVisualItem(window->contentItem(), "libraryEntryEdit");
    QVERIFY(input && input->isVisible());
    input->forceActiveFocus();
    key(Qt::Key_A, Qt::ControlModifier);
    typeText("revised bookmark");
    QCOMPARE(input->property("text").toString(), QString("revised bookmark"));
    // A real background read must preserve the row and its unsaved input.
    evaluate("testWindow.browserUi.refresh_library()");
    QTRY_VERIFY(!evaluate("testWindow.browserUi.library_pending").toBool());
    QCOMPARE(findVisualItem(window->contentItem(), "libraryEntryEdit"), input);
    QCOMPARE(input->property("text").toString(), QString("revised bookmark"));
    QDirIterator files(evaluate("testWindow.storageBasePath").toString(),
                       QStringList{"browser.sqlite"}, QDir::Files,
                       QDirIterator::Subdirectories);
    QVERIFY(files.hasNext());
    sqlite3 *raw = nullptr;
    const auto databasePath = files.next().toUtf8();
    QCOMPARE(sqlite3_open_v2(databasePath.constData(), &raw, SQLITE_OPEN_READWRITE, nullptr), SQLITE_OK);
    const std::unique_ptr<sqlite3, decltype(&sqlite3_close)> database(raw, sqlite3_close);
    sqlite3_busy_timeout(database.get(), 5000);
    QCOMPARE(sqlite3_exec(database.get(), "CREATE TRIGGER polish_reject_edit BEFORE UPDATE ON bookmarks BEGIN SELECT RAISE(ABORT, 'fixture rejected bookmark update'); END", nullptr, nullptr, nullptr), SQLITE_OK);
    key(Qt::Key_Return);
    QTRY_COMPARE(text("mark_action_state"), QString("failed"));
    QVERIFY(input->isVisible());
    QCOMPARE(input->property("text").toString(), QString("revised bookmark"));
    QVERIFY(text("library_values").contains("Original bookmark"));
    QVERIFY(!text("library_values").contains("revised bookmark"));
    auto feedback = window->findChild<QQuickItem *>("libraryOperationFeedback");
    QVERIFY(feedback && feedback->isVisible());
    QCOMPARE(feedback->property("message").toString(), text("mark_action_message"));
    QVERIFY(text("mark_action_detail").contains("fixture rejected"));
    QCOMPARE(sqlite3_exec(database.get(), "DROP TRIGGER polish_reject_edit", nullptr, nullptr, nullptr), SQLITE_OK);
    input->forceActiveFocus(); key(Qt::Key_Return);
    QTRY_COMPARE(text("mark_action_state"), QString("succeeded"));
    QTRY_VERIFY(!input->isVisible());
    QTRY_VERIFY(text("library_values").contains("revised bookmark"));
    QVERIFY(!text("library_values").contains("Original bookmark"));
    // Read errors retain the displayed catalog and can recover without restart.
    QCOMPARE(sqlite3_exec(database.get(), "ALTER TABLE bookmarks RENAME TO polish_saved_bookmarks", nullptr, nullptr, nullptr), SQLITE_OK);
    evaluate("testWindow.browserUi.refresh_library()");
    QTRY_VERIFY(!text("library_error").isEmpty());
    QVERIFY(text("library_values").contains("revised bookmark"));
    QCOMPARE(sqlite3_exec(database.get(), "ALTER TABLE polish_saved_bookmarks RENAME TO bookmarks", nullptr, nullptr, nullptr), SQLITE_OK);
    evaluate("testWindow.browserUi.refresh_library()");
    QTRY_VERIFY(!evaluate("testWindow.browserUi.library_pending").toBool());
    QTRY_COMPARE(text("library_error"), QString());
    // Two-step deletion is followed by a durable catalog publication.
    auto remove = findVisualItem(window->contentItem(), "libraryEntryDeleteButton");
    QVERIFY(remove && remove->isVisible());
    QVERIFY(QMetaObject::invokeMethod(remove, "click"));
    QVERIFY(!evaluate("testWindow.pendingLibraryDelete").toString().isEmpty());
    QVERIFY(QMetaObject::invokeMethod(remove, "click"));
    QCOMPARE(text("mark_action_state"), QString("pending"));
    QTRY_COMPARE(text("mark_action_state"), QString("succeeded"));
    QTRY_COMPARE(text("library_values"), QString());
    auto list = window->findChild<QQuickItem *>("libraryEntryList");
    QVERIFY(list); QTRY_COMPARE(list->property("count").toInt(), 0);
    key(Qt::Key_Escape);
    QVERIFY(command("quickmark-add paper"));
    QTRY_COMPARE(text("mark_action_state"), QString("succeeded"));
    QTRY_VERIFY(!evaluate("testWindow.browserUi.library_pending").toBool());
    interactiveCommand("quickmark-list");
    QTRY_VERIFY(evaluate("testWindow.libraryManagerVisible").toBool());
    QTRY_VERIFY(text("library_values").startsWith("paper\t"));
    QTRY_VERIFY(findVisualItem(window->contentItem(), "libraryEntryEditButton"));
    edit = findVisualItem(window->contentItem(), "libraryEntryEditButton");
    QVERIFY(edit); edit->forceActiveFocus(); key(Qt::Key_Return);
    input = findVisualItem(window->contentItem(), "libraryEntryEdit");
    QVERIFY(input && input->isVisible()); input->forceActiveFocus();
    key(Qt::Key_A, Qt::ControlModifier);
    const auto quickmarkUrl = text("current_url") + "?saved=quickmark";
    typeText(quickmarkUrl); key(Qt::Key_Return);
    QTRY_COMPARE(text("mark_action_state"), QString("succeeded"));
    QTRY_VERIFY(text("library_values").contains("saved=quickmark"));
    QTRY_VERIFY(!input->isVisible());
    window->resize(390, 700);
    evaluate("testWindow.font.pointSize=16");
    QTest::qWait(100);
    const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
    if (!artifacts.isEmpty()) QVERIFY(window->grabWindow().save(artifacts + "/production-library-large.png"));
    key(Qt::Key_Escape);
    const auto originalPageUrl = text("current_url");
    const auto laterPageUrl = originalPageUrl + "?journey=later";
    evaluate("testWindow.browserUi.navigate(" + quoted(laterPageUrl) + ")");
    evaluate("testWindow.executePendingEngineAction()");
    QTRY_COMPARE(text("current_url"), laterPageUrl);
    QTRY_VERIFY(!evaluate("testWindow.activeWebView().loading").toBool());
    QTest::qWait(150);
    evaluate("testWindow.browserUi.refresh_library()");
    QTRY_VERIFY(!evaluate("testWindow.browserUi.library_pending").toBool());
    QTRY_VERIFY(command("journey"));
    QVERIFY(command("action-list"));
    QTRY_VERIFY(!evaluate("testWindow.browserUi.library_pending").toBool());
    QTest::qWait(150);
    QCOMPARE(text("library_kind"), QString("actions"));
    QVERIFY(!text("library_values").isEmpty());
    interactiveCommand("journey");
    QTRY_VERIFY(evaluate("testWindow.libraryManagerVisible").toBool());
    QTRY_COMPARE(text("library_kind"), QString("journey"));
    QTRY_VERIFY(!text("library_values").isEmpty());
    list = window->findChild<QQuickItem *>("libraryEntryList");
    QVERIFY(list); QTRY_VERIFY(list->property("count").toInt() > 0);
    const auto journeyRows = text("library_values").split('\n');
    int originalIndex = -1;
    for (int index = 0; index < journeyRows.size(); ++index) {
        const auto fields = journeyRows[index].split('\t');
        if (fields.size() >= 3 && fields[2] == originalPageUrl) originalIndex = index;
    }
    QVERIFY(originalIndex >= 0);
    QVERIFY(list->setProperty("currentIndex", originalIndex));
    QTRY_VERIFY(list->property("currentItem").value<QObject *>() != nullptr);
    // The real projection must carry its kind and node identity to reopen a
    // different saved page, rather than only dismissing the panel.
    const auto currentUrl = originalPageUrl;
    page("window.polishJourneyMarker='before';true");
    list->forceActiveFocus(); key(Qt::Key_Return);
    QTRY_VERIFY(!evaluate("testWindow.libraryManagerVisible").toBool());
    QTRY_COMPARE(text("current_url"), currentUrl);
    QTRY_VERIFY(page("window.polishJourneyMarker === undefined && document.getElementById('button') !== null").toBool());
}

void ApplicationTests::productPolishPrivateLibrary() {
    interactiveCommand("history");
    QTRY_VERIFY(evaluate("testWindow.libraryManagerVisible").toBool());
    QCOMPARE(text("library_kind"), QString("history"));
    const auto before = text("library_values");
    QVERIFY(before.contains(text("current_url")));
    evaluate("testWindow.refreshActiveLibrary()");
    QCOMPARE(text("library_error"), QString());
    QVERIFY(!evaluate("testWindow.browserUi.library_pending").toBool());
    QCOMPARE(text("library_values"), before);
    key(Qt::Key_Escape);
    interactiveCommand("action-list");
    QTRY_VERIFY(evaluate("testWindow.libraryManagerVisible").toBool());
    QVERIFY(evaluate("testWindow.refreshActiveLibrary()").toBool());
    QCOMPARE(text("library_error"), QString());
    QCOMPARE(text("library_kind"), QString("actions"));
    QVERIFY(!text("library_values").isEmpty());
    key(Qt::Key_Escape);
    // A private session cannot checkpoint to disk. Dirty in-memory state must
    // not leave the runtime scheduling itself every millisecond forever.
    QTRY_VERIFY_WITH_TIMEOUT(evaluate("(function(){var delay=testWindow.browserUi.maintenance_delay_ms();return delay===-1 || delay>=1000})()").toBool(), 5000);
    const auto privateUrl = text("current_url");
    QVERIFY(evaluate("testWindow.temporaryProfile").toBool());
    // Seed a regular destination through its own normal window, then close it.
    // The private source never gains profile-write authority or navigates there.
    evaluate("testWindow.browserWindowFactory.createObject(null, {windowProfileName:'polish-private-destination',windowStartupUrl:'rb://start',windowPrivateProfile:false})");
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
    QTRY_VERIFY(!evaluate("testWindow.browserWindowRegistry[1].ui.profile_bootstrap_pending").toBool());
    evaluate("testWindow.browserWindowRegistry[1].host.windowShutdownApproved=true;testWindow.browserWindowRegistry[1].host.close()");
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 1);
    evaluate("testWindow.refreshProfiles()");
    QTRY_VERIFY(text("profile_values").contains("polish-private-destination"));
    evaluate("testWindow.showPrivateHistoryTransfer('reviewed-private-entry','Reviewed private page'," + quoted(privateUrl) + ")");
    evaluate("testWindow.confirmPrivateHistoryBookmark('polish-private-destination','Destination')");
    QVERIFY(evaluate("testWindow.privateBookmarkTransferPending").toBool());
    evaluate("testWindow.showPrivateHistoryTransfer('another-entry','Another page','https://other.example/')");
    QCOMPARE(evaluate("testWindow.privateHistoryTransferUrl").toString(), privateUrl);
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
    QTRY_VERIFY2_WITH_TIMEOUT(!evaluate("testWindow.privateBookmarkTransferPending").toBool(), qPrintable(evaluate("JSON.stringify({receipt:testWindow.privateBookmarkTransferReceipt,tracking:!!testWindow.privateBookmarkTransferUi,state:testWindow.browserWindowRegistry[1].ui.bookmark_transfer_state,id:testWindow.browserWindowRegistry[1].ui.bookmark_transfer_id,bootstrap:testWindow.browserWindowRegistry[1].ui.profile_bootstrap_pending,url:testWindow.browserWindowRegistry[1].host.windowBookmarkTransferUrl,status:testWindow.browserWindowRegistry[1].ui.status_text})").toString()), 10000);
    QVERIFY2(!evaluate("testWindow.privateBookmarkTransferFailed").toBool(), qPrintable(evaluate("testWindow.privateBookmarkTransferMessage").toString()));
    const auto destination = QString("testWindow.browserWindowRegistry[1]");
    QVERIFY(!evaluate(destination + ".ephemeralProfile").toBool());
    QVERIFY(!evaluate(destination + ".host.windowPrivateProfile").toBool());
    QCOMPARE(evaluate(destination + ".host.activeView.nativeUrl").toString(), QString("rb://start"));
    QVERIFY(evaluate(destination + ".ui.current_url").toString() != privateUrl);
    QTRY_VERIFY(evaluate(destination + ".ui.execute_command('bookmark-list')").toBool());
    QTRY_VERIFY(evaluate(destination + ".ui.library_values").toString().contains(privateUrl));
    QVERIFY(evaluate(destination + ".ui.library_values").toString().contains("Reviewed private page"));
    QTRY_VERIFY(evaluate(destination + ".ui.execute_command('history')").toBool());
    QVERIFY(!evaluate(destination + ".ui.library_values").toString().contains(privateUrl));
    QCOMPARE(text("current_url"), privateUrl);
    evaluate("testWindow.cancelPrivateHistoryTransfer()");
}

void ApplicationTests::clipboardOtherWindows_data() {
  QTest::addColumn<bool>("popup");
  QTest::newRow("secondary") << false;
  QTest::newRow("popup") << true;
}
void ApplicationTests::clipboardOtherWindows() {
  QFETCH(bool, popup);
  QString entry;
  if (popup) {
    QVERIFY(command("bind p hint --target pointer links"));
    key(Qt::Key_P);
    const auto label = hintLabel("Open popup");
    QVERIFY(!label.isEmpty());
    typeText(label);
    key(Qt::Key_Return);
    QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(), 1);
    entry = "testWindow.popupWindowRegistry[0]";
    pageReceiver = entry + ".view";
  } else {
    QVERIFY(evaluate("testWindow.openEphemeralWindow(" +
                     quoted(text("current_url")) + ")")
                .toBool());
    QTRY_VERIFY(
        evaluate("testWindow.browserWindowRegistry.length > 1").toBool());
    entry = "testWindow.browserWindowRegistry[testWindow.browserWindowRegistry."
            "length-1]";
    pageReceiver = entry + ".host.activeView";
  }
  auto child = qobject_cast<QQuickWindow *>(
      evaluate(entry + ".host").value<QObject *>());
  QVERIFY(child);
  QTRY_VERIFY(
      evaluate(pageReceiver + " !== null && !" + pageReceiver + ".loading")
          .toBool());
  QTRY_VERIFY(page("document.getElementById('button') !== null").toBool());
  evaluate(pageReceiver + ".viewUi.escape()");
  child->requestActivate();
  QTRY_VERIFY(child->isActive());
  evaluate(pageReceiver + ".forceActiveFocus()");
  QTest::qWait(100);
  page("document.body.innerHTML='<input id=input "
       "value=Other-window-copy>';document.getElementById('input').focus();"
       "document.getElementById('input').select();true");
  QTRY_VERIFY(page("document.hasFocus()").toBool());
  QCOMPARE(page("document.activeElement.value").toString(),
           QString("Other-window-copy"));
  QGuiApplication::clipboard()->setText("sentinel");
  QTest::keyClick(child, Qt::Key_C, Qt::ControlModifier);
  QTRY_COMPARE(QGuiApplication::clipboard()->text(),
               QString("Other-window-copy"));
  page("let e=document.getElementById('input');e.value='';e.focus();true");
  QGuiApplication::clipboard()->setText("Other-window-paste");
  QTest::keyClick(child, Qt::Key_V, Qt::ControlModifier);
  QTRY_COMPARE(page("document.getElementById('input').value").toString(),
               QString("Other-window-paste"));
  auto view =
      qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());
  QVERIFY(view);
  page("document.getElementById('input').select();true");
  auto local = page("let "
                    "r=document.getElementById('input').getBoundingClientRect()"
                    ";({x:r.left+25,y:r.top+16})")
                   .toMap();
  auto point =
      view->mapToScene(QPointF(local["x"].toDouble(), local["y"].toDouble()));
  QTest::mouseClick(child, Qt::RightButton, Qt::NoModifier, point.toPoint());
  QTRY_VERIFY(
      evaluate("testWindow.pendingContextMenuSelection !== null").toBool());
  QGuiApplication::clipboard()->setText("sentinel");
  evaluate("testWindow.activateContextMenuItem(testWindow.contextMenuItems."
           "filter(function(item){return item.action==='web-copy'})[0])");
  QTest::qWait(150);
  qInfo()<<"other-window context edit outcome"<<text("status_text")<<evaluate(pageReceiver+".viewUi.status_text");
  QTRY_COMPARE(QGuiApplication::clipboard()->text(),
               QString("Other-window-paste"));
  evaluate(entry + ".host.destroy()");
  pageReceiver = "testWindow.activeWebView()";
}
