#include <QApplication>
#include <QDateTime>
#include <QDir>
#include <QEventLoop>
#include <QFileInfo>
#include <QNetworkCookie>
#include <QTimer>
#include <QWebEngineCookieStore>
#include <QWebEnginePage>
#include <QWebEngineProfile>
#include <QtWebEngineCore/qtwebenginecoreglobal.h>

#include <functional>
#include <iostream>
#include <memory>
#include <optional>

namespace {
using Store = QWebEngineCookieStore;
using Result = Store::FerricProbeResult;

bool waitFor(const std::function<bool()> &ready, int deadlineMs = 25000)
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
    deadline.start(deadlineMs);
    loop.exec();
    return ready();
}

bool check(bool condition, const char *label)
{
    std::cout << (condition ? "PASS " : "FAIL ") << label << '\n';
    return condition;
}

bool loadBlank(QWebEnginePage &page)
{
    bool finished = false;
    bool loaded = false;
    const auto connection = QObject::connect(&page, &QWebEnginePage::loadFinished,
                                             [&](bool ok) { loaded = ok; finished = true; });
    page.load(QUrl(QStringLiteral("about:blank")));
    const bool ready = waitFor([&] { return finished; }) && loaded;
    QObject::disconnect(connection);
    return ready;
}

QByteArray fixtureName(const QString &fixture)
{
    if (fixture == QLatin1String("legacy"))
        return QByteArray("ferric_legacy_fixture");
    if (fixture == QLatin1String("older"))
        return QByteArray("ferric_older_fixture");
    return QByteArray("ferric_t0_fixture");
}

bool disposableChildDirectory(const QString &directory)
{
    const QFileInfo childInfo(directory);
    const QString canonicalRoot = QDir(qEnvironmentVariable("FERRIC_SPIKE_DISPOSABLE_ROOT"))
                                          .canonicalPath();
    return childInfo.isDir() && !childInfo.isSymLink() && !canonicalRoot.isEmpty()
            && childInfo.canonicalFilePath().startsWith(canonicalRoot + QLatin1Char('/'));
}

bool seed(const QString &directory, const QString &fixture)
{
    QWebEngineProfile source(QStringLiteral("ferric-source-fixture"));
    source.setPersistentStoragePath(directory);
    source.setPersistentCookiesPolicy(QWebEngineProfile::ForcePersistentCookies);
    QWebEnginePage page(&source);
    if (!check(!source.isOffTheRecord() && source.persistentStoragePath() == directory
                       && loadBlank(page), "isolated-persistent-source-ready"))
        return false;

    bool added = false;
    bool siblingAdded = false;
    const auto name = fixtureName(fixture);
    const auto connection = QObject::connect(source.cookieStore(), &Store::cookieAdded,
                                             [&](const QNetworkCookie &cookie) {
        if (cookie.name() == name)
            added = true;
        if (cookie.name() == QByteArray("ferric_safe_fixture"))
            siblingAdded = true;
    });
    QNetworkCookie cookie(name, QByteArray("synthetic"));
    cookie.setPath(QStringLiteral("/"));
    cookie.setSecure(true);
    cookie.setExpirationDate(QDateTime::currentDateTimeUtc().addDays(1));
    QNetworkCookie sibling(QByteArray("ferric_safe_fixture"), QByteArray("synthetic"));
    sibling.setPath(QStringLiteral("/"));
    sibling.setSecure(true);
    sibling.setExpirationDate(QDateTime::currentDateTimeUtc().addDays(1));
    source.cookieStore()->setCookie(cookie, QUrl(QStringLiteral("https://fixture.test/")));
    source.cookieStore()->setCookie(sibling, QUrl(QStringLiteral("https://fixture.test/")));
    const bool ok = check(waitFor([&] { return added && siblingAdded; }),
                          "ordinary-cookie-pair-persist-request-accepted");
    QObject::disconnect(connection);
    // The runner waits for this process to exit before touching its disposable
    // backing store, so Qt and the fixture writer never share a live database.
    return ok;
}

