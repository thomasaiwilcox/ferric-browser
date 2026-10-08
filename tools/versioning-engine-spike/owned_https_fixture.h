#pragma once

#include <QApplication>
#include <QCryptographicHash>
#include <QEventLoop>
#include <QFile>
#include <QFileInfo>
#include <QHash>
#include <QHostAddress>
#include <QSslCertificate>
#include <QSslConfiguration>
#include <QSslKey>
#include <QSslServer>
#include <QSslSocket>
#include <QTimer>
#include <QWebEngineCertificateError>
#include <QWebEngineCookieStore>
#include <QWebEnginePage>
#include <QWebEngineProfile>
#include <QtWebEngineCore/qtwebenginecoreglobal.h>

#include <functional>
#include <iostream>
#include <memory>
#include <optional>

namespace {
bool check(bool ok, const char *label)
{
    std::cout << (ok ? "PASS " : "FAIL ") << label << '\n';
    return ok;
}

bool waitFor(const std::function<bool()> &ready)
{
    if (ready())
        return true;
    QEventLoop loop;
    QTimer poll;
    QTimer deadline;
    deadline.setSingleShot(true);
    QObject::connect(&poll, &QTimer::timeout, &loop, [&] {
        if (ready())
            loop.quit();
    });
    QObject::connect(&deadline, &QTimer::timeout, &loop, &QEventLoop::quit);
    poll.start(5);
    deadline.start(25000);
    loop.exec();
    return ready();
}

[[maybe_unused]] bool navigate(QWebEnginePage &page, const QUrl &url, bool fromScript = false)
{
    bool finished = false;
    bool loaded = false;
    const auto connection = QObject::connect(&page, &QWebEnginePage::loadFinished,
                                            [&](bool ok) { loaded = ok; finished = true; });
    if (fromScript) {
        // URL consists only of the owned fixture's fixed hostname/path and port.
        page.runJavaScript(QStringLiteral("location.href = '%1'").arg(url.toString()));
    } else {
        page.load(url);
    }
    const bool ok = waitFor([&] { return finished; }) && loaded;
    QObject::disconnect(connection);
    return ok;
}

[[maybe_unused]] bool scriptBoolean(QWebEnginePage &page, const QString &script)
{
    auto result = std::make_shared<std::optional<bool>>();
    page.runJavaScript(script, [result](const QVariant &value) {
        *result = value.isValid() && value.toBool();
    });
    return waitFor([&] { return result->has_value(); }) && **result;
}

// Only boolean observations survive request parsing. No Cookie/Set-Cookie
// headers, certificate keys or cookie values are printed or persisted.
struct Seen {
    bool host = false, domain = false, strict = false, lax = false, none = false;
    bool httpOnly = false, scoped = false, script = false, hostPrefix = false;
    bool securePrefix = false, plain = false, first = false, ancestor = false;
    bool topA = false, topB = false;
    bool edgeValid = false, edgeFar = false, edgeExpired = false;
    bool edgeHostBad = false, edgeSecureBad = false, edgeNoneBad = false;
    bool edgePartitionBad = false;
};

class Fixture final : public QSslServer {
    QHash<QString, Seen> m_seen;
    bool m_unexpectedCertificate = false;
    int m_pinnedCertificates = 0;
    QSslCertificate m_certificate;

    static bool allowedHost(const QString &host)
    {
        return host == u"cookie.localhost" || host == u"sub.cookie.localhost"
                || host == u"top-a.localhost" || host == u"top-b.localhost";
    }

