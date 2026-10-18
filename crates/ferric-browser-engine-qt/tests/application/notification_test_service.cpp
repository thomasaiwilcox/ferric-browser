#include <QtCore/QCoreApplication>
#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtDBus/QDBusArgument>
#include <QtDBus/QDBusConnection>
#include <QtDBus/QDBusMessage>
#include <QtDBus/QDBusVariant>
#include <QtDBus/QDBusVirtualObject>

namespace {
const QString DesktopService = QStringLiteral("org.freedesktop.Notifications");
const QString DesktopPath = QStringLiteral("/org/freedesktop/Notifications");
const QString PortalService = QStringLiteral("org.freedesktop.portal.Desktop");
const QString PortalPath = QStringLiteral("/org/freedesktop/portal/desktop");
const QString PortalInterface = QStringLiteral("org.freedesktop.portal.Notification");
const QString ControlPath = QStringLiteral("/org/ferricbrowser/NotificationTest");
const QString ControlInterface = QStringLiteral("org.ferricbrowser.NotificationTest");

class NotificationService : public QDBusVirtualObject {
public:
    QDBusConnection bus = QDBusConnection::sessionBus();
    QStringList capabilities;
    QList<QDBusMessage> discoveries;
    QList<QDBusMessage> pending;
    QList<QDBusMessage> deliveries;
    QList<uint> closedIds;
    QStringList removedIds;
    bool holdDiscovery = false;
    bool automatic = false;
    uint nextId = 1;