bool observe(const QString &directory, const QString &fixture,
             bool expectRefusal, bool seedChild, bool unpublishedChild)
{
    QWebEngineProfile source(QStringLiteral("ferric-source-fixture"));
    source.setPersistentStoragePath(directory);
    source.setPersistentCookiesPolicy(QWebEngineProfile::ForcePersistentCookies);
    std::unique_ptr<QWebEngineProfile> child = seedChild
            ? std::make_unique<QWebEngineProfile>(QStringLiteral("ferric-child-fixture"))
            : std::make_unique<QWebEngineProfile>();
    const QString childDirectory = directory + QStringLiteral("-child");
    if (seedChild) {
        if (!check(disposableChildDirectory(childDirectory),
                   "fresh-child-stays-in-disposable-root"))
            return false;
        child->setPersistentStoragePath(childDirectory);
        child->setPersistentCookiesPolicy(QWebEngineProfile::ForcePersistentCookies);
    }
    QWebEnginePage sourcePage(&source);
    std::unique_ptr<QWebEnginePage> childPage;
    if (!unpublishedChild)
        childPage = std::make_unique<QWebEnginePage>(child.get());
    else
        child->ferricInitializeCookieManagerProbe();
    if (!check(!source.isOffTheRecord()
                       && (seedChild ? !child->isOffTheRecord()
                                       && child->persistentStoragePath() == childDirectory
                                     : child->isOffTheRecord())
                       && source.persistentStoragePath() == directory
                       && loadBlank(sourcePage)
                       && (!childPage || loadBlank(*childPage)),
               unpublishedChild ? "reopened-source-and-unpublished-child-ready"
                                : "reopened-source-and-fresh-child-ready"))
        return false;

    auto result = std::make_shared<std::optional<Result>>();
    auto completions = std::make_shared<int>(0);
    auto completion = [result, completions](Result value) {
        *result = value;
        ++*completions;
    };
    if (seedChild)
        source.cookieStore()->ferricCookieSeedExistingProbe(child->cookieStore(), completion);
    else
        source.cookieStore()->ferricCookieCopyExistingProbe(child->cookieStore(), completion);
    const bool completed = check(waitFor([&] { return result->has_value(); })
                                     && *completions == 1, "disk-loaded-source-probe-completed-once");
    if (!completed)
        return false;
    if (seedChild)
        std::cout << "seed-status=" << int(result->value().status)
                  << " source-count=" << result->value().sourceCount
                  << " destination-count=" << result->value().destinationCount
                  << " completions=" << *completions << '\n';
    const bool expected = expectRefusal
            ? result->value().status == Store::FerricProbeStatus::UnsupportedRecord
                      && result->value().sourceCount == 2
                      && result->value().destinationCount == 0
            : result->value().status == Store::FerricProbeStatus::ExactReadback
                      && result->value().sourceCount == 2
                      && result->value().destinationCount == 2;
    const bool qualified = check(expected,
                                 unpublishedChild ? "disk-loaded-edge-source-seeded-before-child-page"
                                 : seedChild ? "disk-loaded-edge-source-seeded-exactly-to-fresh-child"
                                 : !expectRefusal ? "ordinary-disk-source-copies-before-mutation"
                                                : fixture == QLatin1String("legacy")
                                                          ? "disk-loaded-legacy-refused-before-child-write"
                                                          : "disk-loaded-older-expiry-refused-before-child-write");
    if (qualified && unpublishedChild) {
        childPage = std::make_unique<QWebEnginePage>(child.get());
        if (!check(loadBlank(*childPage), "seeded-child-page-published-after-readback"))
            return false;
        QWebEngineProfile fresh;
        QWebEnginePage freshPage(&fresh);
        if (!check(loadBlank(freshPage), "post-publication-control-ready"))
            return false;
        auto copied = std::make_shared<std::optional<Result>>();
        auto copyCompletions = std::make_shared<int>(0);
        child->cookieStore()->ferricCookieCopyExistingProbe(fresh.cookieStore(),
                [copied, copyCompletions](Result value) {
            *copied = value;
            ++*copyCompletions;
        });
        const bool unchanged = check(waitFor([&] { return copied->has_value(); })
                                             && *copyCompletions == 1
                                             && copied->value().status == Store::FerricProbeStatus::UnsupportedRecord
                                             && copied->value().sourceCount == 2
                                             && copied->value().destinationCount == 0,
                                     "post-publication-child-retains-two-record-edge-source");
        std::cout << "PRODUCTION_CAPABILITY_ENABLED=false\n";
        return unchanged;
    }
    std::cout << "PRODUCTION_CAPABILITY_ENABLED=false\n";
    return qualified;
}

