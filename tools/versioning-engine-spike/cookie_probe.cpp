#include <QApplication>
#include <QElapsedTimer>
#include <QEventLoop>
#include <QThread>
#include <QTimer>
#include <QWebEngineCookieStore>
#include <QWebEnginePage>
#include <QWebEnginePermission>
#include <QWebEngineProfile>
#include <QtWebEngineCore/qtwebenginecoreglobal.h>

#include <functional>
#include <iostream>
#include <memory>
#include <optional>

namespace {
using Store = QWebEngineCookieStore;
using Status = Store::FerricProbeStatus;
using Result = Store::FerricProbeResult;

struct Completion {
    int count = 0;
    std::optional<Result> result;
    void receive(Result value) { result = value; ++count; }
};

bool waitFor(const std::function<bool()> &ready)
{
    if (ready())
        return true;
    // A real event loop also services deferred QObject deletion; repeated
    // processEvents() calls alone can retain completed native operations.
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

bool check(bool condition, const char *label)
{
    std::cout << (condition ? "PASS " : "FAIL ") << label << '\n';
    return condition;
}

bool loadBlank(QWebEnginePage *page)
{
    bool finished = false;
    bool loaded = false;
    const auto connection = QObject::connect(page, &QWebEnginePage::loadFinished,
                                            [&](bool ok) { loaded = ok; finished = true; });
    page->load(QUrl(QStringLiteral("about:blank")));
    const bool ready = waitFor([&] { return finished; }) && loaded;
    QObject::disconnect(connection);
    return ready;
}

struct DisposableProfile {
    // Destruction order is intentional: the page must die before its profile.
    std::unique_ptr<QWebEngineProfile> profile = std::make_unique<QWebEngineProfile>();
    std::unique_ptr<QWebEnginePage> page = std::make_unique<QWebEnginePage>(profile.get());

    bool initialize()
    {
        return profile->isOffTheRecord() && loadBlank(page.get());
    }

    Store *store() { return profile->cookieStore(); }
};

bool roundTrip(bool nonce)
{
    DisposableProfile source;
    DisposableProfile destination;
    if (!check(source.initialize() && destination.initialize(), "fresh-otr-profiles"))
        return false;
    bool ok = true;
    for (Store *invalid : {static_cast<Store *>(nullptr), source.store()}) {
        auto refused = std::make_shared<Completion>();
        source.store()->ferricCookieCloneProbe(invalid, false, [refused](Result value) {
            refused->receive(value);
        });
        ok &= check(refused->count == 1 && refused->result
                            && refused->result->status == Status::Unavailable,
                    invalid ? "same-store-refused" : "null-destination-refused");
    }
    for (Store *filtered : {source.store(), destination.store()}) {
        filtered->setCookieFilter([](const Store::FilterRequest &) { return false; });
        auto refused = std::make_shared<Completion>();
        source.store()->ferricCookieCloneProbe(destination.store(), false, [refused](Result value) {
            refused->receive(value);
        });
        ok &= check(refused->count == 1 && refused->result
                            && refused->result->status == Status::Unavailable
                            && refused->result->sourceCount == 0
                            && refused->result->destinationCount == 0,
                    filtered == source.store() ? "filtered-source-refused" : "filtered-destination-refused");
        filtered->setCookieFilter(nullptr);
    }
    auto state = std::make_shared<Completion>();
    source.store()->ferricCookieCloneProbe(destination.store(), nonce, [state](Result value) {
        state->receive(value);
    });
    // An overlapping operation is refused; it does not interleave fixture writes.
    auto overlap = std::make_shared<Completion>();
    source.store()->ferricCookieCloneProbe(destination.store(), false,
                                         [overlap](Result value) { overlap->receive(value); });
    ok &= check(overlap->count == 1 && overlap->result
                        && overlap->result->status == Status::Unavailable, "overlap-refused");
    if (!check(waitFor([&] { return state->result.has_value(); }), "bounded-completion"))
        return false;
    ok &= check(state->count == 1, "one-completion");
    const auto result = *state->result;
    if (nonce) {
        ok &= check(result.status == Status::UnsupportedRecord && result.sourceCount == 15
                        && result.destinationCount == 0,
                    "nonced-partition-refused-child-readback-empty");
    } else {
        ok &= check(result.status == Status::ExactReadback && result.sourceCount == 14
                        && result.destinationCount == 14,
                    "all-canonical-members-exact-four-partition-collision-variants");
    }
    if (!ok) {
        // Enum and counts only. Never print cookie objects or inclusion strings.
        std::cout << "RESULT status=" << int(result.status)
                  << " source_count=" << result.sourceCount
                  << " destination_count=" << result.destinationCount << '\n';
        return false;
    }
    state = std::make_shared<Completion>();
    source.store()->ferricCookieCloneProbe(destination.store(), false,
                                         [state](Result value) { state->receive(value); });
    ok &= check(waitFor([&] { return state->result.has_value(); })
                        && state->count == 1
                        && state->result->status == Status::NonEmptyStore,
                "nonempty-source-refused-no-destructive-cleanup");
    if (!nonce) {
        // Use a new source against the already-populated child. Then clone from
        // that source into another fresh child: success proves the refusal did
        // not start writing the synthetic source fixture either.
        DisposableProfile freshSource;
        DisposableProfile freshDestination;
        if (!check(freshSource.initialize() && freshDestination.initialize(),
                   "nonempty-destination-control-profiles"))
            return false;
        state = std::make_shared<Completion>();
        freshSource.store()->ferricCookieCloneProbe(destination.store(), false,
                                                  [state](Result value) { state->receive(value); });
        ok &= check(waitFor([&] { return state->result.has_value(); })
                            && state->count == 1
                            && state->result->status == Status::NonEmptyStore,
                    "nonempty-destination-refused");
        state = std::make_shared<Completion>();
        freshSource.store()->ferricCookieCloneProbe(freshDestination.store(), false,
                                                  [state](Result value) { state->receive(value); });
        ok &= check(waitFor([&] { return state->result.has_value(); })
                            && state->count == 1 && state->result->status == Status::ExactReadback
                            && state->result->sourceCount == 14 && state->result->destinationCount == 14,
                    "destination-refusal-left-source-empty-and-unlocked");
    }
    return ok;
}

bool syntheticPreflightRefusal(Store::FerricSyntheticPreflightCase fixture)
{
    const bool legacy = fixture == Store::FerricSyntheticPreflightCase::LegacyUnknownSource;
    DisposableProfile source;
    DisposableProfile destination;
    DisposableProfile freshDestination;
    if (!check(source.initialize() && destination.initialize() && freshDestination.initialize(),
               legacy ? "legacy-preflight-disposable-profiles" : "older-expiry-preflight-disposable-profiles"))
        return false;
    auto refused = std::make_shared<Completion>();
    source.store()->ferricCookieSyntheticPreflightProbe(destination.store(), fixture,
            [refused](Result value) { refused->receive(value); });
    bool ok = check(waitFor([&] { return refused->result.has_value(); })
                            && refused->count == 1
                            && refused->result->status == Status::UnsupportedRecord
                            && refused->result->sourceCount == 14
                            && refused->result->destinationCount == 0,
                    legacy ? "synthetic-legacy-source-refused-before-child-write"
                           : "synthetic-older-expiry-refused-before-child-write");
    // The unsafe candidate was substituted only in the operation's memory.
    // A fresh child must still copy the real, safe source exactly.
    auto control = std::make_shared<Completion>();
    source.store()->ferricCookieCopyExistingProbe(freshDestination.store(),
            [control](Result value) { control->receive(value); });
    ok &= check(waitFor([&] { return control->result.has_value(); })
                        && control->count == 1
                        && control->result->status == Status::ExactReadback
                        && control->result->sourceCount == 14
                        && control->result->destinationCount == 14,
                legacy ? "synthetic-legacy-refusal-left-real-source-unchanged"
                       : "synthetic-older-expiry-refusal-left-real-source-unchanged");
    return ok;
}

bool opaquePartitionRefusal()
{
    DisposableProfile source;
    DisposableProfile destination;
    DisposableProfile freshDestination;
    if (!check(source.initialize() && destination.initialize() && freshDestination.initialize(),
               "opaque-partition-disposable-profiles"))
        return false;
    auto first = std::make_shared<Completion>();
    source.store()->ferricCookieOpaquePartitionProbe(destination.store(),
            [first](Result value) { first->receive(value); });
    const bool seededAndRefused = check(waitFor([&] { return first->result.has_value(); })
                                            && first->count == 1
                                            && first->result->status == Status::UnsupportedRecord
                                            && first->result->sourceCount == 15
                                            && first->result->destinationCount == 0,
                                    "opaque-partition-source-refused-before-child-write");
    if (!seededAndRefused)
        return false;
    auto second = std::make_shared<Completion>();
    source.store()->ferricCookieCopyExistingProbe(freshDestination.store(),
            [second](Result value) { second->receive(value); });
    return check(waitFor([&] { return second->result.has_value(); })
                         && second->count == 1
                         && second->result->status == Status::UnsupportedRecord
                         && second->result->sourceCount == 15
                         && second->result->destinationCount == 0,
                 "opaque-partition-real-source-snapshot-refused-to-fresh-child");
}

bool midCopySetterRejection()
{
    DisposableProfile source;
    DisposableProfile failedDestination;
    DisposableProfile partialCopy;
    DisposableProfile sourceCopy;
    if (!check(source.initialize() && failedDestination.initialize()
                       && partialCopy.initialize() && sourceCopy.initialize(),
               "mid-copy-rejection-disposable-profiles"))
        return false;
    auto rejected = std::make_shared<Completion>();
    source.store()->ferricCookieMidCopyRejectProbe(failedDestination.store(),
            [rejected](Result value) { rejected->receive(value); });
    const bool sawRejection = check(waitFor([&] { return rejected->result.has_value(); })
                                            && rejected->count == 1
                                            && rejected->result->status == Status::MutationRejected
                                            && rejected->result->sourceCount == 14
                                            && rejected->result->destinationCount == -1,
                                    "mid-copy-rejected-child-count-unknown");
    if (!sawRejection)
        return false;
    // This copy is diagnostic only: the failed child is still disposable and
    // must never become a user-visible branch, even if its contents are known.
    auto partial = std::make_shared<Completion>();
    failedDestination.store()->ferricCookieCopyExistingProbe(partialCopy.store(),
            [partial](Result value) { partial->receive(value); });
    bool ok = check(waitFor([&] { return partial->result.has_value(); })
                            && partial->count == 1
                            && partial->result->status == Status::ExactReadback
                            && partial->result->sourceCount == 1
                            && partial->result->destinationCount == 1,
                    "mid-copy-rejection-left-partial-child");
    auto original = std::make_shared<Completion>();
    source.store()->ferricCookieCopyExistingProbe(sourceCopy.store(),
            [original](Result value) { original->receive(value); });
    ok &= check(waitFor([&] { return original->result.has_value(); })
                        && original->count == 1
                        && original->result->status == Status::ExactReadback
                        && original->result->sourceCount == 14
                        && original->result->destinationCount == 14,
                "mid-copy-rejection-left-source-complete");
    return ok;
}

bool ownerLoss(bool destroySource)
{
    auto source = std::make_unique<DisposableProfile>();
    auto destination = std::make_unique<DisposableProfile>();
    if (!check(source->initialize() && destination->initialize(), "owner-loss-profiles"))
        return false;
    auto state = std::make_shared<Completion>();
    source->store()->ferricCookieCloneProbe(destination->store(), false, [state](Result value) {
        state->receive(value);
    });
    if (destroySource)
        source.reset();
    else
        destination.reset();
    const bool completed = waitFor([&] { return state->result.has_value(); });
    // Drain late callbacks and deferred deletions; they must not complete again.
    QEventLoop drain;
    QTimer::singleShot(50, &drain, &QEventLoop::quit);
    drain.exec();
    return check(completed && state->result->status == Status::OwnerLost && state->count == 1,
                 destroySource ? "source-destroyed-refused-once" : "destination-destroyed-refused-once");
}

bool rtcUnnegotiatedObservation()
{
    DisposableProfile source;
    DisposableProfile control;
    QWebEnginePage sibling(source.profile.get());
    if (!check(source.initialize() && control.initialize() && loadBlank(&sibling),
               "rtc-unnegotiated-profiles"))
        return false;
    using RtcState = QWebEnginePage::FerricRtcProbeState;
    bool ok = check(source.page->ferricRtcProbeState() == RtcState::Unknown,
                    "rtc-unnegotiated-initially-unknown");
    auto created = std::make_shared<std::optional<bool>>();
    source.page->runJavaScript(QStringLiteral(R"JS(
        (() => {
            try {
                window.ferricUnnegotiated = new RTCPeerConnection({iceServers: []});
                return true;
            } catch { return false; }
        })()
    )JS"), [created](const QVariant &value) { *created = value.toBool(); });
    if (!check(waitFor([&] { return created->has_value(); }) && **created,
               "rtc-unnegotiated-constructed-without-offer-or-candidates"))
        return false;
    ok &= check(waitFor([&] {
        return source.page->ferricRtcProbeState() == RtcState::ObservedPeerConnection;
    }), "rtc-unnegotiated-native-observation");
    ok &= check(control.page->ferricRtcProbeState() == RtcState::Unknown,
                "rtc-unnegotiated-other-page-not-retargeted");
    ok &= check(sibling.ferricRtcProbeState() == RtcState::Unknown,
                "rtc-unnegotiated-same-profile-page-not-retargeted");
    // Destroy this entire fixture before creating the connected-pair fixture,
    // so a retained, closed handler cannot satisfy that later observation.
    return ok;
}

bool rtcObservation()
{
    DisposableProfile source;
    DisposableProfile control;
    QWebEnginePage sibling(source.profile.get());
    if (!check(source.initialize() && control.initialize() && loadBlank(&sibling),
               "rtc-fresh-otr-profiles"))
        return false;
    using RtcState = QWebEnginePage::FerricRtcProbeState;
    bool ok = check(source.page->ferricRtcProbeState() == RtcState::Unknown,
                    "rtc-zero-observations-is-unknown");
    auto permissionRequests = std::make_shared<int>(0);
    QObject::connect(source.page.get(), &QWebEnginePage::permissionRequested,
                     [permissionRequests](QWebEnginePermission permission) {
        ++*permissionRequests;
        permission.deny();
    });
    // No STUN/TURN server, remote signalling or external account. Two local
    // peers create a data-only channel; no SDP/candidates escape this renderer.
    source.page->runJavaScript(QStringLiteral(R"JS(
        window.ferricRtcReady = false;
        window.ferricRtcFailed = false;
        (async () => {
            // Non-trickle local signalling: wait for each complete description
            // before passing it to the other peer. This avoids racing candidate
            // delivery against setRemoteDescription in the fixture itself.
            const gathered = peer => new Promise((resolve, reject) => {
                if (peer.iceGatheringState === 'complete') { resolve(); return; }
                const changed = () => {
                    if (peer.iceGatheringState !== 'complete') return;
                    clearTimeout(timer);
                    peer.removeEventListener('icegatheringstatechange', changed);
                    resolve();
                };
                const timer = setTimeout(() => {
                    peer.removeEventListener('icegatheringstatechange', changed);
                    reject(new Error('local gathering timed out'));
                }, 8000);
                peer.addEventListener('icegatheringstatechange', changed);
            });
            window.ferricA = new RTCPeerConnection({iceServers: []});
            window.ferricB = new RTCPeerConnection({iceServers: []});
            ferricB.ondatachannel = e => {
                window.ferricRemoteChannel = e.channel;
                e.channel.onopen = () => { window.ferricRtcReady = true; };
            };
            window.ferricChannel = ferricA.createDataChannel('synthetic');
            await ferricA.setLocalDescription(await ferricA.createOffer());
            await gathered(ferricA);
            await ferricB.setRemoteDescription(ferricA.localDescription);
            await ferricB.setLocalDescription(await ferricB.createAnswer());
            await gathered(ferricB);
            await ferricA.setRemoteDescription(ferricB.localDescription);
        })().catch(() => { window.ferricRtcFailed = true; });
    )JS"));
    bool ready = false;
    QElapsedTimer deadline;
    deadline.start();
    while (!ready && deadline.elapsed() < 20000) {
        auto answer = std::make_shared<std::optional<int>>();
        source.page->runJavaScript(QStringLiteral(
                                      "window.ferricRtcFailed ? -1 : (window.ferricRtcReady ? 1 : 0)"),
                                  [answer](const QVariant &value) {
            *answer = value.isValid() ? value.toInt() : -1;
        });
        if (!waitFor([&] { return answer->has_value(); }))
            return check(false, "rtc-renderer-responsive");
        if (**answer < 0) {
            ok &= check(false, "rtc-local-signalling-completed");
            break;
        }
        ready = **answer == 1;
        QThread::msleep(20);
    }
    ok &= check(ready, "rtc-local-data-only-channel-connected");
    ok &= check(waitFor([&] {
        return source.page->ferricRtcProbeState() == RtcState::ObservedPeerConnection;
    }), "rtc-browser-observer-exact-page-positive");
    ok &= check(control.page->ferricRtcProbeState() == RtcState::Unknown,
                "rtc-other-page-not-retargeted");
    ok &= check(sibling.ferricRtcProbeState() == RtcState::Unknown,
                "rtc-same-profile-page-not-retargeted");
    ok &= check(*permissionRequests == 0, "rtc-data-only-without-media-permission");
    // Deliberately do not freeze. Observation is not race-free admission.
    return ok;
}
} // namespace

int main(int argc, char **argv)
{
    if (argc != 1 || qEnvironmentVariableIsSet("QTWEBENGINE_REMOTE_DEBUGGING")
            || qEnvironmentVariableIsSet("QTWEBENGINE_DISABLE_SANDBOX")
            || !qgetenv("QTWEBENGINE_CHROMIUM_FLAGS").isEmpty()) {
        std::cerr << "Refusing arguments or engine overrides; run through the isolated runner.\n";
        return 2;
    }
    QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
    QApplication app(argc, argv);
    app.setQuitOnLastWindowClosed(false);
    std::cout << std::unitbuf;
    std::cout << "ENGINE qt=" << qVersion() << " webengine=" << qWebEngineVersion()
              << " chromium=" << qWebEngineChromiumVersion()
              << " security_patch=" << qWebEngineChromiumSecurityPatchVersion() << '\n';
    bool ok = roundTrip(false);
    ok &= roundTrip(true);
    ok &= syntheticPreflightRefusal(Store::FerricSyntheticPreflightCase::LegacyUnknownSource);
    ok &= syntheticPreflightRefusal(Store::FerricSyntheticPreflightCase::OlderOverlongExpiry);
    ok &= opaquePartitionRefusal();
    ok &= midCopySetterRejection();
    ok &= ownerLoss(true);
    ok &= ownerLoss(false);
    ok &= rtcUnnegotiatedObservation();
    ok &= rtcObservation();
    std::cout << "PRODUCTION_CAPABILITY_ENABLED=false\n";
    return ok ? 0 : 1;
}
