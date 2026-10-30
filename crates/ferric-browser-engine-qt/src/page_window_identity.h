#pragma once

#include <QObject>
#include <QPointer>
#include <QString>
#include <QWindow>
#include <QtQml/qqmlregistration.h>

// A mapping-time correlation marker for an application-created window. The
// compositor adapter must still corroborate exact ownership and stable ID.
class FerricPageWindowIdentity : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(FerricPageWindowIdentity)
    Q_PROPERTY(QWindow *targetWindow READ window WRITE setWindow NOTIFY windowChanged)
    Q_PROPERTY(QString tag READ tag CONSTANT)
    Q_PROPERTY(bool prepared READ prepared NOTIFY preparedChanged)
    Q_PROPERTY(bool tagRequested READ tagRequested NOTIFY preparedChanged)
public:
    explicit FerricPageWindowIdentity(QObject *parent = nullptr);
    QWindow *window() const;
    void setWindow(QWindow *window);
    QString tag() const;
    bool prepared() const;
    bool tagRequested() const;
signals:
    void windowChanged();
    void preparedChanged();
private:
    QPointer<QWindow> window_;
    QString tag_;
    bool prepared_ = false;
    bool tagRequested_ = false;
};
