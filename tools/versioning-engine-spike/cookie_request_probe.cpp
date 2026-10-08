#include "owned_https_fixture.h"
#include <QDateTime>
#include <QNetworkCookie>

namespace {

bool requestChecks(Fixture &fixture, QWebEnginePage &page)
{
    const QString host = QStringLiteral("cookie.localhost");
    if (!check(navigate(page, fixture.url(host, QStringLiteral("/check"))), "https-first-party-request"))
        return false;
    const auto ordinary = fixture.take(host, QStringLiteral("/check"));
    bool ok = check(ordinary && ordinary->host && ordinary->domain && ordinary->strict
                            && ordinary->lax && ordinary->none && ordinary->httpOnly
                            && ordinary->script && ordinary->hostPrefix && ordinary->securePrefix
                            && !ordinary->scoped, "https-ordinary-flags-prefixes-script-and-path-exclusion");
    ok &= check(scriptBoolean(page, QStringLiteral(
                        "document.cookie.includes('script_cookie=synthetic') && !document.cookie.includes('http_only=')")),
                "https-http-only-hidden-from-script");
    if (!navigate(page, fixture.url(host, QStringLiteral("/scoped/check"))))
        return check(false, "https-scoped-request");
    const auto scoped = fixture.take(host, QStringLiteral("/scoped/check"));
    ok &= check(scoped && scoped->scoped, "https-path-cookie-included-at-scope");
    if (!navigate(page, fixture.url(QStringLiteral("sub.cookie.localhost"), QStringLiteral("/check"))))
        return check(false, "https-subdomain-request");
    const auto subdomain = fixture.take(QStringLiteral("sub.cookie.localhost"), QStringLiteral("/check"));
    ok &= check(subdomain && !subdomain->host && subdomain->domain,
                "https-host-only-not-broadened-domain-cookie-reaches-subdomain");
    if (!navigate(page, fixture.url(QStringLiteral("top-a.localhost"), QStringLiteral("/blank")))
            || !navigate(page, fixture.url(host, QStringLiteral("/check-cross")), true))
        return check(false, "https-cross-site-script-navigation");
    const auto cross = fixture.take(host, QStringLiteral("/check-cross"));
    ok &= check(cross && !cross->strict && cross->lax && cross->none,
                "https-cross-site-navigation-excludes-strict-keeps-lax-and-none");
    if (!navigate(page, fixture.url(host, QStringLiteral("/partition/check-first"))))
        return check(false, "https-first-party-partition-request");
    const auto first = fixture.take(host, QStringLiteral("/partition/check-first"));
    ok &= check(first && first->plain && first->first && !first->ancestor
                        && !first->topA && !first->topB,
                "https-first-party-partition-and-unpartitioned-collision");
    if (!navigate(page, fixture.url(host, QStringLiteral("/ancestor-check"))))
        return check(false, "https-cross-site-ancestor-request");
    const auto ancestor = fixture.take(host, QStringLiteral("/partition/check-ancestor"));
    ok &= check(ancestor && ancestor->ancestor && !ancestor->first
                            && !ancestor->topA && !ancestor->topB,
                "https-same-top-site-cross-site-ancestor-selects-distinct-partition");
    for (const QString &suffix : {QStringLiteral("a"), QStringLiteral("b")}) {
        if (!navigate(page, fixture.url(QStringLiteral("top-%1.localhost").arg(suffix),
                                       QStringLiteral("/check-frame"))))
            return check(false, "https-third-party-partition-request");
        const auto third = fixture.take(host, QStringLiteral("/partition/check-") + suffix);
        ok &= check(third && !third->first && !third->ancestor
                            && third->topA == (suffix == u"a") && third->topB == (suffix == u"b"),
                    suffix == u"a" ? "https-top-site-a-selects-only-its-partition"
                                    : "https-top-site-b-selects-only-its-partition");
    }
    return ok;
}

bool edgeRequestChecks(Fixture &fixture, QWebEnginePage &page, const char *navigationLabel,
                       const char *requestLabel)
{
    const QString host = QStringLiteral("cookie.localhost");
    if (!check(navigate(page, fixture.url(host, QStringLiteral("/edge/check"))), navigationLabel))
        return false;
    const auto seen = fixture.take(host, QStringLiteral("/edge/check"));
    return check(seen && seen->edgeValid && seen->edgeFar && !seen->edgeExpired
                         && !seen->edgeHostBad && !seen->edgeSecureBad
                         && !seen->edgeNoneBad && !seen->edgePartitionBad, requestLabel);
}
} // namespace