bool rejectNonEmptyChild(const QString &directory)
{
    const QString childDirectory = directory + QStringLiteral("-nonempty-child");
    if (!check(disposableChildDirectory(childDirectory),
               "nonempty-child-stays-in-disposable-root"))
        return false;
    QWebEngineProfile source(QStringLiteral("ferric-source-fixture"));
    source.setPersistentStoragePath(directory);
    source.setPersistentCookiesPolicy(QWebEngineProfile::ForcePersistentCookies);
    QWebEngineProfile child(QStringLiteral("ferric-nonempty-child-fixture"));
    child.setPersistentStoragePath(childDirectory);
    child.setPersistentCookiesPolicy(QWebEngineProfile::ForcePersistentCookies);
    QWebEngineProfile fresh;
    QWebEnginePage sourcePage(&source);
    QWebEnginePage childPage(&child);
    QWebEnginePage freshPage(&fresh);
    if (!check(!source.isOffTheRecord() && !child.isOffTheRecord()
                       && fresh.isOffTheRecord()
                       && source.persistentStoragePath() == directory
                       && child.persistentStoragePath() == childDirectory
                       && loadBlank(sourcePage) && loadBlank(childPage)
                       && loadBlank(freshPage),
               "ordinary-source-and-nonempty-child-ready"))
        return false;

    bool markerAdded = false;
    const auto connection = QObject::connect(child.cookieStore(), &Store::cookieAdded,
                                             [&](const QNetworkCookie &cookie) {
        if (cookie.name() == QByteArray("ferric_child_marker"))
            markerAdded = true;
    });
    QNetworkCookie marker(QByteArray("ferric_child_marker"), QByteArray("synthetic"));
    marker.setPath(QStringLiteral("/"));
    marker.setSecure(true);
    marker.setExpirationDate(QDateTime::currentDateTimeUtc().addDays(1));
    child.cookieStore()->setCookie(marker, QUrl(QStringLiteral("https://fixture.test/")));
    const bool marked = check(waitFor([&] { return markerAdded; }),
                              "nonempty-child-marker-accepted");
    QObject::disconnect(connection);
    if (!marked)
        return false;

    auto seeded = std::make_shared<std::optional<Result>>();
    auto seedCompletions = std::make_shared<int>(0);
    source.cookieStore()->ferricCookieSeedExistingProbe(child.cookieStore(),
            [seeded, seedCompletions](Result value) {
        *seeded = value;
        ++*seedCompletions;
    });
    const bool seedCompleted = waitFor([&] { return seeded->has_value(); });
    if (seeded->has_value())
        std::cout << "nonempty-seed-status=" << int(seeded->value().status)
                  << " source-count=" << seeded->value().sourceCount
                  << " destination-count=" << seeded->value().destinationCount
                  << " completions=" << *seedCompletions << '\n';
    if (!check(seedCompleted
                       && *seedCompletions == 1
                       && seeded->value().status == Store::FerricProbeStatus::MutationRejected
                       && seeded->value().sourceCount == 2
                       && seeded->value().destinationCount == -1,
               "nonempty-child-seed-refused"))
        return false;

    auto copied = std::make_shared<std::optional<Result>>();
    auto copyCompletions = std::make_shared<int>(0);
    child.cookieStore()->ferricCookieCopyExistingProbe(fresh.cookieStore(),
            [copied, copyCompletions](Result value) {
        *copied = value;
        ++*copyCompletions;
    });
    const bool unchanged = check(waitFor([&] { return copied->has_value(); })
                                         && *copyCompletions == 1
                                         && copied->value().status == Store::FerricProbeStatus::ExactReadback
                                         && copied->value().sourceCount == 1
                                         && copied->value().destinationCount == 1,
                                 "nonempty-child-retains-only-marker");
    std::cout << "PRODUCTION_CAPABILITY_ENABLED=false\n";
    return unchanged;
}

