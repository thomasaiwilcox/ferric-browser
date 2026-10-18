#include "../../src/download_source.h"
#include <QDir>
#include <QGuiApplication>
#include <QPointer>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQuickWindow>
#include <QTcpServer>
#include <QTcpSocket>
#include <QTemporaryDir>
#include <QtTest>
#include <QtWebEngineQuick/QQuickWebEngineDownloadRequest>
#include <QtWebEngineQuick/QQuickWebEngineProfile>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include <memory>

class DownloadSourceTests final : public QObject {
    Q_OBJECT
  private slots:
    void unknownRequest() {
        FerricDownloadSource source;
        QVERIFY(source.supported());
        QObject unrelated;
        QCOMPARE(source.viewFor(nullptr), nullptr);
        QCOMPARE(source.viewFor(&unrelated), nullptr);
    }

    void sourceLifetime_data() {
        QTest::addColumn<bool>("privateProfile");
        QTest::addColumn<bool>("destroySource");
        QTest::newRow("normal-live") << false << false;
        QTest::newRow("normal-retired") << false << true;
        QTest::newRow("private-live") << true << false;
        QTest::newRow("private-retired") << true << true;
    }

    void sourceLifetime() {
        QFETCH(bool, privateProfile);
        QFETCH(bool, destroySource);
        QTemporaryDir directory(qEnvironmentVariable("XDG_DATA_HOME") + "/source-XXXXXX");
        QVERIFY(directory.isValid());
        // Chromium can finish releasing profile files after the QObject dies.
        // The process-owned environment removes them after QApplication exits.
        directory.setAutoRemove(false);
        QQmlEngine engine;
        QQmlComponent profiles(&engine);
        profiles.setData(R"qml(
            import QtQuick
            import QtWebEngine
            QtObject {
                id: profileSetup
                property string storageRoot
                property bool privateMode
                property var instance: null
                property WebEngineProfilePrototype prototype: WebEngineProfilePrototype {
                    storageName: profileSetup.privateMode ? "" : "owned-source-test"
                    persistentStoragePath: profileSetup.privateMode ? "" : profileSetup.storageRoot + "/storage"
                    cachePath: profileSetup.privateMode ? "" : profileSetup.storageRoot + "/cache"
                }
                Component.onCompleted: instance = prototype.instance()
            }
        )qml", QUrl("file:///owned-download-profile.qml"));
        std::unique_ptr<QObject> profileOwner(profiles.createWithInitialProperties({
            {"storageRoot", directory.path()}, {"privateMode", privateProfile}}));
        QVERIFY2(profileOwner, qPrintable(profiles.errorString()));
        auto profile = qobject_cast<QQuickWebEngineProfile *>(
            profileOwner->property("instance").value<QObject *>());
        QVERIFY(profile);
        QCOMPARE(profile->isOffTheRecord(), privateProfile);
        QTcpServer server;
        QVERIFY(server.listen(QHostAddress::LocalHost, 0));
        QPointer<QTcpSocket> held;
        connect(&server, &QTcpServer::newConnection, &server, [&] {
            while (auto socket = server.nextPendingConnection()) {
                connect(socket, &QTcpSocket::readyRead, socket,
                        [&, socket, bytes = QByteArray{}]() mutable {
                    bytes += socket->readAll();
                    if (!bytes.contains("\r\n\r\n")) return;
                    if (bytes.startsWith("GET /download.bin ")) held = socket;
                    else {
                        socket->write("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                        socket->disconnectFromHost();
                    }
                    bytes.clear();
                });
                connect(socket, &QTcpSocket::disconnected, socket, &QObject::deleteLater);
            }
        });
        const auto url = QUrl(QString("http://127.0.0.1:%1/download.bin").arg(server.serverPort()));
        FerricDownloadSource source;
        QVERIFY(source.supported());
        QQmlComponent component(&engine);
        component.setData(R"qml(
            import QtQuick
            import QtWebEngine
            WebEngineView {
                property url downloadUrl
                property bool ready: !loading && title === "Owned initiator"
                width: 640; height: 480; visible: false; enabled: false
                Component.onCompleted: loadHtml("<!doctype html><title>Owned initiator</title>", downloadUrl)
                function begin() {
                    runJavaScript("(()=>{const a=document.createElement('a');a.href="
                        + JSON.stringify(String(downloadUrl))
                        + ";a.download='owned.bin';document.body.append(a);a.click()})()",
                        WebEngineScript.ApplicationWorld)
                }
            }
        )qml", QUrl("file:///owned-download-source.qml"));
        std::unique_ptr<QObject> view(component.createWithInitialProperties({
            {"profile", QVariant::fromValue(profile)}, {"downloadUrl", url}}));
        QVERIFY2(view, qPrintable(component.errorString()));
        QQuickWindow window;
        auto item = qobject_cast<QQuickItem *>(view.get());
        QVERIFY(item);
        item->setParentItem(window.contentItem());
        window.show();
        QTRY_VERIFY_WITH_TIMEOUT(view->property("ready").toBool(), 10000);
        QPointer<QQuickItem> observed;
        int offers = 0;
        connect(profile, &QQuickWebEngineProfile::downloadRequested, profile,
                [&](QQuickWebEngineDownloadRequest *download) {
            observed = source.viewFor(download);
            ++offers;
            download->cancel();
        });
        QVERIFY(QMetaObject::invokeMethod(view.get(), "begin"));
        QTRY_VERIFY_WITH_TIMEOUT(held, 10000);
        if (destroySource) {
            QPointer<QObject> retired(view.get());
            view.reset();
            QVERIFY(retired.isNull());
        }
        // The real request has reached the server, but no headers were sent.
        // Release only after destruction so the source-less offer is causal.
        held->write("HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\n"
                    "Content-Disposition: attachment; filename=owned.bin\r\n"
                    "Content-Length: 4\r\nConnection: close\r\n\r\ndata");
        held->disconnectFromHost();
        QTRY_COMPARE_WITH_TIMEOUT(offers, 1, 10000);
        if (destroySource) QVERIFY(observed.isNull());
        else QCOMPARE(observed.data(), item);
        // No request was accepted and no output file should have been created.
        QVERIFY(!QFileInfo::exists(directory.filePath("owned.bin")));
        view.reset();
    }
};

int main(int argc, char **argv) {
    QTemporaryDir environment;
    if (!environment.isValid()) return 1;
    for (const auto key : {"XDG_CONFIG_HOME", "XDG_CACHE_HOME", "XDG_DATA_HOME",
                           "XDG_STATE_HOME", "XDG_RUNTIME_DIR"}) {
        const auto path = environment.filePath(QString::fromLatin1(key));
        if (!QDir().mkpath(path)) return 1;
        QFile::setPermissions(path, QFile::ReadOwner | QFile::WriteOwner | QFile::ExeOwner);
        qputenv(key, path.toUtf8());
    }
    QtWebEngineQuick::initialize();
    QGuiApplication application(argc, argv);
    DownloadSourceTests tests;
    return QTest::qExec(&tests, argc, argv);
}

#include "tst_download_source.moc"
