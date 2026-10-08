#include <QtCore/QDateTime>
#include <QtCore/QElapsedTimer>
#include <QtCore/QThread>
#include <QtGui/QGuiApplication>
#include <QtNetwork/QHostAddress>
#include <QtNetwork/QNetworkCookie>
#include <QtNetwork/QTcpServer>
#include <QtNetwork/QTcpSocket>
#include <QtQml/QQmlApplicationEngine>
#include <QtWebEngineCore/QWebEngineCookieStore>
#include <QtWebEngineQuick/QQuickWebEngineProfile>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>

#include <algorithm>
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

bool hasCookie(const QList<QNetworkCookie> &cookies, const QByteArray &name,
               const QByteArray &value, const QString &path)
{
    for (const auto &cookie : cookies) {
        if (cookie.name() == name && cookie.value() == value && cookie.path() == path)
            return true;
    }
    return false;
}

enum class SnapshotStatus { Unknown, KnownEmpty, KnownNonempty };

SnapshotStatus classifySnapshot(bool completed, bool identityLossless, qsizetype observedCount)
{
    if (!completed || !identityLossless)
        return SnapshotStatus::Unknown;
    return observedCount == 0 ? SnapshotStatus::KnownEmpty : SnapshotStatus::KnownNonempty;
}
}