bool snapshotBoundaryRace(const QString &directory)
{
    const QString childDirectory = directory + QStringLiteral("-race-child");
    if (!check(disposableChildDirectory(childDirectory),
               "t0-child-stays-in-disposable-root"))
        return false;
    QWebEngineProfile source(QStringLiteral("ferric-source-fixture"));
    source.setPersistentStoragePath(directory);
    source.setPersistentCookiesPolicy(QWebEngineProfile::ForcePersistentCookies);
    QWebEngineProfile child(QStringLiteral("ferric-t0-child-fixture"));
    child.setPersistentStoragePath(childDirectory);
    child.setPersistentCookiesPolicy(QWebEngineProfile::ForcePersistentCookies);
    QWebEngineProfile sourceControl;
    QWebEngineProfile childControl;
    QWebEngineProfile concurrentChild;
    QWebEngineProfile concurrentControl;
    QWebEnginePage sourcePage(&source);
    QWebEnginePage childPage(&child);
    QWebEnginePage sourceControlPage(&sourceControl);
    QWebEnginePage childControlPage(&childControl);
    QWebEnginePage concurrentChildPage(&concurrentChild);
    QWebEnginePage concurrentControlPage(&concurrentControl);
    if (!check(!source.isOffTheRecord() && !child.isOffTheRecord()
                       && sourceControl.isOffTheRecord() && childControl.isOffTheRecord()
                       && source.persistentStoragePath() == directory
                       && child.persistentStoragePath() == childDirectory
                       && loadBlank(sourcePage) && loadBlank(childPage)
                       && loadBlank(sourceControlPage) && loadBlank(childControlPage)
                       && loadBlank(concurrentChildPage) && loadBlank(concurrentControlPage),
               "t0-source-child-and-controls-ready"))
        return false;

    // INVARIANT: the acknowledged pre-cut write completes before GetAllCookies
    // is requested; the isolated engine overlay injects a second write only
    // after it has copied the source list and before it seeds the child.
    bool preCutAdded = false;
    const auto preCutConnection = QObject::connect(
            source.cookieStore(), &Store::cookieAdded, &source,
            [&](const QNetworkCookie &cookie) {
        if (cookie.name() == QByteArray("ferric_before_t0_fixture"))
            preCutAdded = true;
    });
    QNetworkCookie beforeCut(QByteArray("ferric_before_t0_fixture"), QByteArray("synthetic"));
    beforeCut.setPath(QStringLiteral("/"));
    beforeCut.setSecure(true);
    beforeCut.setExpirationDate(QDateTime::currentDateTimeUtc().addDays(1));
    source.cookieStore()->setCookie(beforeCut, QUrl(QStringLiteral("https://fixture.test/")));
    if (!check(waitFor([&] { return preCutAdded; }), "t0-pre-snapshot-write-acknowledged")) {
        QObject::disconnect(preCutConnection);
        return false;
    }
    QObject::disconnect(preCutConnection);

    auto seeded = std::make_shared<std::optional<Result>>();
    auto seedCompletions = std::make_shared<int>(0);
    source.cookieStore()->ferricCookieSeedExistingProbe(child.cookieStore(),
            [seeded, seedCompletions](Result value) {
        *seeded = value;
        ++*seedCompletions;
    });
    if (!check(waitFor([&] { return seeded->has_value(); })
                       && *seedCompletions == 1
                       && seeded->value().status == Store::FerricProbeStatus::ExactReadback
                       && seeded->value().sourceCount == 3
                       && seeded->value().destinationCount == 3,
               "t0-child-includes-pre-snapshot-write"))
        return false;

    auto sourceCopy = std::make_shared<std::optional<Result>>();
    source.cookieStore()->ferricCookieCopyExistingProbe(sourceControl.cookieStore(),
            [sourceCopy](Result value) { *sourceCopy = value; });
    if (!check(waitFor([&] { return sourceCopy->has_value(); })
                                        && sourceCopy->value().status == Store::FerricProbeStatus::ExactReadback
                                        && sourceCopy->value().sourceCount == 4
                                        && sourceCopy->value().destinationCount == 4,
               "t0-source-includes-post-snapshot-write"))
        return false;

    auto childCopy = std::make_shared<std::optional<Result>>();
    child.cookieStore()->ferricCookieCopyExistingProbe(childControl.cookieStore(),
            [childCopy](Result value) { *childCopy = value; });
    const bool isolated = check(waitFor([&] { return childCopy->has_value(); })
                                        && childCopy->value().status == Store::FerricProbeStatus::ExactReadback
                                        && childCopy->value().sourceCount == 3
                                        && childCopy->value().destinationCount == 3,
                                "t0-child-remains-at-three-record-cut");
    if (!isolated)
        return false;

    // INVARIANT: the multi-step copy-existing probe and public setter are
    // dispatched before the Qt event loop is pumped again. The copy probe
    // performs source/destination preflight reads before its actual complete-
    // list request, so this writer may complete during preflight. The captured
    // list must still be one exact complete state, never a partial/torn list.
    // This does not force a mutation during CookieMonster's map copy. Only the
    // synthetic name/count reaches stdout.
    bool concurrentWriterAdded = false;
    const auto concurrentWriterConnection = QObject::connect(
            source.cookieStore(), &Store::cookieAdded, &source,
            [&](const QNetworkCookie &cookie) {
        if (cookie.name() == QByteArray("ferric_t0_concurrent_fixture"))
            concurrentWriterAdded = true;
    });
    QNetworkCookie concurrentCookie(QByteArray("ferric_t0_concurrent_fixture"),
                                    QByteArray("synthetic"));
    concurrentCookie.setPath(QStringLiteral("/"));
    concurrentCookie.setSecure(true);
    concurrentCookie.setExpirationDate(QDateTime::currentDateTimeUtc().addDays(1));
    auto concurrentSnapshot = std::make_shared<std::optional<Result>>();
    source.cookieStore()->ferricCookieCopyExistingProbe(concurrentChild.cookieStore(),
            [concurrentSnapshot](Result value) { *concurrentSnapshot = value; });
    source.cookieStore()->setCookie(concurrentCookie,
                                    QUrl(QStringLiteral("https://fixture.test/")));
    const bool concurrentCompleted = check(
            waitFor([&] {
                return concurrentSnapshot->has_value() && concurrentWriterAdded;
            }), "t0-copy-operation-and-public-writer-completed");
    QObject::disconnect(concurrentWriterConnection);
    if (!concurrentCompleted)
        return false;

    const int concurrentCutCount = concurrentSnapshot->value().sourceCount;
    std::cout << "copy-probe-cut-count=" << concurrentCutCount
              << " source-after-copy-probe-count=5\n";
    if (!check(concurrentSnapshot->value().status == Store::FerricProbeStatus::ExactReadback
                       && (concurrentCutCount == 4 || concurrentCutCount == 5)
                       && concurrentSnapshot->value().destinationCount == concurrentCutCount,
               "t0-copy-probe-writer-order-resolves-to-exact-cut"))
        return false;

    auto concurrentSourceCopy = std::make_shared<std::optional<Result>>();
    source.cookieStore()->ferricCookieCopyExistingProbe(concurrentControl.cookieStore(),
            [concurrentSourceCopy](Result value) { *concurrentSourceCopy = value; });
    const bool concurrentSourceExact = check(
            waitFor([&] { return concurrentSourceCopy->has_value(); })
                    && concurrentSourceCopy->value().status
                            == Store::FerricProbeStatus::ExactReadback
            && concurrentSourceCopy->value().sourceCount == 5
            && concurrentSourceCopy->value().destinationCount == 5,
            "t0-source-settles-to-five-exact-records");
    std::cout << "PRODUCTION_CAPABILITY_ENABLED=false\n";
    return concurrentSourceExact;
}

