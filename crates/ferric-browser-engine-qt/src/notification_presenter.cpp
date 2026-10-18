#include "notification_presenter.h"

#include <QtCore/QUuid>
#include <QtCore/QVariantMap>
#include <QtDBus/QDBusConnection>
#include <QtDBus/QDBusPendingCallWatcher>
#include <QtDBus/QDBusPendingReply>
#include <QtDBus/QDBusServiceWatcher>
#include <QtDBus/QDBusVariant>
#include <QtWebEngineCore/QWebEngineNotification>
#include <utility>

namespace {
constexpr int PendingLimit = 32;
constexpr int ActiveLimit = 128;
constexpr int DeliveryTimeoutMs = 5000;
constexpr int BusTimeoutMs = 25000;
const QString DesktopService = QStringLiteral("org.freedesktop.Notifications");
const QString DesktopPath = QStringLiteral("/org/freedesktop/Notifications");
const QString PortalService = QStringLiteral("org.freedesktop.portal.Desktop");
const QString PortalPath = QStringLiteral("/org/freedesktop/portal/desktop");
const QString PortalInterface = QStringLiteral("org.freedesktop.portal.Notification");

QDBusMessage method(const QString &owner, bool portal, const QString &name)
{
    return QDBusMessage::createMethodCall(owner, portal ? PortalPath : DesktopPath,
                                          portal ? PortalInterface : DesktopService, name);
}
}

FerricBrowserNotificationPresenter::FerricBrowserNotificationPresenter(QObject *parent)
    : QObject(parent)
{
    auto bus = QDBusConnection::sessionBus();
    bus.connect(QString(), DesktopPath, DesktopService, QStringLiteral("ActionInvoked"),
                this, SLOT(actionInvoked(uint,QString,QDBusMessage)));
    bus.connect(QString(), DesktopPath, DesktopService, QStringLiteral("NotificationClosed"),
                this, SLOT(notificationClosed(uint,uint,QDBusMessage)));
    bus.connect(QString(), PortalPath, PortalInterface, QStringLiteral("ActionInvoked"),
                this, SLOT(portalActionInvoked(QString,QString,QVariantList,QDBusMessage)));
    auto *services = new QDBusServiceWatcher(this);
    services->setConnection(bus);
    services->setWatchMode(QDBusServiceWatcher::WatchForOwnerChange);
    services->addWatchedService(DesktopService);
    services->addWatchedService(PortalService);
    connect(services, &QDBusServiceWatcher::serviceOwnerChanged, this,
            [this](const QString &service, const QString &oldOwner, const QString &) {
                // Initial bus activation is valid. Replacing an existing daemon
                // invalidates its IDs and outstanding delivery attempts.
                if (!oldOwner.isEmpty())
                    serviceChanged(service == DesktopService ? Transport::Desktop : Transport::Portal);
            });
    pendingTimer_.setInterval(100);
    connect(&pendingTimer_, &QTimer::timeout, this,
            &FerricBrowserNotificationPresenter::expirePending);
}

FerricBrowserNotificationPresenter::~FerricBrowserNotificationPresenter()
{
    closeAll();
}

bool FerricBrowserNotificationPresenter::present(QObject *object, const QString &profileScope,
                                               const QString &origin, bool requiredPortal)
{
    auto *notification = qobject_cast<QWebEngineNotification *>(object);
    if (!notification)
        return false;
    if (!enqueue(notification, Message{profileScope, origin, notification->title(),
                 notification->message(), requiredPortal ? Transport::Portal : Transport::Desktop}))
        return false;
    const auto token = nextToken_;
    entries_[token].closedConnection = connect(notification, &QWebEngineNotification::closed,
                                               this, [this, token]() { cancelToken(token); });
    return true;
}

