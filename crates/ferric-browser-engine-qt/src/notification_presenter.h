#ifndef FERRIC_BROWSER_NOTIFICATION_PRESENTER_H
#define FERRIC_BROWSER_NOTIFICATION_PRESENTER_H

#include <QtCore/QDeadlineTimer>
#include <QtCore/QMap>
#include <QtCore/QObject>
#include <QtCore/QPointer>
#include <QtCore/QTimer>
#include <QtDBus/QDBusMessage>
#include <QtQml/qqmlregistration.h>

class FerricBrowserNotificationPresenter : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(NotificationPresenter)

public:
    enum class Transport { Desktop, Portal };
    struct Message {
        QString profileScope;
        QString origin;
        QString title;
        QString body;
        Transport transport = Transport::Desktop;
    };

    explicit FerricBrowserNotificationPresenter(QObject *parent = nullptr);
    ~FerricBrowserNotificationPresenter() override;

    Q_INVOKABLE bool present(QObject *notification, const QString &profileScope,
                             const QString &origin, bool requiredPortal = false);
    // Transport owns delivery; the caller owns show/close/click on its client.
    // This C++ boundary permits protocol tests without a WebEngine renderer.
    bool enqueue(QObject *lifetime, const Message &message);
    Q_INVOKABLE bool cancel(QObject *notification);
    Q_INVOKABLE void closeAll();

signals:
    void notificationPresented(QObject *notification, const QString &profileScope,
                               const QString &origin);
    void notificationFinished(QObject *notification);
    void notificationClicked(QObject *notification, const QString &profileScope,
                             const QString &origin);
    void notificationUnavailable(QObject *notification, const QString &profileScope,
                                 const QString &origin);

private slots:
    void actionInvoked(uint id, const QString &action, const QDBusMessage &message);
    void notificationClosed(uint id, uint reason, const QDBusMessage &message);
    void portalActionInvoked(const QString &id, const QString &action,
                             const QVariantList &parameters, const QDBusMessage &message);

private:
    enum class Phase { Discovering, Sending, Active };
    struct Entry {
        QPointer<QObject> client;
        Message message;
        QString serviceOwner;
        QString portalId;
        uint desktopId = 0;
        quint64 generation = 0;
        Phase phase = Phase::Discovering;
        bool cancelled = false;
        QDeadlineTimer deadline;
        QMetaObject::Connection destroyedConnection;
        QMetaObject::Connection closedConnection;
    };

    void resolveService(quint64 token, bool canActivate = true);
    void discover(quint64 token);
    void send(quint64 token, const QStringList &capabilities);
    void acknowledge(quint64 token, const QDBusMessage &reply);
    void cancelToken(quint64 token, bool unavailable = false);
    void finish(quint64 token, bool unavailable = false);
    void closeRemote(const Entry &entry);
    void serviceChanged(Transport transport);
    void expirePending();
    quint64 generation(Transport transport) const;
    quint64 tokenForRemote(const QString &owner, uint id) const;
    void updateTimer();

    QMap<quint64, Entry> entries_;
    QTimer pendingTimer_;
    quint64 nextToken_ = 0;
    quint64 desktopGeneration_ = 0;
    quint64 portalGeneration_ = 0;
};

#endif
