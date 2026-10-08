#include "owned_https_fixture.h"

#include <QCoreApplication>
#include <QFile>
#include <QFileInfo>
#include <QDebug>
#include <QGuiApplication>
#include <QMetaObject>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QTimer>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>

#include <csignal>
#include <limits>
#include <thread>

#if defined(Q_OS_LINUX)
#include <sys/syscall.h>
#include <sys/types.h>
#include <unistd.h>
#endif

#if defined(Q_OS_LINUX) && defined(SYS_pidfd_open) && defined(SYS_pidfd_send_signal)
namespace {
bool probeRendererIdentity(pid_t pid, QByteArray *startTime, QString *failure)
{
    const auto procRoot = QStringLiteral("/proc/%1/").arg(pid);
    QFile stat(procRoot + QStringLiteral("stat"));
    if (!stat.open(QIODevice::ReadOnly)) {
        *failure = QStringLiteral("renderer /proc stat is unavailable");
        return false;
    }
    const auto statBytes = stat.readAll();
    const auto nameEnd = statBytes.lastIndexOf(')');
    if (nameEnd < 0) {
        *failure = QStringLiteral("renderer /proc stat is malformed");
        return false;
    }
    const auto fields = statBytes.mid(nameEnd + 2).simplified().split(' ');
    if (fields.size() <= 19 || fields.at(19).isEmpty()) {
        *failure = QStringLiteral("renderer start identity is unavailable");
        return false;
    }

    QFile cmdline(procRoot + QStringLiteral("cmdline"));
    if (!cmdline.open(QIODevice::ReadOnly)) {
        *failure = QStringLiteral("reported PID %1 has no readable command line").arg(pid);
        return false;
    }
    const auto commandLine = cmdline.readAll();
    const bool isRenderer = commandLine.contains(QByteArrayLiteral("--type=renderer"));
    if (!isRenderer) {
        auto displayCommandLine = commandLine;
        displayCommandLine.replace('\0', ' ');
        *failure = QStringLiteral("reported PID %1 is not a Chromium renderer (%2)")
                .arg(pid).arg(QString::fromLocal8Bit(displayCommandLine));
        return false;
    }

    QFile status(procRoot + QStringLiteral("status"));
    if (!status.open(QIODevice::ReadOnly)) {
        *failure = QStringLiteral("renderer /proc status is unavailable");
        return false;
    }
    qint64 parentPid = -1;
    for (const auto &line : status.readAll().split('\n')) {
        if (line.startsWith(QByteArrayLiteral("PPid:"))) {
            bool parsed = false;
            parentPid = line.mid(5).trimmed().toLongLong(&parsed);
            if (!parsed) {
                *failure = QStringLiteral("renderer parent PID is malformed");
                return false;
            }
            break;
        }
    }
    if (parentPid != QCoreApplication::applicationPid()) {
        // Chromium may use a sandboxed zygote below its initial zygote. Permit
        // at most two verified QtWebEngineProcess zygotes ending at this app.
        qint64 ancestorPid = parentPid;
        for (int depth = 0; ancestorPid != QCoreApplication::applicationPid() && depth < 2; ++depth) {
            const auto ancestorRoot = QStringLiteral("/proc/%1/").arg(ancestorPid);
            QFile ancestorStatus(ancestorRoot + QStringLiteral("status"));
            if (!ancestorStatus.open(QIODevice::ReadOnly)) {
                *failure = QStringLiteral("renderer zygote ancestry is unavailable");
                return false;
            }
            qint64 nextAncestorPid = -1;
            bool nextAncestorParsed = false;
            for (const auto &line : ancestorStatus.readAll().split('\n')) {
                if (line.startsWith(QByteArrayLiteral("PPid:"))) {
                    nextAncestorPid = line.mid(5).trimmed().toLongLong(&nextAncestorParsed);
                    break;
                }
            }
            QFile ancestorCmdline(ancestorRoot + QStringLiteral("cmdline"));
            const auto ancestorExecutable = QFileInfo(ancestorRoot + QStringLiteral("exe")).symLinkTarget();
            if (!nextAncestorParsed
                    || QFileInfo(ancestorExecutable).fileName() != QStringLiteral("QtWebEngineProcess")
                    || !ancestorCmdline.open(QIODevice::ReadOnly)
                    || !ancestorCmdline.readAll().contains(QByteArrayLiteral("--type=zygote"))) {
                *failure = QStringLiteral("renderer ancestry contains an untrusted process");
                return false;
            }
            ancestorPid = nextAncestorPid;
        }
        if (ancestorPid != QCoreApplication::applicationPid()) {
            *failure = QStringLiteral("renderer zygote ancestry did not terminate at the fixture");
            return false;
        }
    }

    const auto executable = QFileInfo(procRoot + QStringLiteral("exe")).symLinkTarget();
    if (QFileInfo(executable).fileName() != QStringLiteral("QtWebEngineProcess")) {
        *failure = QStringLiteral("renderer executable identity mismatch");
        return false;
    }
    *startTime = fields.at(19);
    return true;
}

bool terminateExactProbeRenderer(qint64 processId, QString *failure)
{
    if (processId <= 1 || processId > std::numeric_limits<pid_t>::max()) {
        *failure = QStringLiteral("Qt reported no usable renderer PID");
        return false;
    }
    const auto pid = static_cast<pid_t>(processId);
    QByteArray startBefore;
    if (!probeRendererIdentity(pid, &startBefore, failure))
        return false;

    const int pidFd = static_cast<int>(syscall(SYS_pidfd_open, pid, 0));
    if (pidFd < 0) {
        *failure = QStringLiteral("pidfd_open refused the renderer");
        return false;
    }
    QByteArray startAfter;
    const bool sameRenderer = probeRendererIdentity(pid, &startAfter, failure)
            && startBefore == startAfter;
    if (!sameRenderer && failure->isEmpty())
        *failure = QStringLiteral("renderer process identity changed before signal");
    const bool signalled = sameRenderer
            && syscall(SYS_pidfd_send_signal, pidFd, SIGKILL, nullptr, 0) == 0;
    if (sameRenderer && !signalled)
        *failure = QStringLiteral("pidfd_send_signal refused the renderer");
    close(pidFd);
    return signalled;
}
} // namespace
#endif

