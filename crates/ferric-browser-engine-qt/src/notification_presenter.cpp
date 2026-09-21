#include "notification_presenter.h"

#include <QtDBus/QDBusConnection>
#include <QtDBus/QDBusMessage>
#include <QtDBus/QDBusPendingCallWatcher>
#include <QtCore/QVariantMap>

FerricBrowserNotificationPresenter::FerricBrowserNotificationPresenter(QObject *parent)
    : QObject(parent)
{
    auto bus = QDBusConnection::sessionBus();
    bus.connect(QStringLiteral("org.freedesktop.Notifications"),
                QStringLiteral("/org/freedesktop/Notifications"),
                QStringLiteral("org.freedesktop.Notifications"),
                QStringLiteral("ActionInvoked"), this,
                SLOT(actionInvoked(uint,QString)));
    bus.connect(QStringLiteral("org.freedesktop.Notifications"),
                QStringLiteral("/org/freedesktop/Notifications"),
                QStringLiteral("org.freedesktop.Notifications"),
                QStringLiteral("NotificationClosed"), this,
                SLOT(notificationClosed(uint,uint)));
}

bool FerricBrowserNotificationPresenter::present(QObject *object,
                                               const QString &profileScope,
                                               const QString &origin)
{
    auto *notification = qobject_cast<QWebEngineNotification *>(object);
    auto bus = QDBusConnection::sessionBus();
    if (!notification || !bus.isConnected())
        return false;

    QPointer<QWebEngineNotification> guardedNotification(notification);

    QDBusMessage message = QDBusMessage::createMethodCall(
        QStringLiteral("org.freedesktop.Notifications"),
        QStringLiteral("/org/freedesktop/Notifications"),
        QStringLiteral("org.freedesktop.Notifications"), QStringLiteral("Notify"));
    QVariantList arguments;
    arguments << QStringLiteral("FerricBrowser") << uint(0) << QString()
              << notification->title() << notification->message()
              << QStringList{QStringLiteral("default"), QStringLiteral("Open")}
              << QVariantMap{} << -1;
    message.setArguments(arguments);
    auto *watcher = new QDBusPendingCallWatcher(bus.asyncCall(message), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this,
            [this, watcher, guardedNotification, profileScope, origin]() {
                const QDBusMessage reply = watcher->reply();
                watcher->deleteLater();
                if (reply.type() == QDBusMessage::ErrorMessage
                    || reply.arguments().isEmpty()) {
                    if (guardedNotification)
                        emit notificationUnavailable(guardedNotification.data(),
                                                     profileScope, origin);
                    return;
                }
                const uint id = reply.arguments().constFirst().toUInt();
                if (!guardedNotification)
                    return;
                entries_.insert(id, Entry{guardedNotification, profileScope, origin});
                connect(guardedNotification, &QWebEngineNotification::closed, this,
                        [this, id]() { entries_.remove(id); });
            });
    if (!bus.isConnected())
        return false;
    return true;
}

void FerricBrowserNotificationPresenter::closeAll()
{
    const auto entries = entries_;
    entries_.clear();
    auto bus = QDBusConnection::sessionBus();
    for (auto it = entries.cbegin(); it != entries.cend(); ++it) {
        QDBusMessage message = QDBusMessage::createMethodCall(
            QStringLiteral("org.freedesktop.Notifications"),
            QStringLiteral("/org/freedesktop/Notifications"),
            QStringLiteral("org.freedesktop.Notifications"),
            QStringLiteral("CloseNotification"));
        message.setArguments({it.key()});
        bus.call(message, QDBus::NoBlock);
        if (it->notification)
            it->notification->close();
    }
}

void FerricBrowserNotificationPresenter::actionInvoked(uint id, const QString &action)
{
    if (action != QStringLiteral("default"))
        return;
    const auto it = entries_.constFind(id);
    if (it == entries_.cend() || !it->notification)
        return;
    it->notification->click();
    emit notificationClicked(it->profileScope, it->origin);
}

void FerricBrowserNotificationPresenter::notificationClosed(uint id, uint)
{
    const auto it = entries_.find(id);
    if (it != entries_.end()) {
        if (it->notification)
            it->notification->close();
        entries_.erase(it);
    }
}
