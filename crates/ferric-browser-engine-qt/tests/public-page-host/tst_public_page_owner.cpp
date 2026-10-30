#include "public_page_owner.h"
#include "public_profile_owner.h"
#include "request_interceptor.h"
#include "native_page_surface.h"

#include <QApplication>

#include <QElapsedTimer>
#include <QHostAddress>
#include <QNetworkProxy>
#include <QPointer>
#include <QQmlContext>
#include <QQmlEngine>
#include <QtTest/QSignalSpy>
#include <QTcpServer>
#include <QTcpSocket>
#include <QTest>
#include <QTemporaryDir>
#include <QUrl>
#include <QtWebEngineCore/QWebEngineNewWindowRequest>
#include <QtWebEngineCore/QWebEnginePage>
#include <QtWebEngineCore/QWebEngineProfile>
#include <QtWebEngineCore/QWebEngineSettings>
#include <QtWebEngineCore/QWebEngineScriptCollection>
#include <QtWebEngineWidgets/QWebEngineView>
#include <QtWebEngineQuick/QQuickWebEngineProfile>

#include <functional>
#include <limits>
#include <memory>
#include <vector>

namespace {

// Persistent Chromium profiles may complete IO until application shutdown.
// Keep their disposable storage alive until Qt has joined those threads.
std::vector<std::unique_ptr<QTemporaryDir>> profileStorage;

bool waitUntil(const std::function<bool()> &condition, int timeoutMs = 10000) {
  QElapsedTimer timer;
  timer.start();
  while (timer.elapsed() < timeoutMs) {
    if (condition()) {
      return true;
    }
    QCoreApplication::processEvents(QEventLoop::AllEvents, 25);
    QTest::qWait(5);
  }
  return condition();
}

bool evaluate(FerricPublicPageOwner *owner, const QString &script,
              QVariant *result, int timeoutMs = 10000) {
  bool completed = false;
  if (!owner->runJavaScript(script,
                            [&completed, result](const QVariant &value) {
                              *result = value;
                              completed = true;
                            })) {
    return false;
  }
  return waitUntil([&completed] { return completed; }, timeoutMs);
}

class LoopbackServer final : public QObject {
public:
  explicit LoopbackServer(QObject *parent = nullptr) : QObject(parent) {
    connect(&server_, &QTcpServer::newConnection, this, [this] {
      while (QTcpSocket *socket = server_.nextPendingConnection()) {
        connect(socket, &QTcpSocket::readyRead, socket, [this, socket] {
          const QByteArray request = socket->readAll();
          if (request.isEmpty() || socket->property("answered").toBool()) {
            return;
          }
          socket->setProperty("answered", true);
          ++requests_;
          paths_.append(QString::fromUtf8(request.split(' ').value(1)));
          const QByteArray body("owner-policy-ok");
          const QByteArray response =
              "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n" +
              QByteArray("Content-Length: ") + QByteArray::number(body.size()) +
              "\r\nConnection: close\r\n\r\n" + body;
          socket->write(response);
          socket->disconnectFromHost();
        });
        connect(socket, &QTcpSocket::disconnected, socket,
                &QObject::deleteLater);
      }
    });
  }

  bool listen() { return server_.listen(QHostAddress::LocalHost, 0); }
  int requests() const { return requests_; }
  int requestsForPath(const QString &path) const { return paths_.count(path); }
  QStringList paths() const { return paths_; }
  QUrl url() const {
    return QUrl(
        QStringLiteral("http://127.0.0.1:%1/policy").arg(server_.serverPort()));
  }

private:
  QTcpServer server_;
  int requests_ = 0;
  QStringList paths_;
};

} // namespace

