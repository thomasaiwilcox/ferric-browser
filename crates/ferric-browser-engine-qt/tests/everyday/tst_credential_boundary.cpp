#include <QEventLoop>
#include <QFile>
#include <QGuiApplication>
#include <QHostAddress>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLibraryInfo>
#include <QProcess>
#include <QQuickWindow>
#include <QSignalSpy>
#include <QSslCertificate>
#include <QSslConfiguration>
#include <QSslKey>
#include <QSslServer>
#include <QSslSocket>
#include <QTcpSocket>
#include <QTemporaryDir>
#include <QTimer>
#include <QWebChannel>
#include <QWebEngineCertificateError>
#include <QWebEngineFrame>
#include <QWebEnginePage>
#include <QWebEngineProfile>
#include <QWebEngineScript>
#include <QWebEngineScriptCollection>
#include <QWebEngineSettings>
#include <QtTest>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>

namespace {
constexpr quint32 CredentialWorld = 42;

class HarmlessProbe final : public QObject {
  Q_OBJECT
public:
  int callCount() const { return callCount_; }
  QString origin() const { return origin_; }
  bool secureContext() const { return secureContext_; }
  QString documentNonce() const { return documentNonce_; }
public slots:
  void record(const QString &origin, bool secureContext,
              const QString &documentNonce) {
    ++callCount_;
    origin_ = origin;
    secureContext_ = secureContext;
    documentNonce_ = documentNonce;
  }

private:
  int callCount_ = 0;
  QString origin_;
  bool secureContext_ = false;
  QString documentNonce_;
};

class FixtureServer final : public QObject {
  Q_OBJECT
public:
  bool start() {
    QTemporaryDir temp;
    if (!temp.isValid())
      return false;
    const auto keyPath = temp.filePath(QStringLiteral("key.pem"));
    const auto certPath = temp.filePath(QStringLiteral("cert.pem"));
    QProcess openssl;
    openssl.start(QStringLiteral(FERRIC_TEST_OPENSSL),
                  {QStringLiteral("req"), QStringLiteral("-x509"),
                   QStringLiteral("-newkey"), QStringLiteral("rsa:2048"),
                   QStringLiteral("-nodes"), QStringLiteral("-days"),
                   QStringLiteral("1"), QStringLiteral("-subj"),
                   QStringLiteral("/CN=localhost"), QStringLiteral("-addext"),
                   QStringLiteral("subjectAltName=DNS:localhost"),
                   QStringLiteral("-keyout"), keyPath, QStringLiteral("-out"),
                   certPath});
    if (!openssl.waitForFinished(10000) || openssl.exitCode() != 0)
      return false;
    QFile key(keyPath), cert(certPath);
    if (!key.open(QIODevice::ReadOnly) || !cert.open(QIODevice::ReadOnly))
      return false;
    auto config = QSslConfiguration::defaultConfiguration();
    config.setLocalCertificate(QSslCertificate(cert.readAll()));
    config.setPrivateKey(QSslKey(key.readAll(), QSsl::Rsa));
    config.setPeerVerifyMode(QSslSocket::VerifyNone);
    server_.setSslConfiguration(config);
    connect(&server_, &QTcpServer::pendingConnectionAvailable, this, [this] {
      while (auto *socket = server_.nextPendingConnection()) {
        const auto serveRequest = [this, socket] {
          auto bytes =
              socket->property("request").toByteArray() + socket->readAll();
          socket->setProperty("request", bytes);
          if (!bytes.contains("\r\n\r\n"))
            return;
          const auto path = bytes.split(' ').value(1);
          const auto body = path == "/frame" ? frame_ : page_;
          socket->write(
              "HTTP/1.1 200 OK\r\nContent-Type: text/html; "
              "charset=utf-8\r\nConnection: close\r\nContent-Length: " +
              QByteArray::number(body.size()) + "\r\n\r\n" + body);
          socket->disconnectFromHost();
        };
        connect(socket, &QTcpSocket::readyRead, socket, serveRequest);
        if (socket->bytesAvailable() > 0)
          serveRequest();
      }
    });
    return server_.listen(QHostAddress::LocalHost);
  }
  void setDocuments(QByteArray page, QByteArray frame) {
    page_ = std::move(page);
    frame_ = std::move(frame);
  }
  QUrl pageUrl() const {
    return QUrl(
        QStringLiteral("https://localhost:%1/page").arg(server_.serverPort()));
  }

private:
  QSslServer server_;
  QByteArray page_;
  QByteArray frame_;
};

QVariant run(QWebEngineFrame frame, const QString &source, quint32 world) {
  QVariant result;
  bool done = false;
  QEventLoop loop;
  QTimer timer;
  timer.setSingleShot(true);
  frame.runJavaScript(source, world,
                      [&result, &done, &loop](const QVariant &value) {
    result = value;
    done = true;
    loop.quit();
  });
  QObject::connect(&timer, &QTimer::timeout, &loop, &QEventLoop::quit);
  QObject::connect(qApp, &QCoreApplication::aboutToQuit, &loop,
                   &QEventLoop::quit);
  timer.start(5000);
  while (!done && timer.isActive())
    loop.exec();
  if (!done)
    qFatal("WebEngine frame JavaScript callback timed out");
  return result;
}

QByteArray readFixture(const QString &name) {
  QFile file(QStringLiteral(FERRIC_CREDENTIAL_FIXTURE_DIR) + QLatin1Char('/') +
             name);
  if (!file.open(QIODevice::ReadOnly))
    return {};
  return file.readAll();
}
} // namespace