bool verifyPersistedChild(const QString &directory, const QString &fixture)
{
    const QString childDirectory = directory + QStringLiteral("-child");
    if (!check(disposableChildDirectory(childDirectory),
               "reopened-child-stays-in-disposable-root"))
        return false;
    QWebEngineProfile child(QStringLiteral("ferric-child-fixture"));
    child.setPersistentStoragePath(childDirectory);
    child.setPersistentCookiesPolicy(QWebEngineProfile::ForcePersistentCookies);
    QWebEngineProfile fresh;
    bool edgeSeen = false;
    bool siblingSeen = false;
    const auto name = fixtureName(fixture);
    const auto connection = QObject::connect(child.cookieStore(), &Store::cookieAdded,
                                            [&](const QNetworkCookie &cookie) {
        if (cookie.name() == name)
            edgeSeen = true;
        if (cookie.name() == QByteArray("ferric_safe_fixture"))
            siblingSeen = true;
    });
    child.cookieStore()->loadAllCookies();
    QWebEnginePage childPage(&child);
    QWebEnginePage freshPage(&fresh);
    if (!check(!child.isOffTheRecord() && fresh.isOffTheRecord()
                       && child.persistentStoragePath() == childDirectory
                       && loadBlank(childPage) && loadBlank(freshPage),
               "persisted-child-and-fresh-control-ready")) {
        QObject::disconnect(connection);
        return false;
    }
    const bool namesObserved = waitFor([&] { return edgeSeen && siblingSeen; }, 1000);
    std::cout << "INFO restarted-child-qt-name-signals-observed="
              << (namesObserved ? "true" : "false") << '\n';
    QObject::disconnect(connection);
    auto result = std::make_shared<std::optional<Result>>();
    auto completions = std::make_shared<int>(0);
    child.cookieStore()->ferricCookieCopyExistingProbe(fresh.cookieStore(),
            [result, completions](Result value) {
        *result = value;
        ++*completions;
    });
    const bool copyCompleted = waitFor([&] { return result->has_value(); });
    if (result->has_value())
        std::cout << "restarted-child-status=" << int(result->value().status)
                  << " source-count=" << result->value().sourceCount
                  << " destination-count=" << result->value().destinationCount
                  << " completions=" << *completions << '\n';
    const bool retained = check(copyCompleted
                                        && *completions == 1
                                        && result->value().status == Store::FerricProbeStatus::UnsupportedRecord
                                        && result->value().sourceCount == 2
                                        && result->value().destinationCount == 0,
                                "restarted-child-retains-edge-refusal-and-safe-sibling");
    std::cout << "PRODUCTION_CAPABILITY_ENABLED=false\n";
    return retained;
}

