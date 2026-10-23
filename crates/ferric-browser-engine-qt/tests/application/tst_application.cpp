#include "tst_application.h"
#include "owned_renderer_process.h"
#include "../../src/url_display.h"
#include "../../src/gui_application.h"
#include <QClipboard>
#include <QtWebEngineQuick/QQuickWebEngineProfile>
#include <QMimeData>
#include <QMetaProperty>
#include <QMetaMethod>
#include <QProcess>
#include <QCryptographicHash>
#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QFileInfo>
#include <QFileDialog>
#include <QFont>
#include <QGuiApplication>
#include <QApplication>
#include <QAbstractButton>
#include <QMessageBox>
#include <QTimer>
#include <QImage>
#include <QScopeGuard>
#include <QHostAddress>
#include <QQmlApplicationEngine>
#include <QQmlComponent>
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
        qFatal("QML expression %s: %s", qPrintable(source), qPrintable(expression.error().toString()));
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
    // Binding persistence is asynchronous. Tests that use a new key must wait
    // for its publication receipt before sending native keyboard input.
    if (accepted && (value.startsWith("bind ") || value.startsWith("unbind "))) {
        QElapsedTimer deadline;
        deadline.start();
        while (!text("status_text").startsWith("Binding saved")
               && !text("status_text").startsWith("Binding removed")
               && deadline.elapsed() < 5000) QTest::qWait(5);
        if (!text("status_text").startsWith("Binding saved")
                && !text("status_text").startsWith("Binding removed")) {
            qInfo() << "binding publication failed" << value << text("status_text");
            return false;
        }
    }
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
                if (path == "/lifetime-download") {
                    socket->write("HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=lifetime.txt\r\nContent-Length: 8\r\nConnection: close\r\n\r\nlifetime");
                    socket->disconnectFromHost();
                    return;
                }
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
    const auto dataTag = QString::fromLatin1(QTest::currentDataTag());
    const bool identityPrivate = testName == "productPolishPrivacyIdentity" && dataTag.endsWith("-private");
    const bool identityTemporary = testName == "productPolishPrivacyIdentity" && dataTag.endsWith("-temporary");
    const bool recoveryTemporary = (testName == "productPolishTabRecovery" || testName == "productPolishLastTabUndo")
        && dataTag.startsWith("temporary-");
    const bool durable = !identityPrivate && !identityTemporary && !recoveryTemporary && (testName.startsWith("durablePersistence")
                         || testName == "selectorValidationRejectsWholeCandidate"
                         || (testName.startsWith("productPolish") && testName != "productPolishPrivateLibrary"
                             && !(testName == "productPolishDiagnosticLifecycle" && QString::fromLatin1(QTest::currentDataTag()) == "private")));
    QString startupConfig = "{\"schema_version\":3}";
    if (testName == "productPolishExternalActionFeedback") {
        const auto tag = QString::fromLatin1(QTest::currentDataTag());
        const auto subject = tag.startsWith("selection") ? "selection" : "url";
        const QJsonArray argv{"-c", "printf '%s' \"$$\" > \"$1\"; printf '%s' \"$2\" > \"$3\"",
            "fixture", directory.filePath("action-child.pid"),
            QString::fromLatin1(subject)=="selection" ? "{selection}" : "{url}",
            directory.filePath("action-subject.txt")};
        startupConfig = QString::fromUtf8(QJsonDocument(QJsonObject{
            {"schema_version",3}, {"action_targets",QJsonObject{{"fixture",QJsonObject{
                {"executable","/bin/sh"}, {"argv",argv}, {"subject_types",QJsonArray{subject}},
                {"detach",tag.contains("detached")}, {"allow_private",false}
            }}}}
        }).toJson(QJsonDocument::Compact));
    }
    if (testName == "productPolishNativeNotifications"
            && QString::fromLatin1(QTest::currentDataTag()) == "required-portal")
        startupConfig = R"JSON({"schema_version":3,"desktop":{"portals":"required"}})JSON";
    if (testName == "productPolishPermissionReadiness" || testName == "productPolishPermissionCapacity")
        startupConfig = R"JSON({"schema_version":3,"permissions":{"geolocation":"allow"}})JSON";
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
    QString startupConfigPath;
    if (testName == "productPolishStartupProblems" || testName == "productPolishProfileRecovery") {
        startupConfigPath = directory.filePath(testName=="productPolishProfileRecovery"
            ? "configuration/root/config.toml" : "private-startup-config.toml");
        QVERIFY(QDir().mkpath(QFileInfo(startupConfigPath).absolutePath()));
        QFile invalidConfig(startupConfigPath);
        QVERIFY(invalidConfig.open(QIODevice::WriteOnly));
        invalidConfig.write("schema_version = [ malformed private setting");
    }
    engine->setInitialProperties(
        {{"temporaryProfile", !durable},
         {"ephemeralProfile", identityTemporary || recoveryTemporary},
         {"ephemeralInvocationToken", identityTemporary ? QString("privacy-identity-fixture")
             : recoveryTemporary ? QString("tab-recovery-fixture") : QString()},
         {"storageBasePath", storageBase},
         {"softwareRendering", true},
         {"nativeWayland", QGuiApplication::platformName() == "wayland"},
         {"startupConfigJson", startupConfig},
         {"startupConfigPath", startupConfigPath},
         {"startupConfigBaseJson", startupConfig},
         {"startupCliOverridesJson", "{\"schema_version\":3}"},
         {"startupProfileOverridesJson", "{\"schema_version\":3}"},
         {"safeMode", true},
         {"userscriptsOff", true},
         {"width", 1000},
         {"height", 800},
         {"startupCleanLink", testName == "productPolishStartupCleanLink"},
         {"startupUrl", QString("http://127.0.0.1:%1/fixture.html%2")
                            .arg(server.serverPort()).arg(testName == "productPolishStartupCleanLink"
                                ? "?keep=yes&utm_source=polish" : "")}});
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
    if (testName == "productPolishStartupCleanLink") {
        window->show();window->requestActivate();
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.profileBootstrapComplete").toBool(),10000);
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.browserUi.link_preview_visible").toBool(),10000);
        QTRY_VERIFY(window->isActive());
        return;
    }
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
QString ApplicationTests::hintLabel(const QString &name, const QString &ownerUi) {
    // Wait for the mode footer's first viewport update and the corresponding
    // dirty-snapshot refresh before aiming at a label.
    QTest::qWait(180);
    QElapsedTimer deadline;
    deadline.start();
    do {
        const auto values =
            QJsonDocument::fromJson(evaluate(ownerUi+".hint_values").toString().toUtf8()).array();
        for (const auto &value : values) {
            const auto object = value.toObject();
            if (object.value("text").toString() == name)
                return object.value("label").toString();
        }
        QTest::qWait(20);
    } while (deadline.elapsed() < 5000);
    qInfo() << "missing hint" << name << text("status_text") << evaluate(ownerUi+".mode");
    return {};
}
bool ApplicationTests::chooseCapability(const QString &name, QQuickWindow *ownerWindow) {
    const auto action = "hint.capability." + name;
    const auto index =
        evaluate("testWindow.contextMenuItems.findIndex(function("
                 "item) { return item.actionId === " +
                 quoted(action) + " && item.enabled; })")
            .toInt();
    if (index < 0)
        return false;
    if (!ownerWindow) ownerWindow = window;
    QTest::qWait(80);
    for (int i = 0; i < index; ++i) {
        QTest::keyClick(ownerWindow, Qt::Key_Down);QTest::qWait(30);
    }
    QTest::keyClick(ownerWindow, Qt::Key_Return);QTest::qWait(30);
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
    QTRY_COMPARE(QJsonDocument::fromJson(text("config_json").toUtf8()).object()["content"].toObject()["zoom"].toDouble(), 1.1);
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
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('content.zoom',"
                      "'1.25',false)")
                 .toBool());
    const auto failedReceipt = text("setting_action_id");
    QVERIFY(!failedReceipt.isEmpty());
    QCOMPARE(text("setting_action_state"), QString("pending"));
    QVERIFY(!evaluate("testWindow.browserUi.set_runtime_setting('content.zoom','1.5',false)").toBool());
    QCOMPARE(text("setting_action_id"), failedReceipt);
    QTRY_COMPARE(text("setting_action_state"), QString("failed"));
    QCOMPARE(text("setting_action_id"), failedReceipt);
    QVERIFY(text("setting_action_message").contains("Couldn't save content.zoom"));
    QVERIFY(!text("setting_action_detail").isEmpty());
    QVERIFY(text("setting_action_detail").contains("runtime-overrides.toml"));
    QTRY_VERIFY(text("status_text").contains("Your current settings are still active"));
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
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('content.zoom','1.25',false)").toBool());
    QVERIFY(text("setting_action_id") != failedReceipt);
    QTRY_COMPARE(text("setting_action_state"), QString("succeeded"));
    QVERIFY(text("setting_action_message").contains("Saved content.zoom"));
    QVERIFY(text("setting_action_detail").isEmpty());
    QTRY_VERIFY(text("config_json") != accepted);
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
    const auto rejectedReceipt = text("setting_action_id");
    QVERIFY(!rejectedReceipt.isEmpty());
    QTRY_COMPARE(text("setting_action_state"), QString("failed"));
    QCOMPARE(text("setting_action_id"), rejectedReceipt);
    QVERIFY(!text("setting_action_detail").isEmpty());
    QTRY_VERIFY_WITH_TIMEOUT(
        text("status_text").contains("selector", Qt::CaseInsensitive), 6000);
    QTRY_VERIFY_WITH_TIMEOUT(
        text("status_text").contains("failed", Qt::CaseInsensitive), 6000);
    QCOMPARE(text("config_json"), accepted);
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('hints.selectors.all', "
                     + quoted("{mode='extend',selectors=['[data-ferric-owned]']}") + ",false)").toBool());
    QVERIFY(text("setting_action_id") != rejectedReceipt);
    const auto retriedReceipt = text("setting_action_id");
    QTRY_COMPARE(text("setting_action_state"), QString("succeeded"));
    QCOMPARE(text("setting_action_id"), retriedReceipt);
    QVERIFY(text("setting_action_message").contains("Saved hints.selectors.all"));
    QVERIFY(text("setting_action_message").contains(":config-reload"));
    QVERIFY(text("setting_action_detail").isEmpty());
    // Selector policy is a reload-boundary setting. Saving it must not
    // silently change the current effective policy.
    QCOMPARE(text("config_json"), accepted);
    QFile persisted(directory.filePath("durable-test/state/runtime-overrides.toml"));
    QVERIFY(persisted.open(QIODevice::ReadOnly));
    QVERIFY(persisted.readAll().contains("[data-ferric-owned]"));
    const auto saved = text("config_json");
    QVERIFY(!evaluate("testWindow.browserUi.set_runtime_setting('ui.font_size_pt','-1',true)").toBool());
    QCOMPARE(text("setting_action_state"), QString("failed"));
    QVERIFY(text("setting_action_id") != retriedReceipt);
    QVERIFY(!text("setting_action_detail").isEmpty());
    QCOMPARE(text("config_json"), saved);
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('desktop.portals','\"required\"',false)").toBool());
    QTRY_COMPARE(text("setting_action_state"), QString("succeeded"));
    QVERIFY(text("setting_action_message").contains("restart Ferric"));
    QCOMPARE(QJsonDocument::fromJson(text("config_json").toUtf8()).object()["desktop"].toObject()["portals"].toString(), QString("auto"));
    QCOMPARE(evaluate("(function(){var rows=testWindow.browserUi.settings_rows;for(var i=0;i<rows.length;++i)if(rows[i].key==='desktop.portals')return rows[i].value;return ''})()").toString(), QString("required"));
}

void ApplicationTests::productPolishSettingDurability() {
    if (qEnvironmentVariable("FERRIC_SETTINGS_SYNC_FIXTURE") != "1")
        QSKIP("Use application_settings_durability with its owned sync-failure interposer");
    QVERIFY(command("set content.zoom=1.1"));
    QTRY_COMPARE(QJsonDocument::fromJson(text("config_json").toUtf8()).object()["content"].toObject()["zoom"].toDouble(), 1.1);
    const auto base = directory.filePath("durable-test/productPolishSettingDurability/");
    QDirIterator overrides(base, {"runtime-overrides.toml"}, QDir::Files, QDirIterator::Subdirectories);
    QVERIFY(overrides.hasNext());
    const auto path = overrides.next();
    QVERIFY(!overrides.hasNext());
    evaluate("testWindow.showSettings('content.zoom')");
    auto surface = window->findChild<QQuickItem *>("settingsSurface");
    QVERIFY(surface && surface->isVisible());
    engine->rootContext()->setContextProperty("settingsWorkspaceUnderTest", surface->parentItem());
    QVERIFY(!qEnvironmentVariableIsSet("FERRIC_SETTINGS_SYNC_FAULT_PATH"));
    qputenv("FERRIC_SETTINGS_SYNC_FAULT_PATH", QFile::encodeName(QFileInfo(path).absolutePath()));
    const auto clearFault = qScopeGuard([] { qunsetenv("FERRIC_SETTINGS_SYNC_FAULT_PATH"); });
    QVERIFY(evaluate("settingsWorkspaceUnderTest.applySetting({key:'content.zoom',type:'number'},'1.2')").toBool());
    QTRY_COMPARE(text("setting_action_state"), QString("succeeded"));
    QVERIFY(text("setting_action_message").contains("system crash"));
    QVERIFY(text("setting_action_detail").contains("saving may not survive a system crash"));
    QCOMPARE(surface->property("notice").toString(), text("setting_action_message"));
    QCOMPARE(surface->property("noticeDetail").toString(), text("setting_action_detail"));
    QVERIFY(!surface->property("noticeFailed").toBool());
    QVERIFY(!surface->property("noticePending").toBool());
    QCOMPARE(QJsonDocument::fromJson(text("config_json").toUtf8()).object()["content"].toObject()["zoom"].toDouble(), 1.2);
    QFile saved(path);
    QVERIFY(saved.open(QIODevice::ReadOnly));
    QVERIFY(saved.readAll().contains("\"content.zoom\" = 1.2"));
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
    QVERIFY(text("status_text").contains("some changes need a browser restart"));
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
    QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.browserUi.profile_startup_details()").toString().contains("Selector validation failed"),6000);
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
    QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.browserUi.profile_startup_details()").toString().contains("timed out"),7000);
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
    QVERIFY(text("status_text").contains("Selector settings changed"));
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
    const auto failure = FerricTest::terminateOwnedRenderer(pid);
    QVERIFY2(failure.isEmpty(), qPrintable(failure));
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
    QVector<QByteArray> arguments;
    for (int index = 0; index < argc; ++index) arguments.append(QByteArray(argv[index]));
    auto application = ferric_browser_gui_application(arguments);
    if (!application || !ferric_browser_gui_preferences_ready()) return 1;
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
    QCOMPARE(evaluate("testWindow.hintWorkspace.hintCollisionLabels("+quoted(before)+").length").toInt(),2);
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
        QTRY_VERIFY(evaluate(entry + ".host.profileBootstrapComplete").toBool());
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
        QTRY_VERIFY(evaluate("testWindow.browserWindowRegistry.length > 1").toBool());
        entry = "testWindow.browserWindowRegistry[testWindow.browserWindowRegistry.length-1].host";
        QTRY_VERIFY(evaluate(entry + ".profileBootstrapComplete && !!" + entry + ".activeView").toBool());
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
        QTRY_VERIFY(evaluate("testWindow.browserWindowRegistry.length > 1").toBool());
        entry = "testWindow.browserWindowRegistry[testWindow.browserWindowRegistry.length-1].host";
        QTRY_VERIFY(evaluate(entry + ".profileBootstrapComplete && !!" + entry + ".activeView").toBool());
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
        QTRY_VERIFY(evaluate("testWindow.browserWindowRegistry.length > 1").toBool());
        entry = "testWindow.browserWindowRegistry[testWindow.browserWindowRegistry.length-1].host";
        QTRY_VERIFY(evaluate(entry + ".profileBootstrapComplete && !!" + entry + ".activeView").toBool());
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


void ApplicationTests::productPolishNativePageHistory_data() {
    QTest::addColumn<bool>("secondary");
    QTest::newRow("primary") << false;
    QTest::newRow("secondary") << true;
}
void ApplicationTests::productPolishNativePageHistory() {
    QFETCH(bool,secondary);
    const auto initial=text("current_url");
    QString owner="({ui:testWindow.browserUi,host:testWindow})";
    if (secondary) {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"
            + quoted(initial) + ",windowProfileName:'native-history-owner',windowProfileLabel:'Native history owner'})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
        owner="testWindow.browserWindowRegistry[1]";
        QTRY_VERIFY(evaluate(owner + ".host.profileBootstrapComplete").toBool());
    }
    pageReceiver=owner+".host.activeWebView()";
    QTRY_VERIFY(page("location.href === " + quoted(initial) + " && document.readyState === 'complete'").toBool());
    page("window.retainedAcrossNativePages=61");
    for (const auto &destination : {QString("rb://blank"),QString("rb://start")}) {
        evaluate(owner + ".host.submitInteractiveCommand(" + quoted("open " + destination) + ")");
        QTRY_COMPARE(evaluate(pageReceiver + ".nativeUrl").toString(),destination);
        QTRY_COMPARE(evaluate(owner + ".ui.current_url").toString(),destination);
    }
    evaluate(owner + ".host.submitInteractiveCommand('back')");
    QTRY_COMPARE(evaluate(pageReceiver + ".nativeUrl").toString(),QString("rb://blank"));
    evaluate(owner + ".host.submitInteractiveCommand('back')");
    QTRY_COMPARE(evaluate(pageReceiver + ".nativeUrl").toString(),QString());
    QTRY_COMPARE(evaluate(owner + ".ui.current_url").toString(),initial);
    QCOMPARE(page("window.retainedAcrossNativePages").toInt(),61);
    for (const auto &destination : {QString("rb://blank"),QString("rb://start")}) {
        evaluate(owner + ".host.submitInteractiveCommand('forward')");
        QTRY_COMPARE(evaluate(pageReceiver + ".nativeUrl").toString(),destination);
        QTRY_COMPARE(evaluate(owner + ".ui.current_url").toString(),destination);
    }
    if (secondary) QCOMPARE(text("current_url"),initial);
}

void ApplicationTests::deferredNativeRequestsKeepCapturedTargets() {
    QTest::failOnWarning(QRegularExpression(".*(TypeError|ReferenceError|invalid context).*"));
    auto ui = evaluate("testWindow.browserUi").value<QObject *>();
    QVERIFY(ui);
    for (const char *name : {"page_userscript_sources", "external_action_ids", "start_page_urls",
                             "switcher_result_ids", "session_preview_urls"}) {
        const auto index = ui->metaObject()->indexOfProperty(name);
        QVERIFY2(index >= 0, name);
        QCOMPARE(ui->metaObject()->property(index).metaType().id(), QMetaType::QStringList);
    }
    const auto requestMethod = ui->metaObject()->indexOfMethod("take_engine_request()");
    QVERIFY(requestMethod >= 0);
    QCOMPARE(ui->metaObject()->method(requestMethod).returnMetaType().id(), QMetaType::QVariant);
    QVERIFY(evaluate(R"JS((function() {
        var ui = testWindow.browserUi;
        var before = ui.config_json;
        return !ui.set_startup_configuration(before, ui.config_base_json,
            ui.cli_overrides_json, ui.profile_overrides_json, ui.config_path, ui.config_source)
            && ui.config_json === before;
    })())JS").toBool());
    // Consume within one JS turn, before the scheduled native pass can run.
    const auto reload = evaluate(R"JS((function() {
        var ui = testWindow.browserUi;
        ui.new_tab(); ui.select_tab(0);
        var original = ui.tab_id_for_index(0);
        var accepted = ui.execute_command("reload ;; tab-next");
        var request = ui.take_engine_request();
        return [accepted, original, ui.tab_id_for_index(ui.active_tab_index), request.kind, request.tabId];
    })())JS").toList();
    QCOMPARE(reload.size(), 5);
    QVERIFY(reload[0].toBool());
    QVERIFY(reload[1].toString() != reload[2].toString());
    QCOMPARE(reload[3].toString(), QStringLiteral("reload"));
    QCOMPARE(reload[4].toString(), reload[1].toString());

    const auto searches = evaluate(R"JS((function() {
        var ui = testWindow.browserUi;
        ui.select_tab(0);
        var original = ui.tab_id_for_index(0);
        var accepted = ui.execute_command('search "Captured query" --case sensitive ;; search-next --count 2 ;; tab-next');
        var requests = [];
        for (var i = 0; i < 3; ++i) {
            var request = ui.take_engine_request();
            requests.push(request.kind, request.tabId, request.query, request.caseMode);
        }
        return [accepted, original].concat(requests);
    })())JS").toList();
    QCOMPARE(searches.size(), 14);
    QVERIFY(searches[0].toBool());
    for (int i = 2; i < searches.size(); i += 4) {
        QCOMPARE(searches[i].toString(), QStringLiteral("find"));
        QCOMPARE(searches[i+1].toString(), searches[1].toString());
        QCOMPARE(searches[i+2].toString(), QStringLiteral("Captured query"));
        QCOMPARE(searches[i+3].toString(), QStringLiteral("sensitive"));
    }
    const auto captured = evaluate(R"JS((function() {
        var ui = testWindow.browserUi;
        ui.select_tab(0);
        var tab = ui.tab_id_for_index(0);
        var accepted = ui.execute_command('scroll down --count 2 ;; jseval "first()" ;; jseval "second()" ;; tab-next');
        var scroll = ui.take_engine_request();
        var first = ui.take_engine_request();
        var second = ui.take_engine_request();
        return [accepted, tab, scroll.kind, scroll.tabId, scroll.direction, scroll.count,
            first.kind, first.tabId, first.script, second.kind, second.tabId, second.script];
    })())JS").toList();
    QCOMPARE(captured.size(), 12);
    QVERIFY(captured[0].toBool());
    QCOMPARE(captured[2].toString(), QStringLiteral("scroll"));
    QCOMPARE(captured[3].toString(), captured[1].toString());
    QCOMPARE(captured[4].toString(), QStringLiteral("down"));
    QCOMPARE(captured[5].toInt(), 2);
    QCOMPARE(captured[6].toString(), QStringLiteral("evaluate"));
    QCOMPARE(captured[7].toString(), captured[1].toString());
    QCOMPARE(captured[8].toString(), QStringLiteral("first()"));
    QCOMPARE(captured[9].toString(), QStringLiteral("evaluate"));
    QCOMPARE(captured[10].toString(), captured[1].toString());
    QCOMPARE(captured[11].toString(), QStringLiteral("second()"));
    const auto blocking = evaluate(R"JS((function() {
        var ui = testWindow.browserUi;
        ui.select_tab(0);
        var before = ui.blocking_bypass_sites.join("\n");
        var accepted = ui.execute_command("blocking-toggle --site ;; tab-next");
        return [accepted, before !== ui.blocking_bypass_sites.join("\n")];
    })())JS").toList();
    QCOMPARE(blocking.size(), 2);
    QVERIFY(blocking[0].toBool());
    QVERIFY(blocking[1].toBool());
    for (const auto &transfer : {QStringLiteral("tab-detach"), QStringLiteral("tab-give missing-window")}) {
        const auto request = evaluate(QStringLiteral(R"JS((function() {
            var ui = testWindow.browserUi;
            ui.select_tab(0);
            var tab = ui.tab_id_for_index(0);
            var accepted = ui.execute_command(%1 + " ;; tab-next");
            var fields = ui.take_engine_action().split("\t");
            return [accepted, tab, fields[fields.length - 1]];
        })())JS").arg(quoted(transfer))).toList();
        QCOMPARE(request.size(), 3);
        QVERIFY(request[0].toBool());
        QCOMPARE(request[2].toString(), request[1].toString());
    }
    QVERIFY(evaluate(R"JS((function() {
        var ui = testWindow.browserUi;
        ui.select_tab(0); ui.execute_command("reload"); ui.close_tab(0);
        var request = ui.take_engine_request();
        return !request || !request.kind;
    })())JS").toBool());
}

