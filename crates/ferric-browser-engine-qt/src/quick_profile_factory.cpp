#include "quick_profile_factory.h"
#include "profile_tls.h"
#include <QQmlComponent>
#include <QQmlEngine>
#include <QThread>
#include <QDir>
#include <utility>
#include <QtWebEngineQuick/QQuickWebEngineProfile>

QObject *FerricQuickProfileFactory::instance()
{
    if (requested_) return profile_;
    requested_ = true;
    auto *engine = qmlEngine(this);
    if (!engine || engine->thread() != QThread::currentThread() || permissionsPolicy_ < 0 || permissionsPolicy_ > 2) return nullptr;
    QStringList certificateFiles;
    for (const auto &url : std::as_const(trustFiles_)) {
        const auto path = url.isLocalFile() ? url.toLocalFile() : url.scheme().isEmpty() ? url.path() : QString();
        if (path.isEmpty() || !QDir::isAbsolutePath(path) || !url.host().isEmpty()
            || !url.userInfo().isEmpty() || url.hasQuery() || url.hasFragment()) return nullptr;
        certificateFiles.append(path);
    }
    QQmlComponent component(engine);
    component.setData("import QtWebEngine; WebEngineProfilePrototype {}", QUrl("qrc:/ferric-native-profile-prototype.qml"));
    auto *prototype = component.createWithInitialProperties({
        {QStringLiteral("storageName"), storageName_},
        {QStringLiteral("persistentStoragePath"), storagePath_},
        {QStringLiteral("cachePath"), cachePath_},
        {QStringLiteral("persistentPermissionsPolicy"), permissionsPolicy_},
        {QStringLiteral("additionalTrustedCertificateFiles"), certificateFiles}
    });
    if (!prototype) return nullptr;
    QQmlEngine::setObjectOwnership(prototype, QQmlEngine::CppOwnership);
    prototype->setParent(this);
    QQuickWebEngineProfile *profile = nullptr;
    if (!QMetaObject::invokeMethod(prototype, "instance", Qt::DirectConnection, Q_RETURN_ARG(QQuickWebEngineProfile *, profile))
        || !profile || !FerricProfileTls::createFresh(profile)) {
        delete prototype;
        return nullptr;
    }
    // The hidden Qt prototype retains its scoped ownership. It was inserted
    // first among our children, so destroying it releases the profile before
    // QObject encounters the profile's former child slot.
    profile->setParent(this);
    QQmlEngine::setObjectOwnership(profile, QQmlEngine::CppOwnership);
    prototype_ = prototype;
    profile_ = profile;
    return profile;
}
