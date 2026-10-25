#include "../../src/profile_lifetime.h"
#include <QDir>
#include <QFile>
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQmlComponent>
#include <QQmlContext>
#include <QJSValue>
#include <QQuickWindow>
#include <QTemporaryDir>
#include <QTcpServer>
#include <QTcpSocket>
#include <QThread>
#include <QtTest>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include <algorithm>
#include <atomic>
#include <thread>
#include <QUrl>

static bool retain(FerricProfileLifetime &lifetime, QObject *host,
                   QObject *ui) {
    return lifetime.retainWindow(host, ui) && lifetime.activateWindow(host);
}

class LifetimeTests : public QObject {
    Q_OBJECT
  public:
    Q_INVOKABLE bool registerPopup(QObject *host, QQuickItem *view,
                                   QObject *ui) {
        return activeLifetime_ && host && view && ui &&
               activeLifetime_->retainWindow(host, ui) &&
               activeLifetime_->activateWindow(host) &&
               activeLifetime_->adoptView(view, host, ui);
    }
    Q_INVOKABLE void recordOpenInAbort(QQuickItem *view,
                                       const QString &leaseId,
                                       int delegatesAfterOpenIn) {
        abortedPopupView_ = view;
        abortedPopupLeaseId_ = leaseId;
        abortedDelegateCountAfterOpenIn_ = delegatesAfterOpenIn;
    }
    bool eventFilter(QObject *watched, QEvent *event) override {
        if (event && event->type() == QEvent::HoverMove) {
            for (const QPointer<QObject> &delegate : watchedDelegates_)
                if (delegate.data() == watched) {
                    ++hoverMoves_;
                    break;
                }
        }
        return false;
    }

  private slots:
    void initTestCase() {
        QVERIFY(fixtureServer_.listen(QHostAddress::LocalHost, 0));
        serveFixtureRequests();
    }
    void pendingWindowCannotSupplyPolicyOrOwnAView() {
        FerricProfileLifetime lifetime;
        QObject host, ui;
        QQuickItem view;
        QVERIFY(lifetime.retainWindow(&host, &ui));
        QCOMPARE(lifetime.policyUi(), nullptr);
        QVERIFY(!lifetime.adoptView(&view, &host, &ui));
        QVERIFY(lifetime.activateWindow(&host));
        QCOMPARE(lifetime.policyUi(), &ui);
    }
    void disposalDestroysPagesBeforeTheirProfile() {
        auto *lifetime = new FerricProfileLifetime;
        auto *profile = new QObject;
        QPointer<QObject> profileGuard = profile;
        QVERIFY(lifetime->ownResource(profile));
        QObject host, ui;
        QVERIFY(retain(*lifetime, &host, &ui));
        auto *view = new QQuickItem;
        bool profileAliveWhenPageDied = false;
        connect(view, &QObject::destroyed, &host,
                [&] { profileAliveWhenPageDied = bool(profileGuard); });
        QVERIFY(lifetime->adoptView(view, &host, &ui));
        delete lifetime;
        QVERIFY(profileAliveWhenPageDied);
        QVERIFY(profileGuard.isNull());
    }
    void profileFactoryRetainsOwnershipUntilPagesAreDestroyed() {
        bool profileAliveAtFactoryDestruction = false;
        struct Factory : QObject {
            QPointer<QObject> instance;
            bool &observed;
            Factory(QObject *profile, bool &alive)
                : instance(profile), observed(alive) {}
            ~Factory() override {
                observed = bool(instance);
                if (instance)
                    delete instance.data();
            }
        };
        auto *lifetime = new FerricProfileLifetime;
        auto *profile = new QObject;
        auto *factory = new Factory(profile, profileAliveAtFactoryDestruction);
        QPointer<QObject> profileGuard = profile;
        QVERIFY(lifetime->ownProfile(factory, profile));
        QCOMPARE(profile->parent(), factory);
        QCOMPARE(factory->parent(), lifetime);
        QObject host, ui;
        QVERIFY(retain(*lifetime, &host, &ui));
        auto *view = new QQuickItem;
        bool pageSawItsProfile = false;
        connect(view, &QObject::destroyed, &host,
                [&] { pageSawItsProfile = bool(profileGuard); });
        QVERIFY(lifetime->adoptView(view, &host, &ui));
        delete lifetime;
        QVERIFY(pageSawItsProfile);
        QVERIFY(profileAliveAtFactoryDestruction);
        QVERIFY(profileGuard.isNull());
    }
    void transferredViewSurvivesOriginalQObjectDestruction() {
        FerricProfileLifetime lifetime;
        auto *source = new QObject;
        auto *sourceUi = new QObject(source);
        QObject destination;
        QObject destinationUi(&destination);
        QQuickItem sourceVisual, destinationVisual;
        auto *view = new QQuickItem(&sourceVisual);
        QPointer<QQuickItem> guard = view;
        QVERIFY(retain(lifetime, source, sourceUi));
        QVERIFY(retain(lifetime, &destination, &destinationUi));
        QVERIFY(lifetime.adoptView(view, source, sourceUi));
        QCOMPARE(view->parent(), &lifetime);
        QCOMPARE(view->parentItem(), &sourceVisual);
        view->setParentItem(&destinationVisual);
        QVERIFY(lifetime.adoptView(view, &destination, &destinationUi));
        QCOMPARE(lifetime.policyUi(), sourceUi);
        QCOMPARE(lifetime.ownerUi(view), &destinationUi);
        delete source;
        QVERIFY(guard);
        QCOMPARE(lifetime.policyUi(), &destinationUi);
        QCOMPARE(view->parentItem(), &destinationVisual);
        lifetime.releaseWindow(&destination);
        QCOMPARE(lifetime.ownerUi(view), nullptr);
        QSignalSpy drained(&lifetime, &FerricProfileLifetime::drained);
        QCOMPARE(lifetime.viewCount(), 1);
        QCOMPARE(drained.count(), 0);
        QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
        QVERIFY(guard.isNull());
        QTRY_COMPARE(drained.count(), 1);
        QCOMPARE(lifetime.viewCount(), 0);
        QVERIFY(!retain(lifetime, &destination, &destinationUi));
    }
    void finalHostDestructionRetiresItsOwnedView() {
        FerricProfileLifetime lifetime;
        auto *host = new QObject;
        auto *ui = new QObject(host);
        auto *view = new QQuickItem;
        QPointer<QQuickItem> guard = view;
        QVERIFY(retain(lifetime, host, ui));
        QVERIFY(lifetime.adoptView(view, host, ui));
        delete host;
        QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
        QVERIFY(guard.isNull());
    }
    void pendingFinalReleaseCanAcceptANewConsumer() {
        FerricProfileLifetime lifetime;
        QObject first, firstUi, second, secondUi;
        QVERIFY(retain(lifetime, &first, &firstUi));
        QSignalSpy drained(&lifetime, &FerricProfileLifetime::drained);
        lifetime.releaseWindow(&first);
        QVERIFY(retain(lifetime, &second, &secondUi));
        QTest::qWait(20);
        QCOMPARE(drained.count(), 0);
        QCOMPARE(lifetime.policyUi(), &secondUi);
        lifetime.releaseWindow(&second);
        QTRY_COMPARE(drained.count(), 1);
    }
    void retiredViewCannotBeResurrectedBeforeDeferredDeletion() {
        FerricProfileLifetime lifetime;
        QObject first, firstUi, second, secondUi;
        QVERIFY(retain(lifetime, &first, &firstUi));
        QVERIFY(retain(lifetime, &second, &secondUi));
        auto *view = new QQuickItem;
        QVERIFY(lifetime.adoptView(view, &first, &firstUi));
        lifetime.releaseWindow(&first);
        QVERIFY(!lifetime.adoptView(view, &second, &secondUi));
        QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
        QCOMPARE(lifetime.viewCount(), 0);
    }
    void destroyingTrackerCancelsConsumerCallbacks() {
        QObject host, ui;
        auto *lifetime = new FerricProfileLifetime;
        QVERIFY(retain(*lifetime, &host, &ui));
        auto *view = new QQuickItem;
        QPointer<QQuickItem> guard = view;
        QVERIFY(lifetime->adoptView(view, &host, &ui));
        delete lifetime;
        QVERIFY(guard.isNull());
        QTest::qWait(20);
    }
    void destroyedUiPublishesTheClearedPolicyOwner() {
        FerricProfileLifetime lifetime;
        QObject host;
        auto *ui = new QObject;
        QVERIFY(retain(lifetime, &host, ui));
        QSignalSpy changed(&lifetime, &FerricProfileLifetime::policyUiChanged);
        delete ui;
        QCOMPARE(lifetime.policyUi(), nullptr);
        QCOMPARE(changed.count(), 1);
        QCOMPARE(lifetime.windowCount(), 0);
    }
    void popupDelegateLifetime();