class PublicPageOwnerTests final : public QObject {
  Q_OBJECT

private slots:
  void sharedNativeSurfacePreservesReservedScriptsAndRevokesCallbacks() {
    FerricBrowserRequestInterceptor interceptor;
    FerricPublicProfileOwner owner(&interceptor);
    QObject lifetime;
    auto *pageOwner = owner.createPage(&lifetime);
    QVERIFY(pageOwner);
    QWebEngineView view;
    view.setPage(pageOwner->page());
    QQmlEngine engine;
    FerricNativePageSurface surface;
    QQmlEngine::setContextForObject(&surface, engine.rootContext());
    surface.settings()->setForceDarkMode(true);
    QVERIFY(surface.bind(pageOwner, &view));
    QVERIFY(!surface.bind(pageOwner, &view));
    QVERIFY(pageOwner->page()->settings()->testAttribute(QWebEngineSettings::ForceDarkMode));
    surface.settings()->setForceDarkMode(false);
    QVERIFY(!pageOwner->page()->settings()->testAttribute(QWebEngineSettings::ForceDarkMode));
    surface.setZoomFactor(1.5);
    QCOMPARE(surface.zoomFactor(), 1.5);
    surface.setZoomFactor(std::numeric_limits<qreal>::quiet_NaN());
    QCOMPARE(surface.zoomFactor(), 1.5);
    QWebEngineScript reserved;
    reserved.setName("credential-private-fixture");
    reserved.setWorldId(42);
    reserved.setInjectionPoint(QWebEngineScript::DocumentCreation);
    reserved.setSourceCode("globalThis.privateSentinel=42");
    pageOwner->page()->scripts().insert(reserved);
    auto ordinary = engine.evaluate("({name:'browser-fixture',sourceCode:'globalThis.ordinarySentinel=7',worldId:1,injectionPoint:2,runsOnSubFrames:false})");
    QVERIFY(surface.userScripts()->insert(ordinary));
    auto forbidden = engine.evaluate("({name:'credential-private-fixture',sourceCode:'',worldId:42,injectionPoint:2,runsOnSubFrames:false})");
    QVERIFY(!surface.userScripts()->insert(forbidden));
    QVERIFY(!surface.userScripts()->remove(forbidden));
    QVERIFY(surface.userScripts()->find("credential-private-fixture").isEmpty());
    QCOMPARE(surface.userScripts()->find("browser-fixture").size(), 1);
    auto malformed = engine.evaluate("({name:'bad',sourceCode:'',worldId:NaN,injectionPoint:2,runsOnSubFrames:false})");
    QVERIFY(!surface.userScripts()->insert(malformed));
    surface.userScripts()->clear();
    QVERIFY(pageOwner->page()->scripts().contains(reserved));
    QVERIFY(surface.userScripts()->find("browser-fixture").isEmpty());
    QVERIFY(surface.userScripts()->insert(ordinary));

    QSignalSpy load(pageOwner->page(), &QWebEnginePage::loadFinished);
    pageOwner->page()->setHtml("<title>surface</title><p>native native</p>", QUrl("https://surface.test/"));
    QVERIFY(load.wait(15000));
    QVERIFY(load.takeFirst().at(0).toBool());
    QCOMPARE(surface.title(), QStringLiteral("surface"));
    QCOMPARE(surface.history(), pageOwner->page()->history());
    QVERIFY(surface.mainFrame().isValid());
    auto callback = engine.evaluate("var observed=0;(function(v){observed=v})");
    QVERIFY(callback.isCallable());
    surface.runJavaScript("ordinarySentinel", 1, callback);
    QVERIFY(waitUntil([&] { return engine.evaluate("observed").toInt() == 7; }));
    surface.runJavaScript("globalThis.privateSentinel=-1", 42);
    bool privateRead = false;
    pageOwner->page()->mainFrame().runJavaScript("privateSentinel", 42, [&](const QVariant &value) {
      privateRead = value.toInt() == 42;
    });
    QVERIFY(waitUntil([&] { return privateRead; }));
    view.resize(640, 420);
    view.show();
    QVERIFY(QTest::qWaitForWindowExposed(&view));
    QSignalSpy find(&surface, &FerricNativePageSurface::findTextFinished);
    surface.findText("native");
    QVERIFY(waitUntil([&] { return find.count() == 1; }));
    QCOMPARE(find.takeFirst().at(0).value<QWebEngineFindTextResult>().numberOfMatches(), 2);

    // Browser callbacks queued in the renderer cannot retain authority after
    // the page is moved, even if the logical document/page pointer survives.
    engine.evaluate("observed=0");
    surface.runJavaScript("99", 1, callback);
    pageOwner->invalidatePresentation();
    bool drained = false;
    pageOwner->page()->mainFrame().runJavaScript("1", 1, [&](const QVariant &) { drained = true; });
    QVERIFY(waitUntil([&] { return drained; }));
    QCOMPARE(engine.evaluate("observed").toInt(), 0);
    QVERIFY(pageOwner->close());
    QVERIFY(!surface.page());
    QVERIFY(!surface.isEnabled());
    QVERIFY(!surface.userScripts()->insert(ordinary));
  }