bool destinationOwnerLoss(const QString &directory)
{
    const QString childDirectory = directory + QStringLiteral("-owner-loss-child");
    if (!check(disposableChildDirectory(childDirectory),
               "owner-loss-child-stays-in-disposable-root"))
        return false;

    QWebEngineProfile source(QStringLiteral("ferric-owner-loss-source"));
    source.setPersistentStoragePath(directory);
    source.setPersistentCookiesPolicy(QWebEngineProfile::ForcePersistentCookies);
    auto child = std::make_unique<QWebEngineProfile>(QStringLiteral("ferric-owner-loss-child"));
    child->setPersistentStoragePath(childDirectory);
    child->setPersistentCookiesPolicy(QWebEngineProfile::ForcePersistentCookies);
    QWebEnginePage sourcePage(&source);
    auto childPage = std::make_unique<QWebEnginePage>(child.get());
    if (!check(source.persistentStoragePath() == directory
                       && child->persistentStoragePath() == childDirectory
                       && loadBlank(sourcePage) && loadBlank(*childPage),
               "owner-loss-source-and-child-ready"))
        return false;

    auto lost = std::make_shared<std::optional<Result>>();
    auto lostCompletions = std::make_shared<int>(0);
    source.cookieStore()->ferricCookieCopyExistingProbe(child->cookieStore(),
            [lost, lostCompletions](Result value) {
        *lost = value;
        ++*lostCompletions;
    });
    // The exact destination owner is destroyed before the event loop can route
    // the pending native reply. A late callback must remain stale.
    childPage.reset();
    child.reset();
    const bool retired = check(waitFor([&] { return lost->has_value(); }, 1000)
                                       && *lostCompletions == 1
                                       && lost->value().status == Store::FerricProbeStatus::OwnerLost,
                               "destroyed-destination-owner-retires-pending-copy-once");
    if (!retired)
        return false;

    QWebEngineProfile control;
    QWebEnginePage controlPage(&control);
    if (!check(loadBlank(controlPage), "owner-loss-control-destination-ready"))
        return false;
    auto recovered = std::make_shared<std::optional<Result>>();
    auto recoveredCompletions = std::make_shared<int>(0);
    source.cookieStore()->ferricCookieCopyExistingProbe(control.cookieStore(),
            [recovered, recoveredCompletions](Result value) {
        *recovered = value;
        ++*recoveredCompletions;
    });
    const bool sourceReusable = check(waitFor([&] { return recovered->has_value(); })
                                              && *recoveredCompletions == 1
                                              && recovered->value().status == Store::FerricProbeStatus::ExactReadback
                                              && recovered->value().sourceCount == 2
                                              && recovered->value().destinationCount == 2,
                                      "source-remains-usable-after-destination-owner-loss");
    std::cout << "PRODUCTION_CAPABILITY_ENABLED=false\n";
    return sourceReusable;
}

