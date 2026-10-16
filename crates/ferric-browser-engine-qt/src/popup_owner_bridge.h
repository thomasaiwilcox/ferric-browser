#pragma once
#include <QObject>
#include <QString>
#include <QtQml/qqmlregistration.h>

// Converts QML QObject references into checked, non-aliasing native owners.
// Runtime/profile admission and transfer remain at the typed Rust boundary.
class FerricPopupOwnerBridge : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(FerricPopupOwnerBridge)
  public:
    using QObject::QObject;
    Q_INVOKABLE bool initialize(QObject *source, QObject *destination,
                                const QString &token);
    Q_INVOKABLE bool inheritProfile(QObject *source, QObject *destination,
                                   const QString &profileName);
};
