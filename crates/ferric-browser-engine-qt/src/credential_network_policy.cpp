#include "credential_network_policy.h"
#include <QCoreApplication>
#include <QNetworkInformation>
#include <QNetworkProxy>
#include <QNetworkProxyFactory>
#include <QNetworkProxyQuery>
#include <QRegularExpression>
#include <QThread>

namespace {
std::optional<QString> environmentFlags()
{
    return qEnvironmentVariableIsSet("QTWEBENGINE_CHROMIUM_FLAGS")
        ? std::make_optional(qEnvironmentVariable("QTWEBENGINE_CHROMIUM_FLAGS")) : std::nullopt;
}
bool proxyEnvironment()
{
    for (const auto *name : {"http_proxy", "https_proxy", "all_proxy", "HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"})
        if (!qgetenv(name).isEmpty()) return true;
    return false;
}
struct Startup {
    std::optional<QString> environment;
    std::optional<QStringList> arguments;
    bool proxyEnvironment = false;
    bool captured = false;
};
Startup startup;
}

std::optional<QStringList> FerricCredentialNetworkPolicy::chromiumArguments(
    const QStringList &arguments, const std::optional<QString> &environment)
{
    if (arguments.isEmpty()) return std::nullopt;
    if (!environment) {
        for (qsizetype index = 1; index < arguments.size(); ++index)
            if (arguments[index].compare(QStringLiteral("--webEngineArgs"), Qt::CaseInsensitive) == 0)
                return arguments.mid(index + 1);
        return QStringList{};
    }
    // Match Qt 6.11.2's parseEnvCommandLine, including its double-quote-only
    // syntax. Reject malformed/unbounded input rather than guessing. An empty
    // but set environment overrides --webEngineArgs just as Qt does.
    // https://github.com/qt/qtwebengine/blob/v6.11.2/src/core/web_engine_context.cpp
    if (environment->size() > 65536 || environment->contains(QChar::Null)) return std::nullopt;
    QStringList result;
    QString argument;
    bool quoted = false;
    for (const QChar character : *environment) {
        if (character == QLatin1Char('"')) quoted = !quoted;
        else if (character == QLatin1Char(' ') && !quoted) {
            if (!argument.isEmpty()) { result.append(argument); argument.clear(); }
        } else argument += character;
    }
    if (quoted) return std::nullopt;
    if (!argument.isEmpty()) result.append(argument);
    return result;
}

bool FerricCredentialNetworkPolicy::argumentsAllowDirect(const QStringList &arguments, const QUrl &requestingOrigin)
{
    if (arguments.size() > 256) return false;
    const auto host = QString::fromLatin1(QUrl::toAce(requestingOrigin.host())).toLower();
    static const QRegularExpression dnsName(QStringLiteral("^[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$"));
    bool mapped = false;
    for (const auto &argument : arguments) {
        const auto flag = argument.toLower();
        if (flag.startsWith(QLatin1String("--host-resolver-rules="))) {
            // A single exact RP-to-loopback mapping does not affect either
            // assigned relay. Never admit wildcard/EXCLUDE/additional rules,
            // port patterns or a requesting origin that is itself a relay.
            if (mapped || requestingOrigin.scheme() != QLatin1String("https")
                || !dnsName.match(host).hasMatch() || host.size() > 253
                || host == QLatin1String("cable.ua5v.com") || host == QLatin1String("cable.auth.com")
                || argument.mid(argument.indexOf(QLatin1Char('=')) + 1)
                    != QStringLiteral("MAP %1 127.0.0.1").arg(host)) return false;
            mapped = true;
        } else if (flag.contains(QLatin1String("proxy")) || flag.contains(QLatin1String("offline"))
            || flag.contains(QLatin1String("resolver")) || flag.startsWith(QLatin1String("--host-rules"))
            || flag.startsWith(QLatin1String("--dns-")) || flag.contains(QLatin1String("embedded-switches"))) return false;
    }
    return true;
}

bool FerricCredentialNetworkPolicy::capture(bool explicitDirect)
{
    if (!QCoreApplication::instance()
        || QThread::currentThread() != QCoreApplication::instance()->thread()) return false;
    if (startup.captured) return !explicitDirect;
    const auto environment = environmentFlags();
    const auto arguments = chromiumArguments(QCoreApplication::arguments(), environment);
    const bool proxy = proxyEnvironment();
    if (explicitDirect) {
        if (proxy || !arguments) return false;
        for (const auto &argument : *arguments)
            if (argument.contains(QStringLiteral("proxy"), Qt::CaseInsensitive)) return false;
        // Qt forwards applicationProxy to Chromium when system discovery is
        // disabled. The fresh-process launch option therefore declares the
        // browser's own route as well as the helper's permitted route.
        QNetworkProxy::setApplicationProxy(QNetworkProxy(QNetworkProxy::NoProxy));
    }
    startup.environment = environment;
    startup.arguments = arguments;
    startup.proxyEnvironment = proxy;
    startup.captured = true;
    return true;
}

bool FerricCredentialNetworkPolicy::direct(const QUrl &requestingOrigin)
{
    // This is an admission check, never a proxy configuration operation. With
    // system configuration enabled Chromium resolves its own PAC/settings;
    // public Qt queries cannot prove that effective route, so decline it.
    if (!startup.captured || !startup.arguments || startup.environment != environmentFlags()
        || startup.proxyEnvironment || proxyEnvironment()
        || !argumentsAllowDirect(*startup.arguments, requestingOrigin)
        || QNetworkProxyFactory::usesSystemConfiguration()
        || QNetworkProxy::applicationProxy().type() != QNetworkProxy::NoProxy) return false;
    for (const auto *relay : {"https://cable.ua5v.com/", "https://cable.auth.com/"}) {
        const auto proxies = QNetworkProxyFactory::proxyForQuery(QNetworkProxyQuery(QUrl(QString::fromLatin1(relay))));
        if (proxies.size() != 1 || proxies.front().type() != QNetworkProxy::NoProxy) return false;
    }
    if (const auto *information = QNetworkInformation::instance()) {
        using Reachability = QNetworkInformation::Reachability;
        if (information->reachability() == Reachability::Disconnected
            || information->reachability() == Reachability::Local
            || information->reachability() == Reachability::Site) return false;
    }
    return true;
}