bool sourceOwnerLoss(const QString &directory)
{
    const QString childDirectory = directory + QStringLiteral("-owner-loss-source-child");
    if (!check(disposableChildDirectory(childDirectory),
               "source-owner-loss-child-stays-in-disposable-root"))
        return false;

    auto source = std::make_unique<QWebEngineProfile>(
            QStringLiteral("ferric-source-owner-loss-source"));
    source->setPersistentStoragePath(directory);
    source->setPersistentCookiesPolicy(QWebEngineProfile::ForcePersistentCookies);
    QWebEngineProfile child(QStringLiteral("ferric-source-owner-loss-child"));
    child.setPersistentStoragePath(childDirectory);
    child.setPersistentCookiesPolicy(QWebEngineProfile::ForcePersistentCookies);
    auto sourcePage = std::make_unique<QWebEnginePage>(source.get());
    QWebEnginePage childPage(&child);
    if (!check(!source->isOffTheRecord() && !child.isOffTheRecord()
                       && source->persistentStoragePath() == directory
                       && child.persistentStoragePath() == childDirectory
                       && loadBlank(*sourcePage) && loadBlank(childPage),
               "source-owner-loss-source-and-child-ready"))
        return false;

    auto lost = std::make_shared<std::optional<Result>>();
    auto lostCompletions = std::make_shared<int>(0);
    source->cookieStore()->ferricCookieCopyExistingProbe(child.cookieStore(),
            [lost, lostCompletions](Result value) {
        *lost = value;
        ++*lostCompletions;
    });
    // Retire the exact source page/profile before the event loop can route the
    // pending native reply. A possibly mutated child remains disposable.
    sourcePage.reset();
    source.reset();
    const bool retired = check(waitFor([&] { return lost->has_value(); }, 1000)
                                       && *lostCompletions == 1
                                       && lost->value().status
                                               == Store::FerricProbeStatus::OwnerLost,
                               "destroyed-source-owner-retires-pending-copy-once");
    std::cout << "PRODUCTION_CAPABILITY_ENABLED=false\n";
    return retired;
}

bool serviceLoss(const QString &directory, const QString &stage)
{
    const QString childDirectory = directory + QStringLiteral("-service-loss-child");
    if (!check(disposableChildDirectory(childDirectory),
               "service-loss-child-stays-in-disposable-root"))
        return false;
    QWebEngineProfile source(QStringLiteral("ferric-service-loss-source"));
    source.setPersistentStoragePath(directory);
    source.setPersistentCookiesPolicy(QWebEngineProfile::ForcePersistentCookies);
    QWebEngineProfile child(QStringLiteral("ferric-service-loss-child"));
    child.setPersistentStoragePath(childDirectory);
    child.setPersistentCookiesPolicy(QWebEngineProfile::ForcePersistentCookies);
    QWebEnginePage sourcePage(&source);
    QWebEnginePage childPage(&child);
    if (!check(!source.isOffTheRecord() && !child.isOffTheRecord()
                       && source.persistentStoragePath() == directory
                       && child.persistentStoragePath() == childDirectory
                       && loadBlank(sourcePage) && loadBlank(childPage),
               "service-loss-source-and-child-ready"))
        return false;

    const bool sourceSnapshot = stage == QLatin1String("service-loss-source");
    const auto fixtureStage = sourceSnapshot ? Store::FerricServiceLossProbeStage::SourceSnapshot
                                             : Store::FerricServiceLossProbeStage::DestinationReadback;
    auto result = std::make_shared<std::optional<Result>>();
    auto completions = std::make_shared<int>(0);
    source.cookieStore()->ferricCookieServiceLossProbe(child.cookieStore(), fixtureStage,
            [result, completions](Result value) {
        *result = value;
        ++*completions;
    });
    if (!check(waitFor([&] { return result->has_value(); }, 5000),
               "disconnected-inflight-cookie-read-times-out-boundedly"))
        return false;

    QEventLoop lateReplyWindow;
    QTimer::singleShot(700, &lateReplyWindow, &QEventLoop::quit);
    lateReplyWindow.exec();
    const auto value = result->value();
    const bool expected = value.status == Store::FerricProbeStatus::TimedOut
            && value.sourceCount == (sourceSnapshot ? 0 : 2)
            && value.destinationCount == (sourceSnapshot ? 0 : -1)
            && *completions == 1;
    std::cout << "service-loss-stage=" << stage.toStdString()
              << " status=" << int(value.status)
              << " source-count=" << value.sourceCount
              << " destination-count=" << value.destinationCount
              << " completions=" << *completions << '\n';
    const bool qualified = check(expected,
                                 sourceSnapshot
                                         ? "source-snapshot-pipe-loss-fails-closed-once"
                                         : "destination-readback-pipe-loss-is-indeterminate-and-child-discarded");
    std::cout << "PRODUCTION_CAPABILITY_ENABLED=false\n";
    return qualified;
}
} // namespace