class CredentialBoundaryTests final : public QObject {
  Q_OBJECT
private slots:
  void isolatedTransportAndMainFrameFacts() {
    auto pageHtml = readFixture(QStringLiteral("credential-hostile.html"));
    auto frameHtml = readFixture(QStringLiteral("credential-frame.html"));
    QVERIFY(!pageHtml.isEmpty());
    QVERIFY(!frameHtml.isEmpty());
    FixtureServer server;
    QVERIFY(server.start());
    const auto frameUrl = QStringLiteral("https://localhost:%1/frame")
                              .arg(server.pageUrl().port());
    pageHtml.replace("__SAME_ORIGIN_FRAME_URL__", frameUrl.toUtf8());
    pageHtml.replace("__RELAXED_ORIGIN_FRAME_URL__", frameUrl.toUtf8());
    server.setDocuments(pageHtml, frameHtml);
    QWebEngineProfile profile;
    profile.setHttpCacheType(QWebEngineProfile::NoCache);
    QWebEnginePage page(&profile);
    connect(&page, &QWebEnginePage::certificateError, &page,
            [](QWebEngineCertificateError error) {
              // Accept only this disposable, local self-signed test cert.
              error.acceptCertificate();
            });
    QWebChannel channel;
    HarmlessProbe probe;
    channel.registerObject(QStringLiteral("probe"), &probe);
    page.setWebChannel(&channel, CredentialWorld);

    auto webChannelPath = QLibraryInfo::path(QLibraryInfo::DataPath) +
                          QStringLiteral("/webchannel/qwebchannel.js");
    if (!QFile::exists(webChannelPath)) {
      webChannelPath = QLibraryInfo::path(QLibraryInfo::PrefixPath) +
                       QStringLiteral("/share/qt6/webchannel/qwebchannel.js");
    }
    QFile webChannelJs(webChannelPath);
    QVERIFY2(webChannelJs.open(QIODevice::ReadOnly),
             qPrintable(webChannelJs.fileName()));
    const auto webChannelSource = QString::fromUtf8(webChannelJs.readAll());

    QWebEngineScript init;
    init.setName(QStringLiteral("credential-test-world42-channel"));
    init.setInjectionPoint(QWebEngineScript::DocumentCreation);
    init.setWorldId(CredentialWorld);
    init.setRunsOnSubFrames(false);
    init.setSourceCode(
        webChannelSource +
        QStringLiteral(
            "\nwindow.__credentialDocumentNonce = crypto.randomUUID();"
            "new QWebChannel(qt.webChannelTransport, function(channel) {"
            "channel.objects.probe.record(location.origin, "
            "window.isSecureContext, window.__credentialDocumentNonce); });"));
    page.scripts().insert(init);

    const auto installHostileWorldAttempt =
        [&](const QString &name, quint32 world, const QString &availabilityFlag,
            const QString &sentFlag) {
          QWebEngineScript attacker;
          attacker.setName(name);
          attacker.setInjectionPoint(QWebEngineScript::DocumentCreation);
          attacker.setWorldId(world);
          attacker.setRunsOnSubFrames(true);
          const auto hostileAttempt =
              QStringLiteral("\nwindow.%1 = typeof qt !== 'undefined' && "
                             "!!qt.webChannelTransport;"
                             "window.%2 = false;"
                             "if (window.%1) { new QWebChannel("
                             "qt.webChannelTransport, function(channel) {"
                             "window.%2 = true;"
                             "channel.objects.probe.record("
                             "'hostile-world-claim', false, 'forged'); }); }")
                  .arg(availabilityFlag, sentFlag);
          attacker.setSourceCode(webChannelSource + hostileAttempt);
          page.scripts().insert(attacker);
        };
    installHostileWorldAttempt(QStringLiteral("credential-hostile-main-world"),
                               QWebEngineScript::MainWorld,
                               QStringLiteral("__credentialMainWorldTransport"),
                               QStringLiteral("__credentialMainWorldSent"));
    installHostileWorldAttempt(
        QStringLiteral("credential-hostile-application-world"),
        QWebEngineScript::ApplicationWorld,
        QStringLiteral("__credentialApplicationWorldTransport"),
        QStringLiteral("__credentialApplicationWorldSent"));

    QSignalSpy loaded(&page, &QWebEnginePage::loadFinished);
    page.load(server.pageUrl());
    QVERIFY(loaded.wait(15000));
    QVERIFY(loaded.takeFirst().first().toBool());
    QVERIFY(page.mainFrame().isValid());
    QTRY_VERIFY_WITH_TIMEOUT(page.mainFrame().children().size() == 2, 10000);

    // Native frame-handle facts are kept distinct from browser-calculated
    // JavaScript facts returned by execution in world 42.
    const auto mainFrame = page.mainFrame();
    QVERIFY(mainFrame.isMainFrame());
    QCOMPARE(mainFrame.url(), page.url());
    QVERIFY(mainFrame.isValid());
    const auto browserOrigin =
        run(mainFrame, QStringLiteral("location.origin"), CredentialWorld)
            .toString();
    const auto browserSecureContext =
        run(mainFrame, QStringLiteral("window.isSecureContext"),
            CredentialWorld)
            .toBool();
    const auto nonce =
        run(mainFrame, QStringLiteral("window.__credentialDocumentNonce"),
            CredentialWorld)
            .toString();
    QCOMPARE(
        browserOrigin,
        QStringLiteral("https://localhost:%1").arg(server.pageUrl().port()));
    QVERIFY(browserSecureContext);
    QVERIFY(!nonce.isEmpty());
    QTRY_COMPARE_WITH_TIMEOUT(probe.callCount(), 1, 5000);
    QCOMPARE(probe.origin(), browserOrigin);
    QCOMPARE(probe.secureContext(), browserSecureContext);
    QCOMPARE(probe.documentNonce(), nonce);
    QCOMPARE(run(mainFrame, QStringLiteral("document.domain"), 0).toString(),
             QStringLiteral("localhost"));
    QCOMPARE(run(mainFrame, QStringLiteral("document.domain"), CredentialWorld)
                 .toString(),
             QStringLiteral("localhost"));
    QVERIFY(!run(mainFrame,
                 QStringLiteral(
                     "Object.prototype.credentialPoison === 'page-world-only'"),
                 CredentialWorld)
                 .toBool());
    QVERIFY(run(mainFrame,
                QStringLiteral(
                    "typeof qt !== 'undefined' && !!qt.webChannelTransport"),
                CredentialWorld)
                .toBool());

    // Page script can claim a different origin, replace `qt`, poison its
    // own prototype, and call its fake transport without reaching native.
    QCOMPARE(run(mainFrame, QStringLiteral("window.credentialClaimedOrigin"), 0)
                 .toString(),
             QStringLiteral("https://claimed.example"));
    QCOMPARE(
        run(mainFrame, QStringLiteral("window.credentialFakeTransportSends"), 0)
            .toInt(),
        1);
    QVERIFY(probe.origin() != QStringLiteral("https://claimed.example"));
    QCOMPARE(probe.callCount(), 1);
    QCOMPARE(run(mainFrame,
                 QStringLiteral(
                     "typeof qt !== 'undefined' && !!qt.webChannelTransport"),
                 0)
                 .toBool(),
             true);
    QCOMPARE(run(mainFrame,
                 QStringLiteral(
                     "typeof qt !== 'undefined' && !!qt.webChannelTransport"),
                 1)
                 .toBool(),
             false);
    QCOMPARE(run(mainFrame,
                 QStringLiteral("window.__credentialMainWorldTransport"),
                 QWebEngineScript::MainWorld)
                 .toBool(),
             false);
    QCOMPARE(run(mainFrame, QStringLiteral("window.__credentialMainWorldSent"),
                 QWebEngineScript::MainWorld)
                 .toBool(),
             false);
    QCOMPARE(run(mainFrame,
                 QStringLiteral("window.__credentialApplicationWorldTransport"),
                 QWebEngineScript::ApplicationWorld)
                 .toBool(),
             false);
    QCOMPARE(run(mainFrame,
                 QStringLiteral("window.__credentialApplicationWorldSent"),
                 QWebEngineScript::ApplicationWorld)
                 .toBool(),
             false);
    auto childFrames = mainFrame.children();
    QCOMPARE(childFrames.size(), 2);
    for (auto child : childFrames) {
      QVERIFY(child.isValid());
      QVERIFY(!child.isMainFrame());
      QCOMPARE(child.url().host(), QStringLiteral("localhost"));
      QTRY_VERIFY_WITH_TIMEOUT(
          run(child, QStringLiteral("document.readyState"), CredentialWorld)
                  .toString() == "complete",
          10000);
      QCOMPARE(run(child,
                   QStringLiteral("window.__credentialMainWorldTransport"),
                   QWebEngineScript::MainWorld)
                   .toBool(),
               false);
      QCOMPARE(run(child, QStringLiteral("window.__credentialMainWorldSent"),
                   QWebEngineScript::MainWorld)
                   .toBool(),
               false);
      QCOMPARE(
          run(child,
              QStringLiteral("window.__credentialApplicationWorldTransport"),
              QWebEngineScript::ApplicationWorld)
              .toBool(),
          false);
      QCOMPARE(run(child,
                   QStringLiteral("window.__credentialApplicationWorldSent"),
                   QWebEngineScript::ApplicationWorld)
                   .toBool(),
               false);
      QCOMPARE(
          run(child, QStringLiteral("window.credentialFakeTransportSends"), 0)
              .toInt(),
          1);
      // This world-42 read is diagnostic only. Qt's API does not make a child
      // runJavaScript call equivalent to a page-originated attacker message.
      qInfo()
          << "child world-42 transport presence"
          << run(child,
                 QStringLiteral(
                     "typeof qt !== 'undefined' && !!qt.webChannelTransport"),
                 CredentialWorld)
                 .toBool();
    }
    QCOMPARE(probe.callCount(), 1);

    // A same-document route change retains the document nonce; a reload
    // creates a different document owner. This is test-world evidence,
    // not a Qt-native document identifier.
    const auto sameDocument = server.pageUrl().toString() + "#spa";
    run(mainFrame, QStringLiteral("location.hash = '#spa'; true"),
        CredentialWorld);
    QTRY_COMPARE_WITH_TIMEOUT(page.url().toString(), sameDocument, 5000);
    QCOMPARE(run(page.mainFrame(),
                 QStringLiteral("window.__credentialDocumentNonce"),
                 CredentialWorld)
                 .toString(),
             nonce);
    QCOMPARE(probe.callCount(), 1);
    QCOMPARE(probe.documentNonce(), nonce);
    QSignalSpy reloaded(&page, &QWebEnginePage::loadFinished);
    page.triggerAction(QWebEnginePage::Reload);
    QVERIFY(reloaded.wait(15000));
    QVERIFY(reloaded.takeFirst().first().toBool());
    const auto reloadedNonce =
        run(page.mainFrame(),
            QStringLiteral("window.__credentialDocumentNonce"), CredentialWorld)
            .toString();
    QVERIFY(reloadedNonce != nonce);
    QTRY_COMPARE_WITH_TIMEOUT(probe.callCount(), 2, 5000);
    QCOMPARE(probe.documentNonce(), reloadedNonce);
  }

