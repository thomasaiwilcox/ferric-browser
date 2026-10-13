#include "tst_application.h"
#include "../../src/url_display.h"
#include <QClipboard>
#include <QFile>
#include <QGuiApplication>
#include <QHostAddress>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQmlExpression>
#include <QQuickItem>
#include <QQuickWindow>
#include <QTcpSocket>
#include <QTemporaryDir>
#include <QtTest>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include <csignal>
#include <unistd.h>

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

void ApplicationTests::init() {
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
                const QByteArray body =
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
    const bool durable = QString::fromLatin1(QTest::currentTestFunction())
                             .startsWith("durablePersistence");
    QString startupConfig = "{\"schema_version\":3}";
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
    engine->setInitialProperties(
        {{"temporaryProfile", !durable},
         {"storageBasePath", directory.filePath("durable-test")},
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
    server.close();
    server.disconnect(this);
    window = nullptr;
    QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
    QTest::qWait(20);
}
void ApplicationTests::startupAndConfigPublication() {
    const auto oldConfig = text("config_json");
    QVERIFY(command("set --temp content.zoom=1.25"));
    QTRY_VERIFY(text("config_json") != oldConfig);
    const auto accepted = text("config_json");
    QVERIFY(!command("set --temp content.zoom=-2"));
    QCOMPARE(text("config_json"), accepted);
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
    QTest::qWait(100);
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
        !evaluate("testWindow.activeWebView().loading").toBool(), 15000);
    QTRY_VERIFY_WITH_TIMEOUT(
        page("document.getElementById('button') !== null").toBool(), 15000);
    QCOMPARE(page("window.clicks").toInt(), 0);
    QCOMPARE(page("window.allClicks || 0").toInt(), 0);
    QCOMPARE(text("mode"), QString("normal"));
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