bool FerricBrowserNotificationPresenter::enqueue(QObject *client, const Message &message)
{
    if (!client || message.profileScope.isEmpty() || message.origin.isEmpty()
            || !QDBusConnection::sessionBus().isConnected())
        return false;
    int pending = 0;
    for (const auto &entry : std::as_const(entries_)) {
        if (entry.client == client)
            return false;
        if (entry.phase != Phase::Active)
            ++pending;
    }
    // Cancelled sends retain their slot until the reply is consumed. Otherwise
    // repeated cancellation could create an unbounded number of bus watchers.
    if (pending >= PendingLimit)
        return false;
    const auto token = ++nextToken_;
    Entry entry;
    entry.client = client;
    entry.message = message;
    entry.generation = generation(message.transport);
    entry.portalId = QStringLiteral("ferric-") + QUuid::createUuid().toString(QUuid::WithoutBraces);
    entry.deadline = QDeadlineTimer(DeliveryTimeoutMs);
    entry.destroyedConnection = connect(client, &QObject::destroyed, this,
                                        [this, token]() { cancelToken(token); });
    entries_.insert(token, entry);
    updateTimer();
    resolveService(token);
    return true;
}

void FerricBrowserNotificationPresenter::resolveService(quint64 token, bool canActivate)
{
    const auto it = entries_.constFind(token);
    if (it == entries_.cend()) return;
    const auto service = it->message.transport == Transport::Portal ? PortalService : DesktopService;
    auto request = QDBusMessage::createMethodCall(QStringLiteral("org.freedesktop.DBus"),
        QStringLiteral("/org/freedesktop/DBus"), QStringLiteral("org.freedesktop.DBus"),
        QStringLiteral("GetNameOwner"));
    request.setArguments({service});
    auto *watcher = new QDBusPendingCallWatcher(
        QDBusConnection::sessionBus().asyncCall(request, BusTimeoutMs), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this,
            [this, token, watcher, canActivate, service]() {
        const QDBusPendingReply<QString> owner(*watcher);
        watcher->deleteLater();
        auto it = entries_.find(token);
        if (it == entries_.end()) return;
        if (it->cancelled) { finish(token); return; }
        if (owner.isError() && canActivate
                && owner.error().name() == QStringLiteral("org.freedesktop.DBus.Error.NameHasNoOwner")) {
            auto request = QDBusMessage::createMethodCall(QStringLiteral("org.freedesktop.DBus"),
                QStringLiteral("/org/freedesktop/DBus"), QStringLiteral("org.freedesktop.DBus"),
                QStringLiteral("StartServiceByName"));
            request.setArguments({service, uint(0)});
            auto *activation = new QDBusPendingCallWatcher(
                QDBusConnection::sessionBus().asyncCall(request, BusTimeoutMs), this);
            connect(activation, &QDBusPendingCallWatcher::finished, this, [this, token, activation]() {
                const QDBusPendingReply<uint> reply(*activation);
                activation->deleteLater();
                const auto it = entries_.constFind(token);
                if (it == entries_.cend()) return;
                if (it->cancelled) finish(token);
                else if (reply.isError() || (reply.value() != 1 && reply.value() != 2)) finish(token, true);
                else resolveService(token, false);
            });
            return;
        }
        if (owner.isError() || !owner.value().startsWith(QLatin1Char(':'))
                || it->generation != generation(it->message.transport)) {
            finish(token, true);
            return;
        }
        // Qt doesn't retain sender metadata on method replies. Resolve and pin
        // the unique name before capability discovery and delivery instead.
        it->serviceOwner = owner.value();
        discover(token);
    });
}

void FerricBrowserNotificationPresenter::discover(quint64 token)
{
    const bool portal = entries_[token].message.transport == Transport::Portal;
    const auto owner = entries_[token].serviceOwner;
    auto request = portal ? QDBusMessage::createMethodCall(owner, PortalPath,
                      QStringLiteral("org.freedesktop.DBus.Properties"), QStringLiteral("Get"))
                          : method(owner, false, QStringLiteral("GetCapabilities"));
    if (portal)
        request.setArguments({PortalInterface, QStringLiteral("version")});
    auto *watcher = new QDBusPendingCallWatcher(
        QDBusConnection::sessionBus().asyncCall(request, BusTimeoutMs), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this, token, watcher, portal]() {
        const auto reply = watcher->reply();
        watcher->deleteLater();
        auto it = entries_.find(token);
        if (it == entries_.end())
            return;
        if (it->cancelled) {
            finish(token);
            return;
        }
        QStringList capabilities;
        bool valid = reply.type() == QDBusMessage::ReplyMessage;
        if (portal) {
            const QDBusPendingReply<QDBusVariant> typed(reply);
            valid = valid && !typed.isError()
                && typed.value().variant().metaType() == QMetaType::fromType<uint>()
                && typed.value().variant().toUInt() >= 1;
        } else {
            const QDBusPendingReply<QStringList> typed(reply);
            valid = valid && !typed.isError();
            capabilities = typed.value();
        }
        if (!valid || it->generation != generation(it->message.transport)) {
            finish(token, true);
            return;
        }
        send(token, capabilities);
    });
}

