#include <QtCore/QCoreApplication>
#include <QtCore/QDir>
#include <QtCore/QElapsedTimer>
#include <QtCore/QFile>
#include <QtCore/QFileInfo>
#include <QtCore/QProcess>
#include <QtCore/QSet>
#include <QtCore/QTemporaryDir>
#include <QtCore/QThread>
#include <QtCore/QUrl>
#include <QtGui/QGuiApplication>
#include <QtNetwork/QHostAddress>
#include <QtNetwork/QTcpServer>
#include <QtNetwork/QTcpSocket>
#include <QtQuick/QQuickWindow>
#include <QtQml/QQmlApplicationEngine>
#include <QtQml/QQmlContext>
#include <QtQml/QQmlError>
#include <QtQml/qqml.h>
#include <QtTest/QTest>
#include <QtWebEngineCore/QWebEngineUrlRequestInfo>
#include <QtWebEngineCore/QWebEngineUrlRequestInterceptor>
#include <QtWebEngineCore/QWebEngineDownloadRequest>
#include <QtWebEngineQuick/QQuickWebEngineDownloadRequest>
#include <QtWebEngineQuick/QQuickWebEngineProfile>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include "../../src/versioning_archive_interceptor.h"

#include <atomic>
#include <functional>
#include <iostream>
#include <memory>

namespace {
class RecordingArchiveInterceptor final : public FerricVersioningArchiveInterceptor {
public:
    void interceptRequest(QWebEngineUrlRequestInfo &request) override
    {
        const auto url = request.requestUrl();
        if (url.isLocalFile()) {
            localAttempts.fetch_add(1, std::memory_order_relaxed);
            if (request.resourceType() == QWebEngineUrlRequestInfo::ResourceTypeMainFrame)
                localMainFrames.fetch_add(1, std::memory_order_relaxed);
        }
        if (url.scheme() == u"http" || url.scheme() == u"https"
            || url.scheme() == u"ws" || url.scheme() == u"wss") {
            attempts.fetch_add(1, std::memory_order_relaxed);
            if (url.path() == u"/form")
                formAttempts.fetch_add(1, std::memory_order_relaxed);
            if (url.path() == u"/redirect")
                redirectAttempts.fetch_add(1, std::memory_order_relaxed);
        }
        FerricVersioningArchiveInterceptor::interceptRequest(request);
    }
    std::atomic_int attempts{0};
    std::atomic_int formAttempts{0};
    std::atomic_int redirectAttempts{0};
    std::atomic_int localAttempts{0};
    std::atomic_int localMainFrames{0};
};

bool waitUntil(QGuiApplication &app, const std::function<bool()> &predicate, int timeoutMs = 15000)
{
    QElapsedTimer elapsed;
    elapsed.start();
    while (!predicate() && elapsed.elapsed() < timeoutMs) {
        app.processEvents();
        QThread::msleep(10);
    }
    return predicate();
}

bool activateNativeWindow(QGuiApplication &app, QQuickWindow *window)
{
    window->requestActivate();
    if (waitUntil(app, [&] { return window->isActive(); }, 1000))
        return true;
    if (qEnvironmentVariableIsSet("HYPRLAND_INSTANCE_SIGNATURE")) {
        QProcess focus;
        const auto selector = QStringLiteral("pid:%1").arg(QCoreApplication::applicationPid());
        focus.start(QStringLiteral("hyprctl"),
                    {QStringLiteral("dispatch"),
                     QStringLiteral("hl.dsp.focus({ window = \"%1\" })").arg(selector)});
        if (focus.waitForStarted(1000)) {
            focus.waitForFinished(1000);
            const auto output = focus.readAllStandardOutput();
            const auto error = focus.readAllStandardError();
            if (focus.exitStatus() != QProcess::NormalExit || focus.exitCode() != 0
                || !error.isEmpty()) {
                std::cerr << "Hyprland focus request failed: "
                          << output.toStdString() << ' ' << error.toStdString() << '\n';
            }
        }
    }
    return waitUntil(app, [&] { return window->isActive(); }, 2000);
}

bool activateArchiveControl(QGuiApplication &app, QObject *root,
                            QQuickWindow *window, int targetControl,
                            bool &inputUnavailable)
{
    if (!window->isVisible() || !QMetaObject::invokeMethod(root, "focusArchive"))
        return false;
    if (!window->isActive()) {
        inputUnavailable = true;
        return false;
    }
    waitUntil(app, [] { return false; }, 50);
    if (!window->isActive()) {
        inputUnavailable = true;
        return false;
    }
    // The hostile MHTML fixture places the submit button at (320, 220) and
    // the redirect link at (80, 95) in this exact 640x480 Quick window.
    const QPoint point = targetControl == 0 ? QPoint(320, 220) : QPoint(80, 95);
    QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier, point, 50);
    waitUntil(app, [] { return false; }, 100);
    return true;
}

bool writeFile(const QString &path, const QByteArray &contents)
{
    QFile file(path);
    if (!file.open(QIODevice::WriteOnly | QIODevice::NewOnly)
        || file.write(contents) != contents.size())
        return false;
    file.close();
    return file.error() == QFile::NoError;
}

QByteArray mhtmlWithUnresolvedRemoteResources(const QByteArray &remoteBase)
{
    QByteArray archive = R"MHTML(From: <Ferric offline archive probe>
MIME-Version: 1.0
Content-Type: multipart/related; type="text/html"; boundary="----=_FerricOfflineProbe"

------=_FerricOfflineProbe
Content-Type: text/html; charset="UTF-8"
Content-Transfer-Encoding: quoted-printable
Content-Location: REMOTE/versioning/offline-resource-probe.html

<!doctype html><html><head><title>Offline resource probe</title>
<link rel="stylesheet" href="REMOTE/versioning/offline-resource.css">
</head><body><h1>Offline resource probe</h1>
<img src="REMOTE/versioning/offline-resource.png" alt="remote image">
<iframe title="remote frame" src="REMOTE/versioning/offline-resource-frame"></iframe>
<form action="REMOTE/versioning/offline-resource-form" method="post">
<input type="hidden" name="probe" value="synthetic">
<button type="submit" style="position:fixed; left:0; top:0; opacity:0; width:640px; height:480px">
Submit offline form</button>
</form>
</body></html>
------=_FerricOfflineProbe--
)MHTML";
    archive.replace("REMOTE", remoteBase);
    archive.replace("\n", "\r\n");
    return archive;
}

bool invoke(QObject *root, const char *method, const QString &arg)
{
    return QMetaObject::invokeMethod(root, method, Q_ARG(QVariant, QVariant(arg)));
}

bool check(bool condition, const char *message)
{
    if (!condition)
        std::cerr << "FAIL: " << message << '\n';
    return condition;
}

bool captureSucceeded(int terminalState, int matchingRequests, bool exactView,
                      bool exactPath, bool expectedFormat, bool nonemptyOutput)
{
    return terminalState == QWebEngineDownloadRequest::DownloadCompleted
        && matchingRequests == 1 && exactView && exactPath && expectedFormat
        && nonemptyOutput;
}
}

