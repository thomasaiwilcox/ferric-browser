#ifndef FERRIC_BROWSER_PAGE_POINTER_ADAPTER_H
#define FERRIC_BROWSER_PAGE_POINTER_ADAPTER_H

#include <QtCore/QPointer>
#include <QtCore/QQueue>
#include <QtCore/QSet>
#include <QtCore/QTimer>
#include <QtCore/QObject>
#include <QtGui/QWindow>
#include <QtQuick/QQuickItem>
#include <QtQml/qqmlregistration.h>

class FerricPagePointerAdapter : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(FerricPagePointerAdapter)
    Q_PROPERTY(bool enabled READ enabled WRITE setEnabled NOTIFY enabledChanged)
    Q_PROPERTY(QQuickItem *targetItem READ targetItem WRITE setTargetItem NOTIFY targetItemChanged)
    Q_PROPERTY(QWindow *targetWindow READ targetWindow WRITE setTargetWindow NOTIFY targetWindowChanged)
    Q_PROPERTY(bool inputBlocked READ inputBlocked WRITE setInputBlocked NOTIFY inputBlockedChanged)
    Q_PROPERTY(QString surfaceSerial READ surfaceSerial NOTIFY surfaceChanged)
    Q_PROPERTY(QString surfaceRevision READ surfaceRevision NOTIFY surfaceChanged)

public:
    explicit FerricPagePointerAdapter(QObject *parent = nullptr);
    ~FerricPagePointerAdapter() override;

    bool enabled() const { return enabled_; }
    void setEnabled(bool enabled);
    QQuickItem *targetItem() const { return targetItem_.data(); }
    void setTargetItem(QQuickItem *item);
    QWindow *targetWindow() const { return targetWindow_.data(); }
    void setTargetWindow(QWindow *window);
    bool inputBlocked() const { return inputBlocked_; }
    void setInputBlocked(bool blocked);
    QString surfaceSerial() const { return QString::number(surfaceSerial_); }
    QString surfaceRevision() const { return QString::number(surfaceRevision_); }

    Q_INVOKABLE void dispatch(const QString &requestId,
                              const QString &sessionId,
                              const QString &serial,
                              const QString &revision,
                              double x,
                              double y,
                              const QString &action);

signals:
    void enabledChanged();
    void targetItemChanged();
    void targetWindowChanged();
    void inputBlockedChanged();
    void surfaceChanged();
    void dispatchAcknowledged(const QString &requestId,
                              const QString &sessionId,
                              const QString &serial,
                              const QString &revision,
                              const QString &outcome);
    void invalidated(const QString &reason);
    void physicalPointerDetected(const QString &reason);

protected:
    bool eventFilter(QObject *watched, QEvent *event) override;

private:
    void advanceSurface(bool targetChanged, const QString &reason);
    bool validTarget() const;
    void acknowledge(const QString &requestId,
                     const QString &sessionId,
                     const QString &serial,
                     const QString &revision,
                     const QString &outcome);

    bool enabled_ = false;
    bool inputBlocked_ = false;
    bool dispatching_ = false;
    bool surfaceExhausted_ = false;
    quint64 surfaceSerial_ = 1;
    quint64 surfaceRevision_ = 1;
    QPointer<QQuickItem> targetItem_;
    QPointer<QWindow> targetWindow_;
    QSet<QString> consumedRequestIds_;
    QQueue<QString> consumedRequestOrder_;
    QSet<int> consumedButtons_;
    bool consumedTabletSequence_ = false;
    bool consumedTouchSequence_ = false;
    QString consumedSessionId_;
    QTimer dispatchWatchdog_;
    QString pendingRequestId_;
    QString pendingSessionId_;
};

#endif