void ApplicationTests::productPolishSecondaryHistoryCommands_data() {
    QTest::addColumn<bool>("forward");
    QTest::newRow("back") << false;
    QTest::newRow("forward") << true;
}
void ApplicationTests::productPolishSecondaryHistoryCommands() {
    QFETCH(bool, forward);
    const auto first = text("current_url");
    evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"
        + quoted(first) + ",windowProfileName:'history-owner',windowProfileLabel:'History owner'})");
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
    const auto token = evaluate("testWindow.browserWindowRegistry[1].windowToken").toString();
    const auto owner = "testWindow.browserWindowRegistry.find(function(e){return e.windowToken === " + quoted(token) + "})";
    QTRY_VERIFY(evaluate(owner + ".host.profileBootstrapComplete").toBool());
    pageReceiver = owner + ".host.activeView";
    QTRY_VERIFY(page("location.href === " + quoted(first) + " && document.readyState === 'complete'").toBool());
    const auto second = first + "?history-second=1";
    evaluate(owner + ".host.submitInteractiveCommand(" + quoted("open " + second) + ")");
    QTRY_VERIFY(page("location.href === " + quoted(second) + " && document.readyState === 'complete'").toBool());
    if (forward) {
        evaluate(owner + ".host.submitInteractiveCommand('back')");
        QTRY_VERIFY(page("location.href === " + quoted(first)).toBool());
        QTRY_VERIFY(evaluate(owner + ".host.activeView.contentCanGoForward").toBool());
    } else QTRY_VERIFY(evaluate(owner + ".host.activeView.contentCanGoBack").toBool());
    evaluate(owner + ".host.submitInteractiveCommand(" + quoted(forward ? "forward" : "back") + ")");
    QTRY_COMPARE_WITH_TIMEOUT(page("location.href").toString(),forward ? second : first,2000);
    QCOMPARE(text("current_url"),first);
}
void ApplicationTests::productPolishTransferredSourceClose_data() {
    QTest::addColumn<bool>("privateProfile");
    QTest::addColumn<bool>("ephemeralProfile");
    QTest::newRow("regular") << false << false;
    QTest::newRow("private") << true << false;
    QTest::newRow("ephemeral") << false << true;
}
void ApplicationTests::productPolishTransferredSourceClose() {
    QTest::failOnWarning(QRegularExpression(".*(invalid context|TypeError|ReferenceError).*"));
    QFETCH(bool,privateProfile);
    QFETCH(bool,ephemeralProfile);
    const auto initial = text("current_url");
    evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"
        + quoted(initial) + ",windowProfileName:'lifetime-owner',windowProfileLabel:'Lifetime owner',windowPrivateProfile:"
        + (privateProfile ? "true" : "false") + ",windowEphemeralProfile:" + (ephemeralProfile ? "true" : "false")
        + ",windowEphemeralInvocationToken:'native-lifetime-probe'})");
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
    const auto token = evaluate("testWindow.browserWindowRegistry[1].windowToken").toString();
    const auto source = "testWindow.browserWindowRegistry.find(function(e){return e.windowToken === " + quoted(token) + "})";
    QTRY_VERIFY(evaluate(source + ".host.profileBootstrapComplete").toBool());
    pageReceiver = source + ".host.activeView";
    QTRY_VERIFY(page("location.href === " + quoted(initial) + " && document.readyState === 'complete'").toBool());
    page("window.retainedAfterOwnerClose=77;document.getElementById('input').value='Unsubmitted form draft';true");
    const QPointer<QObject> live = evaluate(source + ".host.activeView").value<QObject *>();
    QPointer<QQuickWebEngineProfile> profile=qobject_cast<QQuickWebEngineProfile *>(
        evaluate(source + ".host.windowWebEngineProfile").value<QObject *>());
    QVERIFY(profile);
    QCOMPARE(profile->isOffTheRecord(),privateProfile || ephemeralProfile);
    QVERIFY(evaluate(source + ".ui.execute_secondary_interactive_command('tab-detach')").toBool());
    evaluate(source + ".host.processRuntimeWork()");
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),3);
    const auto movedToken = evaluate("testWindow.browserWindowRegistry.find(function(e){return e.windowToken !== "
        + quoted(token) + " && e.host !== testWindow}).windowToken").toString();
    const auto moved = "testWindow.browserWindowRegistry.find(function(e){return e.windowToken === " + quoted(movedToken) + "})";
    QTRY_VERIFY(evaluate(moved + ".host.profileBootstrapComplete").toBool());
    QTRY_VERIFY(live && evaluate(moved + ".host.activeView").value<QObject *>()==live.data());
    QCOMPARE(evaluate(moved + ".host.windowPrivateProfile").toBool(),privateProfile);
    QCOMPARE(evaluate(moved + ".host.windowEphemeralProfile").toBool(),ephemeralProfile);
    QPointer<QQuickWindow> child=qobject_cast<QQuickWindow *>(evaluate(source + ".host").value<QObject *>());
    QVERIFY(child);
    evaluate(source + ".host.windowShutdownApproved=true");
    QVERIFY(child->close());
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
    child->deleteLater();
    QCoreApplication::sendPostedEvents(nullptr,QEvent::DeferredDelete);
    QTRY_VERIFY(child.isNull());
    QVERIFY2(live,"The transferred native view must survive its source window");
    QVERIFY2(profile,"The transferred native profile must survive its source window");
    for (int cycle=0;cycle<3;++cycle) {
        engine->collectGarbage();
        QCoreApplication::sendPostedEvents(nullptr,QEvent::DeferredDelete);
        QTest::qWait(30);
    }
    pageReceiver = moved + ".host.activeView";
    QCOMPARE(page("window.retainedAfterOwnerClose").toInt(),77);
    QCOMPARE(page("document.getElementById('input').value").toString(),QString("Unsubmitted form draft"));
    QVERIFY(evaluate(moved + ".host.activeView.profile === " + moved + ".host.windowWebEngineProfile").toBool());
    QCOMPARE(page("location.href").toString(),initial);
    QCOMPARE(text("current_url"),initial);
    page("location.href=new URL('/lifetime-download',location.href).href;true");
    QTRY_VERIFY2(evaluate(moved + ".host.pendingDownloadId.length>0").toBool(),
        "The real download chooser must belong to the destination window");
    QCOMPARE(evaluate("testWindow.pendingDownloadId").toString(),QString());
    evaluate(moved + ".host.cancelPendingDownload()");
    QTRY_VERIFY(evaluate(moved + ".host.pendingDownloadId.length===0").toBool());
    QPointer<QQuickWindow> destination=qobject_cast<QQuickWindow *>(evaluate(moved + ".host").value<QObject *>());
    QVERIFY(destination);
    evaluate(moved + ".host.windowShutdownApproved=true");
    QVERIFY(destination->close());
    destination->deleteLater();
    QCoreApplication::sendPostedEvents(nullptr,QEvent::DeferredDelete);
    QTRY_VERIFY(destination.isNull());
    QTRY_VERIFY(live.isNull());
    QTRY_VERIFY(profile.isNull());
    pageReceiver="testWindow.activeWebView()";
}
void ApplicationTests::productPolishPopupSourceClose_data() {
    QTest::addColumn<bool>("privateProfile");
    QTest::addColumn<bool>("ephemeralProfile");
    QTest::newRow("regular") << false << false;
    QTest::newRow("private") << true << false;
    QTest::newRow("ephemeral") << false << true;
}
void ApplicationTests::productPolishPopupSourceClose() {
    QTest::failOnWarning(QRegularExpression(".*(invalid context|TypeError|ReferenceError).*"));
    QFETCH(bool,privateProfile);
    QFETCH(bool,ephemeralProfile);
    const auto initial=text("current_url");
    evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"
        + quoted(initial) + ",windowProfileName:'popup-owner',windowProfileLabel:'Popup owner',windowPrivateProfile:"
        + (privateProfile ? "true" : "false") + ",windowEphemeralProfile:" + (ephemeralProfile ? "true" : "false")
        + ",windowEphemeralInvocationToken:'native-popup-owner'})");
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
    const auto source=QString("testWindow.browserWindowRegistry[1]");
    QTRY_VERIFY(evaluate(source + ".host.profileBootstrapComplete").toBool());
    pageReceiver=source + ".host.activeView";
    QTRY_VERIFY(page("document.getElementById('blank')!==null && document.readyState==='complete'").toBool());
    QPointer<QQuickWindow> sourceWindow=qobject_cast<QQuickWindow *>(evaluate(source + ".host").value<QObject *>());
    QPointer<QObject> sourceUi=evaluate(source + ".ui").value<QObject *>();
    auto sourceView=qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());
    QVERIFY(sourceWindow && sourceView && sourceUi);
    sourceWindow->requestActivate();QTRY_VERIFY(sourceWindow->isActive());
    const auto point=page("(()=>{const r=document.getElementById('blank').getBoundingClientRect();return [r.x+r.width/2,r.y+r.height/2]})()").toList();
    QCOMPARE(point.size(),2);
    QTest::mouseClick(sourceWindow,Qt::LeftButton,Qt::NoModifier,
        sourceView->mapToScene(QPointF(point[0].toDouble(),point[1].toDouble())).toPoint());
    QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(),1);
    const auto popup=QString("testWindow.popupWindowRegistry[0]");
    QPointer<QObject> view=evaluate(popup + ".view").value<QObject *>();
    QPointer<QQuickWebEngineProfile> profile=qobject_cast<QQuickWebEngineProfile *>(
        evaluate(popup + ".view.profile").value<QObject *>());
    QVERIFY(view && profile);
    QCOMPARE(profile->isOffTheRecord(),privateProfile || ephemeralProfile);
    QVERIFY(evaluate(popup + ".ui === " + popup + ".view.viewUi && "
        + popup + ".ui !== " + source + ".ui").toBool());
    pageReceiver=popup + ".view";
    QTRY_VERIFY(page("document.getElementById('input')!==null && document.readyState==='complete'").toBool());
    page("window.popupRetainedAfterOwnerClose=91;document.getElementById('input').value='Popup form draft';true");
    QTRY_VERIFY(evaluate(popup + ".ui.blocking_policy_ready && !" + popup + ".ui.profile_bootstrap_pending").toBool());
    const auto retainedPolicy=evaluate("JSON.stringify(testWindow.profileResourceRegistry.forProfile(" + popup + ".view.profile).policySnapshot)").toString();
    // A replacement owner may still be preparing policy when its opener exits.
    // Keep the installed rules until that owner publishes a complete snapshot.
    evaluate(popup + ".ui.blocking_policy_ready=false");
    evaluate(source + ".host.windowShutdownApproved=true");
    QVERIFY(sourceWindow->close());
    sourceWindow->deleteLater();
    QCoreApplication::sendPostedEvents(nullptr,QEvent::DeferredDelete);
    QTRY_VERIFY(sourceWindow.isNull());
    QTRY_VERIFY(sourceUi.isNull());
    QTest::qWait(50);
    QVERIFY2(profile,"A popup profile must survive destruction of its opener window");
    QVERIFY2(view,"A popup page must survive destruction of its opener window");
    QCOMPARE(evaluate("JSON.stringify(testWindow.profileResourceRegistry.forProfile(" + popup + ".view.profile).policySnapshot)").toString(),retainedPolicy);
    evaluate(popup + ".ui.blocking_policy_ready=true");
    QVERIFY(evaluate(popup + ".view.viewUi!==null && " + popup + ".view.viewUi.core_window_id.length>0").toBool());
    QCOMPARE(page("window.popupRetainedAfterOwnerClose").toInt(),91);
    QCOMPARE(page("document.getElementById('input').value").toString(),QString("Popup form draft"));
    QPointer<QObject> popupUi=evaluate(popup + ".view.viewUi").value<QObject *>();
    QVERIFY2(popupUi && popupUi.data()!=evaluate("testWindow.browserUi").value<QObject *>(),
        "Popup commands and permissions must not fall back to an unrelated primary UI");
    auto child=qobject_cast<QQuickWindow *>(evaluate(popup + ".host").value<QObject *>());
    QVERIFY(child);
    page("history.replaceState(null,'','/popup-after-opener-close');true");
    QTRY_COMPARE(evaluate(popup + ".ui.current_url").toString(),page("location.href").toString());
    QCOMPARE(text("current_url"),initial);
    page("setTimeout(function(){window.afterOpenerAnswer=confirm('The opener has closed')},0);true");
    QTRY_VERIFY(evaluate("testWindow.pendingPageDialogRequest !== null").toBool());
    auto dialog=findVisualItem(child->contentItem(),"pageDialogPopup");
    QVERIFY(dialog && dialog->isVisible());
    QCOMPARE(dialog->window(),child);
    QVERIFY(evaluate("testWindow.nativePromptActiveFor(" + popup + ".ui)").toBool());
    QVERIFY(!evaluate("testWindow.nativePromptActiveFor(testWindow.browserUi)").toBool());
    evaluate("testWindow.rejectPageDialog()");
    QTRY_VERIFY(evaluate("testWindow.pendingPageDialogRequest === null").toBool());
    QCOMPARE(page("window.afterOpenerAnswer").toBool(),false);
    page("location.href=new URL('/lifetime-download',location.href).href;true");
    QTRY_VERIFY2(evaluate(popup + ".host.pendingDownloadId.length>0").toBool(),
        "The popup must retain its actual download handler after the opener is destroyed");
    QCOMPARE(evaluate("testWindow.pendingDownloadId").toString(),QString());
    evaluate(popup + ".host.cancelPendingDownload()");
    QTRY_VERIFY(evaluate(popup + ".host.pendingDownloadId.length===0").toBool());
    evaluate(popup + ".host.windowShutdownApproved=true");
    QVERIFY(child->close());
    child->deleteLater();
    QCoreApplication::sendPostedEvents(nullptr,QEvent::DeferredDelete);
    QTRY_VERIFY(view.isNull());
    QTRY_VERIFY(profile.isNull());
    QTRY_VERIFY(popupUi.isNull());
    QTRY_COMPARE(evaluate("testWindow.profileResourceRegistry.groups.length").toInt(),1);
    pageReceiver="testWindow.activeWebView()";
}
void ApplicationTests::productPolishNewWindowRouting_data() {
    QTest::addColumn<bool>("secondary");
    QTest::addColumn<QString>("decision");
    for (bool secondary : {false,true})
        for (const auto &decision : {QString("context"),QString("normal"),QString("private")})
            QTest::newRow(qPrintable((secondary ? "secondary-" : "primary-")+decision)) << secondary << decision;
}

void ApplicationTests::productPolishNewWindowRouting() {
    QFETCH(bool,secondary);
    QFETCH(QString,decision);
    QTest::failOnWarning(QRegularExpression(".*(invalid context|TypeError|ReferenceError).*"));
    const auto initial=text("current_url");
    QString owner="testWindow.browserWindowRegistry[0]";
    if (secondary) {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"
            +quoted(initial)+",windowProfileName:'default',windowProfileLabel:'Default'})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
        owner="testWindow.browserWindowRegistry[1]";
        QTRY_VERIFY(evaluate(owner+".host.profileBootstrapComplete").toBool());
    }
    auto host=qobject_cast<QQuickWindow *>(evaluate(owner+".host").value<QObject *>());
    QVERIFY(host);
    host->requestActivate();QTRY_VERIFY(host->isActive());
    pageReceiver=owner+".view";
    QTRY_VERIFY(page("document.readyState==='complete' && document.getElementById('input')!==null").toBool());
    page("window.routingSourceState=73");
    QVERIFY(evaluate(owner+".ui.execute_command('context-create routed --profile default')").toBool());
    QTRY_VERIFY(evaluate(owner+".ui.status_text.indexOf('Context created: routed')===0").toBool());
    // Ordinary fixtures start without a writable user config. Supply a valid
    // disposable config and authored context before testing route persistence.
    const auto configDirectory=directory.path()+"/routing-config-"+QString::fromLatin1(QTest::currentDataTag());
    QVERIFY(QDir().mkpath(configDirectory));
    QFile configFile(configDirectory+"/config.toml");
    QVERIFY(configFile.open(QIODevice::WriteOnly));configFile.write("schema_version=3\n");configFile.close();
    evaluate(owner+".ui.config_path="+quoted(configFile.fileName()));
    const QJsonObject contexts{{"schema_version",3},{"contexts",QJsonArray{QJsonObject{
        {"name","routed"},{"label","Routed"},{"profile","default"}}}}};
    QVERIFY(evaluate(owner+".ui.set_contexts_configuration("+quoted(QString::fromUtf8(QJsonDocument(contexts).toJson(QJsonDocument::Compact)))+")").toBool());
    QVERIFY2(evaluate(owner+".ui.execute_command("+quoted("context-route add "+initial+" routed --entry-point explicit-open")+")").toBool(),
        qPrintable(evaluate(owner+".ui.status_text").toString()));
    const auto before=evaluate("testWindow.liveWindowEntries().length").toInt();
    const auto tokens=evaluate("JSON.stringify(testWindow.liveWindowEntries().map(function(e){return e.windowToken}))").toString();
    QVERIFY(evaluate(owner+".ui.execute_command("+quoted("open --target "+QString(decision=="private" ? "private-window " : "window ")+initial)+")").toBool());
    evaluate("testWindow.completeBrowserKeyAction("+owner+".ui,"+owner+".host)");
    if (decision!="private") {
        QTRY_VERIFY(evaluate("testWindow.contextRouteVisible").toBool());
        QVERIFY(evaluate("testWindow.contextRouteUi === "+owner+".ui && testWindow.contextRouteHost === "+owner+".host").toBool());
        auto safe=findVisualItem(host->contentItem(),"commandDialogAction-normal");
        QVERIFY(safe);QCOMPARE(safe->window(),host);QTRY_VERIFY(safe->hasActiveFocus());
        const auto artifacts=qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
        if (!artifacts.isEmpty()) {
            QDir().mkpath(artifacts);
            QVERIFY(host->grabWindow().save(artifacts+"/production-"
                +(secondary ? "secondary" : "primary")+"-context-route-"+decision+".png"));
        }
        QTest::keyClick(host,decision=="context" ? Qt::Key_Y : Qt::Key_N);
        QTRY_VERIFY(!evaluate("testWindow.contextRouteVisible").toBool());
    } else {
        QVERIFY(!evaluate("testWindow.contextRouteVisible").toBool());
    }
    QTRY_COMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(),before+1);
    const auto destination="testWindow.liveWindowEntries().find(function(e){return !"+tokens+".includes(e.windowToken)})";
    QVERIFY2(evaluate("!!("+destination+")").toBool(),qPrintable(evaluate(
        "JSON.stringify(testWindow.liveWindowEntries().map(function(e){return [e.windowToken,e.profileName,e.ui.status_text,e.host.profileBootstrapComplete]}))").toString()));
    const auto destinationAdmission=evaluate("JSON.stringify(testWindow.liveWindowEntries().map(function(e){return [e.windowToken,e.profileName,e.ui.status_text,e.host.profileBootstrapComplete]}))").toString();
    QTRY_VERIFY2(evaluate("(function(){var e="+destination+";return !!e && !!e.host && e.host.profileBootstrapComplete})()").toBool(),
        qPrintable(destinationAdmission+" -> "+evaluate("JSON.stringify(testWindow.liveWindowEntries().map(function(e){return [e.windowToken,e.profileName,e.ui.status_text,e.host.profileBootstrapComplete]}))").toString()));
    pageReceiver=destination+".view";
    QTRY_VERIFY(page("document.readyState==='complete' && document.getElementById('input')!==null").toBool());
    QCOMPARE(page("location.href").toString(),initial);
    QCOMPARE(evaluate(destination+".ui.context_name").toString(),decision=="context" ? QString("routed") : QString());
    QCOMPARE(evaluate(destination+".privateProfile").toBool(),decision=="private");
    QCOMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(),before+1);
    pageReceiver=owner+".view";
    QCOMPARE(page("window.routingSourceState").toInt(),73);
    QCOMPARE(page("location.href").toString(),initial);
    pageReceiver="testWindow.activeWebView()";
}

void ApplicationTests::productPolishContextCatalogRetainedAfterFailedProfile() {
    const auto profile=evaluate("testWindow.browserWindowRegistry[0].profileName").toString();
    const auto initialWindow=text("core_window_id");
    QVERIFY(command("context-create polish-retained --profile "+profile));
    QTRY_VERIFY(evaluate("testWindow.browserUi.context_choice_names.indexOf('polish-retained')>=0").toBool());
    QVERIFY(command("window-new"));
    evaluate("testWindow.completeBrowserKeyAction(testWindow.browserUi,testWindow)");
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
    QTRY_VERIFY(evaluate("testWindow.browserWindowRegistry[1].host.profileBootstrapComplete").toBool());
    QVERIFY(evaluate("(function(){var peer=testWindow.browserWindowRegistry[1];var accepted=peer.ui.execute_command('context-delete polish-retained --confirm');testWindow.browserUi.configure_profile(false,false,'','',testWindow.storageBasePath);return accepted})()").toBool());
    QTRY_VERIFY(!evaluate("testWindow.browserUi.profile_bootstrap_pending").toBool());
    QCOMPARE(text("core_window_id"),initialWindow);
    QTRY_VERIFY2(evaluate("testWindow.browserUi.context_choice_names.indexOf('polish-retained')<0").toBool(),"A failed replacement must preserve delivery to the retained active profile");
}

void ApplicationTests::productPolishStartupCleanLink_data() {
    QTest::addColumn<bool>("confirm");
    QTest::newRow("cancel") << false;
    QTest::newRow("confirm") << true;
}
void ApplicationTests::productPolishStartupCleanLink() {
    QFETCH(bool,confirm);
    QVERIFY(evaluate("testWindow.browserUi.link_preview_requires_confirmation").toBool());
    QTRY_VERIFY(evaluate("testWindow.modalInputScope.activeSurface && testWindow.modalInputScope.activeSurface.commandText===':open --clean-link'").toBool());
    const auto initial=evaluate("testWindow.activeWebView().url.toString()").toString();
    QVERIFY2(!initial.startsWith("http://127.0.0.1:"),"A startup preview must not navigate before confirmation");
    QTest::qWait(200); // Initial native-content focus/layout must not dismiss the held preview.
    QVERIFY(evaluate("testWindow.browserUi.link_preview_visible").toBool());
    QTest::keyClick(window,confirm ? Qt::Key_Y : Qt::Key_Escape);
    QTRY_VERIFY(!evaluate("testWindow.browserUi.link_preview_visible").toBool());
    if(confirm) {
        const auto expected=QString("http://127.0.0.1:%1/fixture.html?keep=yes").arg(server.serverPort());
        QTRY_COMPARE(text("current_url"),expected);
        QTRY_COMPARE(evaluate("testWindow.activeWebView().url.toString()").toString(),expected);
        QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.browserUi.load_state==='complete' && !testWindow.activeWebView().loading").toBool(),15000);
        QVERIFY(page("!!document.getElementById('button')").toBool());
    } else {
        QCOMPARE(evaluate("testWindow.activeWebView().url.toString()").toString(),initial);
        QTRY_VERIFY2(evaluate("(function(){var target=testWindow.activeWebView();var focus=testWindow.activeFocusItem;while(focus && focus!==target)focus=focus.parent;return focus===target})()").toBool(),qPrintable(evaluate("JSON.stringify({nativeUrl:testWindow.activeWebView().nativeUrl,focus:testWindow.activeFocusItem?String(testWindow.activeFocusItem):'none'})").toString()));
    }
}