int main(int argc, char **argv)
{
    QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
    QGuiApplication app(argc, argv);
    QtWebEngineQuick::initialize();
    if (!check(argc == 2, "provide QML probe path"))
        return 2;
    QQmlApplicationEngine engine;
    engine.load(QUrl::fromLocalFile(QString::fromLocal8Bit(argv[1])));
    if (!check(engine.rootObjects().size() == 1, "load cookie probe QML"))
        return 2;
    auto *profile = qobject_cast<QQuickWebEngineProfile *>(
        engine.rootObjects().first()->property("probeProfile").value<QObject *>());
    if (!check(profile && profile->isOffTheRecord(), "synthetic profile is off the record"))
        return 1;
    auto *store = profile->cookieStore();
    if (!check(store != nullptr, "public cookie store exists"))
        return 1;
    QList<QNetworkCookie> added;
    QList<QNetworkCookie> removed;
    QObject::connect(store, &QWebEngineCookieStore::cookieAdded, &app,
                     [&](const QNetworkCookie &cookie) { added.append(cookie); });
    QObject::connect(store, &QWebEngineCookieStore::cookieRemoved, &app,
                     [&](const QNetworkCookie &cookie) { removed.append(cookie); });
    store->loadAllCookies();

    const QUrl origin(QStringLiteral("https://one.example.test/"));
    QNetworkCookie hostOnly("same", "host-synthetic");
    hostOnly.setPath("/");
    QNetworkCookie domain("same", "domain-synthetic");
    domain.setDomain(QStringLiteral(".example.test"));
    domain.setPath("/");
    QNetworkCookie pathVariant("same", "path-synthetic");
    pathVariant.setDomain(QStringLiteral(".example.test"));
    pathVariant.setPath(QStringLiteral("/other"));
    QNetworkCookie persistent("expires", "expiry-synthetic");
    persistent.setPath("/");
    persistent.setExpirationDate(QDateTime::currentDateTimeUtc().addDays(1));
    QNetworkCookie flags("flags", "flags-synthetic");
    flags.setPath("/");
    flags.setSecure(true);
    flags.setHttpOnly(true);
    flags.setSameSitePolicy(QNetworkCookie::SameSite::Strict);
    QNetworkCookie lax("samesite-lax", "lax-synthetic");
    lax.setPath("/");
    lax.setSameSitePolicy(QNetworkCookie::SameSite::Lax);
    QNetworkCookie none("samesite-none", "none-synthetic");
    none.setPath("/");
    none.setSecure(true);
    none.setSameSitePolicy(QNetworkCookie::SameSite::None);
    if (!check(flags.sameSitePolicy() == QNetworkCookie::SameSite::Strict,
               "input cookie carries SameSite Strict")
        || !check(lax.sameSitePolicy() == QNetworkCookie::SameSite::Lax,
                  "input cookie carries SameSite Lax")
        || !check(none.sameSitePolicy() == QNetworkCookie::SameSite::None,
                  "input cookie carries SameSite None"))
        return 1;

    store->setCookie(hostOnly, origin);
    store->setCookie(domain, origin);
    store->setCookie(pathVariant, origin);
    store->setCookie(persistent, origin);
    store->setCookie(flags, origin);
    store->setCookie(lax, origin);
    store->setCookie(none, origin);
    const bool observedAll = waitUntil(app, [&] {
                   return hasCookie(added, "same", "host-synthetic", "/")
                       && hasCookie(added, "same", "domain-synthetic", "/")
                       && hasCookie(added, "same", "path-synthetic", "/other")
                       && hasCookie(added, "expires", "expiry-synthetic", "/")
                       && hasCookie(added, "flags", "flags-synthetic", "/")
                       && hasCookie(added, "samesite-lax", "lax-synthetic", "/")
                       && hasCookie(added, "samesite-none", "none-synthetic", "/");
               }, 10000);
    if (!check(observedAll, "synthetic cookie observation matrix")) {
        std::cerr << "observation flags: count=" << added.size()
                  << " host=" << hasCookie(added, "same", "host-synthetic", "/")
                  << " domain=" << hasCookie(added, "same", "domain-synthetic", "/")
                  << " path=" << hasCookie(added, "same", "path-synthetic", "/other")
                  << " expiry=" << hasCookie(added, "expires", "expiry-synthetic", "/")
                  << " flags=" << hasCookie(added, "flags", "flags-synthetic", "/")
                  << " lax=" << hasCookie(added, "samesite-lax", "lax-synthetic", "/")
                  << " none=" << hasCookie(added, "samesite-none", "none-synthetic", "/") << '\n';
        return 1;
    }
    bool hostObserved = false;
    bool domainObserved = false;
    bool pathObserved = false;
    bool expiryObserved = false;
    bool flagsObserved = false;
    bool sameSiteLost = false;
    bool sessionObserved = false;
    int laxPolicy = -1;
    int nonePolicy = -1;
    for (const auto &cookie : added) {
        if (cookie.name() == "same" && cookie.value() == "host-synthetic") {
            hostObserved = cookie.domain() == u"one.example.test";
            sessionObserved = cookie.isSessionCookie();
        }
        if (cookie.name() == "same" && cookie.value() == "domain-synthetic")
            domainObserved = cookie.domain() == u".example.test";
        if (cookie.name() == "same" && cookie.value() == "path-synthetic")
            pathObserved = cookie.path() == u"/other";
        if (cookie.name() == "expires" && cookie.value() == "expiry-synthetic")
            expiryObserved = !cookie.isSessionCookie() && cookie.expirationDate().isValid();
        if (cookie.name() == "flags" && cookie.value() == "flags-synthetic") {
            flagsObserved = cookie.isSecure() && cookie.isHttpOnly();
            sameSiteLost = cookie.sameSitePolicy() == QNetworkCookie::SameSite::Default;
        }
        if (cookie.name() == "samesite-lax" && cookie.value() == "lax-synthetic")
            laxPolicy = static_cast<int>(cookie.sameSitePolicy());
        if (cookie.name() == "samesite-none" && cookie.value() == "none-synthetic")
            nonePolicy = static_cast<int>(cookie.sameSitePolicy());
    }
    if (!check(hostObserved, "host-only fixture observed at origin host")
        || !check(domainObserved, "domain fixture observed with dot scope")
        || !check(pathObserved, "duplicate name with separate path observed")
        || !check(expiryObserved, "persistent expiry exposed")
        || !check(sessionObserved, "session marker exposed")
        || !check(flagsObserved, "Secure and HttpOnly flags exposed")
        || !check(sameSiteLost, "SameSite Strict input lost in native callback")
        || !check(laxPolicy >= 0 && nonePolicy >= 0,
                  "SameSite Lax and None observations expose a policy enum"))
        return 1;

    // Observe actual outgoing requests, not just a cookieAdded domain string.
    // The server records only a synthetic marker's presence, never headers.
    QTcpServer scopeServer;
    if (!check(scopeServer.listen(QHostAddress::LocalHost, 0),
               "listen on disposable loopback scope port"))
        return 1;
    struct ScopeRequest {
        QByteArray path;
        bool includesHostOnly = false;
        bool includesSubdomainControl = false;
    };
    QList<ScopeRequest> scopeRequests;
    QObject::connect(&scopeServer, &QTcpServer::newConnection, &app, [&] {
        while (auto *socket = scopeServer.nextPendingConnection()) {
            QObject::connect(socket, &QTcpSocket::readyRead, &app,
                             [&, socket, buffer = QByteArray{}, handled = false]() mutable {
                                 if (handled)
                                     return;
                                 buffer.append(socket->readAll());
                                 const auto headersEnd = buffer.indexOf("\r\n\r\n");
                                 if (headersEnd < 0)
                                     return;
                                 handled = true;
                                 const auto requestLineEnd = buffer.indexOf("\r\n");
                                 const auto requestLine = buffer.left(requestLineEnd);
                                 const auto headers = buffer.left(headersEnd).toLower();
                                 scopeRequests.append({
                                     requestLine,
                                     headers.contains("vb_host_only=synthetic-scope"),
                                     headers.contains("vb_sub_only=synthetic-subdomain")});
                                 static constexpr auto response =
                                     "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n"
                                     "Content-Length: 35\r\nConnection: close\r\n\r\n"
                                     "<!doctype html><title>scope</title>";
                                 socket->write(response);
                                 socket->disconnectFromHost();
                             });
            QObject::connect(socket, &QTcpSocket::disconnected, socket, &QObject::deleteLater);
        }
    });
    QUrl scopeOrigin;
    scopeOrigin.setScheme(QStringLiteral("http"));
    scopeOrigin.setHost(QStringLiteral("localhost"));
    scopeOrigin.setPort(scopeServer.serverPort());
    auto subdomainOrigin = scopeOrigin;
    subdomainOrigin.setHost(QStringLiteral("sub.localhost"));
    QNetworkCookie scoped("vb_host_only", "synthetic-scope");
    scoped.setPath(QStringLiteral("/"));
    QNetworkCookie subdomainControl("vb_sub_only", "synthetic-subdomain");
    subdomainControl.setPath(QStringLiteral("/"));
    // An empty domain plus explicit origin is the documented host-only
    // conversion; setting a non-dot domain would be broadened by Qt.
    store->setCookie(scoped, scopeOrigin);
    store->setCookie(subdomainControl, subdomainOrigin);
    if (!check(waitUntil(app, [&] {
                   return hasCookie(added, "vb_host_only", "synthetic-scope", "/")
                       && hasCookie(added, "vb_sub_only", "synthetic-subdomain", "/");
               }, 10000), "both host-only conversions were accepted by the store"))
        return 1;
    auto hostUrl = scopeOrigin;
    hostUrl.setPath(QStringLiteral("/scope-host"));
    engine.rootObjects().first()->setProperty("probeUrl", hostUrl);
    if (!check(waitUntil(app, [&] {
                   return std::any_of(scopeRequests.cbegin(), scopeRequests.cend(),
                                      [](const ScopeRequest &request) {
                                          return request.path.contains("/scope-host");
                                      });
               }, 10000), "origin host request reached local server"))
        return 1;
    const auto hostRequest = std::find_if(scopeRequests.cbegin(), scopeRequests.cend(),
                                          [](const ScopeRequest &request) {
                                              return request.path.contains("/scope-host");
                                          });
    if (!check(hostRequest->includesHostOnly, "origin host sends host-only cookie")
        || !check(!hostRequest->includesSubdomainControl,
                  "origin host excludes subdomain-only control cookie"))
        return 1;
    auto subdomainUrl = subdomainOrigin;
    subdomainUrl.setPath(QStringLiteral("/scope-subdomain"));
    engine.rootObjects().first()->setProperty("probeUrl", subdomainUrl);
    if (!check(waitUntil(app, [&] {
                   return std::any_of(scopeRequests.cbegin(), scopeRequests.cend(),
                                      [](const ScopeRequest &request) {
                                          return request.path.contains("/scope-subdomain");
                                      });
               }, 10000), "subdomain request reached local server"))
        return 1;
    const auto subdomainRequest = std::find_if(scopeRequests.cbegin(), scopeRequests.cend(),
                                               [](const ScopeRequest &request) {
                                                   return request.path.contains("/scope-subdomain");
                                               });
    if (!check(!subdomainRequest->includesHostOnly,
               "host-only cookie is not sent to subdomain")
        || !check(subdomainRequest->includesSubdomainControl,
                  "subdomain request sends its own host-only control cookie"))
        return 1;

    // The installed public QNetworkCookie shape does not carry a partition
    // key. Parsing the attribute demonstrates that the representation loses
    // it before a native observer could key a complete Chromium snapshot.
    const auto partitioned = QNetworkCookie::parseCookies(QByteArrayView(
        "part=synthetic; Path=/; Secure; SameSite=None; Partitioned"));
    const auto unpartitioned = QNetworkCookie::parseCookies(QByteArrayView(
        "part=synthetic; Path=/; Secure; SameSite=None"));
    if (!check(partitioned.size() == 1 && unpartitioned.size() == 1,
               "parse synthetic partitioned and plain headers")
        || !check(partitioned.first().hasSameIdentifier(unpartitioned.first()),
                  "public identifier collides without partition dimension")
        || !check(!partitioned.first().toRawForm().toLower().contains("partitioned"),
                  "public serialized representation omits partition attribute"))
        return 1;
    if (!check(classifySnapshot(false, true, 0) == SnapshotStatus::Unknown,
               "quiet unknown enumeration is not an empty jar")
        || !check(classifySnapshot(false, false, added.size()) == SnapshotStatus::Unknown,
                  "observations plus identity collision are not a strict snapshot"))
        return 1;

    const auto beforeRescan = added.size();
    store->loadAllCookies();
    waitUntil(app, [] { return false; }, 250);
    const auto observedRescanEvents = added.size() - beforeRescan;
    // There is no completion signal or snapshot token in the public store.
    // The count is diagnostic only: zero after a quiet interval is unknown,
    // not an empty cookie jar or a completed enumeration.
    if (!check(!added.isEmpty() && removed.isEmpty(), "only synthetic additions observed"))
        return 1;
    std::cout << "PASS: synthetic cookie fields observed; SameSite callback policy Lax="
              << laxPolicy << ", None=" << nonePolicy << "; partition identity absent; "
              << "rescan event count=" << observedRescanEvents
              << " (not a completeness marker)\n";
    return 0;
}