void FerricBrowserNotificationPresenter::send(quint64 token, const QStringList &capabilities)
{
    auto &entry = entries_[token];
    const bool portal = entry.message.transport == Transport::Portal;
    auto request = method(entry.serviceOwner, portal,
                          portal ? QStringLiteral("AddNotification") : QStringLiteral("Notify"));
    if (portal) {
        // Version 1's body is plain text; markup-body is intentionally unused.
        request.setArguments({entry.portalId, QVariantMap{
            {QStringLiteral("title"), entry.message.title},
            {QStringLiteral("body"), entry.message.body},
            {QStringLiteral("default-action"), QStringLiteral("default")}}});
    } else {
        const auto body = capabilities.contains(QStringLiteral("body-markup"))
            ? entry.message.body.toHtmlEscaped() : entry.message.body;
        request.setArguments({QStringLiteral("Ferric Browser"), uint(0), QString(),
            entry.message.title, body,
            QStringList{QStringLiteral("default"), QStringLiteral("Open")}, QVariantMap{}, -1});
    }
    entry.phase = Phase::Sending;
    auto *watcher = new QDBusPendingCallWatcher(
        QDBusConnection::sessionBus().asyncCall(request, BusTimeoutMs), this);
    connect(watcher, &QDBusPendingCallWatcher::finished, this, [this, token, watcher]() {
        const auto reply = watcher->reply();
        watcher->deleteLater();
        acknowledge(token, reply);
    });
}

void FerricBrowserNotificationPresenter::acknowledge(quint64 token, const QDBusMessage &reply)
{
    auto it = entries_.find(token);
    if (it == entries_.end())
        return;
    const bool portal = it->message.transport == Transport::Portal;
    bool valid;
    if (portal) {
        const QDBusPendingReply<> typed(reply);
        valid = !typed.isError();
    } else {
        const QDBusPendingReply<uint> typed(reply);
        valid = !typed.isError() && typed.value() != 0;
        if (valid)
            it->desktopId = typed.value();
    }
    if (!valid || it->cancelled || !it->client
            || it->generation != generation(it->message.transport) || it->deadline.hasExpired()) {
        closeRemote(*it);
        finish(token, !it->cancelled);
        return;
    }
    int active = 0;
    quint64 oldest = 0;
    for (auto entry = entries_.cbegin(); entry != entries_.cend(); ++entry) {
        if (entry->phase == Phase::Active) {
            if (oldest == 0)
                oldest = entry.key();
            ++active;
        }
    }
    if (active >= ActiveLimit)
        cancelToken(oldest);
    // Signals may reenter and cancel this operation while evicting an old one.
    it = entries_.find(token);
    if (it == entries_.end())
        return;
    if (it->cancelled) {
        finish(token);
        return;
    }
    it->phase = Phase::Active;
    const Entry accepted = *it;
    updateTimer();
    emit notificationPresented(accepted.client.data(), accepted.message.profileScope,
                               accepted.message.origin);
}

void FerricBrowserNotificationPresenter::closeRemote(const Entry &entry)
{
    const bool portal = entry.message.transport == Transport::Portal;
    if (entry.serviceOwner.isEmpty() || (!portal && entry.desktopId == 0)
            || entry.phase == Phase::Discovering)
        return;
    auto request = method(entry.serviceOwner, portal,
                           portal ? QStringLiteral("RemoveNotification") : QStringLiteral("CloseNotification"));
    request.setArguments(portal ? QVariantList{entry.portalId} : QVariantList{entry.desktopId});
    // Address the unique owner, never a replacement daemon reusing IDs.
    QDBusConnection::sessionBus().call(request, QDBus::NoBlock);
}

bool FerricBrowserNotificationPresenter::cancel(QObject *client)
{
    for (auto it = entries_.cbegin(); it != entries_.cend(); ++it) {
        if (it->client == client) {
            cancelToken(it.key());
            return true;
        }
    }
    return false;
}