  void staleCredentialCompletionAdmission() {
    auto pageHtml = readFixture(QStringLiteral("credential-hostile.html"));
    auto frameHtml = readFixture(QStringLiteral("credential-frame.html"));
    QVERIFY(!pageHtml.isEmpty());
    QVERIFY(!frameHtml.isEmpty());
    FixtureServer server;
    QVERIFY(server.start());
    const auto frameUrl = QStringLiteral("https://localhost:%1/frame")
                              .arg(server.pageUrl().port());
    pageHtml.replace("__SAME_ORIGIN_FRAME_URL__", frameUrl.toUtf8());
    pageHtml.replace("__RELAXED_ORIGIN_FRAME_URL__", frameUrl.toUtf8());
    server.setDocuments(pageHtml, frameHtml);

    QWebEngineProfile profile;
    profile.setHttpCacheType(QWebEngineProfile::NoCache);
    QWebEnginePage page(&profile);
    page.settings()->setAttribute(QWebEngineSettings::BackForwardCacheEnabled,
                                  true);
    QVERIFY(page.settings()->testAttribute(
        QWebEngineSettings::BackForwardCacheEnabled));
    connect(&page, &QWebEnginePage::certificateError, &page,
            [](QWebEngineCertificateError error) {
              // Accept only this disposable, local self-signed test cert.
              error.acceptCertificate();
            });

    QWebEngineScript identity;
    identity.setName(QStringLiteral("credential-test-world42-document-identity"));
    identity.setInjectionPoint(QWebEngineScript::DocumentCreation);
    identity.setWorldId(CredentialWorld);
    identity.setRunsOnSubFrames(true);
    identity.setSourceCode(
        QStringLiteral(
            "window.__credentialDocumentNonce = crypto.randomUUID();"
            "window.__credentialBFCachePersisted = false;"
            "window.addEventListener('pageshow', function(event) {"
            "window.__credentialBFCachePersisted = event.persisted;"
            "});"));
    page.scripts().insert(identity);

    quint64 nativeDocumentGeneration = 0;
    connect(&page, &QWebEnginePage::loadStarted, &page,
            [&nativeDocumentGeneration] { ++nativeDocumentGeneration; });
    QSignalSpy loaded(&page, &QWebEnginePage::loadFinished);
    page.load(server.pageUrl());
    QVERIFY(loaded.wait(15000));
    QVERIFY(loaded.takeFirst().first().toBool());
    QTRY_COMPARE_WITH_TIMEOUT(page.mainFrame().children().size(), 2, 10000);

    const auto identityScript = QStringLiteral(
        "JSON.stringify({nonce: window.__credentialDocumentNonce, "
        "origin: location.origin, secure: window.isSecureContext, "
        "bfcache: window.__credentialBFCachePersisted})");
    const auto trustedFacts = QJsonDocument::fromJson(
        run(page.mainFrame(), identityScript, CredentialWorld).toString().toUtf8());
    QVERIFY(trustedFacts.isObject());
    const auto trustedOrigin = trustedFacts.object().value("origin").toString();
    QVERIFY(trustedOrigin.startsWith(QStringLiteral("https://localhost:")));
    QVERIFY(trustedFacts.object().value("secure").toBool());

    struct PendingCompletion {
      QWebEngineFrame sourceFrame;
      QString documentNonce;
      quint64 nativeGeneration = 0;
    };
    const auto capturePending = [&](QWebEngineFrame sourceFrame) {
      return PendingCompletion{
          sourceFrame,
          run(sourceFrame,
              QStringLiteral("window.__credentialDocumentNonce"),
              CredentialWorld)
              .toString(),
          nativeDocumentGeneration};
    };
    const auto deliverPending = [&](const PendingCompletion &pending,
                                    bool &finished, bool &admitted) {
      finished = false;
      admitted = false;
      const auto currentMainFrame = page.mainFrame();
      if (!currentMainFrame.isValid() || !currentMainFrame.isMainFrame()) {
        finished = true;
        return;
      }
      currentMainFrame.runJavaScript(
          identityScript, CredentialWorld,
          [&, pending](const QVariant &value) {
            const auto facts = QJsonDocument::fromJson(value.toString().toUtf8());
            if (facts.isObject()) {
              const auto object = facts.object();
              admitted = pending.sourceFrame.isValid() &&
                         pending.sourceFrame.isMainFrame() &&
                         pending.nativeGeneration == nativeDocumentGeneration &&
                         !pending.documentNonce.isEmpty() &&
                         object.value("nonce").toString() ==
                             pending.documentNonce &&
                         object.value("origin").toString() == trustedOrigin &&
                         object.value("secure").toBool();
            }
            finished = true;
          });
    };
    const auto awaitCompletion = [&](const PendingCompletion &pending,
                                     bool &admitted) {
      bool finished = false;
      deliverPending(pending, finished, admitted);
      QTRY_VERIFY_WITH_TIMEOUT(finished, 5000);
    };

    const auto firstMainFrame = page.mainFrame();
    const auto firstNonce = run(firstMainFrame,
                                QStringLiteral("window.__credentialDocumentNonce"),
                                CredentialWorld)
                                .toString();
    QVERIFY(!firstNonce.isEmpty());
    const auto firstGeneration = nativeDocumentGeneration;

    // A real asynchronous main-frame challenge admits a reply while its
    // captured document and native generation are still current.
    bool admitted = false;
    awaitCompletion(capturePending(firstMainFrame), admitted);
    QVERIFY(admitted);

    // Reload the exact same URL, then deliver the older pending completion
    // against the replacement main document.
    const auto sameUrlPending = capturePending(page.mainFrame());
    const auto sameUrl = page.url();
    QSignalSpy reloaded(&page, &QWebEnginePage::loadFinished);
    page.triggerAction(QWebEnginePage::Reload);
    QVERIFY(reloaded.wait(15000));
    QVERIFY(reloaded.takeFirst().first().toBool());
    QCOMPARE(page.url(), sameUrl);
    QTRY_COMPARE_WITH_TIMEOUT(page.mainFrame().children().size(), 2, 10000);
    const auto replacementNonce =
        run(page.mainFrame(), QStringLiteral("window.__credentialDocumentNonce"),
            CredentialWorld)
            .toString();
    QVERIFY(replacementNonce != sameUrlPending.documentNonce);
    QVERIFY(nativeDocumentGeneration > sameUrlPending.nativeGeneration);
    awaitCompletion(sameUrlPending, admitted);
    QVERIFY(!admitted);

    // A child-frame source handle is always rejected even while the top
    // document and generation have not changed.
    const auto childFrame = page.mainFrame().children().first();
    QVERIFY(childFrame.isValid());
    QVERIFY(!childFrame.isMainFrame());
    const auto childPending = capturePending(childFrame);
    QVERIFY(!childPending.documentNonce.isEmpty());
    awaitCompletion(childPending, admitted);
    QVERIFY(!admitted);

    // Enable real history traversal and require pageshow.persisted, which
    // distinguishes an actual BFCache restore from an ordinary reload.
    const auto bfcachePending = capturePending(page.mainFrame());
    const auto bfcacheGeneration = nativeDocumentGeneration;
    const auto bfcacheNonce = bfcachePending.documentNonce;
    auto awayUrl = server.pageUrl();
    awayUrl.setPath(QStringLiteral("/away"));
    QSignalSpy left(&page, &QWebEnginePage::loadFinished);
    page.load(awayUrl);
    QVERIFY(left.wait(15000));
    QVERIFY(left.takeFirst().first().toBool());
    QSignalSpy restored(&page, &QWebEnginePage::loadFinished);
    page.triggerAction(QWebEnginePage::Back);
    QVERIFY(restored.wait(15000));
    QVERIFY(restored.takeFirst().first().toBool());
    const auto restoredFacts = QJsonDocument::fromJson(
        run(page.mainFrame(), identityScript, CredentialWorld).toString().toUtf8());
    QVERIFY(restoredFacts.isObject());
    QCOMPARE(restoredFacts.object().value("nonce").toString(), bfcacheNonce);
    QVERIFY(restoredFacts.object().value("bfcache").toBool());
    QVERIFY(nativeDocumentGeneration > bfcacheGeneration);
    awaitCompletion(bfcachePending, admitted);
    QVERIFY(!admitted);

    // A fresh completion after restoration is still admitted. This also
    // confirms BFCache rejection depends on native generation, not nonce alone.
    QCOMPARE(run(page.mainFrame(), QStringLiteral("window.__credentialDocumentNonce"),
                 CredentialWorld)
                 .toString(),
             bfcacheNonce);
    QVERIFY(nativeDocumentGeneration > firstGeneration);
    awaitCompletion(capturePending(page.mainFrame()), admitted);
    QVERIFY(admitted);
  }
};

extern "C" bool cxx_qt_init_crate_ferric_browser_engine_qt();

int main(int argc, char **argv) {
  QtWebEngineQuick::initialize();
  QGuiApplication app(argc, argv);
  cxx_qt_init_crate_ferric_browser_engine_qt();
  CredentialBoundaryTests tests;
  return QTest::qExec(&tests, argc, argv);
}

#include "tst_credential_boundary.moc"
