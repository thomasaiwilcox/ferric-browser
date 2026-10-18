#pragma once
#include <QObject>
#include <QQuickItem>
#include <QtQml/qqmlregistration.h>

// Isolate the Qt 6.11.2 download view null-access compatibility guard.
// Request policy and captured profile/window ownership remain in the registry.
class FerricDownloadSource : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(FerricDownloadSource)
    Q_PROPERTY(bool supported READ supported CONSTANT)
  public:
    using QObject::QObject;
    bool supported() const;
    Q_INVOKABLE QQuickItem *viewFor(QObject *download) const;
};
