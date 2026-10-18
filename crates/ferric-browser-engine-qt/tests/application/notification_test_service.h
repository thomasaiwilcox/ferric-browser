#ifndef FERRIC_NOTIFICATION_TEST_SERVICE_H
#define FERRIC_NOTIFICATION_TEST_SERVICE_H
#include <QtCore/QCoreApplication>
#include <QtCore/QElapsedTimer>
#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QProcess>
#include <QtDBus/QDBusConnection>
#include <QtDBus/QDBusConnectionInterface>
#include <QtDBus/QDBusMessage>
#include <QtDBus/QDBusReply>
#include <QtDBus/QDBusVariant>
#include <QtTest/QTest>

// Separate processes exercise real bus serialization, unique signal senders
// and daemon replacement. Qt can shortcut calls to local connections.
class NotificationTestService {
    QProcess process_;
    QString controlName_;
public:
    explicit NotificationTestService(const QString &label)
        : controlName_(QStringLiteral("org.ferricbrowser.NotificationTest.") + label) {}
    ~NotificationTestService() { process_.kill(); process_.waitForFinished(2000); }
    bool start(bool own = true)
    {
        process_.setProcessChannelMode(QProcess::ForwardedErrorChannel);
        process_.start(QCoreApplication::applicationDirPath() + QStringLiteral("/notification_test_service"),
                       {controlName_, own ? QStringLiteral("own") : QStringLiteral("passive")});
        if (!process_.waitForStarted(2000)) return false;
        QElapsedTimer timer;
        timer.start();
        while (timer.elapsed() < 2000 && process_.state() != QProcess::NotRunning) {
            if (QDBusConnection::sessionBus().interface()->isServiceRegistered(controlName_).value()) return true;
            QTest::qWait(10);
        }
        return false;
    }
    QDBusMessage call(const QString &method, const QVariantList &arguments = {}) const
    {
        auto request = QDBusMessage::createMethodCall(controlName_,
            QStringLiteral("/org/ferricbrowser/NotificationTest"),
            QStringLiteral("org.ferricbrowser.NotificationTest"), method);
        request.setArguments(arguments);
        const auto reply = QDBusConnection::sessionBus().call(request, QDBus::Block, 2000);
        if (reply.type() == QDBusMessage::ErrorMessage)
            qFatal("Notification mock control failed: %s", qPrintable(reply.errorMessage()));
        return reply;
    }
    QJsonObject snapshot() const
    {
        return QJsonDocument::fromJson(call(QStringLiteral("Snapshot")).arguments()[0].toString().toUtf8()).object();
    }
    int pendingCount() const { return snapshot()["pending"].toInt(); }
    int discoveryCount() const { return snapshot()["discoveries"].toInt(); }
    int deliveryCount() const { return snapshot()["deliveries"].toArray().size(); }
    QJsonObject delivery(int index) const { return snapshot()["deliveries"].toArray()[index].toObject(); }
    QList<uint> closedIds() const
    {
        QList<uint> result;
        for (const auto &id : snapshot()["closed"].toArray()) result.push_back(uint(id.toInteger()));
        return result;
    }
    QStringList removedIds() const
    {
        QStringList result;
        for (const auto &id : snapshot()["removed"].toArray()) result.push_back(id.toString());
        return result;
    }
    void reset() { call(QStringLiteral("Reset")); }
    void configure(const QString &key, const QVariant &value)
    {
        call(QStringLiteral("Configure"), {key, QVariant::fromValue(QDBusVariant(value))});
    }
    void acceptNext(uint id) { call(QStringLiteral("AcceptNext"), {id}); }
    void rejectNext(const QString &kind) { call(QStringLiteral("RejectNext"), {kind}); }
    void replyDiscovery(int index) { call(QStringLiteral("ReplyDiscovery"), {index}); }
    void signal(bool portal, const QString &name, const QVariantList &arguments)
    {
        call(QStringLiteral("Signal"), {portal, name, arguments});
    }
    void releaseDesktop() { call(QStringLiteral("ReleaseDesktop")); }
    void claimDesktop() { call(QStringLiteral("ClaimDesktop")); }
};
#endif