    void respond(QSslSocket *socket, const QByteArray &request)
    {
        const auto lines = request.split('\n');
        const auto first = lines.value(0).trimmed().split(' ');
        QByteArray hostHeader;
        QByteArray cookies;
        for (const auto &line : lines) {
            const auto colon = line.indexOf(':');
            if (colon < 0)
                continue;
            const auto name = line.left(colon).trimmed().toLower();
            if (name == "host")
                hostHeader = line.mid(colon + 1).trimmed();
            if (name == "cookie")
                cookies = line.mid(colon + 1).trimmed();
        }
        const auto host = QString::fromLatin1(hostHeader).section(':', 0, 0);
        if (first.size() != 3 || first[0] != "GET" || !allowedHost(host)
                || hostHeader != host.toLatin1() + ':' + QByteArray::number(serverPort())) {
            socket->abort();
            return;
        }
        const auto path = QUrl::fromEncoded(first[1]).path();
        QList<QByteArray> pairs;
        for (const auto &part : cookies.split(';'))
            pairs.append(part.trimmed());
        Seen seen;
        seen.host = pairs.contains("host=synthetic");
        seen.domain = pairs.contains("domain=synthetic");
        seen.strict = pairs.contains("strict=synthetic");
        seen.lax = pairs.contains("lax=synthetic");
        seen.none = pairs.contains("none=synthetic");
        seen.httpOnly = pairs.contains("http_only=synthetic");
        seen.scoped = pairs.contains("scoped=synthetic");
        seen.script = pairs.contains("script_cookie=synthetic");
        seen.hostPrefix = pairs.contains("__Host-proof=synthetic");
        seen.securePrefix = pairs.contains("__Secure-proof=synthetic");
        seen.plain = pairs.contains("collision=plain");
        seen.first = pairs.contains("collision=first");
        seen.ancestor = pairs.contains("collision=ancestor");
        seen.topA = pairs.contains("collision=top-a");
        seen.topB = pairs.contains("collision=top-b");
        seen.edgeValid = pairs.contains("edge_valid=synthetic");
        seen.edgeFar = pairs.contains("edge_far=synthetic");
        seen.edgeExpired = pairs.contains("edge_expired=synthetic");
        seen.edgeHostBad = pairs.contains("__Host-bad=synthetic");
        seen.edgeSecureBad = pairs.contains("__Secure-bad=synthetic");
        seen.edgeNoneBad = pairs.contains("edge_none_bad=synthetic");
        seen.edgePartitionBad = pairs.contains("edge_partition_bad=synthetic");
        if ((path.contains(u"check") && host == u"cookie.localhost")
                || (path == u"/check" && host == u"sub.cookie.localhost")) {
            if (m_seen.size() >= 64 && !m_seen.contains(host + path)) {
                socket->abort();
                return;
            }
            m_seen.insert(host + path, seen);
        }

        QList<QByteArray> setters;
        QByteArray body = "<!doctype html><meta charset=utf-8><title>Owned fixture</title>";
        if (path == u"/rtc/overflow") {
            body += QByteArray("<iframe src=about:blank></iframe>").repeated(128);
        } else if (path == u"/rtc/peer") {
            body += "<script>window.peer = new RTCPeerConnection({iceServers: []});</script>";
        } else if (path == u"/rtc/same-peer" || path == u"/rtc/cross-peer"
                   || path == u"/rtc/cross-empty") {
            const QString childHost = path == u"/rtc/same-peer" ? host : QStringLiteral("top-a.localhost");
            const QString childPath = path == u"/rtc/cross-empty" ? QStringLiteral("/rtc/empty")
                                                                  : QStringLiteral("/rtc/peer");
            body += "<iframe src=\"" + url(childHost, childPath).toEncoded() + "\"></iframe>";
        } else if (path == u"/rtc/cross-capture" || path == u"/rtc/same-capture") {
            const QString childHost = path == u"/rtc/same-capture" ? QStringLiteral("cookie.localhost")
                                                                     : QStringLiteral("top-a.localhost");
            body += "<iframe allow=\"camera *; microphone *\" src=\"" + url(childHost,
                                                                   QStringLiteral("/rtc/empty")).toEncoded()
                    + "\"></iframe>";
        } else if (host == u"cookie.localhost" && path == u"/seed") {
            setters = {
                "host=synthetic; Secure; SameSite=Lax; Path=/",
                "domain=synthetic; Domain=cookie.localhost; Secure; SameSite=Lax; Path=/",
                "strict=synthetic; Secure; SameSite=Strict; Path=/",
                "lax=synthetic; Secure; SameSite=Lax; Path=/",
                "none=synthetic; Secure; SameSite=None; Path=/",
                "unspecified=synthetic; Secure; Path=/",
                "http_only=synthetic; Secure; HttpOnly; SameSite=Lax; Path=/",
                "scoped=synthetic; Secure; SameSite=Lax; Path=/scoped",
                "persistent=synthetic; Secure; Max-Age=3600; SameSite=Lax; Path=/",
                "priority_low=synthetic; Secure; Priority=Low; SameSite=Lax; Path=/",
                "priority_high=synthetic; Secure; Priority=High; SameSite=Lax; Path=/",
                "__Host-proof=synthetic; Secure; SameSite=Lax; Path=/",
                "__Secure-proof=synthetic; Secure; SameSite=Lax; Path=/"
            };
            body += "<script>document.cookie='script_cookie=synthetic; Secure; SameSite=Lax; Path=/'</script>";
        } else if (host == u"cookie.localhost" && path == u"/partition/seed-first") {
            setters = {"collision=plain; Secure; SameSite=None; Path=/partition",
                       "collision=first; Secure; SameSite=None; Path=/partition; Partitioned"};
        } else if (host == u"cookie.localhost" && path == u"/edge/seed") {
            setters = {
                "edge_valid=synthetic; Secure; SameSite=Lax; Path=/edge; Max-Age=600",
                "edge_far=synthetic; Secure; SameSite=Lax; Path=/edge; Expires=Fri, 01 Jan 2099 00:00:00 GMT",
                "edge_expired=synthetic; Secure; SameSite=Lax; Path=/edge; Max-Age=0",
                "__Host-bad=synthetic; Secure; Domain=cookie.localhost; Path=/",
                "__Secure-bad=synthetic; Path=/edge",
                "edge_none_bad=synthetic; SameSite=None; Path=/edge",
                "edge_partition_bad=synthetic; SameSite=None; Partitioned; Path=/edge"
            };
        } else if (host == u"cookie.localhost"
                   && (path == u"/partition/set-a" || path == u"/partition/set-b")) {
            setters = {QByteArray("collision=") + (path.endsWith(u"a") ? "top-a" : "top-b")
                       + "; Secure; SameSite=None; Path=/partition; Partitioned"};
        } else if (host == u"cookie.localhost" && path == u"/partition/set-ancestor") {
            setters = {"collision=ancestor; Secure; SameSite=None; Path=/partition; Partitioned"};
        } else if (host == u"cookie.localhost"
                   && (path == u"/ancestor-seed" || path == u"/ancestor-check")) {
            const QString middle = path == u"/ancestor-seed" ? QStringLiteral("/ancestor-bridge-seed")
                                                            : QStringLiteral("/ancestor-bridge-check");
            body += "<iframe src=\"" + url(QStringLiteral("top-a.localhost"), middle).toEncoded()
                    + "\"></iframe>";
        } else if (host == u"top-a.localhost"
                   && (path == u"/ancestor-bridge-seed" || path == u"/ancestor-bridge-check")) {
            const QString child = path == u"/ancestor-bridge-seed" ? QStringLiteral("/partition/set-ancestor")
                                                                 : QStringLiteral("/partition/check-ancestor");
            body += "<iframe src=\"" + url(QStringLiteral("cookie.localhost"), child).toEncoded()
                    + "\"></iframe>";
        } else if ((host == u"top-a.localhost" || host == u"top-b.localhost")
                   && (path == u"/seed-frame" || path == u"/check-frame")) {
            const QString suffix = host == u"top-a.localhost" ? QStringLiteral("a") : QStringLiteral("b");
            const QString childPath = path == u"/seed-frame" ? QStringLiteral("/partition/set-")
                                                             : QStringLiteral("/partition/check-");
            body += "<iframe src=\"" + url(QStringLiteral("cookie.localhost"), childPath + suffix).toEncoded()
                    + "\"></iframe>";
        }
        QByteArray response = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n"
                              "Cache-Control: no-store\r\nConnection: close\r\n";
        for (const auto &setter : setters)
            response += "Set-Cookie: " + setter + "\r\n";
        response += "Content-Length: " + QByteArray::number(body.size()) + "\r\n\r\n" + body;
        socket->write(response);
        socket->disconnectFromHost();
    }

public:
    bool start(const QString &certificatePath, const QString &keyPath)
    {
        QFile certificate(certificatePath), key(keyPath);
        if (!certificate.open(QIODevice::ReadOnly) || !key.open(QIODevice::ReadOnly))
            return false;
        m_certificate = QSslCertificate(certificate.readAll(), QSsl::Pem);
        const QSslKey privateKey(key.readAll(), QSsl::Rsa, QSsl::Pem, QSsl::PrivateKey);
        if (m_certificate.isNull() || privateKey.isNull())
            return false;
        auto configuration = QSslConfiguration::defaultConfiguration();
        configuration.setLocalCertificate(m_certificate);
        configuration.setPrivateKey(privateKey);
        configuration.setPeerVerifyMode(QSslSocket::VerifyNone); // no client authentication for this fixture
        setSslConfiguration(configuration);
        setHandshakeTimeout(5000);
        setMaxPendingConnections(16);
        QObject::connect(this, &QSslServer::startedEncryptionHandshake, this,
                         [this](QSslSocket *socket) {
            if (findChildren<QSslSocket *>(QString(), Qt::FindDirectChildrenOnly).size() > 32)
                socket->abort();
        });
        QObject::connect(this, &QSslServer::pendingConnectionAvailable, this, [this] {
            while (hasPendingConnections()) {
                auto *socket = qobject_cast<QSslSocket *>(nextPendingConnection());
                if (!socket)
                    continue;
                socket->setReadBufferSize(32768);
                QObject::connect(socket, &QSslSocket::disconnected, socket, &QObject::deleteLater);
                QTimer::singleShot(10000, socket, [socket] { socket->abort(); });
                auto buffer = std::make_shared<QByteArray>();
                auto served = std::make_shared<bool>(false);
                const auto receive = [this, socket, buffer, served] {
                    if (*served)
                        return;
                    buffer->append(socket->readAll());
                    if (buffer->size() > 16384) {
                        *served = true;
                        socket->abort();
                    } else if (buffer->contains("\r\n\r\n")) {
                        *served = true;
                        respond(socket, *buffer);
                        buffer->clear();
                    }
                };
                QObject::connect(socket, &QSslSocket::readyRead, socket, receive);
                receive();
            }
        });
        return listen(QHostAddress::LocalHost, 0);
    }

