#ifndef FERRIC_BROWSER_NOTIFICATION_PRESENTER_H
#define FERRIC_BROWSER_NOTIFICATION_PRESENTER_H

#include <QtCore/QHash>
#include <QtCore/QObject>
#include <QtCore/QPointer>
#include <QtQml/qqmlregistration.h>
#include <QtWebEngineCore/QWebEngineNotification>

class FerricBrowserNotificationPresenter : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(NotificationPresenter)

public:
    explicit FerricBrowserNotificationPresenter(QObject *parent = nullptr);

    Q_INVOKABLE bool present(QObject *notification, const QString &profileScope,
                              const QString &origin);
    Q_INVOKABLE void closeAll();

signals:
    void notificationClicked(const QString &profileScope, const QString &origin);
    void notificationUnavailable(QObject *notification, const QString &profileScope,
                                 const QString &origin);

private slots:
    void actionInvoked(uint id, const QString &action);
    void notificationClosed(uint id, uint reason);

private:
    struct Entry {
        QPointer<QWebEngineNotification> notification;
        QString profileScope;
        QString origin;
    };

    QHash<uint, Entry> entries_;
    uint nextId_ = 1;
};

#endif
