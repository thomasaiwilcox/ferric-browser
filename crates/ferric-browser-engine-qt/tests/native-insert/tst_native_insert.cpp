#include "../../src/browser_key_router.h"
#include <QtCore/QFile>
#include <QtCore/QElapsedTimer>
#include <QtCore/QTemporaryDir>
#include <QtGui/QInputMethodEvent>
#include <QtQml/QQmlEngine>
#include <QtQml/QQmlContext>
#include <QtQuick/QQuickItem>
#include <QtQuick/QQuickView>
#include <QtTest/QSignalSpy>
#include <QtTest/QTest>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>

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
    bool locked = false;
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
    void enter(bool persistent = false) {
        locked = persistent;
        view->setProperty("ferricMode", persistent ? "insert-locked" : "insert");
        view->setProperty("ferricEpoch", router.inputEpoch() + 1);
    }
    void focus(const QString &id) {
        js("document.getElementById('" + id + "').focus();true");
        view->forceActiveFocus();
    }
private slots:
    void initTestCase() {
        QVERIFY(files.isValid());
        QFile html(files.filePath("fixture.html"));
        QVERIFY(html.open(QIODevice::WriteOnly));
        html.write(R"(<form onsubmit="event.preventDefault();window.submits++">
<input id="single"><button>submit</button></form><textarea id="multi"></textarea>
<input id="complete"><div id="rich" contenteditable="true"></div>
<input id="remove"><input id="navigate">
<script>
window.down=0;window.up=0;window.submits=0;window.shift=false;window.keyups=[];
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
        QFile qml(files.filePath("view.qml"));
        QVERIFY(qml.open(QIODevice::WriteOnly));
        qml.write(R"(import QtQuick
import QtWebEngine
WebEngineView {
    id: fixtureView
    property string ferricMode: "normal"
    property int ferricEpoch: 0
    Binding { target: nativeRouter; property: "transientInsert"; value: fixtureView.ferricMode === "insert" }
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
        connect(&router, &FerricBrowserKeyRouter::keyPressed, this,
                [this](const QString &, int key, int, bool) {
            if (key == Qt::Key_Escape) {
                view->setProperty("ferricMode", "normal");
                locked = false;
                view->setProperty("ferricEpoch", router.inputEpoch() + 1);
                router.acceptCurrentEvent();
            } else if (!router.transientInsert() && !locked && key == Qt::Key_J) {
                ++navigations;
                router.acceptCurrentEvent();
            }
        });
    }
    void init() {
        exits = 0;
        navigations = 0;
        locked = false;
        const int loaded = view->property("loaded").toInt();
        QUrl url = QUrl::fromLocalFile(files.filePath("fixture.html"));
        url.setQuery(QString::number(++fixtureGeneration));
        view->setProperty("url", url);
        QTRY_VERIFY(view->property("loaded").toInt() > loaded);
        QTRY_VERIFY(js("typeof window.down !== 'undefined'").toBool());
        js("down=0;up=0;submits=0;true");
        enter();
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
        QFile script(FERRIC_SCRIPT_PATH);
        QVERIFY(script.open(QIODevice::ReadOnly));
        QString source = QString::fromUtf8(script.readAll());
        source.remove(".pragma library");
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
};
int main(int argc, char **argv) {
    QtWebEngineQuick::initialize();
    QGuiApplication app(argc, argv);
    NativeInsertTest test;
    return QTest::qExec(&test, argc, argv);
}
#include "tst_native_insert.moc"
