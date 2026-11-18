#include "../../src/browser_key_router.h"
#include <QGuiApplication>
#include <QKeyEvent>
#include <QPointer>
#include <QQuickWindow>
#include <QtTest>
#include <memory>
#include <functional>

class ObservedWindow final : public QQuickWindow {
public:
    std::function<void(QEvent *)> observe;
    bool event(QEvent *event) override {
        if (observe && (event->type() == QEvent::KeyPress || event->type() == QEvent::KeyRelease)) {
            observe(event);
            return true;
        }
        return QQuickWindow::event(event);
    }
};

class CallbackFilter final : public QObject {
public:
    std::function<void(QEvent *)> callback;
    bool eventFilter(QObject *, QEvent *event) override {
        callback(event);
        return false;
    }
};

class NativeInputLifetime final : public QObject {
    Q_OBJECT
private slots:
    void routerDestroyedDuringKeyAdmission() {
        QQuickWindow window;
        window.show(); window.requestActivate();
        QVERIFY(QTest::qWaitForWindowActive(&window));
        auto *router = new FerricBrowserKeyRouter;
        QPointer<FerricBrowserKeyRouter> alive(router);
        router->setTargetWindow(&window); router->setEnabled(true);
        connect(router, &FerricBrowserKeyRouter::keyPressed, this, [router] { delete router; });
        QKeyEvent event(QEvent::KeyPress, Qt::Key_P, Qt::NoModifier, "p");
        event.setAccepted(false);
        QCoreApplication::sendEvent(&window, &event);
        QVERIFY(alive.isNull());
        QVERIFY(event.isAccepted());
        ObservedWindow replacement;
        int leaked = 0;
        replacement.observe = [&](QEvent *) { ++leaked; };
        QKeyEvent release(QEvent::KeyRelease, Qt::Key_P, Qt::NoModifier, "p");
        release.setAccepted(false);
        QCoreApplication::sendEvent(&replacement, &release);
        QVERIFY(release.isAccepted());
        QCOMPARE(leaked, 0);
    }
    void windowDestroyedDuringKeyAdmission() {
        auto window = std::make_unique<QQuickWindow>();
        window->show(); window->requestActivate();
        QVERIFY(QTest::qWaitForWindowActive(window.get()));
        FerricBrowserKeyRouter router;
        router.setTargetWindow(window.get()); router.setEnabled(true);
        connect(&router, &FerricBrowserKeyRouter::keyPressed, this, [&window] { window.reset(); });
        QKeyEvent event(QEvent::KeyPress, Qt::Key_P, Qt::NoModifier, "p");
        event.setAccepted(false);
        QCoreApplication::sendEvent(window.get(), &event);
        QVERIFY(!window);
        QVERIFY(event.isAccepted());
        ObservedWindow replacement;
        int leaked = 0;
        replacement.observe = [&](QEvent *) { ++leaked; };
        QKeyEvent release(QEvent::KeyRelease, Qt::Key_P, Qt::NoModifier, "p");
        release.setAccepted(false);
        QCoreApplication::sendEvent(&replacement, &release);
        QVERIFY(release.isAccepted());
        QCOMPARE(leaked, 0);
    }
    void routerDestroyedDuringClipboardAdmission() {
        QQuickWindow window;
        window.show(); window.requestActivate();
        QVERIFY(QTest::qWaitForWindowActive(&window));
        auto *router = new FerricBrowserKeyRouter;
        QPointer<FerricBrowserKeyRouter> alive(router);
        router->setTargetWindow(&window); router->setEnabled(true);
        connect(router, &FerricBrowserKeyRouter::clipboardShortcutOverride, this, [router] { delete router; });
        QKeyEvent event(QEvent::ShortcutOverride, Qt::Key_C, Qt::ControlModifier, "c");
        event.setAccepted(false);
        QCoreApplication::sendEvent(&window, &event);
        QVERIFY(alive.isNull());
        QVERIFY(event.isAccepted());
    }
    void windowDestroyedDuringClipboardAdmission() {
        auto window = std::make_unique<QQuickWindow>();
        window->show(); window->requestActivate();
        QVERIFY(QTest::qWaitForWindowActive(window.get()));
        FerricBrowserKeyRouter router;
        router.setTargetWindow(window.get()); router.setEnabled(true);
        connect(&router, &FerricBrowserKeyRouter::clipboardShortcutOverride, this, [&window] { window.reset(); });
        QKeyEvent event(QEvent::ShortcutOverride, Qt::Key_C, Qt::ControlModifier, "c");
        event.setAccepted(false);
        QCoreApplication::sendEvent(window.get(), &event);
        QVERIFY(!window);
        QVERIFY(event.isAccepted());
    }
    void routerDestroyedDuringNativeSearchDispatch_data() {
        QTest::addColumn<bool>("duringRelease");
        QTest::newRow("press") << false;
        QTest::newRow("release") << true;
    }
    void routerDestroyedDuringNativeSearchDispatch() {
        QFETCH(bool, duringRelease);
        ObservedWindow window;
        QQuickItem view(window.contentItem());
        view.setSize(QSizeF(100, 100));
        window.show(); window.requestActivate();
        QVERIFY(QTest::qWaitForWindowActive(&window));
        auto *router = new FerricBrowserKeyRouter;
        QPointer<FerricBrowserKeyRouter> alive(router);
        router->setTargetWindow(&window); router->setEnabled(true);
        int presses = 0, releases = 0;
        window.observe = [&](QEvent *event) {
            if (event->type() == QEvent::KeyPress) {
                ++presses;
                if (!duringRelease) delete router;
            } else {
                ++releases;
                if (duringRelease) delete router;
            }
        };
        QVERIFY(router->activateSearchResult(&view, false));
        QVERIFY(alive.isNull());
        QCOMPARE(presses, 1);
        QCOMPARE(releases, duringRelease ? 1 : 0);
    }
    void routerDestroyedDuringNativeSearchFocus() {
        QQuickWindow window;
        QQuickItem view(window.contentItem());
        view.setSize(QSizeF(100, 100));
        window.show(); window.requestActivate();
        QVERIFY(QTest::qWaitForWindowActive(&window));
        auto *router = new FerricBrowserKeyRouter;
        QPointer<FerricBrowserKeyRouter> alive(router);
        router->setTargetWindow(&window); router->setEnabled(true);
        connect(&view, &QQuickItem::activeFocusChanged, this, [&] {
            if (view.hasActiveFocus() && alive) delete router;
        });
        QVERIFY(!router->activateSearchResult(&view, false));
        QVERIFY(alive.isNull());
    }
    void nativeSearchDoesNotReleaseIntoReplacementFocus() {
        ObservedWindow window;
        QQuickItem original(window.contentItem()), replacement(window.contentItem());
        original.setSize(QSizeF(100, 100)); replacement.setSize(QSizeF(100, 100));
        window.show(); window.requestActivate();
        QVERIFY(QTest::qWaitForWindowActive(&window));
        FerricBrowserKeyRouter router;
        router.setTargetWindow(&window); router.setEnabled(true);
        int presses = 0, releases = 0;
        window.observe = [&](QEvent *event) {
            if (event->type() == QEvent::KeyPress) {
                ++presses;
                replacement.forceActiveFocus(Qt::OtherFocusReason);
            } else ++releases;
        };
        QVERIFY(router.activateSearchResult(&original, false));
        QCOMPARE(window.activeFocusItem(), &replacement);
        QCOMPARE(presses, 1);
        QCOMPARE(releases, 0);
    }
    void nestedAdmissionKeepsTheOuterBrowserDecision_data() {
        QTest::addColumn<bool>("acceptBeforeNested");
        QTest::newRow("before") << true;
        QTest::newRow("after") << false;
    }
    void nestedAdmissionKeepsTheOuterBrowserDecision() {
        QFETCH(bool, acceptBeforeNested);
        ObservedWindow window;
        window.show(); window.requestActivate();
        QVERIFY(QTest::qWaitForWindowActive(&window));
        FerricBrowserKeyRouter router;
        router.setTargetWindow(&window); router.setEnabled(true);
        int leakedOuterPresses = 0;
        window.observe = [&](QEvent *event) {
            if (static_cast<QKeyEvent *>(event)->key() == Qt::Key_P) ++leakedOuterPresses;
        };
        connect(&router, &FerricBrowserKeyRouter::keyPressed, this, [&](const QString &, int key) {
            if (key == Qt::Key_P) {
                if (acceptBeforeNested) router.acceptCurrentEvent();
                QKeyEvent nested(QEvent::KeyPress, Qt::Key_K, Qt::NoModifier, "k");
                QCoreApplication::sendEvent(&window, &nested);
                if (!acceptBeforeNested) router.acceptCurrentEvent();
            }
        });
        QKeyEvent event(QEvent::KeyPress, Qt::Key_P, Qt::NoModifier, "p");
        QCoreApplication::sendEvent(&window, &event);
        QCOMPARE(leakedOuterPresses, 0);
    }
    void nestedReleaseCannotAcceptOuterPress() {
        ObservedWindow window;
        window.show(); window.requestActivate();
        QVERIFY(QTest::qWaitForWindowActive(&window));
        FerricBrowserKeyRouter router;
        router.setTargetWindow(&window); router.setEnabled(true);
        int outerPresses = 0;
        window.observe = [&](QEvent *event) {
            if (event->type() == QEvent::KeyPress && static_cast<QKeyEvent *>(event)->key() == Qt::Key_P) ++outerPresses;
        };
        connect(&router, &FerricBrowserKeyRouter::keyReleased, this, [&] { router.acceptCurrentEvent(); });
        connect(&router, &FerricBrowserKeyRouter::keyPressed, this, [&](const QString &, int key) {
            if (key == Qt::Key_K) router.acceptCurrentEvent();
            if (key == Qt::Key_P) {
                QKeyEvent nested(QEvent::KeyRelease, Qt::Key_K, Qt::NoModifier, "k");
                QCoreApplication::sendEvent(&window, &nested);
            }
        });
        QKeyEvent held(QEvent::KeyPress, Qt::Key_K, Qt::NoModifier, "k");
        QCoreApplication::sendEvent(&window, &held);
        QKeyEvent outer(QEvent::KeyPress, Qt::Key_P, Qt::NoModifier, "p");
        QCoreApplication::sendEvent(&window, &outer);
        QCOMPARE(outerPresses, 1);
    }
    void nestedClipboardCannotOverrideOuterDecision() {
        ObservedWindow window;
        window.show(); window.requestActivate();
        QVERIFY(QTest::qWaitForWindowActive(&window));
        FerricBrowserKeyRouter router;
        router.setTargetWindow(&window); router.setEnabled(true);
        bool nested = false;
        connect(&router, &FerricBrowserKeyRouter::clipboardShortcutOverride, this, [&] {
            if (nested) return;
            nested = true;
            QKeyEvent inner(QEvent::ShortcutOverride, Qt::Key_C, Qt::ControlModifier, "c");
            QCoreApplication::sendEvent(&window, &inner);
            router.acceptCurrentEvent();
        });
        QKeyEvent outer(QEvent::ShortcutOverride, Qt::Key_C, Qt::ControlModifier, "c");
        outer.setAccepted(false);
        QCoreApplication::sendEvent(&window, &outer);
        QVERIFY(outer.isAccepted());
    }
    void nestedReleaseWaitsForPressOwnership_data() {
        QTest::addColumn<bool>("browserOwned");
        QTest::newRow("browser") << true;
        QTest::newRow("page") << false;
    }
    void nestedReleaseWaitsForPressOwnership() {
        QFETCH(bool, browserOwned);
        ObservedWindow window;
        window.show(); window.requestActivate();
        QVERIFY(QTest::qWaitForWindowActive(&window));
        FerricBrowserKeyRouter router;
        router.setTargetWindow(&window); router.setEnabled(true);
        QStringList received;
        window.observe = [&](QEvent *event) { received << (event->type() == QEvent::KeyPress ? "press" : "release"); };
        connect(&router, &FerricBrowserKeyRouter::keyReleased, this, [&] { received << "browser-release"; });
        connect(&router, &FerricBrowserKeyRouter::keyPressed, this, [&](const QString &, int key) {
            if (key != Qt::Key_P) return;
            QKeyEvent release(QEvent::KeyRelease, Qt::Key_P, Qt::NoModifier, "p");
            QCoreApplication::sendEvent(&window, &release);
            if (browserOwned) router.acceptCurrentEvent(); else router.pageOwnsCurrentEvent();
        });
        QKeyEvent press(QEvent::KeyPress, Qt::Key_P, Qt::NoModifier, "p");
        QCoreApplication::sendEvent(&window, &press);
        QCoreApplication::sendPostedEvents();
        QCOMPARE(received, browserOwned ? QStringList({"browser-release"}) : QStringList({"press", "release"}));
    }
    void nestedReleaseSeesPendingPressAcrossReleaseCallback() {
        ObservedWindow window;
        window.show(); window.requestActivate();
        QVERIFY(QTest::qWaitForWindowActive(&window));
        FerricBrowserKeyRouter router;
        router.setTargetWindow(&window); router.setEnabled(true);
        QStringList received;
        window.observe = [&](QEvent *event) { received << (event->type() == QEvent::KeyPress ? "press" : "release"); };
        connect(&router, &FerricBrowserKeyRouter::keyReleased, this, [&](int key) {
            if (key == Qt::Key_K) {
                QKeyEvent release(QEvent::KeyRelease, Qt::Key_P, Qt::NoModifier, "p");
                QCoreApplication::sendEvent(&window, &release);
            } else received << "browser-release";
        });
        connect(&router, &FerricBrowserKeyRouter::keyPressed, this, [&](const QString &, int key) {
            if (key == Qt::Key_K) router.acceptCurrentEvent();
            if (key == Qt::Key_P) {
                QKeyEvent release(QEvent::KeyRelease, Qt::Key_K, Qt::NoModifier, "k");
                QCoreApplication::sendEvent(&window, &release);
                router.acceptCurrentEvent();
            }
        });
        QKeyEvent held(QEvent::KeyPress, Qt::Key_K, Qt::NoModifier, "k");
        QCoreApplication::sendEvent(&window, &held);
        QKeyEvent press(QEvent::KeyPress, Qt::Key_P, Qt::NoModifier, "p");
        QCoreApplication::sendEvent(&window, &press);
        QCOMPARE(received, QStringList({"browser-release"}));
    }
    void newerPressSupersedesPendingGesture() {
        ObservedWindow window;
        window.show(); window.requestActivate();
        QVERIFY(QTest::qWaitForWindowActive(&window));
        FerricBrowserKeyRouter router;
        router.setTargetWindow(&window); router.setEnabled(true);
        int presses = 0, releases = 0, browserReleases = 0;
        window.observe = [&](QEvent *event) { if (event->type() == QEvent::KeyPress) ++presses; else ++releases; };
        connect(&router, &FerricBrowserKeyRouter::keyReleased, this, [&] { ++browserReleases; });
        int depth = 0;
        connect(&router, &FerricBrowserKeyRouter::keyPressed, this, [&](const QString &, int key) {
            if (key != Qt::Key_P) return;
            if (depth++ == 0) {
                QKeyEvent oldRelease(QEvent::KeyRelease, Qt::Key_P, Qt::NoModifier, "p");
                QCoreApplication::sendEvent(&window, &oldRelease);
                QKeyEvent newer(QEvent::KeyPress, Qt::Key_P, Qt::NoModifier, "p");
                QCoreApplication::sendEvent(&window, &newer);
            } else router.acceptCurrentEvent();
        });
        QKeyEvent oldPress(QEvent::KeyPress, Qt::Key_P, Qt::NoModifier, "p");
        QCoreApplication::sendEvent(&window, &oldPress);
        QCOMPARE(presses, 0);
        QCOMPARE(releases, 0);
        QCOMPARE(browserReleases, 0);
        QKeyEvent newRelease(QEvent::KeyRelease, Qt::Key_P, Qt::NoModifier, "p");
        QCoreApplication::sendEvent(&window, &newRelease);
        QCOMPARE(browserReleases, 1);
    }
    void nestedPagePressKeepsClipboardOwnership() {
        ObservedWindow window;
        window.show(); window.requestActivate();
        QVERIFY(QTest::qWaitForWindowActive(&window));
        FerricBrowserKeyRouter router;
        router.setTargetWindow(&window); router.setEnabled(true);
        connect(&router, &FerricBrowserKeyRouter::keyPressed, this, [&] { router.pageOwnsCurrentEvent(); });
        bool nested = false;
        connect(&router, &FerricBrowserKeyRouter::clipboardShortcutOverride, this, [&] {
            if (nested) return;
            nested = true;
            QKeyEvent press(QEvent::KeyPress, Qt::Key_C, Qt::ControlModifier, "c");
            QCoreApplication::sendEvent(&window, &press);
            router.acceptCurrentEvent();
        });
        QKeyEvent outer(QEvent::ShortcutOverride, Qt::Key_C, Qt::ControlModifier, "c");
        outer.setAccepted(false);
        QCoreApplication::sendEvent(&window, &outer);
        QVERIFY(!outer.isAccepted());
    }
    void releaseDuringForwardedPressWaitsForDelivery() {
        ObservedWindow window;
        window.show(); window.requestActivate();
        QVERIFY(QTest::qWaitForWindowActive(&window));
        FerricBrowserKeyRouter router;
        router.setTargetWindow(&window); router.setEnabled(true);
        QStringList received;
        window.observe = [&](QEvent *event) { received << (event->type() == QEvent::KeyPress ? "press" : "release"); };
        CallbackFilter filter;
        filter.callback = [&](QEvent *event) {
            if (event->type() != QEvent::KeyPress) return;
            QKeyEvent release(QEvent::KeyRelease, Qt::Key_P, Qt::NoModifier, "p");
            QCoreApplication::sendEvent(&window, &release);
        };
        window.installEventFilter(&filter);
        connect(&router, &FerricBrowserKeyRouter::keyPressed, this, [&] {
            QKeyEvent release(QEvent::KeyRelease, Qt::Key_P, Qt::NoModifier, "p");
            QCoreApplication::sendEvent(&window, &release);
            router.pageOwnsCurrentEvent();
        });
        QKeyEvent press(QEvent::KeyPress, Qt::Key_P, Qt::NoModifier, "p");
        QCoreApplication::sendEvent(&window, &press);
        QCOMPARE(received, QStringList({"press", "release"}));
    }
    void deferredReleaseCannotReachAnotherWindow_data() {
        QTest::addColumn<bool>("retargetOnly");
        QTest::newRow("focus-moved") << false;
        QTest::newRow("retargeted") << true;
    }
    void deferredReleaseCannotReachAnotherWindow() {
        QFETCH(bool, retargetOnly);
        ObservedWindow first, second;
        first.show(); second.show(); first.requestActivate();
        QVERIFY(QTest::qWaitForWindowActive(&first));
        FerricBrowserKeyRouter router;
        router.setTargetWindow(&first); router.setEnabled(true);
        int foreignReleases = 0, inactivePresses = 0;
        first.observe = [&](QEvent *event) { if (event->type() == QEvent::KeyPress) ++inactivePresses; };
        second.observe = [&](QEvent *event) { if (event->type() == QEvent::KeyRelease) ++foreignReleases; };
        connect(&router, &FerricBrowserKeyRouter::keyPressed, this, [&] {
            if (retargetOnly) router.setTargetWindow(&second);
            else {
                second.requestActivate();
                QVERIFY(QTest::qWaitForWindowActive(&second));
            }
            QKeyEvent release(QEvent::KeyRelease, Qt::Key_P, Qt::NoModifier, "p");
            QCoreApplication::sendEvent(&second, &release);
            router.pageOwnsCurrentEvent();
        });
        QKeyEvent press(QEvent::KeyPress, Qt::Key_P, Qt::NoModifier, "p");
        QCoreApplication::sendEvent(&first, &press);
        QCOMPARE(foreignReleases, 0);
        QCOMPARE(inactivePresses, 0);
    }
    void nativeSearchRechecksFocusAuthorization() {
        ObservedWindow window;
        QQuickItem view(window.contentItem());
        view.setSize(QSizeF(100, 100));
        window.show(); window.requestActivate();
        QVERIFY(QTest::qWaitForWindowActive(&window));
        FerricBrowserKeyRouter router;
        router.setTargetWindow(&window); router.setEnabled(true);
        int presses = 0;
        window.observe = [&](QEvent *) { ++presses; };
        connect(&view, &QQuickItem::activeFocusChanged, this, [&] {
            if (view.hasActiveFocus()) router.setEnabled(false);
        });
        QVERIFY(!router.activateSearchResult(&view, false));
        QCOMPARE(presses, 0);
    }
    void nestedAdmissionKeepsTheOuterPageDecision() {
        ObservedWindow window;
        window.show(); window.requestActivate();
        QVERIFY(QTest::qWaitForWindowActive(&window));
        FerricBrowserKeyRouter router;
        router.setTargetWindow(&window); router.setEnabled(true);
        int presses = 0, releases = 0;
        window.observe = [&](QEvent *event) {
            if (static_cast<QKeyEvent *>(event)->key() != Qt::Key_P) return;
            if (event->type() == QEvent::KeyPress) ++presses;
            else ++releases;
        };
        connect(&router, &FerricBrowserKeyRouter::keyPressed, this, [&](const QString &, int key) {
            if (key == Qt::Key_P) {
                router.pageOwnsCurrentEvent();
                QKeyEvent nested(QEvent::KeyPress, Qt::Key_K, Qt::NoModifier, "k");
                QCoreApplication::sendEvent(&window, &nested);
            } else if (key == Qt::Key_K) router.acceptCurrentEvent();
        });
        QKeyEvent press(QEvent::KeyPress, Qt::Key_P, Qt::NoModifier, "p");
        QKeyEvent release(QEvent::KeyRelease, Qt::Key_P, Qt::NoModifier, "p");
        QCoreApplication::sendEvent(&window, &press);
        QCoreApplication::sendEvent(&window, &release);
        QCOMPARE(presses, 1);
        QCOMPARE(releases, 1);
    }
};
QTEST_MAIN(NativeInputLifetime)
#include "tst_native_input_lifetime.moc"