  private:
    void serveFixtureRequests();
    int watchNativeDelegates(QQuickItem *view);
    QQuickItem *popupView(QObject *root, int serial) const;
    QQuickWindow *popupWindow(QObject *root, int serial) const;
    void clickPopupLink(QQuickWindow *window, int y = 35);
    void closePopup(FerricProfileLifetime &lifetime, QObject *root,
                    int serial);
    void verifyClosedPopup(FerricProfileLifetime &lifetime,
                           QQuickItem *view) const;

    FerricProfileLifetime *activeLifetime_ = nullptr;
    QTcpServer fixtureServer_;
    QString fixtureUrl_;
    QList<QPointer<QObject>> watchedDelegates_;
    int hoverMoves_ = 0;
    int delegateDestructions_ = 0;
    QPointer<QQuickItem> abortedPopupView_;
    QString abortedPopupLeaseId_;
    int abortedDelegateCountAfterOpenIn_ = -1;
};

void LifetimeTests::serveFixtureRequests() {
    fixtureUrl_ = QStringLiteral("http://127.0.0.1:%1/page0.html")
                      .arg(fixtureServer_.serverPort());
    connect(&fixtureServer_, &QTcpServer::newConnection, this, [this] {
        while (QTcpSocket *socket = fixtureServer_.nextPendingConnection()) {
            connect(socket, &QTcpSocket::readyRead, socket, [socket] {
                QByteArray request = socket->property("requestBuffer").toByteArray();
                request += socket->readAll();
                socket->setProperty("requestBuffer", request);
                if (!request.contains("\r\n\r\n") ||
                    socket->property("served").toBool())
                    return;
                socket->setProperty("served", true);
                const QByteArray path = request.split(' ').value(1);
                bool ok = false;
                const int page = path.mid(5).split('.').value(0).toInt(&ok);
                const int next = ok ? page + 1 : 1;
                const QByteArray link = QByteArray("/page") +
                                        QByteArray::number(next) + ".html";
                QByteArray body = R"HTML(<!doctype html><meta charset="utf-8">
<style>body{margin:0}a{display:block;width:300px;height:70px;padding:12px;box-sizing:border-box}</style>
<a id="next" href=")HTML" + link +
                                  R"HTML(" target="_blank" rel="opener">Open child</a>
<a id="second" href=")HTML" + link +
                                  R"HTML(" target="_blank" rel="opener" style="margin-top:18px">Open another child</a>
<script>window.documentNonce=Date.now()+Math.random();window.persistentState=73;window.d30MessageRoundTrip=false;window.addEventListener('message',e=>{if(e.data==='child-ready')e.source.postMessage('child-reply','*');if(e.data==='child-reply')window.d30MessageRoundTrip=true});window.addEventListener('load',()=>{if(window.opener)window.opener.postMessage('child-ready','*')})</script>)HTML";
                socket->write(
                    QByteArray("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\nContent-Length: ") +
                    QByteArray::number(body.size()) + "\r\n\r\n" + body);
                socket->disconnectFromHost();
            });
            connect(socket, &QTcpSocket::disconnected, socket,
                    &QObject::deleteLater);
        }
    });
}

int LifetimeTests::watchNativeDelegates(QQuickItem *view) {
    if (!view)
        return 0;
    const QByteArray expected =
        QByteArrayLiteral("QtWebEngineCore::RenderWidgetHostViewQtDelegateItem");
    QList<QQuickItem *> pending{view};
    int added = 0;
    while (!pending.isEmpty()) {
        QQuickItem *item = pending.takeLast();
        pending.append(item->childItems());
        if (QByteArray(item->metaObject()->className()) != expected ||
            !item->inherits(expected.constData()))
            continue;
        const bool alreadyWatched = std::any_of(
            watchedDelegates_.cbegin(), watchedDelegates_.cend(),
            [item](const QPointer<QObject> &candidate) {
                return candidate.data() == item;
            });
        if (alreadyWatched)
            continue;
        watchedDelegates_.append(item);
        item->installEventFilter(this);
        connect(item, &QObject::destroyed, this, [this] {
            ++delegateDestructions_;
        });
        ++added;
    }
    return added;
}

QQuickItem *LifetimeTests::popupView(QObject *root, int serial) const {
    if (!root || serial < 1)
        return nullptr;
    const QJSValue views = root->property("popupViews").value<QJSValue>();
    return qobject_cast<QQuickItem *>(views.property(serial).toQObject());
}

QQuickWindow *LifetimeTests::popupWindow(QObject *root, int serial) const {
    if (!root || serial < 1)
        return nullptr;
    const QJSValue windows = root->property("popupWindows").value<QJSValue>();
    return qobject_cast<QQuickWindow *>(windows.property(serial - 1).toQObject());
}

void LifetimeTests::clickPopupLink(QQuickWindow *window, int y) {
    QVERIFY(window);
    QTest::mouseMove(window, QPoint(330, 240));
    QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier,
                      QPoint(60, y));
}

void LifetimeTests::closePopup(FerricProfileLifetime &lifetime,
                               QObject *root, int serial) {
    auto *window = popupWindow(root, serial);
    QVERIFY(window);
    QVERIFY(QMetaObject::invokeMethod(
        root, "removePopupForTest", Q_ARG(QVariant, QVariant(serial))));
    QPointer<QQuickWindow> guard = window;
    lifetime.releaseWindow(window);
    window->close();
    window->deleteLater();
    QTRY_VERIFY_WITH_TIMEOUT(guard.isNull(), 5000);
}

