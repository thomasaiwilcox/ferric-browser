#include "owned_https_fixture.h"
#include <QWebEngineNewWindowRequest>
#include <QWebEnginePermission>
#include <QWebEngineSettings>

#include <QCoreApplication>
#include <QFile>
#include <QFileInfo>

#include <csignal>
#include <cstddef>
#include <cstdio>
#include <limits>
#include <vector>

#if defined(Q_OS_LINUX)
#include <sys/syscall.h>
#include <sys/types.h>
#include <unistd.h>
#endif

namespace {
using State = QWebEnginePage::FerricRtcRegistryProbeState;
using Result = QWebEnginePage::FerricRtcRegistryProbeResult;
struct Completion {
    int count = 0;
    std::optional<Result> result;
};

std::shared_ptr<Completion> beginProbe(QWebEnginePage &page)
{
    auto result = std::make_shared<Completion>();
    page.ferricRtcFrozenRegistryProbe([result](Result value) {
        ++result->count;
        result->result = value;
    });
    return result;
}

bool expect(QWebEnginePage &page, State state, int frames, int minimumProcesses,
            const char *label)
{
    const auto completion = beginProbe(page);
    const bool received = waitFor([&] { return completion->result.has_value(); });
    const bool matches = received && completion->count == 1
            && completion->result->state == state && completion->result->frameCount == frames
            && completion->result->processCount >= minimumProcesses;
    if (received) {
        std::cout << "REGISTRY state=" << int(completion->result->state)
                  << " frames=" << completion->result->frameCount
                  << " processes=" << completion->result->processCount << '\n';
    }
    // Qt's property changes before the renderer has necessarily thawed. Await
    // an actual renderer task before reusing this page for another experiment.
    const bool restored = page.lifecycleState() == QWebEnginePage::LifecycleState::Active
            && scriptBoolean(page, QStringLiteral("true"));
    return check(matches && restored, label);
}

#if defined(Q_OS_LINUX) && defined(SYS_pidfd_open) && defined(SYS_pidfd_send_signal)
bool probeRendererIdentity(pid_t pid, QByteArray *startTime, QString *failure)
{
    auto reject = [failure](const QString &reason) {
        if (failure)
            *failure = reason;
        return false;
    };
    const auto expectedExecutable = QFileInfo(
                                            QString::fromLocal8Bit(qgetenv("QTWEBENGINEPROCESS_PATH")))
                                            .canonicalFilePath();
    if (expectedExecutable.isEmpty())
        return reject(QStringLiteral("isolated QtWebEngineProcess path is unavailable"));

    // Chromium may insert up to two QtWebEngineProcess helpers between the
    // renderer and the Qt fixture, and it sanitizes argv after launch. The PID comes from
    // QWebEnginePage::renderProcessPid(); verify that exact target and every
    // bounded ancestor against the isolated executable and a stable start time.
    QByteArray ancestry;
    pid_t currentPid = pid;
    int engineProcessCount = 0;
    const auto fixturePid = static_cast<pid_t>(QCoreApplication::applicationPid());
    while (currentPid != fixturePid) {
        if (engineProcessCount >= 3)
            return reject(QStringLiteral("renderer ancestry exceeded two intermediate Qt processes"));

        const auto procRoot = QStringLiteral("/proc/%1/").arg(currentPid);
        QFile stat(procRoot + QStringLiteral("stat"));
        if (!stat.open(QIODevice::ReadOnly))
            return reject(QStringLiteral("renderer ancestry stat is unavailable"));
        const auto statBytes = stat.readAll();
        const auto nameEnd = statBytes.lastIndexOf(')');
        if (nameEnd < 0)
            return reject(QStringLiteral("renderer ancestry stat is malformed"));
        const auto fields = statBytes.mid(nameEnd + 2).simplified().split(' ');
        if (fields.size() <= 19 || fields.at(19).isEmpty())
            return reject(QStringLiteral("renderer ancestry start identity is unavailable"));

        QFile status(procRoot + QStringLiteral("status"));
        if (!status.open(QIODevice::ReadOnly))
            return reject(QStringLiteral("renderer ancestry status is unavailable"));
        qint64 parentPid = -1;
        for (const auto &line : status.readAll().split('\n')) {
            if (line.startsWith(QByteArrayLiteral("PPid:"))) {
                bool parsed = false;
                parentPid = line.mid(5).trimmed().toLongLong(&parsed);
                if (!parsed)
                    return reject(QStringLiteral("renderer ancestry parent is malformed"));
                break;
            }
        }
        if (parentPid <= 1 || parentPid > std::numeric_limits<pid_t>::max())
            return reject(QStringLiteral("renderer ancestry is not rooted in the fixture process"));

        const auto executable = QFileInfo(procRoot + QStringLiteral("exe")).canonicalFilePath();
        if (executable != expectedExecutable)
            return reject(QStringLiteral("renderer ancestry escaped the isolated QtWebEngineProcess"));

        ancestry += QByteArray::number(currentPid) + ':' + fields.at(19) + ':'
                + QByteArray::number(parentPid) + ';';
        currentPid = static_cast<pid_t>(parentPid);
        ++engineProcessCount;
    }
    *startTime = ancestry;
    return true;
}

bool terminateExactProbeRenderer(qint64 processId, QString *failure)
{
    if (processId <= 1 || processId > std::numeric_limits<pid_t>::max())
        return failure ? (*failure = QStringLiteral("Qt reported an invalid renderer PID"), false)
                       : false;
    const auto pid = static_cast<pid_t>(processId);
    QByteArray startBefore;
    if (!probeRendererIdentity(pid, &startBefore, failure))
        return false;

    const int pidFd = static_cast<int>(syscall(SYS_pidfd_open, pid, 0));
    if (pidFd < 0) {
        if (failure)
            *failure = QStringLiteral("pidfd_open rejected the verified renderer");
        return false;
    }
    QByteArray startAfter;
    QString secondCheckFailure;
    const bool sameRenderer = probeRendererIdentity(pid, &startAfter, &secondCheckFailure)
            && startBefore == startAfter;
    const bool signalled = sameRenderer && syscall(SYS_pidfd_send_signal, pidFd, SIGKILL,
                                                   nullptr, 0) == 0;
    if (!sameRenderer && failure)
        *failure = secondCheckFailure.isEmpty()
                ? QStringLiteral("renderer process identity changed after pidfd_open")
                : secondCheckFailure;
    else if (!signalled && failure)
        *failure = QStringLiteral("pidfd_send_signal rejected the verified renderer");
    close(pidFd);
    return signalled;
}
#endif
} // namespace