    QString introspect(const QString &path) const override
    {
        if (path != PortalPath) return {};
        return QStringLiteral(R"XML(<interface name="org.freedesktop.portal.Notification">
          <property name="version" type="u" access="read"/>
          <method name="AddNotification"><arg type="s" direction="in"/><arg type="a{sv}" direction="in"/></method>
          <method name="RemoveNotification"><arg type="s" direction="in"/></method>
          <signal name="ActionInvoked"><arg type="s"/><arg type="s"/><arg type="av"/></signal>
        </interface>)XML");
    }
    bool handleMessage(const QDBusMessage &message, const QDBusConnection &) override
    {
        message.setDelayedReply(true);
        if (message.path() == ControlPath) {
            control(message);
        } else if (message.member() == QStringLiteral("Introspect")) {
            bus.send(message.createReply(QStringLiteral("<node>") + introspect(message.path()) + QStringLiteral("</node>")));
        } else if (message.member() == QStringLiteral("GetAll")) {
            bus.send(message.createReply(QVariant::fromValue(QVariantMap{{QStringLiteral("version"), uint(1)}})));
        } else if (message.member() == QStringLiteral("GetCapabilities") || message.member() == QStringLiteral("Get")) {
            discoveries.push_back(message);
            if (!holdDiscovery) replyDiscovery(message);
        } else if (message.member() == QStringLiteral("Notify") || message.member() == QStringLiteral("AddNotification")) {
            deliveries.push_back(message);
            if (automatic) replyDelivery(message, nextId++);
            else pending.push_back(message);
        } else if (message.member() == QStringLiteral("CloseNotification")) {
            const uint id = message.arguments().constFirst().toUInt();
            closedIds.push_back(id);
            if (message.isReplyRequired()) bus.send(message.createReply());
            signal(false, QStringLiteral("NotificationClosed"), {id, uint(3)});
        } else if (message.member() == QStringLiteral("RemoveNotification")) {
            removedIds.push_back(message.arguments().constFirst().toString());
            if (message.isReplyRequired()) bus.send(message.createReply());
        } else {
            bus.send(message.createErrorReply(QStringLiteral("org.freedesktop.DBus.Error.UnknownMethod"), message.member()));
        }
        return true;
    }
    void replyDiscovery(const QDBusMessage &message)
    {
        if (message.member() == QStringLiteral("Get"))
            bus.send(message.createReply(QVariant::fromValue(QDBusVariant(uint(1)))));
        else bus.send(message.createReply(QVariant::fromValue(capabilities)));
    }
    void replyDelivery(const QDBusMessage &message, uint id)
    {
        bus.send(message.member() == QStringLiteral("AddNotification")
            ? message.createReply() : message.createReply(QVariant::fromValue(id)));
    }
    void signal(bool portal, const QString &name, const QVariantList &arguments)
    {
        auto message = QDBusMessage::createSignal(portal ? PortalPath : DesktopPath,
                        portal ? PortalInterface : DesktopService, name);
        message.setArguments(arguments);
        bus.send(message);
    }
    void reset()
    {
        capabilities = {QStringLiteral("body"), QStringLiteral("body-markup"), QStringLiteral("actions")};
        discoveries.clear(); pending.clear(); deliveries.clear(); closedIds.clear(); removedIds.clear();
        holdDiscovery = false; automatic = false; nextId = 1;
    }
    QString snapshot() const
    {
        QJsonArray requests;
        for (const auto &delivery : deliveries) {
            auto arguments = delivery.arguments();
            if (delivery.member() == QStringLiteral("AddNotification"))
                arguments[1] = qdbus_cast<QVariantMap>(arguments[1]);
            requests.push_back(QJsonObject{{"member", delivery.member()}, {"signature", delivery.signature()},
                                          {"arguments", QJsonArray::fromVariantList(arguments)}});
        }
        QJsonArray closed;
        for (auto id : closedIds) closed.push_back(qint64(id));
        return QString::fromUtf8(QJsonDocument(QJsonObject{
            {"pending", pending.size()}, {"discoveries", discoveries.size()}, {"deliveries", requests},
            {"closed", closed}, {"removed", QJsonArray::fromStringList(removedIds)}}).toJson(QJsonDocument::Compact));
    }
    void control(const QDBusMessage &message)
    {
        const auto arguments = message.arguments();
        const auto name = message.member();
        if (name == QStringLiteral("Snapshot")) {
            bus.send(message.createReply(snapshot()));
            return;
        }
        if (name == QStringLiteral("Reset")) reset();
        else if (name == QStringLiteral("Configure")) {
            const auto key = arguments[0].toString();
            const auto value = qvariant_cast<QDBusVariant>(arguments[1]).variant();
            if (key == QStringLiteral("holdDiscovery")) holdDiscovery = value.toBool();
            else if (key == QStringLiteral("automatic")) automatic = value.toBool();
            else if (key == QStringLiteral("capabilities")) capabilities = qdbus_cast<QStringList>(value);
        } else if (name == QStringLiteral("AcceptNext") && !pending.isEmpty()) {
            replyDelivery(pending.takeFirst(), arguments[0].toUInt());
        } else if (name == QStringLiteral("RejectNext") && !pending.isEmpty()) {
            const auto request = pending.takeFirst();
            if (arguments[0].toString() == QStringLiteral("type"))
                bus.send(request.createReply(QStringLiteral("42")));
            else bus.send(request.createErrorReply(QStringLiteral("org.freedesktop.DBus.Error.Failed"), QStringLiteral("fixture")));
        } else if (name == QStringLiteral("ReplyDiscovery")) {
            replyDiscovery(discoveries[arguments[0].toInt()]);
        } else if (name == QStringLiteral("Signal")) {
            signal(arguments[0].toBool(), arguments[1].toString(), qdbus_cast<QVariantList>(arguments[2]));
        } else if (name == QStringLiteral("ReleaseDesktop")) {
            bus.unregisterService(DesktopService);
        } else if (name == QStringLiteral("ClaimDesktop")) {
            if (!bus.registerService(DesktopService)) {
                bus.send(message.createErrorReply(QStringLiteral("org.freedesktop.DBus.Error.Failed"), QStringLiteral("Cannot claim desktop service")));
                return;
            }
        }
        bus.send(message.createReply());
    }
};
}

int main(int argc, char **argv)
{
    QCoreApplication application(argc, argv);
    if (qEnvironmentVariable("FERRIC_NOTIFICATION_TEST_BUS") != QStringLiteral("isolated")
            || application.arguments().size() != 3)
        return 2;
    NotificationService service;
    service.reset();
    if (!service.bus.registerVirtualObject(ControlPath, &service)
            || !service.bus.registerVirtualObject(DesktopPath, &service)
            || !service.bus.registerVirtualObject(PortalPath, &service)
            || !service.bus.registerService(application.arguments()[1]))
        return 3;
    if (application.arguments()[2] == QStringLiteral("own")
            && (!service.bus.registerService(DesktopService) || !service.bus.registerService(PortalService)))
        return 4;
    return application.exec();
}
