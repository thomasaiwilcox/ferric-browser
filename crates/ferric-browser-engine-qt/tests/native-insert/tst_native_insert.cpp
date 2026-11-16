#include "../../src/browser_key_router.h"
#include "../../src/browser_page_pointer_adapter.h"
#include "native_probe_input.h"
#include <QtCore/QFile>
#include <QtCore/QFileInfo>
#include <QtCore/QElapsedTimer>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonArray>
#include <QtCore/QPointer>
#include <QtCore/QScopeGuard>
#include <QtCore/QTemporaryDir>
#include <QtCore/QTimer>
#include <QtGui/QKeyEvent>
#include <QtGui/QMouseEvent>
#include <QtGui/QInputMethodEvent>
#include <QtGui/QInputMethod>
#include <QtQml/QQmlEngine>
#include <QtQml/QQmlContext>
#include <QtQuick/QQuickItem>
#include <QtQuick/QQuickView>
#include <QtTest/QSignalSpy>
#include <QtTest/QTest>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include <algorithm>
#include <memory>
#include <functional>

class NativeKeyTrace final : public QObject {
public:
    explicit NativeKeyTrace(QObject *target) : window(target) { QCoreApplication::instance()->installEventFilter(this); }
    ~NativeKeyTrace() override { QCoreApplication::instance()->removeEventFilter(this); }
    QObject *window;
    QVariantList samples;
    bool eventFilter(QObject *watched, QEvent *event) override {
        if(event->type()==QEvent::InputMethod && QGuiApplication::focusWindow()==window) {
            const auto *input=static_cast<QInputMethodEvent *>(event);
            samples.append(QVariantMap{{"type",int(event->type())},{"preedit",input->preeditString()},
                {"commit",input->commitString()}});
        }
        if (watched == window && (event->type() == QEvent::ShortcutOverride
            || event->type() == QEvent::KeyPress || event->type() == QEvent::KeyRelease)) {
            const auto *key = static_cast<QKeyEvent *>(event);
            samples.append(QVariantMap{{"type",int(event->type())},{"key",key->key()},
                {"scan",key->nativeScanCode()},{"symbol",key->nativeVirtualKey()},
                {"nativeModifiers",key->nativeModifiers()},{"text",key->text()},
                {"modifiers",int(key->modifiers())},{"repeat",key->isAutoRepeat()}});
        }
        return false;
    }
};

// Test-only held-pointer experiment. Production Ferric currently exposes only
// discrete pointer actions; this helper deliberately stays inside the native
// fixture and exercises QQuickWindow's public hit-testing path.
class DragProbeTransaction final : public QObject {
public:
    DragProbeTransaction(FerricPagePointerAdapter *adapter, QQuickItem *item,
                         QWindow *window, QObject *parent = nullptr)
        : QObject(parent), adapter_(adapter), item_(item), window_(window)
    {
        if (window_)
            window_->installEventFilter(this);
        if (QCoreApplication::instance())
            QCoreApplication::instance()->installEventFilter(this);
        watchdog_.setSingleShot(true);
        watchdog_.setInterval(1000);
        connect(&watchdog_, &QTimer::timeout, this, [this] { cancel(QStringLiteral("watchdog")); });
        if (adapter_) {
            connect(adapter_, &FerricPagePointerAdapter::surfaceChanged, this, [this] {
                if (active_ && (adapter_->surfaceSerial() != serial_
                                || adapter_->surfaceRevision() != revision_
                                || adapter_->inputBlocked()
                                || adapter_->targetItem() != item_
                                || adapter_->targetWindow() != window_))
                    cancel(QStringLiteral("surface-changed"));
            });
        }
    }

    ~DragProbeTransaction() override
    {
        if (window_)
            window_->removeEventFilter(this);
        if (QCoreApplication::instance())
            QCoreApplication::instance()->removeEventFilter(this);
    }

    bool begin(const QPointF &startItemPosition, const QPointF &destinationItemPosition)
    {
        if (active_ || !adapter_ || !item_ || !window_
            || adapter_->targetItem() != item_ || adapter_->targetWindow() != window_
            || item_->window() != window_ || !item_->isVisible() || !window_->isVisible())
            return false;
        const QRectF itemBounds(QPointF(0, 0), QSizeF(item_->width(), item_->height()));
        if (!itemBounds.contains(startItemPosition) || !itemBounds.contains(destinationItemPosition))
            return false;
        serial_ = adapter_->surfaceSerial();
        revision_ = adapter_->surfaceRevision();
        lastPosition_ = item_->mapToScene(startItemPosition);
        destination_ = item_->mapToScene(destinationItemPosition);
        if (!sendMouse(QEvent::MouseMove, lastPosition_, Qt::NoButton, Qt::NoButton))
            return false;
        lastPositionAtStart_ = lastPosition_;
        active_ = true;
        held_ = sendMouse(QEvent::MouseButtonPress, lastPosition_, Qt::LeftButton,
                          Qt::LeftButton);
        if (!held_) {
            active_ = false;
            return false;
        }
        watchdog_.start();
        return true;
    }

    bool moveStraight(int durationMs = 200)
    {
        if (!active_ || !item_)
            return false;
        for (int step = 1; step <= 12 && active_; ++step) {
            const qreal fraction = qreal(step) / 12.0;
            lastPosition_ = lastPositionAtStart_ + (destination_ - lastPositionAtStart_) * fraction;
            if (!sendMouse(QEvent::MouseMove, lastPosition_, Qt::NoButton, Qt::LeftButton))
                return false;
            QTest::qWait(durationMs / 12);
        }
        return active_;
    }

    bool finish()
    {
        if (!active_ || !held_)
            return false;
        watchdog_.stop();
        const bool sent = sameReceiver()
            && sendMouse(QEvent::MouseButtonRelease, lastPosition_, Qt::LeftButton, Qt::NoButton);
        held_ = false;
        active_ = false;
        return sent;
    }

    void cancel(const QString &reason)
    {
        if (!active_)
            return;
        cancelReason_ = reason;
        watchdog_.stop();
        if (held_ && sameReceiver()) {
            cleanupReleaseSent_ = sendMouse(QEvent::MouseButtonRelease, lastPosition_,
                                             Qt::LeftButton, Qt::NoButton);
        }
        held_ = false;
        active_ = false;
    }

    bool active() const { return active_; }
    bool held() const { return held_; }
    bool cleanupReleaseSent() const { return cleanupReleaseSent_; }
    QString cancelReason() const { return cancelReason_; }

protected:
    bool eventFilter(QObject *watched, QEvent *event) override
    {
        if ((watched == window_ || watched == QCoreApplication::instance())
            && event->type() == QEvent::KeyPress
            && QGuiApplication::focusWindow() == window_
            && static_cast<QKeyEvent *>(event)->key() == Qt::Key_Escape)
            cancel(QStringLiteral("escape"));
        return false;
    }

private:
    bool sameReceiver() const
    {
        return adapter_ && item_ && window_ && adapter_->targetItem() == item_
            && adapter_->targetWindow() == window_ && item_->window() == window_;
    }

    bool sendMouse(QEvent::Type type, const QPointF &position, Qt::MouseButton button,
                   Qt::MouseButtons buttons)
    {
        if (!window_)
            return false;
        const QPoint global = window_->mapToGlobal(position.toPoint());
        QMouseEvent event(type, position, position, global, button, buttons, Qt::NoModifier,
                          Qt::MouseEventSynthesizedByApplication);
        return QCoreApplication::sendEvent(window_.data(), &event);
    }

    QPointer<FerricPagePointerAdapter> adapter_;
    QPointer<QQuickItem> item_;
    QPointer<QWindow> window_;
    QTimer watchdog_;
    QPointF lastPosition_;
    QPointF lastPositionAtStart_;
    QPointF destination_;
    QString serial_;
    QString revision_;
    QString cancelReason_;
    bool active_ = false;
    bool held_ = false;
    bool cleanupReleaseSent_ = false;
};

class NativeKeyHook final : public QObject {
public:
    std::function<void(QKeyEvent *)> callback;
protected:
    bool eventFilter(QObject *watched, QEvent *event) override {
        const QPointer<QObject> alive(watched);
        if (callback && (event->type() == QEvent::KeyPress || event->type() == QEvent::KeyRelease))
            callback(static_cast<QKeyEvent *>(event));
        return alive.isNull();
    }
};

// Exercise the production application event filter on WebEngine's actual
// Quick focus proxy. No synthetic JS keyboard events or browser automation.
class NativeInsertTest : public QObject {
    Q_OBJECT
    QTemporaryDir files;
    QQuickView window;
    QQuickItem *view = nullptr;
    FerricBrowserKeyRouter router;
    int exits = 0;
    int navigations = 0;
    int passExits = 0;
    bool locked = false;
    bool semicolonPending = false;
    bool mixedPageKey = false;
    bool mixedBrowserKey = false;
    bool nativePromptActive = false;
    int mixedRouteCalls = 0;
    int browserOwnedRepeats = 0;
    int fixtureGeneration = 0;

    QVariant js(const QString &source) {
        // QML owns the asynchronous callback; expose its result as a property.
        view->setProperty("probeDone", false);
        QMetaObject::invokeMethod(view, "probe", Q_ARG(QVariant, source));
        QElapsedTimer elapsed;
        elapsed.start();
        while (!view->property("probeDone").toBool() && elapsed.elapsed() < 5000)
            QTest::qWait(5);
        if (!view->property("probeDone").toBool())
            qFatal("WebEngine JavaScript callback timed out");
        return view->property("probeResult");
    }
    QVariant browserJs(const QString &source) {
        view->setProperty("probeDone", false);
        QMetaObject::invokeMethod(view, "probeBrowser", Q_ARG(QVariant, source));
        QElapsedTimer elapsed;
        elapsed.start();
        while (!view->property("probeDone").toBool() && elapsed.elapsed() < 5000)
            QTest::qWait(5);
        if (!view->property("probeDone").toBool())
            qFatal("ApplicationWorld callback timed out");
        return view->property("probeResult");
    }
    QString productionBrowserScript() {
        QFile script(FERRIC_SCRIPT_PATH);
        if (!script.open(QIODevice::ReadOnly))
            qFatal("Production BrowserScripts resource unavailable");
        QFile interaction(QFileInfo(QStringLiteral(FERRIC_SCRIPT_PATH))
                              .absoluteDir()
                              .filePath(QStringLiteral("PageInteractionScripts.js")));
        if (!interaction.open(QIODevice::ReadOnly))
            qFatal("Production PageInteractionScripts resource unavailable");
        QString module = QString::fromUtf8(interaction.readAll());
        module.remove(".pragma library");
        module.remove(".import \"InteractionRootRuntime.js\" as InteractionRootRuntime");
        module.remove(".import \"InteractionWitnessRuntime.js\" as InteractionWitnessRuntime");
        const auto runtime = [](const QString &name) {
            QFile file(QFileInfo(QStringLiteral(FERRIC_SCRIPT_PATH)).absoluteDir().filePath(name+".js"));
            if (!file.open(QIODevice::ReadOnly)) qFatal("Production interaction runtime unavailable");
            QString source=QString::fromUtf8(file.readAll());source.remove(".pragma library");
            return "var "+name+"=(function(){"+source+";return {source:source};})();";
        };
        QString facade = QString::fromUtf8(script.readAll());
        facade.remove(".pragma library");
        facade.remove(".import \"PageInteractionScripts.js\" as PageInteractionScripts");
        const QString namespaceSource =
            QStringLiteral("var PageInteractionScripts=(function(){") + module
            + QStringLiteral(
                ";return {VERSION:VERSION,boundedCount:boundedCount,hintNativeAncestryLifecycle:hintNativeAncestryLifecycle,hintFrameHost:hintFrameHost,"
                "scrollTargetRuntime:scrollTargetRuntime,scroll:scroll,"
                "scrollTargetPolicy:scrollTargetPolicy,scrollTargetStatus:scrollTargetStatus,"
                "hintSelector:hintSelector,hintProxyRuntime:hintProxyRuntime,"
                "hintCollector:hintCollector,hintDirtyRevision:hintDirtyRevision,"
                "hintStopTracking:hintStopTracking,hintFresh:hintFresh,"
                "hintPointerPreflight:hintPointerPreflight,hintFocus:hintFocus,"
                "hintClick:hintClick,hintSetScrollTarget:hintSetScrollTarget};})() ;");
        return runtime("InteractionRootRuntime")+runtime("InteractionWitnessRuntime")+namespaceSource+facade;
    }
    QString browserScript(const QString &expression) {
        return js(productionBrowserScript() + ";" + expression).toString();
    }
    QVariant browserValueScript(const QString &expression) {
        return browserJs(productionBrowserScript() + ";eval(" + expression + ")");
    }
    void enter(bool persistent = false) {
        locked = persistent;
        view->setProperty("ferricMode", persistent ? "insert-locked" : "insert");
        view->setProperty("ferricEpoch", router.inputEpoch() + 1);
    }
    void focus(const QString &id) {
        js("document.getElementById('" + id + "').focus();true");
        view->forceActiveFocus();
    }
    void pass(bool once = true) {
        locked = false;
        view->setProperty("ferricMode", once ? "pass-through-once" : "pass-through");
        view->setProperty("ferricEpoch", router.inputEpoch() + 1);
    }
    void normal() {
        locked = false;
        view->setProperty("ferricMode", "normal");
        view->setProperty("ferricEpoch", router.inputEpoch() + 1);
    }
    void openPolicyTrace() {
        const int loaded = view->property("loaded").toInt();
        view->setProperty("url", QUrl::fromLocalFile(QStringLiteral(FERRIC_POLICY_FIXTURE_PATH)));
        QTRY_VERIFY(view->property("loaded").toInt() > loaded);
        QVERIFY(js("typeof policyFixture==='object'").toBool());
        focus("command-target");
        QVERIFY(js("policyFixture.clear();true").toBool());
    }
    void observePageReceiver() {
        QJSEngine scripts;
        const auto loaded = scripts.evaluate(productionBrowserScript());
        QVERIFY2(!loaded.isError(),qPrintable(loaded.toString()));
        const auto script = scripts.evaluate("focusObserverSource()+';'+focusProbe()");
        QVERIFY(!script.isError());
        QSignalSpy observed(&router,&FerricBrowserKeyRouter::pageReceiverObserved);
        router.probePageReceiver(view,script.toString(),router.inputEpoch());
        QTRY_COMPARE(observed.size(),1);
        QVERIFY2(!router.pageReceiverKey().isEmpty(),qPrintable(observed.first().at(3).toString()));
    }
    void ownedProbe(bool once = false) {
        openPolicyTrace();
        QVERIFY(router.configureFeasibilityMapping(view, true));
        pass(once);
    }
    void probeSource(int key, bool press, bool repeat = false, uint scan = 0,
                     Qt::KeyboardModifiers modifiers = Qt::NoModifier) {
        const QString text = key >= Qt::Key_A && key <= Qt::Key_Z
            ? QString(QChar(key)).toLower() : QString();
        QKeyEvent event(press ? QEvent::KeyPress : QEvent::KeyRelease,
                       key, modifiers, scan, text.isEmpty() ? 0 : text.front().unicode(),
                       0, text, repeat);
        QCoreApplication::sendEvent(&window, &event);
    }
private slots:
    void initTestCase() {
        QVERIFY(files.isValid());
        QFile html(files.filePath("fixture.html"));
        QVERIFY(html.open(QIODevice::WriteOnly));
        html.write(R"(<body tabindex="0" id="player"><div id="native-editor" class="ace_editor" role="textbox" aria-label="Native editor">
<div class="ace_gutter">1</div><textarea id="native-editor-receiver" class="ace_text-input">FIXTURE_PRIVATE_NATIVE_BUFFER</textarea></div>
<label id="native-label" for="native-check">Native checkbox</label><input id="native-check" type="checkbox" style="position:absolute;opacity:0;width:0;height:0;pointer-events:none">
<form onsubmit="event.preventDefault();window.submits++">
<input id="single"><button>submit</button></form><textarea id="multi"></textarea>
<input id="complete"><div id="rich" contenteditable="true"></div>
<input id="remove"><input id="navigate">
<script>
window.down=0;window.up=0;window.submits=0;window.shift=false;window.keyups=[];
window.nativeLabelClicks=0;window.nativeControlClicks=0;window.nativeControlChanges=0;
document.getElementById('native-label').addEventListener('click',()=>nativeLabelClicks++);
document.getElementById('native-check').addEventListener('click',()=>nativeControlClicks++);
document.getElementById('native-check').addEventListener('change',()=>nativeControlChanges++);
window.events=[];
for (const type of ['keydown','keyup']) document.addEventListener(type,e=>{
events.push({type,key:e.key,ctrl:e.ctrlKey,shift:e.shiftKey,repeat:e.repeat});});
document.addEventListener('keydown',e=>{if(e.key==='Enter'){down++;shift=e.shiftKey;
if(e.target.id==='complete'){e.preventDefault();e.target.value='accepted';}
if(e.target.id==='remove'){e.preventDefault();e.target.remove();}
if(e.target.id==='navigate'){e.preventDefault();location.href='next.html';}}});
document.addEventListener('keyup',e=>{keyups.push(e.key);if(e.key==='Enter')up++;});
</script>)");
        html.close();
        QFile next(files.filePath("next.html"));
        QVERIFY(next.open(QIODevice::WriteOnly));
        next.write("<input id='next'>");
        next.close();
        // Local PCM media, independent of codecs, network or a live service.
        QFile audio(files.filePath("silence.wav"));
        QVERIFY(audio.open(QIODevice::WriteOnly));
        QByteArray wav("RIFF\x24\x7d\x00\x00WAVEfmt \x10\x00\x00\x00"
                       "\x01\x00\x01\x00\x80\x3e\x00\x00\x00\x7d\x00\x00"
                       "\x02\x00\x10\x00" "data\x00\x7d\x00\x00", 44);
        wav.append(QByteArray(32000, '\0'));
        QCOMPARE(audio.write(wav), qint64(wav.size()));
        audio.close();
        QFile qml(files.filePath("view.qml"));
        QVERIFY(qml.open(QIODevice::WriteOnly));
        qml.write(R"(import QtQuick
import QtWebEngine
WebEngineView {
    id: fixtureView
    property string ferricMode: "normal"
    property int ferricEpoch: 0
    Binding { target: nativeRouter; property: "transientInsert"; value: fixtureView.ferricMode === "insert" }
    Binding { target: nativeRouter; property: "passNextKey"; value: fixtureView.ferricMode === "pass-through-once" }
    Binding { target: nativeRouter; property: "passThrough"; value: fixtureView.ferricMode === "pass-through" }
    Binding { target: nativeRouter; property: "inputEpoch"; value: fixtureView.ferricEpoch }
    width: 640; height: 480; focus: true
    property int focusNotifications: 0
    onJavaScriptConsoleMessage: function(level, message, line, source) {
        if (message === "__ferric_browser_focus_changed") focusNotifications++
    }
    property int loaded: 0
    onLoadingChanged: function(request) { if (request.status === WebEngineView.LoadSucceededStatus) loaded++ }
    property bool probeDone: false
    property var probeResult
    settings.playbackRequiresUserGesture: true
    property bool probeFullscreenEnabled: false
    settings.fullScreenSupportEnabled: probeFullscreenEnabled
    property int probeFullscreenRequests: 0
    onFullScreenRequested: function(request) {
        probeFullscreenRequests++
        request.accept()
    }
    function probeBrowser(source) {
        runJavaScript(source, WebEngineScript.ApplicationWorld, function(value) { probeResult = value; probeDone = true })
    }
    function probe(source) {
        runJavaScript(source, function(value) { probeResult = value; probeDone = true })
    }
})");
        qml.close();
        window.engine()->rootContext()->setContextProperty("nativeRouter", &router);
        window.setSource(QUrl::fromLocalFile(qml.fileName()));
        QCOMPARE(window.status(), QQuickView::Ready);
        view = window.rootObject();
        window.show();
        window.requestActivate();
        QTRY_COMPARE(QGuiApplication::focusWindow(), &window);
        router.setTargetWindow(&window);
        router.setEnabled(true);
        connect(&router, &FerricBrowserKeyRouter::transientEnterDispatched, this, [this](int epoch) {
            QCOMPARE(epoch, router.inputEpoch());
            ++exits;
            view->setProperty("ferricMode", "normal");
            view->setProperty("ferricEpoch", epoch + 1);
        });
        connect(&router, &FerricBrowserKeyRouter::passNextKeyDispatched, this, [this](int epoch) {
            QCOMPARE(epoch, router.inputEpoch());
            ++passExits;
            normal();
        });
        connect(&window, &QWindow::activeChanged, this, [this] {
            if (!window.isActive() && router.passNextKey())
                normal();
        });
        // Match the application's ownership check at ShortcutOverride too:
        // these fake Normal-mode V bindings take priority over native paste.
        connect(&router, &FerricBrowserKeyRouter::clipboardShortcutOverride, this,
                [this](int key, int modifiers) {
            if (view->property("ferricMode").toString() == "normal" && key == Qt::Key_V
                && (modifiers == Qt::ControlModifier
                    || modifiers == (Qt::ControlModifier | Qt::ShiftModifier)))
                router.acceptCurrentEvent();
        });
        connect(&router, &FerricBrowserKeyRouter::keyPressed, this,
                [this](const QString &, int key, int modifiers, bool, bool browserOwnedRepeat) {
            const auto mode = view->property("ferricMode").toString();
            if (key == Qt::Key_F && browserOwnedRepeat) {
                ++browserOwnedRepeats;
                router.acceptCurrentEvent();
                return;
            }
            // Mirrors FerricBrowserRuntimeSurface's early native-prompt
            // ownership guard: leave the original Qt event unaccepted.
            if (nativePromptActive)
                return;
            if (mode == "normal" && (key == Qt::Key_F || key == Qt::Key_G) && mixedPageKey) {
                ++mixedRouteCalls;
                router.pageOwnsCurrentEvent();
                return;
            }
            if (mode == "normal" && key == Qt::Key_F && mixedBrowserKey) {
                ++mixedRouteCalls;
                router.acceptCurrentEvent();
                return;
            }
            if (mode == "pass-through" || mode == "pass-through-once") {
                if (key == Qt::Key_Escape && modifiers == Qt::ControlModifier) {
                    normal();
                    router.acceptCurrentEvent();
                }
                return;
            }
            if (key == Qt::Key_Escape) {
                view->setProperty("ferricMode", "normal");
                locked = false;
                view->setProperty("ferricEpoch", router.inputEpoch() + 1);
                router.acceptCurrentEvent();
            } else if (mode == "normal" && key == Qt::Key_Semicolon && modifiers == 0) {
                semicolonPending = true;
                router.acceptCurrentEvent();
            } else if (mode == "normal" && key == Qt::Key_V
                       && (modifiers == (Qt::ControlModifier | Qt::ShiftModifier)
                           || modifiers == Qt::ControlModifier
                           || (semicolonPending && modifiers == 0))) {
                pass(modifiers != Qt::ControlModifier);
                semicolonPending = false;
                router.acceptCurrentEvent();
            } else if (!router.transientInsert() && !locked && key == Qt::Key_J) {
                ++navigations;
                router.acceptCurrentEvent();
            }
        });
    }
    void init() {
        QVERIFY(router.configureFeasibilityMapping(view, false));
        router.setFeasibilityInputBlocked(false);
        QCOMPARE(router.feasibilitySourceCount(), 0);
        exits = 0;
        navigations = 0;
        passExits = 0;
        locked = false;
        semicolonPending = false;
        mixedPageKey = false;
        mixedBrowserKey = false;
        nativePromptActive = false;
        mixedRouteCalls = 0;
        browserOwnedRepeats = 0;
        const int loaded = view->property("loaded").toInt();
        QUrl url = QUrl::fromLocalFile(files.filePath("fixture.html"));
        url.setQuery(QString::number(++fixtureGeneration));
        view->setProperty("url", url);
        QTRY_VERIFY(view->property("loaded").toInt() > loaded);
        QTRY_VERIFY(js("typeof window.down !== 'undefined'").toBool());
        js("down=0;up=0;submits=0;true");
        enter();
    }
    void canonicalDecisionText_data() {
        QTest::addColumn<int>("logicalKey");
        QTest::addColumn<quint32>("scan");
        QTest::addColumn<QString>("text");
        QTest::addColumn<QString>("expected");
        QTest::newRow("metadata-free-shift")<<int(Qt::Key_P)<<quint32(0)<<QString("p")<<QString("P");
        QTest::newRow("metadata-free-empty")<<int(Qt::Key_P)<<quint32(0)<<QString()<<QString("P");
        QTest::newRow("native-produced-lower")<<int(Qt::Key_P)<<quint32(33)<<QString("p")<<QString("p");
        QTest::newRow("native-produced-upper")<<int(Qt::Key_P)<<quint32(33)<<QString("P")<<QString("P");
        QTest::newRow("metadata-free-unicode")<<int(Qt::Key_P)<<quint32(0)<<QString::fromUtf8("é")<<QString::fromUtf8("é");
        QTest::newRow("native-unicode")<<int(Qt::Key_P)<<quint32(33)<<QString::fromUtf8("é")<<QString::fromUtf8("é");
        QTest::newRow("unicode-ascii-fold")<<int(Qt::Key_K)<<quint32(0)<<QString::fromUtf8("K")<<QString::fromUtf8("K");
    }
    void canonicalDecisionText() {
        QFETCH(int,logicalKey);QFETCH(quint32,scan);QFETCH(QString,text);QFETCH(QString,expected);
        normal();
        QObject owner;
        connect(&router,&FerricBrowserKeyRouter::keyPressed,&owner,[this]{router.acceptCurrentEvent();});
        QSignalSpy spy(&router,&FerricBrowserKeyRouter::keyPressed);
        QKeyEvent down(QEvent::KeyPress,logicalKey,Qt::ShiftModifier,scan,0,0,text);
        QKeyEvent up(QEvent::KeyRelease,logicalKey,Qt::ShiftModifier,scan,0,0,text);
        QCoreApplication::sendEvent(&window,&down);
        QCOMPARE(spy.size(),1);QCOMPARE(spy.first().at(0).toString(),expected);
        QCOMPARE(down.text(),text);QCOMPARE(down.nativeScanCode(),scan);
        QCoreApplication::sendEvent(&window,&up);
    }
    void extractedInteractionResourceFailsClosed() {
        const QString source = productionBrowserScript();
        const QVariantMap mismatch = browserJs(
            source + ";PageInteractionScripts.VERSION='0';eval(hintCollector('buttons'))")
                                         .toMap();
        QCOMPARE(mismatch.value("error").toString(), QString("script-version-mismatch"));

        const QVariantMap missing = browserJs(
            source + ";delete PageInteractionScripts.hintCollector;eval(hintCollector('buttons'))")
                                        .toMap();
        QCOMPARE(missing.value("error").toString(), QString("script-resource-unavailable"));

        const QVariantMap exception = browserJs(
            source
            + ";PageInteractionScripts.hintCollector=function(){throw new Error('fixture');};"
              "eval(hintCollector('buttons'))")
                                          .toMap();
        QCOMPARE(exception.value("error").toString(), QString("script-error"));
    }