void ApplicationTests::productPolishCleanLinkOwner_data() {
    QTest::addColumn<QString>("kind");
    for (const auto &kind : {QString("primary"),QString("secondary"),QString("private"),QString("popup")}) QTest::newRow(qPrintable(kind)) << kind;
}
void ApplicationTests::productPolishCleanLinkOwner() {
    QFETCH(QString,kind);
    QString owner="testWindow.browserWindowRegistry[0]";
    if (kind!="primary") {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"+quoted(text("current_url"))+",windowProfileName:'link-owner',windowProfileLabel:'Link owner',windowPrivateProfile:"+(kind=="private"?"true":"false")+"})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
        owner="testWindow.browserWindowRegistry[1]";
        QTRY_VERIFY(evaluate(owner+".host.profileBootstrapComplete").toBool());
    }
    if(kind=="popup") {
        pageReceiver=owner+".view";
        QTRY_VERIFY_WITH_TIMEOUT(evaluate(owner+".ui.load_state==='complete' && !"+owner+".view.loading && "+owner+".view.url.toString().indexOf('http://127.0.0.1:')===0").toBool(),15000);
        QVERIFY(page("document.readyState==='complete' && !!document.getElementById('blank')").toBool());
        auto host=qobject_cast<QQuickWindow *>(evaluate(owner+".host").value<QObject *>());
        auto view=qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());
        QVERIFY(host && view);host->requestActivate();QTRY_VERIFY(host->isActive());
        const auto point=page("(()=>{const r=document.getElementById('blank').getBoundingClientRect();return [r.x+r.width/2,r.y+r.height/2]})()").toList();
        QTest::mouseClick(host,Qt::LeftButton,Qt::NoModifier,view->mapToScene(QPointF(point[0].toDouble(),point[1].toDouble())).toPoint());
        QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(),1);
        owner="testWindow.popupWindowRegistry[0]";
    }
    auto host=qobject_cast<QQuickWindow *>(evaluate(owner+".host").value<QObject *>());
    auto view=qobject_cast<QQuickItem *>(evaluate(owner+".view").value<QObject *>());
    QVERIFY(host && view);
    pageReceiver=owner+".view";
    QTRY_VERIFY_WITH_TIMEOUT(evaluate(owner+".ui.load_state==='complete' && !"+owner+".view.loading && "+owner+".view.url.toString().indexOf('http://127.0.0.1:')===0").toBool(),15000);
    QVERIFY(page("document.readyState==='complete'").toBool());
    const auto initial=evaluate(owner+".ui.current_url").toString();
    const auto primaryInitial=text("current_url");
    const auto clean=QString("http://127.0.0.1:%1/cleaned?keep=yes").arg(server.serverPort());
    const auto dirty=clean+"&utm_source=polish";
    const auto submit=[&](const QString &value) {
        host->requestActivate();
        QTRY_VERIFY(host->isActive());
        evaluate(owner+".ui.enter_command()");
        auto input=host->findChild<QQuickItem *>("commandLineInput");
        QVERIFY(input);
        QVERIFY(input->setProperty("text",value));
        input->forceActiveFocus();
        QTRY_VERIFY(input->hasActiveFocus());
        QTest::keyClick(host,Qt::Key_Return);
    };
    submit("url-clean "+dirty);
    QTRY_VERIFY2(evaluate(owner+".ui.link_preview_visible").toBool(),qPrintable(evaluate(owner+".ui.status_text").toString()));
    QTRY_VERIFY(evaluate(owner+".host.modalInputScope.activeSurface && "+owner+".host.modalInputScope.activeSurface.commandText===':url-clean'").toBool());
    QCOMPARE(evaluate(owner+".ui.current_url").toString(),initial);
    QVERIFY(!evaluate(owner+".ui.link_preview_requires_confirmation").toBool());
    if(kind!="primary") {
        QVERIFY(!evaluate("testWindow.browserChromeInputActive").toBool());
        QVERIFY(command("url-explain "+dirty));
        QTRY_VERIFY(evaluate("testWindow.linkPreviewVisible").toBool());
    }
    host->requestActivate();QTRY_VERIFY(host->isActive());
    QTest::keyClick(host,Qt::Key_Escape);
    QTRY_VERIFY(!evaluate(owner+".ui.link_preview_visible").toBool());
    if(kind!="primary") {
        QVERIFY(evaluate("testWindow.linkPreviewVisible").toBool());
        evaluate("testWindow.browserUi.clear_link_preview()");
    }
    QTRY_VERIFY(view->hasActiveFocus());
    submit("open --clean-link "+dirty);
    QTRY_VERIFY2(evaluate(owner+".ui.link_preview_visible && "+owner+".ui.link_preview_requires_confirmation").toBool(),qPrintable(evaluate(owner+".ui.status_text").toString()));
    QCOMPARE(evaluate(owner+".ui.current_url").toString(),initial);
    QTRY_VERIFY(evaluate(owner+".host.modalInputScope.activeSurface.commandText===':open --clean-link'").toBool());
    const auto artifacts=qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
    if(!artifacts.isEmpty()) {
        QDir().mkpath(artifacts);
        QVERIFY(host->grabWindow().save(artifacts+"/production-clean-link-"+kind+".png"));
    }
    QTest::keyClick(host,Qt::Key_Y);
    QTRY_VERIFY(!evaluate(owner+".ui.link_preview_visible").toBool());
    QTRY_COMPARE(evaluate(owner+".ui.current_url").toString(),clean);
    QTRY_COMPARE(evaluate(owner+".view.url.toString()").toString(),clean);
    QTRY_VERIFY_WITH_TIMEOUT(evaluate(owner+".ui.load_state==='complete' && !"+owner+".view.loading && "+owner+".view.url.toString().indexOf('http://127.0.0.1:')===0").toBool(),15000);
    QVERIFY(page("document.readyState==='complete'").toBool());
    if(kind!="primary") QCOMPARE(text("current_url"),primaryInitial);
    // A navigation of the same view must invalidate a held preview, including reloads.
    submit("open --clean-link "+dirty);
    QTRY_VERIFY(evaluate(owner+".ui.link_preview_visible").toBool());
    evaluate(owner+".view.reload()");
    QTRY_VERIFY(!evaluate(owner+".ui.link_preview_visible").toBool());
    QTRY_VERIFY_WITH_TIMEOUT(evaluate(owner+".ui.load_state==='complete' && !"+owner+".view.loading && "+owner+".view.url.toString().indexOf('http://127.0.0.1:')===0").toBool(),15000);
    QVERIFY(page("document.readyState==='complete'").toBool());
    QCOMPARE(evaluate(owner+".ui.current_url").toString(),clean);
}

void ApplicationTests::productPolishDevToolsOwner_data() {
    QTest::addColumn<QString>("kind");
    for (const auto &kind : {QString("primary"),QString("secondary"),QString("private"),QString("popup")}) QTest::newRow(qPrintable(kind)) << kind;
}
void ApplicationTests::productPolishDevToolsOwner() {
    QFETCH(QString,kind);
    QString owner="testWindow.browserWindowRegistry[0]";
    if (kind!="primary") {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"+quoted(text("current_url"))+",windowProfileName:'link-owner',windowProfileLabel:'Link owner',windowPrivateProfile:"+(kind=="private"?"true":"false")+"})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
        owner="testWindow.browserWindowRegistry[1]";
        QTRY_VERIFY(evaluate(owner+".host.profileBootstrapComplete").toBool());
    }
    if(kind=="popup") {
        pageReceiver=owner+".view";
        QTRY_VERIFY_WITH_TIMEOUT(evaluate(owner+".ui.load_state==='complete' && !"+owner+".view.loading && !"+owner+".ui.profile_bootstrap_pending").toBool(),15000);
        QVERIFY(page("document.readyState==='complete' && !!document.getElementById('blank')").toBool());
        auto host=qobject_cast<QQuickWindow *>(evaluate(owner+".host").value<QObject *>());
        auto view=qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());
        QVERIFY(host && view);host->requestActivate();QTRY_VERIFY(host->isActive());
        const auto point=page("(()=>{const r=document.getElementById('blank').getBoundingClientRect();return [r.x+r.width/2,r.y+r.height/2]})()").toList();
        QTest::mouseClick(host,Qt::LeftButton,Qt::NoModifier,view->mapToScene(QPointF(point[0].toDouble(),point[1].toDouble())).toPoint());
        QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(),1);
        owner="testWindow.popupWindowRegistry[0]";
    }
    auto host=qobject_cast<QQuickWindow *>(evaluate(owner+".host").value<QObject *>());
    QVERIFY(host);
    QTRY_VERIFY(evaluate(owner+".host.profileBootstrapComplete && !"+owner+".ui.profile_bootstrap_pending && "+owner+".view.ownsTab()").toBool());
    QTRY_VERIFY(evaluate(owner+".host.activeWebView() === "+owner+".view").toBool());
    const auto submit=[&](const QString &value) {
        host->requestActivate();QTRY_VERIFY(host->isActive());
        evaluate(owner+".ui.enter_command()");
        auto input=host->findChild<QQuickItem *>("commandLineInput");QVERIFY(input);
        QVERIFY(input->setProperty("text",value));input->forceActiveFocus();
        QTRY_VERIFY(input->hasActiveFocus());QTest::keyClick(host,Qt::Key_Return);
    };
    submit("devtools --detach");
    const auto tools=kind=="primary" ? "testWindow.devToolsDetachedWindow" : owner+".host.devToolsWindow";
    QTRY_VERIFY2(evaluate(tools+" !== null && "+tools+".visible").toBool(),qPrintable(evaluate(owner+".ui.status_text").toString()));
    QVERIFY2(evaluate(tools+".ownerWindow === "+owner+".host").toBool(),
        "Detached developer tools must retain the actual requesting host");
    auto inspector=qobject_cast<QQuickWindow *>(evaluate(tools).value<QObject *>());
    QVERIFY(inspector);QVERIFY(inspector->close());
    QTRY_VERIFY2(evaluate(tools+" === null").toBool(),"Closing the inspector must clear its owning host's handle");
    submit("devtools");
    QTRY_VERIFY(evaluate(owner+".host.devToolsWorkspace.attachedVisible").toBool());
    auto console=qobject_cast<QQuickItem *>(evaluate(owner+".host.devToolsWorkspace.attachedInspector").value<QObject *>());
    QVERIFY(console);console->forceActiveFocus();
    QTRY_VERIFY(evaluate(owner+".host.devToolsWorkspace.inputActive").toBool());
    QVERIFY(!evaluate(owner+".host.browserKeyFocusActive").toBool());
    auto pageView=qobject_cast<QQuickItem *>(evaluate(owner+".view").value<QObject *>());
    QVERIFY(pageView);pageView->forceActiveFocus();
    QTRY_VERIFY(!evaluate(owner+".host.devToolsWorkspace.inputActive").toBool());
    QTRY_VERIFY(evaluate(owner+".host.browserKeyFocusActive").toBool());
    submit("devtools");
    QTRY_VERIFY(!evaluate(owner+".host.devToolsWorkspace.visible").toBool());
    // An invalid replacement request leaves the current page and inspector intact.
    const auto original=evaluate(owner+".ui.core_window_id").toString();
    submit("devtools --detach");
    QTRY_VERIFY(evaluate(tools+" !== null").toBool());
    evaluate(owner+".ui.configure_profile(false,false,'','',testWindow.storageBasePath)");
    QTRY_VERIFY(!evaluate(owner+".ui.profile_bootstrap_pending").toBool());
    QCOMPARE(evaluate(owner+".ui.core_window_id").toString(),original);
    QVERIFY(evaluate(tools+" !== null && "+tools+".visible").toBool());
    if(kind=="primary") evaluate("testWindow.closeDevTools()");
    else {
        QPointer<QQuickWindow> inspectorLifetime=qobject_cast<QQuickWindow *>(evaluate(tools).value<QObject *>());
        QVERIFY(inspectorLifetime);
        evaluate(owner+".host.windowShutdownApproved=true");
        QVERIFY(host->close());
        QTRY_VERIFY(inspectorLifetime.isNull());
    }

}

void ApplicationTests::productPolishInteractiveNewWindow_data() {
    QTest::addColumn<QString>("kind");
    for (const auto &kind : {QString("primary"),QString("secondary"),QString("private"),QString("popup")}) QTest::newRow(qPrintable(kind)) << kind;
}
void ApplicationTests::productPolishInteractiveNewWindow() {
    QFETCH(QString,kind);
    QString owner="testWindow.browserWindowRegistry[0]";
    if (kind!="primary") {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"+quoted(text("current_url"))+",windowProfileName:'interactive-owner',windowProfileLabel:'Interactive owner',windowPrivateProfile:"+(kind=="private"?"true":"false")+"})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
        owner="testWindow.browserWindowRegistry[1]";
        QTRY_VERIFY(evaluate(owner+".host.profileBootstrapComplete").toBool());
    }
    if(kind=="popup") {
        pageReceiver=owner+".view";
        QTRY_VERIFY_WITH_TIMEOUT(evaluate(owner+".ui.load_state==='complete' && !"+owner+".view.loading && !"+owner+".ui.profile_bootstrap_pending").toBool(),15000);
        QVERIFY(page("document.readyState==='complete' && !!document.getElementById('blank')").toBool());
        auto host=qobject_cast<QQuickWindow *>(evaluate(owner+".host").value<QObject *>());
        auto view=qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());
        QVERIFY(host && view);host->requestActivate();QTRY_VERIFY(host->isActive());
        const auto point=page("(()=>{const r=document.getElementById('blank').getBoundingClientRect();return [r.x+r.width/2,r.y+r.height/2]})()").toList();
        QTest::mouseClick(host,Qt::LeftButton,Qt::NoModifier,view->mapToScene(QPointF(point[0].toDouble(),point[1].toDouble())).toPoint());
        QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(),1);
        owner="testWindow.popupWindowRegistry[0]";
    }
    QTRY_VERIFY(evaluate(owner+".host.profileBootstrapComplete && !"+owner+".ui.profile_bootstrap_pending && "+owner+".view.ownsTab()").toBool());
    const auto count=evaluate("testWindow.browserWindowRegistry.length").toInt();
    const auto profile=evaluate(owner+".profileName").toString();
    const auto privateProfile=evaluate(owner+".privateProfile").toBool();
    auto host=qobject_cast<QQuickWindow *>(evaluate(owner+".host").value<QObject *>());QVERIFY(host);
    host->requestActivate();QTRY_VERIFY(host->isActive());
    evaluate(owner+".ui.enter_command()");
    auto input=host->findChild<QQuickItem *>("commandLineInput");QVERIFY(input);
    QVERIFY(input->setProperty("text","window-new"));input->forceActiveFocus();
    QTRY_VERIFY(input->hasActiveFocus());QTest::keyClick(host,Qt::Key_Return);
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),count+1);
    const auto opened=QString("testWindow.browserWindowRegistry[%1]").arg(count);
    QTRY_VERIFY(evaluate(opened+".host.profileBootstrapComplete && !"+opened+".ui.profile_bootstrap_pending").toBool());
    QCOMPARE(evaluate(opened+".profileName").toString(),profile);
    QCOMPARE(evaluate(opened+".privateProfile").toBool(),privateProfile);
}

void ApplicationTests::productPolishSessionPermissionSharing_data() {
    QTest::addColumn<QString>("kind");
    for (const auto &kind : {QString("primary"),QString("private"),QString("popup")}) QTest::newRow(qPrintable(kind)) << kind;
}
void ApplicationTests::productPolishSessionPermissionSharing() {
    QFETCH(QString,kind);
    QString owner="testWindow.browserWindowRegistry[0]";
    if (kind!="primary") {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"+quoted(text("current_url"))+",windowProfileName:'interactive-owner',windowProfileLabel:'Interactive owner',windowPrivateProfile:"+(kind=="private"?"true":"false")+"})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
        owner="testWindow.browserWindowRegistry[1]";
        QTRY_VERIFY(evaluate(owner+".host.profileBootstrapComplete").toBool());
    }
    if(kind=="popup") {
        pageReceiver=owner+".view";
        QTRY_VERIFY_WITH_TIMEOUT(evaluate(owner+".ui.load_state==='complete' && !"+owner+".view.loading && !"+owner+".ui.profile_bootstrap_pending").toBool(),15000);
        QVERIFY(page("document.readyState==='complete' && !!document.getElementById('blank')").toBool());
        auto host=qobject_cast<QQuickWindow *>(evaluate(owner+".host").value<QObject *>());
        auto view=qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());
        QVERIFY(host && view);host->requestActivate();QTRY_VERIFY(host->isActive());
        const auto point=page("(()=>{const r=document.getElementById('blank').getBoundingClientRect();return [r.x+r.width/2,r.y+r.height/2]})()").toList();
        QTest::mouseClick(host,Qt::LeftButton,Qt::NoModifier,view->mapToScene(QPointF(point[0].toDouble(),point[1].toDouble())).toPoint());
        QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(),1);
        owner="testWindow.popupWindowRegistry[0]";
    }
    QTRY_VERIFY(evaluate(owner+".host.profileBootstrapComplete && !"+owner+".ui.profile_bootstrap_pending && "+owner+".view.ownsTab()").toBool());
    const auto origin=QString("https://permission.test");
    QVERIFY(evaluate(owner+".ui.remember_permission("+quoted(origin)+",'camera','allow','session')").toBool());
    QCOMPARE(evaluate(owner+".ui.permission_decision("+quoted(origin)+",'camera')").toString(),QString("allow"));
    const auto count=evaluate("testWindow.browserWindowRegistry.length").toInt();
    QVERIFY(evaluate(owner+".ui.execute_command('window-new')").toBool());
    evaluate(kind=="primary" ? "testWindow.executePendingEngineAction()" : owner+".host.applySwitcherEngineAction()");
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),count+1);
    const auto peer=QString("testWindow.browserWindowRegistry[%1]").arg(count);
    QTRY_VERIFY(evaluate(peer+".host.profileBootstrapComplete && !"+peer+".ui.profile_bootstrap_pending").toBool());
    QCOMPARE(evaluate(peer+".ui.permission_decision("+quoted(origin)+",'camera')").toString(),QString("allow"));
    QVERIFY(evaluate(owner+".ui.remember_permission("+quoted(origin)+",'camera','deny','session')").toBool());
    QCOMPARE(evaluate(peer+".ui.permission_decision("+quoted(origin)+",'camera')").toString(),QString("deny"));
}

void ApplicationTests::productPolishCombinedSessionPermissionReset() {
    evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"+quoted(text("current_url"))+",windowProfileName:'permission-reset',windowProfileLabel:'Permission reset',windowPrivateProfile:true})");
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
    const auto owner=QString("testWindow.browserWindowRegistry[1]");
    QTRY_VERIFY(evaluate(owner+".host.profileBootstrapComplete && !"+owner+".ui.profile_bootstrap_pending").toBool());
    const auto origin=QString("https://permission-reset.test");
    QVERIFY(evaluate(owner+".ui.remember_permission("+quoted(origin)+",'camera-and-microphone','allow','session')").toBool());
    for(const auto &capability:{QString("camera"),QString("microphone")})
        QCOMPARE(evaluate(owner+".ui.permission_decision("+quoted(origin)+","+quoted(capability)+")").toString(),QString("allow"));
    QVERIFY(evaluate(owner+".ui.reset_permission("+quoted(origin)+",'camera-and-microphone')").toBool());
    for(const auto &capability:{QString("camera"),QString("microphone")})
        QCOMPARE(evaluate(owner+".ui.permission_decision("+quoted(origin)+","+quoted(capability)+")").toString(),QString("ask"));
}

void ApplicationTests::productPolishContextCatalogBootstrap() {
    const auto profile=evaluate("testWindow.browserWindowRegistry[0].profileName").toString();
    QVERIFY(command("context-create polish-bootstrap --profile "+profile));
    QTRY_VERIFY(evaluate("testWindow.browserUi.context_choice_names.indexOf('polish-bootstrap')>=0").toBool());
    auto object=evaluate("testWindow.browserWindowFactory.createObject(null,{"
        "windowStartupUrl:'rb://blank',windowProfileName:"+quoted(profile)+","
        "windowProfileLabel:'Bootstrap catalog',windowStorageBasePath:testWindow.storageBasePath,"
        "windowSharedProfile:testWindow.browserProfile,windowInheritedUi:testWindow.browserUi})").value<QObject *>();
    QPointer<QQuickWindow> child=qobject_cast<QQuickWindow *>(object);
    QVERIFY(child);
    engine->rootContext()->setContextProperty("catalogBootstrapWindow",child.data());
    // Its typed profile has been inherited, but GUI admission still awaits
    // asynchronous filtering setup. No event loop has run since construction.
    QVERIFY(!child->property("profileBootstrapComplete").toBool());
    QVERIFY(command("context-delete polish-bootstrap --confirm"));
    QTRY_VERIFY(child->property("profileBootstrapComplete").toBool());
    const auto entry=QString("testWindow.browserWindowRegistry.find(function(e){return e.host===catalogBootstrapWindow})");
    QTRY_VERIFY(evaluate("!!("+entry+")").toBool());
    QTRY_VERIFY2(evaluate(entry+".ui.context_choice_names.indexOf('polish-bootstrap')<0").toBool(),
        "Admission must catch a committed deletion delivered during bootstrap");
    evaluate("catalogBootstrapWindow.windowShutdownApproved=true");
    QVERIFY(child->close());child->deleteLater();
    QCoreApplication::sendPostedEvents(nullptr,QEvent::DeferredDelete);
    QTRY_VERIFY(child.isNull());
}

void ApplicationTests::productPolishNewWindows_data() {
    QTest::addColumn<QString>("sourceKind");
    QTest::addColumn<bool>("privateProfile");
    QTest::addColumn<bool>("temporaryProfile");
    QTest::addColumn<bool>("explicitProfile");
    QTest::newRow("primary") << QString("primary") << false << false << false;
    for (const auto &kind : {QString("secondary"),QString("popup")}) {
        QTest::newRow(qPrintable(kind+"-regular")) << kind << false << false << false;
        QTest::newRow(qPrintable(kind+"-private")) << kind << true << false << false;
        QTest::newRow(qPrintable(kind+"-temporary")) << kind << false << true << false;
        QTest::newRow(qPrintable(kind+"-private-explicit-durable")) << kind << true << false << true;
        QTest::newRow(qPrintable(kind+"-temporary-explicit-durable")) << kind << false << true << true;
    }
}

