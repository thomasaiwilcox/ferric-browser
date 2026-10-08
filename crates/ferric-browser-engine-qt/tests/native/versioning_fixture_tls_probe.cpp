#include <QtCore/QElapsedTimer>
#include <QtCore/QFile>
#include <QtCore/QCryptographicHash>
#include <QtCore/QThread>
#include <QtGui/QGuiApplication>
#include <QtNetwork/QNetworkCookie>
#include <QtNetwork/QSslCertificate>
#include <QtWebEngineCore/QWebEngineCertificateError>
#include <QtWebEngineCore/QWebEngineCookieStore>
#include <QtWebEngineCore/QWebEnginePage>
#include <QtWebEngineCore/QWebEngineProfile>

#include <functional>
#include <iostream>

namespace {
bool check(bool condition, const char *message)
{
    if (!condition)
        std::cerr << "FAIL: " << message << '\n';
    return condition;
}

bool waitUntil(QGuiApplication &app, const std::function<bool()> &predicate, int timeoutMs)
{
    QElapsedTimer elapsed;
    elapsed.start();
    while (!predicate() && elapsed.elapsed() < timeoutMs) {
        app.processEvents();
        QThread::msleep(10);
    }
    return predicate();
}
}

int main(int argc, char **argv)
{
    QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
    QGuiApplication app(argc, argv);
    if (!check(argc == 3, "provide fixture URL and certificate path"))
        return 2;

    const QUrl origin(QString::fromLocal8Bit(argv[1]));
    if (!check(origin.scheme() == u"https" && origin.host() == u"127.0.0.1"
                   && origin.port() > 0 && origin.path() == u"/versioning/cookies/secure",
               "fixture target is the expected loopback HTTPS endpoint"))
        return 2;
    QFile certificateFile(QString::fromLocal8Bit(argv[2]));
    if (!check(certificateFile.open(QIODevice::ReadOnly), "read disposable fixture certificate"))
        return 2;
    const QSslCertificate expected(certificateFile.readAll(), QSsl::Pem);
    if (!check(!expected.isNull(), "parse disposable fixture certificate"))
        return 2;
    const QByteArray expectedDigest = expected.digest(QCryptographicHash::Sha256);

    QWebEngineProfile profile;
    if (!check(profile.isOffTheRecord(), "probe profile is off the record"))
        return 2;
    QWebEnginePage page(&profile);
    int certificateErrors = 0;
    bool rejectedUnexpectedCertificate = false;
    int lastCertificateError = 0;
    int certificateChainLength = 0;
    QObject::connect(&page, &QWebEnginePage::certificateError, &app,
                     [&](const QWebEngineCertificateError &error) {
                         ++certificateErrors;
                         const auto chain = error.certificateChain();
                         lastCertificateError = error.type();
                         certificateChainLength = chain.size();
                         const bool pinned = error.isOverridable()
                             && error.type() == QWebEngineCertificateError::CertificateAuthorityInvalid
                             && error.url().scheme() == u"https"
                             && error.url().host() == origin.host()
                             && error.url().port() == origin.port()
                             && error.url().path().startsWith(u"/versioning/")
                             && !chain.isEmpty()
                             && chain.first().digest(QCryptographicHash::Sha256) == expectedDigest;
                         auto decision = error;
                         if (pinned)
                             decision.acceptCertificate();
                         else {
                             rejectedUnexpectedCertificate = true;
                             decision.rejectCertificate();
                         }
                     });

    bool hostCookieObserved = false;
    bool pathCookieObserved = false;
    QObject::connect(profile.cookieStore(), &QWebEngineCookieStore::cookieAdded, &app,
                     [&](const QNetworkCookie &cookie) {
                         if (cookie.name() == "vb_host")
                             hostCookieObserved = cookie.isSecure() && cookie.isHttpOnly()
                                 && cookie.path() == u"/versioning";
                         if (cookie.name() == "vb_path")
                             pathCookieObserved = cookie.isSecure()
                                 && cookie.path() == u"/versioning/cookies";
                     });

    bool loadFinished = false;
    bool loadSucceeded = false;
    QObject::connect(&page, &QWebEnginePage::loadFinished, &app, [&](bool ok) {
        loadFinished = true;
        loadSucceeded = ok;
    });
    page.load(origin);
    if (!waitUntil(app, [&] { return loadFinished; }, 15000)) {
        std::cerr << "diagnostic: certificate_errors=" << certificateErrors
                  << " last_type=" << lastCertificateError
                  << " chain_length=" << certificateChainLength
                  << " rejected=" << rejectedUnexpectedCertificate << '\n';
        return 1;
    }
    if (!loadSucceeded) {
        std::cerr << "diagnostic: certificate_errors=" << certificateErrors
                  << " last_type=" << lastCertificateError
                  << " chain_length=" << certificateChainLength
                  << " rejected=" << rejectedUnexpectedCertificate << '\n';
    }
    if (!check(loadSucceeded, "pinned fixture certificate permits navigation")
        || !check(certificateErrors == 1 && !rejectedUnexpectedCertificate,
                  "only the pinned loopback certificate is accepted")
        || !check(waitUntil(app, [&] { return hostCookieObserved && pathCookieObserved; }, 5000),
                  "Secure and HttpOnly cookie attributes reach the disposable profile"))
        return 1;

    // Partitioned/SameSite identity is deliberately not qualified here: the
    // public callback cannot expose those dimensions losslessly (VB-005).
    std::cout << "PASS: disposable off-record profile loaded pinned loopback HTTPS; "
                 "Secure and HttpOnly cookie observations reached the public store\n";
    std::cout << "UNQUALIFIED: SameSite and Partitioned identity cannot be proven "
                 "through the public cookie callback\n";
    return 0;
}