    QUrl url(const QString &host, const QString &path) const
    {
        QUrl result;
        result.setScheme(QStringLiteral("https"));
        result.setHost(host);
        result.setPort(serverPort());
        result.setPath(path);
        return result;
    }

    void pin(QWebEnginePage &page)
    {
        QObject::connect(&page, &QWebEnginePage::certificateError, &page,
                         [this](QWebEngineCertificateError error) {
            const auto chain = error.certificateChain();
            const bool pinned = error.isOverridable()
                    && error.type() == QWebEngineCertificateError::CertificateAuthorityInvalid
                    && error.url().scheme() == u"https" && allowedHost(error.url().host())
                    && error.url().port() == serverPort() && !chain.isEmpty()
                    && chain.first().digest(QCryptographicHash::Sha256)
                            == m_certificate.digest(QCryptographicHash::Sha256);
            if (pinned) {
                ++m_pinnedCertificates;
                error.acceptCertificate();
            } else {
                m_unexpectedCertificate = true;
                error.rejectCertificate();
            }
        });
    }

    std::optional<Seen> take(const QString &host, const QString &path)
    {
        const auto key = host + path;
        if (!m_seen.contains(key))
            return std::nullopt;
        return m_seen.take(key);
    }

    bool certificateScopeHeld() const { return m_pinnedCertificates > 0 && !m_unexpectedCertificate; }
};

} // namespace