void LifetimeTests::verifyClosedPopup(FerricProfileLifetime &lifetime,
                                     QQuickItem *view) const {
    QVERIFY(view);
    QTRY_VERIFY_WITH_TIMEOUT(view->property("lifecycleState").toInt() == 2,
                             10000);
    QTRY_VERIFY_WITH_TIMEOUT(
        view->property("renderProcessPid").toLongLong() == 0, 10000);
    QVERIFY(!view->isVisible());
    QVERIFY(!view->isEnabled());
    QCOMPARE(lifetime.ownerUi(view), nullptr);
}

void LifetimeTests::popupDelegateLifetime() {
    auto lifetime = std::make_unique<FerricProfileLifetime>();
    activeLifetime_ = lifetime.get();
    watchedDelegates_.clear();
    hoverMoves_ = 0;
    delegateDestructions_ = 0;
    QSignalSpy drained(lifetime.get(), &FerricProfileLifetime::drained);
    QSignalSpy leaseReleased(lifetime.get(),
                             &FerricProfileLifetime::popupOpenerLeaseReleased);

    QQmlApplicationEngine engine;
    engine.rootContext()->setContextProperty("fixtureUrlForTest", fixtureUrl_);
    engine.rootContext()->setContextProperty("lifetime", lifetime.get());
    engine.rootContext()->setContextProperty("testHarness", this);
    static const char qml[] = R"QML(
import QtQuick
import QtQuick.Controls
import QtWebEngine
ApplicationWindow {
    id: root
    objectName: "lifetimeRootWindow"
    width: 800
    height: 600
    visible: true
    property url fixtureUrl: fixtureUrlForTest
    property int popupRequestCount: 0
    property int popupAdoptionCount: 0
    property int popupAdmissionFailures: 0
    property int popupCreationCount: 0
    property bool inspectParentClosed: false
    property bool abortAfterOpenInForTest: false
    property var popupViews: [openerView]
    property var popupWindows: []
    property var popupLeaseIds: []
    property var leaseNativeCountsAtAcquire: []
    property var leaseNativeCountsAfterOpenIn: []
    property var readyPages: [false]
    property var noncesAtLoad: [""]
    property var noncesAfterParentClose: [""]
    property var openerConnected: [false]
    property var openerClosedAfterParentClose: [false]
    property var persistentStateAfterParentClose: [false]
    property var messageRoundTrips: [false]
    property alias openerViewObject: openerView
    property alias sharedProfile: diagnosticProfile
    WebEngineProfile { id: diagnosticProfile }
    WebEngineProfile { id: alternateProfile }
    WebEngineView {
        id: openerView
        objectName: "openerView"
        anchors.fill: parent
        profile: diagnosticProfile
        url: root.fixtureUrl
    }
    WebEngineView {
        id: wrongProfileView
        objectName: "wrongProfileView"
        visible: false
        width: 1
        height: 1
        profile: alternateProfile
        url: "about:blank"
    }
    WebEngineView {
        id: unreadyView
        objectName: "unreadyView"
        visible: false
        width: 1
        height: 1
        profile: diagnosticProfile
        url: "about:blank"
    }
    Component {
        id: popupWindowComponent
        ApplicationWindow {
            id: popupWindow
            objectName: "popupWindow-" + popupSerial
            width: 800
            height: 600
            visible: false
            required property var fixtureRoot
            required property int popupDepth
            required property int popupSerial
            property alias webView: popupView
            WebEngineView {
                id: popupView
                objectName: "popupView-" + popupWindow.popupSerial
                anchors.fill: parent
                profile: popupWindow.fixtureRoot.sharedProfile
                Component.onCompleted: {
                    if (!testHarness.registerPopup(popupWindow, popupView,
                                                   popupWindow.fixtureRoot))
                        popupWindow.fixtureRoot.popupAdmissionFailures += 1
                }
            }
            Connections {
                target: popupView
                function onNewWindowRequested(request) {
                    popupWindow.fixtureRoot.adoptPopup(
                        request, popupView, popupWindow.popupDepth + 1)
                }
            }
        }
    }
    Connections {
        target: openerView
        function onNewWindowRequested(request) {
            root.adoptPopup(request, openerView, 1)
        }
    }
    function setAt(name, index, value) {
        var copy = root[name].slice(0)
        copy[index] = value
        root[name] = copy
    }
    function removePopupViewForTest(index) {
        var copy = root.popupViews.slice(0)
        copy[index] = null
        root.popupViews = copy
    }
    function removePopupForTest(serial) {
        removePopupViewForTest(serial)
        var copy = root.popupWindows.slice(0)
        copy[serial - 1] = null
        root.popupWindows = copy
    }
    function adoptPopup(request, sourceView, depth) {
        const serial = ++popupCreationCount
        const popup = popupWindowComponent.createObject(root, {
            fixtureRoot: root,
            popupDepth: depth,
            popupSerial: serial
        })
        if (!popup) { popupAdmissionFailures += 1; return }
        const target = popup.webView
        const leaseId = lifetime.acquirePopupOpenerLease(sourceView, target)
        if (!leaseId) {
            popupAdmissionFailures += 1
            popup.destroy()
            return
        }
        setAt("popupViews", popupViews.length, target)
        setAt("popupWindows", popupWindows.length, popup)
        setAt("popupLeaseIds", popupLeaseIds.length, leaseId)
        setAt("leaseNativeCountsAtAcquire", leaseNativeCountsAtAcquire.length,
              lifetime.popupOpenerLeaseDelegateCount(leaseId))
        if (!lifetime.beginPopupOpenerLease(leaseId)) {
            popupAdmissionFailures += 1
            lifetime.cancelPopupOpenerLease(leaseId)
            popup.destroy()
            return
        }
        request.openIn(target)
        const delegatesAfterOpenIn =
            lifetime.popupOpenerLeaseDelegateCount(leaseId)
        setAt("leaseNativeCountsAfterOpenIn",
              leaseNativeCountsAfterOpenIn.length, delegatesAfterOpenIn)
        if (!lifetime.confirmPopupOpenerLease(leaseId))
            popupAdmissionFailures += 1
        popupAdoptionCount += 1
        popupRequestCount += 1
        if (root.abortAfterOpenInForTest) {
            testHarness.recordOpenInAbort(target, leaseId,
                                          delegatesAfterOpenIn)
            root.removePopupForTest(serial)
            lifetime.releaseWindow(popup)
            popup.close()
            popup.destroy()
            return
        }
        popup.visible = true
    }
    Timer {
        interval: 50
        repeat: true
        running: true
        onTriggered: {
            for (var i = 0; i < root.popupViews.length; ++i) {
                const index = i
                const view = root.popupViews[index]
                if (!view || typeof view.runJavaScript !== "function") continue
                view.runJavaScript(
                    "document.readyState === 'complete' && location.pathname.indexOf('/page') === 0",
                    function(ready) {
                        if (ready === true && !root.readyPages[index])
                            root.setAt("readyPages", index, true)
                    })
                view.runJavaScript(
                    "window.documentNonce",
                    function(value) {
                        if (typeof value === "number" && Number.isFinite(value)
                                && !root.noncesAtLoad[index])
                            root.setAt("noncesAtLoad", index, String(value))
                    })
                view.runJavaScript(
                    "window.opener !== null",
                    function(connected) {
                        if (connected === true)
                            root.setAt("openerConnected", index, true)
                    })
                view.runJavaScript(
                    "window.d30MessageRoundTrip === true",
                    function(ready) {
                        if (ready === true)
                            root.setAt("messageRoundTrips", index, true)
                    })
                const issuedAfterParentClose = root.inspectParentClosed
                if (issuedAfterParentClose) {
                    view.runJavaScript(
                        "document.readyState === 'complete' && location.pathname.indexOf('/page') === 0 && typeof window.documentNonce === 'number' && Number.isFinite(window.documentNonce) ? window.documentNonce : null",
                        function(value) {
                            if (typeof value !== "number" || !Number.isFinite(value))
                                return
                            if (root.inspectParentClosed)
                                root.setAt("noncesAfterParentClose", index,
                                           String(value))
                        })
                    view.runJavaScript(
                        "window.opener === null || window.opener.closed === true",
                        function(closed) {
                            if (closed === true)
                                root.setAt("openerClosedAfterParentClose",
                                           index, true)
                        })
                    view.runJavaScript(
                        "window.persistentState === 73",
                        function(preserved) {
                            if (preserved === true)
                                root.setAt("persistentStateAfterParentClose",
                                           index, true)
                        })
                }
            }
        }
    }
}
)QML";
    QQmlComponent component(&engine);
    component.setData(QByteArray(qml), QUrl(QStringLiteral("qrc:/lifetime.qml")));
    QObject *rootObject = component.create();
    QVERIFY2(rootObject, qPrintable(component.errorString()));
    auto *rootWindow = qobject_cast<QQuickWindow *>(rootObject);
    QVERIFY(rootWindow);
    QPointer<QQuickWindow> rootWindowGuard = rootWindow;
    QQuickItem *opener = rootObject->property("openerViewObject")
                             .value<QObject *>()
                             ? qobject_cast<QQuickItem *>(rootObject->property(
                                   "openerViewObject").value<QObject *>())
                             : nullptr;
    QVERIFY(opener);
    QObject *profile = rootObject->property("sharedProfile").value<QObject *>();
    QVERIFY(profile);
    QVERIFY(lifetime->ownResource(profile));
    QPointer<QObject> profileGuard = profile;
    bool profileAliveForEveryPage = true;
    connect(opener, &QObject::destroyed, this, [&] {
        profileAliveForEveryPage = profileAliveForEveryPage && bool(profileGuard);
    });
    QVERIFY(retain(*lifetime, rootWindow, rootWindow));
    QVERIFY(lifetime->adoptView(opener, rootWindow, rootWindow));
    rootWindow->show();

    auto *wrongProfile = rootObject->findChild<QQuickItem *>(
        QStringLiteral("wrongProfileView"));
    QVERIFY(wrongProfile);
    QVERIFY(lifetime->adoptView(wrongProfile, rootWindow, rootWindow));
    QVERIFY(lifetime->acquirePopupOpenerLease(opener, wrongProfile).isEmpty());
    QObject unregisteredHost, unregisteredUi;
    QVERIFY(!lifetime->adoptView(wrongProfile, &unregisteredHost,
                                 &unregisteredUi));
    QCOMPARE(lifetime->ownerUi(wrongProfile), rootWindow);
    QQuickItem untracked;
    QVERIFY(lifetime->acquirePopupOpenerLease(opener, &untracked).isEmpty());
    QObject pendingHost, pendingUi;
    QVERIFY(lifetime->retainWindow(&pendingHost, &pendingUi));
    auto *unready = rootObject->findChild<QQuickItem *>(
        QStringLiteral("unreadyView"));
    QVERIFY(unready);
    QVERIFY(!lifetime->adoptView(unready, &pendingHost, &pendingUi));
    QVERIFY(lifetime->acquirePopupOpenerLease(opener, unready).isEmpty());
    lifetime->releaseWindow(&pendingHost);
    QVERIFY(lifetime->adoptView(unready, rootWindow, rootWindow));
    QPointer<QQuickItem> abortedPopupTarget = unready;
    QCOMPARE(watchNativeDelegates(unready), 0);
    const QString abortedLease =
        lifetime->acquirePopupOpenerLease(opener, unready);
    QVERIFY(!abortedLease.isEmpty());
    QCOMPARE(lifetime->popupOpenerLeaseDelegateCount(abortedLease), 0);
    QCOMPARE(lifetime->popupOpenerLeaseCount(), 1);
    QVERIFY(lifetime->cancelPopupOpenerLease(abortedLease));
    QCOMPARE(lifetime->popupOpenerLeaseCount(), 0);
    QVERIFY(!lifetime->cancelPopupOpenerLease(abortedLease));
    const QString currentLease =
        lifetime->acquirePopupOpenerLease(opener, unready);
    QVERIFY(!currentLease.isEmpty());
    QVERIFY(currentLease != abortedLease);
    QVERIFY(!lifetime->cancelPopupOpenerLease(abortedLease));
    QCOMPARE(lifetime->popupOpenerLeaseCount(), 1);
    QVERIFY(lifetime->cancelPopupOpenerLease(currentLease));
    const QString earlyCloseLease =
        lifetime->acquirePopupOpenerLease(opener, unready);
    QVERIFY(!earlyCloseLease.isEmpty());
    QCOMPARE(lifetime->popupOpenerLeaseDelegateCount(earlyCloseLease), 0);
    QCOMPARE(lifetime->popupOpenerLeaseCount(), 1);
    lifetime->releaseView(unready);
    QTRY_VERIFY_WITH_TIMEOUT(abortedPopupTarget.isNull(), 5000);
    QTRY_COMPARE_WITH_TIMEOUT(lifetime->popupOpenerLeaseCount(), 0, 5000);

    QQmlApplicationEngine alternateEngine;
    alternateEngine.rootContext()->setContextProperty("sharedProfileForTest",
                                                       profile);
    QQmlComponent alternateComponent(&alternateEngine);
    alternateComponent.setData(
        QByteArrayLiteral("import QtQuick\nimport QtWebEngine\n"
                          "Item { WebEngineView { objectName: \"crossEngineView\"; "
                          "profile: sharedProfileForTest } }"),
        QUrl(QStringLiteral("qrc:/cross-engine.qml")));
    QObject *alternateRoot = alternateComponent.create();
    QVERIFY2(alternateRoot, qPrintable(alternateComponent.errorString()));
    auto *crossEngineView = alternateRoot->findChild<QQuickItem *>(
        QStringLiteral("crossEngineView"));
    QVERIFY(crossEngineView);
    QQuickWindow alternateHost;
    QObject alternateUi;
    crossEngineView->setParentItem(alternateHost.contentItem());
    QVERIFY(lifetime->retainWindow(&alternateHost, &alternateUi));
    QVERIFY(lifetime->activateWindow(&alternateHost));
    QVERIFY(lifetime->adoptView(crossEngineView, &alternateHost,
                                &alternateUi));
    QVERIFY(lifetime->acquirePopupOpenerLease(opener, crossEngineView).isEmpty());
    lifetime->releaseWindow(&alternateHost);
    QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
    QCoreApplication::processEvents();

    QSignalSpy openerRetired(lifetime.get(), &FerricProfileLifetime::viewRetiring);
    QTRY_VERIFY_WITH_TIMEOUT(rootObject->property("readyPages").toList().value(0).toBool(),
                             15000);
    clickPopupLink(rootWindow, 35);
    QTRY_COMPARE_WITH_TIMEOUT(rootObject->property("popupAdoptionCount").toInt(),
                              1, 15000);
    QTRY_VERIFY_WITH_TIMEOUT(rootObject->property("readyPages").toList().value(1).toBool(),
                             15000);
    auto *firstView = popupView(rootObject, 1);
    auto *firstWindow = popupWindow(rootObject, 1);
    QVERIFY(firstView && firstWindow);
    QVERIFY(watchNativeDelegates(firstView) > 0);
    const QString firstLease =
        rootObject->property("popupLeaseIds").toList().value(0).toString();
    QVERIFY(!firstLease.isEmpty());
    QTRY_VERIFY_WITH_TIMEOUT(
        lifetime->popupOpenerLeaseDelegateCount(firstLease) > 0, 10000);
    QCOMPARE(rootObject->property("leaseNativeCountsAtAcquire")
                 .value<QJSValue>().property(0).toInt(), 0);
    std::atomic_bool workerRejected{false};
    std::thread worker([&] {
        workerRejected.store(
            lifetime->acquirePopupOpenerLease(opener, firstView).isEmpty());
    });
    worker.join();
    QVERIFY(workerRejected.load());

    clickPopupLink(rootWindow, 125);
    QTRY_COMPARE_WITH_TIMEOUT(rootObject->property("popupAdoptionCount").toInt(),
                              2, 15000);
    QTRY_VERIFY_WITH_TIMEOUT(rootObject->property("readyPages").toList().value(2).toBool(),
                             15000);
    auto *secondView = popupView(rootObject, 2);
    auto *secondWindow = popupWindow(rootObject, 2);
    QVERIFY(secondView && secondWindow);
    QVERIFY(watchNativeDelegates(secondView) > 0);

    clickPopupLink(firstWindow, 35);
    QTRY_COMPARE_WITH_TIMEOUT(rootObject->property("popupAdoptionCount").toInt(),
                              3, 15000);
    QTRY_VERIFY_WITH_TIMEOUT(rootObject->property("readyPages").toList().value(3).toBool(),
                             15000);
    auto *thirdView = popupView(rootObject, 3);
    auto *thirdWindow = popupWindow(rootObject, 3);
    QVERIFY(thirdView && thirdWindow);
    QVERIFY(watchNativeDelegates(thirdView) > 0);

    QQuickItem *const thirdParentItem = thirdView->parentItem();
    const QUrl thirdUrl = thirdView->property("url").toUrl();
    QPointer<QObject> previousDelegate;
    const QByteArray expectedDelegate =
        QByteArrayLiteral("QtWebEngineCore::RenderWidgetHostViewQtDelegateItem");
    for (const auto &candidate : std::as_const(watchedDelegates_)) {
        auto *item = qobject_cast<QQuickItem *>(candidate.data());
        if (!item || QByteArray(item->metaObject()->className()) !=
                         expectedDelegate ||
            !item->inherits(expectedDelegate.constData()))
            continue;
        for (auto *parent = item; parent; parent = parent->parentItem()) {
            if (parent == thirdView) {
                previousDelegate = candidate;
                break;
            }
        }
        if (previousDelegate)
            break;
    }
    QVERIFY(previousDelegate);
    const int delegateDeathsBeforeDiscard = delegateDestructions_;
    const QString thirdLease = rootObject->property("popupLeaseIds")
                                   .value<QJSValue>().property(2).toString();
    QVERIFY(!thirdLease.isEmpty());
    QTRY_VERIFY_WITH_TIMEOUT(
        lifetime->popupOpenerLeaseDelegateCount(thirdLease) > 0, 10000);
    thirdView->setVisible(false);
    QVERIFY(thirdView->setProperty("lifecycleState", 2));
    QTRY_VERIFY_WITH_TIMEOUT(
        thirdView->property("renderProcessPid").toLongLong() == 0, 15000);
    QCOMPARE(thirdView->window(), thirdWindow);
    QCOMPARE(thirdView->parentItem(), thirdParentItem);
    QVERIFY(thirdView->setProperty("lifecycleState", 0));
    QVERIFY(QMetaObject::invokeMethod(thirdView, "reload"));
    thirdView->setVisible(true);
    QTRY_VERIFY_WITH_TIMEOUT(
        thirdView->property("renderProcessPid").toLongLong() > 0, 15000);
    QTRY_VERIFY_WITH_TIMEOUT(!thirdView->property("loading").toBool(), 15000);
    QCOMPARE(thirdView->property("url").toUrl(), thirdUrl);
    QCOMPARE(thirdView->property("profile").value<QObject *>(), profile);
    QCOMPARE(thirdView->window(), thirdWindow);
    QCOMPARE(thirdView->parentItem(), thirdParentItem);
    const auto findCurrentDelegate = [thirdView, expectedDelegate] {
        QList<QQuickItem *> pending{thirdView};
        while (!pending.isEmpty()) {
            QQuickItem *item = pending.takeLast();
            if (QByteArray(item->metaObject()->className()) ==
                    expectedDelegate &&
                item->inherits(expectedDelegate.constData()))
                return item;
            pending.append(item->childItems());
        }
        return static_cast<QQuickItem *>(nullptr);
    };
    QTRY_VERIFY_WITH_TIMEOUT(findCurrentDelegate(), 15000);
    QQuickItem *currentDelegate = findCurrentDelegate();
    QVERIFY(currentDelegate);
    watchNativeDelegates(thirdView);
    const bool alreadyWatched = std::any_of(
        watchedDelegates_.cbegin(), watchedDelegates_.cend(),
        [currentDelegate](const QPointer<QObject> &candidate) {
            return candidate.data() == currentDelegate;
        });
    if (!alreadyWatched) {
        watchedDelegates_.append(currentDelegate);
        currentDelegate->installEventFilter(this);
        connect(currentDelegate, &QObject::destroyed, this, [this] {
            ++delegateDestructions_;
        });
    }
    QPointer<QObject> replacementDelegate;
    for (const auto &candidate : std::as_const(watchedDelegates_)) {
        auto *item = qobject_cast<QQuickItem *>(candidate.data());
        if (!item || QByteArray(item->metaObject()->className()) !=
                         expectedDelegate ||
            !item->inherits(expectedDelegate.constData()))
            continue;
        for (auto *parent = item; parent; parent = parent->parentItem()) {
            if (parent == thirdView) {
                replacementDelegate = candidate;
                break;
            }
        }
        if (replacementDelegate)
            break;
    }
    QVERIFY(replacementDelegate);
    QCOMPARE(replacementDelegate.data(), currentDelegate);
    QVERIFY(replacementDelegate != previousDelegate);
    QTRY_VERIFY_WITH_TIMEOUT(previousDelegate.isNull(), 10000);
    QVERIFY(delegateDestructions_ > delegateDeathsBeforeDiscard);
    QTRY_VERIFY_WITH_TIMEOUT(
        lifetime->popupOpenerLeaseDelegateCount(thirdLease) > 0, 10000);
    hoverMoves_ = 0;
    QTest::mouseMove(thirdWindow, QPoint(300, 240));
    QTest::mouseMove(thirdWindow, QPoint(460, 390));
    QTRY_VERIFY_WITH_TIMEOUT(hoverMoves_ > 0, 5000);

    clickPopupLink(thirdWindow, 35);
    QTRY_COMPARE_WITH_TIMEOUT(rootObject->property("popupAdoptionCount").toInt(),
                              4, 15000);
    QTRY_VERIFY_WITH_TIMEOUT(rootObject->property("readyPages").toList().value(4).toBool(),
                             15000);
    auto *fourthView = popupView(rootObject, 4);
    auto *fourthWindow = popupWindow(rootObject, 4);
    QVERIFY(fourthView && fourthWindow);
    QVERIFY(watchNativeDelegates(fourthView) > 0);
    QCOMPARE(rootObject->property("popupAdmissionFailures").toInt(), 0);
    QCOMPARE(lifetime->popupOpenerLeaseCount(), 4);
    QTRY_VERIFY_WITH_TIMEOUT(rootObject->property("messageRoundTrips").toList().value(4).toBool(),
                             10000);
    QTRY_VERIFY_WITH_TIMEOUT(!rootObject->property("noncesAtLoad").toList().value(4).toString().isEmpty(),
                             10000);
    const QString fourthLease = rootObject->property("popupLeaseIds").toList().value(3).toString();
    QVERIFY(!fourthLease.isEmpty());
    QTRY_VERIFY_WITH_TIMEOUT(lifetime->popupOpenerLeaseDelegateCount(fourthLease) > 0,
                             10000);

    QPointer<QQuickItem> openerGuard = opener;
    QPointer<QQuickItem> firstGuard = firstView;
    QPointer<QQuickItem> secondGuard = secondView;
    QPointer<QQuickItem> thirdGuard = thirdView;
    QPointer<QQuickItem> fourthGuard = fourthView;
    int nativeDeathsAtThirdDestruction = -1;
    connect(thirdView, &QObject::destroyed, this, [&] {
        nativeDeathsAtThirdDestruction = delegateDestructions_;
        profileAliveForEveryPage = profileAliveForEveryPage && bool(profileGuard);
    });
    connect(firstView, &QObject::destroyed, this, [&] {
        profileAliveForEveryPage = profileAliveForEveryPage && bool(profileGuard);
    });
    connect(secondView, &QObject::destroyed, this, [&] {
        profileAliveForEveryPage = profileAliveForEveryPage && bool(profileGuard);
    });
    connect(fourthView, &QObject::destroyed, this, [&] {
        profileAliveForEveryPage = profileAliveForEveryPage && bool(profileGuard);
    });

    QVERIFY(QMetaObject::invokeMethod(
        rootObject, "removePopupViewForTest", Q_ARG(QVariant, QVariant(0))));
    lifetime->releaseView(opener);
    rootObject->setProperty("inspectParentClosed", true);
    verifyClosedPopup(*lifetime, opener);
    closePopup(*lifetime, rootObject, 1);
    verifyClosedPopup(*lifetime, firstView);
    QTRY_COMPARE_WITH_TIMEOUT(lifetime->popupOpenerLeaseCount(), 3, 10000);
    const QString retiredFirstLease = rootObject->property("popupLeaseIds")
                                          .value<QJSValue>()
                                          .property(0).toString();
    QCOMPARE(lifetime->popupOpenerLeaseDelegateCount(retiredFirstLease), 0);
    lifetime->releaseView(firstView);
    QCOMPARE(lifetime->popupOpenerLeaseCount(), 3);
    QVERIFY(lifetime->acquirePopupOpenerLease(opener, firstView).isEmpty());
    QVERIFY(openerGuard);
    closePopup(*lifetime, rootObject, 2);
    QTRY_VERIFY_WITH_TIMEOUT(openerGuard.isNull(), 10000);
    QTRY_COMPARE_WITH_TIMEOUT(lifetime->popupOpenerLeaseCount(), 2, 10000);
    QVERIFY(thirdGuard && fourthGuard);

    closePopup(*lifetime, rootObject, 3);
    verifyClosedPopup(*lifetime, thirdView);
    QTRY_VERIFY_WITH_TIMEOUT(firstGuard.isNull(), 10000);
    QTRY_COMPARE_WITH_TIMEOUT(lifetime->popupOpenerLeaseCount(), 1, 10000);
    QVERIFY(secondGuard.isNull());
    QVERIFY(thirdGuard && fourthGuard);
    QTRY_VERIFY_WITH_TIMEOUT(rootObject->property("persistentStateAfterParentClose").toList().value(4).toBool(),
                             10000);
    QTRY_VERIFY_WITH_TIMEOUT(rootObject->property("openerClosedAfterParentClose").toList().value(4).toBool(),
                             10000);
    const QString nonceBefore = rootObject->property("noncesAtLoad").toList().value(4).toString();
    QTRY_COMPARE_WITH_TIMEOUT(rootObject->property("noncesAfterParentClose").toList().value(4).toString(),
                              nonceBefore, 10000);
    QVERIFY(rootObject->property("messageRoundTrips").toList().value(4).toBool());

    hoverMoves_ = 0;
    QTest::mouseMove(fourthWindow, QPoint(330, 240));
    QTest::mouseMove(fourthWindow, QPoint(460, 390));
    QTRY_VERIFY_WITH_TIMEOUT(hoverMoves_ > 0, 5000);
    QCOMPARE(rootObject->property("popupRequestCount").toInt(), 4);
    QCOMPARE(rootObject->property("popupAdoptionCount").toInt(), 4);
    rootObject->setProperty("abortAfterOpenInForTest", true);
    clickPopupLink(fourthWindow, 35);
    QTRY_COMPARE_WITH_TIMEOUT(rootObject->property("popupRequestCount").toInt(),
                              5, 15000);
    QTRY_COMPARE_WITH_TIMEOUT(rootObject->property("popupAdoptionCount").toInt(),
                              5, 15000);
    QVERIFY(!abortedPopupLeaseId_.isEmpty());
    QCOMPARE(lifetime->popupOpenerLeaseCount(), 1);
    QVERIFY(!lifetime->cancelPopupOpenerLease(abortedPopupLeaseId_));
    QCOMPARE(rootObject->property("leaseNativeCountsAtAcquire")
                 .value<QJSValue>().property(4).toInt(), 0);
    QCOMPARE(abortedDelegateCountAfterOpenIn_, 0);
    QTRY_VERIFY_WITH_TIMEOUT(abortedPopupView_.isNull(), 10000);
    QTRY_COMPARE_WITH_TIMEOUT(lifetime->popupOpenerLeaseCount(), 1, 10000);
    QCOMPARE(rootObject->property("popupAdmissionFailures").toInt(), 0);

    closePopup(*lifetime, rootObject, 4);
    QTRY_VERIFY_WITH_TIMEOUT(thirdGuard.isNull(), 10000);
    QTRY_COMPARE_WITH_TIMEOUT(lifetime->popupOpenerLeaseCount(), 0, 10000);
    QVERIFY(fourthGuard.isNull());
    QVERIFY(nativeDeathsAtThirdDestruction > 0);

    QVERIFY(profileAliveForEveryPage);
    QVERIFY(leaseReleased.count() >= 4);
    QVERIFY(openerRetired.count() >= 4);

    QPointer<QObject> shutdownProfileGuard;
    bool shutdownProfileDestroyedAfterConsumers = false;
    {
        FerricProfileLifetime shutdownLifetime;
        activeLifetime_ = &shutdownLifetime;
        QQmlApplicationEngine shutdownEngine;
        shutdownEngine.rootContext()->setContextProperty("fixtureUrlForTest",
                                                         fixtureUrl_);
        shutdownEngine.rootContext()->setContextProperty("lifetime",
                                                         &shutdownLifetime);
        shutdownEngine.rootContext()->setContextProperty("testHarness", this);
        QQmlComponent shutdownComponent(&shutdownEngine);
        shutdownComponent.setData(QByteArray(qml),
                                  QUrl(QStringLiteral("qrc:/shutdown.qml")));
        QObject *shutdownRoot = shutdownComponent.create();
        QVERIFY2(shutdownRoot, qPrintable(shutdownComponent.errorString()));
        auto *shutdownWindow = qobject_cast<QQuickWindow *>(shutdownRoot);
        QVERIFY(shutdownWindow);
        auto *shutdownOpener = qobject_cast<QQuickItem *>(
            shutdownRoot->property("openerViewObject").value<QObject *>());
        QVERIFY(shutdownOpener);
        QObject *shutdownProfile = shutdownRoot->property("sharedProfile")
                                       .value<QObject *>();
        QVERIFY(shutdownProfile);
        QVERIFY(shutdownLifetime.ownResource(shutdownProfile));
        shutdownProfileGuard = shutdownProfile;
        bool shutdownProfileAliveForEveryPage = true;
        QVERIFY(retain(shutdownLifetime, shutdownWindow, shutdownWindow));
        QVERIFY(shutdownLifetime.adoptView(shutdownOpener, shutdownWindow,
                                           shutdownWindow));
        shutdownWindow->show();
        QTRY_VERIFY_WITH_TIMEOUT(
            shutdownRoot->property("readyPages").toList().value(0).toBool(),
            15000);

        clickPopupLink(shutdownWindow, 35);
        QTRY_COMPARE_WITH_TIMEOUT(
            shutdownRoot->property("popupAdoptionCount").toInt(), 1, 15000);
        auto *shutdownChild = popupView(shutdownRoot, 1);
        auto *shutdownChildWindow = popupWindow(shutdownRoot, 1);
        QVERIFY(shutdownChild && shutdownChildWindow);
        QTRY_VERIFY_WITH_TIMEOUT(
            shutdownRoot->property("readyPages").toList().value(1).toBool(),
            15000);
        clickPopupLink(shutdownChildWindow, 35);
        QTRY_COMPARE_WITH_TIMEOUT(
            shutdownRoot->property("popupAdoptionCount").toInt(), 2, 15000);
        auto *shutdownGrandchild = popupView(shutdownRoot, 2);
        QVERIFY(shutdownGrandchild);
        QTRY_VERIFY_WITH_TIMEOUT(
            shutdownRoot->property("readyPages").toList().value(2).toBool(),
            15000);
        clickPopupLink(shutdownWindow, 125);
        QTRY_COMPARE_WITH_TIMEOUT(
            shutdownRoot->property("popupAdoptionCount").toInt(), 3, 15000);
        auto *shutdownSibling = popupView(shutdownRoot, 3);
        QVERIFY(shutdownSibling);
        QTRY_VERIFY_WITH_TIMEOUT(
            shutdownRoot->property("readyPages").toList().value(3).toBool(),
            15000);

        const QList<QQuickItem *> shutdownViews{
            shutdownOpener, shutdownChild, shutdownGrandchild, shutdownSibling};
        QList<QPointer<QQuickItem>> shutdownViewGuards;
        for (QQuickItem *view : shutdownViews) {
            QVERIFY(watchNativeDelegates(view) > 0);
            shutdownViewGuards.append(view);
            connect(view, &QObject::destroyed, this, [&] {
                shutdownProfileAliveForEveryPage =
                    shutdownProfileAliveForEveryPage &&
                    bool(shutdownProfileGuard);
            });
        }
        QList<QPointer<QObject>> shutdownNativeGuards;
        for (const auto &candidate : std::as_const(watchedDelegates_)) {
            QObject *delegate = candidate.data();
            auto *item = qobject_cast<QQuickItem *>(delegate);
            if (!item)
                continue;
            for (QQuickItem *view : shutdownViews) {
                bool ownedByView = false;
                for (QQuickItem *parent = item; parent;
                     parent = parent->parentItem()) {
                    if (parent == view) {
                        ownedByView = true;
                        break;
                    }
                }
                if (ownedByView) {
                    shutdownNativeGuards.append(candidate);
                    break;
                }
            }
        }
        connect(shutdownProfile, &QObject::destroyed, this, [&] {
            shutdownProfileDestroyedAfterConsumers =
                std::all_of(shutdownViewGuards.cbegin(),
                            shutdownViewGuards.cend(),
                            [](const QPointer<QQuickItem> &view) {
                                return view.isNull();
                            }) &&
                std::all_of(shutdownNativeGuards.cbegin(),
                            shutdownNativeGuards.cend(),
                            [](const QPointer<QObject> &delegate) {
                                return delegate.isNull();
                            });
        });
        const QByteArray expectedShutdownDelegate = QByteArrayLiteral(
            "QtWebEngineCore::RenderWidgetHostViewQtDelegateItem");
        QHash<QString, QPointer<QObject>> childDelegateForLease;
        QHash<QString, QPointer<QQuickItem>> childViewForLease;
        QHash<QString, QPointer<QQuickItem>> sourceForLease;
        const auto delegateInView = [expectedShutdownDelegate](QQuickItem *view)
            -> QObject * {
            QList<QQuickItem *> pending{view};
            while (!pending.isEmpty()) {
                QQuickItem *item = pending.takeLast();
                if (QByteArray(item->metaObject()->className()) ==
                        expectedShutdownDelegate &&
                    item->inherits(expectedShutdownDelegate.constData()))
                    return item;
                pending.append(item->childItems());
            }
            return nullptr;
        };
        for (int i = 0; i < 3; ++i) {
            const QString leaseId = shutdownRoot->property("popupLeaseIds")
                                        .value<QJSValue>().property(i).toString();
            QVERIFY(!leaseId.isEmpty());
            const int sourceIndex = i == 2 ? 0 : i;
            const int childIndex = i + 1;
            QQuickItem *sourceView = shutdownViews.at(sourceIndex);
            QQuickItem *childView = shutdownViews.at(childIndex);
            QObject *childDelegate = delegateInView(childView);
            QVERIFY(childDelegate);
            childDelegateForLease.insert(leaseId, childDelegate);
            childViewForLease.insert(leaseId, childView);
            sourceForLease.insert(leaseId, sourceView);
            QTRY_VERIFY_WITH_TIMEOUT(
                shutdownLifetime.popupOpenerLeaseDelegateCount(leaseId) > 0,
                10000);
        }
        bool eachLeaseReleasedAfterChildDelegate = true;
        bool eachLeaseReleasedWithRetiredTarget = true;
        bool eachLeaseReleasedBeforeSourceDeath = true;
        QSignalSpy shutdownLeaseReleased(
            &shutdownLifetime,
            &FerricProfileLifetime::popupOpenerLeaseReleased);
        connect(&shutdownLifetime,
                &FerricProfileLifetime::popupOpenerLeaseReleased, this,
                [&](const QString &leaseId) {
                    if (!childDelegateForLease.contains(leaseId))
                        return;
                    eachLeaseReleasedAfterChildDelegate =
                        eachLeaseReleasedAfterChildDelegate &&
                        childDelegateForLease.value(leaseId).isNull();
                    QQuickItem *target = childViewForLease.value(leaseId);
                    const bool targetRetired =
                        !target ||
                        (target->property("lifecycleState").toInt() == 2 &&
                         target->property("renderProcessPid").toLongLong() == 0 &&
                         !target->isVisible() && !target->isEnabled() &&
                         shutdownLifetime.ownerUi(target) == nullptr);
                    eachLeaseReleasedWithRetiredTarget =
                        eachLeaseReleasedWithRetiredTarget && targetRetired;
                    eachLeaseReleasedBeforeSourceDeath =
                        eachLeaseReleasedBeforeSourceDeath &&
                        !sourceForLease.value(leaseId).isNull();
                });
        for (int i = 0; i < 3; ++i) {
            const QString leaseId = shutdownRoot->property("popupLeaseIds")
                                        .value<QJSValue>().property(i).toString();
            QVERIFY(!leaseId.isEmpty());
            QTRY_VERIFY_WITH_TIMEOUT(
                shutdownLifetime.popupOpenerLeaseDelegateCount(leaseId) > 0,
                10000);
        }
        QCOMPARE(shutdownLifetime.windowCount(), 4);
        QCOMPARE(shutdownLifetime.viewCount(), 4);
        QCOMPARE(shutdownLifetime.popupOpenerLeaseCount(), 3);
        QSignalSpy shutdownDrained(&shutdownLifetime,
                                   &FerricProfileLifetime::drained);
        shutdownLifetime.shutdown();
        QCOMPARE(shutdownLifetime.windowCount(), 0);
        QVERIFY(shutdownLifetime.popupOpenerLeaseCount() <= 3);
        QVERIFY(shutdownLifetime.acquirePopupOpenerLease(
                    shutdownOpener, shutdownChild).isEmpty());
        QVERIFY(!shutdownLifetime.retainWindow(shutdownWindow, shutdownRoot));
        QTRY_COMPARE_WITH_TIMEOUT(shutdownLifetime.viewCount(), 0, 15000);
        QTRY_COMPARE_WITH_TIMEOUT(shutdownLifetime.popupOpenerLeaseCount(), 0,
                                 15000);
        QTRY_COMPARE_WITH_TIMEOUT(shutdownDrained.count(), 1, 15000);
        QCOMPARE(shutdownLeaseReleased.count(), 3);
        QVERIFY(eachLeaseReleasedAfterChildDelegate);
        QVERIFY(eachLeaseReleasedWithRetiredTarget);
        QVERIFY(eachLeaseReleasedBeforeSourceDeath);
        for (const auto &view : std::as_const(shutdownViewGuards))
            QVERIFY(view.isNull());
        QVERIFY(shutdownProfileAliveForEveryPage);
        QVERIFY(shutdownProfileGuard);
        delete shutdownRoot;
        QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
        QCoreApplication::processEvents();
        QVERIFY(shutdownProfileGuard);
    }
    QVERIFY(shutdownProfileGuard.isNull());
    QVERIFY(shutdownProfileDestroyedAfterConsumers);
    activeLifetime_ = lifetime.get();

    lifetime->releaseWindow(rootWindow);
    lifetime->shutdown();
    QTRY_COMPARE_WITH_TIMEOUT(lifetime->viewCount(), 0, 10000);
    QTRY_COMPARE_WITH_TIMEOUT(lifetime->windowCount(), 0, 10000);
    QTRY_COMPARE_WITH_TIMEOUT(drained.count(), 1, 10000);
    rootWindow->close();
    delete rootObject;
    QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
    QCoreApplication::processEvents();
    QVERIFY(rootWindowGuard.isNull());
    QVERIFY(profileGuard);
    lifetime.reset();
    QVERIFY(profileGuard.isNull());
    activeLifetime_ = nullptr;
}

