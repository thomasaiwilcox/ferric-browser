#include <QtCore/QCoreApplication>
#include <QtCore/QDir>
#include <QtCore/QElapsedTimer>
#include <QtCore/QFile>
#include <QtCore/QFileInfo>
#include <QtCore/QTemporaryDir>
#include <QtCore/QThread>
#include <QtCore/QUrl>
#include <QtGui/QGuiApplication>
#include <QtNetwork/QHostAddress>
#include <QtNetwork/QTcpServer>
#include <QtNetwork/QTcpSocket>
#include <QtQml/QQmlApplicationEngine>
#include <QtQml/QQmlContext>
#include <QtWebEngineCore/QWebEngineDownloadRequest>
#include <QtWebEngineQuick/QQuickWebEngineDownloadRequest>
#include <QtWebEngineQuick/QQuickWebEngineProfile>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>

#if QT_CONFIG(webengine_extensions)
#include <QtWebEngineCore/QWebEngineExtensionInfo>
#include <QtWebEngineCore/QWebEngineExtensionManager>
#include <QtCore/QUrlQuery>
#endif

#include <functional>
#include <iostream>

namespace {
bool waitUntil(QGuiApplication &app, const std::function<bool()> &predicate,
               int timeoutMs = 15000)
{
    QElapsedTimer elapsed;
    elapsed.start();
    while (!predicate() && elapsed.elapsed() < timeoutMs) {
        app.processEvents();
        QThread::msleep(10);
    }
    return predicate();
}

bool check(bool condition, const char *message)
{
    if (!condition)
        std::cerr << "FAIL: " << message << '\n';
    return condition;
}
}