void ApplicationTests::productPolishNewWindows() {
    QFETCH(QString,sourceKind);
    QFETCH(bool,privateProfile);
    QFETCH(bool,temporaryProfile);
    QFETCH(bool,explicitProfile);
    QTest::failOnWarning(QRegularExpression(".*(invalid context|TypeError|ReferenceError).*"));
    const auto initial=text("current_url");
    const auto sourceBase=sourceKind!="primary" && !privateProfile && !temporaryProfile
        ? directory.path()+"/window-scope-"+QString::fromLatin1(QTest::currentDataTag())
        : evaluate("testWindow.storageBasePath").toString();
    QString owner="testWindow.browserWindowRegistry[0]";
    if (sourceKind!="primary") {
        // Explicit-durable rows deliberately collide with the regular profile's
        // name; an explicit selector must still leave private browsing.
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"
            +quoted(initial)+",windowProfileName:"+quoted(explicitProfile ? "default" : "new-window-owner")
            +",windowProfileLabel:'New window owner',windowPrivateProfile:"
            +(privateProfile ? "true" : "false")+",windowEphemeralProfile:"
            +(temporaryProfile ? "true" : "false")+",windowEphemeralInvocationToken:'new-window-session',windowStorageBasePath:"
            +quoted(sourceBase)+"})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
        owner="testWindow.browserWindowRegistry[1]";
        QTRY_VERIFY(evaluate(owner+".host.profileBootstrapComplete").toBool());
        if (!privateProfile && !temporaryProfile) {
            QTRY_VERIFY(QDirIterator(sourceBase,QStringList{"browser.sqlite"},QDir::Files,QDirIterator::Subdirectories).hasNext());
            QCOMPARE(evaluate(owner+".profile.persistentStoragePath").toString(),sourceBase+"/webengine/new-window-owner");
        }
    }
    if (!privateProfile && !temporaryProfile) {
        const auto profile=evaluate(owner+".profileName").toString();
        const auto definitions=evaluate(owner+".ui.contexts_json").toString();
        QVERIFY(evaluate(owner+".ui.execute_command("+quoted("context-create polish-discard --profile "+profile)+")").toBool());
        QTRY_VERIFY(evaluate(owner+".ui.context_choice_names.indexOf('polish-discard')>=0").toBool());
        QVERIFY(evaluate(owner+".ui.execute_command('context-delete polish-discard --confirm')").toBool());
        QTRY_VERIFY(evaluate(owner+".ui.context_choice_names.indexOf('polish-discard')<0").toBool());
        QCOMPARE(evaluate(owner+".ui.contexts_json").toString(),definitions);
        QVERIFY(evaluate(owner+".ui.context_choice_names.indexOf('polish-work')<0").toBool());
        QVERIFY(evaluate(owner+".ui.execute_command("+quoted("context-create polish-work --profile "+profile)+")").toBool());
        // Page loading can replace a status notice before the next test tick.
        // Observe the committed catalog and profile, not transient prose.
        QTRY_VERIFY2(evaluate(owner+".ui.context_choice_names.indexOf('polish-work')>=0").toBool(),
            qPrintable(evaluate(owner+".ui.status_text").toString()));
        QCOMPARE(evaluate(owner+".ui.context_choice_profiles["+owner+".ui.context_choice_names.indexOf('polish-work')]").toString(),profile);
        QCOMPARE(evaluate(owner+".ui.context_choice_labels["+owner+".ui.context_choice_names.indexOf('polish-work')]").toString(),QString("polish-work"));
        QCOMPARE(evaluate(owner+".ui.contexts_json").toString(),definitions);
        evaluate(owner+".ui.set_context_entry_reuse(true)");
        QVERIFY(evaluate(owner+".ui.execute_command('context-enter polish-work')").toBool());
        evaluate(owner+".ui.set_context_entry_reuse(false)");
        QTRY_COMPARE(evaluate(owner+".ui.context_name").toString(),QString("polish-work"));
    }
    pageReceiver=owner+".view";
    QTRY_VERIFY(page("document.readyState==='complete' && document.getElementById('blank')!==null").toBool());
    if (sourceKind=="popup") {
        auto host=qobject_cast<QQuickWindow *>(evaluate(owner+".host").value<QObject *>());
        auto view=qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());
        QVERIFY(host && view);
        host->requestActivate();QTRY_VERIFY(host->isActive());
        const auto point=page("(()=>{const r=document.getElementById('blank').getBoundingClientRect();return [r.x+r.width/2,r.y+r.height/2]})()").toList();
        QCOMPARE(point.size(),2);
        QTest::mouseClick(host,Qt::LeftButton,Qt::NoModifier,
            view->mapToScene(QPointF(point[0].toDouble(),point[1].toDouble())).toPoint());
        QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(),1);
        owner="testWindow.popupWindowRegistry[0]";
        pageReceiver=owner+".view";
        QTRY_VERIFY(page("document.readyState==='complete' && document.getElementById('input')!==null").toBool());
    }
    const auto sourceUrl=page("location.href").toString();
    const auto sourceProfile=evaluate(owner+".profile").value<QObject *>();
    const auto sourceToken=evaluate(owner+".windowToken").toString();
    owner="testWindow.liveWindowEntries().find(function(e){return e.windowToken==="+quoted(sourceToken)+"})";
    pageReceiver=owner+".view";
    QVERIFY(evaluate(owner+".ui.execute_command('set --temp ui.font_size_pt=17')").toBool());
    const auto before=evaluate("testWindow.liveWindowEntries().length").toInt();
    const auto tokens=evaluate("JSON.stringify(testWindow.liveWindowEntries().map(function(e){return e.windowToken}))").toString();
    const auto desired=initial+"?new-window="+sourceKind;
    const auto request=QString("open --target window ")+(explicitProfile ? "--profile default " : "")+desired;
    QVERIFY(evaluate(owner+".ui.execute_command("+quoted(request)+")").toBool());
    evaluate("testWindow.completeBrowserKeyAction("+owner+".ui,"+owner+".host)");
    QTRY_COMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(),before+1);
    const auto destination="testWindow.liveWindowEntries().find(function(e){return !"+tokens+".includes(e.windowToken)})";
    QVERIFY2(evaluate("!!("+destination+")").toBool(),qPrintable(evaluate(
        "JSON.stringify(testWindow.liveWindowEntries().map(function(e){return [e.windowToken,e.profileName,e.ui.status_text,e.host.profileBootstrapComplete]}))").toString()));
    const auto destinationAdmission=evaluate("JSON.stringify(testWindow.liveWindowEntries().map(function(e){return [e.windowToken,e.profileName,e.ui.status_text,e.host.profileBootstrapComplete]}))").toString();
    QTRY_VERIFY2(evaluate("(function(){var e="+destination+";return !!e && !!e.host && e.host.profileBootstrapComplete})()").toBool(),
        qPrintable(destinationAdmission+" -> "+evaluate("JSON.stringify(testWindow.liveWindowEntries().map(function(e){return [e.windowToken,e.profileName,e.ui.status_text,e.host.profileBootstrapComplete]}))").toString()));
    pageReceiver=destination+".view";
    QTRY_COMPARE(page("location.href").toString(),desired);
    QCOMPARE(evaluate(destination+".privateProfile").toBool(),!explicitProfile && privateProfile);
    QCOMPARE(evaluate(destination+".ephemeralProfile").toBool(),!explicitProfile && temporaryProfile);
    QCOMPARE(evaluate(destination+".ui.context_name").toString(),
        !privateProfile && !temporaryProfile ? QString("polish-work") : QString());
    if (!privateProfile && !temporaryProfile) {
        QVERIFY(evaluate(destination+".ui.context_choice_names.indexOf('polish-work')>=0").toBool());
        QVERIFY(evaluate(destination+".ui.context_choice_names.indexOf('polish-discard')<0").toBool());
        const auto sourceProfileName=evaluate(owner+".profileName").toString();
        const auto primaryChoices=evaluate("JSON.stringify(testWindow.browserUi.context_choice_names)").toString();
        // Both windows already exist: concurrent creations must converge, and
        // deleting in either owner must remove the other owner's stale choice.
        QVERIFY(evaluate(owner+".ui.execute_command("+quoted("context-create polish-shared-a --profile "+sourceProfileName)+")").toBool());
        QVERIFY(evaluate(destination+".ui.execute_command("+quoted("context-create polish-shared-b --profile "+sourceProfileName)+")").toBool());
        for (const auto &entry : {owner,destination}) {
            QTRY_VERIFY(evaluate(entry+".ui.context_choice_names.indexOf('polish-shared-a')>=0 && "+entry+".ui.context_choice_names.indexOf('polish-shared-b')>=0").toBool());
        }
        QVERIFY(evaluate(destination+".ui.execute_command('context-delete polish-shared-a --confirm')").toBool());
        QVERIFY(evaluate(owner+".ui.execute_command('context-delete polish-shared-b --confirm')").toBool());
        for (const auto &entry : {owner,destination}) {
            QTRY_VERIFY(evaluate(entry+".ui.context_choice_names.indexOf('polish-shared-a')<0 && "+entry+".ui.context_choice_names.indexOf('polish-shared-b')<0").toBool());
        }
        if (sourceKind!="primary")
            QCOMPARE(evaluate("JSON.stringify(testWindow.browserUi.context_choice_names)").toString(),primaryChoices);
    } else if (!explicitProfile) {
        QCOMPARE(evaluate(destination+".ui.context_choice_names.length").toInt(),0);
    }
    if (explicitProfile) {
        QCOMPARE(evaluate(destination+".profile").value<QObject *>(),evaluate("testWindow.browserProfile").value<QObject *>());
        QVERIFY(evaluate(destination+".profile").value<QObject *>()!=sourceProfile);
    } else {
        QCOMPARE(evaluate(destination+".profile").value<QObject *>(),sourceProfile);
        QCOMPARE(evaluate(destination+".host.windowStorageBasePath").toString(),sourceBase);
        QCOMPARE(evaluate("JSON.parse("+destination+".ui.config_json).ui.font_size_pt").toInt(),17);
        if (temporaryProfile)
            QCOMPARE(evaluate(destination+".ephemeralInvocationToken").toString(),QString("new-window-session"));
    }
    pageReceiver=owner+".view";
    QCOMPARE(page("location.href").toString(),sourceUrl);
    const auto destinationToken=evaluate(destination+".windowToken").toString();
    const auto nextOwner="testWindow.liveWindowEntries().find(function(e){return e.windowToken==="+quoted(destinationToken)+"})";
    const auto beforeBlank=evaluate("JSON.stringify(testWindow.liveWindowEntries().map(function(e){return e.windowToken}))").toString();
    QVERIFY(evaluate(nextOwner+".ui.execute_command('window-new')").toBool());
    evaluate("testWindow.completeBrowserKeyAction("+nextOwner+".ui,"+nextOwner+".host)");
    QTRY_COMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(),before+2);
    const auto blank="testWindow.liveWindowEntries().find(function(e){return !"+beforeBlank+".includes(e.windowToken)})";
    QVERIFY2(evaluate("!!("+blank+")").toBool(),qPrintable(evaluate(
        "JSON.stringify(testWindow.liveWindowEntries().map(function(e){return [e.windowToken,e.profileName,e.ui.status_text,e.host.profileBootstrapComplete]}))").toString()));
    const auto blankAdmission=evaluate("JSON.stringify(testWindow.liveWindowEntries().map(function(e){return [e.windowToken,e.profileName,e.ui.status_text,e.host.profileBootstrapComplete]}))").toString();
    QTRY_VERIFY2(evaluate("(function(){var e="+blank+";return !!e && !!e.host && e.host.profileBootstrapComplete})()").toBool(),
        qPrintable(blankAdmission+" -> "+evaluate("JSON.stringify(testWindow.liveWindowEntries().map(function(e){return [e.windowToken,e.profileName,e.ui.status_text,e.host.profileBootstrapComplete]}))").toString()));
    QCOMPARE(evaluate(blank+".profile").value<QObject *>(),evaluate(nextOwner+".profile").value<QObject *>());
    QCOMPARE(evaluate(blank+".ui.context_name").toString(),evaluate(nextOwner+".ui.context_name").toString());
    QCOMPARE(evaluate(blank+".privateProfile").toBool(),evaluate(nextOwner+".privateProfile").toBool());
    QCOMPARE(evaluate(blank+".ephemeralProfile").toBool(),evaluate(nextOwner+".ephemeralProfile").toBool());
    const auto beforePrivate=evaluate("JSON.stringify(testWindow.liveWindowEntries().map(function(e){return e.windowToken}))").toString();
    QVERIFY(evaluate(nextOwner+".ui.execute_command('window-new --private')").toBool());
    evaluate("testWindow.completeBrowserKeyAction("+nextOwner+".ui,"+nextOwner+".host)");
    QTRY_COMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(),before+3);
    const auto fresh="testWindow.liveWindowEntries().find(function(e){return !"+beforePrivate+".includes(e.windowToken)})";
    QVERIFY2(evaluate("!!("+fresh+")").toBool(),qPrintable(evaluate(
        "JSON.stringify(testWindow.liveWindowEntries().map(function(e){return [e.windowToken,e.profileName,e.ui.status_text,e.host.profileBootstrapComplete]}))").toString()));
    const auto freshAdmission=evaluate("JSON.stringify(testWindow.liveWindowEntries().map(function(e){return [e.windowToken,e.profileName,e.ui.status_text,e.host.profileBootstrapComplete]}))").toString();
    QTRY_VERIFY2(evaluate("(function(){var e="+fresh+";return !!e && !!e.host && e.host.profileBootstrapComplete})()").toBool(),
        qPrintable(freshAdmission+" -> "+evaluate("JSON.stringify(testWindow.liveWindowEntries().map(function(e){return [e.windowToken,e.profileName,e.ui.status_text,e.host.profileBootstrapComplete]}))").toString()));
    QVERIFY(evaluate(fresh+".privateProfile && !"+fresh+".ephemeralProfile && "+fresh+".profile.offTheRecord").toBool());
    QVERIFY(evaluate(fresh+".profile !== "+nextOwner+".profile").toBool());
    QCOMPARE(evaluate(fresh+".ui.context_name").toString(),QString());
    if (sourceKind!="primary") {
        pageReceiver=nextOwner+".view";
        page("window.newWindowRetainedState=47");
        QPointer<QQuickWindow> sourceWindow=qobject_cast<QQuickWindow *>(evaluate(owner+".host").value<QObject *>());
        QVERIFY(sourceWindow);
        evaluate(owner+".host.windowShutdownApproved=true");
        QVERIFY(sourceWindow->close());sourceWindow->deleteLater();
        QCoreApplication::sendPostedEvents(nullptr,QEvent::DeferredDelete);
        QTRY_VERIFY(sourceWindow.isNull());
        QCOMPARE(page("window.newWindowRetainedState").toInt(),47);
        QCOMPARE(page("location.href").toString(),desired);
    }
    if (sourceKind=="primary" && !privateProfile && !temporaryProfile) {
        const auto profile=evaluate(nextOwner+".profileName").toString();
        QVERIFY(evaluate(nextOwner+".ui.execute_command("+quoted("context-create polish-lifetime --profile "+profile)+")").toBool());
        QTRY_VERIFY(evaluate(blank+".ui.context_choice_names.indexOf('polish-lifetime')>=0").toBool());
        QPointer<QQuickWindow> retiring=qobject_cast<QQuickWindow *>(evaluate(blank+".host").value<QObject *>());
        QVERIFY(retiring);
        // Delete the receiving QObject before its queued catalog delivery runs.
        QVERIFY(evaluate(nextOwner+".ui.execute_command('context-delete polish-lifetime --confirm')").toBool());
        evaluate(blank+".host.windowShutdownApproved=true");
        QVERIFY(retiring->close());retiring->deleteLater();
        QCoreApplication::sendPostedEvents(nullptr,QEvent::DeferredDelete);
        QTRY_VERIFY(retiring.isNull());
        QVERIFY(evaluate(nextOwner+".ui.context_choice_names.indexOf('polish-lifetime')<0").toBool());
    }
    pageReceiver="testWindow.activeWebView()";
}

void ApplicationTests::productPolishSecondaryHints_data() {
    QTest::addColumn<QString>("kind");
    QTest::addColumn<bool>("pointer");
    for (const auto &kind : {"secondary", "private", "popup"}) {
        QTest::newRow(kind) << QString::fromLatin1(kind) << false;
        QTest::newRow((QByteArray(kind)+"-pointer").constData()) << QString::fromLatin1(kind) << true;
    }
}

void ApplicationTests::productPolishSecondaryHints() {
    QTest::failOnWarning(QRegularExpression(".*(invalid context|TypeError|ReferenceError|Binding loop).*"));
    QFETCH(QString, kind);QFETCH(bool, pointer);
    const auto initial=text("current_url");
    QString entry;
    if (kind=="popup") {
        QVERIFY(command("bind p hint --target pointer links"));key(Qt::Key_P);
        typeText(hintLabel("Open popup"));key(Qt::Key_Return);
        QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(),1);
        entry="testWindow.popupWindowRegistry[0]";
    } else {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"+quoted(initial)
            +",windowProfileName:'hint-owner',windowProfileLabel:'Hint owner',windowPrivateProfile:"
            +(kind=="private" ? "true" : "false")+"})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
        entry="testWindow.browserWindowRegistry[1]";
    }
    QTRY_VERIFY(evaluate(entry+".host.profileBootstrapComplete && !"+entry+".ui.profile_bootstrap_pending").toBool());
    auto child=qobject_cast<QQuickWindow *>(evaluate(entry+".host").value<QObject *>());QVERIFY(child);
    pageReceiver=entry+".host.activeView";
    const auto documentUrl=kind=="popup" ? QUrl(initial).resolved(QUrl("/popup.html")).toString() : initial;
    QTRY_VERIFY(page("location.href === "+quoted(documentUrl)+" && document.readyState==='complete'").toBool());
    QTRY_VERIFY(!evaluate("testWindow.browserUi.hint_pointer_pending").toBool());
    // Keep a different hint session in the inactive primary window. Child
    // gestures must leave its labels, selection and navigation untouched.
    QVERIFY(command("hint links"));
    QVERIFY(!hintLabel("Same document").isEmpty());
    const auto primaryHints=text("hint_values");
    QVERIFY(evaluate(entry+".ui.set_runtime_setting('ui.tabs','always',true)").toBool());
    const auto position=kind=="secondary" ? "bottom" : kind=="private" ? "left" : "right";
    QVERIFY(evaluate(entry+".ui.set_runtime_setting('ui.tab_position',"+quoted(position)+",true)").toBool());
    QVERIFY(evaluate(entry+".ui.set_runtime_setting('ui.font_size_pt','16',true)").toBool());
    evaluate(pageReceiver+".zoomFactor="+QString::number(pointer ? 1.25 : 0.8));
    child->requestActivate();QTRY_VERIFY(child->isActive());evaluate(pageReceiver+".forceActiveFocus()");
    const auto request=pointer ? "hint --target pointer links" : "hint links";
    QVERIFY2(evaluate(entry+".ui.execute_secondary_interactive_command("+quoted(QString("bind h ")+request)+")").toBool(),
        qPrintable(evaluate(entry+".ui.status_text").toString()));
    QTRY_VERIFY(evaluate(entry+".ui.status_text").toString().startsWith("Binding saved"));
    QTest::keyClick(child,Qt::Key_H);
    evaluate(entry+".host.processRuntimeWork()");
    QTRY_VERIFY2(evaluate(entry+".ui.hint_visible").toBool(),
        qPrintable(evaluate(entry+".ui.status_text").toString()));
    const auto label=hintLabel("Same document",entry+".ui");QVERIFY(!label.isEmpty());
    auto overlay=findVisualItem(child->contentItem(),"hintOverlay");QVERIFY(overlay);QVERIFY(overlay->isVisible());
    QCOMPARE(overlay->window(),child);
    auto view=qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());QVERIFY(view);
    QVERIFY(qAbs(overlay->width()-view->width())<=1);QVERIFY(qAbs(overlay->height()-view->height())<=1);
    const auto overlayOrigin=overlay->mapToScene(QPointF());
    const auto viewOrigin=view->mapToScene(QPointF());
    QVERIFY(qAbs(overlayOrigin.x()-viewOrigin.x())<=1);QVERIFY(qAbs(overlayOrigin.y()-viewOrigin.y())<=1);
    const auto artifacts=qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
    if (!artifacts.isEmpty()) {
        QDir().mkpath(artifacts);QVERIFY(child->grabWindow().save(artifacts+"/production-hints-"
            +QString::fromLatin1(QTest::currentDataTag())+".png"));
    }
    for (const auto character : label) {
        QTest::keyClick(child,Qt::Key(character.toUpper().unicode()),character.isUpper() ? Qt::ShiftModifier : Qt::NoModifier);
        QTest::qWait(30);
    }
    QTest::keyClick(child,Qt::Key_Return);
    QTRY_VERIFY(page("location.hash==='#fragment'").toBool());
    if (pointer) {QCOMPARE(page("window.lastClicked").toString(),QString("same"));QVERIFY(page("window.lastTrusted").toBool());}
    QTRY_VERIFY(!evaluate(entry+".ui.hint_visible").toBool());
    QCOMPARE(text("hint_values"),primaryHints);QCOMPARE(text("current_url"),initial);
    QVERIFY(evaluate("testWindow.browserUi.hint_visible").toBool());
    if (!pointer) {
        // The searchable control list takes focus in this window and requires
        // confirmation. It must not activate a result just from query text.
        QVERIFY(evaluate(entry+".ui.execute_secondary_interactive_command('bind t target-search controls')").toBool());
        QTRY_VERIFY(evaluate(entry+".ui.status_text").toString().startsWith("Binding saved"));
        QTest::keyClick(child,Qt::Key_T);
        QTRY_VERIFY(evaluate(entry+".ui.target_search_active").toBool());
        auto searchInput=findVisualItem(child->contentItem(),"targetSearchInput");QVERIFY(searchInput);
        QTRY_VERIFY(searchInput->hasActiveFocus());
        for (const auto character : QString("activate exact button")) {
            QTest::keyClick(child,Qt::Key(character.toUpper().unicode()),character.isUpper() ? Qt::ShiftModifier : Qt::NoModifier);
            QTest::qWait(30);
        }
        QTRY_COMPARE(evaluate(entry+".host.hintWorkspace.hintState.query").toString(),QString("activate exact button"));
        QTRY_COMPARE(evaluate(entry+".host.hintWorkspace.targetSearchRows().length").toInt(),1);
        QTRY_VERIFY(!evaluate(entry+".host.hintWorkspace.targetSearchSelectedIdentity").toString().isEmpty());
        QCOMPARE(page("window.clicks").toInt(),0);
        if (!artifacts.isEmpty()) QVERIFY(child->grabWindow().save(artifacts+"/production-target-search-"+kind+".png"));
        QTest::keyClick(child,Qt::Key_Return);
        QTRY_COMPARE(page("window.clicks").toInt(),1);
        QTRY_VERIFY(!evaluate(entry+".ui.target_search_active").toBool());
        QTRY_VERIFY(evaluate("(function(){var item="+entry+".host.activeFocusItem;while(item && item!=="+pageReceiver+")item=item.parent;return !!item})()").toBool());
        // Region actions use the shared palette but retain the requesting
        // workspace through its Grid handoff and native coordinate checks.
        QVERIFY(evaluate(entry+".ui.execute_secondary_interactive_command('bind r hint --target choose regions')").toBool());
        QTRY_VERIFY(evaluate(entry+".ui.status_text").toString().startsWith("Binding saved"));
        QTest::keyClick(child,Qt::Key_R);
        const auto region=hintLabel("Scoped panel",entry+".ui");QVERIFY(!region.isEmpty());
        for (const auto character : region) { QTest::keyClick(child,Qt::Key(character.toUpper().unicode()));QTest::qWait(30); }
        QTRY_VERIFY2_WITH_TIMEOUT(evaluate("testWindow.pendingContextMenuUi === "+entry+".ui && testWindow.contextMenuItems.length>0").toBool(),
            qPrintable(evaluate(entry+".ui.status_text").toString()),5000);
        QVERIFY(chooseCapability("grid-within", child));
        QTRY_VERIFY(evaluate(entry+".ui.spatial_visible").toBool());
        QVERIFY(evaluate(entry+".host.hintWorkspace.spatialAdapterTargetItem === "+pageReceiver).toBool());
        if (!artifacts.isEmpty()) QVERIFY(child->grabWindow().save(artifacts+"/production-scoped-grid-"+kind+".png"));
        QTest::keyClick(child,Qt::Key_1);
        QTRY_COMPARE(evaluate(entry+".ui.spatial_depth").toInt(),1);
        // Observe the signal synchronously: native dispatch may finish before
        // QTest returns from the key event, even with a zero key delay.
        QQmlComponent dispatchProbeComponent(engine.get());
        dispatchProbeComponent.setData(R"QML(
import QtQuick
QtObject {
    id: probe
    required property var ui
    required property var workspace
    required property var view
    property bool captured: false
    property string actionBefore: ""
    property string actionAfter: ""
    property bool wasSelecting: true
    property bool observationApplied: true
    property Connections observer: Connections {
        target: probe.ui
        function onSpatial_dispatch_requestChanged() {
            const payload = probe.ui.spatial_dispatch_request
            if (!payload || probe.captured) return
            probe.captured = true
            probe.actionBefore = payload
            probe.wasSelecting = probe.ui.spatial_selecting
            // A retired selection observation must not cancel the action
            // whose own geometry validation is now pending.
            probe.observationApplied = probe.ui.spatial_scope_geometry_validated(
                "", "{}", probe.workspace.spatialAdapterSurfaceSerial,
                probe.workspace.spatialAdapterSurfaceRevision,
                probe.view.width, probe.view.height, probe.view.zoomFactor)
            probe.actionAfter = probe.ui.spatial_dispatch_request
        }
    }
}
)QML",QUrl());
        QVERIFY2(dispatchProbeComponent.isReady(),qPrintable(dispatchProbeComponent.errorString()));
        std::unique_ptr<QObject> dispatchProbe(dispatchProbeComponent.createWithInitialProperties({
            {"ui",evaluate(entry+".ui")},
            {"workspace",evaluate(entry+".host.hintWorkspace")},
            {"view",evaluate(pageReceiver)}
        }));
        QVERIFY2(dispatchProbe,qPrintable(dispatchProbeComponent.errorString()));
        QTest::keyClick(child,Qt::Key_Return);
        QTRY_VERIFY2(dispatchProbe->property("captured").toBool(),
            qPrintable(evaluate(entry+".ui.status_text").toString()));
        QVERIFY(!dispatchProbe->property("actionBefore").toString().isEmpty());
        QVERIFY(!dispatchProbe->property("wasSelecting").toBool());
        QVERIFY(!dispatchProbe->property("observationApplied").toBool());
        QCOMPARE(dispatchProbe->property("actionAfter").toString(),dispatchProbe->property("actionBefore").toString());
        QTRY_VERIFY2_WITH_TIMEOUT(page("window.scopedClicks || 0").toInt()==1,
            qPrintable(evaluate("JSON.stringify({status:"+entry+".ui.status_text,mode:"+entry+".ui.mode,grid:"+entry+".ui.spatial_visible,depth:"+entry+".ui.spatial_depth,focus:String("+entry+".host.activeFocusItem)})").toString()
                + " DOM " + page("JSON.stringify({clicks:window.scopedClicks||0,last:window.lastClicked,point:window.clientPoint,all:window.allClicks,zoom:devicePixelRatio,scrollY:scrollY,button:document.getElementById('scoped').getBoundingClientRect().toJSON()})").toString()),15000);
        QVERIFY(page("window.lastTrusted").toBool());
        QCOMPARE(text("hint_values"),primaryHints);QCOMPARE(text("current_url"),initial);
        QVERIFY(evaluate(entry+".ui.execute_secondary_interactive_command('hint links')").toBool());
        QVERIFY(!hintLabel("Same document",entry+".ui").isEmpty());
        QTest::keyClick(child,Qt::Key_Escape);
        QTRY_VERIFY(!evaluate(entry+".ui.hint_visible").toBool());
        QVERIFY(evaluate("testWindow.browserUi.hint_visible").toBool());
    }
    if (!pointer) {
        // Closing with a live collector must retire only this owner's leases.
        QVERIFY(evaluate(entry+".ui.execute_secondary_interactive_command('hint links')").toBool());
        QVERIFY(!hintLabel("Same document",entry+".ui").isEmpty());
    }
    evaluate(entry+".host.windowShutdownApproved=true");QVERIFY(child->close());
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),1);
    QVERIFY(evaluate("testWindow.browserUi.hint_visible").toBool());
    QCOMPARE(text("hint_values"),primaryHints);
}