int main(int argc, char **argv)
{
    if (argc != 3 || qEnvironmentVariableIsSet("QTWEBENGINE_REMOTE_DEBUGGING")
            || qEnvironmentVariableIsSet("QTWEBENGINE_DISABLE_SANDBOX")
            || !qgetenv("QTWEBENGINE_CHROMIUM_FLAGS").isEmpty())
        return 2;
    const QString certificatePath = QString::fromLocal8Bit(argv[1]);
    const QString keyPath = QString::fromLocal8Bit(argv[2]);
    if (!QFileInfo(certificatePath).isAbsolute() || !QFileInfo(keyPath).isAbsolute())
        return 2;
    QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
    QApplication app(argc, argv);
    app.setQuitOnLastWindowClosed(false);
    std::cout << std::unitbuf;
    std::cout << "ENGINE qt=" << qVersion() << " webengine=" << qWebEngineVersion()
              << " chromium=" << qWebEngineChromiumVersion()
              << " security_patch=" << qWebEngineChromiumSecurityPatchVersion() << '\n';
    Fixture fixture;
    if (!check(fixture.start(certificatePath, keyPath), "https-owned-loopback-tls-server"))
        return 1;
    QWebEngineProfile sourceProfile, childProfile;
    QWebEnginePage source(&sourceProfile), child(&childProfile);
    fixture.pin(source);
    fixture.pin(child);
    if (!check(sourceProfile.isOffTheRecord() && childProfile.isOffTheRecord()
                       && navigate(child, QUrl(QStringLiteral("about:blank"))), "https-owned-otr-profiles"))
        return 1;
    if (!check(navigate(source, fixture.url(QStringLiteral("cookie.localhost"), QStringLiteral("/seed")))
                       && navigate(source, fixture.url(QStringLiteral("cookie.localhost"), QStringLiteral("/partition/seed-first")))
                       && navigate(source, fixture.url(QStringLiteral("cookie.localhost"), QStringLiteral("/ancestor-seed")))
                       && navigate(source, fixture.url(QStringLiteral("top-a.localhost"), QStringLiteral("/seed-frame")))
                       && navigate(source, fixture.url(QStringLiteral("top-b.localhost"), QStringLiteral("/seed-frame"))),
               "https-response-and-script-cookie-fixtures-created"))
        return 1;
    if (!requestChecks(fixture, source))
        return 1;
    using Store = QWebEngineCookieStore;
    struct Completion { int count = 0; std::optional<Store::FerricProbeResult> result; };
    auto completion = std::make_shared<Completion>();
    sourceProfile.cookieStore()->ferricCookieCopyExistingProbe(childProfile.cookieStore(),
                                                              [completion](Store::FerricProbeResult result) {
        ++completion->count;
        completion->result = result;
    });
    if (!check(waitFor([&] { return completion->result.has_value(); }) && completion->count == 1,
               "https-native-copy-completed-once"))
        return 1;
    const auto result = *completion->result;
    if (!check(result.status == Store::FerricProbeStatus::ExactReadback
                       && result.sourceCount == 19 && result.destinationCount == 19,
               "https-nineteen-canonical-records-exact-before-child-https-navigation")) {
        std::cout << "RESULT status=" << int(result.status) << " source_count=" << result.sourceCount
                  << " destination_count=" << result.destinationCount << '\n';
        return 1;
    }
    bool ok = requestChecks(fixture, child);
    {
        QWebEngineProfile edgeSourceProfile, edgeChildProfile;
        QWebEnginePage edgeSource(&edgeSourceProfile), edgeChild(&edgeChildProfile);
        fixture.pin(edgeSource);
        fixture.pin(edgeChild);
        std::optional<QDateTime> farExpiry;
        QObject::connect(edgeSourceProfile.cookieStore(), &QWebEngineCookieStore::cookieAdded,
                         &edgeSource, [&](const QNetworkCookie &cookie) {
            if (cookie.name() == "edge_far")
                farExpiry = cookie.expirationDate().toUTC();
        });
        if (!check(edgeSourceProfile.isOffTheRecord() && edgeChildProfile.isOffTheRecord()
                           && navigate(edgeChild, QUrl(QStringLiteral("about:blank")))
                           && navigate(edgeSource, fixture.url(QStringLiteral("cookie.localhost"),
                                                               QStringLiteral("/edge/seed"))),
                   "https-edge-owned-otr-response-fixture-created"))
            return 1;
        const auto now = QDateTime::currentDateTimeUtc();
        ok &= check(waitFor([&] { return farExpiry.has_value(); }) && farExpiry->isValid()
                            && now.daysTo(*farExpiry) >= 390 && now.daysTo(*farExpiry) <= 401,
                    "https-far-future-response-expiry-clamped-at-source");
        ok &= edgeRequestChecks(fixture, edgeSource, "https-edge-source-request",
                                "https-edge-source-keeps-valid-and-rejects-expired-invalid-setters");
        auto edgeCompletion = std::make_shared<Completion>();
        edgeSourceProfile.cookieStore()->ferricCookieCopyExistingProbe(edgeChildProfile.cookieStore(),
                [edgeCompletion](Store::FerricProbeResult value) {
            ++edgeCompletion->count;
            edgeCompletion->result = value;
        });
        if (!check(waitFor([&] { return edgeCompletion->result.has_value(); })
                           && edgeCompletion->count == 1
                           && edgeCompletion->result->status == Store::FerricProbeStatus::ExactReadback
                           && edgeCompletion->result->sourceCount == 2
                           && edgeCompletion->result->destinationCount == 2,
                   "https-edge-two-accepted-records-copy-exactly-after-whole-list-preflight"))
            return 1;
        ok &= edgeRequestChecks(fixture, edgeChild, "https-edge-child-request",
                                "https-edge-child-keeps-valid-and-rejects-expired-invalid-setters");
    }
    ok &= check(fixture.certificateScopeHeld(), "https-only-pinned-fixture-certificate-accepted");
    std::cout << "PRODUCTION_CAPABILITY_ENABLED=false\n";
    return ok ? 0 : 1;
}
