#include "public_page_owner.h"
#include "public_profile_owner.h"
#include "request_interceptor.h"

#include <QElapsedTimer>
#include <QHostAddress>
#include <QNetworkProxy>
#include <QPointer>
#include <QtTest/QSignalSpy>
#include <QTcpServer>
#include <QTcpSocket>
#include <QTest>
#include <QUrl>
#include <QtWebEngineCore/QWebEngineNewWindowRequest>
#include <QtWebEngineCore/QWebEnginePage>
#include <QtWebEngineCore/QWebEngineProfile>
#include <QtWebEngineCore/QWebEngineSettings>
#include <QtWebEngineQuick/QQuickWebEngineProfile>

#include <functional>

namespace {

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
  QUrl url() const {
    return QUrl(
        QStringLiteral("http://127.0.0.1:%1/policy").arg(server_.serverPort()));
  }

private:
  QTcpServer server_;
  int requests_ = 0;
};

} // namespace

class PublicPageOwnerTests final : public QObject {
  Q_OBJECT

private slots:
  void plainProfileOwnsPageAndAppliesExistingRequestPolicy() {
    QNetworkProxy::setApplicationProxy(QNetworkProxy::NoProxy);
    LoopbackServer server;
    QVERIFY(server.listen());

    FerricBrowserRequestInterceptor interceptor;
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
    QVERIFY(waitUntil([&] { return server.requests() == 1; }));

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
    QCOMPARE(server.requests(), 1);

    interceptor.setSecurityDenyHosts({});
    QSignalSpy restoredLoad(pageOwner->page(), &QWebEnginePage::loadFinished);
    QUrl restoredUrl = server.url();
    restoredUrl.setPath(QStringLiteral("/restored"));
    pageOwner->page()->load(restoredUrl);
    QVERIFY(restoredLoad.wait(15000));
    QVERIFY(restoredLoad.takeFirst().at(0).toBool());
    QVERIFY(waitUntil([&] { return server.requests() == 2; }));

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
    QVERIFY(
        evaluate(child, QStringLiteral("window.opener.closed"), &openerClosed));
    QVERIFY(openerClosed.toBool());
    QVariant ancestorClosed;
    QVERIFY(evaluate(grandchild, QStringLiteral("window.opener.opener.closed"),
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

QTEST_MAIN(PublicPageOwnerTests)
#include "tst_public_page_owner.moc"