void ApplicationTests::productPolishPopupCommands() {
    QTest::failOnWarning(QRegularExpression(".*(invalid context|TypeError|ReferenceError).*"));
    const auto openerUrl=text("current_url");
    QVERIFY(command("bind p hint --target pointer links"));key(Qt::Key_P);
    typeText(hintLabel("Open popup"));key(Qt::Key_Return);
    QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(),1);
    const auto token=evaluate("testWindow.popupWindowRegistry[0].windowToken").toString();
    QVERIFY(!token.isEmpty());
    const auto entry="testWindow.popupWindowRegistry.find(function(e){return e.windowToken==="+quoted(token)+"})";
    QPointer<QQuickWindow> child=qobject_cast<QQuickWindow *>(evaluate(entry+".host").value<QObject *>());
    QVERIFY(child);
    pageReceiver=entry+".view";
    QTRY_VERIFY(page("document.readyState==='complete' && document.getElementById('input')!==null").toBool());
    QTRY_VERIFY(!evaluate(entry+".ui.profile_bootstrap_pending").toBool());
    const auto desired=page("new URL('/fixture.html?popup-command=1',location.href).href").toString();
    QVERIFY(evaluate(entry+".ui.execute_secondary_interactive_command("+quoted("open "+desired)+")").toBool());
    QTRY_COMPARE(page("location.href").toString(),desired);
    QTRY_VERIFY(QJsonDocument::fromJson(evaluate(entry+".ui.refresh_interaction_status()")
        .toString().toUtf8()).object().value("target").toObject().value("document_known").toBool());
    child->requestActivate();QTRY_VERIFY(child->isActive());
    evaluate(pageReceiver+".forceActiveFocus()");
    QTest::keyClick(child,Qt::Key_Colon);
    QTRY_COMPARE(evaluate(entry+".ui.mode").toString(),QString("command"));
    QTest::keyClick(child,Qt::Key_Escape);
    QTRY_COMPARE(evaluate(entry+".ui.mode").toString(),QString("normal"));
    QCOMPARE(text("mode"),QString("normal"));
    QVERIFY(evaluate(entry+".ui.execute_secondary_interactive_command('settings')").toBool());
    auto settings=child->findChild<QObject *>("secondarySettingsWorkspace");
    QVERIFY(settings);
    QTRY_VERIFY(settings->property("active").toBool());
    QTest::keyClick(child,Qt::Key_Escape);
    QTRY_VERIFY(!settings->property("active").toBool());
    QVERIFY(evaluate(entry+".ui.execute_secondary_interactive_command('profiles')").toBool());
    QTRY_VERIFY(evaluate(entry+".host.profileSessionWorkspace.profileManagerVisible").toBool());
    QVERIFY(!evaluate("testWindow.profileManagerVisible").toBool());
    evaluate(entry+".host.profileSessionWorkspace.closeProfiles()");
    page("window.popupFirstTabState=47");
    const auto first=evaluate(pageReceiver).value<QObject *>();
    const auto secondUrl=desired+"&tab=2";
    QVERIFY(evaluate(entry+".ui.execute_secondary_interactive_command("+quoted("tab-open "+secondUrl)+")").toBool());
    QTRY_COMPARE(evaluate(entry+".ui.tab_count").toInt(),2);
    QTRY_COMPARE(evaluate(entry+".host.tabViews.length").toInt(),2);
    QTRY_COMPARE(page("location.href").toString(),secondUrl);
    QVERIFY(evaluate(pageReceiver).value<QObject *>()!=first);
    evaluate(entry+".ui.select_tab(0)");
    QTRY_COMPARE(evaluate(pageReceiver).value<QObject *>(),first);
    QCOMPARE(page("window.popupFirstTabState").toInt(),47);
    QCOMPARE(evaluate("testWindow.shutdownParticipantCount()").toInt(),1);
    QCOMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(),2);
    QCOMPARE(text("current_url"),openerUrl);
    auto view=qobject_cast<QQuickItem *>(first);
    const auto point=page("(()=>{const r=document.getElementById('blank').getBoundingClientRect();return [r.x+r.width/2,r.y+r.height/2]})()").toList();
    QCOMPARE(point.size(),2);
    QTest::mouseClick(child,Qt::LeftButton,Qt::NoModifier,
        view->mapToScene(QPointF(point[0].toDouble(),point[1].toDouble())).toPoint());
    QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(),2);
    const auto nested="testWindow.popupWindowRegistry.find(function(e){return e.windowToken!=="+quoted(token)+"})";
    QPointer<QQuickWindow> nestedWindow=qobject_cast<QQuickWindow *>(evaluate(nested+".host").value<QObject *>());
    QVERIFY(nestedWindow);
    QVERIFY(evaluate(nested+".view.profile === "+entry+".view.profile && "+nested+".ui !== "+entry+".ui").toBool());
    pageReceiver=nested+".view";
    QTRY_VERIFY(page("document.readyState==='complete' && document.getElementById('input')!==null").toBool());
    page("window.nestedPopupState=95");
    evaluate(entry+".host.windowShutdownApproved=true");
    QVERIFY(child->close());child->deleteLater();
    QCoreApplication::sendPostedEvents(nullptr,QEvent::DeferredDelete);
    QTRY_VERIFY(child.isNull());
    QCOMPARE(page("window.nestedPopupState").toInt(),95);
    evaluate(nested+".host.windowShutdownApproved=true");
    QVERIFY(nestedWindow->close());nestedWindow->deleteLater();
    QCoreApplication::sendPostedEvents(nullptr,QEvent::DeferredDelete);
    QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(),0);
    QTRY_COMPARE(evaluate("testWindow.liveWindowEntries().length").toInt(),1);
    pageReceiver="testWindow.activeWebView()";
}

void ApplicationTests::productPolishProvisionalTransferFallback_data() {
    QTest::addColumn<bool>("secondary");
    QTest::newRow("primary") << false;
    QTest::newRow("secondary") << true;
}
void ApplicationTests::productPolishProvisionalTransferFallback() {
    QFETCH(bool,secondary);
    const auto initial=text("current_url");
    QString owner="({ui:testWindow.browserUi,host:testWindow})";
    if (secondary) {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"
            + quoted(initial) + ",windowProfileName:'fallback-owner',windowProfileLabel:'Fallback owner'})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
        owner="testWindow.browserWindowRegistry[1]";
        QTRY_VERIFY(evaluate(owner + ".host.profileBootstrapComplete").toBool());
    }
    pageReceiver=owner+".host.activeWebView()";
    QTRY_VERIFY(page("location.href === " + quoted(initial) + " && document.readyState === 'complete'").toBool());
    page("window.retainedBeforeFallback=99");
    QCOMPARE(evaluate(owner + ".host.activeWebView().viewTabs.get(0).url").toString(),initial);
    QTest::qWait(100);
    const auto historyBefore=evaluate(owner + ".ui.content_history_export(0)").toString();
    QVERIFY(evaluate(owner + ".host.prepareTransferFallback()").toBool());
    QTest::qWait(100);
    const auto historyAfter=evaluate(owner + ".ui.content_history_export(0)").toString();
    QCOMPARE(page("location.href").toString(),initial);
    QCOMPARE(page("window.retainedBeforeFallback").toInt(),99);
    QCOMPARE(historyAfter,historyBefore);
    QCOMPARE(evaluate(owner + ".host.activeWebView().viewTabs.get(0).url").toString(),initial);
    evaluate(owner + ".host.discardPreparedTransferFallback()");
}

void ApplicationTests::productPolishSecondaryTabs() {
    QTRY_VERIFY(!evaluate("testWindow.browserUi.profile_bootstrap_pending").toBool());
    const auto primaryUrl = text("current_url");
    const auto primaryTabs = evaluate("testWindow.browserUi.tab_count").toInt();
    evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"
        + quoted(primaryUrl) + ",windowProfileName:'tabs-owner',windowProfileLabel:'Tabs owner'})");
    QTRY_VERIFY(evaluate("testWindow.browserWindowRegistry.length>1").toBool());
    const auto token = evaluate("testWindow.browserWindowRegistry[testWindow.browserWindowRegistry.length-1].windowToken").toString();
    const auto entry = "testWindow.browserWindowRegistry.find(function(e){return e.windowToken === " + quoted(token) + "})";
    QTRY_VERIFY(evaluate(entry + ".host.profileBootstrapComplete").toBool());
    pageReceiver = entry + ".host.activeView";
    QTRY_VERIFY(page("location.href === " + quoted(primaryUrl)
        + " && document.readyState === 'complete'").toBool());
    page("window.retainedState=41");
    const QPointer<QObject> firstPage = evaluate(entry + ".host.activeView").value<QObject *>();
    QVERIFY2(evaluate(entry + ".ui.execute_secondary_interactive_command(" + quoted("tab-open " + primaryUrl) + ")").toBool(),
             qPrintable(evaluate(entry + ".ui.status_text").toString()));
    evaluate(entry + ".host.processRuntimeWork()");
    QTRY_COMPARE(evaluate(entry + ".ui.tab_count").toInt(), 2);
    QTRY_COMPARE(evaluate(entry + ".host.tabViews.length").toInt(), 2);
    QTRY_VERIFY(page("location.href === " + quoted(primaryUrl)
        + " && document.readyState === 'complete'").toBool());
    QVERIFY(!page("window.retainedState === 41").toBool());
    page("window.retainedState=82");
    const auto secondId = evaluate(entry + ".ui.tab_id_for_index(1)").toString();
    QVERIFY(evaluate(entry + ".ui.execute_secondary_interactive_command("
        + quoted("tab-move " + secondId + " left") + ")").toBool());
    evaluate(entry + ".host.processRuntimeWork()");
    QTRY_COMPARE(evaluate(entry + ".ui.active_tab_index").toInt(), 0);
    QTRY_VERIFY(evaluate(entry + ".host.activeView.visible").toBool());
    QTRY_COMPARE(page("window.retainedState").toInt(), 82);
    QVERIFY(evaluate(entry + ".ui.execute_secondary_interactive_command('tab-prev')").toBool());
    evaluate(entry + ".host.processRuntimeWork()");
    QTRY_COMPARE(evaluate(entry + ".host.activeView").value<QObject *>(), firstPage.data());
    QTRY_VERIFY(evaluate(entry + ".host.activeView.visible").toBool());
    QTRY_COMPARE(page("window.retainedState").toInt(), 41);
    QVERIFY(evaluate(entry + ".ui.execute_secondary_interactive_command('tab-next')").toBool());
    evaluate(entry + ".host.processRuntimeWork()");
    QTRY_COMPARE(page("window.retainedState").toInt(), 82);
    QVERIFY(evaluate(entry + ".ui.execute_secondary_interactive_command('tab-close')").toBool());
    evaluate(entry + ".host.processRuntimeWork()");
    QTRY_COMPARE(evaluate(entry + ".ui.tab_count").toInt(), 1);
    QTRY_COMPARE(page("window.retainedState").toInt(), 41);
    QCOMPARE(text("current_url"), primaryUrl);
    QCOMPARE(evaluate("testWindow.browserUi.tab_count").toInt(), primaryTabs);
    const QPointer<QObject> livePage = evaluate(entry + ".host.activeView").value<QObject *>();
    QVERIFY(livePage);
    // Moving a live page must retain its JS state and leave a real fallback
    // in the source, rather than treating transferred windows as one-view shells.
    QVERIFY(evaluate(entry + ".ui.execute_secondary_interactive_command('tab-detach')").toBool());
    evaluate(entry + ".host.processRuntimeWork()");
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 3);
    const auto movedToken = evaluate("testWindow.browserWindowRegistry.find(function(e){return e.windowToken !== "
        + quoted(token) + " && e.host !== testWindow}).windowToken").toString();
    const auto moved = "testWindow.browserWindowRegistry.find(function(e){return e.windowToken === " + quoted(movedToken) + "})";
    QTRY_VERIFY(evaluate(moved + ".host.profileBootstrapComplete").toBool());
    QTRY_COMPARE(evaluate(moved + ".ui.tab_count").toInt(), 1);
    QTRY_COMPARE(evaluate(moved + ".host.activeView").value<QObject *>(), livePage.data());
    pageReceiver = moved + ".host.activeView";
    QTRY_COMPARE(page("window.retainedState").toInt(), 41);
    QTRY_COMPARE(evaluate(entry + ".host.tabViews.length").toInt(), 1);
    QVERIFY(evaluate(entry + ".ui.core_window_id !== testWindow.browserUi.core_window_id").toBool());
    QVERIFY(evaluate("testWindow.browserWindowEntryForTarget(" + entry + ".ui.core_window_id).ui === " + entry + ".ui").toBool());
    QVERIFY(evaluate("testWindow.browserWindowEntryForTarget(testWindow.browserUi.core_window_id).host === testWindow").toBool());
    const auto destination = token;
    QVERIFY(evaluate(moved + ".ui.execute_secondary_interactive_command(" + quoted("tab-give " + destination) + ")").toBool());
    evaluate(moved + ".host.processRuntimeWork()");
    const auto transferStatus = evaluate(moved + ".ui.status_text").toString();
    QTRY_VERIFY2(evaluate(entry + ".host.activeView").value<QObject *>() == livePage.data(),
        qPrintable(transferStatus + " / "
            + evaluate(entry + ".ui.status_text").toString()));
    pageReceiver = entry + ".host.activeView";
    QTRY_COMPARE(page("window.retainedState").toInt(), 41);
    QCOMPARE(text("current_url"), primaryUrl);
}

void ApplicationTests::productPolishSecondaryManagers() {
    QTRY_VERIFY(!evaluate("testWindow.browserUi.profile_bootstrap_pending").toBool());
    const auto primaryUrl = text("current_url");
    evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"
        + quoted(primaryUrl) + ",windowProfileName:'manager-owner',windowProfileLabel:'Manager owner'})");
    QTRY_VERIFY(evaluate("testWindow.browserWindowRegistry.length>1").toBool());
    const auto token = evaluate("testWindow.browserWindowRegistry[1].windowToken").toString();
    const auto entry = "testWindow.browserWindowRegistry.find(function(e){return e.windowToken === " + quoted(token) + "})";
    QTRY_VERIFY(evaluate(entry + ".host.profileBootstrapComplete").toBool());
    auto child = qobject_cast<QQuickWindow *>(evaluate(entry + ".host").value<QObject *>());
    QVERIFY(child);
    const auto managers = entry + ".host.profileSessionWorkspace";
    QVERIFY(evaluate(entry + ".ui.execute_secondary_interactive_command('profiles')").toBool());
    evaluate(entry + ".host.processRuntimeWork()");
    QTRY_VERIFY(evaluate(managers + ".profileManagerVisible").toBool());
    QVERIFY(!evaluate("testWindow.profileManagerVisible").toBool());
    QVERIFY(!evaluate(entry + ".host.browserKeyFocusActive").toBool());
    QVERIFY(command("profiles"));
    auto primaryDraft = window->findChild<QQuickItem *>("profileNameInput");
    QVERIFY(primaryDraft);
    QVERIFY(primaryDraft->setProperty("text", "keep-primary-draft"));
    auto name = child->findChild<QQuickItem *>("profileNameInput");
    auto label = child->findChild<QQuickItem *>("profileLabelInput");
    auto create = child->findChild<QQuickItem *>("profileCreateButton");
    QVERIFY(name && label && create);
    QVERIFY(name->setProperty("text", "created-by-child"));
    QVERIFY(label->setProperty("text", "Created in child"));
    QVERIFY(QMetaObject::invokeMethod(create, "click"));
    QTRY_COMPARE(evaluate(entry + ".ui.profile_action_state").toString(), QString("succeeded"));
    QTRY_COMPARE(name->property("text").toString(), QString());
    QVERIFY(text("profile_action_id").isEmpty());
    evaluate(managers + ".closeProfiles()");
    QVERIFY(evaluate("testWindow.profileManagerVisible").toBool());
    QCOMPARE(primaryDraft->property("text").toString(), QString("keep-primary-draft"));
    evaluate("testWindow.profileSessionWorkspaceObject.closeProfiles()");
    QVERIFY(evaluate(entry + ".ui.execute_secondary_interactive_command('tab-open rb://blank')").toBool());
    evaluate(entry + ".host.processRuntimeWork()");
    QTRY_COMPARE(evaluate(entry + ".ui.tab_count").toInt(), 2);
    QVERIFY(evaluate(entry + ".ui.save_named_session('child-tabs')").toBool());
    QTRY_COMPARE(evaluate(entry + ".ui.session_save_state").toString(), QString("succeeded"));
    QVERIFY(evaluate(entry + ".ui.execute_secondary_interactive_command('tab-close')").toBool());
    evaluate(entry + ".host.processRuntimeWork()");
    QTRY_COMPARE(evaluate(entry + ".ui.tab_count").toInt(), 1);
    evaluate(entry + ".host.submitInteractiveCommand('session-load child-tabs')");
    QTRY_VERIFY(evaluate(managers + ".sessionPreviewVisible").toBool());
    QTRY_VERIFY(!evaluate(managers + ".sessionPreviewLoading").toBool());
    QVERIFY(!evaluate(managers + ".sessionPreviewError").toBool());
    QVERIFY(!evaluate("testWindow.sessionPreviewVisible").toBool());
    QVERIFY(evaluate(managers + ".loadPreviewedSession()").toBool());
    QTRY_COMPARE(evaluate(entry + ".ui.tab_count").toInt(), 2);
    QTRY_COMPARE(evaluate(entry + ".host.tabViews.length").toInt(), 2);
    QTRY_VERIFY(!evaluate(managers + ".sessionPreviewVisible").toBool());
    QTRY_COMPARE(evaluate(entry + ".host.activeView.nativeUrl").toString(), QString("rb://blank"));
    // Appending must add real views and retain the existing live renderers.
    const auto retainedView = evaluate(entry + ".host.tabViews[0]").value<QObject *>();
    evaluate(entry + ".host.submitInteractiveCommand('session-load --append child-tabs')");
    QTRY_VERIFY(evaluate(managers + ".sessionPreviewVisible").toBool());
    QTRY_VERIFY(!evaluate(managers + ".sessionPreviewLoading").toBool());
    QVERIFY(evaluate(managers + ".sessionPreviewAppend").toBool());
    QVERIFY(evaluate(managers + ".loadPreviewedSession()").toBool());
    QTRY_COMPARE(evaluate(entry + ".ui.tab_count").toInt(), 4);
    QTRY_COMPARE(evaluate(entry + ".host.tabViews.length").toInt(), 4);
    QCOMPARE(evaluate(entry + ".host.tabViews[0]").value<QObject *>(), retainedView);
    QTRY_VERIFY(!evaluate(managers + ".sessionPreviewVisible").toBool());

    // Opening a regular profile from private management must start at home
    // without exporting the current private URL.
    evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"
        + quoted(primaryUrl + "?private-only=1")
        + ",windowProfileName:'private-manager',windowProfileLabel:'Private',windowPrivateProfile:true})");
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 3);
    const auto privateToken = evaluate("testWindow.browserWindowRegistry.find(function(e){return e.privateProfile}).windowToken").toString();
    const auto privateEntry = "testWindow.browserWindowRegistry.find(function(e){return e.windowToken === " + quoted(privateToken) + "})";
    QTRY_VERIFY(evaluate(privateEntry + ".host.profileBootstrapComplete").toBool());
    QTRY_COMPARE(evaluate(privateEntry + ".ui.current_url").toString(), primaryUrl + "?private-only=1");
    QVERIFY(evaluate(privateEntry + ".ui.request_profile_window('created-by-child')").toBool());
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 4);
    const auto opened = QString("testWindow.browserWindowRegistry.find(function(e){return e.profileName === 'created-by-child'})");
    QTRY_VERIFY(evaluate(opened + ".host.profileBootstrapComplete").toBool());
    QTRY_COMPARE(evaluate(opened + ".host.activeView.nativeUrl").toString(), QString("rb://start"));
    QCOMPARE(evaluate(privateEntry + ".ui.current_url").toString(), primaryUrl + "?private-only=1");
    QVERIFY(evaluate(entry + ".ui.execute_secondary_interactive_command('set --temp ui.font_size_pt=16')").toBool());
    QTRY_COMPARE(child->property("font").value<QFont>().pointSizeF(), 16.0);
    child->resize(640, 480);
    const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
    for (const auto &surface : {QString("profiles"), QString("sessions")}) {
        evaluate(entry + ".host.submitInteractiveCommand(" + quoted(surface) + ")");
        QTRY_VERIFY(evaluate(managers + (surface == "profiles" ? ".profileManagerVisible" : ".sessionManagerVisible")).toBool());
        QTest::qWait(100);
        auto panel = child->findChild<QQuickItem *>(surface == "profiles" ? "profileList" : "sessionList");
        while (panel && panel->objectName() != "ferricModalPanel") panel = panel->parentItem();
        QVERIFY(panel && panel->isVisible());
        QVERIFY(panel->width() > child->width() / 2);
        QVERIFY(panel->height() > child->height() / 2);
        if (!artifacts.isEmpty())
            QVERIFY(child->grabWindow().save(artifacts + "/production-secondary-" + surface + "-compact-16pt.png"));
        evaluate(managers + (surface == "profiles" ? ".closeProfiles()" : ".closeSessions()"));
    }
    QCOMPARE(text("current_url"), primaryUrl);
    QCOMPARE(evaluate("testWindow.browserUi.tab_count").toInt(), 1);
}