  void persistentProfilesAreConfiguredBeforePageCreation() {
    profileStorage.push_back(std::make_unique<QTemporaryDir>());
    auto &storage = *profileStorage.back();
    QVERIFY(storage.isValid());
    FerricPublicProfileOwner::Configuration configuration{
        QStringLiteral("ferric-owner-fixture"), storage.path() + "/data",
        storage.path() + "/cache"};
    FerricBrowserRequestInterceptor interceptor;
    FerricPublicProfileOwner owner(&interceptor, configuration);
    QVERIFY(owner.isReady());
    QVERIFY(!owner.profile()->isOffTheRecord());
    QCOMPARE(owner.profile()->storageName(), configuration.storageName);
    QCOMPARE(owner.profile()->persistentStoragePath(), configuration.persistentStoragePath);
    QCOMPARE(owner.profile()->cachePath(), configuration.cachePath);
    QCOMPARE(owner.profile()->persistentPermissionsPolicy(),
             QWebEngineProfile::PersistentPermissionsPolicy::AskEveryTime);

    FerricBrowserRequestInterceptor otherInterceptor;
    FerricPublicProfileOwner duplicate(&otherInterceptor, configuration);
    QVERIFY(!duplicate.isReady());
    QCOMPARE(duplicate.pageCount(), 0);
    QObject lifetime;
    auto *page = owner.createPage(&lifetime);
    QVERIFY(page);
    QSignalSpy load(page->page(), &QWebEnginePage::loadFinished);
    page->page()->setHtml("<title>persistent owner</title>", QUrl("https://owner.test/"));
    QVERIFY(load.wait(15000));
    QVERIFY(load.takeFirst().at(0).toBool());
    QVariant value;
    QVERIFY(evaluate(page, "localStorage.setItem('ownerFixture','persistent');localStorage.ownerFixture", &value));
    QCOMPARE(value.toString(), QStringLiteral("persistent"));

    FerricBrowserRequestInterceptor privateInterceptor;
    FerricPublicProfileOwner privateOwner(&privateInterceptor);
    QVERIFY(privateOwner.isReady());
    QVERIFY(privateOwner.profile()->isOffTheRecord());
    auto *privatePage = privateOwner.createPage(&lifetime);
    QVERIFY(privatePage);
    QSignalSpy privateLoad(privatePage->page(), &QWebEnginePage::loadFinished);
    privatePage->page()->setHtml("<title>private owner</title>", QUrl("https://owner.test/"));
    QVERIFY(privateLoad.wait(15000));
    QVERIFY(evaluate(privatePage, "localStorage.getItem('ownerFixture')===null", &value));
    QVERIFY(value.toBool());
    QVERIFY(privateOwner.shutdown());
    QVERIFY(owner.shutdown());

    // An empty storage name never accepts disk paths, and a persistent name
    // never accepts implicit relative storage. Neither case allocates pages.
    configuration.storageName.clear();
    FerricPublicProfileOwner invalidPrivate(&otherInterceptor, configuration);
    QVERIFY(!invalidPrivate.isReady());
    configuration.storageName = "persistent";
    configuration.persistentStoragePath = "relative";
    FerricPublicProfileOwner invalidPersistent(&otherInterceptor, configuration);
    QVERIFY(!invalidPersistent.isReady());
  }

