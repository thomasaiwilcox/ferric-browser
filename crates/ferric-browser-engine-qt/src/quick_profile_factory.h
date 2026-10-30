#pragma once
#include <QObject>
#include <QPointer>
#include <QString>
#include <QUrl>
#include <QtQml/qqmlregistration.h>

// Creates the underlying documented Qt prototype privately, establishes
// native TLS observation at actual profile birth, then publishes the profile.
class FerricQuickProfileFactory : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(FerricProfilePrototype)
    Q_PROPERTY(QString storageName MEMBER storageName_)
    Q_PROPERTY(QString persistentStoragePath MEMBER storagePath_)
    Q_PROPERTY(QString cachePath MEMBER cachePath_)
    Q_PROPERTY(int persistentPermissionsPolicy MEMBER permissionsPolicy_)
    Q_PROPERTY(QList<QUrl> additionalTrustedCertificateFiles MEMBER trustFiles_)
public:
    using QObject::QObject;
    Q_INVOKABLE QObject *instance();
private:
    QString storageName_, storagePath_, cachePath_;
    int permissionsPolicy_ = 0;
    QList<QUrl> trustFiles_;
    bool requested_ = false;
    QPointer<QObject> prototype_, profile_;
};