void ApplicationTests::productPolishWindowSettings_data() {
    QTest::addColumn<QString>("kind");
    QTest::newRow("regular") << QString("regular");
    QTest::newRow("private") << QString("private");
    QTest::newRow("temporary") << QString("temporary");
}

void ApplicationTests::productPolishWindowSettings() {
    QFETCH(QString, kind);
    const auto primaryFont = evaluate("testWindow.browserUi.chrome_font_size_pt").toDouble();
    const auto primaryUrl = text("current_url");
    if (kind == "regular") {
        QVERIFY(evaluate("testWindow.browserUi.create_profile('settings-owner','Settings owner')").toBool());
        QTRY_COMPARE(text("profile_action_state"), QString("succeeded"));
        evaluate("testWindow.openProfile('settings-owner','Settings owner','rb://blank')");
    } else if (kind == "private") {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:'rb://blank',windowProfileName:'private-settings',windowProfileLabel:'Private settings',windowPrivateProfile:true})");
    } else {
        QVERIFY(evaluate("testWindow.openEphemeralWindow('rb://blank')").toBool());
    }
    QTRY_VERIFY(evaluate("testWindow.browserWindowRegistry.length>1").toBool());
    const auto entry = QString("testWindow.browserWindowRegistry[testWindow.browserWindowRegistry.length-1]");
    auto child = qobject_cast<QQuickWindow *>(evaluate(entry + ".host").value<QObject *>());
    QVERIFY(child);
    QTRY_VERIFY(evaluate(entry + ".host.profileBootstrapComplete").toBool());
    evaluate(entry + ".host.activeView.forceActiveFocus()");
    child->requestActivate();
    QTRY_VERIFY(child->isActive());
    evaluate(entry + ".ui.enter_command()");
    evaluate(entry + ".host.submitInteractiveCommand('settings')");
    auto workspace = child->findChild<QObject *>("secondarySettingsWorkspace");
    QVERIFY(workspace);
    QTRY_VERIFY(workspace->property("settingsVisible").toBool());
    QTRY_COMPARE(evaluate(entry + ".ui.mode").toString(), QString("normal"));
    auto searchInput = child->findChild<QQuickItem *>("settingsSearchInput");
    QVERIFY(searchInput);
    QTRY_VERIFY(searchInput->hasActiveFocus());
    QVERIFY(!evaluate("testWindow.settingsVisible").toBool());
    QVERIFY(!evaluate(entry + ".host.browserKeyFocusActive").toBool());
    QCOMPARE(workspace->property("transientProfile").toBool(), kind != "regular");
    auto install = findVisualItem(child->contentItem(), "settingsInstallScript");
    QVERIFY(install);
    QTRY_VERIFY(!evaluate(entry + ".ui.userscript_inventory_pending").toBool());
    QCOMPARE(evaluate(entry + ".ui.userscript_inventory_ready").toBool(), kind == "regular");
    QTRY_COMPARE(install->isEnabled(), kind == "regular");
    QVERIFY(workspace->setProperty("settingsSearch", "ui.font_size_pt"));
    engine->rootContext()->setContextProperty("settingsWorkspaceUnderTest", workspace);
    evaluate("settingsWorkspaceUnderTest.refreshSettings()");
    auto settingsList = findVisualItem(child->contentItem(), "settingsList");
    QVERIFY(settingsList);
    QTRY_COMPARE(settingsList->property("count").toInt(), 1);
    // Removed delegates can remain in the visual tree until deferred deletion.
    // Select the filtered model row, rather than the first named descendant.
    const auto filteredEditor = [settingsList]() -> QQuickItem * {
        QQuickItem *row = nullptr;
        QMetaObject::invokeMethod(settingsList, "itemAtIndex", Q_RETURN_ARG(QQuickItem *, row), Q_ARG(int, 0));
        return row ? findVisualItem(row, "settingTextEditor") : nullptr;
    };
    QPointer<QQuickItem> editor;
    QTRY_VERIFY((editor = filteredEditor()) && editor->isVisible());
    QCOMPARE(QQmlExpression(qmlContext(editor), editor, "settingRow.rowData.key").evaluate().toString(),
        QString("ui.font_size_pt"));
    QVERIFY(workspace->setProperty("settingsTemporary", true));
    QVERIFY(editor->setProperty("text", "14"));
    QVERIFY(QMetaObject::invokeMethod(editor, "accepted"));
    QTRY_COMPARE(evaluate(entry + ".ui.chrome_font_size_pt").toDouble(), 14.0);
    QCOMPARE(evaluate("testWindow.browserUi.chrome_font_size_pt").toDouble(), primaryFont);
    QCOMPARE(text("current_url"), primaryUrl);
    QTRY_COMPARE(child->property("font").value<QFont>().pointSizeF(), 14.0);
    // Validation errors must retain the edit without exposing the raw error as
    // the primary message or applying it to either window.
    editor = filteredEditor();
    QVERIFY(editor);
    child->resize(640, 480);
    child->requestActivate();
    QTRY_VERIFY(child->isActive());
    QVERIFY(!child->grabWindow().isNull());
    QTest::qWait(100);
    editor->forceActiveFocus();
    QTRY_VERIFY(editor->hasActiveFocus());
    QTest::keyClick(child, Qt::Key_A, Qt::ControlModifier);
    QTest::keyClick(child, Qt::Key_0);
    QTest::keyClick(child, Qt::Key_Return);
    QTRY_VERIFY2(workspace->property("noticeFailed").toBool(),
        qPrintable(QString("editor=%1; status=%2").arg(editor ? editor->property("text").toString() : QString("destroyed"),
            evaluate(entry + ".ui.status_text").toString())));
    QVERIFY(editor);
    QVERIFY(!workspace->property("noticeDetail").toString().isEmpty());
    QCOMPARE(editor->property("text").toString(), QString("0"));
    QCOMPARE(evaluate(entry + ".ui.chrome_font_size_pt").toDouble(), 14.0);
    auto form = findVisualItem(child->contentItem(), "settingsScroll");
    QVERIFY(form);
    auto viewport = qobject_cast<QQuickItem *>(form->property("contentItem").value<QObject *>());
    QVERIFY(viewport);
    QTRY_VERIFY2(viewport->mapRectToScene(viewport->boundingRect()).adjusted(-1,-1,1,1)
        .contains(editor->mapRectToScene(editor->boundingRect())),
        "The focused invalid setting must remain fully visible in the compact form");
    const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
    if (!artifacts.isEmpty()) {
        child->resize(640, 480);
        QTest::qWait(100);
        QVERIFY(child->grabWindow().save(artifacts + "/production-window-settings-" + kind + ".png"));
    }
    child->requestActivate();
    QTest::keyClick(child, Qt::Key_Escape);
    QTRY_VERIFY(!workspace->property("settingsVisible").toBool());
    evaluate(entry + ".ui.enter_command()");
    evaluate(entry + ".host.submitInteractiveCommand('help ui.font_size_pt')");
    QTRY_VERIFY(workspace->property("settingsVisible").toBool());
    QCOMPARE(workspace->property("settingsSearch").toString(), QString("ui.font_size_pt"));
    QTest::keyClick(child, Qt::Key_Escape);
    QTRY_VERIFY(!workspace->property("settingsVisible").toBool());
    evaluate(entry + ".host.windowShutdownApproved=true");
    child->close();
}

void ApplicationTests::productPolishIndependentWindowInput() {
    QVERIFY(evaluate("testWindow.openEphemeralWindow('rb://blank')").toBool());
    QTRY_VERIFY(evaluate("testWindow.browserWindowRegistry.length > 1").toBool());
    const auto entry = QString("testWindow.browserWindowRegistry[testWindow.browserWindowRegistry.length-1]");
    auto child = qobject_cast<QQuickWindow *>(evaluate(entry + ".host").value<QObject *>());
    QVERIFY(child);
    QTRY_VERIFY(evaluate(entry + ".host.profileBootstrapComplete").toBool());
    QVERIFY(command("settings"));
    QTRY_VERIFY(evaluate("testWindow.settingsVisible").toBool());
    evaluate("testWindow.settingsSearch='primary draft'");
    child->requestActivate();
    QTRY_VERIFY(child->isActive());
    evaluate(entry + ".host.activeView.forceActiveFocus()");
    QVERIFY(evaluate(entry + ".host.browserKeyFocusActive").toBool());
    QTest::keyClick(child, Qt::Key_Colon);
    QTRY_COMPARE(evaluate(entry + ".ui.mode").toString(), QString("command"));
    QTest::keyClick(child, Qt::Key_Escape);
    QTRY_COMPARE(evaluate(entry + ".ui.mode").toString(), QString("normal"));
    QVERIFY(evaluate("testWindow.settingsVisible").toBool());
    QCOMPARE(evaluate("testWindow.settingsSearch").toString(), QString("primary draft"));
    evaluate(entry + ".ui.enter_command()");
    evaluate(entry + ".host.submitInteractiveCommand('settings')");
    auto workspace = child->findChild<QObject *>("secondarySettingsWorkspace");
    QVERIFY(workspace);
    QTRY_VERIFY(workspace->property("settingsVisible").toBool());
    auto search = child->findChild<QQuickItem *>("settingsSearchInput");
    QVERIFY(search);
    QTRY_VERIFY(search->hasActiveFocus());
    window->requestActivate();
    QTRY_VERIFY(window->isActive());
    key(Qt::Key_Escape);
    QTRY_VERIFY(!evaluate("testWindow.settingsVisible").toBool());
    QVERIFY(workspace->property("settingsVisible").toBool());
    child->requestActivate();
    QTRY_VERIFY(child->isActive());
    QTRY_VERIFY(search->hasActiveFocus());
    QTest::keyClick(child, Qt::Key_Escape);
    QTRY_VERIFY(!workspace->property("settingsVisible").toBool());
    evaluate(entry + ".host.windowShutdownApproved=true");
    child->close();
}

void ApplicationTests::productPolishPermissionWindowOwnership_data() {
    QTest::addColumn<bool>("closeQueued");
    QTest::newRow("handoff") << false;
    QTest::newRow("close-queued-window") << true;
}
void ApplicationTests::productPolishPermissionWindowOwnership() {
    QFETCH(bool, closeQueued);
    QVERIFY(evaluate("testWindow.browserUi.create_profile('permission-owner','Permission owner')").toBool());
    QTRY_COMPARE(text("profile_action_state"), QString("succeeded"));
    evaluate("testWindow.openProfile('permission-owner','Permission owner'," + quoted(text("current_url")) + ")");
    const auto entry = QString("testWindow.browserWindowRegistry[testWindow.browserWindowRegistry.length-1]");
    QTRY_VERIFY(evaluate("testWindow.browserWindowRegistry.length>1").toBool());
    auto child = qobject_cast<QQuickWindow *>(evaluate(entry + ".host").value<QObject *>());
    QVERIFY(child);
    QTRY_VERIFY(evaluate(entry + ".host.profileBootstrapComplete").toBool());
    const auto request = QString("navigator.geolocation.getCurrentPosition(()=>window.geoResult='allowed',()=>window.geoResult='denied');true");
    QVERIFY(page(request).toBool());
    QTRY_VERIFY(evaluate("testWindow.permissionPromptVisible").toBool());
    QVERIFY(evaluate("testWindow.pendingPermissionUi===testWindow.browserUi").toBool());
    pageReceiver = entry + ".host.activeView";
    QTRY_VERIFY(page("document.getElementById('button')!==null").toBool());
    QVERIFY(page(request).toBool());
    QTRY_COMPARE(evaluate("testWindow.permissionPromptQueue.length").toInt(), 2);
    QCOMPARE(evaluate("testWindow.pendingPermissionGroupCount").toInt(), 1);
    if (closeQueued) {
        evaluate(entry + ".host.windowShutdownApproved=true");
        child->close();
        QTRY_COMPARE(evaluate("testWindow.permissionPromptQueue.length").toInt(), 1);
        QVERIFY(evaluate("testWindow.pendingPermissionUi===testWindow.browserUi").toBool());
        QVERIFY(evaluate("testWindow.permissionPromptVisible").toBool());
        evaluate("testWindow.decidePermission(false,'')");
        QTRY_VERIFY(!evaluate("testWindow.permissionPromptVisible").toBool());
        pageReceiver = "testWindow.activeWebView()";
        QTRY_COMPARE(page("window.geoResult").toString(), QString("denied"));
        return;
    }
    evaluate("testWindow.decidePermission(false,'')");
    QTRY_VERIFY(evaluate("testWindow.pendingPermissionUi===" + entry + ".ui").toBool());
    QVERIFY(evaluate("testWindow.pendingPermissionHost===" + entry + ".host").toBool());
    QTRY_VERIFY(!evaluate("testWindow.nativePromptActiveFor(testWindow.browserUi)").toBool());
    QTRY_VERIFY(evaluate("testWindow.browserKeyFocusActive").toBool());
    QVERIFY(evaluate("testWindow.nativePromptActiveFor(" + entry + ".ui)").toBool());
    QVERIFY(!evaluate(entry + ".host.browserKeyFocusActive").toBool());
    evaluate("testWindow.decidePermission(false,'')");
    QTRY_VERIFY(!evaluate("testWindow.permissionPromptVisible").toBool());
    QTRY_COMPARE(page("window.geoResult").toString(), QString("denied"));
    QTRY_VERIFY(evaluate(entry + ".host.browserKeyFocusActive").toBool());
    pageReceiver = "testWindow.activeWebView()";
    QTRY_COMPARE(page("window.geoResult").toString(), QString("denied"));
    evaluate(entry + ".host.windowShutdownApproved=true");
    child->close();
}

void ApplicationTests::productPolishOverlappingModalInput_data() {
    QTest::addColumn<bool>("removal");
    QTest::addColumn<bool>("enter");
    QTest::addColumn<QSize>("windowSize");
    QTest::addColumn<int>("fontSize");
    for (const auto &size : {QSize(900, 700), QSize(640, 480), QSize(390, 600)}) {
        const int fontSize = size.width() == 900 ? 10 : size.width() == 640 ? 16 : 20;
        for (const bool removal : {false, true}) {
            for (const bool enter : {false, true}) {
                const auto name = QString("%1-%2-%3").arg(removal ? "removal" : "settings",
                    enter ? "enter" : "escape").arg(size.width()).toLatin1();
                QTest::newRow(name.constData()) << removal << enter << size << fontSize;
            }
        }
    }
}
void ApplicationTests::productPolishOverlappingModalInput() {
    QFETCH(bool, removal);
    QFETCH(bool, enter);
    QFETCH(QSize, windowSize);
    QFETCH(int, fontSize);
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('ui.font_size_pt',"
                     + quoted(QString::number(fontSize)) + ",true)").toBool());
    QTRY_COMPARE(evaluate("testWindow.browserUi.chrome_font_size_pt").toInt(), fontSize);
    window->resize(windowSize);
    QVERIFY(command("settings"));
    QTRY_VERIFY(evaluate("testWindow.settingsVisible").toBool());
    evaluate("testWindow.settingsSearch='preserved draft'");
    auto workspace = window->findChild<QObject *>("primarySettingsWorkspace");
    QVERIFY(workspace);
    auto search = window->findChild<QQuickItem *>("settingsSearchInput");
    QVERIFY(search);
    QTRY_VERIFY(search->hasActiveFocus());
    if (removal) {
        engine->rootContext()->setContextProperty("modalSettingsUnderTest", workspace);
        evaluate("modalSettingsUnderTest.confirmRemoveUserscript('pending_script')");
        QTRY_COMPARE(evaluate("testWindow.pendingUserscriptRemoval").toString(), QString("pending_script"));
    }
    QVERIFY(page("navigator.geolocation.getCurrentPosition(()=>window.geoResult='allowed',()=>window.geoResult='denied');true").toBool());
    QTRY_VERIFY(evaluate("testWindow.permissionPromptVisible").toBool());
    auto deny = findVisualItem(window->contentItem(), "commandDialogAction-deny");
    QVERIFY(deny);
    QTRY_VERIFY(deny->hasActiveFocus());
    auto prompt = findVisualItem(window->contentItem(), "sitePermissionPrompt");
    QVERIFY(prompt);
    auto header = findVisualItem(prompt, "dialogHeaderScroll");
    auto message = findVisualItem(prompt, "dialogMessageLabel");
    auto denyLabel = findVisualItem(deny, "commandActionLabel");
    QVERIFY(header && message && denyLabel);
    QTRY_VERIFY(message->mapToItem(header, QPointF(0, message->height())).y() <= header->height() + 1);
    QCOMPARE(denyLabel->property("lineCount").toInt(), 1);
    const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
    if (!artifacts.isEmpty()) {
        QTest::qWait(50);
        QVERIFY(window->grabWindow().save(artifacts + "/production-permission-"
            + QString::fromLatin1(QTest::currentDataTag()) + ".png"));
    }
    key(enter ? Qt::Key_Return : Qt::Key_Escape);
    QTRY_VERIFY(!evaluate("testWindow.permissionPromptVisible").toBool());
    QTRY_COMPARE(page("window.geoResult").toString(), QString("denied"));
    QVERIFY(evaluate("testWindow.settingsVisible").toBool());
    QCOMPARE(evaluate("testWindow.settingsSearch").toString(), QString("preserved draft"));
    if (removal) {
        QCOMPARE(evaluate("testWindow.pendingUserscriptRemoval").toString(), QString("pending_script"));
        key(Qt::Key_Escape);
        QTRY_VERIFY(evaluate("testWindow.pendingUserscriptRemoval").toString().isEmpty());
        QVERIFY(evaluate("testWindow.settingsVisible").toBool());
    }
    QTRY_VERIFY(search->hasActiveFocus());
    key(Qt::Key_Escape);
    QTRY_VERIFY(!evaluate("testWindow.settingsVisible").toBool());
    QTRY_VERIFY(evaluate("testWindow.browserKeyFocusActive").toBool());
}