  void plainProfileOwnsPageAndAppliesExistingRequestPolicy() {
    QNetworkProxy::setApplicationProxy(QNetworkProxy::NoProxy);
    LoopbackServer server;
    QVERIFY(server.listen());

    FerricBrowserRequestInterceptor interceptor;
    interceptor.setEnabled(true);
    FerricPublicProfileOwner owner(&interceptor);
    QVERIFY(owner.isReady());
    QVERIFY(owner.profile()->isOffTheRecord());
    QObject host;
    FerricPublicPageOwner *pageOwner = owner.createPage(&host);
    QVERIFY(pageOwner);
    QCOMPARE(pageOwner->profile(), owner.profile());
    QCOMPARE(owner.pageCount(), 1);

    QPointer<QWebEngineProfile> profile(owner.profile());
    QSignalSpy firstLoad(pageOwner->page(), &QWebEnginePage::loadFinished);
    QUrl allowedUrl = server.url();
    allowedUrl.setPath(QStringLiteral("/allowed"));
    pageOwner->page()->load(allowedUrl);
    QVERIFY(firstLoad.wait(15000));
    QVERIFY(firstLoad.takeFirst().at(0).toBool());
    QVERIFY2(waitUntil([&] { return server.requestsForPath("/allowed") == 1; }), qPrintable(server.paths().join(',')));

    interceptor.setSecurityDenyHosts({QStringLiteral("127.0.0.1")});
    const auto beforeBlocked = interceptor.blockedCount();
    QSignalSpy blockedLoad(pageOwner->page(), &QWebEnginePage::loadFinished);
    QUrl deniedUrl = server.url();
    deniedUrl.setPath(QStringLiteral("/denied"));
    pageOwner->page()->load(deniedUrl);
    QVERIFY(waitUntil([&] {
      return interceptor.blockedCount() > beforeBlocked ||
             !blockedLoad.isEmpty();
    }));
    QVERIFY(interceptor.blockedCount() > beforeBlocked);
    QCOMPARE(server.requestsForPath("/allowed"), 1);
    QCOMPARE(server.requestsForPath("/denied"), 0);

    interceptor.setSecurityDenyHosts({});
    QSignalSpy restoredLoad(pageOwner->page(), &QWebEnginePage::loadFinished);
    QUrl restoredUrl = server.url();
    restoredUrl.setPath(QStringLiteral("/restored"));
    pageOwner->page()->load(restoredUrl);
    QVERIFY(restoredLoad.wait(15000));
    QVERIFY(restoredLoad.takeFirst().at(0).toBool());
    QVERIFY2(waitUntil([&] { return server.requestsForPath("/restored") == 1; }), qPrintable(server.paths().join(',')));
    QCOMPARE(server.requestsForPath("/denied"), 0);

    QVERIFY(owner.shutdown());
    QVERIFY(profile.isNull());
    QCOMPARE(owner.pageCount(), 0);
    QVERIFY(!owner.isReady());
  }

  void quickProfileInterceptorRegression() {
    FerricBrowserRequestInterceptor interceptor;
    QQuickWebEngineProfile quickProfile;
    QVERIFY(interceptor.attach(&quickProfile));
    QVERIFY(interceptor.detach(&quickProfile));
  }

  void sameProfilePopupAndDescendantsSurviveSourceClose() {
    FerricBrowserRequestInterceptor interceptor;
    FerricPublicProfileOwner owner(&interceptor);
    QVERIFY(owner.isReady());
    owner.profile()->settings()->setAttribute(
        QWebEngineSettings::JavascriptCanOpenWindows, true);

    QObject topHost;
    QObject childHost;
    QObject grandchildHost;
    auto *top = owner.createPage(&topHost);
    QVERIFY(top);
    auto *child = owner.createPage(&childHost);
    QVERIFY(child);
    auto *grandchild = owner.createPage(&grandchildHost);
    QVERIFY(grandchild);
    QCOMPARE(owner.pageCount(), 3);

    bool childAccepted = false;
    connect(top->page(), &QWebEnginePage::newWindowRequested, this,
            [&](QWebEngineNewWindowRequest &request) {
              childAccepted = child->adoptNewWindowFrom(top, request);
            });
    top->page()->setHtml(
        QStringLiteral("<html><body><script>window.open('about:blank','child')<"
                       "/script></body></html>"),
        QUrl(QStringLiteral("https://fixture.invalid/top")));
    QVERIFY(waitUntil([&] { return childAccepted; }));

    QVariant openerPresent;
    QVERIFY(waitUntil([&] {
      return evaluate(child, QStringLiteral("window.opener !== null"),
                      &openerPresent) && openerPresent.toBool();
    }, 5000));
    QVERIFY(openerPresent.toBool());

    bool grandchildAccepted = false;
    connect(child->page(), &QWebEnginePage::newWindowRequested, this,
            [&](QWebEngineNewWindowRequest &request) {
              grandchildAccepted =
                  grandchild->adoptNewWindowFrom(child, request);
            });
    QVariant openResult;
    QVERIFY(evaluate(
        child, QStringLiteral("window.open('about:blank','grandchild'); true"),
        &openResult));
    QVERIFY(waitUntil([&] { return grandchildAccepted; }));

    QPointer<QWebEnginePage> topPage(top->page());
    QPointer<QWebEnginePage> childPage(child->page());
    QPointer<QWebEnginePage> grandchildPage(grandchild->page());
    QVERIFY(top->close());
    QVERIFY(topPage.isNull());
    QVERIFY(!childPage.isNull());
    QVERIFY(!grandchildPage.isNull());
    QVERIFY(owner.profile() != nullptr);
    QCOMPARE(owner.pageCount(), 2);

    QVariant openerClosed;
    QVERIFY(evaluate(
        child, QStringLiteral("window.opener === null || window.opener.closed === true"),
        &openerClosed));
    QVERIFY(openerClosed.toBool());
    QVariant ancestorClosed;
    QVERIFY(evaluate(
        grandchild,
        QStringLiteral("window.opener !== null && (window.opener.opener === null || window.opener.opener.closed === true)"),
        &ancestorClosed));
    QVERIFY(ancestorClosed.toBool());

    QVERIFY(child->close());
    QVERIFY(grandchild->close());
    QCOMPARE(owner.pageCount(), 0);
    QVERIFY(owner.shutdown());
  }