int main(int argc, char **argv)
{
    QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
    QGuiApplication app(argc, argv);
    QtWebEngineQuick::initialize();
    if (!check(argc == 2, "provide the disposable viewless-download QML fixture"))
        return 2;

    // Qt WebEngine 6.10.3 and 6.11.2 both crash in internal engine code while
    // enabling this disposable extension, before its popup can be opened.
    // Keep the test stimulus from crashing the runner; production capability
    // remains unqualified and disabled.
#if QT_VERSION >= QT_VERSION_CHECK(6, 10, 3) && QT_VERSION < QT_VERSION_CHECK(6, 12, 0)
    std::cout << "SKIP: Qt WebEngine extension activation is not safe to qualify "
                 "on the tested 6.10.3/6.11 runtimes\n";
    return 77;
#endif

#if !QT_CONFIG(webengine_extensions)
    std::cout << "SKIP: Qt was built without its public WebEngine extension manager\n";
    return 77;
#else
    QTemporaryDir operation;
    if (!check(operation.isValid(), "create disposable profile/download directory"))
        return 2;
    const auto storagePath = operation.filePath("profile/storage");
    const auto cachePath = operation.filePath("profile/cache");
    if (!check(QDir().mkpath(storagePath) && QDir().mkpath(cachePath),
               "create disposable persistent profile paths"))
        return 2;

    QTcpServer server;
    if (!check(server.listen(QHostAddress::LocalHost), "listen on disposable loopback port"))
        return 2;
    int downloadRequestsServed = 0;
    const QByteArray payload("native-viewless-download-payload\n");
    QObject::connect(&server, &QTcpServer::newConnection, &app, [&] {
        while (auto *socket = server.nextPendingConnection()) {
            QObject::connect(socket, &QTcpSocket::disconnected,
                             socket, &QObject::deleteLater);
            QObject::connect(socket, &QTcpSocket::readyRead, socket,
                             [socket, &downloadRequestsServed, payload] {
                const auto request = socket->readAll();
                if (request.isEmpty())
                    return;
                if (socket->property("ferric-viewless-replied").toBool())
                    return;
                socket->setProperty("ferric-viewless-replied", true);
                if (request.startsWith("GET /viewless-download "))
                    ++downloadRequestsServed;
                const QByteArray response = QByteArray("HTTP/1.1 200 OK\r\n")
                    + "Content-Type: application/octet-stream\r\n"
                    + "Content-Disposition: attachment; filename=\"viewless.bin\"\r\n"
                    + "Content-Length: " + QByteArray::number(payload.size()) + "\r\n"
                    + "Connection: close\r\n\r\n" + payload;
                socket->write(response);
                socket->disconnectFromHost();
            });
        }
    });

    const QUrl downloadUrl(QStringLiteral("http://127.0.0.1:%1/viewless-download")
                               .arg(server.serverPort()));
    QQmlApplicationEngine engine;
    engine.rootContext()->setContextProperty("viewlessProbeStoragePath", storagePath);
    engine.rootContext()->setContextProperty("viewlessProbeCachePath", cachePath);
    engine.rootContext()->setContextProperty("viewlessProbeOutputDirectory", operation.path());
    engine.rootContext()->setContextProperty("viewlessProbeDownloadUrl", downloadUrl.toString());
    engine.load(QUrl::fromLocalFile(QFileInfo(QString::fromLocal8Bit(argv[1])).absoluteFilePath()));
    if (!check(engine.rootObjects().size() == 1, "load viewless-download QML fixture"))
        return 2;
    auto *root = engine.rootObjects().first();
    auto *profile = qobject_cast<QQuickWebEngineProfile *>(
        root->property("profileForProbe").value<QObject *>());
    if (!check(profile && !profile->isOffTheRecord(),
               "create a named, non-off-the-record Quick profile"))
        return 2;
    auto *manager = profile->extensionManager();
    if (!check(manager != nullptr, "obtain public profile extension manager"))
        return 2;

    int nativeDownloadSignals = 0;
    bool nativeDownloadHasNoPage = false;
    QObject::connect(profile, &QQuickWebEngineProfile::downloadRequested, &app,
                     [&](QQuickWebEngineDownloadRequest *download) {
        if (!download || download->url() != downloadUrl || download->isSavePageDownload())
            return;
        ++nativeDownloadSignals;
        nativeDownloadHasNoPage = download->page() == nullptr;
    });

    const auto extensionPath = QFileInfo(QString::fromLocal8Bit(argv[1]))
                                   .absoluteDir()
                                   .filePath("viewless_download_extension");
    bool extensionLoadFinished = false;
    bool extensionLoadFailed = false;
    QObject::connect(manager, &QWebEngineExtensionManager::loadFinished, &app,
                     [&](const QWebEngineExtensionInfo &extension) {
        if (extension.name() != u"Ferric Viewless Download Probe")
            return;
        extensionLoadFinished = true;
        extensionLoadFailed = !extension.isLoaded();
    });
    manager->loadExtension(extensionPath);
    if (!check(waitUntil(app, [&] { return extensionLoadFinished; }),
               "load the disposable Manifest V3 extension"))
        return 1;
    if (!check(!extensionLoadFailed,
               "Qt accepted the disposable extension (dynamic diagnostics suppressed)")) {
        return 1;
    }

    QWebEngineExtensionInfo probeExtension;
    for (const auto &extension : manager->extensions()) {
        if (extension.name() == u"Ferric Viewless Download Probe")
            probeExtension = extension;
    }
    if (!check(probeExtension.isLoaded() && !probeExtension.id().isEmpty(),
               "loaded extension has a runtime identity"))
        return 1;
    manager->setExtensionEnabled(probeExtension, true);
    if (!check(waitUntil(app, [&] {
            for (const auto &extension : manager->extensions()) {
                if (extension.id() == probeExtension.id())
                    return extension.isEnabled();
            }
            return false;
        }), "enable the test-only extension"))
        return 1;
    QUrl popupUrl = probeExtension.actionPopupUrl();
    if (!check(popupUrl.isValid() && popupUrl.scheme() == u"chrome-extension",
               "resolve the extension's public action popup URL"))
        return 1;
    QUrlQuery query;
    query.addQueryItem(QStringLiteral("downloadUrl"), downloadUrl.toString());
    popupUrl.setQuery(query);
    if (!check(QMetaObject::invokeMethod(root, "openExtensionPopup",
                                         Q_ARG(QVariant, QVariant(popupUrl.toString()))),
               "open the extension action popup on the same Quick profile"))
        return 1;
    if (!check(waitUntil(app, [&] { return root->property("sourceLoaded").toBool()
                                       || !root->property("sourceFailure").toString().isEmpty(); }),
               "load extension popup"))
        return 1;
    if (!check(root->property("sourceFailure").toString().isEmpty(),
               "extension popup load succeeded"))
        return 1;

    if (!check(waitUntil(app, [&] {
            return root->property("viewlessDownloadFinished").toInt() == 1;
        }, 20000), "extension service worker produced and finished a profile download"))
        return 1;

    const auto outputPath = operation.filePath("viewless.bin");
    QFile output(outputPath);
    if (!check(output.open(QIODevice::ReadOnly), "read the routed profile download"))
        return 1;
    const auto downloadedBytes = output.readAll();
    const bool exactOwnerRoute = root->property("viewlessRoutedToExactProfileOwner").toBool()
        && root->property("viewlessFinishedWithExactOwner").toBool();
    const bool completed = root->property("viewlessDownloadState").toInt()
        == QWebEngineDownloadRequest::DownloadCompleted;
    if (!check(nativeDownloadSignals == 1 && nativeDownloadHasNoPage
                   && root->property("viewlessDownloadRequests").toInt() == 1,
               "one genuine native profile request has no originating QWebEnginePage")
        || !check(exactOwnerRoute,
                  "request and terminal callback retained the exact profile owner")
        || !check(completed && downloadedBytes == payload && downloadRequestsServed == 1,
                  "routed download completed with the exact local payload")
        || !check(!root->property("viewlessRequestRefused").toBool(),
                  "profile-owner route did not refuse the request"))
        return 1;

    std::cout << "FERRIC_VIEWLESS_DOWNLOAD_RESULT:1/1; native page is null; "
                 "exact profile owner retained through terminal callback\n";
    return 0;
#endif
}