void ApplicationTests::productPolishUserscriptFeedback() {
    QTRY_VERIFY(!evaluate("testWindow.browserUi.profile_bootstrap_pending").toBool());
    QVERIFY(command("settings"));
    QTRY_VERIFY(evaluate("testWindow.settingsVisible").toBool());
    auto workspace = window->findChild<QObject *>("primarySettingsWorkspace");
    QVERIFY(workspace);
    auto scriptsTab = window->findChild<QQuickItem *>("settingsScriptsTab");
    QVERIFY(scriptsTab);
    QVERIFY(QMetaObject::invokeMethod(scriptsTab, "click"));
    QTRY_VERIFY(!evaluate("testWindow.browserUi.userscript_inventory_pending").toBool());
    QVERIFY(evaluate("testWindow.browserUi.userscript_inventory_ready").toBool());
    QVERIFY(!evaluate("testWindow.browserUi.set_userscript_enabled('', false)").toBool());
    const auto rejectedDetail = workspace->property("noticeDetail").toString();
    QVERIFY(!rejectedDetail.isEmpty());
    QCOMPARE(text("userscript_install_state"), "error:" + rejectedDetail);
    QVERIFY(!text("status_text").contains(rejectedDetail));
    QFile source(directory.filePath("toggle_owner.js"));
    QVERIFY(source.open(QIODevice::WriteOnly));
    source.write("void 0;"); source.close();
    QFile manifest(directory.filePath("toggle_owner.toml"));
    QVERIFY(manifest.open(QIODevice::WriteOnly));
    manifest.write("schema_version = 1\nname = \"toggle_owner\"\nexecutable = \"/bin/true\"\n"
                   "page_world = true\nsource = \"toggle_owner.js\"\n"
                   "matches = [\"http://127.0.0.1:*/*\"]\nrun_at = \"document_end\"\n");
    manifest.close();
    engine->rootContext()->setContextProperty("settingsWorkspaceUnderTest", workspace);
    engine->rootContext()->setContextProperty("settingsManifestUnderTest", QUrl::fromLocalFile(manifest.fileName()));
    QVERIFY2(evaluate("settingsWorkspaceUnderTest.installManifest(settingsManifestUnderTest)").toBool(),
             qPrintable(evaluate("testWindow.settingsNotice").toString() + ": " + workspace->property("noticeDetail").toString()));
    // Refresh while the accepted mutation owns the single-flight worker.
    // Repeated refreshes coalesce and must not replace its operation feedback.
    QVERIFY(evaluate("testWindow.browserUi.refresh_userscript_inventory()").toBool());
    QVERIFY(evaluate("testWindow.browserUi.refresh_userscript_inventory()").toBool());
    QVERIFY(evaluate("testWindow.browserUi.userscript_inventory_pending").toBool());
    QTRY_COMPARE(text("userscript_install_state"), QString("installed"));
    QTRY_VERIFY(!evaluate("testWindow.browserUi.userscript_inventory_pending").toBool());
    QCOMPARE(text("userscript_inventory_error"), QString());
    auto toggle = findVisualItem(window->contentItem(), "userscriptEnableToggle");
    QTRY_VERIFY((toggle = findVisualItem(window->contentItem(), "userscriptEnableToggle")));
    QVERIFY(toggle->property("checked").toBool());
    const auto storageBase = evaluate("testWindow.storageBasePath").toString();
    QDirIterator installed(storageBase, QStringList{"toggle_owner.toml"}, QDir::Files, QDirIterator::Subdirectories);
    QVERIFY(installed.hasNext());
    const auto installedPath = installed.next();
    QVERIFY(!installed.hasNext());
    const auto scriptsDirectory = QFileInfo(installedPath).absolutePath();
    const auto backupDirectory = scriptsDirectory + ".held";
    // A read failure keeps the last loaded rows visible, disables stale
    // actions and never claims that no scripts are installed.
    QVERIFY(QDir().rename(scriptsDirectory, backupDirectory));
    QFile readBlocker(scriptsDirectory);
    QVERIFY(readBlocker.open(QIODevice::WriteOnly));
    readBlocker.write("fixture blocks the inventory read"); readBlocker.close();
    QVERIFY(evaluate("testWindow.browserUi.refresh_userscript_inventory()").toBool());
    QVERIFY(evaluate("testWindow.browserUi.refresh_userscript_inventory()").toBool());
    QTRY_VERIFY(!evaluate("testWindow.browserUi.userscript_inventory_pending").toBool());
    const auto inventoryDetail = text("userscript_inventory_error");
    QVERIFY2(!inventoryDetail.isEmpty(), "inventory failure must retain its diagnostic detail");
    QVERIFY(evaluate("testWindow.browserUi.userscript_names.indexOf('toggle_owner') >= 0").toBool());
    QTRY_VERIFY(!toggle->isEnabled());
    auto inventoryFeedback = window->findChild<QQuickItem *>("userscriptInventoryFeedback");
    auto inventoryEmpty = window->findChild<QQuickItem *>("userscriptInventoryEmpty");
    QVERIFY(inventoryFeedback && inventoryFeedback->isVisible());
    QVERIFY(inventoryEmpty && !inventoryEmpty->isVisible());
    QVERIFY(!inventoryFeedback->property("message").toString().contains(inventoryDetail));
    QVERIFY(!text("status_text").contains(inventoryDetail));
    auto inventoryDetails = inventoryFeedback->findChild<QQuickItem *>("managerFeedbackDetails");
    auto inventoryDisclosure = inventoryFeedback->findChild<QQuickItem *>("managerFeedbackDetailsButton");
    QVERIFY(inventoryDetails && !inventoryDetails->isVisible());
    QVERIFY(inventoryDisclosure);
    QVERIFY(QMetaObject::invokeMethod(inventoryDisclosure, "click"));
    QTRY_VERIFY(inventoryDetails->isVisible());
    QCOMPARE(inventoryDetails->property("text").toString(), inventoryDetail);
    QVERIFY(QFile::remove(scriptsDirectory));
    QVERIFY(QDir().rename(backupDirectory, scriptsDirectory));
    QVERIFY(evaluate("testWindow.browserUi.refresh_userscript_inventory()").toBool());
    QTRY_VERIFY(!evaluate("testWindow.browserUi.userscript_inventory_pending").toBool());
    QCOMPARE(text("userscript_inventory_error"), QString());
    QTRY_VERIFY((toggle = findVisualItem(window->contentItem(), "userscriptEnableToggle"))
                && toggle->isEnabled() && toggle->property("checked").toBool());
    QVERIFY(QDir().rename(scriptsDirectory, backupDirectory));
    QFile blocker(scriptsDirectory);
    QVERIFY(blocker.open(QIODevice::WriteOnly));
    blocker.write("fixture blocks the script update"); blocker.close();
    QVERIFY(QMetaObject::invokeMethod(toggle, "click"));
    QVERIFY(!toggle->property("checked").toBool());
    QVERIFY(workspace->property("noticePending").toBool());
    QTRY_VERIFY(text("userscript_install_state").startsWith("error:"));
    QTRY_VERIFY(toggle->property("checked").toBool());
    QVERIFY(workspace->property("noticeFailed").toBool());
    QVERIFY(!workspace->property("noticeDetail").toString().isEmpty());
    // Keep the complete worker error in explicit disclosure, and give the
    // ordinary status bar a readable summary instead of raw diagnostics.
    const auto errorDetail = workspace->property("noticeDetail").toString();
    QCOMPARE(text("userscript_install_state"), "error:" + errorDetail);
    QVERIFY2(!text("status_text").contains(errorDetail), qPrintable(errorDetail));
    QVERIFY(QFile::remove(scriptsDirectory));
    QVERIFY(QDir().rename(backupDirectory, scriptsDirectory));
    // Restoring an externally replaced directory requires a successful catalog
    // refresh before its stale rows become actionable again.
    QVERIFY(evaluate("testWindow.browserUi.refresh_userscript_inventory()").toBool());
    QTRY_VERIFY(!evaluate("testWindow.browserUi.userscript_inventory_pending").toBool());
    QCOMPARE(text("userscript_inventory_error"), QString());
    QTRY_VERIFY(toggle->isEnabled());
    QVERIFY(QMetaObject::invokeMethod(toggle, "click"));
    QTRY_COMPARE(text("userscript_install_state"), QString("disabled"));
    QTRY_VERIFY((toggle = findVisualItem(window->contentItem(), "userscriptEnableToggle"))
                && !toggle->property("checked").toBool() && toggle->isEnabled());
    QVERIFY(!workspace->property("noticePending").toBool());
    QVERIFY(!workspace->property("noticeFailed").toBool());
    QFile saved(installedPath);
    QVERIFY(saved.open(QIODevice::ReadOnly));
    QVERIFY(saved.readAll().contains("enabled = false"));
    saved.close();
    QFile corruptManifest(scriptsDirectory + "/unrelated.toml");
    QVERIFY(corruptManifest.open(QIODevice::WriteOnly));
    corruptManifest.write("invalid = ["); corruptManifest.close();
    QVERIFY(QMetaObject::invokeMethod(toggle, "click"));
    QTRY_COMPARE(text("userscript_install_state"), QString("enabled"));
    QVERIFY(!text("userscript_inventory_error").isEmpty());
    QVERIFY(text("status_text").startsWith("Userscript enabled"));
    QVERIFY(!workspace->property("noticeFailed").toBool());
    QVERIFY(saved.open(QIODevice::ReadOnly));
    QVERIFY(saved.readAll().contains("enabled = true")); saved.close();
    QVERIFY(corruptManifest.remove());
    QVERIFY(evaluate("testWindow.browserUi.refresh_userscript_inventory()").toBool());
    QTRY_VERIFY(!evaluate("testWindow.browserUi.userscript_inventory_pending").toBool());
    QCOMPARE(text("userscript_inventory_error"), QString());
    QTRY_VERIFY((toggle = findVisualItem(window->contentItem(), "userscriptEnableToggle"))
                && toggle->isEnabled() && toggle->property("checked").toBool());
    auto remove = findVisualItem(window->contentItem(), "userscriptRemoveButton");
    QVERIFY(remove);
    remove->forceActiveFocus();
    QSignalSpy rendered(window, &QQuickWindow::frameSwapped);
    window->requestUpdate();
    QTRY_VERIFY(!rendered.isEmpty());
    QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier,
                     remove->mapToScene(QPointF(remove->width()/2, remove->height()/2)).toPoint());
    auto confirmation = findVisualItem(window->contentItem(), "userscriptRemovalConfirmation");
    QTRY_VERIFY((confirmation = findVisualItem(window->contentItem(), "userscriptRemovalConfirmation")));
    key(Qt::Key_Escape);
    QTRY_VERIFY(!confirmation->isVisible());
    QVERIFY(evaluate("testWindow.settingsVisible").toBool());
    QVERIFY(QFile::exists(installedPath));
    remove->forceActiveFocus();
    rendered.clear(); window->requestUpdate(); QTRY_VERIFY(!rendered.isEmpty());
    QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier,
                     remove->mapToScene(QPointF(remove->width()/2, remove->height()/2)).toPoint());
    QTRY_VERIFY(confirmation->isVisible());
    auto confirm = findVisualItem(confirmation, "commandDialogAction-confirm");
    QVERIFY(confirm);
    rendered.clear(); window->requestUpdate(); QTRY_VERIFY(!rendered.isEmpty());
    QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier,
                     confirm->mapToScene(QPointF(confirm->width()/2, confirm->height()/2)).toPoint());
    QTRY_COMPARE(text("userscript_install_state"), QString("removed"));
    QVERIFY(!QFile::exists(installedPath));
    QVERIFY(evaluate("testWindow.settingsVisible").toBool());
    key(Qt::Key_Escape);
    QTRY_VERIFY(!evaluate("testWindow.settingsVisible").toBool());
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

void ApplicationTests::productPolishSessionSaveBackpressure() {
    QTRY_VERIFY(evaluate("testWindow.browserUi.flush_durable_state()").toBool());
    QDirIterator files(evaluate("testWindow.storageBasePath").toString(),
        QStringList{"browser.sqlite"},QDir::Files,QDirIterator::Subdirectories);
    QVERIFY(files.hasNext());
    sqlite3 *raw=nullptr;
    const auto path=files.next().toUtf8();
    QCOMPARE(sqlite3_open_v2(path.constData(),&raw,SQLITE_OPEN_READWRITE,nullptr),SQLITE_OK);
    const std::unique_ptr<sqlite3,decltype(&sqlite3_close)> database(raw,sqlite3_close);
    sqlite3_busy_timeout(database.get(),5000);
    QCOMPARE(sqlite3_exec(database.get(),"BEGIN IMMEDIATE",nullptr,nullptr,nullptr),SQLITE_OK);
    // A real history write holds the metadata worker behind this transaction.
    // The automatic recovery save then queues behind it without blocking Qt.
    page("history.replaceState(null,'','/session-save-backpressure');true");
    QTRY_VERIFY(text("current_url").endsWith("/session-save-backpressure"));
    const auto requestedUrl=text("current_url");
    QVERIFY(evaluate("testWindow.browserUi.set_scroll_position(0,0,17)").toBool());
    QTest::qWait(750);
    QVERIFY2(evaluate("testWindow.browserUi.save_named_session('backpressure-named')").toBool(),
        qPrintable(text("status_text")));
    QCOMPARE(text("session_save_state"),QString("pending"));
    QVERIFY(!evaluate("testWindow.browserUi.save_named_session('duplicate-named')").toBool());
    // The accepted named save owns the snapshot from the request, even if
    // the user continues browsing before the worker catches up.
    page("history.replaceState(null,'','/after-requested-session-save');true");
    QTRY_VERIFY(text("current_url").endsWith("/after-requested-session-save"));
    QCOMPARE(sqlite3_exec(database.get(),"ROLLBACK",nullptr,nullptr,nullptr),SQLITE_OK);
    QTRY_COMPARE(text("session_save_state"),QString("succeeded"));
    QTRY_VERIFY(evaluate("testWindow.browserUi.list_named_sessions().indexOf('backpressure-named')>=0").toBool());
    evaluate("testWindow.showSessionPreview('backpressure-named',false)");
    QTRY_VERIFY(!evaluate("testWindow.sessionPreviewLoading").toBool());
    QVERIFY(!evaluate("testWindow.sessionPreviewError").toBool());
    QCOMPARE(evaluate("testWindow.browserUi.session_preview_urls[0]").toString(),requestedUrl);
    QVERIFY(text("status_text")!=QString("Session checkpoint saved"));
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
    const auto folder = QUrl::fromLocalFile(directory.path()).toString(QUrl::FullyEncoded);
    const auto name = QString::fromUtf8("new résumé + %2F#?.txt");
    for (const auto &suggestion : {directory.filePath(name), name, "/" + directory.filePath(name)}) {
        const auto value = evaluate("testWindow.browserUi.file_dialog_default_url(" + quoted(suggestion)
            + "," + quoted(folder) + ")").toUrl();
        QCOMPARE(ferric_browser_local_file_path(value), directory.filePath(name));
        QVERIFY(value.toString(QUrl::FullyEncoded).contains("%252F%23%3F"));
    }
    for (const auto &base : {QString(), QString("https://example.test/"), QString("file://server/tmp/")})
        QVERIFY(evaluate("testWindow.browserUi.file_dialog_default_url(" + quoted(name) + "," + quoted(base) + ")").toUrl().isEmpty());
    const auto nulName = QStringLiteral("bad") + QChar::Null + QStringLiteral("name.txt");
    QVERIFY(evaluate("testWindow.browserUi.file_dialog_default_url(" + quoted(nulName) + "," + quoted(folder) + ")").toUrl().isEmpty());
    QVERIFY(evaluate("testWindow.browserUi.file_dialog_default_url(''," + quoted(folder) + ")").toUrl().isEmpty());
    for (const auto &invalid : QStringList{"https://example.com/file", "file:relative.txt", "file://server/tmp/file", "file:///tmp/file?query", "file:///tmp/file#fragment", "file:///tmp/bad%00name", "file:///tmp/bad%xy", "file://user@localhost/tmp/file", ""}) {
        QVERIFY2(ferric_browser_local_file_path(QUrl::fromEncoded(invalid.toUtf8(), QUrl::StrictMode)).isEmpty(), qPrintable(invalid));
    }
}

void ApplicationTests::productPolishStartupProblems() {
    QTRY_VERIFY(!evaluate("testWindow.browserUi.profile_bootstrap_pending").toBool());
    const auto configPath = directory.filePath("private-startup-config.toml");
    const auto originalWindow = text("core_window_id");
    const auto originalUrl = text("current_url");
    // Observe a real asynchronous watcher failure, rather than whichever
    // startup/navigation status happens to win the initial event ordering.
    QFile invalidConfig(configPath);
    QVERIFY(invalidConfig.open(QIODevice::WriteOnly | QIODevice::Truncate));
    invalidConfig.write("schema_version = [ malformed private setting after watcher");
    invalidConfig.close();
    QTRY_VERIFY(evaluate("testWindow.browserUi.profile_startup_details()").toString().contains("after watcher"));
    QVERIFY(evaluate("testWindow.browserUi.profile_startup_notice").toString().contains("settings couldn't be loaded"));
    // Navigation may replace transient startup status; the retained local
    // issue and the diagnostics summary must still explain the degradation.
    QVERIFY(!text("status_text").contains(configPath));
    QVERIFY(evaluate("testWindow.browserUi.profile_startup_details()").toString().contains(configPath));
    // A rejected replacement must leave the usable profile and its original
    // configuration failure intact, while retaining the latest attempt too.
    evaluate("testWindow.browserUi.configure_profile(false,false,'Invalid replacement','',testWindow.storageBasePath)");
    QTRY_VERIFY(!evaluate("testWindow.browserUi.profile_bootstrap_pending").toBool());
    QCOMPARE(text("core_window_id"), originalWindow);
    QCOMPARE(text("current_url"), originalUrl);
    QVERIFY(evaluate("testWindow.browserUi.profile_startup_notice").toString().contains("couldn't be opened"));
    const auto detail = evaluate("testWindow.browserUi.profile_startup_details()").toString();
    QVERIFY(detail.contains("Latest profile attempt"));
    QVERIFY(detail.contains(configPath));
    QVERIFY(detail.contains("\n1 |"));
    QVERIFY(!detail.contains(QChar(0xfffd)));
    evaluate("testWindow.showDiagnostics()");
    QTRY_VERIFY(evaluate("testWindow.browserUi.diagnostics_available").toBool());
    const auto reportText = evaluate("testWindow.diagnosticsText").toString();
    const auto report = QJsonDocument::fromJson(reportText.toUtf8()).object();
    const auto startup = report["profile_startup"].toObject();
    QCOMPARE(startup["latest_attempt"].toObject()["status"].toString(), QString("failed"));
    QCOMPARE(startup["latest_attempt"].toObject()["failure_category"].toString(), QString("profile-open"));
    QVERIFY(startup["active_profile"].toObject()["issues"].toArray().contains("configuration"));
    QVERIFY(evaluate("testWindow.browserUi.diagnostics_summary.join(' ')").toString().contains("Some settings couldn't be loaded"));
    QVERIFY(!reportText.contains("private-startup-config"));
    QVERIFY(!reportText.contains("malformed private setting"));
    auto toggle = findVisualItem(window->contentItem(), "diagnosticsStartupToggle");
    auto details = findVisualItem(window->contentItem(), "diagnosticsStartupText");
    QVERIFY(toggle && details);
    QVERIFY(!details->isVisible());
    QVERIFY(QMetaObject::invokeMethod(toggle, "click"));
    QTRY_VERIFY(details->isVisible());
    QCOMPARE(details->property("text").toString(), detail);
    const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
    if (!artifacts.isEmpty()) {
        QDir().mkpath(artifacts);
        QVERIFY(window->grabWindow().save(artifacts+"/production-local-startup-details.png"));
    }
    const auto path = directory.filePath("startup-report.json");
    QVERIFY(evaluate("testWindow.browserUi.export_diagnostics("+quoted(path)+")").toBool());
    QFile saved(path);QVERIFY(saved.open(QIODevice::ReadOnly));
    const auto payload = saved.readAll();
    QVERIFY(!payload.contains("private-startup-config"));
    QVERIFY(!payload.contains("malformed private setting"));
    evaluate("testWindow.closeDiagnostics()");
}

void ApplicationTests::productPolishProfileRecovery_data() {
    QTest::addColumn<QString>("position");
    for (const auto &position : {"top", "bottom", "left", "right"})
        QTest::newRow(position) << QString::fromLatin1(position);
}

void ApplicationTests::productPolishProfileRecovery() {
    QTest::failOnWarning(QRegularExpression(".*(invalid context|TypeError|ReferenceError|Binding loop).*"));
    QFETCH(QString, position);
    QTRY_VERIFY(!evaluate("testWindow.browserUi.profile_bootstrap_pending").toBool());
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('ui.statusbar','never',true)").toBool());
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('ui.tabs','always',true)").toBool());
    QVERIFY(evaluate("testWindow.browserUi.set_runtime_setting('ui.tab_position',"+quoted(position)+",true)").toBool());
    auto notice=findVisualItem(window->contentItem(),"profileHealthNotice");
    auto action=findVisualItem(window->contentItem(),"profileHealthDetails");
    QVERIFY(notice && action);QTRY_VERIFY(notice->isVisible());
    evaluate("testWindow.browserUi.status_text='A later navigation status'");
    QVERIFY(!evaluate("testWindow.statusBarVisible").toBool());
    QVERIFY(text("profile_startup_notice").contains("settings couldn't be loaded"));
    QTRY_VERIFY(evaluate("(()=>{var v=testWindow.activeWebView();return v.mapToItem(testWindow.contentItem,0,v.height).y})()").toDouble()
        <= notice->mapToItem(window->contentItem(),QPointF()).y()+1);
    page("window.profileRecoverySentinel=47;true");
    const auto artifacts=qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
    if (!artifacts.isEmpty()) {
        QDir().mkpath(artifacts);
        QVERIFY(window->grabWindow().save(artifacts+"/production-profile-notice-"+position+".png"));
    }
    action->forceActiveFocus();QTRY_VERIFY(action->hasActiveFocus());
    key(Qt::Key_Return);
    QTRY_VERIFY(evaluate("testWindow.diagnosticsVisible").toBool());
    QCOMPARE(evaluate("testWindow.diagnosticsUi").value<QObject *>(),evaluate("testWindow.browserUi").value<QObject *>());
    QTRY_VERIFY(evaluate("testWindow.browserUi.diagnostics_available").toBool());
    key(Qt::Key_Escape);QTRY_VERIFY(!evaluate("testWindow.diagnosticsVisible").toBool());
    const auto includePath=directory.filePath("configuration/included/settings.toml");
    QVERIFY(QDir().mkpath(QFileInfo(includePath).absolutePath()));
    QFile included(includePath);QVERIFY(included.open(QIODevice::WriteOnly|QIODevice::Truncate));
    included.write("[ui]\nfont_size_pt=12\n");
    if (position=="bottom") included.write("[hints.selectors.all]\nmode='replace'\nselectors=['button[data-recovery]']\n");
    included.close();
    QFile corrected(text("config_path"));
    QVERIFY(corrected.open(QIODevice::WriteOnly|QIODevice::Truncate));
    corrected.write("schema_version=3\ninclude=['../included/settings.toml']\n");corrected.close();
    QVERIFY(command("config-reload"));
    QTRY_COMPARE(text("profile_startup_notice"),QString());
    QTRY_VERIFY(!notice->isVisible());
    QCOMPARE(evaluate("testWindow.browserUi.profile_startup_details()").toString(),QString());
    QCOMPARE(page("window.profileRecoverySentinel").toInt(),47);
    key(Qt::Key_Colon);QTRY_COMPARE(text("mode"),QString("command"));
    key(Qt::Key_Escape);QTRY_COMPARE(text("mode"),QString("normal"));
    // Inherited windows must not resurrect a recovered startup failure from
    // the cached admission effect.
    QVERIFY(command("open --target window "+text("current_url")));
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
    QTRY_VERIFY(evaluate("testWindow.browserWindowRegistry[1].host.profileBootstrapComplete").toBool());
    QCOMPARE(evaluate("testWindow.browserWindowRegistry[1].ui.profile_startup_notice").toString(),QString());
    QCOMPARE(evaluate("testWindow.browserWindowRegistry[1].ui.profile_startup_details()").toString(),QString());
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry[1].ui.chrome_font_size_pt").toDouble(),12.0);
    QVERIFY(included.open(QIODevice::WriteOnly|QIODevice::Truncate));
    included.write("[ui]\nfont_size_pt=16\n# changed included settings\n");
    if (position=="bottom") included.write("[hints.selectors.all]\nmode='replace'\nselectors=['button[data-recovery]']\n");
    included.close();
    QTRY_COMPARE_WITH_TIMEOUT(evaluate("testWindow.browserUi.chrome_font_size_pt").toDouble(),16.0,8000);
    QTRY_COMPARE_WITH_TIMEOUT(evaluate("testWindow.browserWindowRegistry[1].ui.chrome_font_size_pt").toDouble(),16.0,8000);
    QCOMPARE(page("window.profileRecoverySentinel").toInt(),47);
    evaluate("testWindow.browserWindowRegistry[1].host.windowShutdownApproved=true;testWindow.browserWindowRegistry[1].host.close()");
}