  void crossProfilePopupAdoptionIsRejected() {
    FerricBrowserRequestInterceptor interceptor;
    FerricPublicProfileOwner sourceOwner(&interceptor);
    FerricPublicProfileOwner destinationOwner(&interceptor);
    QVERIFY(sourceOwner.isReady());
    QVERIFY(destinationOwner.isReady());
    QVERIFY(sourceOwner.profile() != destinationOwner.profile());
    sourceOwner.profile()->settings()->setAttribute(
        QWebEngineSettings::JavascriptCanOpenWindows, true);

    QObject sourceHost;
    QObject destinationHost;
    auto *source = sourceOwner.createPage(&sourceHost);
    auto *destination = destinationOwner.createPage(&destinationHost);
    QVERIFY(source);
    QVERIFY(destination);
    bool requestSeen = false;
    bool adopted = true;
    connect(source->page(), &QWebEnginePage::newWindowRequested, this,
            [&](QWebEngineNewWindowRequest &request) {
              requestSeen = true;
              adopted = destination->adoptNewWindowFrom(source, request);
            });
    source->page()->setHtml(
        QStringLiteral(
            "<script>window.open('about:blank','cross-profile')</script>"),
        QUrl(QStringLiteral("https://fixture.invalid/source")));
    QVERIFY(waitUntil([&] { return requestSeen; }));
    QVERIFY(!adopted);
    QVERIFY(destination->page()->url().isEmpty() ||
            destination->page()->url() == QUrl(QStringLiteral("about:blank")));
    QVERIFY(source->close());
    QVERIFY(destination->close());
    QVERIFY(sourceOwner.shutdown());
    QVERIFY(destinationOwner.shutdown());
  }

  void delayedCallbacksAreInvalidatedByNavigationCloseAndHostDeath() {
    FerricBrowserRequestInterceptor interceptor;
    FerricPublicProfileOwner owner(&interceptor);
    QObject host;
    auto *pageOwner = owner.createPage(&host);
    QVERIFY(pageOwner);
    const auto original = pageOwner->captureOperation();

    QSignalSpy firstLoad(pageOwner->page(), &QWebEnginePage::loadFinished);
    pageOwner->page()->setHtml(QStringLiteral("<p>first</p>"));
    QVERIFY(firstLoad.wait(15000));
    QVERIFY(firstLoad.takeFirst().at(0).toBool());
    QVERIFY(pageOwner->documentGeneration() > 0);
    QVERIFY(!pageOwner->isCurrent(original));

    QVariant title;
    QVERIFY(
        evaluate(pageOwner, QStringLiteral("document.body.innerText"), &title));
    QCOMPARE(title.toString(), QStringLiteral("first"));

    const auto pending = pageOwner->captureOperation();
    bool delivered = false;
    QVERIFY(
        pageOwner->queueIfCurrent(pending, [&delivered] { delivered = true; }));
    QVERIFY(pageOwner->close());
    QCoreApplication::processEvents(QEventLoop::AllEvents, 25);
    QVERIFY(!delivered);
    QVERIFY(!pageOwner->close());
    QCOMPARE(owner.pageCount(), 0);

    auto hostOwner = std::make_unique<QObject>();
    QPointer<QWebEnginePage> hostPage;
    auto *hostBoundPage = owner.createPage(hostOwner.get());
    QVERIFY(hostBoundPage);
    hostPage = hostBoundPage->page();
    hostOwner.reset();
    QVERIFY(hostPage.isNull());
    QCOMPARE(owner.pageCount(), 0);
    QVERIFY(owner.shutdown());
  }
};

int main(int argc, char **argv)
{
  int result;
  {
    QApplication application(argc, argv);
    PublicPageOwnerTests tests;
    result = QTest::qExec(&tests, argc, argv);
  }
  profileStorage.clear();
  return result;
}
#include "tst_public_page_owner.moc"