int main(int argc, char **argv)
{
    if (argc != 4 || qEnvironmentVariableIsSet("QTWEBENGINE_REMOTE_DEBUGGING")
            || qEnvironmentVariableIsSet("QTWEBENGINE_DISABLE_SANDBOX")
            || !qgetenv("QTWEBENGINE_CHROMIUM_FLAGS").isEmpty()) {
        std::cerr << "Refusing arguments or engine overrides; run through the isolated runner.\n";
        return 2;
    }
    const QString stage = QString::fromLocal8Bit(argv[1]);
    const QString directory = QString::fromLocal8Bit(argv[2]);
    const QString fixture = QString::fromLocal8Bit(argv[3]);
    const QString disposableRoot = qEnvironmentVariable("FERRIC_SPIKE_DISPOSABLE_ROOT");
    const QString canonicalRoot = QDir(disposableRoot).canonicalPath();
    const QString canonicalDirectory = QFileInfo(directory).canonicalFilePath();
    if ((stage != QLatin1String("seed") && stage != QLatin1String("control")
            && stage != QLatin1String("observe") && stage != QLatin1String("seed-child")
            && stage != QLatin1String("seed-nonempty")
            && stage != QLatin1String("seed-child-unpublished")
            && stage != QLatin1String("owner-loss")
            && stage != QLatin1String("source-owner-loss")
            && stage != QLatin1String("service-loss-source")
            && stage != QLatin1String("service-loss-destination")
            && stage != QLatin1String("race")
            && stage != QLatin1String("verify-child"))
            || (fixture != QLatin1String("legacy") && fixture != QLatin1String("older")
                && fixture != QLatin1String("t0"))
            || !disposableRoot.contains(QStringLiteral("/probe-runtime."))
            || canonicalRoot.isEmpty() || canonicalDirectory.isEmpty()
            || !canonicalDirectory.startsWith(canonicalRoot + QLatin1Char('/'))
            || QFileInfo(directory).isSymLink()) {
        std::cerr << "Invalid isolated fixture stage.\n";
        return 2;
    }
    QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
    QApplication app(argc, argv);
    app.setQuitOnLastWindowClosed(false);
    std::cout << std::unitbuf;
    std::cout << "ENGINE qt=" << qVersion() << " webengine=" << qWebEngineVersion()
              << " chromium=" << qWebEngineChromiumVersion()
              << " security_patch=" << qWebEngineChromiumSecurityPatchVersion() << '\n';
    return (stage == QLatin1String("seed") ? seed(directory, fixture)
           : stage == QLatin1String("verify-child") ? verifyPersistedChild(directory, fixture)
           : stage == QLatin1String("seed-nonempty") ? rejectNonEmptyChild(directory)
           : stage == QLatin1String("owner-loss") ? destinationOwnerLoss(directory)
           : stage == QLatin1String("source-owner-loss") ? sourceOwnerLoss(directory)
           : stage == QLatin1String("service-loss-source")
                     || stage == QLatin1String("service-loss-destination")
                     ? serviceLoss(directory, stage)
           : stage == QLatin1String("race") ? snapshotBoundaryRace(directory)
           : observe(directory, fixture, stage == QLatin1String("observe"),
                     stage == QLatin1String("seed-child")
                             || stage == QLatin1String("seed-child-unpublished"),
                     stage == QLatin1String("seed-child-unpublished"))) ? 0 : 1;
}