int main(int argc, char **argv) {
    QTemporaryDir privateXdgRoot;
    if (!privateXdgRoot.isValid())
        return 2;
    QDir xdg(privateXdgRoot.path());
    if (!xdg.mkpath("config") || !xdg.mkpath("cache") ||
        !xdg.mkpath("data") || !xdg.mkpath("runtime"))
        return 2;
    const QByteArray runtime = QFile::encodeName(xdg.filePath("runtime"));
    QFile::setPermissions(xdg.filePath("runtime"),
                          QFileDevice::ReadOwner | QFileDevice::WriteOwner |
                              QFileDevice::ExeOwner);
    qputenv("XDG_CONFIG_HOME", QFile::encodeName(xdg.filePath("config")));
    qputenv("XDG_CACHE_HOME", QFile::encodeName(xdg.filePath("cache")));
    qputenv("XDG_DATA_HOME", QFile::encodeName(xdg.filePath("data")));
    qputenv("XDG_RUNTIME_DIR", runtime);
    qputenv("QT_QPA_PLATFORM", "offscreen");
    qputenv("QT_QUICK_BACKEND", "software");
    qunsetenv("QTWEBENGINE_DISABLE_SANDBOX");
    QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
    QtWebEngineQuick::initialize();
    QGuiApplication app(argc, argv);
    LifetimeTests tests;
    return QTest::qExec(&tests, argc, argv);
}
#include "tst_lifetime.moc"