void ApplicationTests::productPolishFailedProfileDiagnostics() {
    QTest::failOnWarning(QRegularExpression(".*(invalid context|TypeError|ReferenceError|Binding loop).*"));
    QPointer<QQuickWindow> child=qobject_cast<QQuickWindow *>(evaluate(
        "testWindow.browserWindowFactory.createObject(null,{windowProfileName:'',windowProfileLabel:'Failed profile'})").value<QObject *>());
    QVERIFY(child);engine->rootContext()->setContextProperty("failedStartupHost",child.data());
    QTRY_VERIFY(!evaluate("failedStartupHost.windowUi.profile_bootstrap_pending").toBool());
    QTRY_VERIFY(evaluate("failedStartupHost.windowUi.profile_startup_notice.length>0").toBool());
    QVERIFY(!evaluate("failedStartupHost.windowUi.profile_available()").toBool());
    QVERIFY(!evaluate("failedStartupHost.profileBootstrapComplete").toBool());
    QVERIFY(evaluate("testWindow.browserWindowEntryForUi(failedStartupHost.windowUi)===null").toBool());
    // An explicit host must match the requested QObject even for read-only
    // diagnostics; a failed startup is never authority for another window.
    evaluate("testWindow.showDiagnostics(testWindow.browserUi,failedStartupHost)");
    QVERIFY(!evaluate("testWindow.diagnosticsVisible").toBool());
    evaluate("testWindow.showDiagnostics(failedStartupHost.windowUi,testWindow)");
    QVERIFY(!evaluate("testWindow.diagnosticsVisible").toBool());
    auto action=findVisualItem(child->contentItem(),"profileHealthDetails");
    QVERIFY(action);QTRY_VERIFY(action->isVisible());
    child->requestActivate();QTRY_VERIFY(child->isActive());action->forceActiveFocus();
    QTest::keyClick(child,Qt::Key_Return);
    QTRY_VERIFY(evaluate("testWindow.diagnosticsVisible && testWindow.diagnosticsUi===failedStartupHost.windowUi").toBool());
    QTRY_VERIFY(evaluate("failedStartupHost.windowUi.diagnostics_available").toBool());
    const auto report=QJsonDocument::fromJson(evaluate("testWindow.diagnosticsText").toString().toUtf8()).object();
    QCOMPARE(report["profile_startup"].toObject()["active_profile"].toObject()["status"].toString(),QString("unavailable"));
    const auto artifacts=qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
    if (!artifacts.isEmpty()) {
        QDir().mkpath(artifacts);
        QVERIFY(child->grabWindow().save(artifacts+"/production-failed-profile-diagnostics.png"));
    }
    evaluate("failedStartupHost.windowShutdownApproved=true");QVERIFY(child->close());
    QTRY_VERIFY(!evaluate("testWindow.diagnosticsVisible").toBool());
    engine->rootContext()->setContextProperty("failedStartupHost",QVariant::fromValue<QObject *>(nullptr));
    child->deleteLater();QCoreApplication::sendPostedEvents(nullptr,QEvent::DeferredDelete);
    QTRY_VERIFY(child.isNull());
}

void ApplicationTests::productPolishDiagnosticOwners_data() {
    QTest::addColumn<QString>("kind");
    QTest::addColumn<QString>("position");
    QTest::newRow("secondary") << QString("secondary") << QString("top");
    QTest::newRow("private") << QString("private") << QString("top");
    QTest::newRow("popup") << QString("popup") << QString("top");
    QTest::newRow("secondary-bottom") << QString("secondary") << QString("bottom");
    QTest::newRow("private-left") << QString("private") << QString("left");
    QTest::newRow("popup-right") << QString("popup") << QString("right");
}

void ApplicationTests::productPolishDiagnosticOwners() {
    QTest::failOnWarning(QRegularExpression(".*(invalid context|TypeError|ReferenceError|Binding loop).*"));
    QFETCH(QString, kind);
    QFETCH(QString, position);
    const auto initial = text("current_url");
    QString owner;
    if (kind == "popup") {
        QVERIFY(command("bind p hint --target pointer links"));key(Qt::Key_P);
        typeText(hintLabel("Open popup"));key(Qt::Key_Return);
        QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(), 1);
        owner = "testWindow.popupWindowRegistry[0]";
    } else {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"+quoted(initial)
            +",windowProfileName:'diagnostic-owner',windowProfileLabel:'Diagnostic owner',windowPrivateProfile:"
            +(kind=="private" ? "true" : "false")+"})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
        owner = "testWindow.browserWindowRegistry[1]";
    }
    QTRY_VERIFY(evaluate(owner+".host.profileBootstrapComplete && !"+owner+".ui.profile_bootstrap_pending").toBool());
    QPointer<QQuickWindow> child = qobject_cast<QQuickWindow *>(evaluate(owner+".host").value<QObject *>());
    QVERIFY(child);
    pageReceiver = owner+".host.activeView";
    QTRY_VERIFY(!evaluate(pageReceiver+".loading").toBool());
    const auto documentUrl = kind=="popup" ? QUrl(initial).resolved(QUrl("/popup.html")).toString() : initial;
    QTRY_VERIFY(page("location.href === "+quoted(documentUrl)+" && document.readyState === 'complete'").toBool());
    QCOMPARE(page("window.diagnosticOwnerSentinel=83;window.diagnosticOwnerSentinel").toInt(),83);
    QVERIFY(evaluate(owner+".ui.set_runtime_setting('ui.font_size_pt','15',true)").toBool());
    QVERIFY(evaluate(owner+".ui.set_runtime_setting('ui.statusbar','never',true)").toBool());
    QVERIFY(evaluate(owner+".ui.set_runtime_setting('ui.tabs','always',true)").toBool());
    QVERIFY(evaluate(owner+".ui.set_runtime_setting('ui.tab_position',"+quoted(position)+",true)").toBool());
    evaluate(owner+".ui.configure_profile(false,false,'Failed replacement','',testWindow.storageBasePath)");
    QTRY_VERIFY(evaluate(owner+".ui.profile_startup_notice.indexOf('current profile is still active')>=0").toBool());
    auto notice=findVisualItem(child->contentItem(),"profileHealthNotice");
    QVERIFY(notice);QTRY_VERIFY(notice->isVisible());
    QTRY_VERIFY(evaluate("(()=>{var h="+owner+".host,v=h.activeView;return v.mapToItem(h.contentItem,0,v.height).y})()").toDouble()
        <= notice->mapToItem(child->contentItem(),QPointF()).y()+1);
    evaluate("testWindow.browserUi.record_request_resolution('certificate','resolved')");
    evaluate(owner+".ui.record_request_resolution('page-dialog','resolved')");
    QVERIFY2(evaluate(owner+".ui.execute_secondary_interactive_command('diagnostics')").toBool(),qPrintable(evaluate(owner+".ui.status_text").toString()));
    evaluate(owner+".host.processRuntimeWork()");
    QTRY_VERIFY(evaluate("testWindow.diagnosticsVisible && testWindow.diagnosticsUi === "+owner
        +".ui && testWindow.diagnosticsHost === "+owner+".host").toBool());
    auto close = findVisualItem(child->contentItem(), "diagnosticsClose");
    QVERIFY(close);QCOMPARE(close->window(), child.data());
    auto diagnostics=findVisualItem(child->contentItem(),"browserDiagnostics");
    QVERIFY(diagnostics);
    auto title=findVisualItem(diagnostics,"dialogTitleLabel");
    QVERIFY(title);
    QTRY_COMPARE(title->property("font").value<QFont>().pointSizeF(),child->property("font").value<QFont>().pointSizeF()+1);
    QVERIFY(evaluate("testWindow.diagnosticsAppearance === "+owner+".host.chromeAppearance").toBool());
    QTRY_VERIFY(evaluate(owner+".ui.diagnostics_available").toBool());
    QVERIFY(!evaluate("testWindow.browserUi.diagnostics_available").toBool());
    const auto preview = evaluate("testWindow.diagnosticsText").toString();
    const auto report = QJsonDocument::fromJson(preview.toUtf8()).object();
    const auto counts = report["request_resolutions"].toObject()["counts"].toArray();
    bool ownRecord=false;
    for (const auto &value : counts) {
        const auto row=value.toObject();
        QVERIFY(row["kind"] != "certificate");
        if (row["kind"]=="page-dialog") ownRecord=true;
    }
    QVERIFY(ownRecord);
    if (kind=="private") QVERIFY(evaluate(owner+".ui.diagnostics_summary.join(' ').indexOf('kept in memory')>=0").toBool());
    auto copy = findVisualItem(child->contentItem(), "diagnosticsCopy");
    QVERIFY(copy);QVERIFY(QMetaObject::invokeMethod(copy, "click"));
    QCOMPARE(QGuiApplication::clipboard()->text(), preview);
    QVERIFY(!evaluate("testWindow.copyNoticeVisible").toBool());
    QCOMPARE(evaluate("testWindow.diagnosticsExportNotice").toString(),QString("Report preview copied"));
    const auto artifacts=qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
    if (!artifacts.isEmpty()) {
        QDir().mkpath(artifacts);
        QVERIFY(child->grabWindow().save(artifacts+"/production-diagnostics-"+QString::fromLatin1(QTest::currentDataTag())+".png"));
    }
    auto save = findVisualItem(child->contentItem(), "diagnosticsSave");
    QVERIFY(save);QVERIFY(QMetaObject::invokeMethod(save, "click"));
    auto chooser = window->findChild<QObject *>("diagnosticsExportChooser");
    QVERIFY(chooser);QTRY_VERIFY(chooser->property("visible").toBool());
    QCOMPARE(chooser->property("parentWindow").value<QObject *>(), child.data());
    QVERIFY(QMetaObject::invokeMethod(chooser, "reject"));
    child->requestActivate();QTRY_VERIFY(child->isActive());
    QTest::keyClick(child, Qt::Key_Escape);
    QTRY_VERIFY(!evaluate("testWindow.diagnosticsVisible").toBool());
    QVERIFY(!evaluate(owner+".ui.diagnostics_available").toBool());
    QCOMPARE(page("window.diagnosticOwnerSentinel").toInt(),83);
    // Closing an owner cancels its pending review and reparents the shared
    // presentation before that window's native content is destroyed.
    auto noticeAction=findVisualItem(child->contentItem(),"profileHealthDetails");
    QVERIFY(noticeAction);noticeAction->forceActiveFocus();QTRY_VERIFY(noticeAction->hasActiveFocus());
    QTest::keyClick(child,Qt::Key_Return);
    QTRY_VERIFY(evaluate("testWindow.diagnosticsVisible && testWindow.diagnosticsUi === "+owner+".ui").toBool());
    evaluate(owner+".host.windowShutdownApproved=true");
    QVERIFY(child->close());
    QTRY_VERIFY(!evaluate("testWindow.diagnosticsVisible").toBool());
    child->deleteLater();QCoreApplication::sendPostedEvents(nullptr,QEvent::DeferredDelete);
    QTRY_VERIFY(child.isNull());
    QCOMPARE(text("current_url"), initial);
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
    const auto activateExportPicker = [&] {
        QFileDialog *picker = nullptr;
        for (auto widget : QApplication::topLevelWidgets())
            if (widget->isVisible() && widget->objectName() == "diagnosticsExportChooser")
                picker = qobject_cast<QFileDialog *>(widget);
        QVERIFY(picker);
        picker->activateWindow();
        QTRY_VERIFY(picker->isActiveWindow());
    };
    activateExportPicker();
    if (QTest::currentTestFailed()) return;
    // The owned widget chooser validates an existing save destination before
    // emitting acceptance. Drive its confirmation like a user, with a bound
    // on any nested dialog loop. Browser export still must refuse overwrite.
    bool overwriteQuestionSeen = false;
    bool confirmationTimedOut = false;
    QElapsedTimer confirmationDeadline;
    confirmationDeadline.start();
    QTimer confirmOverwrite;
    confirmOverwrite.setInterval(10);
    connect(&confirmOverwrite, &QTimer::timeout, this, [&] {
        for (auto widget : QApplication::topLevelWidgets()) {
            auto question = qobject_cast<QMessageBox *>(widget);
            if (!question || !question->isVisible()
                    || !question->parentWidget()
                    || question->parentWidget()->objectName() != "diagnosticsExportChooser") continue;
            if (auto yes = question->button(QMessageBox::Yes)) {
                overwriteQuestionSeen = true;
                QTest::mouseClick(yes, Qt::LeftButton);
                return;
            }
            question->reject();
            confirmationTimedOut = true;
        }
        if (confirmationDeadline.elapsed() > 3000) {
            confirmationTimedOut = true;
            for (auto widget : QApplication::topLevelWidgets())
                if (auto dialog = qobject_cast<QDialog *>(widget)) dialog->reject();
        }
    });
    confirmOverwrite.start();
    QVERIFY(QMetaObject::invokeMethod(chooser, "accept"));
    confirmOverwrite.stop();
    QVERIFY(!confirmationTimedOut);
    qInfo() << "Owned save overwrite confirmation observed" << overwriteQuestionSeen;
    QTRY_VERIFY(evaluate("testWindow.diagnosticsExportNotice.indexOf(\"The report couldn't be saved\") === 0").toBool());
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
    activateExportPicker();
    if (QTest::currentTestFailed()) return;
    QVERIFY(QMetaObject::invokeMethod(chooser, "accept"));
    QTRY_VERIFY(QFile::exists(path));
    QTRY_VERIFY(evaluate("testWindow.diagnosticsExportNotice.indexOf('Report saved') === 0").toBool());
    QVERIFY(!evaluate("testWindow.browserUi.diagnostics_available").toBool());
    QFile saved(path);
    QVERIFY(saved.open(QIODevice::ReadOnly));
    QCOMPARE(saved.readAll(), preview.toUtf8());
    QTRY_VERIFY(window->isActive());
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
    auto browseList = findVisualItem(window->contentItem(), "libraryEntryList");
    QVERIFY(browseList);
    key(Qt::Key_Backtab, Qt::ShiftModifier);
    QTRY_VERIFY(window->activeFocusItem() == browseList
        || window->activeFocusItem() == browseList->property("currentItem").value<QObject *>());
    QCOMPARE(input->property("text").toString(), QString("revised bookmark"));
    QVERIFY(input->isVisible());
    key(Qt::Key_Tab);
    QTRY_COMPARE(window->activeFocusItem(), input);
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
    QTRY_VERIFY(evaluate(entry + ".host.profileBootstrapComplete").toBool());
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

static QQuickItem *visibleCopyAgain(QQuickItem *item) {
    if (item->isVisible() && item->property("text").toString()=="Copy again") return item;
    for (auto child : item->childItems()) if (auto found=visibleCopyAgain(child)) return found;
    return nullptr;
}
void ApplicationTests::productPolishCopyFeedback_data() {
    QTest::addColumn<QString>("kind");QTest::addColumn<QString>("scenario");
    for (const auto kind : {"secondary","private","popup"}) {
        for (const auto scenario : {"placement","repeat-owner","unavailable-primary"})
            QTest::newRow((QByteArray(kind)+"-"+scenario).constData()) << QString(kind) << QString(scenario);
    }
}
void ApplicationTests::productPolishCopyFeedback() {
    QTest::failOnWarning(QRegularExpression(".*(invalid context|TypeError|ReferenceError|Binding loop).*"));
    QFETCH(QString,kind);QFETCH(QString,scenario);const bool repeat=scenario=="repeat-owner";QString entry;
    const auto initial=text("current_url");
    if (kind=="popup") {
        QVERIFY(command("bind p hint --target pointer links"));key(Qt::Key_P);
        typeText(hintLabel("Open popup"));key(Qt::Key_Return);
        QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(),1);
        entry="testWindow.popupWindowRegistry[0]";
    } else {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"+quoted(initial)
            +",windowProfileName:'copy-feedback-owner',windowProfileLabel:'Copy feedback',windowPrivateProfile:"
            +(kind=="private" ? "true" : "false")+"})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
        entry="testWindow.browserWindowRegistry[1]";
    }
    QTRY_VERIFY(evaluate(entry+".host.profileBootstrapComplete && !"+entry+".ui.profile_bootstrap_pending").toBool());
    auto child=qobject_cast<QQuickWindow *>(evaluate(entry+".host").value<QObject *>());QVERIFY(child);
    pageReceiver=entry+".host.activeView";
    QTRY_VERIFY(page("document.readyState==='complete' && document.getElementById('button')!==null").toBool());
    QVERIFY(evaluate(entry+".ui.set_runtime_setting('ui.font_size_pt','16',true)").toBool());
    QVERIFY(evaluate(entry+".ui.set_runtime_setting('ui.tabs','always',true)").toBool());
    const auto tabPosition=kind=="secondary" ? "bottom" : kind=="private" ? "left" : "right";
    QVERIFY(evaluate(entry+".ui.set_runtime_setting('ui.tab_position',"+quoted(tabPosition)+",true)").toBool());
    child->requestActivate();QTRY_VERIFY(child->isActive());
    const auto copied=QString("https://example.test/copied-")+kind;
    QVERIFY(evaluate("testWindow.copyToClipboard("+quoted(copied)+",false,false,"+entry+".ui)").toBool());
    QCOMPARE(QGuiApplication::clipboard()->text(),copied);
    QVERIFY(!child->grabWindow().isNull());
    if (!repeat) QTRY_VERIFY2_WITH_TIMEOUT(visibleCopyAgain(child->contentItem()),"Copy feedback must appear in the window that initiated it",1500);
    auto childAgain=visibleCopyAgain(child->contentItem());
    auto primaryAgain=visibleCopyAgain(window->contentItem());
    qInfo()<<"copy notice hosts"<<kind<<"primary"<<(primaryAgain!=nullptr)<<"child"<<(childAgain!=nullptr);
    const auto artifacts=qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
    if (!artifacts.isEmpty() && !repeat) {
        QDir().mkpath(artifacts);QVERIFY(window->grabWindow().save(artifacts+"/production-copy-feedback-root-"+kind+".png"));
        QVERIFY(child->grabWindow().save(artifacts+"/production-copy-feedback-child-"+kind+".png"));
    }
    if (scenario=="unavailable-primary") {
        if (QGuiApplication::clipboard()->supportsSelection()) QSKIP("Failure fixture needs an unavailable primary selection");
        QVERIFY(!evaluate("testWindow.copyToClipboard('must-not-fall-back',false,true,"+entry+".ui)").toBool());
        QCOMPARE(QGuiApplication::clipboard()->text(),copied);
        QVERIFY(!evaluate("testWindow.copyNoticeVisible").toBool());
        QVERIFY(evaluate("testWindow.copiedValue").toString().isEmpty());
        QCOMPARE(evaluate(entry+".ui.status_text").toString(),QString("Primary selection is unavailable"));
    } else if (!repeat) {
        QVERIFY2(childAgain,"Copy feedback must appear in the window that initiated it");
        QVERIFY2(!primaryAgain,"A child copy must not display its feedback in the primary window");
        QCOMPARE(childAgain->property("font").value<QFont>().pointSizeF(),evaluate(entry+".host.font.pointSize").toDouble());
        auto notice=findVisualItem(child->contentItem(),"copyNotice");QVERIFY(notice);
        auto view=qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());QVERIFY(view);
        const auto origin=notice->mapToItem(view,QPointF());
        QVERIFY(origin.x()>=-1 && origin.y()>=-1);
        QVERIFY(origin.x()+notice->width()<=view->width()+1);
        QVERIFY(origin.y()+notice->height()<=view->height()+1);
    } else {
        auto again=childAgain ? childAgain : primaryAgain;QVERIFY(again);
        auto noticeWindow=again->window();noticeWindow->requestActivate();QTRY_VERIFY(noticeWindow->isActive());
        evaluate("testWindow.browserUi.status_text='Title copy pending';"+entry+".ui.status_text='URL copy pending'");
        QGuiApplication::clipboard()->setText("repeat-sentinel");
        // Status/font/tab changes can move a bottom-anchored control. Click
        // the geometry of a rendered frame, as a person using the UI does.
        QVERIFY(!noticeWindow->grabWindow().isNull());
        QVERIFY(again->isVisible());
        const auto point=again->mapToScene(QPointF(again->width()/2,again->height()/2));
        QTest::mouseClick(noticeWindow,Qt::LeftButton,Qt::NoModifier,point.toPoint());
        qInfo()<<"copy repeat status owners"<<text("status_text")<<evaluate(entry+".ui.status_text");
        QCOMPARE(QGuiApplication::clipboard()->text(),copied);
        QCOMPARE(evaluate(entry+".ui.status_text").toString(),QString("URL copied"));
        QCOMPARE(text("status_text"),QString("Title copy pending"));
        page("window.copyFeedbackKeyups=[];document.addEventListener('keyup',e=>window.copyFeedbackKeyups.push(e.key));true");
        again->forceActiveFocus();QTRY_VERIFY(again->hasActiveFocus());
        if (kind=="private") { QTest::qWait(6100);QVERIFY(evaluate("testWindow.copyNoticeVisible").toBool()); }
        evaluate("testWindow.browserUi.status_text='Title copy pending';"+entry+".ui.status_text='URL copy pending'");
        QGuiApplication::clipboard()->setText("keyboard-repeat-sentinel");
        QTest::keyClick(noticeWindow,Qt::Key_Return);
        QCOMPARE(QGuiApplication::clipboard()->text(),copied);
        QCOMPARE(evaluate(entry+".ui.status_text").toString(),QString("URL copied"));
        QCOMPARE(text("status_text"),QString("Title copy pending"));
        QTest::keyClick(noticeWindow,Qt::Key_Escape);
        QTRY_VERIFY(!evaluate("testWindow.copyNoticeVisible").toBool());
        QTRY_VERIFY(evaluate("(function(){var item="+entry+".host.activeFocusItem;while(item && item!=="+pageReceiver+")item=item.parent;return !!item})()").toBool());
        QTest::qWait(50);
        QCOMPARE(page("window.copyFeedbackKeyups.filter(k=>k==='Enter'||k==='Escape').length").toInt(),0);
    }
    evaluate(entry+".host.windowShutdownApproved=true");QVERIFY(child->close());
    QTRY_VERIFY(!evaluate("testWindow.copyNoticeVisible").toBool());
    QTRY_VERIFY(evaluate("testWindow.copiedValue.length===0 && testWindow.copyNoticeUi===null").toBool());
    // Reparenting into an owner must not destroy the shared feedback surface
    // when that owner closes. A subsequent primary copy still has its control.
    window->requestActivate();QTRY_VERIFY(window->isActive());
    QVERIFY(evaluate("testWindow.copyToClipboard('primary-after-owner-close',false,false,testWindow.browserUi)").toBool());
    QTRY_VERIFY(visibleCopyAgain(window->contentItem()));
    QCOMPARE(QGuiApplication::clipboard()->text(),QString("primary-after-owner-close"));
}