int main(int argc, char **argv)
{
    QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
    QGuiApplication app(argc, argv);
    QtWebEngineQuick::initialize();
    if (!check(argc == 3, "provide QML probe and disposable archive output paths"))
        return 2;
    QTemporaryDir operation;
    if (!check(operation.isValid(), "create disposable operation directory"))
        return 2;
    const auto sourcePath = operation.filePath("source.html");
    const auto savePath = operation.filePath("capture.mhtml");
    const auto productionRoutePath = operation.filePath("production-route.mhtml");
    const auto refusedRoutePath = operation.filePath("refused-route.mhtml");
    const auto hostilePath = operation.filePath("hostile.mhtml");
    const auto unresolvedResourcesPath = operation.filePath("unresolved-resources.mhtml");
    const auto sourceProfileStoragePath = operation.filePath("source-profile/storage");
    const auto sourceProfileCachePath = operation.filePath("source-profile/cache");
    if (!check(QDir().mkpath(sourceProfileStoragePath)
                   && QDir().mkpath(sourceProfileCachePath),
               "create isolated persistent-source profile directories"))
        return 2;
    QTcpServer listener;
    if (!check(listener.listen(QHostAddress::LocalHost), "listen on disposable loopback port"))
        return 2;
    int networkHits = 0;
    int sourceProfileControlRequests = 0;
    int unresolvedArchiveResourceRequests = 0;
    bool archiveNetworkPhase = false;
    int postResultRequests = 0;
    bool postResultBodyMatched = false;
    QObject::connect(&listener, &QTcpServer::newConnection, &app, [&] {
        while (auto *connection = listener.nextPendingConnection()) {
            if (archiveNetworkPhase)
                ++networkHits;
            auto request = std::make_shared<QByteArray>();
            QObject::connect(connection, &QTcpSocket::disconnected,
                             connection, &QObject::deleteLater);
            QObject::connect(connection, &QTcpSocket::readyRead, connection,
                             [connection, request, &postResultRequests, &postResultBodyMatched,
                              &sourceProfileControlRequests,
                              &unresolvedArchiveResourceRequests] {
                request->append(connection->readAll());
                const auto headerEnd = request->indexOf("\r\n\r\n");
                if (headerEnd < 0)
                    return;
                const auto header = request->left(headerEnd);
                if (header.contains(" /versioning/offline-resource-"))
                    ++unresolvedArchiveResourceRequests;
                const bool postResult = header.startsWith("POST /capture-result ");
                const bool postSource = header.startsWith("GET /post-source ");
                const bool sourceProfileControl =
                    header.startsWith("GET /source-profile-control ");
                if (sourceProfileControl)
                    ++sourceProfileControlRequests;
                qsizetype contentLength = 0;
                if (postResult) {
                    for (const auto &line : header.split('\n')) {
                        const auto trimmed = line.trimmed();
                        if (trimmed.toLower().startsWith("content-length:")) {
                            bool valid = false;
                            contentLength = trimmed.mid(sizeof("content-length:") - 1)
                                                .trimmed().toLongLong(&valid);
                            if (!valid || contentLength < 0 || contentLength > 1024) {
                                connection->disconnectFromHost();
                                return;
                            }
                        }
                    }
                    if (request->size() < headerEnd + 4 + contentLength)
                        return;
                    ++postResultRequests;
                    postResultBodyMatched = request->mid(headerEnd + 4, contentLength)
                                                .contains("payload=synthetic");
                }
                const QByteArray body = sourceProfileControl
                    ? QByteArray("<!doctype html><title>Source profile control</title>"
                                 "<h1>Normal source profile remains live</h1>")
                    : postSource
                    ? QByteArray(R"HTML(<!doctype html><title>POST source</title>
<form id="post-form" action="/capture-result" method="post">
<input name="payload" value="synthetic"><button type="submit">Submit</button>
</form>)HTML")
                    : postResult
                        ? QByteArray("<!doctype html><title>Ferric POST capture result</title>"
                                     "<h1>Ferric POST capture result</h1>")
                        : QByteArray("<!doctype html><title>Not found</title>");
                const QByteArray status = (sourceProfileControl || postSource || postResult)
                    ? QByteArray("HTTP/1.1 200 OK\r\n")
                    : QByteArray("HTTP/1.1 404 Not Found\r\n");
                connection->write(status + "Content-Type: text/html; charset=utf-8\r\n"
                                  "Connection: close\r\nContent-Length: "
                                  + QByteArray::number(body.size()) + "\r\n\r\n" + body);
                connection->disconnectFromHost();
            });
        }
    });
    const auto remoteBase = QByteArray("http://127.0.0.1:")
        + QByteArray::number(listener.serverPort());
    QByteArray source = R"HTML(<!doctype html><title>Synthetic capture</title>
<h1>Local synthetic fixture</h1>
<button id="control" style="position:fixed;left:20px;top:20px;width:600px;height:400px;z-index:99" onclick="this.setAttribute('data-clicked','yes')">input control</button>
<a id="control-link" style="position:fixed;left:0;top:0;width:100px;height:20px;z-index:100" href="REMOTE/link">link control</a>
<a id="ordinary" href="data:text/plain,ordinary-payload" download="ordinary.txt">download</a>)HTML";
    source.replace("REMOTE", remoteBase);
    if (!check(writeFile(sourcePath, source), "write source fixture"))
        return 2;

    // The interceptor must outlive the QML-owned profile that references it.
    RecordingArchiveInterceptor guard;
    qmlRegisterType<FerricVersioningArchiveInterceptor>(
        "io.github.ferricbrowser", 1, 0, "VersioningArchiveInterceptor");
    QQmlApplicationEngine engine;
    engine.rootContext()->setContextProperty(
        "archiveProbeSourceProfileStoragePath", sourceProfileStoragePath);
    engine.rootContext()->setContextProperty(
        "archiveProbeSourceProfileCachePath", sourceProfileCachePath);
    QObject::connect(&engine, &QQmlApplicationEngine::warnings, &app,
                     [](const QList<QQmlError> &warnings) {
                         for (const auto &warning : warnings)
                             std::cerr << "QML warning: " << warning.toString().toStdString()
                                       << '\n';
                     });
    engine.load(QUrl::fromLocalFile(QString::fromLocal8Bit(argv[1])));
    if (!check(engine.rootObjects().size() == 1, "load archive probe QML")) {
        std::cerr << "QML loader diagnostics: platform="
                  << QGuiApplication::platformName().toStdString()
                  << " rootObjects=" << engine.rootObjects().size() << '\n';
        return 2;
    }
    auto *root = engine.rootObjects().first();
    auto *window = qobject_cast<QQuickWindow *>(root);
    if (!check(window != nullptr, "archive probe is a native Quick window"))
        return 1;
    auto *sourceProfile = qobject_cast<QQuickWebEngineProfile *>(
        root->property("sourceProfile").value<QObject *>());
    auto *archiveProfile = qobject_cast<QQuickWebEngineProfile *>(
        root->property("archiveProfile").value<QObject *>());
    if (!check(sourceProfile && archiveProfile && sourceProfile != archiveProfile,
               "distinct public WebEngine profiles"))
        return 2;
    if (!check(!sourceProfile->isOffTheRecord() && archiveProfile->isOffTheRecord(),
               "disk-backed source profile and off-the-record archive profile"))
        return 2;
    if (!check(sourceProfile->property("persistentStoragePath").toString()
                       == sourceProfileStoragePath
                   && sourceProfile->property("cachePath").toString()
                          == sourceProfileCachePath,
               "source profile storage and cache stay under the disposable operation"))
        return 2;
    int sourcePageSaveRequests = 0;
    bool sourcePageSaveExposedPage = false;
    QObject::connect(sourceProfile, &QQuickWebEngineProfile::downloadRequested,
                     &app, [&](QQuickWebEngineDownloadRequest *download) {
                         if (!download->isSavePageDownload())
                             return;
                         ++sourcePageSaveRequests;
                         // This is the public base-class page() accessor. The
                         // pinned Qt Quick adapter currently returns null for
                         // QML-origin downloads, unlike the Widgets adapter.
                         if (download->page())
                             sourcePageSaveExposedPage = true;
                     });
    if (!check(guard.attach(archiveProfile), "attach production archive interceptor"))
        return 2;
    if (!check(!guard.attach(sourceProfile),
               "one archive interceptor refuses a second off-record profile"))
        return 2;
    if (!check(root->property("archiveFocusOnNavigationEnabled").toBool(),
               "archive receives focus after its authorised navigation")
        || !check(!root->property("archiveJavascriptEnabled").toBool(), "archive scripts disabled")
        || !check(!root->property("archiveLocalRemoteEnabled").toBool(),
                  "archive local-to-remote access disabled")
        || !check(!root->property("archiveLocalFilesEnabled").toBool(),
                  "archive local-to-file access disabled"))
        return 1;
    root->setProperty("ordinaryDirectory", operation.path());

    // A POST-result document is not safe to replay as a GET, but an exact
    // one-use page-save permit may still archive the current rendered page.
    const auto postSourceUrl = QString::fromLatin1(remoteBase + "/post-source");
    const auto postResultUrl = QString::fromLatin1(remoteBase + "/capture-result");
    const auto postArchivePath = operation.filePath("post-result.mhtml");
    if (!check(invoke(root, "startSource", postSourceUrl), "load POST source fixture")
        || !check(waitUntil(app, [&] {
                      return root->property("sourceLoaded").toBool()
                          || root->property("sourceFailed").toBool();
                  }), "POST source load completes")
        || !check(!root->property("sourceFailed").toBool(), "POST source load succeeds")
        || !check(QMetaObject::invokeMethod(root, "submitPostSource"),
                  "submit same-origin POST fixture")
        || !check(waitUntil(app, [&] {
                      return root->property("sourceLoaded").toBool()
                          || root->property("sourceFailed").toBool();
                  }), "POST result load completes")
        || !check(!root->property("sourceFailed").toBool(), "POST result load succeeds")
        || !check(postResultRequests == 1 && postResultBodyMatched,
                  "loopback fixture receives exact POST body")
        || !check(root->property("sourceCurrentUrl").toString() == postResultUrl,
                  "current native view is the POST-result route")
        || !check(invoke(root, "saveSourceThroughProductionRoute", postArchivePath),
                  "save POST result through exact production route")
        || !check(waitUntil(app, [&] {
                      return root->property("productionSaveFinishedState").toInt() >= 0;
                  }), "POST-result save reaches terminal state")
        || !check(root->property("productionSaveAdmissionCalls").toInt() == 1
                      && root->property("productionSaveAdmissionExact").toBool(),
                  "POST-result save reaches exact native admission once")
        || !check(root->property("productionSaveFinishedState").toInt()
                      == QWebEngineDownloadRequest::DownloadCompleted,
                  "POST-result MHTML save completes")
        || !check(QFileInfo(postArchivePath).size() > 0,
                  "POST-result MHTML output is nonempty"))
        return 1;
    QFile postArchive(postArchivePath);
    if (!check(postArchive.open(QIODevice::ReadOnly), "read POST-result archive")
        || !check(postArchive.readAll().contains("Ferric POST capture result"),
                  "POST-result archive contains rendered response"))
        return 1;
    if (!check(sourcePageSaveRequests == 1,
               "QML page save reaches the public profile download signal once")
        || !check(!sourcePageSaveExposedPage,
                  "public download page() accessor exposes no QML page origin"))
        return 1;
    std::cout << "FERRIC_ARCHIVE_POST_RESULT:method=POST;body_matched=1"
              << ";exact_save=1;completed=1;public_page_origin=unavailable" << '\n';
    if (!check(invoke(root, "startSource", QUrl::fromLocalFile(sourcePath).toString()),
               "start source load")
        || !check(waitUntil(app, [&] { return root->property("sourceLoaded").toBool()
                                           || root->property("sourceFailed").toBool(); }),
                  "source load completes")
        || !check(!root->property("sourceFailed").toBool(), "source load succeeds"))
        return 1;
    if (!activateNativeWindow(app, window)) {
        std::cerr << "native Wayland focus unavailable for source input control\n";
        return 77;
    }
    if (!check(QMetaObject::invokeMethod(root, "focusSource"), "focus source input control"))
        return 1;
    QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier, QPoint(320, 220), 50);
    waitUntil(app, [] { return false; }, 250);
    if (!check(QMetaObject::invokeMethod(root, "inspectSourceControl"),
               "inspect source input control")
        || !check(waitUntil(app, [&] {
                      return root->property("sourceControlInspected").toBool();
                  }), "source input control inspection completes")
        || !check(root->property("sourceControlClicked").toBool(),
                  "QtTest click reaches a regular local WebEngine page"))
        return 1;
    QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier, QPoint(50, 10), 50);
    if (!check(waitUntil(app, [&] {
                   return root->property("sourceLinkNavigations").toInt() > 0;
               }), "QtTest click creates a regular local link navigation"))
        return 1;
    if (!check(invoke(root, "saveSource", savePath), "invoke QML page save")
        || !check(waitUntil(app, [&] { return root->property("saveFinishedState").toInt() >= 0; }),
                  "page-save terminal state"))
        return 1;
    // isFinished alone is intentionally not accepted as success.
    if (!check(root->property("saveRequests").toInt() == 1, "one page-save request")
        || !check(root->property("saveFinishedIsFinished").toBool(),
                  "completed page-save callback is terminal")
        || !check(root->property("saveMatchedView").toBool(), "public view correlation")
        || !check(root->property("saveMatchedPath").toBool(), "exact preauthorised path")
        || !check(root->property("saveMatchedFormat").toBool(), "MHTML format")
        || !check(captureSucceeded(root->property("saveFinishedState").toInt(), 1,
                                   root->property("saveMatchedView").toBool(),
                                   root->property("saveMatchedPath").toBool(),
                                   root->property("saveMatchedFormat").toBool(),
                                   QFileInfo(savePath).size() > 0),
                  "one completed correlated nonempty MHTML output"))
        return 1;
    const auto verifyRefusal = [&] {
        if (!check(invoke(root, "refuseProductionSave", refusedRoutePath),
                   "invoke real save with Rust-admission refusal")
            || !check(waitUntil(app, [&] {
                          return root->property("productionSaveRejected").toBool();
                      }), "production route rejects refused native request")
            || !check(root->property("productionSaveAdmissionCalls").toInt() == 1
                          && root->property("productionSaveAdmissionExact").toBool(),
                      "refused request reached exact Rust-adapter boundary once")
            || !check(root->property("productionSaveCancelledOperations").toInt() == 1,
                      "refused request cancels only its capture operation")
            || !check(root->property("productionSaveRetiredExact").toBool(),
                      "refused request retires exact native ID and staging path")
            || !check(root->property("productionSaveRefusedStateAfterCancel").toInt()
                          == QWebEngineDownloadRequest::DownloadCancelled
                          && root->property("productionSaveRefusedIsFinishedAfterCancel").toBool(),
                      "refused native request is cancelled synchronously")
            || !check(root->property("productionSaveSameTabRetryBlocked").toBool(),
                      "refused native save quarantines its tab before another save can start")
            || !check(!QFileInfo(refusedRoutePath).exists(),
                      "refused native save leaves no archive output"))
            return false;
        std::cout << "FERRIC_ARCHIVE_ROUTE_REFUSAL_RESULT:cancelled=1;retired=1"
                  << ";same_tab_retry_blocked=1" << '\n';
        return true;
    };
    if (!check(QMetaObject::invokeMethod(root, "startHeldOrdinaryDownload"),
               "start ordinary download before production page-save")
        || !check(waitUntil(app, [&] {
                      return root->property("ordinaryRequestHeld").toBool();
                  }), "ordinary Qt request remains outstanding")
        || !check(invoke(root, "saveSourceThroughProductionRoute", productionRoutePath),
                  "invoke production page-save alongside held ordinary request")
        || !check(waitUntil(app, [&] {
                      return root->property("productionSaveFinishedState").toInt() >= 0
                          && root->property("ordinaryFinishedState").toInt() >= 0;
                  }), "production save and ordinary download completion")
        || !check(root->property("productionSaveAdmissionCalls").toInt() == 1,
                  "one Rust-adapter admission call from real Qt save request")
        || !check(root->property("productionSaveAdmissionExact").toBool(),
                  "production route observes exact native ID, view, path and format")
        || !check(root->property("productionSaveFinishedState").toInt()
                      == QWebEngineDownloadRequest::DownloadCompleted,
                  "production-routed save completes")
        || !check(QFileInfo(productionRoutePath).size() > 0,
                  "production-routed save writes nonempty MHTML")
        || !check(root->property("ordinaryRequests").toInt() == 1
                      && root->property("ordinaryFinishedState").toInt()
                             == QWebEngineDownloadRequest::DownloadCompleted,
                  "ordinary download stays independently routed and completes")
        || !check(root->property("ordinaryReleasedWhileSavePending").toBool(),
                  "ordinary request accepted during production save admission"))
        return 1;
    std::cout << "FERRIC_ARCHIVE_ROUTE_RESULT:exact_request=1;completed=1"
              << ";ordinary_released_during_save=1" << '\n';
    // Save a second page before the cancellation probes. Qt serializes the
    // hostile form/link content as MHTML, so the DOM is not reconstructed by
    // editing MIME bytes in a previously saved archive.
    const auto hostileSourcePath = operation.filePath("hostile.html");
    QByteArray hostileSource = R"HTML(<!doctype html><title>Hostile&#x061c;&#x200e;&#x200f;&#x202e; archive</title>