    void nativeEditorProxyFocusKeepsInsertAndComposition() {
        const QVariant payload = browserValueScript("hintCollector('inputs',true)");
        const QVariantList candidates = payload.toMap().value("candidates").toList();
        QVariantMap selected;
        for (const QVariant &candidate : candidates) {
            const QVariantMap row = candidate.toMap();
            if (row.value("kind").toString() == "editor-focus"
                && row.value("text").toString() == "Native editor")
                selected = row;
        }
        QVERIFY(!selected.isEmpty());
        QVERIFY(selected.value("related_element_id").toUInt() > 0);
        QVERIFY(!QJsonDocument::fromVariant(payload).toJson().contains("FIXTURE_PRIVATE_NATIVE_BUFFER"));
        const QString selectedJson = QString::fromUtf8(
            QJsonDocument::fromVariant(selected).toJson(QJsonDocument::Compact));
        QVERIFY(browserValueScript("hintFocus(" + selectedJson + ")").toBool());
        QCOMPARE(js("document.activeElement.id").toString(), QString("native-editor-receiver"));

        QTest::keyClick(&window, Qt::Key_I);
        QTest::keyClick(&window, Qt::Key_I, Qt::ShiftModifier);
        auto *receiver = window.activeFocusItem();
        QVERIFY(receiver);
        QInputMethodEvent preedit("composing", {});
        QCoreApplication::sendEvent(receiver, &preedit);
        QInputMethodEvent commit;
        commit.setCommitString("confirmed");
        QCoreApplication::sendEvent(receiver, &commit);
        QTest::keyClick(&window, Qt::Key_Return, Qt::ShiftModifier);
        QTRY_VERIFY(js("document.getElementById('native-editor-receiver').value.includes('i')").toBool());
        QTRY_VERIFY(js("window.down===1 && window.shift===true").toBool());
        QCOMPARE(exits, 0);
        QVERIFY(router.transientInsert());
        QTest::keyClick(&window, Qt::Key_Return);
        QTRY_VERIFY(js("window.down===2 && window.shift===false").toBool());
        QCOMPARE(exits, 1);
        QVERIFY(!router.transientInsert());
    }
    void nativeLabelControlProxyUsesOneNativeActivation() {
        const QVariant payload = browserValueScript("hintCollector('inputs',true)");
        const QVariantList candidates = payload.toMap().value("candidates").toList();
        QVariantMap selected;
        for (const QVariant &candidate : candidates) {
            const QVariantMap row = candidate.toMap();
            if (row.value("kind").toString() == "label-control"
                && row.value("text").toString() == "Native checkbox")
                selected = row;
        }
        QVERIFY(!selected.isEmpty());
        const QString selectedJson = QString::fromUtf8(
            QJsonDocument::fromVariant(selected).toJson(QJsonDocument::Compact));
        QVERIFY(browserValueScript("hintClick(" + selectedJson + ")").toBool());
        QVERIFY(js("document.getElementById('native-check').checked").toBool());
        QCOMPARE(js("window.nativeLabelClicks").toInt(), 1);
        QCOMPARE(js("window.nativeControlClicks").toInt(), 1);
        QCOMPARE(js("window.nativeControlChanges").toInt(), 1);
    }
    void cleanup() {
        window.requestActivate();
        QTRY_COMPARE(QGuiApplication::focusWindow(), &window);
    }
    void passNext_data() {
        QTest::addColumn<int>("key");
        QTest::addColumn<int>("modifiers");
        QTest::addColumn<QString>("domKey");
        QTest::newRow("fullscreen-letter") << int(Qt::Key_F) << 0 << "f";
        QTest::newRow("escape") << int(Qt::Key_Escape) << 0 << "Escape";
        QTest::newRow("enter") << int(Qt::Key_Return) << 0 << "Enter";
        // QTest's text stays lowercase even with Shift; the page must receive
        // that original text and modifier combination unchanged.
        QTest::newRow("shift-letter") << int(Qt::Key_K) << int(Qt::ShiftModifier) << "k";
        QTest::newRow("ctrl-letter") << int(Qt::Key_F) << int(Qt::ControlModifier) << "f";
    }
    void sitePolicyBaselineNativeTrace_data() {
        QTest::addColumn<bool>("once");
        QTest::newRow("one-shot") << true;
        QTest::newRow("persistent") << false;
    }
    void sitePolicyBaselineNativeTrace() {
        QFETCH(bool, once);
        const int loaded = view->property("loaded").toInt();
        view->setProperty("url", QUrl::fromLocalFile(QStringLiteral(FERRIC_POLICY_FIXTURE_PATH)));
        QTRY_VERIFY(view->property("loaded").toInt() > loaded);
        QVERIFY(js("typeof policyFixture==='object'").toBool());
        focus("command-target");
        QVERIFY(js("policyFixture.clear(); true").toBool());
        pass(once);
        QTest::keyPress(&window, Qt::Key_P);
        QCOMPARE(passExits, 0);
        QTest::keyRelease(&window, Qt::Key_P);
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(), 2);
        QCOMPARE(js("policyFixture.snapshot().events[0].type").toString(), "keydown");
        QCOMPARE(js("policyFixture.snapshot().events[1].type").toString(), "keyup");
        QCOMPARE(js("policyFixture.snapshot().events[0].key").toString(), "p");
        QCOMPARE(js("policyFixture.snapshot().events[1].key").toString(), "p");
        QCOMPARE(js("policyFixture.snapshot().events[0].focus").toString(), "command-target");
        QCOMPARE(js("policyFixture.snapshot().activations").toInt(), 0);
        QCOMPARE(passExits, once ? 1 : 0);
        QCOMPARE(navigations, 0);
        // Retain actual code/trust/activation observations without treating
        // constructed Qt input as physical/compositor qualification.
        qInfo().noquote() << "SITE_POLICY_BASELINE_QT_TRACE"
                         << QJsonDocument::fromVariant(js("policyFixture.snapshot()"))
                                .toJson(QJsonDocument::Compact);
    }
    void sitePolicyNativeMetadataControls_data() {
        QTest::addColumn<uint>("scan");
        QTest::newRow("zero-native-metadata") << uint(0);
        QTest::newRow("incorrect-copied-source-p") << uint(33);
        QTest::newRow("explicit-virtual-k") << uint(45);
    }
    void sitePolicyNativeMetadataControls() {
        QFETCH(uint, scan);
        openPolicyTrace();
        pass(false);
        auto *receiver = window.activeFocusItem();
        QVERIFY(receiver && (receiver == view || view->isAncestorOf(receiver)));
        const uint symbol = scan == 0 ? 0 : scan == 33 ? 0x70 : 0x6b;
        QKeyEvent down(QEvent::KeyPress, Qt::Key_K, Qt::NoModifier,
                       scan, symbol, 0, QStringLiteral("k"));
        QKeyEvent up(QEvent::KeyRelease, Qt::Key_K, Qt::NoModifier,
                     scan, symbol, 0, QStringLiteral("k"));
        QCoreApplication::sendEvent(receiver, &down);
        QCoreApplication::sendEvent(receiver, &up);
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(), 2);
        const bool xkb = QGuiApplication::platformName() == QStringLiteral("xcb")
            || QGuiApplication::platformName() == QStringLiteral("wayland");
        const QString code = xkb && scan == 33 ? QStringLiteral("KeyP") : QStringLiteral("KeyK");
        QCOMPARE(js("policyFixture.snapshot().events[0].key").toString(), QStringLiteral("k"));
        QCOMPARE(js("policyFixture.snapshot().events[0].code").toString(), code);
        QCOMPARE(js("policyFixture.snapshot().events[1].code").toString(), code);
        QCOMPARE(js("policyFixture.snapshot().events[0].keyCode").toInt(), 75);
        QCOMPARE(js("policyFixture.snapshot().events[0].location").toInt(), 0);
        qInfo().noquote() << "P01_NATIVE_METADATA_CONTROL"
            << QGuiApplication::platformName() << scan
            << QJsonDocument::fromVariant(js("policyFixture.snapshot()"))
                   .toJson(QJsonDocument::Compact);
    }
    void pageReceiverBodyWithUnfocusedChild() {
        openPolicyTrace();pass(false);
        QVERIFY(js("var frame=document.createElement('iframe');frame.srcdoc='<button>Unfocused child</button>';document.body.append(frame);document.body.tabIndex=-1;document.body.focus();true").toBool());
        observePageReceiver();
        QVERIFY(!router.pageReceiverKey().isEmpty());
    }
    void compiledPageDestinations_data() {
        QTest::addColumn<QString>("token");
        QTest::addColumn<QString>("text");
        QTest::addColumn<QString>("name");
        QTest::addColumn<QString>("expectedKey");
        QTest::addColumn<int>("modifiers");
        for (ushort value = '!'; value <= '~'; ++value) {
            const QString text{QChar(value)};
            QTest::newRow(qPrintable(QStringLiteral("ascii-%1").arg(value)))
                << (value == '+' ? QStringLiteral("Plus") : text) << text << QString()
                << text << int(Qt::NoModifier);
        }
        const QStringList names = {"Space","Enter","Tab","Backtab","Backspace","Delete",
            "Insert","Escape","Left","Right","Up","Down","Home","End","PgUp","PgDown",
            "Back","Forward","F1","F2","F3","F4","F5","F6","F7","F8","F9","F10","F11","F12"};
        for (const auto &name : names) {
            QString key = name;
            if (name == "Space") key = " ";
            else if (name == "Backtab") key = "Tab";
            else if (name == "Left" || name == "Right" || name == "Up" || name == "Down") key = "Arrow" + name;
            else if (name == "PgUp") key = "PageUp";
            else if (name == "PgDown") key = "PageDown";
            else if (name == "Back") key = "BrowserBack";
            else if (name == "Forward") key = "BrowserForward";
            QTest::newRow(qPrintable(name)) << name << QString() << name << key << int(Qt::NoModifier);
        }
        QTest::newRow("ctrl-letter") << QString("Ctrl+k") << QString("k") << QString() << QString("k") << int(Qt::ControlModifier);
        QTest::newRow("ctrl-named") << QString("Ctrl+Left") << QString() << QString("Left") << QString("ArrowLeft") << int(Qt::ControlModifier);
        QTest::newRow("ctrl-shift-letter") << QString("Ctrl+Shift+k") << QString("k") << QString() << QString("K") << int(Qt::ControlModifier | Qt::ShiftModifier);
        QTest::newRow("alt-letter") << QString("Alt+k") << QString("k") << QString() << QString("k") << int(Qt::AltModifier);
        QTest::newRow("meta-letter") << QString("Meta+k") << QString("k") << QString() << QString("k") << int(Qt::MetaModifier);
        QTest::newRow("ctrl-alt-letter") << QString("Ctrl+Alt+k") << QString("k") << QString() << QString("k") << int(Qt::ControlModifier | Qt::AltModifier);
    }
    void compiledPageDestinations() {
        QFETCH(QString, token); QFETCH(QString, text); QFETCH(QString, name);
        QFETCH(QString, expectedKey); QFETCH(int, modifiers);
        const bool control = modifiers & Qt::ControlModifier;
        openPolicyTrace();
        pass(false);
        observePageReceiver();
        QVERIFY(js("document.addEventListener('keydown',e=>e.preventDefault());true").toBool());
        if(control)QVERIFY(js("window.ctrlFlags=0;window.ctrlTimeline=0;window.controlHeld=false;"
            "document.addEventListener('keydown',e=>{if(e.key==='Control')controlHeld=true;"
            "else {if(e.ctrlKey)ctrlFlags++;if(controlHeld)ctrlTimeline++;}});"
            "document.addEventListener('keyup',e=>{if(e.key==='Control')controlHeld=false;});true").toBool());
        const QVariantMap descriptor{{"text",text},{"named",name},{"ctrl",control},
            {"alt",bool(modifiers & Qt::AltModifier)},{"meta",bool(modifiers & Qt::MetaModifier)},
            {"shift",bool(modifiers & Qt::ShiftModifier)}};
        QObject policy;
        QString result;
        int readmissions = 0;
        connect(&router, &FerricBrowserKeyRouter::keyPressed, &policy,
            [&](const QString &, int key, int, bool, bool) {
                if (key != Qt::Key_P) { ++readmissions; return; }
                result = router.submitCurrentPageCommand(view, token, descriptor, true, nullptr, {}, router.pageReceiverKey());
            });
        probeSource(Qt::Key_P, true, false, 33);
        QCOMPARE(result, QString("owned"));
        probeSource(Qt::Key_P, true, true, 33);
        probeSource(Qt::Key_P, false, false, 33);
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(), 3);
        const QString expectedKeys = QStringList{expectedKey,expectedKey,expectedKey}.join('|');
        QCOMPARE(js("policyFixture.snapshot().events.map(e=>e.key).join('|')").toString(), expectedKeys);
        QCOMPARE(js("policyFixture.snapshot().events.map(e=>e.repeat).join(',')").toString(), "false,true,false");
        QCOMPARE(js("policyFixture.snapshot().events.map(e=>e.ctrl).join(',')").toString(),
                 control ? "true,true,true" : "false,false,false");
        QCOMPARE(js("policyFixture.snapshot().events[0].alt").toBool(), bool(modifiers & Qt::AltModifier));
        QCOMPARE(js("policyFixture.snapshot().events[0].meta").toBool(), bool(modifiers & Qt::MetaModifier));
        if (modifiers) QCOMPARE(js("policyFixture.snapshot().events[0].shift").toBool(), bool(modifiers & Qt::ShiftModifier));
        QString expectedCode;
        if (!text.isEmpty()) {
            const QChar character = text.front();
            if (character.toLower() >= QLatin1Char('a') && character.toLower() <= QLatin1Char('z'))
                expectedCode = "Key" + QString(character.toUpper());
            else {
                const QString digits = "1234567890";
                int digit = digits.indexOf(character);
                if (digit < 0) digit = QString("!@#$%^&*()").indexOf(character);
                if (digit >= 0) expectedCode = "Digit" + QString(digits[digit]);
                else {
                    const QStringList codes = {"Backquote","Minus","Equal","BracketLeft","BracketRight",
                        "Backslash","Semicolon","Quote","Comma","Period","Slash"};
                    int index = QString("`-=[]\\;',./").indexOf(character);
                    if (index < 0) index = QString("~_+{}|:\"<>?").indexOf(character);
                    QVERIFY(index >= 0); expectedCode = codes[index];
                }
            }
        } else if (name == "Backtab") expectedCode = "Tab";
        else if (name == "PgUp") expectedCode = "PageUp";
        else if (name == "PgDown") expectedCode = "PageDown";
        else if (name == "Left" || name == "Right" || name == "Up" || name == "Down") expectedCode = "Arrow" + name;
        else if (name == "Back" || name == "Forward") expectedCode = "Browser" + name;
        else expectedCode = name;
        // Offscreen uses Qt's logical fallback, which has no physical code for
        // browser navigation keys. The native backend must supply Browser*.
        if (QGuiApplication::platformName() == "offscreen" && (name == "Back" || name == "Forward"))
            expectedCode.clear();
        for (int index = 0; index < 3; ++index) {
            const QString event = QStringLiteral("policyFixture.snapshot().events[%1]").arg(index);
            QCOMPARE(js(event + ".code").toString(), expectedCode);
            QCOMPARE(js(event + ".location").toInt(), 0);
            QCOMPARE(js(event + ".which").toInt(), js(event + ".keyCode").toInt());
        }
        QCOMPARE(router.feasibilitySourceCount(), 0);
        QCOMPARE(router.feasibilityOutputCount(), 0);
        QCOMPARE(readmissions, 0);
        if(control) {
            QCOMPARE(js("ctrlFlags").toInt(),2);
            QCOMPARE(js("ctrlTimeline").toInt(),0);
            QVERIFY(!js("controlHeld").toBool());
        }
        qInfo().noquote() << "P10_COMPILED_DESTINATION" << token
            << QJsonDocument::fromVariant(js("policyFixture.snapshot()")).toJson(QJsonDocument::Compact);
    }
    void compiledOutputChannels_data() {
        QTest::addColumn<QString>("destination");
        QTest::addColumn<bool>("nativeFirst");
        QTest::addColumn<bool>("twoMappings");
        QTest::addColumn<bool>("independent");
        for (const auto &destination : {QString("k"),QString("Left"),QString("Ctrl+k")})
            for (bool nativeFirst : {false,true})
                for (bool twoMappings : {false,true})
                    for (bool independent : {false,true})
                        QTest::newRow(qPrintable(QString("%1-first%2-maps%3-independent%4")
                            .arg(destination).arg(nativeFirst).arg(twoMappings).arg(independent)))
                            << destination << nativeFirst << twoMappings << independent;
    }
    void compiledOutputChannels() {
        QFETCH(QString,destination); QFETCH(bool,nativeFirst);
        QFETCH(bool,twoMappings); QFETCH(bool,independent);
        openPolicyTrace(); pass(false); observePageReceiver();
        QVERIFY(js("document.addEventListener('keydown',e=>e.preventDefault());true").toBool());
        const bool named = destination == "Left";
        const bool control = destination == "Ctrl+k";
        const QVariantMap descriptor{{"text",named ? "" : "k"},{"named",named ? "Left" : ""},
            {"ctrl",control},{"alt",false},{"meta",false},{"shift",false}};
        const QVariantMap other{{"text","z"},{"named",""},{"ctrl",false},
            {"alt",false},{"meta",false},{"shift",false}};
        const int nativeKey = named ? Qt::Key_Left : Qt::Key_K;
        const int secondKey = twoMappings ? Qt::Key_Q : nativeKey;
        const auto nativeModifiers = control ? Qt::ControlModifier : Qt::NoModifier;
        QStringList results;
        QObject policy;
        connect(&router,&FerricBrowserKeyRouter::keyPressed,&policy,
            [&](const QString &,int key,int,bool,bool) {
                if (key == Qt::Key_P || (twoMappings && key == Qt::Key_Q)) {
                    const bool different = independent && key == Qt::Key_Q;
                    results << router.submitCurrentPageCommand(view,different ? "z" : destination,
                        different ? other : descriptor,true,nullptr,{},router.pageReceiverKey());
                } else if (!twoMappings && key == nativeKey) {
                    const bool suppressed = router.observeCurrentPageKey(view,destination);
                    results << (suppressed ? "collision" : "native");
                }
            });
        const auto sendSecond = [&](bool press, bool repeat = false) {
            probeSource(secondKey,press,repeat,0,twoMappings ? Qt::NoModifier : nativeModifiers);
        };
        if (nativeFirst) { sendSecond(true); probeSource(Qt::Key_P,true); }
        else { probeSource(Qt::Key_P,true); sendSecond(true); }
        const bool separate = independent && twoMappings;
        QCOMPARE(router.feasibilityOutputCount(), separate ? 2 : 1);
        // Release the winner before repeating the loser: no promotion/replay.
        if (nativeFirst) { sendSecond(false); probeSource(Qt::Key_P,true,true); probeSource(Qt::Key_P,false); }
        else { probeSource(Qt::Key_P,false); sendSecond(true,true); sendSecond(false); }
        QTRY_COMPARE(js("policyFixture.snapshot().events.filter(e=>e.type==='keyup').length").toInt(), separate ? 2 : 1);
        QCOMPARE(js("policyFixture.snapshot().events.length").toInt(), separate ? 5 : 2);
        QCOMPARE(router.feasibilitySourceCount(),0);
        QCOMPARE(router.feasibilityOutputCount(),0);
        QVERIFY(results.contains("owned") || results.contains("native"));
    }
    void sitePolicyNativeMappingProbe_data() {
        QTest::addColumn<uint>("sourceScan");
        QTest::newRow("qt-source") << uint(0);
        QTest::newRow("native-metadata-source") << uint(33);
    }
    void sitePolicyNativeMappingProbe() {
        QFETCH(uint, sourceScan);
        openPolicyTrace();
        pass(false);
        // Temporary probe policy lives only in this fixture. Submission and
        // asynchronous-output provenance use the production native adapter.
        QObject probeConfiguration;
        bool downSubmitted = false, upSubmitted = false;
        int destinationReadmissions = 0;
        connect(&router, &FerricBrowserKeyRouter::keyPressed, &probeConfiguration,
                [&](const QString &, int key, int modifiers, bool repeat, bool) {
            if (key == Qt::Key_K) ++destinationReadmissions;
            if (key != Qt::Key_P || modifiers != Qt::NoModifier) return;
            downSubmitted = router.submitFeasibilityPageKey(view, true, repeat);
            router.acceptCurrentEvent();
        });
        connect(&router, &FerricBrowserKeyRouter::keyReleased, &probeConfiguration,
                [&](int key, int) {
            if (key == Qt::Key_P)
                upSubmitted = router.submitFeasibilityPageKey(view, false);
        });
        const auto returnedBefore = router.feasibilityReturnedEvents();
        QKeyEvent down(QEvent::KeyPress, Qt::Key_P, Qt::NoModifier,
                       sourceScan, sourceScan ? 0x70 : 0, 0, QStringLiteral("p"));
        QKeyEvent up(QEvent::KeyRelease, Qt::Key_P, Qt::NoModifier,
                     sourceScan, sourceScan ? 0x70 : 0, 0, QStringLiteral("p"));
        QCoreApplication::sendEvent(&window, &down);
        QVERIFY(downSubmitted);
        QCoreApplication::sendEvent(&window, &up);
        QVERIFY(upSubmitted);
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(), 2);
        for (int index = 0; index != 2; ++index) {
            const QString event = QStringLiteral("policyFixture.snapshot().events[%1]").arg(index);
            QCOMPARE(js(event + ".key").toString(), QStringLiteral("k"));
            QCOMPARE(js(event + ".code").toString(), QStringLiteral("KeyK"));
            QCOMPARE(js(event + ".logicalText").toString(), QStringLiteral("k"));
            QCOMPARE(js(event + ".keyCode").toInt(), 75);
            QCOMPARE(js(event + ".location").toInt(), 0);
            QVERIFY(!js(event + ".repeat||" + event + ".ctrl||" + event + ".shift||"
                        + event + ".alt||" + event + ".meta").toBool());
        }
        QCOMPARE(js("policyFixture.snapshot().events[0].type").toString(), QStringLiteral("keydown"));
        QCOMPARE(js("policyFixture.snapshot().events[1].type").toString(), QStringLiteral("keyup"));
        // The page deliberately leaves K unhandled. Observe the actual
        // asynchronous return instead of relying on an arbitrary delay.
        QTRY_VERIFY(router.feasibilityReturnedEvents() > returnedBefore);
        QCOMPARE(destinationReadmissions, 0);
        QCOMPARE(navigations, 0);
        QCOMPARE(passExits, 0);
        qInfo().noquote() << "P01_NATIVE_MAPPING_QT_TRACE"
            << QGuiApplication::platformName() << sourceScan
            << QJsonDocument::fromVariant(js("policyFixture.snapshot()"))
                   .toJson(QJsonDocument::Compact);
    }
    void sitePolicyNativeOwnedGesture_data() {
        QTest::addColumn<bool>("once");
        QTest::newRow("one-shot") << true;
        QTest::newRow("persistent") << false;
    }
    void sitePolicyNativeOwnedGesture() {
        QFETCH(bool, once);
        ownedProbe(once);
        const auto returns = router.feasibilityReturnedEvents();
        probeSource(Qt::Key_P, true);
        QCOMPARE(passExits, 0);
        QCOMPARE(router.feasibilitySourceCount(), 1);
        QCOMPARE(router.feasibilityOutputCount(), 1);
        probeSource(Qt::Key_P, false, true);
        probeSource(Qt::Key_P, true, true);
        probeSource(Qt::Key_P, false, true);
        probeSource(Qt::Key_P, true, true);
        QCOMPARE(passExits, 0);
        probeSource(Qt::Key_P, false);
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(), 4);
        QCOMPARE(js("policyFixture.snapshot().events.map(e=>e.key).join(',')").toString(), "k,k,k,k");
        QCOMPARE(js("policyFixture.snapshot().events.map(e=>e.repeat).join(',')").toString(), "false,true,true,false");
        QCOMPARE(js("policyFixture.snapshot().events.map(e=>e.type).join(',')").toString(), "keydown,keydown,keydown,keyup");
        QCOMPARE(passExits, once ? 1 : 0);
        QCOMPARE(router.feasibilitySourceCount(), 0);
        QCOMPARE(router.feasibilityOutputCount(), 0);
        QTRY_VERIFY(router.feasibilityReturnedEvents() > returns);
        QCOMPARE(navigations, 0);
    }
    void sitePolicyNativeCollision_data() {
        QTest::addColumn<bool>("mappedFirst");
        QTest::addColumn<bool>("winnerReleasedFirst");
        QTest::newRow("mapped-winner-first-up") << true << true;
        QTest::newRow("mapped-loser-first-up") << true << false;
        QTest::newRow("native-winner-first-up") << false << true;
        QTest::newRow("native-loser-first-up") << false << false;
    }
    void sitePolicyNativeCollision() {
        QFETCH(bool, mappedFirst);
        QFETCH(bool, winnerReleasedFirst);
        ownedProbe();
        const int winner = mappedFirst ? Qt::Key_P : Qt::Key_K;
        const int loser = mappedFirst ? Qt::Key_K : Qt::Key_P;
        probeSource(winner, true);
        probeSource(loser, true);
        probeSource(loser, true, true);
        QCOMPARE(router.feasibilitySourceCount(), 2);
        QCOMPARE(router.feasibilityOutputCount(), 1);
        QVERIFY(!router.requestFeasibilityRaw());
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(), 1);
        probeSource(winnerReleasedFirst ? winner : loser, false);
        if (winnerReleasedFirst) {
            QCOMPARE(router.feasibilityOutputCount(), 0);
            QVERIFY(!router.requestFeasibilityRaw());
            probeSource(loser, true, true);
        }
        probeSource(winnerReleasedFirst ? loser : winner, false);
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(), 2);
        QCOMPARE(js("policyFixture.snapshot().events.map(e=>e.key).join(',')").toString(), "k,k");
        QCOMPARE(js("policyFixture.snapshot().events[1].type").toString(), "keyup");
        QCOMPARE(router.feasibilitySourceCount(), 0);
        QCOMPARE(router.feasibilityOutputCount(), 0);
        QVERIFY(router.requestFeasibilityRaw());
    }
    void sitePolicyNativeUnchangedParity() {
        ownedProbe();
        probeSource(Qt::Key_K, true, false, 45);
        probeSource(Qt::Key_K, true, false, 46);
        QCOMPARE(router.feasibilitySourceCount(), 2);
        QCOMPARE(router.feasibilityOutputCount(), 1);
        probeSource(Qt::Key_K, false, false, 45);
        probeSource(Qt::Key_K, false, false, 46);
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(), 4);
        QCOMPARE(js("policyFixture.snapshot().events.map(e=>e.key).join(',')").toString(), "k,k,k,k");
        QCOMPARE(router.feasibilitySourceCount(), 0);
        QCOMPARE(router.feasibilityOutputCount(), 0);
    }
    void sitePolicyNativeOneShotOverlap() {
        ownedProbe(true);
        probeSource(Qt::Key_J, true);
        probeSource(Qt::Key_P, true);
        probeSource(Qt::Key_J, false);
        QCOMPARE(passExits, 1);
        QCOMPARE(view->property("ferricMode").toString(), "normal");
        QCOMPARE(router.feasibilitySourceCount(), 1);
        QCOMPARE(router.feasibilityOutputCount(), 1);
        probeSource(Qt::Key_P, true, true);
        probeSource(Qt::Key_P, false);
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(), 5);
        QCOMPARE(js("policyFixture.snapshot().events.map(e=>e.key).join(',')").toString(), "j,k,j,k,k");
        QCOMPARE(passExits, 1);
        QCOMPARE(navigations, 0);
        QCOMPARE(router.feasibilitySourceCount(), 0);
        QCOMPARE(router.feasibilityOutputCount(), 0);
    }
    void compiledSourceStopsOnPreedit() {
        openPolicyTrace();pass(false);observePageReceiver();
        const QVariantMap destination{{"text","k"},{"named",""},{"ctrl",false},
            {"alt",false},{"meta",false},{"shift",false}};
        QObject policy;
        connect(&router,&FerricBrowserKeyRouter::keyPressed,&policy,
            [&](const QString &,int key,int,bool,bool) {
                if(key==Qt::Key_P)router.submitCurrentPageCommand(view,"k",destination,true,nullptr,{},router.pageReceiverKey());
            });
        probeSource(Qt::Key_P,true,false,33);
        const auto release=qScopeGuard([&]{probeSource(Qt::Key_P,false,false,33);});
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(),1);
        const QPointer<QQuickItem> receiver(window.activeFocusItem());QVERIFY(receiver);
        const auto finishComposition=qScopeGuard([&]{if(receiver){QInputMethodEvent commit;QCoreApplication::sendEvent(receiver,&commit);}});
        QInputMethodEvent preedit("composing",{});QCoreApplication::sendEvent(receiver,&preedit);
        QVERIFY(router.compositionActive());
        QCOMPARE(router.feasibilityOutputCount(),0);
        QCOMPARE(router.feasibilitySourceCount(),1);
        probeSource(Qt::Key_P,true,true,33);
        QTRY_COMPARE(js("policyFixture.snapshot().events.filter(e=>e.key==='k'&&e.type==='keyup').length").toInt(),1);
        QCOMPARE(js("policyFixture.snapshot().events.filter(e=>e.key==='k'&&e.type==='keydown').length").toInt(),1);
    }
    void overlappingEpochFocusRefresh_data() {
        QTest::addColumn<bool>("child");
        QTest::addColumn<int>("requests");
        for (bool child : {false,true})
            for (int requests : {2,8})
                QTest::newRow(qPrintable(QString("child%1-requests%2").arg(child).arg(requests))) << child << requests;
    }
    void overlappingEpochFocusRefresh() {
        QFETCH(bool,child); QFETCH(int,requests);
        openPolicyTrace(); pass(false);
        if (child) {
            QVERIFY(js("var epochFrame=document.createElement('iframe');epochFrame.srcdoc='<button id=receiver>Child</button>';document.body.append(epochFrame);true").toBool());
            QTRY_VERIFY(js("!!epochFrame.contentDocument&&!!epochFrame.contentDocument.getElementById('receiver')").toBool());
            QVERIFY(js("epochFrame.contentDocument.getElementById('receiver').focus();true").toBool());
        }
        QJSEngine scripts;
        QVERIFY(!scripts.evaluate(productionBrowserScript()).isError());
        const auto script=scripts.evaluate("focusObserverSource()+';'+focusProbe()").toString();
        QSignalSpy observed(&router,&FerricBrowserKeyRouter::pageReceiverObserved);
        for (int index=0;index<requests;++index) {
            router.setInputEpoch(router.inputEpoch()+1);
            router.probePageReceiver(view,script,router.inputEpoch());
        }
        QTRY_COMPARE(observed.size(),1);
        QCOMPARE(observed.first().at(1).toInt(),router.inputEpoch());
        QVERIFY2(!router.pageReceiverKey().isEmpty(),qPrintable(observed.first().at(3).toString()));
        const auto facts=QJsonDocument::fromJson(observed.first().at(3).toString().toUtf8()).object();
        QCOMPARE(facts.value("receiver").toObject().value("top").toBool(),!child);
    }
    void overlappingFocusRefreshKeepsHeldOutput() {
        openPolicyTrace(); pass(false); observePageReceiver();
        const QVariantMap destination{{"text","k"},{"named",""},{"ctrl",false},
            {"alt",false},{"meta",false},{"shift",false}};
        QObject policy;
        connect(&router,&FerricBrowserKeyRouter::keyPressed,&policy,
            [&](const QString &,int key,int,bool,bool) {
                if (key == Qt::Key_P)
                    router.submitCurrentPageCommand(view,"k",destination,true,nullptr,{},router.pageReceiverKey());
            });
        probeSource(Qt::Key_P,true,false,33);
        const auto release=qScopeGuard([&]{probeSource(Qt::Key_P,false,false,33);});
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(),1);
        QJSEngine scripts;
        QVERIFY(!scripts.evaluate(productionBrowserScript()).isError());
        const auto script=scripts.evaluate("focusProbe()").toString();
        QSignalSpy observed(&router,&FerricBrowserKeyRouter::pageReceiverObserved);
        router.probePageReceiver(view,script,router.inputEpoch());
        router.probePageReceiver(view,script,router.inputEpoch());
        QTRY_COMPARE(observed.size(),2);
        QCOMPARE(router.feasibilityOutputCount(),1);
        QVERIFY(!router.pageReceiverKey().isEmpty());
        for (const auto &arguments:observed)
            QVERIFY(!QJsonDocument::fromJson(arguments.at(3).toString().toUtf8()).object().value("state").isNull());
    }
    void compiledPolicyPublicationCancelsHeldSource() {
        openPolicyTrace(); pass(false); observePageReceiver();
        router.setPolicyContext("original-policy");
        const QVariantMap destination{{"text","k"},{"named",""},{"ctrl",false},
            {"alt",false},{"meta",false},{"shift",false}};
        QObject policy;
        QString result;
        connect(&router,&FerricBrowserKeyRouter::keyPressed,&policy,
            [&](const QString &,int key,int,bool,bool) {
                if (key == Qt::Key_P)
                    result = router.submitCurrentPageCommand(view,"k",destination,true,nullptr,{},router.pageReceiverKey());
            });
        probeSource(Qt::Key_P,true,false,33);
        QCOMPARE(result,QString("owned"));
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(),1);
        router.setPolicyContext("replacement-policy");
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(),2);
        QCOMPARE(js("policyFixture.snapshot().events[1].type").toString(),QString("keyup"));
        QCOMPARE(router.feasibilityOutputCount(),0);
        QVERIFY(!router.canEnterRawPassThrough()); // source remains held
        probeSource(Qt::Key_P,true,true,33);
        probeSource(Qt::Key_P,false,false,33);
        QCOMPARE(router.feasibilitySourceCount(),0);
        QCOMPARE(js("policyFixture.snapshot().events.length").toInt(),2);
        QVERIFY(router.canEnterRawPassThrough());
        probeSource(Qt::Key_P,true,false,33);
        probeSource(Qt::Key_P,false,false,33);
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(),4);
    }
    void mappedEscapePreservesProtectedRecovery_data() {
        QTest::addColumn<int>("recoveryModifiers");
        QTest::newRow("escape") << int(Qt::ControlModifier);
        QTest::newRow("capture") << int(Qt::ControlModifier | Qt::ShiftModifier);
    }
    void mappedEscapePreservesProtectedRecovery() {
        QFETCH(int, recoveryModifiers);
        openPolicyTrace(); pass(false); observePageReceiver();
        const QVariantMap destination{{"text","k"},{"named",""},{"ctrl",false},
            {"alt",false},{"meta",false},{"shift",false}};
        QObject policy;
        int protectedPresses = 0;
        QString result;
        connect(&router,&FerricBrowserKeyRouter::keyPressed,&policy,
            [&](const QString &,int key,int modifiers,bool repeat,bool) {
                if (key != Qt::Key_Escape || repeat) return;
                if (modifiers == Qt::NoModifier)
                    result = router.submitCurrentPageCommand(view,"k",destination,true,nullptr,{},router.pageReceiverKey());
                else if (modifiers == recoveryModifiers) {
                    ++protectedPresses;
                    router.acceptCurrentEvent();
                }
            });
        constexpr quint32 escapeScan = 9;
        const auto releaseSource = qScopeGuard([&] {
            if (!router.feasibilitySourceCount()) return;
            router.beginFeasibilityRecovery((quint64(1) << 63) | escapeScan);
            probeSource(Qt::Key_Escape,true,false,escapeScan);
            probeSource(Qt::Key_Escape,false,false,escapeScan);
        });
        probeSource(Qt::Key_Escape,true,false,escapeScan);
        QCOMPARE(result,QString("owned"));
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(),1);
        router.setEnabled(false); router.setEnabled(true); // original up was lost
        probeSource(Qt::Key_Escape,true,false,escapeScan,Qt::KeyboardModifiers(recoveryModifiers));
        probeSource(Qt::Key_Escape,false,false,escapeScan,Qt::KeyboardModifiers(recoveryModifiers));
        QCOMPARE(protectedPresses,1);
        QCOMPARE(js("policyFixture.snapshot().events.length").toInt(),1);
        QCOMPARE(router.feasibilitySourceCount(),0);
    }
    void productionRawAdmissionAndProtectedRecovery_data() {
        QTest::addColumn<bool>("interruptRecovery");
        QTest::newRow("physical-source-recovery") << false;
        QTest::newRow("capture-cancelled-on-deactivation") << true;
    }
    void productionRawAdmissionAndProtectedRecovery() {
        QFETCH(bool,interruptRecovery);
        openPolicyTrace(); pass(false); observePageReceiver();
        const QVariantMap destination{{"text","k"},{"named",""},{"ctrl",false},
            {"alt",false},{"meta",false},{"shift",false}};
        QObject policy;
        QString result, recovery;
        connect(&router,&FerricBrowserKeyRouter::keyPressed,&policy,
            [&](const QString &,int key,int modifiers,bool,bool) {
                if (key == Qt::Key_P) result = router.submitCurrentPageCommand(view,"k",destination,true,nullptr,{},router.pageReceiverKey());
                if (key == Qt::Key_Escape && modifiers == (Qt::ControlModifier|Qt::ShiftModifier)) {
                    normal(); recovery = router.beginPageKeyRecovery(); router.acceptCurrentEvent();
                }
            });
        QVERIFY(router.canEnterRawPassThrough());
        probeSource(Qt::Key_P,true,false,33);
        QCOMPARE(result,QString("owned"));
        QVERIFY(!router.canEnterRawPassThrough());
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(),1);
        // Deactivation loses the source's up; refocus does not arm raw.
        router.setEnabled(false); router.setEnabled(true);
        QCOMPARE(router.feasibilitySourceCount(),1);
        QCOMPARE(router.feasibilityOutputCount(),0);
        QVERIFY(!router.canEnterRawPassThrough());
        probeSource(Qt::Key_Escape,true,false,0,Qt::ControlModifier|Qt::ShiftModifier);
        probeSource(Qt::Key_Escape,false,false,0,Qt::ControlModifier|Qt::ShiftModifier);
        QVERIFY(recovery.startsWith("Press and release"));
        probeSource(Qt::Key_P,true,true,33);
        QVERIFY(!router.canEnterRawPassThrough()); // still held, no fresh down
        if (interruptRecovery) {
            router.setEnabled(false);router.setEnabled(true);
            probeSource(Qt::Key_Q,true,false,33,Qt::ShiftModifier);
            probeSource(Qt::Key_Q,false,false,33,Qt::ShiftModifier);
            QCOMPARE(router.feasibilitySourceCount(),1);
            QVERIFY(!router.canEnterRawPassThrough());
            probeSource(Qt::Key_Escape,true,false,0,Qt::ControlModifier|Qt::ShiftModifier);
            probeSource(Qt::Key_Escape,false,false,0,Qt::ControlModifier|Qt::ShiftModifier);
            QVERIFY(recovery.startsWith("Press and release"));
        }
        // A changed layout/case cannot hide the stored physical source.
        probeSource(Qt::Key_Q,true,false,33,Qt::ShiftModifier);
        QVERIFY(!router.canEnterRawPassThrough());
        probeSource(Qt::Key_Q,false,false,33,Qt::ShiftModifier);
        QCOMPARE(router.feasibilitySourceCount(),0);
        QVERIFY(router.canEnterRawPassThrough());
        QVERIFY(!router.feasibilityRaw());
        QCOMPARE(js("policyFixture.snapshot().events.length").toInt(),1);
    }
    void sitePolicyNativeRawSession() {
        ownedProbe();
        QVERIFY(router.requestFeasibilityRaw());
        probeSource(Qt::Key_P, true, false, 33);
        probeSource(Qt::Key_P, false, false, 33);
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(), 2);
        QCOMPARE(js("policyFixture.snapshot().events.map(e=>e.key).join(',')").toString(), "p,p");
        normal();
        pass(false);
        QVERIFY(!router.feasibilityRaw());
        js("policyFixture.clear();true");
        probeSource(Qt::Key_P, true);
        QVERIFY(!router.requestFeasibilityRaw());
        probeSource(Qt::Key_P, false);
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(), 2);
        QCOMPARE(js("policyFixture.snapshot().events.map(e=>e.key).join(',')").toString(), "k,k");
        QCOMPARE(router.feasibilitySourceCount(), 0);
    }
    void sitePolicyNativeCancellation_data() {
        QTest::addColumn<QString>("reason");
        QTest::addColumn<bool>("cleanup");
        QTest::newRow("prompt") << QString("prompt") << false;
        QTest::newRow("focus") << QString("focus") << false;
        QTest::newRow("disabled") << QString("disabled") << false;
        QTest::newRow("configuration") << QString("configuration") << true;
        QTest::newRow("modifiers") << QString("modifiers") << true;
    }
    void sitePolicyNativeCancellation() {
        QFETCH(QString, reason);
        QFETCH(bool, cleanup);
        ownedProbe();
        probeSource(Qt::Key_P, true);
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(), 1);
        QQuickItem chrome(window.contentItem());
        if (reason == "prompt") router.setFeasibilityInputBlocked(true);
        else if (reason == "focus") chrome.forceActiveFocus();
        else if (reason == "disabled") router.setEnabled(false);
        else if (reason == "configuration") QVERIFY(router.configureFeasibilityMapping(view, false));
        else probeSource(Qt::Key_P, true, true, 0, Qt::ShiftModifier);
        QCOMPARE(router.feasibilitySourceCount(), 1);
        QCOMPARE(router.feasibilityOutputCount(), 0);
        QVERIFY(!router.requestFeasibilityRaw());
        view->forceActiveFocus();
        router.setFeasibilityInputBlocked(false);
        router.setEnabled(true);
        probeSource(Qt::Key_P, true, true);
        probeSource(Qt::Key_P, false);
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(), cleanup ? 2 : 1);
        if (cleanup)
            QCOMPARE(js("policyFixture.snapshot().events[1].type").toString(), "keyup");
        QCOMPARE(router.feasibilitySourceCount(), 0);
        QCOMPARE(router.feasibilityOutputCount(), 0);
    }
    void sitePolicyNativeNavigationCancellation() {
        ownedProbe();
        probeSource(Qt::Key_P, true);
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(), 1);
        const int loaded = view->property("loaded").toInt();
        view->setProperty("url", QUrl::fromLocalFile(files.filePath("next.html")));
        QTRY_VERIFY(view->property("loaded").toInt() > loaded);
        js("window.replacementKeys=[];document.addEventListener('keyup',e=>replacementKeys.push(e.key));true");
        view->forceActiveFocus();
        QCOMPARE(router.feasibilityOutputCount(), 0);
        QVERIFY(!router.requestFeasibilityRaw());
        probeSource(Qt::Key_P, false);
        QCOMPARE(js("replacementKeys.length").toInt(), 0);
        QCOMPARE(router.feasibilitySourceCount(), 0);
    }
    void sitePolicyNativeLostReleaseRecovery() {
        ownedProbe();
        probeSource(Qt::Key_P, true, false, 33);
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(), 1);
        QQuickWindow other;
        other.show(); other.requestActivate();
        QTRY_COMPARE(QGuiApplication::focusWindow(), &other);
        window.requestActivate();
        QTRY_COMPARE(QGuiApplication::focusWindow(), &window);
        view->forceActiveFocus();
        QCOMPARE(router.feasibilityOutputCount(), 0);
        QVERIFY(!router.requestFeasibilityRaw());
        probeSource(Qt::Key_U, true, false, 33);
        probeSource(Qt::Key_U, false, false, 33);
        QCOMPARE(router.feasibilitySourceCount(), 1);
        QVERIFY(!router.requestFeasibilityRaw());
        QVERIFY(router.beginFeasibilityRecovery((quint64(1) << 63) | 33));
        // Simulate changed logical text with the same stored physical source.
        probeSource(Qt::Key_U, true, false, 33);
        probeSource(Qt::Key_U, false, true, 33);
        QCOMPARE(router.feasibilitySourceCount(), 1);
        probeSource(Qt::Key_U, false, false, 33);
        QCOMPARE(router.feasibilitySourceCount(), 0);
        QVERIFY(!router.feasibilityRaw());
        QCOMPARE(js("policyFixture.snapshot().events.length").toInt(), 1);
        QVERIFY(router.requestFeasibilityRaw());
        probeSource(Qt::Key_P, true, false, 33);
        probeSource(Qt::Key_P, false, false, 33);
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(), 3);
        QCOMPARE(js("policyFixture.snapshot().events.slice(1).map(e=>e.key).join(',')").toString(), "p,p");
    }
    void sitePolicyNativeBlockedOneShot() {
        ownedProbe(true);
        QVERIFY(QMetaObject::invokeMethod(&router, "invalidateFeasibilityDocument"));
        probeSource(Qt::Key_P, true);
        QCOMPARE(passExits, 0);
        QCOMPARE(router.feasibilitySourceCount(), 1);
        probeSource(Qt::Key_P, false);
        QCOMPARE(passExits, 1);
        QCOMPARE(router.feasibilitySourceCount(), 0);
        QCOMPARE(js("policyFixture.snapshot().events.length").toInt(), 0);
    }
    void compiledNativeLayoutInput_data() {
        QTest::addColumn<QString>("position");
        QTest::addColumn<QString>("sourceText");
        QTest::addColumn<QString>("code");
        QTest::addColumn<int>("modifiers");
        QTest::addColumn<bool>("caps");
        QTest::addColumn<bool>("num");
        QTest::addColumn<bool>("mapped");
        QTest::addColumn<bool>("dead");
        const auto row=[](const char *name,const QString &position,const QString &text,const QString &code,
            int modifiers=0,bool caps=false,bool num=false,bool mapped=false,bool dead=false) {
            QTest::newRow(name)<<position<<text<<code<<modifiers<<caps<<num<<mapped<<dead;
        };
        const auto layout=qEnvironmentVariable("FERRIC_NATIVE_PROBE_LAYOUT","us");
        row("mapped-p","AD10","p","KeyP",0,false,false,true);
        row("mapped-shift-p","AD10","P","KeyP",Qt::ShiftModifier,false,false,true);
        row("mapped-caps-p","AD10","P","KeyP",0,true,false,true);
        row("mapped-shift-caps-p","AD10","p","KeyP",Qt::ShiftModifier,true,false,true);
        row("mapped-layout-letter","AD06",layout=="de"?"z":"y","KeyY",0,false,false,true);
        row("original-caps-p","AD10","P","KeyP",0,true);
        row("original-shift-caps-p","AD10","p","KeyP",Qt::ShiftModifier,true);
        row("original-shift-digit","AE03",layout=="gb"?QString::fromUtf8("£"):layout=="de"?QString::fromUtf8("§"):QString("#"),"Digit3",Qt::ShiftModifier);
        row("original-layout-text","AC11",layout=="de"?QString::fromUtf8("ä"):QString("'"),"Quote");
        row("original-keypad-digit","KP1","1","Numpad1",0,false,true);
        row("original-keypad-enter","KPEN","","NumpadEnter");
        if (layout=="de") {
            row("original-altgr","AD01","@","KeyQ",Qt::GroupSwitchModifier);
            row("original-dead-compose","AD03",QString::fromUtf8("é"),"KeyE",0,false,false,false,true);
        }
    }
    void compiledNativeLayoutInput() {
        QFETCH(QString,position);QFETCH(QString,sourceText);QFETCH(QString,code);
        QFETCH(int,modifiers);QFETCH(bool,caps);QFETCH(bool,num);QFETCH(bool,mapped);QFETCH(bool,dead);
        if (!QGuiApplication::platformName().startsWith("wayland"))
            QSKIP("Layout qualification requires the isolated owned Wayland compositor");
        window.setTitle(QStringLiteral("ferric-p10-layout-%1").arg(QCoreApplication::applicationPid()));
        openPolicyTrace();pass(false);
        if (mapped) observePageReceiver();
        else {
            QVERIFY(js("document.getElementById('editor').focus();true").toBool());
            QTRY_VERIFY(QInputMethod::queryFocusObject(Qt::ImEnabled,{}).toBool());
        }
        NativeKeyTrace trace(&window);
        const auto failedTrace=qScopeGuard([&]{if(QTest::currentTestFailed())qInfo().noquote()
            <<"P10_LAYOUT_FAILURE"<<QJsonDocument::fromVariant(trace.samples).toJson(QJsonDocument::Compact)
            <<"DOM"<<QJsonDocument::fromVariant(js("policyFixture.snapshot()")).toJson(QJsonDocument::Compact);});
        QObject policy;
        QString result,error;
        const QVariantMap destination{{"text","k"},{"named",""},{"ctrl",false},
            {"alt",false},{"meta",false},{"shift",false}};
        connect(&router,&FerricBrowserKeyRouter::keyPressed,&policy,
            [&](const QString &text,int key,int,bool repeat,bool) {
                if (repeat) return;
                if (mapped && text==sourceText)
                    result=router.submitCurrentPageCommand(view,"k",destination,true,nullptr,{},router.pageReceiverKey());
                else if (!mapped && !text.isEmpty() && key!=Qt::Key_AltGr)
                    router.observeCurrentPageKey(view,text,true);
            });
        bool capsSet=false,numSet=false;
        const auto unlock=qScopeGuard([&]{
            if(capsSet)sendNativeProbeKey(&window,Qt::Key_CapsLock,&error);
            if(numSet)sendNativeProbeKey(&window,Qt::Key_NumLock,&error);
        });
        if(caps) {
            QVERIFY2(sendNativeProbeKey(&window,Qt::Key_CapsLock,&error),qPrintable(error));capsSet=true;
            QTRY_VERIFY(js("policyFixture.snapshot().events.some(e=>e.key==='CapsLock'&&e.type==='keyup')").toBool());
        }
        if(num) {
            QVERIFY2(sendNativeProbeKey(&window,Qt::Key_NumLock,&error),qPrintable(error));numSet=true;
            QTRY_VERIFY(js("policyFixture.snapshot().events.some(e=>e.key==='NumLock'&&e.type==='keyup')").toBool());
        }
        QVERIFY(js("policyFixture.clear();true").toBool());trace.samples.clear();
        if(dead)QVERIFY2(sendNativeProbeKey(&window,0,&error,Qt::NoModifier,"AE12"),qPrintable(error));
        QVERIFY2(sendNativeProbeKey(&window,0,&error,Qt::KeyboardModifiers(modifiers),position),qPrintable(error));
        if(mapped) {
            QTRY_COMPARE(result,QString("owned"));
            QTRY_COMPARE(js("policyFixture.snapshot().events.filter(e=>e.key==='k').length").toInt(),2);
            QVERIFY(js("policyFixture.snapshot().events.filter(e=>e.key==='k').every(e=>e.code==='KeyK'&&!e.shift&&!e.caps&&e.trusted)").toBool());
            QVERIFY(js("policyFixture.snapshot().events.every(e=>e.key==='k'||e.key==='Shift')").toBool());
            QCOMPARE(js("policyFixture.snapshot().events.filter(e=>e.key==='Shift').length").toInt(),
                modifiers&Qt::ShiftModifier?2:0);
        } else {
            QTRY_COMPARE(js("document.getElementById('editor').value").toString(),sourceText);
            const auto quoted=QString::fromUtf8(QJsonDocument(QJsonArray{code}).toJson(QJsonDocument::Compact));
            QTRY_VERIFY(js("policyFixture.snapshot().events.some(e=>e.type==='keyup'&&e.code==="+quoted+"[0])").toBool());
            QVERIFY(js("policyFixture.snapshot().events.every(e=>e.trusted)").toBool());
            if(position.startsWith("KP"))
                QVERIFY(js("policyFixture.snapshot().events.filter(e=>e.type==='keydown'||e.type==='keyup').every(e=>e.location===3)").toBool());
        }
        QVERIFY(std::any_of(trace.samples.cbegin(),trace.samples.cend(),[](const QVariant &sample){
            return sample.toMap().value("scan").toUInt()!=0;
        }));
        QCOMPARE(router.feasibilitySourceCount(),0);QCOMPARE(router.feasibilityOutputCount(),0);
        qInfo().noquote()<<"P10_LAYOUT"<<qEnvironmentVariable("FERRIC_NATIVE_PROBE_LAYOUT","us")
            <<QTest::currentDataTag()<<"Qt"<<QJsonDocument::fromVariant(trace.samples).toJson(QJsonDocument::Compact)
            <<"DOM"<<QJsonDocument::fromVariant(js("policyFixture.snapshot()")).toJson(QJsonDocument::Compact);
    }
    void sitePolicyNativePlatformInput_data() {
        QTest::addColumn<bool>("once");
        QTest::addColumn<bool>("raw");
        QTest::newRow("mapped-persistent") << false << false;
        QTest::newRow("mapped-one-shot") << true << false;
        QTest::newRow("raw-persistent") << false << true;
    }
    void sitePolicyNativePlatformInput() {
        QFETCH(bool, once);
        QFETCH(bool, raw);
        if (QGuiApplication::platformName() != "xcb"
            && !QGuiApplication::platformName().startsWith("wayland"))
            QSKIP("Native source probe requires X11/XTEST or the owned Wayland compositor");
        window.setTitle(QStringLiteral("ferric-p01-native-%1").arg(QCoreApplication::applicationPid()));
        openPolicyTrace(); pass(once); observePageReceiver();
        QObject policy;
        const QVariantMap destination{{"text","k"},{"named",""},{"ctrl",false},
            {"alt",false},{"meta",false},{"shift",false}};
        connect(&router,&FerricBrowserKeyRouter::keyPressed,&policy,
            [&](const QString &text,int key,int,bool repeat,bool) {
                if (repeat || key != Qt::Key_P || text != "p") return;
                if (raw) { router.observeCurrentPageKey(view,"p",true); router.pageOwnsCurrentEvent(); }
                else router.submitCurrentPageCommand(view,"k",destination,true,nullptr,{},router.pageReceiverKey());
            });
        NativeKeyTrace trace(&window);
        QString error;
        QVERIFY2(sendNativeProbeKey(&window, Qt::Key_P, &error), qPrintable(error));
        QTRY_VERIFY2(js("policyFixture.snapshot().events.length").toInt() == 2,
            qPrintable(QStringLiteral("Native source trace: %1; sources=%2 outputs=%3 active=%4 enabled=%5")
                .arg(QString::fromUtf8(QJsonDocument::fromVariant(trace.samples).toJson(QJsonDocument::Compact)))
                .arg(router.feasibilitySourceCount()).arg(router.feasibilityOutputCount())
                .arg(window.isActive()).arg(router.enabled())));
        const auto dom = js("policyFixture.snapshot().events").toList();
        qInfo().noquote() << "P01 native source" << QGuiApplication::platformName()
            << QJsonDocument::fromVariant(trace.samples).toJson(QJsonDocument::Compact)
            << "DOM" << QJsonDocument::fromVariant(dom).toJson(QJsonDocument::Compact);
        QVERIFY(std::any_of(trace.samples.cbegin(), trace.samples.cend(), [](const QVariant &sample) {
            const auto event = sample.toMap();
            return event.value("key").toInt() == Qt::Key_P && event.value("scan").toUInt() == 33
                && event.value("modifiers").toInt() == 0;
        }));
        QCOMPARE(js("policyFixture.snapshot().events.map(e=>e.key).join(',')").toString(), raw ? "p,p" : "k,k");
        QCOMPARE(js("policyFixture.snapshot().events.map(e=>e.code).join(',')").toString(), raw ? "KeyP,KeyP" : "KeyK,KeyK");
        QCOMPARE(js("policyFixture.snapshot().events.map(e=>e.type).join(',')").toString(), "keydown,keyup");
        QVERIFY(js("policyFixture.snapshot().events.every(e=>e.trusted&&!e.repeat&&!e.ctrl&&!e.shift&&!e.alt&&!e.meta)").toBool());
        QVERIFY(js("policyFixture.snapshot().events.some(e=>e.type==='keydown'&&e.activation)").toBool());
        QCOMPARE(router.feasibilitySourceCount(), 0);
        QCOMPARE(router.feasibilityOutputCount(), 0);
        QCOMPARE(passExits, once ? 1 : 0);
    }
    void sitePolicyNativeMedia() {
        window.setTitle(QStringLiteral("ferric-p10-permission-%1").arg(QCoreApplication::applicationPid()));
        openPolicyTrace();pass(false);observePageReceiver();
        QObject policy;
        const QVariantMap destination{{"text","k"},{"named",""},{"ctrl",false},
            {"alt",false},{"meta",false},{"shift",false}};
        connect(&router,&FerricBrowserKeyRouter::keyPressed,&policy,
            [&](const QString &text,int key,int,bool repeat,bool) {
                if (!repeat && key==Qt::Key_P && text=="p")
                    router.submitCurrentPageCommand(view,"k",destination,true,nullptr,{},router.pageReceiverKey());
            });
        const auto url = QString::fromUtf8(QJsonDocument(QJsonArray{
            QUrl::fromLocalFile(files.filePath("silence.wav")).toString()}).toJson(QJsonDocument::Compact));
        QVERIFY(js(QStringLiteral("window.probeAudio=new Audio((%1)[0]);probeAudio.preload='auto';"
                                  "window.mediaOutcome='pending';probeAudio.load();true").arg(url)).toBool());
        QTRY_VERIFY(js("probeAudio.readyState>=2").toBool());
        js("probeAudio.play().then(()=>window.mediaOutcome='allowed').catch(e=>window.mediaOutcome=e.name);true");
        QTRY_VERIFY(js("mediaOutcome!=='pending'").toBool());
        QCOMPARE(js("mediaOutcome").toString(), "NotAllowedError");
        js("window.mediaOutcome='pending';document.addEventListener('keydown',e=>{if(e.key==='k')"
           "probeAudio.play().then(()=>window.mediaOutcome='allowed').catch(e=>window.mediaOutcome=e.name)});true");
        if (QGuiApplication::platformName().startsWith("wayland")) {
            QString error;QVERIFY2(sendNativeProbeKey(&window,Qt::Key_P,&error),qPrintable(error));
        } else { probeSource(Qt::Key_P,true);probeSource(Qt::Key_P,false); }
        QTRY_VERIFY(js("mediaOutcome!=='pending'").toBool());
        qInfo() << "P10 compiled local media after mapped K:" << js("mediaOutcome");
        QCOMPARE(js("mediaOutcome").toString(), "allowed");
        QVERIFY(js("!probeAudio.paused").toBool());
        js("probeAudio.pause();true");
    }
    void sitePolicyNativeFullscreen_data() {
        QTest::addColumn<bool>("featureEnabled");
        QTest::newRow("Qt-feature-disabled") << false;
        QTest::newRow("Qt-feature-enabled") << true;
    }
    void sitePolicyNativeFullscreen() {
        QFETCH(bool, featureEnabled);
        window.setTitle(QStringLiteral("ferric-p10-permission-%1").arg(QCoreApplication::applicationPid()));
        openPolicyTrace();pass(false);observePageReceiver();
        QObject policy;
        const QVariantMap destination{{"text","k"},{"named",""},{"ctrl",false},
            {"alt",false},{"meta",false},{"shift",false}};
        connect(&router,&FerricBrowserKeyRouter::keyPressed,&policy,
            [&](const QString &text,int key,int,bool repeat,bool) {
                if (!repeat && key==Qt::Key_P && text=="p")
                    router.submitCurrentPageCommand(view,"k",destination,true,nullptr,{},router.pageReceiverKey());
            });
        view->setProperty("probeFullscreenEnabled", featureEnabled);
        view->setProperty("probeFullscreenRequests", 0);
        js("window.fullscreenOutcome='pending';document.addEventListener('keydown',e=>{if(e.key==='k')"
           "document.documentElement.requestFullscreen().then(()=>window.fullscreenOutcome='allowed')"
           ".catch(e=>window.fullscreenOutcome=e.name)});true");
        if (QGuiApplication::platformName().startsWith("wayland")) {
            QString error;QVERIFY2(sendNativeProbeKey(&window,Qt::Key_P,&error),qPrintable(error));
        } else { probeSource(Qt::Key_P,true);probeSource(Qt::Key_P,false); }
        QTRY_VERIFY(js("fullscreenOutcome!=='pending'").toBool());
        qInfo() << "P10 compiled fullscreen:" << featureEnabled << js("fullscreenOutcome")
                << "Qt requests:" << view->property("probeFullscreenRequests");
        QCOMPARE(js("fullscreenOutcome").toString(), featureEnabled ? "allowed" : "TypeError");
        QCOMPARE(js("!!document.fullscreenElement").toBool(), featureEnabled);
        QCOMPARE(view->property("probeFullscreenRequests").toInt(), featureEnabled ? 1 : 0);
        if (featureEnabled) {
            js("document.exitFullscreen();true");
            QTRY_VERIFY(js("!document.fullscreenElement").toBool());
        }
        view->setProperty("probeFullscreenEnabled", false);
    }
    void sitePolicyNativeReentrantSetter_data() {
        QTest::addColumn<QString>("setter");
        QTest::newRow("epoch") << QString("epoch");
        QTest::newRow("persistent") << QString("persistent");
        QTest::newRow("one-shot") << QString("one-shot");
    }
    void sitePolicyNativeReentrantSetter() {
        QFETCH(QString, setter);
        ownedProbe(setter == "one-shot");
        probeSource(Qt::Key_P, true);
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(), 1);
        NativeKeyHook hook;
        bool nested = false;
        const int outerEpoch = router.inputEpoch() + 1;
        const int newerEpoch = outerEpoch + 1;
        hook.callback = [&](QKeyEvent *event) {
            if (nested || event->type() != QEvent::KeyRelease || event->key() != Qt::Key_K) return;
            nested = true;
            if (setter == "epoch") router.setInputEpoch(newerEpoch);
            else if (setter == "persistent") router.setPassThrough(true);
            else router.setPassNextKey(true);
        };
        window.activeFocusItem()->installEventFilter(&hook);
        if (setter == "epoch") router.setInputEpoch(outerEpoch);
        else if (setter == "persistent") router.setPassThrough(false);
        else router.setPassNextKey(false);
        const int observed = setter == "epoch" ? router.inputEpoch()
            : setter == "persistent" ? int(router.passThrough()) : int(router.passNextKey());
        probeSource(Qt::Key_P, false);
        QVERIFY(nested);
        QCOMPARE(observed, setter == "epoch" ? newerEpoch : 1);
        QCOMPARE(router.feasibilitySourceCount(), 0);
    }
    void sitePolicyNativeRouterDestruction_data() {
        QTest::addColumn<QString>("phase");
        QTest::addColumn<bool>("collisionLoser");
        QTest::newRow("down") << QString("down") << false;
        QTest::newRow("admission") << QString("admission") << false;
        QTest::newRow("admission-after-terminal-release") << QString("admission-released") << false;
        QTest::newRow("repeat") << QString("repeat") << false;
        QTest::newRow("terminal-release") << QString("release") << false;
        QTest::newRow("repeat-with-suppressed-source") << QString("repeat") << true;
    }
    void sitePolicyNativeRouterDestruction() {
        QFETCH(QString, phase);
        QFETCH(bool, collisionLoser);
        openPolicyTrace();
        auto owner = std::make_unique<FerricBrowserKeyRouter>();
        QPointer<FerricBrowserKeyRouter> alive(owner.get());
        owner->setTargetWindow(&window); owner->setEnabled(true); owner->setPassThrough(true);
        const auto prepare = [&](FerricBrowserKeyRouter *target) {
            QJSEngine scripts;
            const auto loaded = scripts.evaluate(productionBrowserScript());
            QVERIFY2(!loaded.isError(), qPrintable(loaded.toString()));
            const auto script = scripts.evaluate("focusObserverSource()+';'+focusProbe()");
            QVERIFY(!script.isError());
            QSignalSpy observed(target, &FerricBrowserKeyRouter::pageReceiverObserved);
            target->probePageReceiver(view, script.toString(), target->inputEpoch());
            QTRY_COMPARE(observed.size(), 1);
            QVERIFY(!target->pageReceiverKey().isEmpty());
        };
        const auto connectPolicy = [&](FerricBrowserKeyRouter *target) {
            connect(target, &FerricBrowserKeyRouter::keyPressed, target,
                [&, target](const QString &, int key) {
                    if (key == Qt::Key_P) {
                        const QVariantMap destination{{"text","k"},{"named",""},
                            {"ctrl",false},{"alt",false},{"meta",false},{"shift",false}};
                        target->submitCurrentPageCommand(view, "k", destination, true,
                            nullptr, {}, target->pageReceiverKey());
                    } else if (key == Qt::Key_K) target->observeCurrentPageKey(view, "k");
                });
        };
        prepare(owner.get());
        NativeKeyHook hook;
        if (phase.startsWith("admission")) {
            connect(owner.get(), &FerricBrowserKeyRouter::keyPressed, &hook,
                    [&](const QString &, int key) {
                if (key != Qt::Key_P) return;
                if (phase == "admission-released") probeSource(Qt::Key_P, false);
                owner.reset();
            });
        }
        connectPolicy(owner.get());
        hook.callback = [&](QKeyEvent *event) {
            if (!owner || event->key() != Qt::Key_K) return;
            const bool match = phase == "down"
                ? event->type() == QEvent::KeyPress && !event->isAutoRepeat()
                : phase == "repeat" ? event->type() == QEvent::KeyPress && event->isAutoRepeat()
                                     : event->type() == QEvent::KeyRelease;
            if (match) owner.reset();
        };
        window.activeFocusItem()->installEventFilter(&hook);
        probeSource(Qt::Key_P, true);
        if (collisionLoser) probeSource(Qt::Key_K, true);
        if (phase == "repeat") probeSource(Qt::Key_P, true, true);
        if (phase == "release") probeSource(Qt::Key_P, false);
        QVERIFY(alive.isNull());
        FerricBrowserKeyRouter replacement;
        replacement.setTargetWindow(&window); replacement.setEnabled(true);
        replacement.setPassThrough(true);
        prepare(&replacement);
        connectPolicy(&replacement);
        if (phase == "release" || phase == "admission-released") {
            // Its source terminal release already happened: no held tombstone.
            const bool rawAvailable = replacement.canEnterRawPassThrough();
            replacement.setPassThrough(false); replacement.setPassThrough(true);
            probeSource(Qt::Key_P, true); probeSource(Qt::Key_P, false);
            QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(), phase == "release" ? 4 : 2);
            QVERIFY(rawAvailable);
            QCOMPARE(js("policyFixture.snapshot().events.map(e=>e.key).join(',')").toString(),
                     phase == "release" ? "k,k,k,k" : "k,k");
        } else {
            const bool refused = !replacement.canEnterRawPassThrough();
            probeSource(Qt::Key_P, false);
            const bool loserStillRefuses = !collisionLoser || !replacement.canEnterRawPassThrough();
            if (collisionLoser) probeSource(Qt::Key_K, false);
            probeSource(Qt::Key_L, true); probeSource(Qt::Key_L, false);
            QTRY_VERIFY(js("policyFixture.snapshot().events.some(e=>e.key==='l'&&e.type==='keyup')").toBool());
            QVERIFY(refused);
            QVERIFY(loserStillRefuses);
            const QString expected = phase == "admission" ? "l,l"
                : phase == "down" ? "k,l,l" : "k,k,l,l";
            QCOMPARE(js("policyFixture.snapshot().events.map(e=>e.key).join(',')").toString(), expected);
            QVERIFY(replacement.canEnterRawPassThrough());
        }
    }
    void compiledViewDestroyedDuringDispatch_data() {
        QTest::addColumn<QString>("phase");
        for (const auto *phase : {"down", "repeat", "release"})
            QTest::newRow(phase) << QString::fromLatin1(phase);
    }
    void compiledViewDestroyedDuringDispatch() {
        QFETCH(QString, phase);
        openPolicyTrace(); pass(false); observePageReceiver();
        const QPointer<QQuickItem> original(view);
        QObject policy;
        connect(&router, &FerricBrowserKeyRouter::keyPressed, &policy,
            [&](const QString &, int key) {
                if (key != Qt::Key_P) return;
                const QVariantMap destination{{"text","k"},{"named",""},
                    {"ctrl",false},{"alt",false},{"meta",false},{"shift",false}};
                router.submitCurrentPageCommand(view, "k", destination, true,
                    nullptr, {}, router.pageReceiverKey());
            });
        NativeKeyHook hook;
        hook.callback = [&](QKeyEvent *event) {
            if (event->key() != Qt::Key_K) return;
            const bool match = phase == "down"
                ? event->type() == QEvent::KeyPress && !event->isAutoRepeat()
                : phase == "repeat" ? event->type() == QEvent::KeyPress && event->isAutoRepeat()
                                     : event->type() == QEvent::KeyRelease;
            if (match) { window.setSource(QUrl()); view = nullptr; }
        };
        window.activeFocusItem()->installEventFilter(&hook);
        probeSource(Qt::Key_P, true);
        if (phase != "down") {
            QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(), 1);
            probeSource(Qt::Key_P, phase == "repeat", phase == "repeat");
        }
        // Restore the fixture before assertions so later cases retain their owner.
        window.setSource(QUrl::fromLocalFile(files.filePath("view.qml")));
        view = window.rootObject();
        QVERIFY(view);
        QVERIFY(original.isNull());
        QCOMPARE(router.feasibilityOutputCount(), 0);
        openPolicyTrace(); pass(false);
        if (phase != "release") {
            probeSource(Qt::Key_P, true, true);
            probeSource(Qt::Key_P, false);
        }
        probeSource(Qt::Key_L, true); probeSource(Qt::Key_L, false);
        QTRY_VERIFY(js("policyFixture.snapshot().events.some(e=>e.key==='l'&&e.type==='keyup')").toBool());
        QCOMPARE(js("policyFixture.snapshot().events.map(e=>e.key).join(',')").toString(), "l,l");
        QCOMPARE(router.feasibilitySourceCount(), 0);
        QVERIFY(router.canEnterRawPassThrough());
    }
    void sitePolicyNativeCrossRouterSupersession_data() {
        QTest::addColumn<bool>("destroyOuter");
        QTest::newRow("outer-survives") << false;
        QTest::newRow("outer-destroyed") << true;
    }
    void sitePolicyNativeCrossRouterSupersession() {
        QFETCH(bool, destroyOuter);
        openPolicyTrace();
        auto outer = std::make_unique<FerricBrowserKeyRouter>();
        outer->setTargetWindow(&window);
        outer->setEnabled(true);
        outer->setPassThrough(true);
        QVERIFY(outer->configureFeasibilityMapping(view, true));
        QObject context;
        bool nested = false;
        connect(outer.get(), &FerricBrowserKeyRouter::keyPressed, &context,
                [&](const QString &, int key) {
            if (key != Qt::Key_P || nested) return;
            nested = true;
            {
                FerricBrowserKeyRouter newer;
                newer.setTargetWindow(&window);
                newer.setEnabled(true);
                newer.setPassThrough(true);
                QVERIFY(newer.configureFeasibilityMapping(view, true));
                probeSource(Qt::Key_P, true);
                probeSource(Qt::Key_P, false);
                QCOMPARE(newer.feasibilitySourceCount(), 0);
            }
            // The newer owner has already retired. Its absence must not let
            // an older pending down deliver, or leave a held tombstone.
            if (destroyOuter) outer.reset();
        });
        probeSource(Qt::Key_P, true);
        const auto staleSources = outer ? outer->feasibilitySourceCount() : 0;
        if (staleSources) probeSource(Qt::Key_P, false);
        FerricBrowserKeyRouter replacement;
        replacement.setTargetWindow(&window);
        replacement.setEnabled(true);
        replacement.setPassThrough(true);
        const bool rawAvailable = replacement.requestFeasibilityRaw();
        // Preserve isolation when running this regression against broken code.
        if (!rawAvailable) probeSource(Qt::Key_P, false);
        probeSource(Qt::Key_L, true); probeSource(Qt::Key_L, false);
        QTRY_VERIFY(js("policyFixture.snapshot().events.some(e=>e.key==='l'&&e.type==='keyup')").toBool());
        QVERIFY(nested);
        QCOMPARE(staleSources, 0);
        QVERIFY(rawAvailable);
        QCOMPARE(js("policyFixture.snapshot().events.map(e=>e.key).join(',')").toString(), "k,k,l,l");
    }
    void sitePolicyNativeNestedRelease_data() {
        QTest::addColumn<bool>("duringAdmission");
        QTest::newRow("admission") << true;
        QTest::newRow("destination-down") << false;
    }
    void sitePolicyNativeNestedRelease() {
        QFETCH(bool, duringAdmission);
        ownedProbe(true);
        NativeKeyHook hook;
        QObject context;
        bool sent = false;
        if (duringAdmission) {
            connect(&router, &FerricBrowserKeyRouter::keyPressed, &context,
                    [&](const QString &, int key) {
                if (key == Qt::Key_P && !sent) { sent = true; probeSource(Qt::Key_P, false); }
            });
        } else {
            hook.callback = [&](QKeyEvent *event) {
                if (event->key() == Qt::Key_K && event->type() == QEvent::KeyPress && !sent) {
                    sent = true; probeSource(Qt::Key_P, false);
                }
            };
            window.activeFocusItem()->installEventFilter(&hook);
        }
        probeSource(Qt::Key_P, true);
        QVERIFY(sent);
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(), 2);
        QCOMPARE(js("policyFixture.snapshot().events.map(e=>e.type).join(',')").toString(), "keydown,keyup");
        QCOMPARE(js("policyFixture.snapshot().events.map(e=>e.key).join(',')").toString(), "k,k");
        QCOMPARE(passExits, 1);
        QCOMPARE(router.feasibilitySourceCount(), 0);
        QCOMPARE(router.feasibilityOutputCount(), 0);
    }
    void sitePolicyNativeReentrantWinnerRelease() {
        ownedProbe();
        probeSource(Qt::Key_K, true);
        NativeKeyHook hook;
        bool admitted = false;
        hook.callback = [&](QKeyEvent *event) {
            if (event->key() == Qt::Key_K && event->type() == QEvent::KeyRelease && !admitted) {
                admitted = true;
                probeSource(Qt::Key_P, true);
            }
        };
        window.activeFocusItem()->installEventFilter(&hook);
        probeSource(Qt::Key_K, false);
        QVERIFY(admitted);
        QCOMPARE(router.feasibilitySourceCount(), 1);
        QCOMPARE(router.feasibilityOutputCount(), 0);
        QVERIFY(!router.requestFeasibilityRaw());
        probeSource(Qt::Key_P, true, true);
        probeSource(Qt::Key_P, false);
        QTRY_COMPARE(js("policyFixture.snapshot().events.length").toInt(), 2);
        QCOMPARE(js("policyFixture.snapshot().events.map(e=>e.key).join(',')").toString(), "k,k");
        QCOMPARE(router.feasibilitySourceCount(), 0);
    }
    void sitePolicyNativeOwnerlessCapture_data() {
        QTest::addColumn<bool>("duringAdmission");
        QTest::newRow("destination-down") << false;
        QTest::newRow("admission") << true;
    }
    void sitePolicyNativeOwnerlessCapture() {
        QFETCH(bool, duringAdmission);
        openPolicyTrace();
        auto owner = std::make_unique<FerricBrowserKeyRouter>();
        owner->setTargetWindow(&window); owner->setEnabled(true); owner->setPassThrough(true);
        QVERIFY(owner->configureFeasibilityMapping(view, true));
        QCoreApplication::instance()->removeEventFilter(&router);
        struct RestoreFilter {
            QObject *router;
            ~RestoreFilter() { QCoreApplication::instance()->installEventFilter(router); }
        } restore{&router};
        NativeKeyHook hook;
        if (duringAdmission) {
            connect(owner.get(), &FerricBrowserKeyRouter::keyPressed, &hook,
                    [&](const QString &, int key) { if (key == Qt::Key_P) owner.reset(); });
        }
        hook.callback = [&](QKeyEvent *event) {
            if (owner && event->key() == Qt::Key_K && event->type() == QEvent::KeyPress)
                owner.reset();
        };
        window.activeFocusItem()->installEventFilter(&hook);
        probeSource(Qt::Key_P, true);
        QVERIFY(!owner);
        // No window router exists here. The app-owned source capture alone
        // must consume the owed physical release before any new owner exists.
        probeSource(Qt::Key_P, false);
        probeSource(Qt::Key_L, true); probeSource(Qt::Key_L, false);
        QTRY_VERIFY(js("policyFixture.snapshot().events.some(e=>e.key==='l'&&e.type==='keyup')").toBool());
        QCOMPARE(js("policyFixture.snapshot().events.map(e=>e.key).join(',')").toString(),
                 duringAdmission ? "l,l" : "k,l,l");
        FerricBrowserKeyRouter replacement;
        replacement.setTargetWindow(&window); replacement.setEnabled(true);
        replacement.setPassThrough(true);
        QVERIFY(replacement.requestFeasibilityRaw());
    }
    void passNext() {
        QFETCH(int, key);
        QFETCH(int, modifiers);
        QFETCH(QString, domKey);
        focus("player");
        pass();
        QTest::keyPress(&window, Qt::Key(key), Qt::KeyboardModifiers(modifiers));
        QCOMPARE(passExits, 0);
        QVERIFY(router.passNextKey());
        QTest::keyRelease(&window, Qt::Key(key), Qt::KeyboardModifiers(modifiers));
        QCOMPARE(passExits, 1);
        QVERIFY(!router.passNextKey());
        const QString filter = "events.filter(e=>e.key==='" + domKey + "')";
        QTRY_COMPARE(js(filter + ".length").toInt(), 2);
        QCOMPARE(js(filter + "[0].type").toString(), "keydown");
        QCOMPARE(js(filter + "[1].type").toString(), "keyup");
        QCOMPARE(js(filter + "[0].ctrl").toBool(), bool(modifiers & Qt::ControlModifier));
        QCOMPARE(js(filter + "[0].shift").toBool(), bool(modifiers & Qt::ShiftModifier));
        QTest::keyClick(&window, Qt::Key_J);
        QCOMPARE(navigations, 1);
    }
    void passEntryOwnsItsRepeatsAndRelease_data() {
        QTest::addColumn<bool>("once");
        QTest::newRow("once") << true;
        QTest::newRow("persistent") << false;
    }
    void passEntryOwnsItsRepeatsAndRelease() {
        QFETCH(bool, once);
        focus("player");
        normal();
        const auto modifiers = once ? Qt::ControlModifier | Qt::ShiftModifier : Qt::ControlModifier;
        QTest::keyPress(&window, Qt::Key_V, modifiers);
        QCOMPARE(router.passNextKey(), once);
        QKeyEvent repeatRelease(QEvent::KeyRelease, Qt::Key_V, modifiers, "V", true);
        QKeyEvent repeatPress(QEvent::KeyPress, Qt::Key_V, modifiers, "V", true);
        QCoreApplication::sendEvent(&window, &repeatRelease);
        QCoreApplication::sendEvent(&window, &repeatPress);
        QTest::keyRelease(&window, Qt::Key_V, modifiers);
        QCOMPARE(passExits, 0);
        QCOMPARE(js("events.filter(e=>e.key.toLowerCase()==='v').length").toInt(), 0);
        QTest::keyClick(&window, Qt::Key_F);
        QCOMPARE(passExits, once ? 1 : 0);
        QTRY_COMPARE(js("events.filter(e=>e.key==='f').length").toInt(), 2);
    }
    void passModifierDoesNotComplete() {
        focus("player");
        pass();
        QTest::keyClick(&window, Qt::Key_Shift);
        QTest::keyClick(&window, Qt::Key_Control);
        QCOMPARE(passExits, 0);
        QVERIFY(router.passNextKey());
        QTest::keyClick(&window, Qt::Key_F);
        QCOMPARE(passExits, 1);
    }
    void passEntryChainOwnsHeldPrefix() {
        focus("player");
        normal();
        QTest::keyPress(&window, Qt::Key_Semicolon);
        QTest::keyPress(&window, Qt::Key_V);
        QVERIFY(router.passNextKey());
        QKeyEvent repeatRelease(QEvent::KeyRelease, Qt::Key_Semicolon, Qt::NoModifier, ";", true);
        QKeyEvent repeatPress(QEvent::KeyPress, Qt::Key_Semicolon, Qt::NoModifier, ";", true);
        QCoreApplication::sendEvent(&window, &repeatRelease);
        QCoreApplication::sendEvent(&window, &repeatPress);
        QTest::keyRelease(&window, Qt::Key_Semicolon);
        QTest::keyRelease(&window, Qt::Key_V);
        QCOMPARE(passExits, 0);
        QCOMPARE(js("events.filter(e=>e.key===';'||e.key==='v').length").toInt(), 0);
        QTest::keyClick(&window, Qt::Key_F);
        QCOMPARE(passExits, 1);
        QTRY_COMPARE(js("events.filter(e=>e.key==='f').length").toInt(), 2);
    }
    void passHeldAndOverlappingKeys() {
        focus("player");
        pass();
        QTest::keyPress(&window, Qt::Key_F);
        QTest::keyPress(&window, Qt::Key_J);
        QKeyEvent repeatRelease(QEvent::KeyRelease, Qt::Key_F, Qt::NoModifier, "f", true);
        QKeyEvent repeatPress(QEvent::KeyPress, Qt::Key_F, Qt::NoModifier, "f", true);
        QCoreApplication::sendEvent(&window, &repeatRelease);
        QCoreApplication::sendEvent(&window, &repeatPress);
        QCOMPARE(passExits, 0);
        QTest::keyRelease(&window, Qt::Key_F);
        QCOMPARE(passExits, 1);
        QKeyEvent overlapRelease(QEvent::KeyRelease, Qt::Key_J, Qt::NoModifier, "j", true);
        QKeyEvent overlapPress(QEvent::KeyPress, Qt::Key_J, Qt::NoModifier, "j", true);
        QCoreApplication::sendEvent(&window, &overlapRelease);
        QCoreApplication::sendEvent(&window, &overlapPress);
        QTest::keyRelease(&window, Qt::Key_J);
        QCOMPARE(navigations, 0);
        QTRY_COMPARE(js("events.filter(e=>e.key==='f'&&e.type==='keydown').length").toInt(), 2);
        QTRY_COMPARE(js("events.filter(e=>e.key==='f'&&e.type==='keyup').length").toInt(), 1);
        QTRY_COMPARE(js("events.filter(e=>e.key==='j').length").toInt(), 3);
        QTest::keyPress(&window, Qt::Key_J);
        // A delayed renderer key-up must not steal this browser gesture's
        // release ownership and leak an unmatched key-up to the webpage.
        QKeyEvent bubbledRelease(QEvent::KeyRelease, Qt::Key_J, Qt::NoModifier, "j");
        QCoreApplication::sendEvent(window.contentItem(), &bubbledRelease);
        QTest::keyRelease(&window, Qt::Key_J);
        QCOMPARE(navigations, 1);
        QCOMPARE(js("events.filter(e=>e.key==='j').length").toInt(), 3);
    }
    void mixedPageOwnerSurvivesPolicyAndEpochChanges() {
        focus("player");
        normal();
        mixedPageKey = true;
        const int before = mixedRouteCalls;
        QTest::keyPress(&window, Qt::Key_F);
        QTest::keyPress(&window, Qt::Key_G);
        QCOMPARE(mixedRouteCalls, before + 2);

        mixedPageKey = false;
        router.setInputEpoch(router.inputEpoch() + 1);
        QKeyEvent repeatFRelease(QEvent::KeyRelease, Qt::Key_F, Qt::NoModifier, "f", true);
        QKeyEvent repeatFPress(QEvent::KeyPress, Qt::Key_F, Qt::NoModifier, "f", true);
        QKeyEvent repeatGRelease(QEvent::KeyRelease, Qt::Key_G, Qt::NoModifier, "g", true);
        QKeyEvent repeatGPress(QEvent::KeyPress, Qt::Key_G, Qt::NoModifier, "g", true);
        QCoreApplication::sendEvent(&window, &repeatFRelease);
        QCoreApplication::sendEvent(&window, &repeatFPress);
        QCoreApplication::sendEvent(&window, &repeatGRelease);
        QCoreApplication::sendEvent(&window, &repeatGPress);
        QTest::keyRelease(&window, Qt::Key_F);
        QTest::keyRelease(&window, Qt::Key_G);
        QCOMPARE(mixedRouteCalls, before + 2);
        QTRY_COMPARE(js("events.filter(e=>e.key==='f'&&e.type==='keydown').length").toInt(), 2);
        QTRY_COMPARE(js("events.filter(e=>e.key==='g'&&e.type==='keydown').length").toInt(), 2);
        QTRY_COMPARE(js("events.filter(e=>(e.key==='f'||e.key==='g')&&e.type==='keyup').length").toInt(), 2);
    }
    void browserBindingContextRetiresHeldSources() {
        focus("player");
        normal();
        QObject callbacks;
        QString context = "A";
        int dispatched = 0;
        const auto connection = connect(&router, &FerricBrowserKeyRouter::keyPressed,
            &callbacks, [&](const QString &, int key, int, bool, bool) {
                if (key != Qt::Key_Z && key != Qt::Key_X) return;
                if (router.captureBindingContext(context)) ++dispatched;
                router.acceptCurrentEvent();
            });
        Q_UNUSED(connection);
        const auto cleanup = qScopeGuard([&] {
            probeSource(Qt::Key_Z, false);
            probeSource(Qt::Key_X, false);
        });
        QSignalSpy releases(&router, &FerricBrowserKeyRouter::keyReleased);
        probeSource(Qt::Key_Z, true);
        probeSource(Qt::Key_Z, true, true);
        probeSource(Qt::Key_X, true);
        QCOMPARE(dispatched, 3);
        context = "B";
        probeSource(Qt::Key_Z, true, true);
        probeSource(Qt::Key_X, true, true);
        probeSource(Qt::Key_Z, false, true);
        QCOMPARE(dispatched, 3);
        QCOMPARE(releases.count(), 0);
        // Once cancellation was observed, even a repeated token cannot revive
        // that held gesture. A genuine new down may replace a lost release.
        context = "A";
        probeSource(Qt::Key_Z, true, true);
        QCOMPARE(dispatched, 3);
        probeSource(Qt::Key_Z, false);
        probeSource(Qt::Key_X, false);
        QCOMPARE(releases.count(), 2);
        probeSource(Qt::Key_Z, true);
        QCOMPARE(dispatched, 4);
        context = "C";
        probeSource(Qt::Key_Z, true);
        probeSource(Qt::Key_Z, true, true);
        QCOMPARE(dispatched, 6);
        probeSource(Qt::Key_Z, false);
        QCOMPARE(releases.count(), 3);
        QCOMPARE(js("events.filter(e=>e.key==='z'||e.key==='x').length").toInt(), 0);
    }
    void browserBindingContextPublicationDuringAdmission() {
        focus("player");
        normal();
        QObject callbacks;
        QString context = "A";
        QStringList dispatched;
        bool nested = false;
        connect(&router, &FerricBrowserKeyRouter::keyPressed, &callbacks,
            [&](const QString &, int key, int, bool repeat, bool) {
                if (key != Qt::Key_Z) return;
                if (!router.captureBindingContext(context)) {
                    router.acceptCurrentEvent();
                    return;
                }
                dispatched.append(context);
                if (!repeat && !nested) {
                    nested = true;
                    context = "B";
                    probeSource(Qt::Key_Z, true, true);
                }
                router.acceptCurrentEvent();
            });
        const auto cleanup = qScopeGuard([&] { probeSource(Qt::Key_Z, false); });
        QSignalSpy releases(&router, &FerricBrowserKeyRouter::keyReleased);
        probeSource(Qt::Key_Z, true);
        QCOMPARE(dispatched, QStringList({"A"}));
        probeSource(Qt::Key_Z, true, true);
        QCOMPARE(dispatched, QStringList({"A"}));
        probeSource(Qt::Key_Z, false);
        QCOMPARE(releases.count(), 1);
        probeSource(Qt::Key_Z, true);
        QCOMPARE(dispatched, QStringList({"A", "B"}));
        probeSource(Qt::Key_Z, false);
        QCOMPARE(releases.count(), 2);
        QCOMPARE(js("events.filter(e=>e.key==='z').length").toInt(), 0);
    }
    void browserBindingContextNestedFreshPress() {
        focus("player");
        normal();
        QObject callbacks;
        QString context = "A";
        QStringList dispatched;
        bool nested = false;
        connect(&router, &FerricBrowserKeyRouter::keyPressed, &callbacks,
            [&](const QString &, int key, int, bool repeat, bool) {
                if (key != Qt::Key_Z) return;
                if (!router.captureBindingContext(context)) {
                    router.acceptCurrentEvent();
                    return;
                }
                dispatched.append(context);
                if (!repeat && !nested) {
                    nested = true;
                    context = "B";
                    probeSource(Qt::Key_Z, true);
                }
                router.acceptCurrentEvent();
            });
        const auto cleanup = qScopeGuard([&] { probeSource(Qt::Key_Z, false); });
        probeSource(Qt::Key_Z, true);
        probeSource(Qt::Key_Z, true, true);
        QCOMPARE(dispatched, QStringList({"A", "B", "B"}));
        probeSource(Qt::Key_Z, false);
        QCOMPARE(js("events.filter(e=>e.key==='z').length").toInt(), 0);
    }
    void browserBindingContextForeignRouter() {
        focus("player");
        normal();
        QObject callbacks;
        int firstActions = 0;
        connect(&router, &FerricBrowserKeyRouter::keyPressed, &callbacks,
            [&](const QString &, int key, int, bool, bool) {
                if (key != Qt::Key_Z) return;
                if (router.captureBindingContext("first")) ++firstActions;
                router.acceptCurrentEvent();
            });
        bool held = true;
        const auto cleanup = qScopeGuard([&] {
            if (held) probeSource(Qt::Key_Z, false);
        });
        QSignalSpy firstReleases(&router, &FerricBrowserKeyRouter::keyReleased);
        probeSource(Qt::Key_Z, true);
        QCOMPARE(firstActions, 1);
        QQuickWindow secondWindow;
        FerricBrowserKeyRouter second;
        secondWindow.resize(200, 150);
        secondWindow.show();
        secondWindow.requestActivate();
        QTRY_COMPARE(QGuiApplication::focusWindow(), &secondWindow);
        second.setTargetWindow(&secondWindow);
        second.setEnabled(true);
        int secondActions = 0;
        connect(&second, &FerricBrowserKeyRouter::keyPressed, &callbacks,
            [&](const QString &, int key, int, bool, bool) {
                if (key != Qt::Key_Z) return;
                if (second.captureBindingContext("second")) ++secondActions;
                second.acceptCurrentEvent();
            });
        QSignalSpy secondReleases(&second, &FerricBrowserKeyRouter::keyReleased);
        QKeyEvent repeat(QEvent::KeyPress, Qt::Key_Z, Qt::NoModifier, "z", true);
        QCoreApplication::sendEvent(&secondWindow, &repeat);
        QCOMPARE(secondActions, 0);
        QKeyEvent release(QEvent::KeyRelease, Qt::Key_Z, Qt::NoModifier, "z");
        QCoreApplication::sendEvent(&secondWindow, &release);
        held = false;
        QCOMPARE(firstReleases.count(), 1);
        QCOMPARE(secondReleases.count(), 0);
        QTest::keyClick(&secondWindow, Qt::Key_Z);
        QCOMPARE(secondActions, 1);
        QCOMPARE(secondReleases.count(), 1);
    }
    void browserBindingContextDestroyedOwner_data() {
        QTest::addColumn<bool>("duringDispatch");
        QTest::newRow("after-admission") << false;
        QTest::newRow("during-admission") << true;
    }
    void browserBindingContextDestroyedOwner() {
        QFETCH(bool, duringDispatch);
        focus("player");
        normal();
        QObject callbacks;
        auto *first = new FerricBrowserKeyRouter;
        QPointer<FerricBrowserKeyRouter> guardedFirst(first);
        const auto cleanupFirst = qScopeGuard([&] {
            if (guardedFirst) delete guardedFirst.data();
        });
        first->setTargetWindow(&window);
        first->setEnabled(true);
        int firstActions = 0;
        connect(first, &FerricBrowserKeyRouter::keyPressed, &callbacks,
            [&](const QString &, int key, int, bool, bool) {
                if (key != Qt::Key_Z) return;
                if (first->captureBindingContext("first")) ++firstActions;
                first->acceptCurrentEvent();
                if (duringDispatch) delete first;
            });
        probeSource(Qt::Key_Z, true);
        QCOMPARE(firstActions, 1);
        if (!duringDispatch) delete first;
        QVERIFY(guardedFirst.isNull());
        FerricBrowserKeyRouter second;
        second.setTargetWindow(&window);
        second.setEnabled(true);
        int secondActions = 0;
        connect(&second, &FerricBrowserKeyRouter::keyPressed, &callbacks,
            [&](const QString &, int key, int, bool, bool) {
                if (key != Qt::Key_Z) return;
                if (second.captureBindingContext("second")) ++secondActions;
                second.acceptCurrentEvent();
            });
        const auto cleanupHeld = qScopeGuard([&] { probeSource(Qt::Key_Z, false); });
        probeSource(Qt::Key_Z, true, true);
        QCOMPARE(secondActions, 0);
        probeSource(Qt::Key_Z, false);
        probeSource(Qt::Key_Z, true);
        QCOMPARE(secondActions, 1);
        probeSource(Qt::Key_Z, false);
        QCOMPARE(js("events.filter(e=>e.key==='z').length").toInt(), 0);
    }
    void browserBindingContextPendingForeignReturns() {
        focus("player");
        normal();
        QObject callbacks;
        FerricBrowserKeyRouter second;
        second.setTargetWindow(&window);
        int firstActions = 0;
        int secondActions = 0;
        bool nested = false;
        connect(&second, &FerricBrowserKeyRouter::keyPressed, &callbacks,
            [&](const QString &, int key, int, bool, bool) {
                if (key != Qt::Key_Z) return;
                if (second.captureBindingContext("second")) ++secondActions;
                second.acceptCurrentEvent();
            });
        connect(&router, &FerricBrowserKeyRouter::keyPressed, &callbacks,
            [&](const QString &, int key, int, bool repeat, bool) {
                if (key != Qt::Key_Z) return;
                if (!router.captureBindingContext("first")) {
                    router.acceptCurrentEvent();
                    return;
                }
                ++firstActions;
                if (!repeat && !nested) {
                    nested = true;
                    second.setEnabled(true);
                    probeSource(Qt::Key_Z, true, true);
                    second.setEnabled(false);
                }
                router.acceptCurrentEvent();
            });
        const auto cleanup = qScopeGuard([&] { probeSource(Qt::Key_Z, false); });
        probeSource(Qt::Key_Z, true);
        QCOMPARE(firstActions, 1);
        QCOMPARE(secondActions, 0);
        probeSource(Qt::Key_Z, true, true);
        QCOMPARE(firstActions, 1);
        probeSource(Qt::Key_Z, false);
        probeSource(Qt::Key_Z, true);
        QCOMPARE(firstActions, 2);
        probeSource(Qt::Key_Z, false);
    }
    void browserBindingContextTargetReplacement() {
        focus("player");
        normal();
        QObject callbacks;
        QQuickWindow replacement;
        replacement.resize(200, 150);
        bool held = false;
        const auto cleanup = qScopeGuard([&] {
            if (held) {
                QKeyEvent release(QEvent::KeyRelease, Qt::Key_Z, Qt::NoModifier, "z");
                QCoreApplication::sendEvent(&replacement, &release);
            }
            router.setTargetWindow(&window);
            replacement.hide();
            window.requestActivate();
        });
        int dispatched = 0;
        connect(&router, &FerricBrowserKeyRouter::keyPressed, &callbacks,
            [&](const QString &, int key, int, bool repeat, bool) {
                if (key != Qt::Key_Z) return;
                if (!router.captureBindingContext("same-ui")) {
                    router.acceptCurrentEvent();
                    return;
                }
                ++dispatched;
                if (!repeat && dispatched == 1) router.setTargetWindow(&replacement);
                router.acceptCurrentEvent();
            });
        held = true;
        probeSource(Qt::Key_Z, true);
        QCOMPARE(dispatched, 1);
        replacement.show();
        replacement.requestActivate();
        QTRY_COMPARE(QGuiApplication::focusWindow(), &replacement);
        QKeyEvent repeat(QEvent::KeyPress, Qt::Key_Z, Qt::NoModifier, "z", true);
        QCoreApplication::sendEvent(&replacement, &repeat);
        QCOMPARE(dispatched, 1);
        QKeyEvent release(QEvent::KeyRelease, Qt::Key_Z, Qt::NoModifier, "z");
        QCoreApplication::sendEvent(&replacement, &release);
        held = false;
        QTest::keyClick(&replacement, Qt::Key_Z);
        QCOMPARE(dispatched, 2);
    }
    void browserBindingContextFocusReceiverReturns() {
        focus("player");
        normal();
        QObject callbacks;
        int dispatched = 0;
        connect(&router, &FerricBrowserKeyRouter::keyPressed, &callbacks,
            [&](const QString &, int key, int, bool, bool) {
                if (key != Qt::Key_Z) return;
                if (router.captureBindingContext("same-ui")) ++dispatched;
                router.acceptCurrentEvent();
            });
        const auto cleanup = qScopeGuard([&] { probeSource(Qt::Key_Z, false); });
        probeSource(Qt::Key_Z, true);
        QCOMPARE(dispatched, 1);
        QQuickItem chrome;
        chrome.setParentItem(window.contentItem());
        chrome.forceActiveFocus();
        QCOMPARE(window.activeFocusItem(), &chrome);
        view->forceActiveFocus();
        QTRY_VERIFY(window.activeFocusItem() != &chrome);
        probeSource(Qt::Key_Z, true, true);
        QCOMPARE(dispatched, 1);
        probeSource(Qt::Key_Z, false);
        probeSource(Qt::Key_Z, true);
        QCOMPARE(dispatched, 2);
        probeSource(Qt::Key_Z, false);
    }
    void mixedBrowserOwnerSurvivesPageMappingChange() {
        focus("player");
        normal();
        mixedBrowserKey = true;
        QTest::keyPress(&window, Qt::Key_F);
        QCOMPARE(mixedRouteCalls, 1);
        mixedBrowserKey = false;
        mixedPageKey = true;
        QKeyEvent repeatRelease(QEvent::KeyRelease, Qt::Key_F, Qt::NoModifier, "f", true);
        QKeyEvent repeatPress(QEvent::KeyPress, Qt::Key_F, Qt::NoModifier, "f", true);
        QCoreApplication::sendEvent(&window, &repeatRelease);
        QCoreApplication::sendEvent(&window, &repeatPress);
        QTest::keyRelease(&window, Qt::Key_F);
        QCOMPARE(browserOwnedRepeats, 1);
        QCOMPARE(mixedRouteCalls, 1);
        QCOMPARE(js("events.filter(e=>e.key==='f').length").toInt(), 0);
    }
    void nativePromptInputBypassesMixedRouting() {
        focus("player");
        normal();
        mixedPageKey = true;
        nativePromptActive = true;
        const int before = mixedRouteCalls;

        QTest::keyClick(&window, Qt::Key_F);

        QCOMPARE(mixedRouteCalls, before);
        QTRY_COMPARE(js("events.filter(e=>e.key==='f'&&e.type==='keydown').length").toInt(), 1);
        QTRY_COMPARE(js("events.filter(e=>e.key==='f'&&e.type==='keyup').length").toInt(), 1);
        nativePromptActive = false;
    }
    void passPersistentAndRecovery() {
        focus("player");
        pass(false);
        for (auto key : {Qt::Key_J, Qt::Key_Escape, Qt::Key_Return})
            QTest::keyClick(&window, key);
        QCOMPARE(view->property("ferricMode").toString(), "pass-through");
        QCOMPARE(navigations, 0);
        QCOMPARE(passExits, 0);
        QTRY_COMPARE(js("events.filter(e=>e.key==='Escape').length").toInt(), 2);
        for (bool once : {false, true}) {
            pass(once);
            QTest::keyClick(&window, Qt::Key_Escape, Qt::ControlModifier);
            QCOMPARE(view->property("ferricMode").toString(), "normal");
        }
        QCOMPARE(js("events.filter(e=>e.key==='Escape'&&e.ctrl).length").toInt(), 0);
        QCOMPARE(passExits, 0);
    }
    void passEpochInvalidationAndLostRelease() {
        focus("player");
        pass();
        QTest::keyPress(&window, Qt::Key_F);
        pass(); // A new session replaces the one owning the outstanding press.
        QTest::keyRelease(&window, Qt::Key_F);
        QCOMPARE(passExits, 0);
        QVERIFY(router.passNextKey());
        QTest::keyPress(&window, Qt::Key_F); // Its release is lost.
        QTest::keyClick(&window, Qt::Key_F);
        QCOMPARE(passExits, 1);
        QTRY_COMPARE(js("events.filter(e=>e.key==='f'&&e.type==='keydown').length").toInt(), 3);
        QTRY_COMPARE(js("events.filter(e=>e.key==='f'&&e.type==='keyup').length").toInt(), 2);
    }
    void passNavigationInvalidatesCompletion() {
        focus("navigate");
        pass();
        QTest::keyPress(&window, Qt::Key_Return);
        QTRY_VERIFY(view->property("url").toUrl().path().endsWith("next.html"));
        normal();
        QTest::keyRelease(&window, Qt::Key_Return);
        QCOMPARE(passExits, 0);
    }
    void passDeactivation() {
        focus("player");
        QWindow other;
        other.show();
        for (bool once : {true, false}) {
            pass(once);
            QTest::keyPress(&window, Qt::Key_F);
            other.requestActivate();
            QTRY_COMPARE(QGuiApplication::focusWindow(), &other);
            QCOMPARE(view->property("ferricMode").toString(), once ? "normal" : "pass-through");
            QTest::keyRelease(&other, Qt::Key_F);
            QCOMPARE(passExits, 0);
            window.requestActivate();
            QTRY_COMPARE(QGuiApplication::focusWindow(), &window);
        }
    }
    void nativeEnter_data() {
        QTest::addColumn<QString>("id");
        QTest::newRow("form") << "single";
        QTest::newRow("textarea") << "multi";
        QTest::newRow("autocomplete") << "complete";
        QTest::newRow("contenteditable") << "rich";
        QTest::newRow("self-removal") << "remove";
    }
    void nativeEnter() {
        QFETCH(QString, id);
        focus(id);
        QTest::keyPress(&window, Qt::Key_Return);
        QCOMPARE(exits, 0);
        QVERIFY(router.transientInsert());
        QTRY_COMPARE(js("down").toInt(), 1);
        QTest::keyRelease(&window, Qt::Key_Return);
        QCOMPARE(exits, 1);
        QTRY_COMPARE(js("up").toInt(), 1);
        if (id == "single") QCOMPARE(js("submits").toInt(), 1);
        if (id == "multi") QCOMPARE(js("document.getElementById('multi').value").toString(), "\n");
        if (id == "complete") QCOMPARE(js("document.getElementById('complete').value").toString(), "accepted");
        if (id == "rich") QVERIFY(js("document.getElementById('rich').childNodes.length > 0").toBool());
        if (id == "remove") QVERIFY(js("!document.getElementById('remove')").toBool());
        QTest::keyClick(&window, Qt::Key_J);
        QCOMPARE(navigations, 1);
        QCOMPARE(exits, 1);
    }
    void modifiedEnter_data() {
        QTest::addColumn<int>("modifiers");
        QTest::addColumn<bool>("persistent");
        for (bool persistent : {false, true}) {
            for (int modifier : {int(Qt::ShiftModifier), int(Qt::ControlModifier), int(Qt::AltModifier), int(Qt::MetaModifier)}) {
                QByteArray name = QByteArray::number(modifier) + (persistent ? "-locked" : "-transient");
                QTest::newRow(name) << modifier << persistent;
            }
        }
        QTest::newRow("locked-ordinary") << 0 << true;
    }
    void modifiedEnter() {
        QFETCH(int, modifiers);
        QFETCH(bool, persistent);
        enter(persistent);
        focus("multi");
        QTest::keyClick(&window, Qt::Key_Return, Qt::KeyboardModifiers(modifiers));
        QTRY_COMPARE(js("down").toInt(), 1);
        QTRY_COMPARE(js("up").toInt(), 1);
        QCOMPARE(js("shift").toBool(), bool(modifiers & Qt::ShiftModifier));
        QCOMPARE(exits, 0);
        QCOMPARE(router.transientInsert(), !persistent);
    }
    void compositionCommitPreservesText() {
        focus("multi");
        QTRY_VERIFY(QInputMethod::queryFocusObject(Qt::ImEnabled,{}).toBool());
        const QPointer<QQuickItem> receiver(window.activeFocusItem());QVERIFY(receiver);
        const auto finish=qScopeGuard([&]{if(receiver){QInputMethodEvent end;QCoreApplication::sendEvent(receiver,&end);}});
        QInputMethodEvent preedit("pending",{});QCoreApplication::sendEvent(receiver,&preedit);
        QVERIFY(router.compositionActive());
        QTRY_COMPARE(js("document.getElementById('multi').value").toString(),QString("pending"));
        QInputMethodEvent commit;commit.setCommitString("confirmed");QCoreApplication::sendEvent(receiver,&commit);
        QVERIFY(!router.compositionActive());
        QTRY_COMPARE(js("document.getElementById('multi').value").toString(),QString("confirmed"));
        QCOMPARE(exits,0);
    }
    void composition() {
        focus("multi");
        auto *receiver = window.activeFocusItem();
        QVERIFY(receiver);
        QInputMethodEvent preedit("composing", {});
        QCoreApplication::sendEvent(receiver, &preedit);
        QTest::keyClick(&window, Qt::Key_Return);
        QCOMPARE(exits, 0);
        QVERIFY(router.transientInsert());
        QInputMethodEvent commit;
        commit.setCommitString("confirmed");
        QCoreApplication::sendEvent(receiver, &commit);
        QTest::keyClick(&window, Qt::Key_Return);
        QCOMPARE(exits, 1);
    }
    void compositionAlreadyActiveOnEntry() {
        view->setProperty("ferricMode", "normal");
        focus("multi");
        auto *receiver = window.activeFocusItem();
        QVERIFY(receiver);
        QInputMethodEvent preedit("composing", {});
        QCoreApplication::sendEvent(receiver, &preedit);
        enter();
        QTest::keyClick(&window, Qt::Key_Return);
        QCOMPARE(exits, 0);
        QVERIFY(router.transientInsert());
        QInputMethodEvent commit;
        commit.setCommitString("confirmed");
        QCoreApplication::sendEvent(receiver, &commit);
        QTest::keyClick(&window, Qt::Key_Return);
        QCOMPARE(exits, 1);
    }
    void destroyedCompositionReceiver() {
        focus("multi");
        {
            QObject receiver;
            QInputMethodEvent preedit("composing", {});
            QCoreApplication::sendEvent(&receiver, &preedit);
        }
        // A destroyed receiver cannot retain an active preedit interaction.
        QTest::keyClick(&window, Qt::Key_Return);
        QCOMPARE(exits, 1);
        QTRY_COMPARE(js("down").toInt(), 1);
        QTRY_COMPARE(js("up").toInt(), 1);
    }
    void invalidatedGesture() {
        focus("multi");
        QTest::keyPress(&window, Qt::Key_Return);
        // A new tab/page or newer user mode selection invalidates the exit.
        enter();
        QTest::keyRelease(&window, Qt::Key_Return);
        QCOMPARE(exits, 0);
        QVERIFY(router.transientInsert());
        QTRY_COMPARE(js("up").toInt(), 1);
    }
    void completionAfterRouterDeactivation() {
        focus("multi");
        QTest::keyPress(&window, Qt::Key_Return);
        // Browser chrome can take keyboard ownership during the interaction.
        router.setEnabled(false);
        QTest::keyRelease(&window, Qt::Key_Return);
        router.setEnabled(true);
        QCOMPARE(exits, 1);
        QTRY_COMPARE(js("down").toInt(), 1);
        QTRY_COMPARE(js("up").toInt(), 1);
    }
    void completionAfterWindowFocusChange() {
        focus("multi");
        QTest::keyPress(&window, Qt::Key_Return);
        QQuickWindow other;
        struct ReleaseObserver : QObject {
            int releases = 0;
            bool eventFilter(QObject *, QEvent *event) override {
                if (event->type() == QEvent::KeyRelease
                    && static_cast<QKeyEvent *>(event)->key() == Qt::Key_Return)
                    ++releases;
                return false;
            }
        } observer;
        other.installEventFilter(&observer);
        other.show();
        other.requestActivate();
        QTRY_COMPARE(QGuiApplication::focusWindow(), &other);
        QTest::keyRelease(&other, Qt::Key_Return);
        const int completed = exits;
        other.hide();
        window.requestActivate();
        QTRY_COMPARE(QGuiApplication::focusWindow(), &window);
        QCOMPARE(completed, 1);
        QCOMPARE(observer.releases, 1);
        QTRY_COMPARE(js("down").toInt(), 1);
        // Qt selected the new window as the native release receiver.
        QCOMPARE(js("up").toInt(), 0);
    }
    void escapeExitsBoth() {
        focus("single");
        js("window.escapes=0;document.addEventListener('keydown',e=>{if(e.key==='Escape')escapes++});true");
        for (bool persistent : {false, true}) {
            enter(persistent);
            QTest::keyClick(&window, Qt::Key_Escape);
            QVERIFY(!router.transientInsert());
            QVERIFY(!locked);
            QCOMPARE(js("escapes").toInt(), 0);
            QTest::keyClick(&window, Qt::Key_J);
        }
        QCOMPARE(navigations, 2);
    }
    void repeatWaitsForFinalRelease() {
        focus("multi");
        QTest::keyPress(&window, Qt::Key_Return);
        QKeyEvent repeatRelease(QEvent::KeyRelease, Qt::Key_Return, Qt::NoModifier, "\r", true);
        QKeyEvent repeatPress(QEvent::KeyPress, Qt::Key_Return, Qt::NoModifier, "\r", true);
        QCoreApplication::sendEvent(&window, &repeatRelease);
        QCoreApplication::sendEvent(&window, &repeatPress);
        QCOMPARE(exits, 0);
        QTest::keyRelease(&window, Qt::Key_Return);
        QCOMPARE(exits, 1);
        QTRY_COMPARE(js("down").toInt(), 2);
        // WebEngine suppresses the auto-repeat release; only final key-up is DOM-visible.
        QTRY_COMPARE(js("up").toInt(), 1);
    }
    void keypadEnter() {
        focus("multi");
        QTest::keyClick(&window, Qt::Key_Enter, Qt::KeypadModifier);
        QCOMPARE(exits, 1);
        QTRY_COMPARE(js("down").toInt(), 1);
        QTRY_COMPARE(js("up").toInt(), 1);
    }
    void focusObserver() {
        const QString source = productionBrowserScript();
        // The same observer/probe resource used in Ferric's isolated world.
        const QString observer = js(source + ";focusObserverSource()").toString();
        const QString probe = js("focusProbe()").toString();
        QVERIFY(!observer.isEmpty());
        js(observer);
        focus("single");
        QVariantMap first = js(probe).toMap();
        QVERIFY(first.value("editable").toBool());
        QVERIFY(first.value("context").toInt() > 0);
        QCOMPARE(js(probe).toMap().value("context"), first.value("context"));
        focus("multi");
        QVariantMap second = js(probe).toMap();
        QVERIFY(second.value("context") != first.value("context"));
        QVERIFY(second.value("sequence").toInt() > first.value("sequence").toInt());
        const int notices = view->property("focusNotifications").toInt();
        js("document.getElementById('multi').remove();true");
        QTRY_VERIFY(view->property("focusNotifications").toInt() > notices);
        QVariantMap removed = js("window.__ferric_browserFocusState").toMap();
        QVERIFY(!removed.value("editable").toBool());
        QCOMPARE(removed.value("context").toInt(), 0);
        QVERIFY(removed.value("sequence").toInt() > second.value("sequence").toInt());
        focus("single");
        QVERIFY(js(probe).toMap().value("editable").toBool());
        const int ancestorNotices = view->property("focusNotifications").toInt();
        js("document.querySelector('form').remove();true");
        QTRY_VERIFY(view->property("focusNotifications").toInt() > ancestorNotices);
        QVERIFY(!js("window.__ferric_browserFocusState.editable").toBool());
    }
    // T002: focus through the production isolated-world Hint pipeline, then
    // send original Qt input through the production key router.
    void shadowHintFocusAcceptsOriginalText() {
        browserJs("{const host=document.createElement('div');host.id='shadow-host';host.style='position:fixed;left:0;top:0;width:240px;height:80px;z-index:100';document.body.prepend(host);host.attachShadow({mode:'open'}).innerHTML='<input id=shadow-input aria-label=Shadow style=width:180px;height:32px>'; }true");
        QVariantMap snapshot;
        QTRY_VERIFY(!(snapshot=browserJs(browserScript("hintCollector('inputs')")).toMap()).value("candidates").toList().isEmpty());
        QVariantMap selectedCandidate;
        for (const auto &value:snapshot.value("candidates").toList()) {
            const auto candidate=value.toMap();
            if (candidate.value("text").toString()=="Shadow")
                selectedCandidate=candidate;
        }
        QVERIFY(selectedCandidate.value("element_id").toInt()>0);
        const QString json=QString::fromUtf8(
            QJsonDocument::fromVariant(selectedCandidate).toJson(QJsonDocument::Compact));
        const QString fresh=browserScript("hintFresh("+json+")");
        QVERIFY(browserJs(fresh).toMap().value("visible").toBool());
        QVERIFY(browserJs(browserScript("hintFocus("+json+")")).toBool());
        enter();
        view->forceActiveFocus();
        QTest::keyClick(&window, Qt::Key_A);
        QTRY_COMPARE(browserJs("document.getElementById('shadow-host').shadowRoot.getElementById('shadow-input').value").toString(),QString("a"));
        QCOMPARE(exits,0);
        browserJs(browserScript("hintStopTracking()"));
    }
    // T011: exercise the production adapter against WebEngine's actual Quick
    // target. The control reacts to pointerdown only; semantic click is not a
    // fallback and each native transport event is observed once.
    void pointerOnlyControlReceivesOneNativeClick() {
        normal();
        browserJs("window.pointerTrace=[];const target=document.createElement('div');"
                  "target.id='pointer-only';target.setAttribute('role','button');"
                  "target.setAttribute('aria-label','Pointer only');"
                  "target.style='position:fixed;left:180px;top:140px;width:180px;height:50px;z-index:100';"
                  "for(const type of ['pointerdown','mousedown','pointerup','mouseup','click'])"
                  "target.addEventListener(type,event=>{pointerTrace.push({type,trusted:event.isTrusted,"
                  "activation:navigator.userActivation.isActive});if(type==='pointerdown')"
                  "target.dataset.activated=String(Number(target.dataset.activated||0)+1);});"
                  "document.body.append(target);true");
        QVariantMap snapshot;
        QTRY_VERIFY(!(snapshot=browserJs(browserScript("hintCollector('buttons')")).toMap())
                         .value("candidates").toList().isEmpty());
        QVariantMap candidate;
        for (const auto &value : snapshot.value("candidates").toList()) {
            const auto item = value.toMap();
            if (item.value("text").toString() == "Pointer only") candidate = item;
        }
        QVERIFY(candidate.value("element_id").toInt() > 0);
        const QString candidateJson = QString::fromUtf8(
            QJsonDocument::fromVariant(candidate).toJson(QJsonDocument::Compact));
        const QString preflightScript = browserScript(
            "hintPointerPreflight(" + candidateJson + ",640,480)");
        const QVariantMap preflight = browserJs(preflightScript).toMap();
        QVERIFY2(preflight.value("ok").toBool(), "retained pointer control should pass preflight");

        FerricPagePointerAdapter adapter;
        adapter.setTargetWindow(&window);
        adapter.setTargetItem(view);
        adapter.setEnabled(true);
        QSignalSpy acknowledgements(&adapter, &FerricPagePointerAdapter::dispatchAcknowledged);
        adapter.dispatch("pointer-request-1", "pointer-session-1",
                         adapter.surfaceSerial(), adapter.surfaceRevision(),
                         preflight.value("x").toDouble(), preflight.value("y").toDouble(),
                         "left");
        QCOMPARE(acknowledgements.size(), 1);
        QCOMPARE(acknowledgements.first().at(4).toString(), QString("delivered"));
        QTRY_COMPARE(browserJs("document.getElementById('pointer-only').dataset.activated").toString(),
                     QString("1"));
        const QVariantList trace = browserJs("pointerTrace").toList();
        qInfo().noquote() << "T014 pointer event evidence:"
                          << QJsonDocument::fromVariant(trace).toJson(QJsonDocument::Compact);
        QCOMPARE(trace.size(), 5);
        QCOMPARE(trace.at(0).toMap().value("type").toString(), QString("pointerdown"));
        QCOMPARE(trace.at(1).toMap().value("type").toString(), QString("mousedown"));
        QCOMPARE(trace.at(2).toMap().value("type").toString(), QString("pointerup"));
        QCOMPARE(trace.at(3).toMap().value("type").toString(), QString("mouseup"));
        QCOMPARE(trace.at(4).toMap().value("type").toString(), QString("click"));
        QVERIFY(trace.at(0).toMap().contains("trusted"));
        QVERIFY(trace.at(0).toMap().contains("activation"));
        adapter.setEnabled(false);
        browserJs(browserScript("hintStopTracking()"));
    }
    // T046: one production-adapter request mirrors Qt's public
    // Press-Release-Press-DoubleClick-Release sequence and exposes the actual
    // WebEngine DOM order rather than a two-click JS substitute.
    void pointerAdapterInvalidation_data() {
        QTest::addColumn<QString>("invalidation");
        QTest::addColumn<bool>("afterPress");
        for (const auto &kind : {"blocked", "disabled", "replacement", "resize", "deactivated"}) {
            QTest::newRow(qPrintable(QString("move-%1").arg(kind))) << QString(kind) << false;
            QTest::newRow(qPrintable(QString("press-%1").arg(kind))) << QString(kind) << true;
        }
    }
    void pointerAdapterInvalidation() {
        QFETCH(QString, invalidation);
        QFETCH(bool, afterPress);
        QQuickWindow surface;
        surface.resize(320, 240);
        QQuickItem target(surface.contentItem()), replacement(surface.contentItem());
        target.setWidth(320); target.setHeight(240);
        replacement.setWidth(320); replacement.setHeight(240);
        surface.show(); surface.requestActivate();
        QTRY_VERIFY(surface.isActive());
        FerricPagePointerAdapter adapter;
        adapter.setTargetItem(&target); adapter.setTargetWindow(&surface); adapter.setEnabled(true);
        struct Observer : QObject {
            QWindow *surface;
            std::function<void()> invalidate;
            bool afterPress;
            int presses = 0, releases = 0;
            bool changed = false;
            bool eventFilter(QObject *watched, QEvent *event) override {
                if (watched != surface) return false;
                if (event->type() == QEvent::MouseButtonPress) ++presses;
                if (event->type() == QEvent::MouseButtonRelease) ++releases;
                if (!changed && event->type() == (afterPress ? QEvent::MouseButtonPress : QEvent::MouseMove)) {
                    changed = true; invalidate();
                }
                return false;
            }
        } observer;
        observer.surface = &surface; observer.afterPress = afterPress;
        QQuickWindow other;
        observer.invalidate = [&] {
            if (invalidation == "blocked") adapter.setInputBlocked(true);
            else if (invalidation == "disabled") adapter.setEnabled(false);
            else if (invalidation == "replacement") adapter.setTargetItem(&replacement);
            else if (invalidation == "resize") target.setWidth(300);
            else { other.resize(100,100); other.show(); other.requestActivate(); QTest::qWait(30); }
        };
        surface.installEventFilter(&observer);
        QSignalSpy acknowledgements(&adapter, &FerricPagePointerAdapter::dispatchAcknowledged);
        const auto serial = adapter.surfaceSerial(), revision = adapter.surfaceRevision();
        adapter.dispatch("invalidated-request", "invalidated-session", serial, revision, 50, 50, "left");
        QVERIFY(observer.changed);
        QCOMPARE(observer.presses, afterPress ? 1 : 0);
        QCOMPARE(observer.releases, afterPress && invalidation != "replacement" ? 1 : 0);
        QCOMPARE(acknowledgements.size(), 1);
        QCOMPARE(acknowledgements.at(0).at(4).toString(), afterPress ? QString("uncertain") : QString("rejected"));
        // An invalidated request cannot restart with its captured surface stamp.
        adapter.setInputBlocked(false); target.setWidth(320); adapter.setTargetItem(&target);
        surface.requestActivate(); QTRY_VERIFY(surface.isActive());
        adapter.dispatch("invalidated-request", "invalidated-session", serial, revision, 50, 50, "left");
        QCOMPARE(observer.presses, afterPress ? 1 : 0);
        surface.removeEventFilter(&observer);
    }
    void pointerAdapterSceneMapping_data() {
        QTest::addColumn<QString>("change");
        QTest::addColumn<QString>("stage");
        QTest::newRow("stable-transformed-view") << QString("stable") << QString("move");
        for (const auto &kind : {"target-x", "target-y", "ancestor-x", "ancestor-y",
                                 "target-rotation", "ancestor-scale", "window-position",
                                 "window-size"}) {
            for (const auto &phase : {"move", "press", "release"})
                QTest::newRow(qPrintable(QString("%1-%2").arg(phase, kind)))
                    << QString(kind) << QString(phase);
        }
    }
    void pointerAdapterSceneMapping() {
        QFETCH(QString, change);
        QFETCH(QString, stage);
        struct Receiver : QQuickItem {
            explicit Receiver(QQuickItem *parent) : QQuickItem(parent) {
                setAcceptedMouseButtons(Qt::LeftButton);
            }
            int presses = 0, releases = 0;
            std::function<void()> afterPress, afterRelease;
            void mousePressEvent(QMouseEvent *event) override {
                ++presses; event->accept();
                if (afterPress) afterPress();
            }
            void mouseReleaseEvent(QMouseEvent *event) override {
                ++releases; event->accept();
                if (afterRelease) afterRelease();
            }
        };
        QQuickWindow surface;
        surface.resize(400, 240);
        Receiver background(surface.contentItem());
        background.setWidth(400); background.setHeight(240);
        QQuickItem parent(surface.contentItem());
        parent.setWidth(100); parent.setHeight(100);
        Receiver target(&parent);
        target.setWidth(100); target.setHeight(100);
        if (change == "stable") {
            parent.setPosition(QPointF(40, 20)); parent.setScale(1.2);
            target.setRotation(10);
        }
        surface.show(); surface.requestActivate(); QTRY_VERIFY(surface.isActive());
        FerricPagePointerAdapter adapter;
        adapter.setTargetItem(&target); adapter.setTargetWindow(&surface); adapter.setEnabled(true);
        adapter.setDoubleClickEnabled(stage == "release");
        bool changed = false;
        const auto mutate = [&] {
            if (changed || change == "stable") return;
            changed = true;
            if (change == "target-x") target.setX(200);
            else if (change == "target-y") target.setY(150);
            else if (change == "ancestor-x") parent.setX(200);
            else if (change == "ancestor-y") parent.setY(150);
            // These preserve the selected centre point, so checking only that
            // point would miss the changed scene transform.
            else if (change == "target-rotation") target.setRotation(90);
            else if (change == "ancestor-scale") parent.setScale(2);
            else if (change == "window-position")
                surface.setPosition(surface.position() + QPoint(40, 25));
            else if (change == "window-size") surface.resize(350, 220);
        };
        struct Observer : QObject {
            std::function<void()> onMove;
            bool eventFilter(QObject *, QEvent *event) override {
                if (event->type() == QEvent::MouseMove) onMove();
                return false;
            }
        } observer;
        observer.onMove = stage == "move" ? std::function<void()>(mutate) : [] {};
        if (stage == "press") target.afterPress = mutate;
        if (stage == "release") target.afterRelease = mutate;
        surface.installEventFilter(&observer);
        QSignalSpy acknowledgements(&adapter, &FerricPagePointerAdapter::dispatchAcknowledged);
        const auto serial = adapter.surfaceSerial(), revision = adapter.surfaceRevision();
        adapter.dispatch("mapping-request", "mapping-session", serial, revision, 50, 50,
                         stage == "release" ? "double" : "left");
        const int expected = change == "stable" || stage != "move" ? 1 : 0;
        QCOMPARE(changed, change != "stable");
        QCOMPARE(background.presses, 0);
        QCOMPARE(background.releases, 0);
        QCOMPARE(target.presses, expected);
        QCOMPARE(target.releases, expected);
        QCOMPARE(acknowledgements.size(), 1);
        QCOMPARE(acknowledgements.first().at(4).toString(), change == "stable" ? QString("delivered")
                 : stage == "move" ? QString("rejected") : QString("uncertain"));
        adapter.dispatch("mapping-request", "mapping-session", serial, revision, 50, 50, "left");
        QCOMPARE(acknowledgements.size(), 2);
        QCOMPARE(acknowledgements.last().at(4).toString(), QString("rejected"));
        QCOMPARE(background.presses, 0);
        QCOMPARE(target.presses, expected);
        surface.removeEventFilter(&observer);
    }
    void pointerAdapterDoubleClickSequence() {
        normal();
        const QString documentUrl = browserJs("location.href").toString();
        browserJs("window.pointerTrace=[];window.pointerClicks=0;window.pointerDoubles=0;"
                  "const target=document.createElement('button');target.id='double-target';"
                  "target.setAttribute('aria-label','Double target');"
                  // Keep this away from T011's click location so Qt's window-wide
                  // click counter starts a fresh native sequence in the full suite.
                  "target.style='position:fixed;left:400px;top:350px;width:180px;height:50px;z-index:100';"
                  "for(const type of ['pointerdown','mousedown','pointerup','mouseup','click','dblclick'])"
                  "target.addEventListener(type,event=>{pointerTrace.push({type,detail:event.detail,"
                  "trusted:event.isTrusted,activation:navigator.userActivation.isActive});"
                  "if(type==='click')pointerClicks++;if(type==='dblclick')pointerDoubles++;});"
                  "document.body.append(target);true");
        QVariantMap snapshot;
        QTRY_VERIFY(!(snapshot=browserJs(browserScript("hintCollector('buttons')")).toMap())
                         .value("candidates").toList().isEmpty());
        QVariantMap candidate;
        for (const auto &value : snapshot.value("candidates").toList()) {
            const auto item = value.toMap();
            if (item.value("text").toString() == "Double target") candidate = item;
        }
        QVERIFY(candidate.value("element_id").toInt() > 0);
        const QString candidateJson = QString::fromUtf8(
            QJsonDocument::fromVariant(candidate).toJson(QJsonDocument::Compact));
        const QVariantMap preflight = browserJs(browserScript(
            "hintPointerPreflight(" + candidateJson + ",640,480)")).toMap();
        QVERIFY2(preflight.value("ok").toBool(), "retained double-click target should pass preflight");

        FerricPagePointerAdapter adapter;
        adapter.setTargetWindow(&window);
        adapter.setTargetItem(view);
        adapter.setEnabled(true);
        QVERIFY(!adapter.doubleClickEnabled());
        QSignalSpy acknowledgements(&adapter, &FerricPagePointerAdapter::dispatchAcknowledged);
        const QString serial = adapter.surfaceSerial();
        const QString revision = adapter.surfaceRevision();
        adapter.dispatch("double-request-unqualified", "double-session-1", serial, revision,
                         preflight.value("x").toDouble(), preflight.value("y").toDouble(),
                         "double");
        QCOMPARE(acknowledgements.size(), 1);
        QCOMPARE(acknowledgements.first().at(4).toString(), QString("unavailable"));
        QCOMPARE(browserJs("pointerTrace.length").toInt(), 0);

        adapter.setDoubleClickEnabled(true);
        adapter.dispatch("double-request-1", "double-session-1", serial, revision,
                         preflight.value("x").toDouble(), preflight.value("y").toDouble(),
                         "double");
        QCOMPARE(acknowledgements.size(), 2);
        QCOMPARE(acknowledgements.at(1).at(4).toString(), QString("delivered"));
        QTRY_VERIFY(browserJs("pointerTrace.some(event=>event.type==='dblclick')").toBool());
        const QVariantList trace = browserJs("pointerTrace").toList();
        qInfo().noquote() << "T046 Qt/WebEngine double-click event evidence:"
                          << QJsonDocument::fromVariant(trace).toJson(QJsonDocument::Compact);
        QStringList types;
        for (const auto &value : trace) {
            const QVariantMap event = value.toMap();
            types.append(event.value("type").toString());
            QVERIFY(event.value("trusted").toBool());
            QVERIFY(event.value("activation").toBool());
        }
        QCOMPARE(types, QStringList({"pointerdown", "mousedown", "pointerup", "mouseup",
                                     "click", "pointerdown", "mousedown", "pointerup",
                                     "mouseup", "click", "dblclick"}));
        QCOMPARE(browserJs("pointerClicks").toInt(), 2);
        QCOMPARE(browserJs("pointerDoubles").toInt(), 1);
        QCOMPARE(browserJs("location.href").toString(), documentUrl);

        adapter.dispatch("double-request-1", "double-session-1", serial, revision,
                         preflight.value("x").toDouble(), preflight.value("y").toDouble(),
                         "double");
        QCOMPARE(acknowledgements.size(), 3);
        QCOMPARE(acknowledgements.last().at(4).toString(), QString("rejected"));
        QCOMPARE(browserJs("pointerTrace.length").toInt(), trace.size());
        adapter.setEnabled(false);
        browserJs(browserScript("hintStopTracking()"));
    }
    void ordinaryText() {
        for (bool persistent : {false, true}) {
            enter(persistent);
            focus("single");
            for (const auto character : QString("iiIjk")) {
                QKeyEvent press(QEvent::KeyPress, character.toUpper().unicode(), Qt::NoModifier, QString(character));
                QKeyEvent release(QEvent::KeyRelease, character.toUpper().unicode(), Qt::NoModifier, QString(character));
                QCoreApplication::sendEvent(&window, &press);
                QCoreApplication::sendEvent(&window, &release);
            }
            QTRY_VERIFY(js("document.getElementById('single').value.endsWith('iiIjk')").toBool());
            QCOMPARE(exits, 0);
            QCOMPARE(navigations, 0);
            QCOMPARE(router.transientInsert(), !persistent);
        }
    }
    void nativeReleaseAfterLostCommandRelease() {
        view->setProperty("ferricMode", "normal");
        locked = false;
        QTest::keyPress(&window, Qt::Key_J); // Normal command; its release is lost.
        QCOMPARE(navigations, 1);
        enter(true);
        focus("multi");
        QTest::keyClick(&window, Qt::Key_J);
        QTRY_COMPARE(js("document.getElementById('multi').value").toString(), "j");
        QTRY_VERIFY(js("keyups.includes('j')").toBool());
    }
    void freshGestureAfterLostRelease() {
        focus("multi");
        QTest::keyPress(&window, Qt::Key_Return);
        // The release can occur in another application after deactivation.
        enter();
        QTest::keyClick(&window, Qt::Key_Return);
        QCOMPARE(exits, 1);
        QTRY_COMPARE(js("down").toInt(), 2);
        QTRY_COMPARE(js("up").toInt(), 1);
    }
    void modifiedGestureAfterLostRelease_data() {
        QTest::addColumn<int>("modifiers");
        for (int modifier : {int(Qt::ShiftModifier), int(Qt::ControlModifier), int(Qt::AltModifier), int(Qt::MetaModifier)})
            QTest::newRow(QByteArray::number(modifier)) << modifier;
    }
    void modifiedGestureAfterLostRelease() {
        QFETCH(int, modifiers);
        focus("multi");
        QTest::keyPress(&window, Qt::Key_Return); // Its release is lost.
        QTest::keyClick(&window, Qt::Key_Return, Qt::KeyboardModifiers(modifiers));
        QCOMPARE(exits, 0);
        QVERIFY(router.transientInsert());
        QTRY_COMPARE(js("down").toInt(), 2);
        QTRY_COMPARE(js("up").toInt(), 1);
        // The next ordinary gesture must still finish the interaction.
        QTest::keyClick(&window, Qt::Key_Return);
        QCOMPARE(exits, 1);
    }
    void compositionGestureAfterLostRelease() {
        focus("multi");
        QTest::keyPress(&window, Qt::Key_Return); // Its release is lost.
        auto *receiver = window.activeFocusItem();
        QVERIFY(receiver);
        QInputMethodEvent preedit("composing", {});
        QCoreApplication::sendEvent(receiver, &preedit);
        QTest::keyClick(&window, Qt::Key_Return);
        QCOMPARE(exits, 0);
        QVERIFY(router.transientInsert());
        QInputMethodEvent commit;
        commit.setCommitString("confirmed");
        QCoreApplication::sendEvent(receiver, &commit);
        QTest::keyClick(&window, Qt::Key_Return);
        QCOMPARE(exits, 1);
    }
    void navigation() {
        focus("navigate");
        QTest::keyPress(&window, Qt::Key_Return);
        QTRY_VERIFY(view->property("url").toUrl().path().endsWith("next.html"));
        view->setProperty("ferricMode", "normal");
        view->setProperty("ferricEpoch", router.inputEpoch() + 1);
        QTest::keyRelease(&window, Qt::Key_Return);
        QCOMPARE(exits, 0);
    }

    void boundedDragProbeAndCleanup() {
        normal();
        const int loaded = view->property("loaded").toInt();
        QUrl probeUrl = QUrl::fromLocalFile(FERRIC_DRAG_FIXTURE_PATH);
        probeUrl.setQuery(QString::number(++fixtureGeneration));
        view->setProperty("url", probeUrl);
        QTRY_VERIFY(view->property("loaded").toInt() > loaded);
        QTRY_VERIFY(js("!!window.dragProbe").toBool());

        FerricPagePointerAdapter adapter;
        adapter.setTargetWindow(&window);
        adapter.setTargetItem(view);
        QSignalSpy surfaceChanges(&adapter, &FerricPagePointerAdapter::surfaceChanged);

        const auto resetTrace = [this] {
            js("dragProbe.events=[];dragProbe.draws=0;dragProbe.dropCommits=0;"
               "document.getElementById('range').value='10';true");
        };
        const auto trace = [this] { return js("dragProbe.events").toList(); };
        const auto reportTrace = [this, &trace](const char *label) {
            qInfo().noquote() << label << QJsonDocument::fromVariant(trace()).toJson(
                                      QJsonDocument::Compact);
        };

        // Plain pointer-driven in-view widget: endpoints are fixed before the
        // press; the only path is 12 straight-line moves over 200 ms.
        resetTrace();
        DragProbeTransaction widget(&adapter, view, &window);
        QVERIFY(widget.begin(QPointF(60, 60), QPointF(205, 90)));
        QVERIFY(widget.moveStraight());
        QVERIFY(widget.finish());
        QTRY_VERIFY(js("dragProbe.events.some(e=>e.type==='pointerup')").toBool());
        const QVariantList widgetTrace = trace();
        reportTrace("T047 pointer-driven widget trace:");
        QVERIFY(std::any_of(widgetTrace.cbegin(), widgetTrace.cend(), [](const QVariant &entry) {
            const QVariantMap event = entry.toMap();
            return event.value("type").toString() == "pointerdown"
                && event.value("target").toString() == "pointer-pad"
                && event.value("trusted").toBool();
        }));
        QVERIFY(std::any_of(widgetTrace.cbegin(), widgetTrace.cend(), [](const QVariant &entry) {
            const QVariantMap event = entry.toMap();
            return event.value("type").toString() == "pointerup"
                && event.value("target").toString() == "pointer-pad"
                && event.value("trusted").toBool();
        }));
        QVERIFY(std::any_of(widgetTrace.cbegin(), widgetTrace.cend(), [](const QVariant &entry) {
            return entry.toMap().value("type").toString() == "pointermove";
        }));

        // Pointer capture is observed independently of ordinary hit-tested
        // widgets. A captured pointer remains on the capture pad off-element.
        resetTrace();
        DragProbeTransaction captured(&adapter, view, &window);
        QVERIFY(captured.begin(QPointF(60, 160), QPointF(500, 400)));
        QVERIFY(captured.moveStraight());
        QVERIFY(captured.finish());
        QTRY_VERIFY(js("dragProbe.events.some(e=>e.type==='lostpointercapture')").toBool());
        const QVariantList captureTrace = trace();
        reportTrace("T047 captured pointer trace:");
        QVERIFY(std::any_of(captureTrace.cbegin(), captureTrace.cend(), [](const QVariant &entry) {
            return entry.toMap().value("type").toString() == "gotpointercapture";
        }));
        QVERIFY(std::any_of(captureTrace.cbegin(), captureTrace.cend(), [](const QVariant &entry) {
            const QVariantMap event = entry.toMap();
            return event.value("type").toString() == "pointermove"
                && event.value("target").toString() == "capture-pad"
                && event.value("captureTarget").toBool();
        }));

        // The canvas-like surface and native range input retain their own
        // observable behaviour; neither result is generalized to all sites.
        resetTrace();
        DragProbeTransaction canvas(&adapter, view, &window);
        QVERIFY(canvas.begin(QPointF(300, 40), QPointF(500, 160)));
        QVERIFY(canvas.moveStraight());
        QVERIFY(canvas.finish());
        QTRY_VERIFY(js("dragProbe.draws>0").toBool());
        qInfo() << "T047 canvas-like pointermove count:" << js("dragProbe.draws").toInt();

        resetTrace();
        DragProbeTransaction slider(&adapter, view, &window);
        QVERIFY(slider.begin(QPointF(48, 255), QPointF(244, 255)));
        QVERIFY(slider.moveStraight());
        QVERIFY(slider.finish());
        QTRY_VERIFY(js("dragProbe.events.some(e=>e.type==='pointerup')").toBool());
        reportTrace("T047 range input trace:");
        const int rangeValue = js("Number(document.getElementById('range').value)").toInt();
        QVERIFY(rangeValue >= 90);
        qInfo() << "T047 range value after drag:" << rangeValue;

        // Native HTML drag/drop is a separate category with a visible commit
        // counter; the test records whether release caused a local side effect.
        resetTrace();
        DragProbeTransaction htmlDrag(&adapter, view, &window);
        QVERIFY(htmlDrag.begin(QPointF(80, 350), QPointF(450, 360)));
        QVERIFY(htmlDrag.moveStraight());
        QVERIFY(htmlDrag.finish());
        QTest::qWait(100);
        const int dropCommits = js("dragProbe.dropCommits").toInt();
        QVERIFY(dropCommits >= 0 && dropCommits <= 1);
        QVERIFY(js("dragProbe.events.some(e=>e.type==='dragstart')").toBool());
        QCOMPARE(js("dragProbe.events.filter(e=>e.type==='pointercancel').length").toInt(), 1);
        QCOMPARE(js("dragProbe.events.filter(e=>e.type==='drop').length").toInt(), 0);
        QCOMPARE(dropCommits, 0);
        reportTrace("T049 native HTML drag/drop trace:");
        qInfo() << "T049 local drop commits after release:" << dropCommits;

        const auto verifyOneCleanupRelease = [this](const char *label) {
            QTRY_COMPARE(js("dragProbe.events.filter(e=>e.type==='pointerup').length").toInt(), 1);
            QTRY_COMPARE(js("dragProbe.events.filter(e=>e.type==='mouseup').length").toInt(), 1);
            QVERIFY(js("dragProbe.events.filter(e=>e.type==='pointerdown').length===1").toBool());
            QVERIFY(js("dragProbe.events.filter(e=>e.type==='pointerup'||e.type==='mouseup')"
                       ".every(e=>e.trusted&&e.buttons===0)").toBool());
            qInfo() << label << "pointerdown=1 pointerup=1 mouseup=1; no drop was retried";
        };

        // Escape, prompt ownership, deactivation and watchdog expiry each
        // clear the test-only held state and release only to the live owner.
        resetTrace();
        DragProbeTransaction escape(&adapter, view, &window);
        QVERIFY(escape.begin(QPointF(48, 255), QPointF(244, 255)));
        QVERIFY(escape.moveStraight());
        QVERIFY(js("Number(document.getElementById('range').value)").toInt() >= 90);
        QTest::keyClick(&window, Qt::Key_Escape);
        QCOMPARE(escape.cancelReason(), QStringLiteral("escape"));
        QVERIFY(!escape.active());
        QVERIFY(!escape.held());
        QVERIFY(escape.cleanupReleaseSent());
        verifyOneCleanupRelease("T049 Escape cleanup:");
        const int escapedRangeValue = js("Number(document.getElementById('range').value)").toInt();
        QVERIFY(escapedRangeValue >= 90);
        QVERIFY(js("dragProbe.events.some(e=>e.type==='change')").toBool());
        qInfo() << "T049 cancellation kept the already changed range value; no rollback:"
                << escapedRangeValue;

        resetTrace();
        DragProbeTransaction prompt(&adapter, view, &window);
        QVERIFY(prompt.begin(QPointF(60, 60), QPointF(205, 90)));
        adapter.setInputBlocked(true);
        QCOMPARE(prompt.cancelReason(), QStringLiteral("surface-changed"));
        QVERIFY(!prompt.held());
        QVERIFY(prompt.cleanupReleaseSent());
        verifyOneCleanupRelease("T049 prompt cleanup:");
        adapter.setInputBlocked(false);

        resetTrace();
        DragProbeTransaction deactivated(&adapter, view, &window);
        QVERIFY(deactivated.begin(QPointF(60, 60), QPointF(205, 90)));
        QEvent deactivate(QEvent::WindowDeactivate);
        QCoreApplication::sendEvent(&window, &deactivate);
        QCOMPARE(deactivated.cancelReason(), QStringLiteral("surface-changed"));
        QVERIFY(!deactivated.held());
        QVERIFY(deactivated.cleanupReleaseSent());
        verifyOneCleanupRelease("T049 deactivation cleanup:");
        window.requestActivate();
        QTRY_COMPARE(QGuiApplication::focusWindow(), &window);

        resetTrace();
        DragProbeTransaction timedOut(&adapter, view, &window);
        QVERIFY(timedOut.begin(QPointF(60, 60), QPointF(205, 90)));
        QTRY_VERIFY_WITH_TIMEOUT(!timedOut.active(), 1500);
        QCOMPARE(timedOut.cancelReason(), QStringLiteral("watchdog"));
        QVERIFY(!timedOut.held());
        QVERIFY(timedOut.cleanupReleaseSent());
        verifyOneCleanupRelease("T049 watchdog cleanup:");
        qInfo() << "T047 surface-change signals during probe:" << surfaceChanges.size();

        // Destroying the original QQuickView invalidates both QPointers. The
        // transaction clears its held bit without sending into the replacement
        // view; this also verifies the replacement document stays untouched.
        auto original = std::make_unique<QQuickView>();
        original->engine()->rootContext()->setContextProperty("nativeRouter", &router);
        original->setSource(QUrl::fromLocalFile(files.filePath("view.qml")));
        QCOMPARE(original->status(), QQuickView::Ready);
        QQuickItem *originalPage = original->rootObject();
        originalPage->setProperty("url", probeUrl);
        original->show();
        original->requestActivate();
        QTRY_VERIFY(originalPage->property("loaded").toInt() > 0);
        originalPage->setProperty("probeDone", false);
        QMetaObject::invokeMethod(originalPage, "probe", Q_ARG(QVariant, QString("!!window.dragProbe")));
        QTRY_VERIFY(originalPage->property("probeDone").toBool());
        QVERIFY(originalPage->property("probeResult").toBool());
        FerricPagePointerAdapter teardownAdapter;
        teardownAdapter.setTargetWindow(original.get());
        teardownAdapter.setTargetItem(originalPage);
        auto teardown = std::make_unique<DragProbeTransaction>(
            &teardownAdapter, originalPage, original.get());
        QVERIFY(teardown->begin(QPointF(60, 60), QPointF(205, 90)));
        QTest::qWait(50);
        original.reset();
        QVERIFY(!teardown->active());
        QVERIFY(!teardown->held());
        QVERIFY(!teardown->cleanupReleaseSent());
        QVERIFY(teardown->cancelReason() == QStringLiteral("surface-changed"));

        auto replacement = std::make_unique<QQuickView>();
        replacement->engine()->rootContext()->setContextProperty("nativeRouter", &router);
        replacement->setSource(QUrl::fromLocalFile(files.filePath("view.qml")));
        QCOMPARE(replacement->status(), QQuickView::Ready);
        QQuickItem *replacementPage = replacement->rootObject();
        replacementPage->setProperty("url", probeUrl);
        replacement->show();
        replacement->requestActivate();
        QTRY_VERIFY(replacementPage->property("loaded").toInt() > 0);
        replacementPage->setProperty("probeDone", false);
        QMetaObject::invokeMethod(replacementPage, "probe",
                                  Q_ARG(QVariant, QString("dragProbe.events.length")));
        QTRY_VERIFY(replacementPage->property("probeDone").toBool());
        QCOMPARE(replacementPage->property("probeResult").toInt(), 0);
        teardownAdapter.setTargetWindow(replacement.get());
        teardownAdapter.setTargetItem(replacementPage);
        qInfo() << "T049 destroyed receiver cleanup: release sent ="
                << teardown->cleanupReleaseSent() << "; replacement DOM events ="
                << replacementPage->property("probeResult").toInt();
    }
};
int main(int argc, char **argv) {
    QtWebEngineQuick::initialize();
    QGuiApplication app(argc, argv);
    NativeInsertTest test;
    return QTest::qExec(&test, argc, argv);
}
#include "tst_native_insert.moc"