void FerricBrowserNotificationPresenter::cancelToken(quint64 token, bool unavailable)
{
    auto it = entries_.find(token);
    if (it == entries_.end() || it->cancelled)
        return;
    const Entry cancelled = *it;
    disconnect(it->destroyedConnection);
    disconnect(it->closedConnection);
    if (it->phase == Phase::Active) {
        entries_.erase(it);
    } else {
        // Retain a tombstone until the in-flight reply supplies the remote ID.
        it->cancelled = true;
        it->client.clear();
    }
    closeRemote(cancelled);
    updateTimer();
    if (cancelled.client) {
        if (unavailable)
            emit notificationUnavailable(cancelled.client.data(), cancelled.message.profileScope,
                                         cancelled.message.origin);
        else
            emit notificationFinished(cancelled.client.data());
    }
}

void FerricBrowserNotificationPresenter::finish(quint64 token, bool unavailable)
{
    auto it = entries_.find(token);
    if (it == entries_.end())
        return;
    const Entry finished = *it;
    entries_.erase(it);
    disconnect(finished.destroyedConnection);
    disconnect(finished.closedConnection);
    updateTimer();
    if (finished.client) {
        if (unavailable)
            emit notificationUnavailable(finished.client.data(), finished.message.profileScope,
                                         finished.message.origin);
        else
            emit notificationFinished(finished.client.data());
    }
}

void FerricBrowserNotificationPresenter::closeAll()
{
    const auto tokens = entries_.keys();
    for (auto token : tokens)
        cancelToken(token);
}

quint64 FerricBrowserNotificationPresenter::generation(Transport transport) const
{
    return transport == Transport::Portal ? portalGeneration_ : desktopGeneration_;
}

void FerricBrowserNotificationPresenter::serviceChanged(Transport transport)
{
    if (transport == Transport::Portal)
        ++portalGeneration_;
    else
        ++desktopGeneration_;
    const auto tokens = entries_.keys();
    for (auto token : tokens) {
        const auto it = entries_.constFind(token);
        if (it != entries_.cend() && it->message.transport == transport)
            cancelToken(token, true);
    }
}

void FerricBrowserNotificationPresenter::expirePending()
{
    const auto tokens = entries_.keys();
    for (auto token : tokens) {
        const auto it = entries_.constFind(token);
        if (it != entries_.cend() && it->phase != Phase::Active
                && !it->cancelled && it->deadline.hasExpired())
            cancelToken(token, true);
    }
}

void FerricBrowserNotificationPresenter::updateTimer()
{
    for (const auto &entry : std::as_const(entries_)) {
        if (entry.phase != Phase::Active && !entry.cancelled) {
            if (!pendingTimer_.isActive())
                pendingTimer_.start();
            return;
        }
    }
    pendingTimer_.stop();
}

quint64 FerricBrowserNotificationPresenter::tokenForRemote(const QString &owner, uint id) const
{
    for (auto it = entries_.cbegin(); it != entries_.cend(); ++it) {
        if (it->phase == Phase::Active && it->message.transport == Transport::Desktop
                && it->desktopId == id && it->serviceOwner == owner)
            return it.key();
    }
    return 0;
}

void FerricBrowserNotificationPresenter::actionInvoked(uint id, const QString &action,
                                                      const QDBusMessage &message)
{
    const auto it = entries_.constFind(tokenForRemote(message.service(), id));
    if (action == QStringLiteral("default") && it != entries_.cend() && it->client) {
        const Entry clicked = *it;
        emit notificationClicked(clicked.client.data(), clicked.message.profileScope,
                                 clicked.message.origin);
    }
}

void FerricBrowserNotificationPresenter::notificationClosed(uint id, uint, const QDBusMessage &message)
{
    finish(tokenForRemote(message.service(), id));
}

void FerricBrowserNotificationPresenter::portalActionInvoked(const QString &id, const QString &action,
                const QVariantList &, const QDBusMessage &message)
{
    for (const auto &entry : std::as_const(entries_)) {
        if (entry.phase == Phase::Active && entry.message.transport == Transport::Portal
                && entry.serviceOwner == message.service() && entry.portalId == id
                && action == QStringLiteral("default") && entry.client) {
            const Entry clicked = entry;
            emit notificationClicked(clicked.client.data(), clicked.message.profileScope,
                                     clicked.message.origin);
            return;
        }
    }
}