<style>@import url('REMOTE/css');</style>
<img src="REMOTE/image" alt="remote image" style="display:none">
<iframe src="REMOTE/frame" title="remote frame" style="display:none"></iframe>
<form action="REMOTE/form" method="post"><input type="hidden" name="x" value="1">
<button id="form-submit" type="submit" style="position:fixed;left:20px;top:20px;width:600px;height:400px;z-index:99" autofocus>submit</button></form>
<a id="redirect-link" href="REMOTE/redirect" style="position:fixed;left:20px;top:90px;z-index:100">follow redirect</a>
<a id="download-link" href="REMOTE/download" download="fixture.txt" style="position:fixed;left:20px;top:140px;width:180px;height:30px;z-index:101">download</a>
<a id="protocol-link" href="ferric-probe:blocked" style="position:fixed;left:20px;top:190px;width:180px;height:30px;z-index:101">external protocol</a>
<a id="embedded-download-link" href="data:text/plain;base64,ZmVycmljLWFyY2hpdmUtcHJvYmU=" download="embedded.txt" style="position:fixed;left:20px;top:240px;width:180px;height:30px;z-index:101">embedded download</a>
<script>window.open('REMOTE/popup')</script>)HTML";
    hostileSource.replace("REMOTE", remoteBase);
    if (!check(writeFile(hostileSourcePath, hostileSource), "write hostile source fixture"))
        return 2;
    root->setProperty("sourceJavascriptEnabled", false);
    if (!check(invoke(root, "startSource", QUrl::fromLocalFile(hostileSourcePath).toString()),
               "load hostile source fixture")
        || !check(waitUntil(app, [&] { return root->property("sourceLoaded").toBool()
                                           || root->property("sourceFailed").toBool(); }),
                  "hostile source load completes")
        || !check(!root->property("sourceFailed").toBool(), "hostile source load succeeds"))
        return 2;
    if (!activateNativeWindow(app, window))
        return 77;
    if (!check(QMetaObject::invokeMethod(root, "focusSource"),
               "focus hostile HTML source form"))
        return 1;
    QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier, QPoint(320, 220), 50);
    if (!check(waitUntil(app, [&] {
                   return root->property("sourceFormNavigations").toInt() > 0;
               }), "regular local HTML form submits through native input")
        || !check(invoke(root, "saveSource", hostilePath), "save hostile source as MHTML"))
        return 2;
    if (!waitUntil(app, [&] { return root->property("saveFinishedState").toInt() >= 0; })) {
        std::cerr << "hostile save diagnostics: requests="
                  << root->property("saveRequests").toInt()
                  << " active=" << root->property("activeSaveState").toInt()
                  << " finished=" << root->property("saveFinishedState").toInt()
                  << " active_id=" << root->property("activeSaveId").toInt()
                  << " last_finished_id=" << root->property("lastFinishedDownloadId").toInt()
                  << " last_finished_state=" << root->property("lastFinishedDownloadState").toInt()
                  << " refused_id=" << root->property("productionSaveRefusedId").toInt()
                  << " refused_state_after_cancel="
                  << root->property("productionSaveRefusedStateAfterCancel").toInt()
                  << " refused_finished_after_cancel="
                  << root->property("productionSaveRefusedIsFinishedAfterCancel").toBool()
                  << " refused_finish_state="
                  << root->property("productionSaveRefusedFinishedState").toInt()
                  << " refused_bytes=" << QFileInfo(refusedRoutePath).size()
                  << " bytes=" << QFileInfo(hostilePath).size() << '\n';
        check(false, "hostile page-save reaches terminal state");
        return 2;
    }
    if (!check(root->property("saveFinishedState").toInt()
                      == QWebEngineDownloadRequest::DownloadCompleted,
                  "hostile page-save completed")
        || !check(root->property("saveMatchedView").toBool()
                      && root->property("saveMatchedPath").toBool()
                      && root->property("saveMatchedFormat").toBool()
                      && QFileInfo(hostilePath).size() > 0,
                  "hostile page-save has exact view/path/format and nonempty output"))
        return 2;
    QFile hostileArchive(hostilePath);
    if (!check(hostileArchive.open(QIODevice::ReadOnly), "read generated hostile archive"))
        return 2;
    const auto hostileBytes = hostileArchive.readAll();
    if (!check(hostileBytes.contains("form-submit"),
               "generated MHTML retains the synthetic form control")
        || !check(hostileBytes.contains("/css") && hostileBytes.contains("/image"),
                  "generated MHTML retains remote CSS and image references")
        || !check(hostileBytes.contains("download-link")
                      && hostileBytes.contains("protocol-link")
                      && hostileBytes.contains("embedded-download-link"),
                  "generated MHTML retains download and external-protocol controls")) {
        return 2;
    }

    if (!check(writeFile(unresolvedResourcesPath,
                         mhtmlWithUnresolvedRemoteResources(remoteBase)),
               "write synthetic MHTML with unresolved remote CSS, image, and frame references"))
        return 2;
    QFile unresolvedResourcesArchive(unresolvedResourcesPath);
    if (!check(unresolvedResourcesArchive.open(QIODevice::ReadOnly),
               "read synthetic unresolved-resource MHTML"))
        return 2;
    const auto unresolvedResourcesBytes = unresolvedResourcesArchive.readAll();
    if (!check(unresolvedResourcesBytes.contains(remoteBase + "/versioning/offline-resource.css")
                   && unresolvedResourcesBytes.contains(
                       remoteBase + "/versioning/offline-resource.png")
                   && unresolvedResourcesBytes.contains(
                       remoteBase + "/versioning/offline-resource-frame")
                   && unresolvedResourcesBytes.contains(
                       remoteBase + "/versioning/offline-resource-form"),
               "synthetic MHTML retains remote stylesheet/image/frame/form references"))
        return 2;
    unresolvedResourcesArchive.close();

    // Prove this exact coordinate creates a native download in ordinary
    // source HTML before asking how the MHTML sandbox handles the same link.
    root->setProperty("ordinaryFileName", QStringLiteral("embedded-control.txt"));
    root->setProperty("ordinaryFinishedState", -1);
    const auto ordinaryBeforeEmbedded = root->property("ordinaryRequests").toInt();
    QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier, QPoint(80, 255), 50);
    if (!check(waitUntil(app, [&] {
                   return root->property("ordinaryRequests").toInt()
                       > ordinaryBeforeEmbedded;
               }), "ordinary HTML embedded-download click reaches Qt downloadRequested")
        || !check(waitUntil(app, [&] {
                      return root->property("ordinaryFinishedState").toInt() >= 0;
                  }), "ordinary embedded download reaches terminal state")
        || !check(root->property("ordinaryFinishedState").toInt()
                      == QWebEngineDownloadRequest::DownloadCompleted,
                  "ordinary embedded download completes in disposable directory"))
        return 2;

    const auto cancelledPath = operation.filePath("cancelled.mhtml");
    if (!check(invoke(root, "cancelSourceSave", cancelledPath), "cancel a page save")
        || !check(waitUntil(app, [&] {
                   return root->property("cancelledRequestState").toInt() >= 0;
               }), "cancelled page-save reaches terminal request state")) {
        std::cerr << "cancel state: request="
                  << root->property("cancelledRequestState").toInt()
                  << " finished=" << root->property("cancelledFinishState").toInt()
                  << " isFinished=" << root->property("cancelledRequestIsFinished").toBool()
                  << '\n';
        return 1;
    }
    if (!check(root->property("cancelledRequestState").toInt()
                   == QWebEngineDownloadRequest::DownloadCancelled,
               "cancelled save reports cancelled")
        || !check(root->property("cancelledRequestIsFinished").toBool(),
                  "cancelled page-save request is finished without completion")
        || !check(root->property("cancelledFinishState").toInt()
                      != QWebEngineDownloadRequest::DownloadCompleted,
                  "isFinished without DownloadCompleted is not a successful finish callback")
        || !check(!QFileInfo(cancelledPath).exists(),
                  "isFinished cancelled page-save creates no archive output")
        || !check(!captureSucceeded(QWebEngineDownloadRequest::DownloadCancelled, 1,
                                    true, true, true, true),
                  "cancelled state never qualifies as capture")
        || !check(!captureSucceeded(QWebEngineDownloadRequest::DownloadInterrupted, 1,
                                    true, true, true, true),
                  "interrupted state never qualifies as capture")
        || !check(!captureSucceeded(QWebEngineDownloadRequest::DownloadCompleted, 2,
                                    true, true, true, true),
                  "ambiguous association never qualifies as capture"))
        return 1;
    const auto interruptedPath = operation.filePath("missing-parent/interrupted.mhtml");
    if (!check(invoke(root, "saveSource", interruptedPath), "request invalid-path save")
        || !check(waitUntil(app, [&] { return root->property("saveRequests").toInt() >= 4; }, 3000),
                  "invalid-path save request observed"))
        return 1;
    // On this Qt build an invalid parent directory can remain InProgress
    // rather than yielding an interruption signal. A bounded caller must
    // cancel/clean up at its deadline, never publish it as completed.
    waitUntil(app, [] { return false; }, 1000);
    if (!check(root->property("activeSaveState").toInt()
                   == QWebEngineDownloadRequest::DownloadInProgress,
               "invalid-path page-save reached in-progress state")
        || !check(root->property("saveFinishedState").toInt()
                      != QWebEngineDownloadRequest::DownloadCompleted,
                  "invalid-path save did not complete")
        || !check(!QFileInfo(interruptedPath).exists(), "invalid output absent")
        || !check(QMetaObject::invokeMethod(root, "cancelActiveSave"),
                  "cancel stalled invalid-path save"))
        return 1;
    if (!check(waitUntil(app, [&] {
                   const auto state = root->property("activeSaveState").toInt();
                   return state == QWebEngineDownloadRequest::DownloadCancelled
                       || state == QWebEngineDownloadRequest::DownloadInterrupted;
               }), "started page-save cancellation reaches terminal state")
        || !check(root->property("activeSaveState").toInt()
                      == QWebEngineDownloadRequest::DownloadCancelled,
               "started page-save reports cancelled")
        || !check(root->property("saveFinishedState").toInt()
                      != QWebEngineDownloadRequest::DownloadCompleted,
                  "started cancelled save never reports completed callback")
        || !check(!QFileInfo(interruptedPath).exists(),
                  "cancelled started page-save publishes no output"))
        return 1;

    // An abandoned native save poisons only its logical tab. Exercise the
    // admission refusal after the accepted save and overlapping ordinary
    // download above, then assert same-tab retry is blocked before Qt is
    // asked to reuse its page-save pipeline.
    if (!verifyRefusal())
        return 1;

    archiveNetworkPhase = true;
    if (!check(QFile::setPermissions(savePath,
                                     QFileDevice::ReadOwner | QFileDevice::WriteOwner),
               "restrict saved MHTML to the current user")
        || !check(guard.authorizeArchiveUrl(
                      QUrl::fromLocalFile(savePath).toString(QUrl::FullyEncoded)),
                  "authorize one exact saved MHTML file")
        || !check(invoke(root, "openArchive", QUrl::fromLocalFile(savePath).toString()),
               "open saved archive in guarded profile")
        || !check(waitUntil(app, [&] { return root->property("archiveLoaded").toBool()
                                           || root->property("archiveFailed").toBool(); }),
                  "saved archive reaches terminal state"))
        return 1;
    if (!check(!root->property("archiveFailed").toBool()
                   && root->property("archiveTitle").toString() == u"Synthetic capture",
               "captured MHTML renders in guarded profile"))
        return 1;
    if (!activateNativeWindow(app, window))
        return 77;
    const auto rejectedBeforeMhtmlLink = root->property("archiveRejectedNavigations").toInt();
    if (!check(QMetaObject::invokeMethod(root, "focusArchive"),
               "focus saved MHTML link control"))
        return 1;
    QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier, QPoint(50, 10), 50);
    if (!check(waitUntil(app, [&] {
                   return root->property("archiveRejectedNavigations").toInt()
                       > rejectedBeforeMhtmlLink;
               }), "QtTest click creates a saved MHTML link navigation"))
        return 1;

    if (!check(QFile::setPermissions(hostilePath,
                                        QFileDevice::ReadOwner | QFileDevice::WriteOwner),
                  "restrict hostile MHTML to the current user")
        || !check(guard.authorizeArchiveUrl(
                      QUrl::fromLocalFile(hostilePath).toString(QUrl::FullyEncoded)),
                  "authorize one exact hostile MHTML file"))
        return 2;
    if (!check(invoke(root, "openArchive", QUrl::fromLocalFile(hostilePath).toString()),
               "open archive in guarded profile")
        || !waitUntil(app, [&] { return root->property("archiveLoaded").toBool()
                                    || root->property("archiveFailed").toBool(); })) {
        std::cerr << "archive status: loaded=" << root->property("archiveLoaded").toBool()
                  << " failed=" << root->property("archiveFailed").toBool()
                  << " rejected=" << root->property("archiveRejectedNavigations").toInt()
                  << " attempts=" << guard.attempts.load()
                  << " local=" << guard.localAttempts.load()
                  << " localMainFrames=" << guard.localMainFrames.load()
                  << " bytes=" << QFileInfo(hostilePath).size()
                  << " navigation=" << root->property("lastNavigationUrl").toString().toStdString()
                  << " allowed=" << root->property("allowedArchiveUrl").toString().toStdString()
                  << '\n';
        return 1;
    }
    // Allow delayed archive activity to reach the interceptor.
    waitUntil(app, [] { return false; }, 500);
    if (!check(!root->property("archiveFailed").toBool(), "hostile archive renders")
        || !check(root->property("archiveTitle").toString()
                      == u"Hostile\u061c\u200e\u200f\u202e archive",
                  "hostile MHTML body is parsed")
        || !check(root->property("archiveDownloadRequests").toInt() == 0,
                  "archive caused no download")
        || !check(root->property("archivePopupRequests").toInt() == 0,
                  "disabled script caused no popup")
        || !check(networkHits == 0, "hostile archive caused zero loopback traffic"))
        return 1;
    if (!activateNativeWindow(app, window)) {
        std::cerr << "native Wayland focus unavailable: windowActive=" << window->isActive()
                  << " archiveActiveFocus="
                  << root->property("archiveHasActiveFocus").toBool() << '\n';
        return 77;
    }
    bool nativeInputUnavailable = false;
    const auto activateControl = [&](int targetControl, const char *failureMessage) {
        if (activateArchiveControl(app, root, window, targetControl, nativeInputUnavailable))
            return 0;
        if (nativeInputUnavailable) {
            std::cout << "SKIP: native Wayland input unavailable; archive release gate remains disabled\n";
            return 77;
        }
        check(false, failureMessage);
        return 1;
    };
    const auto earlyRedirectNavigations = root->property("archiveRedirectNavigations").toInt();
    if (const auto inputStatus = activateControl(
            1, "activate archived link before form qualification");
        inputStatus != 0)
        return inputStatus;
    if (!check(waitUntil(app, [&] {
                   return root->property("archiveRedirectNavigations").toInt()
                       > earlyRedirectNavigations;
               }), "QtTest click reaches a link in the hostile MHTML"))
        return 1;
    const auto formNavigationsBeforeClick = root->property("archiveFormNavigations").toInt();
    const auto rejectedBeforeForm = root->property("archiveRejectedNavigations").toInt();
    if (const auto inputStatus = activateControl(
            0, "focus and activate archived form with QtTest window click");
        inputStatus != 0)
        return inputStatus;
    // Chromium's MHTML sandbox can suppress the form before Qt's navigation
    // callback. If a later Chromium build allows the callback, both Ferric
    // layers must reject the same synthetic action.
    const bool formReachedView = waitUntil(app, [&] {
        return root->property("archiveFormNavigations").toInt() > formNavigationsBeforeClick;
    }, 1000);
    if (formReachedView) {
        if (!check(root->property("lastFormNavigationUrl").toString()
                       == QString::fromUtf8(remoteBase + "/form"),
                   "form activation targets the synthetic loopback endpoint")
            || !check(root->property("archiveRejectedNavigations").toInt()
                          > rejectedBeforeForm,
                      "user-activated form is rejected by the view"))
            return 1;
        root->setProperty("diagnosticAllowRemoteNavigation", true);
        if (const auto inputStatus = activateControl(
                0, "reactivate archived form through the profile gate");
            inputStatus != 0)
            return inputStatus;
        if (!check(waitUntil(app, [&] { return guard.formAttempts.load() > 0; }),
                   "profile interceptor blocks user-activated form submission"))
            return 1;
        root->setProperty("diagnosticAllowRemoteNavigation", false);
    } else if (!check(root->property("archiveFormNavigations").toInt()
                              == formNavigationsBeforeClick,
                      "MHTML sandbox suppresses form navigation before the Qt view callback")
               || !check(guard.formAttempts.load() == 0,
                         "MHTML sandbox sends no form request to the profile")
               || !check(networkHits == 0, "MHTML form sends no loopback traffic")) {
        return 1;
    }
    if (!check(networkHits == 0, "archived form sends no loopback traffic"))
        return 1;

    // If a form request reached the profile, it may have left an error
    // document. Reload under the normal view gate in either case.
    if (!check(guard.authorizeArchiveUrl(
                   QUrl::fromLocalFile(hostilePath).toString(QUrl::FullyEncoded)),
               "reauthorize the exact hostile MHTML before reload"))
        return 1;
    if (!check(invoke(root, "openArchive", QUrl::fromLocalFile(hostilePath).toString()),
               "reload hostile archive after attempted form")
        || !check(waitUntil(app, [&] { return root->property("archiveLoaded").toBool()
                                           || root->property("archiveFailed").toBool(); }),
                  "reloaded hostile archive reaches a terminal state")
        || !check(!root->property("archiveFailed").toBool(),
                  "reloaded hostile archive renders under the normal view gate"))
        return 1;

    // A real click on an archived external link is rejected by the view gate,
    // then independently blocked by the production profile interceptor.
    const auto redirectNavigationsBeforeClick =
        root->property("archiveRedirectNavigations").toInt();
    const auto rejectedRedirectsBeforeClick =
        root->property("archiveRejectedRedirectNavigations").toInt();
    if (const auto inputStatus = activateControl(
            1, "activate archived redirect link with QtTest window click");
        inputStatus != 0)
        return inputStatus;
    if (!check(waitUntil(app, [&] {
                   return root->property("archiveRedirectNavigations").toInt()
                       > redirectNavigationsBeforeClick;
               }), "QtTest click on active Wayland window activates archived redirect link")
        || !check(root->property("lastNavigationUrl").toString()
                      == QString::fromUtf8(remoteBase + "/redirect"),
                  "redirect link targets only the synthetic loopback endpoint")
        || !check(root->property("archiveRejectedRedirectNavigations").toInt()
                      > rejectedRedirectsBeforeClick,
                  "archive view rejects user-activated redirect target"))
        return 1;

    root->setProperty("diagnosticAllowRemoteNavigation", true);
    const auto interceptedRedirectsBeforeClick = guard.redirectAttempts.load();
    if (const auto inputStatus = activateControl(
            1, "reactivate archived redirect link through the profile gate");
        inputStatus != 0)
        return inputStatus;
    if (!check(waitUntil(app, [&] {
                   return guard.redirectAttempts.load() > interceptedRedirectsBeforeClick;
               }),
               "production profile interceptor blocks user-activated redirect")
        || !check(networkHits == 0, "intercepted form and redirect send no traffic"))
        return 1;
    waitUntil(app, [] { return false; }, 250);
    if (!check(networkHits == 0, "no delayed form or redirect loopback traffic"))
        return 1;
    root->setProperty("diagnosticAllowRemoteNavigation", false);

    // An archive was already rendered above. Switching the exact permit must
    // not leave that earlier local file readable through a cached load, and
    // clearing the permit must refuse even the most recently allowed file.
    const auto savedUrl = QUrl::fromLocalFile(savePath).toString(QUrl::FullyEncoded);
    const auto hostileUrl = QUrl::fromLocalFile(hostilePath).toString(QUrl::FullyEncoded);
    if (!check(guard.authorizeArchiveUrl(savedUrl),
               "switch exact archive authorization to the earlier saved file")
        || !check(invoke(root, "openArchive", savedUrl),
                  "open newly authorized saved archive after hostile archive")
        || !check(waitUntil(app, [&] { return root->property("archiveLoaded").toBool()
                                           || root->property("archiveFailed").toBool(); }),
                  "switched archive reaches terminal state")
        || !check(!root->property("archiveFailed").toBool()
                      && root->property("archiveTitle").toString() == u"Synthetic capture",
                  "only the newly authorized archive renders"))
        return 1;
    if (!check(invoke(root, "openArchive", hostileUrl),
               "attempt previously authorized hostile archive without a new permit")
        || !check(waitUntil(app, [&] { return root->property("archiveLoaded").toBool()
                                           || root->property("archiveFailed").toBool(); }),
                  "stale archive attempt reaches terminal state")
        || !check(root->property("archiveFailed").toBool()
                      && !root->property("archiveLoaded").toBool(),
                  "profile refuses stale archive after authorization switch"))
        return 1;
    guard.clearAuthorization();
    if (!check(invoke(root, "openArchive", savedUrl),
               "attempt saved archive after exact authorization revocation")
        || !check(waitUntil(app, [&] { return root->property("archiveLoaded").toBool()
                                           || root->property("archiveFailed").toBool(); }),
                  "revoked archive attempt reaches terminal state")
        || !check(root->property("archiveFailed").toBool()
                      && !root->property("archiveLoaded").toBool(),
                  "profile refuses cached archive after permit revocation")
        || !check(networkHits == 0, "switch and revocation cause no network traffic"))
        return 1;

    // Load the shipping archive component, not the simplified probe view.
    // Its private interceptor must accept the exact file independently of
    // the revoked probe interceptor, then acknowledge blank before cleanup.
    if (!check(invoke(root, "openProductionArchive", savedUrl),
               "create production archive component with exact saved URL")
        || !check(waitUntil(app, [&] {
                      return root->property("productionArchiveLoaded").toBool()
                          || root->property("productionArchiveFailed").toBool();
                  }), "production archive reaches terminal load state")
        || !check(root->property("productionArchiveLoaded").toBool()
                      && !root->property("productionArchiveFailed").toBool()
                      && root->property("productionArchiveTitle").toString()
                             == u"Synthetic capture",
                  "production component renders exact saved archive"))
        return 1;
    if (!check(QMetaObject::invokeMethod(root, "closeProductionArchive"),
               "ask production component to blank archive")
        || !check(waitUntil(app, [&] {
                      return root->property("productionArchiveBlanked").toBool()
                          || root->property("productionArchiveBlankTimedOut").toBool();
                  }), "production archive reaches blank or bounded timeout")
        || !check(root->property("productionArchiveBlanked").toBool()
                      && !root->property("productionArchiveBlankTimedOut").toBool(),
                  "production component acknowledges blank before file cleanup")
        || !check(QFileInfo(savePath).exists(),
                  "archive file remains until blank acknowledgement")
        || !check(networkHits == 0, "production archive load and blank cause no network traffic"))
        return 1;
    if (!check(invoke(root, "startSource", QUrl::fromLocalFile(sourcePath).toString()),
               "reload ordinary source profile after production archive close")
        || !check(waitUntil(app, [&] { return root->property("sourceLoaded").toBool()
                                           || root->property("sourceFailed").toBool(); }),
                  "ordinary source profile reaches terminal state after archive use")
        || !check(root->property("sourceLoaded").toBool()
                      && !root->property("sourceFailed").toBool(),
                  "production archive component did not replace source profile interceptor"))
        return 1;
    if (!check(invoke(root, "openProductionArchive", hostileUrl),
               "load hostile MHTML through a fresh production archive component")
        || !check(waitUntil(app, [&] {
                      return root->property("productionArchiveLoaded").toBool()
                          || root->property("productionArchiveFailed").toBool();
                  }), "production hostile archive reaches terminal load state")
        || !check(root->property("productionArchiveLoaded").toBool()
                      && !root->property("productionArchiveFailed").toBool()
                      && root->property("productionArchiveTitle").toString()
                             == u"Hostile[bidi][bidi][bidi][bidi] archive",
                  "production component renders hostile archive offline"))
        return 1;
    waitUntil(app, [] { return false; }, 500);
    if (!check(networkHits == 0, "production hostile archive causes no passive network traffic"))
        return 1;
    if (!activateNativeWindow(app, window))
        return 77;
    auto *productionItem = root->property("productionArchiveItem").value<QObject *>();
    if (!check(productionItem != nullptr, "current production archive component exists"))
        return 1;
    const auto productionProfiles = productionItem->findChildren<QQuickWebEngineProfile *>();
    if (!check(productionProfiles.size() == 1, "one current production archive profile"))
        return 1;
    auto *productionProfile = productionProfiles.first();
    if (!check(productionProfile != sourceProfile && productionProfile != archiveProfile
                   && productionProfile->isOffTheRecord(),
               "production archive uses one distinct off-the-record profile"))
        return 1;
    productionProfile->setDownloadPath(operation.path());
    if (!check(productionProfile->downloadPath() == operation.path(),
               "production download attempts stay inside disposable directory"))
        return 1;
    int productionDownloadRequests = 0;
    QSet<quint32> productionCancelledDownloads;
    QObject::connect(productionProfile, &QQuickWebEngineProfile::downloadRequested,
                     &app, [&](QQuickWebEngineDownloadRequest *download) {
                         ++productionDownloadRequests;
                         const auto id = download->id();
                         if (download->state() == QWebEngineDownloadRequest::DownloadCancelled)
                             productionCancelledDownloads.insert(id);
                         QObject::connect(download, &QWebEngineDownloadRequest::stateChanged,
                                          &app, [&, id](QWebEngineDownloadRequest::DownloadState state) {
                                              if (state == QWebEngineDownloadRequest::DownloadCancelled)
                                                  productionCancelledDownloads.insert(id);
                                          });
                     });
    const auto productionRejectedBeforeClick =
        root->property("productionArchiveRejected").toInt();
    QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier, QPoint(80, 95), 50);
    if (!check(waitUntil(app, [&] {
                   return root->property("productionArchiveRejected").toInt()
                       > productionRejectedBeforeClick;
               }), "production component rejects clicked external MHTML link")
        || !check(networkHits == 0, "production component click causes no network traffic"))
        return 1;
    const auto rejectedBeforeDownload = root->property("productionArchiveRejected").toInt();
    QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier, QPoint(80, 155), 50);
    waitUntil(app, [] { return false; }, 500);
    const auto rejectedAfterDownload = root->property("productionArchiveRejected").toInt();
    if (!check(rejectedAfterDownload > rejectedBeforeDownload,
               "production component rejects clicked archived download link")
        || !check(productionDownloadRequests == 0,
                  "archived download link creates no uncancelled download request")
        || !check(networkHits == 0, "production archived download link causes no network traffic"))
        return 1;
    const auto rejectedBeforeProtocol = root->property("productionArchiveRejected").toInt();
    QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier, QPoint(80, 205), 50);
    waitUntil(app, [] { return false; }, 500);
    const auto rejectedAfterProtocol = root->property("productionArchiveRejected").toInt();
    if (!check(rejectedAfterProtocol > rejectedBeforeProtocol,
               "production component rejects clicked external protocol")
        || !check(networkHits == 0, "production archived external protocol causes no network traffic"))
        return 1;
    const auto rejectedBeforeEmbedded = root->property("productionArchiveRejected").toInt();
    QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier, QPoint(80, 255), 50);
    waitUntil(app, [] { return false; }, 1000);
    const auto embeddedViewRejected =
        root->property("productionArchiveRejected").toInt() > rejectedBeforeEmbedded;
    if (productionDownloadRequests > 0
        && !check(waitUntil(app, [&] {
                      return productionCancelledDownloads.size() == productionDownloadRequests;
                  }, 3000), "all production archive downloads are cancelled"))
        return 1;
    // A Chromium MHTML sandbox may suppress the data-download action before
    // either Qt callback. Re-click the known external link to distinguish
    // that from a dead or unfocused archive view.
    const auto rejectedBeforeControl = root->property("productionArchiveRejected").toInt();
    QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier, QPoint(80, 95), 50);
    if (!check(waitUntil(app, [&] {
                   return root->property("productionArchiveRejected").toInt()
                       > rejectedBeforeControl;
               }), "archive view still receives input after embedded download attempt")
        || !check(!QFileInfo(operation.filePath("embedded.txt")).exists(),
                  "archived embedded download publishes no file")
        || !check(networkHits == 0, "embedded archive download causes no network traffic")
        || !check(QMetaObject::invokeMethod(root, "closeProductionArchive"),
                  "blank production hostile archive after rejected navigation"))
        return 1;
    if (!check(productionItem->property("closingToBlank").toBool(),
               "production viewer is closing before a deferred reopen")
        || !check(QMetaObject::invokeMethod(productionItem, "openAuthorizedArchive"),
                  "invoke the production viewer's deferred-open guard")
        || !check(productionItem->property("closingToBlank").toBool(),
                  "deferred archive open is refused while blanking")
        || !check(waitUntil(app, [&] {
                      return root->property("productionArchiveBlanked").toBool()
                          || root->property("productionArchiveBlankTimedOut").toBool();
                  }), "production hostile archive reaches blank or bounded timeout"))
        return 1;
    bool hostileBlankRetried = false;
    if (root->property("productionArchiveBlankTimedOut").toBool()) {
        hostileBlankRetried = true;
        std::cerr << "production hostile archive blank timed out after rejected link; "
                     "retrying exact close\n";
        root->setProperty("productionArchiveBlankTimedOut", false);
        if (!check(QMetaObject::invokeMethod(root, "closeProductionArchive"),
                   "retry production archive blank after bounded timeout")
            || !check(waitUntil(app, [&] {
                          return root->property("productionArchiveBlanked").toBool()
                              || root->property("productionArchiveBlankTimedOut").toBool();
                      }), "retried production archive reaches blank or timeout"))
            return 1;
    }
    if (!check(root->property("productionArchiveBlanked").toBool()
                   && !root->property("productionArchiveBlankTimedOut").toBool(),
               "production hostile archive eventually acknowledges blank"))
        return 1;

    // A Qt-produced MHTML can omit unresolved frames before archive viewing.
    // This synthetic valid package preserves explicit remote CSS/image/frame
    // references so the shipping offline profile is exercised with those
    // requests still present in the archived main document.
    const auto unresolvedResourcesUrl =
        QUrl::fromLocalFile(unresolvedResourcesPath).toString(QUrl::FullyEncoded);
    networkHits = 0;
    if (!check(QFile::setPermissions(unresolvedResourcesPath,
                                     QFileDevice::ReadOwner | QFileDevice::WriteOwner),
               "restrict synthetic unresolved-resource MHTML to the current user")
        || !check(invoke(root, "openProductionArchive", unresolvedResourcesUrl),
                  "open synthetic unresolved-resource MHTML in production component")
        || !check(waitUntil(app, [&] {
                      return root->property("productionArchiveLoaded").toBool()
                          || root->property("productionArchiveFailed").toBool();
                  }), "synthetic unresolved-resource MHTML reaches a terminal state")
        || !check(root->property("productionArchiveLoaded").toBool()
                      && !root->property("productionArchiveFailed").toBool()
                      && root->property("productionArchiveTitle").toString()
                             == u"Offline resource probe",
                  "production component renders the synthetic MHTML main document"))
        return 1;
    auto *resourcesItem = root->property("productionArchiveItem").value<QObject *>();
    const auto resourceInterceptors = resourcesItem
        ? resourcesItem->findChildren<FerricVersioningArchiveInterceptor *>()
        : QList<FerricVersioningArchiveInterceptor *>{};
    if (!check(resourceInterceptors.size() == 1,
               "synthetic archive has exactly one production interceptor"))
        return 1;
    auto *resourceInterceptor = resourceInterceptors.first();
    const auto viewRejectedBeforeForm = root->property("productionArchiveRejected").toInt();
    // The transparent submit control spans the fixed disposable view so the
    // native click is user-activated even when MHTML sandbox policy suppresses
    // the form callback before Qt's navigation/profile interception layers.
    QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier, QPoint(320, 220), 50);
    waitUntil(app, [] { return false; }, 750);
    const auto blockedStylesheets = resourceInterceptor->blockedRequestCount(
        QWebEngineUrlRequestInfo::ResourceTypeStylesheet);
    const auto blockedImages = resourceInterceptor->blockedRequestCount(
        QWebEngineUrlRequestInfo::ResourceTypeImage);
    const auto blockedSubframes = resourceInterceptor->blockedRequestCount(
        QWebEngineUrlRequestInfo::ResourceTypeSubFrame);
    const auto blockedMainFrames = resourceInterceptor->blockedRequestCount(
        QWebEngineUrlRequestInfo::ResourceTypeMainFrame);
    const auto viewFormRejections = root->property("productionArchiveRejected").toInt()
        - viewRejectedBeforeForm;
    // The shipping view disables local-content access to remote URLs before
    // requests reach the profile. Depending on Chromium's MHTML sandbox, that
    // renderer policy may suppress these references without invoking the
    // interceptor. Count any requests that do reach the production profile,
    // but accept renderer-side suppression as a separate (stricter) barrier;
    // never report zero network traffic as proof of interceptor callbacks.
    if (!check(networkHits == 0 && unresolvedArchiveResourceRequests == 0,
               "unresolved CSS/image/frame and clicked form produce no network traffic"))
        return 1;

    // Keep the isolated archive visible while the separate source profile
    // loads a live control route. This proves the archive's restrictive
    // interceptor is not installed on, or replacing, the source profile.
    const auto sourceProfileControlUrl =
        QString::fromLatin1(remoteBase + "/source-profile-control");
    archiveNetworkPhase = false;
    if (!check(invoke(root, "startSource", sourceProfileControlUrl),
               "navigate the separate source profile under the archive overlay")
        || !check(waitUntil(app, [&] {
                      return root->property("sourceLoaded").toBool()
                          || root->property("sourceFailed").toBool();
                  }), "source-profile control request reaches a terminal load state")
        || !check(root->property("sourceLoaded").toBool()
                      && !root->property("sourceFailed").toBool()
                      && root->property("sourceCurrentUrl").toString()
                             == sourceProfileControlUrl,
                  "source profile loads its independent live control while archive is open")
        || !check(sourceProfileControlRequests == 1,
                  "independent source profile request reaches the loopback server")
        || !check(root->property("productionArchiveLoaded").toBool()
                      && root->property("productionArchiveMode").toBool(),
                  "source navigation does not replace the open archive view")
        || !check(networkHits == 0 && unresolvedArchiveResourceRequests == 0,
                  "source control is the only request during archive isolation"))
        return 1;
    archiveNetworkPhase = true;

    if (!check(QMetaObject::invokeMethod(root, "closeProductionArchive"),
               "blank synthetic unresolved-resource archive")
        || !check(waitUntil(app, [&] {
                      return root->property("productionArchiveBlanked").toBool()
                          || root->property("productionArchiveBlankTimedOut").toBool();
                  }), "synthetic unresolved-resource archive reaches blank or timeout")
        || !check(root->property("productionArchiveBlanked").toBool()
                      && !root->property("productionArchiveBlankTimedOut").toBool(),
                  "synthetic unresolved-resource archive acknowledges blank")
        || !check(QFileInfo(unresolvedResourcesPath).exists(),
                  "synthetic archive remains present until its owning cleanup")
        || !check(networkHits == 0 && unresolvedArchiveResourceRequests == 0,
                  "blanking archive sends no unresolved-resource request"))
        return 1;
    std::cout << "FERRIC_ARCHIVE_BLOCKED_RESOURCES:stylesheet=" << blockedStylesheets
              << ";image=" << blockedImages << ";subframe=" << blockedSubframes
              << ";main_frame=" << blockedMainFrames
              << ";form_view_rejections=" << viewFormRejections
              << ";source_profile_requests=" << sourceProfileControlRequests
              << ";unresolved_archive_requests=" << unresolvedArchiveResourceRequests
              << ";network_hits=" << networkHits << '\n';

    // Exercise the same production chrome host that schedules Rust cleanup
    // after the isolated component's successful blank acknowledgement. The
    // fixture bridge records that handoff; application/storage tests own the
    // separate exact-file removal result.
    if (!check(invoke(root, "openIntegratedArchive", savedUrl),
               "open production archive host with exact saved file")
        || !check(waitUntil(app, [&] {
                      return root->property("integratedArchivePresented").toBool();
                  }), "production archive host presents exact saved file")
        || !check(root->property("integratedCleanupCalls").toInt() == 0,
                  "archive host does not schedule cleanup while file is shown")
        || !check(QMetaObject::invokeMethod(root, "closeIntegratedArchive"),
                  "request close through production archive host")
        || !check(waitUntil(app, [&] {
                      return root->property("integratedCleanupCalls").toInt() > 0;
                  }), "production archive host schedules cleanup after blank")
        || !check(root->property("integratedCleanupCalls").toInt() == 1
                      && root->property("integratedCleanupExact").toBool()
                      && root->property("integratedRuntimeSchedules").toInt() == 1,
                  "blank acknowledgement schedules one exact runtime cleanup")
        || !check(!root->property("integratedArchiveActive").toBool(),
                  "cleared archive URL deactivates production viewer")
        || !check(QFileInfo(savePath).exists(),
                  "test bridge leaves file present for exact application cleanup")
        || !check(networkHits == 0, "production host handoff sends no network request"))
        return 1;
    std::cout << "FERRIC_ARCHIVE_HOST_RESULT:cleanup_calls=1;runtime_schedules=1"
              << ";viewer_active=0;network_hits=" << networkHits << '\n';
    if (!check(invoke(root, "openIntegratedArchive", savedUrl),
               "reopen production archive host for cleanup-refusal case")
        || !check(waitUntil(app, [&] {
                      return root->property("integratedArchivePresented").toBool();
                  }), "production archive host presents reopened file")
        || !check(QMetaObject::invokeMethod(root, "failNextIntegratedArchiveClose"),
                  "arm one exact cleanup refusal")
        || !check(QMetaObject::invokeMethod(root, "closeIntegratedArchive"),
                  "blank reopened archive before refused cleanup")
        || !check(waitUntil(app, [&] {
                      return root->property("integratedCleanupCalls").toInt() == 1;
                  }), "first cleanup scheduling attempt reaches bridge")
        || !check(root->property("integratedArchiveActive").toBool()
                      && root->property("integratedRuntimeSchedules").toInt() == 0,
                  "refused cleanup retains blank viewer and schedules no storage work")
        || !check(QFileInfo(savePath).exists(),
                  "refused cleanup retains exact staged file")
        || !check(QMetaObject::invokeMethod(root, "closeIntegratedArchive"),
                  "retry close after cleanup scheduling refusal")
        || !check(waitUntil(app, [&] {
                      return root->property("integratedCleanupCalls").toInt() == 2;
                  }), "retry reaches exact cleanup bridge")
        || !check(root->property("integratedCleanupExact").toBool()
                      && root->property("integratedRuntimeSchedules").toInt() == 1
                      && !root->property("integratedArchiveActive").toBool(),
                  "retry schedules cleanup once and deactivates viewer")
        || !check(QFileInfo(savePath).exists(),
                  "staged file remains for application-side exact cleanup")
        || !check(networkHits == 0, "cleanup retry causes no network request"))
        return 1;
    std::cout << "FERRIC_ARCHIVE_HOST_RETRY_RESULT:cleanup_calls=2;runtime_schedules=1"
              << ";viewer_active=0;network_hits=" << networkHits << '\n';
    std::cout << "FERRIC_ARCHIVE_PROBE_RESULT:hostile_link_rejected=1"
              << ";hostile_blank_retry=" << (hostileBlankRetried ? 1 : 0)
              << ";download_requests=" << productionDownloadRequests
              << ";download_rejected="
              << (rejectedAfterDownload > rejectedBeforeDownload ? 1 : 0)
              << ";protocol_rejected="
              << (rejectedAfterProtocol > rejectedBeforeProtocol ? 1 : 0)
              << ";embedded_view_rejected=" << (embeddedViewRejected ? 1 : 0)
              << ";embedded_download_requests=" << productionDownloadRequests
              << ";embedded_download_cancelled=" << productionCancelledDownloads.size()
              << ";embedded_control_live=1"
              << ";network_hits=" << networkHits << '\n';
    std::cout << "FERRIC_ARCHIVE_REFUSED_FINAL:exists="
              << (QFileInfo(refusedRoutePath).exists() ? 1 : 0)
              << ";bytes=" << QFileInfo(refusedRoutePath).size() << '\n';
    // Export only after the complete native probe passes. The Rust adapter
    // fixture consumes this exact Qt-produced MHTML before xtask removes its
    // disposable directory; QFile::copy refuses to replace an existing file.
    const auto exportPath = QString::fromLocal8Bit(argv[2]);
    if (!check(QFile::copy(postArchivePath, exportPath),
               "export verified POST-result MHTML for adapter fixture")
        || !check(QFile::setPermissions(exportPath, QFile::ReadOwner | QFile::WriteOwner),
                  "restrict exported MHTML fixture"))
        return 1;
    std::cout << "PASS: save routing and guarded archive; blocked HTTP attempts="
              << guard.attempts.load() << ", rejected navigations="
              << root->property("archiveRejectedNavigations").toInt() << '\n';
    return 0;
}