int main(int argc, char **argv)
{
    const QByteArray chromiumFlags = qgetenv("QTWEBENGINE_CHROMIUM_FLAGS");
    const QByteArray normalRtcFlags = "--use-fake-device-for-media-stream";
    const QByteArray strictRtcFlags = normalRtcFlags + " --site-per-process";
    const bool strictIsolation = chromiumFlags == strictRtcFlags;
    if (argc != 3 || qEnvironmentVariableIsSet("QTWEBENGINE_REMOTE_DEBUGGING")
            || qEnvironmentVariableIsSet("QTWEBENGINE_DISABLE_SANDBOX")
            || (chromiumFlags != normalRtcFlags && !strictIsolation))
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
    std::cout << "STRICT_SITE_ISOLATION=" << (strictIsolation ? "true" : "false") << '\n';
    Fixture fixture;
    if (!check(fixture.start(certificatePath, keyPath), "rtc-owned-loopback-tls-server"))
        return 1;
    QWebEngineProfile profile;
    if (!check(profile.isOffTheRecord(), "rtc-disposable-otr-profile"))
        return 1;
    const auto url = [&](const QString &path) { return fixture.url(QStringLiteral("cookie.localhost"), path); };
    bool ok = true;
    {
        QWebEnginePage page(&profile);
        fixture.pin(page);
        if (!check(navigate(page, url(QStringLiteral("/rtc/empty"))), "rtc-media-admission-page-loaded"))
            return 1;
        std::optional<QWebEnginePermission> audioPermission;
        int audioPermissionCount = 0;
        const auto permissionConnection = QObject::connect(
                &page, &QWebEnginePage::permissionRequested, &page,
                [&](QWebEnginePermission permission) {
            if (permission.permissionType() == QWebEnginePermission::PermissionType::MediaAudioCapture) {
                ++audioPermissionCount;
                audioPermission = std::move(permission);
            }
        });
        bool scriptDispatchReturned = false;
        page.runJavaScript(QStringLiteral(
                "navigator.mediaDevices.getUserMedia({audio:true}).then(stream => {"
                "window.ferricAudioStream=stream; window.ferricAudioStarted=true;"
                "}).catch(error => { window.ferricAudioError=String(error); }); true"),
                [&](const QVariant &value) { scriptDispatchReturned = value.isValid() && value.toBool(); });
        if (!check(waitFor([&] { return audioPermissionCount == 1 && audioPermission.has_value(); })
                           && scriptDispatchReturned && audioPermission->isValid()
                           && audioPermission->state() == QWebEnginePermission::State::Ask,
                   "rtc-media-permission-held-before-stream-start"))
            return 1;

        using LeaseResult = QWebEnginePage::FerricRtcFrozenLeaseProbeResult;
        std::optional<LeaseResult> pendingResult;
        page.ferricRtcAcquireFrozenRegistryLeaseProbe(
                [&](LeaseResult result) { pendingResult = std::move(result); }, [] {});
        ok &= check(pendingResult.has_value()
                            && pendingResult->result.state == State::Unknown
                            && !pendingResult->scopeStillHeld && !pendingResult->release
                            && page.lifecycleState() == QWebEnginePage::LifecycleState::Active,
                    "rtc-pending-media-permission-refuses-absence-lease");

        audioPermission->grant();
        if (!check(waitFor([&] { return scriptBoolean(page, QStringLiteral("window.ferricAudioStarted === true")); }),
                   "rtc-fake-audio-stream-started-after-permission"))
            return 1;

        std::optional<LeaseResult> activeResult;
        page.ferricRtcAcquireFrozenRegistryLeaseProbe(
                [&](LeaseResult result) { activeResult = std::move(result); }, [] {});
        ok &= check(activeResult.has_value() && activeResult->result.state == State::Unknown
                            && !activeResult->scopeStillHeld && !activeResult->release,
                    "rtc-active-media-stream-refuses-absence-lease");

        const bool stopRequested = scriptBoolean(page, QStringLiteral(
                "window.ferricAudioStream.getTracks().forEach(track => track.stop()); true"));
        std::optional<LeaseResult> cleanupResult;
        bool cleanupProbeInFlight = false;
        const bool mediaReleaseObserved = stopRequested && waitFor([&] {
            if (!cleanupProbeInFlight && !cleanupResult) {
                cleanupProbeInFlight = true;
                page.ferricRtcAcquireFrozenRegistryLeaseProbe(
                        [&](LeaseResult result) {
                    cleanupResult = std::move(result);
                    cleanupProbeInFlight = false;
                }, [] {});
            }
            if (!cleanupResult)
                return false;
            if (cleanupResult->result.state == State::EmptyRegistriesAfterFreeze) {
                if (cleanupResult->release)
                    cleanupResult->release();
                return true;
            }
            cleanupResult.reset();
            return false;
        });
        ok &= check(mediaReleaseObserved, "rtc-stopped-media-releases-activity-interlock");
        QObject::disconnect(permissionConnection);
    }
    {
        QWebEngineProfile denialProfile;
        if (!check(denialProfile.isOffTheRecord(), "rtc-denial-disposable-otr-profile"))
            return 1;
        QWebEnginePage page(&denialProfile);
        fixture.pin(page);
        if (!check(navigate(page, url(QStringLiteral("/rtc/empty"))),
                   "rtc-denial-media-admission-page-loaded"))
            return 1;

        std::optional<QWebEnginePermission> audioPermission;
        int audioPermissionCount = 0;
        const auto permissionConnection = QObject::connect(
                &page, &QWebEnginePage::permissionRequested, &page,
                [&](QWebEnginePermission permission) {
            if (permission.permissionType() == QWebEnginePermission::PermissionType::MediaAudioCapture) {
                ++audioPermissionCount;
                audioPermission = std::move(permission);
            }
        });
        bool scriptDispatchReturned = false;
        page.runJavaScript(QStringLiteral(
                "navigator.mediaDevices.getUserMedia({audio:true}).then(stream => {"
                "window.ferricDeniedStreamStarted=true;"
                "stream.getTracks().forEach(track => track.stop());"
                "}).catch(error => { window.ferricDeniedError=String(error); }); true"),
                [&](const QVariant &value) { scriptDispatchReturned = value.isValid() && value.toBool(); });
        if (!check(waitFor([&] { return audioPermissionCount == 1 && audioPermission.has_value(); })
                           && scriptDispatchReturned && audioPermission->isValid()
                           && audioPermission->state() == QWebEnginePermission::State::Ask,
                   "rtc-denial-media-permission-held-before-decision"))
            return 1;

        using LeaseResult = QWebEnginePage::FerricRtcFrozenLeaseProbeResult;
        std::optional<LeaseResult> pendingResult;
        page.ferricRtcAcquireFrozenRegistryLeaseProbe(
                [&](LeaseResult result) { pendingResult = std::move(result); }, [] {});
        ok &= check(pendingResult.has_value()
                            && pendingResult->result.state == State::Unknown
                            && !pendingResult->scopeStillHeld && !pendingResult->release,
                    "rtc-denial-pending-permission-refuses-absence-lease");

        audioPermission->deny();
        const bool promiseRejected = waitFor([&] {
            return scriptBoolean(page, QStringLiteral(
                    "typeof window.ferricDeniedError === 'string' && window.ferricDeniedError.length > 0"));
        });
        std::optional<LeaseResult> cleanupResult;
        bool cleanupProbeInFlight = false;
        const bool denialReleaseObserved = promiseRejected && waitFor([&] {
            if (!cleanupProbeInFlight && !cleanupResult) {
                cleanupProbeInFlight = true;
                page.ferricRtcAcquireFrozenRegistryLeaseProbe(
                        [&](LeaseResult result) {
                    cleanupResult = std::move(result);
                    cleanupProbeInFlight = false;
                }, [] {});
            }
            if (!cleanupResult)
                return false;
            if (cleanupResult->result.state == State::EmptyRegistriesAfterFreeze) {
                if (cleanupResult->release)
                    cleanupResult->release();
                return true;
            }
            cleanupResult.reset();
            return false;
        });
        ok &= check(denialReleaseObserved && page.lifecycleState() == QWebEnginePage::LifecycleState::Active,
                    "rtc-denied-media-permission-releases-activity-token");
        QObject::disconnect(permissionConnection);
    }
    {
        QWebEnginePage page(&profile);
        fixture.pin(page);
        if (!check(navigate(page, url(QStringLiteral("/rtc/empty"))),
                   "rtc-fake-combined-media-page-loaded"))
            return 1;

        std::optional<QWebEnginePermission> combinedPermission;
        int combinedPermissionCount = 0;
        int separateMediaPermissionCount = 0;
        const auto permissionConnection = QObject::connect(
                &page, &QWebEnginePage::permissionRequested, &page,
                [&](QWebEnginePermission permission) {
            switch (permission.permissionType()) {
            case QWebEnginePermission::PermissionType::MediaAudioVideoCapture:
                ++combinedPermissionCount;
                combinedPermission = std::move(permission);
                break;
            case QWebEnginePermission::PermissionType::MediaAudioCapture:
            case QWebEnginePermission::PermissionType::MediaVideoCapture:
                ++separateMediaPermissionCount;
                permission.deny();
                break;
            default:
                break;
            }
        });

        bool scriptDispatchReturned = false;
        page.runJavaScript(QStringLiteral(
                "navigator.mediaDevices.getUserMedia({audio:true,video:true})"
                ".then(stream=>{window.ferricCombinedStream=stream;"
                "window.ferricCombinedAudio=stream.getAudioTracks()[0];"
                "window.ferricCombinedVideo=stream.getVideoTracks()[0];"
                "try{const processor=new MediaStreamTrackProcessor({track:window.ferricCombinedVideo});"
                "const reader=processor.readable.getReader();"
                "reader.read().then(({value,done})=>{if(!done&&value){"
                "window.ferricCombinedFrameWidth=value.displayWidth;"
                "window.ferricCombinedFrameHeight=value.displayHeight;"
                "window.ferricCombinedFrame=window.ferricCombinedFrameWidth>0&&"
                "window.ferricCombinedFrameHeight>0;value.close();}});"
                "}catch(e){window.ferricCombinedFrameError=String(e); }"
                "window.ferricCombinedStarted=!!window.ferricCombinedAudio&&"
                "!!window.ferricCombinedVideo&&window.ferricCombinedAudio.kind==='audio'&&"
                "window.ferricCombinedVideo.kind==='video'&&"
                "window.ferricCombinedAudio.readyState==='live'&&"
                "window.ferricCombinedVideo.readyState==='live';"
                "}).catch(error=>{window.ferricCombinedError=String(error);});true"),
                [&](const QVariant &value) {
            scriptDispatchReturned = value.isValid() && value.toBool();
        });

        const bool permissionHeld = waitFor([&] {
            return scriptDispatchReturned && combinedPermissionCount == 1
                    && combinedPermission.has_value();
        });
        if (!check(permissionHeld && separateMediaPermissionCount == 0
                           && combinedPermission->isValid()
                           && combinedPermission->state() == QWebEnginePermission::State::Ask,
                   "rtc-fake-combined-media-permission-held"))
            return 1;

        using LeaseResult = QWebEnginePage::FerricRtcFrozenLeaseProbeResult;
        std::optional<LeaseResult> pendingResult;
        page.ferricRtcAcquireFrozenRegistryLeaseProbe(
                [&](LeaseResult result) { pendingResult = std::move(result); }, [] {});
        ok &= check(pendingResult.has_value()
                            && pendingResult->result.state == State::Unknown
                            && !pendingResult->scopeStillHeld && !pendingResult->release
                            && page.lifecycleState() == QWebEnginePage::LifecycleState::Active,
                    "rtc-pending-combined-media-permission-refuses-absence-lease");

        combinedPermission->grant();
        const bool captureStarted = waitFor([&] {
            return scriptBoolean(page, QStringLiteral(
                    "window.ferricCombinedStarted===true&&"
                    "window.ferricCombinedAudio.readyState==='live'&&"
                    "window.ferricCombinedVideo.readyState==='live'"));
        });
        if (!check(captureStarted, "rtc-fake-combined-audio-video-capture-started"))
            return 1;
        ok &= check(combinedPermissionCount == 1 && separateMediaPermissionCount == 0,
                    "rtc-combined-media-uses-one-combined-permission");
        ok &= check(waitFor([&] {
                    return scriptBoolean(page, QStringLiteral(
                            "window.ferricCombinedFrame===true"));
                }), "rtc-fake-combined-video-frame-delivered");

        std::optional<LeaseResult> activeResult;
        page.ferricRtcAcquireFrozenRegistryLeaseProbe(
                [&](LeaseResult result) { activeResult = std::move(result); }, [] {});
        ok &= check(activeResult.has_value() && activeResult->result.state == State::Unknown
                            && !activeResult->scopeStillHeld && !activeResult->release,
                    "rtc-live-combined-audio-video-refuses-absence-lease");

        const bool stopRequested = scriptBoolean(page, QStringLiteral(
                "window.ferricCombinedStream.getTracks().forEach(track=>track.stop());true"));
        std::optional<LeaseResult> cleanupResult;
        bool cleanupProbeInFlight = false;
        const bool captureReleaseObserved = stopRequested && waitFor([&] {
            if (!cleanupProbeInFlight && !cleanupResult) {
                cleanupProbeInFlight = true;
                page.ferricRtcAcquireFrozenRegistryLeaseProbe(
                        [&](LeaseResult result) {
                    cleanupResult = std::move(result);
                    cleanupProbeInFlight = false;
                }, [] {});
            }
            if (!cleanupResult)
                return false;
            if (cleanupResult->result.state == State::EmptyRegistriesAfterFreeze) {
                if (cleanupResult->release)
                    cleanupResult->release();
                return true;
            }
            cleanupResult.reset();
            return false;
        });
        ok &= check(captureReleaseObserved,
                    "rtc-stopped-combined-audio-video-releases-activity-interlock");
        QObject::disconnect(permissionConnection);
    }
    {
        QWebEngineProfile denialProfile;
        if (!check(denialProfile.isOffTheRecord(), "rtc-combined-denial-disposable-otr-profile"))
            return 1;
        QWebEnginePage page(&denialProfile);
        fixture.pin(page);
        if (!check(navigate(page, url(QStringLiteral("/rtc/empty"))),
                   "rtc-combined-denial-page-loaded"))
            return 1;

        std::optional<QWebEnginePermission> combinedPermission;
        int combinedPermissionCount = 0;
        int separatePermissionCount = 0;
        const auto permissionConnection = QObject::connect(
                &page, &QWebEnginePage::permissionRequested, &page,
                [&](QWebEnginePermission permission) {
            switch (permission.permissionType()) {
            case QWebEnginePermission::PermissionType::MediaAudioVideoCapture:
                ++combinedPermissionCount;
                combinedPermission = std::move(permission);
                break;
            case QWebEnginePermission::PermissionType::MediaAudioCapture:
            case QWebEnginePermission::PermissionType::MediaVideoCapture:
                ++separatePermissionCount;
                permission.deny();
                break;
            default:
                break;
            }
        });
        bool scriptDispatchReturned = false;
        page.runJavaScript(QStringLiteral(
                "navigator.mediaDevices.getUserMedia({audio:true,video:true})"
                ".then(stream=>{window.ferricCombinedDeniedStarted=true;"
                "stream.getTracks().forEach(track=>track.stop());"
                "}).catch(error=>{window.ferricCombinedDeniedError=String(error);});true"),
                [&](const QVariant &value) {
            scriptDispatchReturned = value.isValid() && value.toBool();
        });
        if (!check(waitFor([&] {
                        return scriptDispatchReturned && combinedPermissionCount == 1
                                && combinedPermission.has_value();
                    })
                           && separatePermissionCount == 0
                           && combinedPermission->isValid()
                           && combinedPermission->state() == QWebEnginePermission::State::Ask,
                   "rtc-combined-denial-permission-held"))
            return 1;

        using LeaseResult = QWebEnginePage::FerricRtcFrozenLeaseProbeResult;
        std::optional<LeaseResult> pendingResult;
        page.ferricRtcAcquireFrozenRegistryLeaseProbe(
                [&](LeaseResult result) { pendingResult = std::move(result); }, [] {});
        ok &= check(pendingResult.has_value()
                            && pendingResult->result.state == State::Unknown
                            && !pendingResult->scopeStillHeld && !pendingResult->release,
                    "rtc-pending-combined-denial-refuses-absence-lease");

        combinedPermission->deny();
        const bool promiseRejected = waitFor([&] {
            return scriptBoolean(page, QStringLiteral(
                    "typeof window.ferricCombinedDeniedError==='string'&&"
                    "window.ferricCombinedDeniedError.length>0"));
        });
        std::optional<LeaseResult> cleanupResult;
        bool cleanupProbeInFlight = false;
        const bool denialReleaseObserved = promiseRejected && waitFor([&] {
            if (!cleanupProbeInFlight && !cleanupResult) {
                cleanupProbeInFlight = true;
                page.ferricRtcAcquireFrozenRegistryLeaseProbe(
                        [&](LeaseResult result) {
                    cleanupResult = std::move(result);
                    cleanupProbeInFlight = false;
                }, [] {});
            }
            if (!cleanupResult)
                return false;
            if (cleanupResult->result.state == State::EmptyRegistriesAfterFreeze) {
                if (cleanupResult->release)
                    cleanupResult->release();
                return true;
            }
            cleanupResult.reset();
            return false;
        });
        ok &= check(denialReleaseObserved && separatePermissionCount == 0
                            && page.lifecycleState() == QWebEnginePage::LifecycleState::Active,
                    "rtc-denied-combined-media-releases-activity-interlock");
        QObject::disconnect(permissionConnection);
    }
    {
        QWebEnginePage page(&profile);
        fixture.pin(page);
        page.settings()->setAttribute(QWebEngineSettings::ScreenCaptureEnabled, true);
        if (!check(navigate(page, url(QStringLiteral("/rtc/empty"))),
                   "rtc-preselected-desktop-page-loaded"))
            return 1;
        using LeaseResult = QWebEnginePage::FerricRtcFrozenLeaseProbeResult;
        std::optional<LeaseResult> held;
        int invalidations = 0;
        page.ferricRtcAcquireFrozenRegistryLeaseProbe(
                [&](LeaseResult result) { held = std::move(result); },
                [&] { ++invalidations; });
        if (!check(waitFor([&] { return held.has_value(); })
                           && held->result.state == State::EmptyRegistriesAfterFreeze
                           && held->scopeStillHeld && held->scopeStillHeld(),
                   "rtc-preselected-desktop-test-holds-empty-lease"))
            return 1;
        const bool invalidIdRefused = page.ferricPreselectedDesktopAdmissionProbe();
        ok &= check(invalidIdRefused && invalidations == 1 && !held->scopeStillHeld()
                            && page.lifecycleState() == QWebEnginePage::LifecycleState::Active,
                    "rtc-preselected-desktop-fast-path-invalidates-before-dispatch");
        if (held->release)
            held->release();
    }
    {
        QWebEnginePage page(&profile);
        fixture.pin(page);
        page.settings()->setAttribute(QWebEngineSettings::ScreenCaptureEnabled, true);
        if (!check(navigate(page, url(QStringLiteral("/rtc/empty"))),
                   "rtc-fake-desktop-page-loaded"))
            return 1;
        using LeaseResult = QWebEnginePage::FerricRtcFrozenLeaseProbeResult;
        bool scriptDispatchReturned = false;
        page.runJavaScript(QStringLiteral(
                "navigator.mediaDevices.getUserMedia({audio:false,video:{mandatory:{"
                "chromeMediaSource:'desktop',chromeMediaSourceId:'screen:-3:0'}}})"
                ".then(stream=>{window.ferricDesktopStream=stream;"
                "window.ferricDesktopTrack=stream.getVideoTracks()[0];"
                "try{const processor=new MediaStreamTrackProcessor({track:window.ferricDesktopTrack});"
                "const reader=processor.readable.getReader();"
                "reader.read().then(({value,done})=>{if(!done&&value){"
                "window.ferricDesktopFrameWidth=value.displayWidth;"
                "window.ferricDesktopFrameHeight=value.displayHeight;"
                "window.ferricDesktopFrame=window.ferricDesktopFrameWidth>0&&"
                "window.ferricDesktopFrameHeight>0;value.close();}});"
                "}catch(e){window.ferricDesktopFrameError=String(e); }"
                "window.ferricDesktopStarted=!!window.ferricDesktopTrack&&"
                "window.ferricDesktopTrack.readyState==='live';"
                "}).catch(()=>{window.ferricDesktopError=true;});true"),
                [&](const QVariant &value) { scriptDispatchReturned = value.isValid() && value.toBool(); });
        const bool captureStarted = waitFor([&] {
            return scriptDispatchReturned && scriptBoolean(page, QStringLiteral(
                    "window.ferricDesktopStarted===true&&window.ferricDesktopTrack.readyState==='live'"));
        });
        if (!check(captureStarted, "rtc-fake-desktop-capture-device-started"))
            return 1;
        ok &= check(waitFor([&] {
                    return scriptBoolean(page, QStringLiteral("window.ferricDesktopFrame===true"));
                }), "rtc-fake-desktop-synthetic-frame-delivered");

        std::optional<LeaseResult> activeResult;
        page.ferricRtcAcquireFrozenRegistryLeaseProbe(
                [&](LeaseResult result) { activeResult = std::move(result); }, [] {});
        ok &= check(activeResult.has_value() && activeResult->result.state == State::Unknown
                            && !activeResult->scopeStillHeld && !activeResult->release,
                    "rtc-live-fake-desktop-capture-refuses-absence-lease");

        const bool stopRequested = scriptBoolean(page, QStringLiteral(
                "window.ferricDesktopTrack.stop();true"));
        std::optional<LeaseResult> cleanupResult;
        bool cleanupProbeInFlight = false;
        const bool captureReleaseObserved = stopRequested && waitFor([&] {
            if (!cleanupProbeInFlight && !cleanupResult) {
                cleanupProbeInFlight = true;
                page.ferricRtcAcquireFrozenRegistryLeaseProbe(
                        [&](LeaseResult result) {
                    cleanupResult = std::move(result);
                    cleanupProbeInFlight = false;
                }, [] {});
            }
            if (!cleanupResult)
                return false;
            if (cleanupResult->result.state == State::EmptyRegistriesAfterFreeze) {
                if (cleanupResult->release)
                    cleanupResult->release();
                return true;
            }
            cleanupResult.reset();
            return false;
        });
        ok &= check(captureReleaseObserved, "rtc-stopped-fake-desktop-releases-activity-interlock");
    }
    {
        QWebEnginePage page(&profile);
        fixture.pin(page);
        if (!check(navigate(page, url(QStringLiteral("/rtc/empty"))),
                   "rtc-fake-camera-page-loaded"))
            return 1;

        std::vector<QWebEnginePermission> mediaPermissions;
        int videoPermissionCount = 0;
        int combinedPermissionCount = 0;
        bool permissionOverflow = false;
        bool grantNewPermissions = false;
        const auto permissionConnection = QObject::connect(
                &page, &QWebEnginePage::permissionRequested, &page,
                [&](QWebEnginePermission permission) {
            switch (permission.permissionType()) {
            case QWebEnginePermission::PermissionType::MediaVideoCapture:
                ++videoPermissionCount;
                break;
            case QWebEnginePermission::PermissionType::MediaAudioVideoCapture:
                ++combinedPermissionCount;
                break;
            default:
                return;
            }
            if (mediaPermissions.size() >= 2) {
                permissionOverflow = true;
                permission.deny();
                return;
            }
            mediaPermissions.push_back(std::move(permission));
            if (grantNewPermissions)
                mediaPermissions.back().grant();
        });

        bool scriptDispatchReturned = false;
        page.runJavaScript(QStringLiteral(
                "navigator.mediaDevices.getUserMedia({audio:false,video:true})"
                ".then(stream=>{window.ferricCameraStream=stream;"
                "window.ferricCameraTrack=stream.getVideoTracks()[0];"
                "try{const processor=new MediaStreamTrackProcessor({track:window.ferricCameraTrack});"
                "const reader=processor.readable.getReader();"
                "reader.read().then(({value,done})=>{if(!done&&value){"
                "window.ferricCameraFrameWidth=value.displayWidth;"
                "window.ferricCameraFrameHeight=value.displayHeight;"
                "window.ferricCameraFrame=window.ferricCameraFrameWidth>0&&"
                "window.ferricCameraFrameHeight>0;value.close();}});"
                "}catch(e){window.ferricCameraFrameError=String(e);}"
                "window.ferricCameraStarted=!!window.ferricCameraTrack&&"
                "window.ferricCameraTrack.kind==='video'&&"
                "window.ferricCameraTrack.readyState==='live';"
                "}).catch(error=>{window.ferricCameraError=String(error);});true"),
                [&](const QVariant &value) {
            scriptDispatchReturned = value.isValid() && value.toBool();
        });

        const bool cameraPermissionHeld = waitFor([&] {
            return scriptDispatchReturned && !mediaPermissions.empty();
        });
        if (!check(cameraPermissionHeld
                           && !permissionOverflow
                           && (videoPermissionCount + combinedPermissionCount) > 0
                           && mediaPermissions.front().isValid()
                           && mediaPermissions.front().state() == QWebEnginePermission::State::Ask,
                   "rtc-fake-camera-permission-pending"))
            return 1;

        using LeaseResult = QWebEnginePage::FerricRtcFrozenLeaseProbeResult;
        std::optional<LeaseResult> pendingResult;
        page.ferricRtcAcquireFrozenRegistryLeaseProbe(
                [&](LeaseResult result) { pendingResult = std::move(result); }, [] {});
        ok &= check(pendingResult.has_value()
                            && pendingResult->result.state == State::Unknown
                            && !pendingResult->scopeStillHeld && !pendingResult->release
                            && page.lifecycleState() == QWebEnginePage::LifecycleState::Active,
                    "rtc-pending-camera-permission-refuses-absence-lease");

        grantNewPermissions = true;
        for (std::size_t index = 0; index < mediaPermissions.size(); ++index) {
            // A grant can synchronously deliver another permission callback;
            // copy the small Qt handle before granting so vector growth cannot
            // invalidate the object used by the call.
            const QWebEnginePermission permission = mediaPermissions[index];
            permission.grant();
        }
        const bool cameraStarted = waitFor([&] {
            return scriptBoolean(page, QStringLiteral(
                    "window.ferricCameraStarted===true&&"
                    "window.ferricCameraTrack.readyState==='live'"));
        });
        if (!check(cameraStarted, "rtc-fake-camera-track-started"))
            return 1;
        ok &= check(waitFor([&] {
                    return scriptBoolean(page, QStringLiteral(
                            "window.ferricCameraFrame===true"));
                }), "rtc-fake-camera-synthetic-frame-delivered");

        std::optional<LeaseResult> activeResult;
        page.ferricRtcAcquireFrozenRegistryLeaseProbe(
                [&](LeaseResult result) { activeResult = std::move(result); }, [] {});
        ok &= check(activeResult.has_value() && activeResult->result.state == State::Unknown
                            && !activeResult->scopeStillHeld && !activeResult->release,
                    "rtc-live-fake-camera-refuses-absence-lease");

        const bool stopRequested = scriptBoolean(page, QStringLiteral(
                "window.ferricCameraTrack.stop();true"));
        std::optional<LeaseResult> cleanupResult;
        bool cleanupProbeInFlight = false;
        const bool cameraReleaseObserved = stopRequested && waitFor([&] {
            if (!cleanupProbeInFlight && !cleanupResult) {
                cleanupProbeInFlight = true;
                page.ferricRtcAcquireFrozenRegistryLeaseProbe(
                        [&](LeaseResult result) {
                    cleanupResult = std::move(result);
                    cleanupProbeInFlight = false;
                }, [] {});
            }
            if (!cleanupResult)
                return false;
            if (cleanupResult->result.state == State::EmptyRegistriesAfterFreeze) {
                if (cleanupResult->release)
                    cleanupResult->release();
                return true;
            }
            cleanupResult.reset();
            return false;
        });
        ok &= check(cameraReleaseObserved,
                    "rtc-stopped-fake-camera-releases-activity-interlock");
        QObject::disconnect(permissionConnection);
    }
    {
        QWebEnginePage page(&profile);
        fixture.pin(page);
        page.settings()->setAttribute(QWebEngineSettings::ScreenCaptureEnabled, true);
        if (!check(navigate(page, url(QStringLiteral("/rtc/empty"))),
                   "rtc-simulated-desktop-handoff-page-loaded"))
            return 1;
        using LeaseResult = QWebEnginePage::FerricRtcFrozenLeaseProbeResult;
        std::optional<LeaseResult> held;
        int invalidations = 0;
        page.ferricRtcAcquireFrozenRegistryLeaseProbe(
                [&](LeaseResult result) { held = std::move(result); },
                [&] { ++invalidations; });
        if (!check(waitFor([&] { return held.has_value(); })
                           && held->result.state == State::EmptyRegistriesAfterFreeze
                           && held->scopeStillHeld && held->scopeStillHeld(),
                   "rtc-simulated-desktop-test-holds-empty-lease"))
            return 1;
        const bool handoffObserved = page.ferricPreselectedFakeDesktopActivityHandoffProbe();
        ok &= check(handoffObserved && invalidations == 1 && !held->scopeStillHeld()
                            && page.lifecycleState() == QWebEnginePage::LifecycleState::Active,
                    "rtc-simulated-handoff-and-counter-boundaries-refuse");
        if (held->release)
            held->release();
    }
    {
        QWebEnginePage page(&profile);
        fixture.pin(page);
        if (!check(navigate(page, url(QStringLiteral("/rtc/empty"))), "rtc-empty-page-loaded"))
            return 1;
        ok &= expect(page, State::EmptyRegistriesAfterFreeze, 1, 1, "rtc-empty-native-registry-after-freeze-and-thaw");
        if (!check(scriptBoolean(page, QStringLiteral(
                    "window.peer = new RTCPeerConnection({iceServers: []}); peer.signalingState === 'stable'")),
                   "rtc-new-unnegotiated-peer-created"))
            return 1;
        ok &= expect(page, State::RegisteredAfterFreeze, 1, 1, "rtc-unnegotiated-peer-native-registry-positive");
        if (!check(scriptBoolean(page, QStringLiteral("peer.close(); peer.signalingState === 'closed'")),
                   "rtc-peer-closed-but-retained"))
            return 1;
        ok &= expect(page, State::RegisteredAfterFreeze, 1, 1, "rtc-closed-retained-peer-conservatively-positive");
    }
    for (const bool microtask : {false, true}) {
        QWebEnginePage page(&profile);
        fixture.pin(page);
        if (!check(navigate(page, url(QStringLiteral("/rtc/empty"))), "rtc-freeze-handler-page-load"))
            return 1;
        const QString body = microtask
                ? QStringLiteral("queueMicrotask(() => { window.peer = new RTCPeerConnection({iceServers: []}); });")
                : QStringLiteral("window.peer = new RTCPeerConnection({iceServers: []});");
        if (!scriptBoolean(page, QStringLiteral(
                    "document.addEventListener('freeze', () => { %1 }, {once:true}); true").arg(body)))
            return 1;
        ok &= expect(page, State::RegisteredAfterFreeze, 1, 1,
                     microtask ? "rtc-freeze-handler-microtask-peer-observed" : "rtc-freeze-handler-peer-observed");
        ok &= check(scriptBoolean(page, QStringLiteral("!!window.peer")),
                    microtask ? "rtc-microtask-fixture-actually-created-peer" : "rtc-freeze-fixture-actually-created-peer");
    }
    for (const QString &path : {QStringLiteral("/rtc/same-peer"), QStringLiteral("/rtc/cross-peer"),
                               QStringLiteral("/rtc/cross-empty")}) {
        QWebEnginePage page(&profile);
        fixture.pin(page);
        if (!check(navigate(page, url(path)), "rtc-child-frame-page-load"))
            return 1;
        const bool empty = path == u"/rtc/cross-empty";
        const bool cross = path != u"/rtc/same-peer";
        ok &= expect(page, empty ? State::EmptyRegistriesAfterFreeze : State::RegisteredAfterFreeze,
                     2, cross && strictIsolation ? 2 : 1,
                     empty ? "rtc-empty-cross-origin-frame-registries"
                         : cross ? "rtc-cross-origin-child-peer-observed" : "rtc-same-origin-child-peer-observed");
    }
    {
        QWebEnginePage page(&profile);
        fixture.pin(page);
        if (!navigate(page, url(QStringLiteral("/rtc/same-peer")))
                || !scriptBoolean(page, QStringLiteral(
                    "document.addEventListener('freeze', () => document.querySelector('iframe').remove(), {once:true}); true")))
            return 1;
        const auto completion = beginProbe(page);
        ok &= check(waitFor([&] { return completion->result.has_value(); }) && completion->count == 1
                            && completion->result->state == State::Unknown
                            && page.lifecycleState() == QWebEnginePage::LifecycleState::Active
                            && scriptBoolean(page, QStringLiteral("!document.querySelector('iframe')")),
                    "rtc-frame-removal-during-freeze-is-unknown-and-restored");
    }
    {
        QWebEnginePage page(&profile);
        fixture.pin(page);
        if (!navigate(page, url(QStringLiteral("/rtc/empty")))
                || !scriptBoolean(page, QStringLiteral(
                    "document.addEventListener('freeze', () => document.body.append(document.createElement('iframe')), {once:true}); true")))
            return 1;
        const auto completion = beginProbe(page);
        ok &= check(waitFor([&] { return completion->result.has_value(); }) && completion->count == 1
                            && completion->result->state == State::Unknown
                            && page.lifecycleState() == QWebEnginePage::LifecycleState::Active
                            && scriptBoolean(page, QStringLiteral("!!document.querySelector('iframe')")),
                    "rtc-frame-creation-during-freeze-is-unknown-and-restored");
    }
    {
        QWebEnginePage page(&profile);
        fixture.pin(page);
        if (!navigate(page, url(QStringLiteral("/rtc/empty"))))
            return 1;
        using LeaseResult = QWebEnginePage::FerricRtcFrozenLeaseProbeResult;
        std::optional<LeaseResult> refused;
        bool scriptCompleted = false;
        bool createdPeer = false;
        page.runJavaScript(QStringLiteral(
                "const start = performance.now(); while (performance.now() - start < 150) {} "
                "window.pendingPeer = new RTCPeerConnection({iceServers: []}); true"),
                [&](const QVariant &value) {
            scriptCompleted = true;
            createdPeer = value.isValid() && value.toBool();
        });
        page.ferricRtcAcquireFrozenRegistryLeaseProbe(
                [&](LeaseResult result) { refused = std::move(result); }, [] {});
        ok &= check(waitFor([&] { return refused.has_value(); })
                            && refused->result.state == State::Unknown
                            && !refused->scopeStillHeld && !refused->release
                            && page.lifecycleState() == QWebEnginePage::LifecycleState::Active,
                    "rtc-pending-main-script-refuses-lease-before-freeze");
        ok &= check(waitFor([&] { return scriptCompleted; }) && createdPeer,
                    "rtc-pending-main-script-completes-after-refusal");
        ok &= expect(page, State::RegisteredAfterFreeze, 1, 1,
                     "rtc-pending-main-script-peer-requires-fresh-admission");
    }
    {
        QWebEnginePage page(&profile);
        fixture.pin(page);
        if (!navigate(page, url(QStringLiteral("/rtc/cross-empty"))))
            return 1;
        auto children = page.mainFrame().children();
        if (!check(children.size() == 1 && children.front().isValid(),
                   "rtc-pending-child-script-target-present"))
            return 1;
        using LeaseResult = QWebEnginePage::FerricRtcFrozenLeaseProbeResult;
        std::optional<LeaseResult> refused;
        children.front().runJavaScript(QStringLiteral(
                "const start = performance.now(); while (performance.now() - start < 150) {} "
                "window.pendingPeer = new RTCPeerConnection({iceServers: []}); true"));
        page.ferricRtcAcquireFrozenRegistryLeaseProbe(
                [&](LeaseResult result) { refused = std::move(result); }, [] {});
        ok &= check(waitFor([&] { return refused.has_value(); })
                            && refused->result.state == State::Unknown
                            && !refused->scopeStillHeld && !refused->release
                            && page.lifecycleState() == QWebEnginePage::LifecycleState::Active,
                    "rtc-pending-fire-and-forget-child-script-refuses-lease");
        std::optional<bool> childObserved;
        children.front().runJavaScript(QStringLiteral("!!window.pendingPeer"),
                                       [&](const QVariant &value) {
            childObserved = value.isValid() && value.toBool();
        });
        ok &= check(waitFor([&] { return childObserved.has_value(); }) && *childObserved,
                    "rtc-pending-fire-and-forget-child-script-completes");
        ok &= expect(page, State::RegisteredAfterFreeze, 2, strictIsolation ? 2 : 1,
                     "rtc-pending-child-script-peer-requires-fresh-admission");
    }
    {
        QWebEnginePage page(&profile);
        fixture.pin(page);
        if (!navigate(page, url(QStringLiteral("/rtc/empty"))))
            return 1;
        using LeaseResult = QWebEnginePage::FerricRtcFrozenLeaseProbeResult;
        std::optional<LeaseResult> result;
        int invalidations = 0;
        bool scriptCompleted = false;
        bool createdPeer = false;
        page.ferricRtcAcquireFrozenRegistryLeaseProbe(
                [&](LeaseResult value) { result = std::move(value); },
                [&] { ++invalidations; });
        page.runJavaScript(QStringLiteral(
                "window.admissionPeer = new RTCPeerConnection({iceServers: []}); true"),
                [&](const QVariant &value) {
            scriptCompleted = true;
            createdPeer = value.isValid() && value.toBool();
        });
        ok &= check(waitFor([&] { return result.has_value() && scriptCompleted; })
                            && createdPeer && page.lifecycleState() == QWebEnginePage::LifecycleState::Active
                            && (!result->scopeStillHeld || !result->scopeStillHeld())
                            && (result->result.state == State::Unknown || invalidations == 1),
                    "rtc-script-during-admission-cannot-leave-lease-held");
        ok &= expect(page, State::RegisteredAfterFreeze, 1, 1,
                     "rtc-script-during-admission-peer-requires-fresh-admission");
    }
    {
        QWebEnginePage page(&profile);
        fixture.pin(page);
        if (!navigate(page, url(QStringLiteral("/rtc/empty"))))
            return 1;
        const auto first = beginProbe(page);
        const auto overlap = beginProbe(page);
        ok &= check(overlap->count == 1 && overlap->result && overlap->result->state == State::Unknown,
                    "rtc-overlapping-probe-refused-immediately");
        ok &= check(waitFor([&] { return first->result.has_value(); }) && first->count == 1
                            && first->result->state == State::EmptyRegistriesAfterFreeze
                            && scriptBoolean(page, QStringLiteral("true")), "rtc-original-probe-survives-overlap");
        page.setLifecycleState(QWebEnginePage::LifecycleState::Frozen);
        const auto alreadyFrozen = beginProbe(page);
        ok &= check(alreadyFrozen->count == 1 && alreadyFrozen->result
                            && alreadyFrozen->result->state == State::Unknown
                            && page.lifecycleState() == QWebEnginePage::LifecycleState::Frozen,
                    "rtc-does-not-thaw-an-unowned-existing-freeze");
        page.setLifecycleState(QWebEnginePage::LifecycleState::Active);
        if (!scriptBoolean(page, QStringLiteral("true")))
            return 1;
        page.setVisible(true);
        const auto visible = beginProbe(page);
        ok &= check(visible->count == 1 && visible->result && visible->result->state == State::Unknown,
                    "rtc-visible-page-refused");
        page.setVisible(false);
    }
    {
        auto page = std::make_unique<QWebEnginePage>(&profile);
        fixture.pin(*page);
        if (!navigate(*page, url(QStringLiteral("/rtc/empty"))))
            return 1;
        const auto destroyed = beginProbe(*page);
        page.reset();
        ok &= check(waitFor([&] { return destroyed->result.has_value(); }) && destroyed->count == 1
                    && destroyed->result->state == State::Unknown, "rtc-owner-destruction-completes-once-unknown");
    }
    {
        QWebEnginePage page(&profile);
        fixture.pin(page);
        if (!navigate(page, url(QStringLiteral("/rtc/empty"))))
            return 1;
        const auto navigated = beginProbe(page);
        const bool newDocument = navigate(page, url(QStringLiteral("/rtc/peer")));
        ok &= check(newDocument && waitFor([&] { return navigated->result.has_value(); })
                            && navigated->count == 1 && navigated->result->state == State::Unknown
                            && page.lifecycleState() == QWebEnginePage::LifecycleState::Active
                            && scriptBoolean(page, QStringLiteral("!!window.peer")),
                    "rtc-navigation-during-probe-is-unknown-without-retargeting");
        ok &= expect(page, State::RegisteredAfterFreeze, 1, 1, "rtc-replacement-document-needs-a-new-probe");
        const auto shown = beginProbe(page);
        page.setVisible(true);
        ok &= check(waitFor([&] { return shown->result.has_value(); }) && shown->count == 1
                            && shown->result->state == State::Unknown && page.isVisible()
                            && page.lifecycleState() == QWebEnginePage::LifecycleState::Active,
                    "rtc-visibility-change-invalidates-probe-without-hiding-page");
        page.setVisible(false);
    }
    {
        QWebEnginePage opener(&profile), opened(&profile);
        fixture.pin(opener);
        fixture.pin(opened);
        opener.settings()->setAttribute(QWebEngineSettings::JavascriptCanOpenWindows, true);
        bool adopted = false;
        const auto connection = QObject::connect(&opener, &QWebEnginePage::newWindowRequested,
                                                 [&](QWebEngineNewWindowRequest &request) {
            request.openIn(&opened);
            adopted = true;
        });
        if (!check(navigate(opener, url(QStringLiteral("/rtc/empty")))
                           && scriptBoolean(opener, QStringLiteral("window.related = window.open('about:blank'); !!related"))
                           && waitFor([&] { return adopted; })
                           && scriptBoolean(opened, QStringLiteral("!!window.opener")),
                   "rtc-related-opener-fixture-created"))
            return 1;
        const auto related = beginProbe(opener);
        ok &= check(waitFor([&] { return related->result.has_value(); }) && related->count == 1
                            && related->result->state == State::Unknown
                            && opener.lifecycleState() == QWebEnginePage::LifecycleState::Active,
                    "rtc-related-active-contents-refused");
        QObject::disconnect(connection);
    }
    {
        QWebEnginePage page(&profile);
        fixture.pin(page);
        if (!navigate(page, url(QStringLiteral("/rtc/empty"))))
            return 1;
        using LeaseResult = QWebEnginePage::FerricRtcFrozenLeaseProbeResult;
        std::optional<LeaseResult> held;
        int readyCount = 0;
        int invalidations = 0;
        page.ferricRtcAcquireFrozenRegistryLeaseProbe([&](LeaseResult result) {
            ++readyCount;
            held = std::move(result);
        }, [&] { ++invalidations; });
        if (!check(waitFor([&] { return held.has_value(); }) && readyCount == 1
                           && held->result.state == State::EmptyRegistriesAfterFreeze
                           && held->scopeStillHeld && held->scopeStillHeld()
                           && held->release && page.lifecycleState() == QWebEnginePage::LifecycleState::Frozen,
                   "rtc-empty-registry-lease-retains-native-freeze"))
            return 1;
        const auto overlap = beginProbe(page);
        ok &= check(overlap->count == 1 && overlap->result && overlap->result->state == State::Unknown
                            && held->scopeStillHeld(), "rtc-held-lease-refuses-overlapping-probe");
        bool injected = false;
        bool scriptCompleted = false;
        bool invalidatedBeforeScriptCallback = false;
        page.runJavaScript(QStringLiteral(
                "window.injectedPeer = new RTCPeerConnection({iceServers: []}); true"),
                [&](const QVariant &value) {
            scriptCompleted = true;
            injected = value.isValid() && value.toBool();
            invalidatedBeforeScriptCallback = invalidations == 1
                    && page.lifecycleState() == QWebEnginePage::LifecycleState::Active
                    && !held->scopeStillHeld();
        });
        ok &= check(invalidations == 1 && !held->scopeStillHeld()
                            && page.lifecycleState() == QWebEnginePage::LifecycleState::Active,
                    "rtc-application-script-invalidates-lease-before-dispatch");
        ok &= check(waitFor([&] { return scriptCompleted; }) && injected
                            && invalidatedBeforeScriptCallback
                            && scriptBoolean(page, QStringLiteral("!!window.injectedPeer")),
                    "rtc-application-script-runs-only-after-lease-invalidation");
        held->release();
        held->release();
        ok &= check(!held->scopeStillHeld() && invalidations == 1,
                    "rtc-release-after-script-invalidation-is-idempotent");
        ok &= expect(page, State::RegisteredAfterFreeze, 1, 1,
                     "rtc-post-release-peer-requires-fresh-admission");
    }
    {
        QWebEnginePage page(&profile);
        fixture.pin(page);
        if (!navigate(page, url(QStringLiteral("/rtc/cross-empty"))))
            return 1;
        auto children = page.mainFrame().children();
        if (!check(children.size() == 1 && children.front().isValid(),
                   "rtc-child-frame-script-target-present"))
            return 1;
        using LeaseResult = QWebEnginePage::FerricRtcFrozenLeaseProbeResult;
        std::optional<LeaseResult> held;
        int invalidations = 0;
        page.ferricRtcAcquireFrozenRegistryLeaseProbe(
                [&](LeaseResult result) { held = std::move(result); },
                [&] { ++invalidations; });
        if (!check(waitFor([&] { return held.has_value(); })
                           && held->result.state == State::EmptyRegistriesAfterFreeze
                           && held->scopeStillHeld(), "rtc-child-script-lease-acquired"))
            return 1;
        bool scriptCompleted = false;
        bool scriptRanAfterInvalidation = false;
        children.front().runJavaScript(QStringLiteral(
                "window.injectedPeer = new RTCPeerConnection({iceServers: []}); true"),
                [&](const QVariant &value) {
            scriptCompleted = true;
            scriptRanAfterInvalidation = value.isValid() && value.toBool()
                    && invalidations == 1 && !held->scopeStillHeld()
                    && page.lifecycleState() == QWebEnginePage::LifecycleState::Active;
        });
        ok &= check(invalidations == 1 && !held->scopeStillHeld()
                            && page.lifecycleState() == QWebEnginePage::LifecycleState::Active
                            && waitFor([&] { return scriptCompleted; }) && scriptRanAfterInvalidation,
                    "rtc-child-script-invalidates-before-dispatch");
        ok &= expect(page, State::RegisteredAfterFreeze, 2, strictIsolation ? 2 : 1,
                     "rtc-child-script-peer-requires-fresh-admission");
    }
    {
        QWebEnginePage page(&profile);
        fixture.pin(page);
        if (!navigate(page, url(QStringLiteral("/rtc/cross-empty"))))
            return 1;
        using LeaseResult = QWebEnginePage::FerricRtcFrozenLeaseProbeResult;
        std::optional<LeaseResult> held;
        int invalidations = 0;
        page.ferricRtcAcquireFrozenRegistryLeaseProbe(
                [&](LeaseResult result) { held = std::move(result); },
                [&] { ++invalidations; });
        if (!check(waitFor([&] { return held.has_value(); })
                           && held->result.state == State::EmptyRegistriesAfterFreeze
                           && held->result.frameCount == 2
                           && held->result.processCount >= (strictIsolation ? 2 : 1)
                           && held->scopeStillHeld(), "rtc-child-frame-lease-retained"))
            return 1;
        page.setVisible(true);
        ok &= check(waitFor([&] { return invalidations == 1; })
                            && !held->scopeStillHeld()
                            && page.lifecycleState() == QWebEnginePage::LifecycleState::Active
                            && page.isVisible(), "rtc-visibility-invalidates-held-lease");
        held->release();
        page.setVisible(false);
    }
    {
        QWebEnginePage page(&profile);
        fixture.pin(page);
        if (!navigate(page, url(QStringLiteral("/rtc/peer"))))
            return 1;
        using LeaseResult = QWebEnginePage::FerricRtcFrozenLeaseProbeResult;
        std::optional<LeaseResult> result;
        page.ferricRtcAcquireFrozenRegistryLeaseProbe(
                [&](LeaseResult value) { result = std::move(value); }, [] {});
        ok &= check(waitFor([&] { return result.has_value(); })
                            && result->result.state == State::RegisteredAfterFreeze
                            && !result->scopeStillHeld && !result->release
                            && page.lifecycleState() == QWebEnginePage::LifecycleState::Active,
                    "rtc-existing-peer-refuses-held-lease-and-restores-page");
    }
    {
        auto page = std::make_unique<QWebEnginePage>(&profile);
        fixture.pin(*page);
        if (!navigate(*page, url(QStringLiteral("/rtc/empty"))))
            return 1;
        using LeaseResult = QWebEnginePage::FerricRtcFrozenLeaseProbeResult;
        std::optional<LeaseResult> held;
        int invalidations = 0;
        page->ferricRtcAcquireFrozenRegistryLeaseProbe(
                [&](LeaseResult result) { held = std::move(result); },
                [&] { ++invalidations; });
        if (!check(waitFor([&] { return held.has_value(); })
                           && held->result.state == State::EmptyRegistriesAfterFreeze
                           && held->scopeStillHeld(), "rtc-owner-loss-fixture-holds-lease"))
            return 1;
        page.reset();
        ok &= check(waitFor([&] { return invalidations == 1; })
                            && !held->scopeStillHeld(), "rtc-owner-loss-invalidates-held-lease-once");
        held->release();
    }
#if defined(Q_OS_LINUX) && defined(SYS_pidfd_open) && defined(SYS_pidfd_send_signal)
    {
        auto page = std::make_unique<QWebEnginePage>(&profile);
        fixture.pin(*page);
        if (!navigate(*page, url(QStringLiteral("/rtc/empty")))
            || !check(waitFor([&] { return page->renderProcessPid() > 1; }),
                      "rtc-renderer-loss-fixture-has-live-renderer"))
            return 1;
        using LeaseResult = QWebEnginePage::FerricRtcFrozenLeaseProbeResult;
        std::optional<LeaseResult> held;
        int invalidations = 0;
        page->ferricRtcAcquireFrozenRegistryLeaseProbe(
                [&](LeaseResult result) { held = std::move(result); },
                [&] { ++invalidations; });
        if (!check(waitFor([&] { return held.has_value(); })
                           && held->result.state == State::EmptyRegistriesAfterFreeze
                           && held->scopeStillHeld(), "rtc-renderer-loss-fixture-holds-empty-lease"))
            return 1;

        bool rendererTerminated = false;
        QObject::connect(page.get(), &QWebEnginePage::renderProcessTerminated, page.get(),
                         [&](QWebEnginePage::RenderProcessTerminationStatus, int) {
            rendererTerminated = true;
        });
        QString rendererSignalFailure;
        if (!check(terminateExactProbeRenderer(page->renderProcessPid(), &rendererSignalFailure),
                   "rtc-renderer-loss-signals-only-the-verified-probe-renderer")) {
            const auto failureText = rendererSignalFailure.toUtf8();
            std::fprintf(stderr, "Renderer-loss safety guard declined the test signal: %s\n",
                         failureText.constData());
            return 1;
        }
        const bool terminationObserved = waitFor([&] { return rendererTerminated; });
        const bool invalidationObserved = waitFor([&] { return invalidations == 1; });
        ok &= check(terminationObserved, "rtc-renderer-loss-native-termination-observed");
        ok &= check(invalidationObserved && !held->scopeStillHeld(),
                    "rtc-renderer-loss-invalidates-held-lease-exactly-once");

        const auto afterCrash = beginProbe(*page);
        ok &= check(waitFor([&] { return afterCrash->result.has_value(); })
                            && afterCrash->count == 1
                            && afterCrash->result->state == State::Unknown,
                    "rtc-renderer-loss-follow-up-observation-is-unknown");
        held->release();
    }
#endif
    {
        QWebEnginePage page(&profile);
        fixture.pin(page);
        if (!check(navigate(page, url(QStringLiteral("/rtc/overflow"))), "rtc-frame-budget-fixture-loaded"))
            return 1;
        bool froze = false;
        const auto connection = QObject::connect(&page, &QWebEnginePage::lifecycleStateChanged,
                                                 [&](QWebEnginePage::LifecycleState state) {
            froze |= state == QWebEnginePage::LifecycleState::Frozen;
        });
        const auto overflow = beginProbe(page);
        ok &= check(waitFor([&] { return overflow->result.has_value(); }) && overflow->count == 1
                            && overflow->result->state == State::Unknown
                            && overflow->result->frameCount == 128 && !froze
                            && scriptBoolean(page, QStringLiteral("window.frames.length === 128")),
                    "rtc-frame-budget-overflow-refused-before-freeze");
        QObject::disconnect(connection);
    }
    ok &= check(fixture.certificateScopeHeld(), "rtc-only-pinned-fixture-certificate-accepted");
    std::cout << "PRODUCTION_CAPABILITY_ENABLED=false\n";
    return ok ? 0 : 1;
}