int main(int argc, char **argv)
{
    const QByteArray chromiumFlags = qgetenv("QTWEBENGINE_CHROMIUM_FLAGS");
    if (argc != 3
            || (chromiumFlags != "--use-fake-device-for-media-stream"
                && chromiumFlags
                        != "--use-fake-device-for-media-stream --site-per-process"
                && chromiumFlags
                        != "--use-fake-device-for-media-stream --disable-gpu")
            || qEnvironmentVariableIsSet("QTWEBENGINE_REMOTE_DEBUGGING")
            || qEnvironmentVariableIsSet("QTWEBENGINE_DISABLE_SANDBOX")
            || qEnvironmentVariableIsSet("LD_PRELOAD"))
        return 2;

    const QString certificatePath = QString::fromLocal8Bit(argv[1]);
    const QString keyPath = QString::fromLocal8Bit(argv[2]);
    if (!QFileInfo(certificatePath).isAbsolute() || !QFileInfo(keyPath).isAbsolute())
        return 2;

    QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
    QtWebEngineQuick::initialize();
    QGuiApplication app(argc, argv);
    app.setQuitOnLastWindowClosed(false);

    Fixture fixture;
    if (!check(fixture.start(certificatePath, keyPath), "rtc-quick-owned-loopback-tls-server"))
        return 1;

    const QString qmlImportPath = qEnvironmentVariable("FERRIC_SPIKE_QML_IMPORT_PATH");
    if (qmlImportPath.isEmpty())
        return 2;
    std::cout << "RTC child-capture probe flags: capture="
              << qEnvironmentVariableIsSet("FERRIC_SPIKE_CROSS_FRAME_CAPTURE_PROBE")
              << " same-origin="
              << qEnvironmentVariableIsSet("FERRIC_SPIKE_CROSS_FRAME_CAPTURE_SAME_ORIGIN_PROBE")
              << " forced-grant="
              << qEnvironmentVariableIsSet("FERRIC_SPIKE_FORCE_CHILD_VIDEO_GRANT_PROBE") << '\n';

    QQmlApplicationEngine engine;
    engine.addImportPath(qmlImportPath);
    QObject::connect(&engine, &QQmlApplicationEngine::warnings, &app,
                     [](const QList<QQmlError> &warnings) {
        for (const auto &warning : warnings)
            std::cerr << warning.toString().toStdString() << '\n';
    });
    engine.rootContext()->setContextProperty(
            "ferricFixtureUrl", fixture.url(QStringLiteral("cookie.localhost"),
                                             QStringLiteral("/rtc/empty")));
    engine.rootContext()->setContextProperty(
            "ferricCrossCaptureFixtureUrl", fixture.url(QStringLiteral("cookie.localhost"),
                    qEnvironmentVariableIsSet("FERRIC_SPIKE_CROSS_FRAME_CAPTURE_SAME_ORIGIN_PROBE")
                            ? QStringLiteral("/rtc/same-capture") : QStringLiteral("/rtc/cross-capture")));
    engine.rootContext()->setContextProperty(
            "ferricRunCrossFrameCaptureProbe",
            qEnvironmentVariableIsSet("FERRIC_SPIKE_CROSS_FRAME_CAPTURE_PROBE"));
    engine.rootContext()->setContextProperty(
            "ferricSameOriginCaptureProbe",
            qEnvironmentVariableIsSet("FERRIC_SPIKE_CROSS_FRAME_CAPTURE_SAME_ORIGIN_PROBE"));
    engine.rootContext()->setContextProperty(
            "ferricForcedChildPermissionProbe",
            qEnvironmentVariableIsSet("FERRIC_SPIKE_FORCE_CHILD_VIDEO_GRANT_PROBE"));
    engine.rootContext()->setContextProperty(
            "ferricExpectedQuickProcessCount",
            chromiumFlags.contains("--site-per-process") ? 2 : 1);
    engine.load(QUrl::fromLocalFile(QStringLiteral(FERRIC_RTC_QUICK_QML_PATH)));
    const auto roots = engine.rootObjects();
    std::cout << "RTC Qt Quick QML root count " << roots.size() << '\n';
    if (roots.size() != 1)
        return 1;

    QObject *root = roots.front();
    QObject *probeView = root->findChild<QObject *>(QStringLiteral("ferricRtcQuickProbeView"));
    if (!check(probeView != nullptr, "rtc-quick-exact-view-found"))
        return 1;

    bool foreignThreadGuardAttempted = false;
    QTimer foreignThreadGuardTimer;
    foreignThreadGuardTimer.setInterval(20);
    QObject::connect(&foreignThreadGuardTimer, &QTimer::timeout, &app, [&] {
        if (foreignThreadGuardAttempted || root->property("phase").toInt() != 30)
            return;
        foreignThreadGuardAttempted = true;
        bool leaseBefore = false;
        const bool leaseReadBefore = QMetaObject::invokeMethod(
                probeView, "ferricRtcEmptyLeaseValidForProbe", Qt::DirectConnection,
                Q_RETURN_ARG(bool, leaseBefore));
        bool foreignThreadCallsRefused = false;
        std::thread foreignThread([&] {
            bool acquireResult = true;
            bool validResult = true;
            const bool acquireInvoked = QMetaObject::invokeMethod(
                    probeView, "ferricAcquireRtcEmptyLeaseForProbe", Qt::DirectConnection,
                    Q_RETURN_ARG(bool, acquireResult));
            const bool validInvoked = QMetaObject::invokeMethod(
                    probeView, "ferricRtcEmptyLeaseValidForProbe", Qt::DirectConnection,
                    Q_RETURN_ARG(bool, validResult));
            const bool releaseInvoked = QMetaObject::invokeMethod(
                    probeView, "ferricReleaseRtcEmptyLeaseForProbe", Qt::DirectConnection);
            foreignThreadCallsRefused = acquireInvoked && !acquireResult
                    && validInvoked && !validResult && releaseInvoked;
        });
        foreignThread.join();

        bool leaseAfter = false;
        const bool leaseReadAfter = QMetaObject::invokeMethod(
                probeView, "ferricRtcEmptyLeaseValidForProbe", Qt::DirectConnection,
                Q_RETURN_ARG(bool, leaseAfter));
        const bool passed = leaseReadBefore && leaseBefore && foreignThreadCallsRefused
                && leaseReadAfter && leaseAfter;
        root->setProperty("foreignThreadGuardProbePassed", passed);
    });
    foreignThreadGuardTimer.start();

    QObject::connect(root, SIGNAL(finished(int)), &app, SLOT(quit()));
    QTimer rendererLossTimer;
#if defined(Q_OS_LINUX) && defined(SYS_pidfd_open) && defined(SYS_pidfd_send_signal)
    bool rendererTerminationAttempted = false;
    rendererLossTimer.setInterval(20);
    QObject::connect(&rendererLossTimer, &QTimer::timeout, &app, [&] {
        if (rendererTerminationAttempted || root->property("phase").toInt() != 29)
            return;
        rendererTerminationAttempted = true;
        QObject *view = root->findChild<QObject *>(QStringLiteral("ferricRtcQuickProbeView"));
        const auto processId = view ? view->property("renderProcessPid").toLongLong() : -1;
        QString failure;
        const bool signalled = terminateExactProbeRenderer(processId, &failure);
        root->setProperty("rendererTerminationReason", failure);
        root->setProperty("rendererTerminationSignalled", signalled);
        if (!signalled)
            qWarning() << "RTC Qt Quick probe refused renderer signal after identity checks";
    });
    rendererLossTimer.start();
#endif
    QTimer::singleShot(90000, &app, &QCoreApplication::quit);
    app.exec();
    const int result = root->property("result").toInt();
    std::cout << "RTC Qt Quick probe result " << result
              << " phase " << root->property("phase").toInt()
              << " reason " << root->property("failureReason").toString().toStdString()
              << '\n';
    return result == 0 ? 0 : 1;
}
