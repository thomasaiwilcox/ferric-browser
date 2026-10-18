#include "../../src/notification_presenter.h"
#include "notification_test_service.h"

#include <QtCore/QScopedPointer>
#include <QtTest/QSignalSpy>
#include <QtTest/QTest>

namespace {
using Presenter = FerricBrowserNotificationPresenter;

Presenter::Message sample(Presenter::Transport transport = Presenter::Transport::Desktop)
{
    return {QStringLiteral("normal:Work"), QStringLiteral("https://example.test"),
        QStringLiteral("A <literal> title"), QStringLiteral("<b>Literal</b> & more"), transport};
}
}

class NotificationTransportTests : public QObject {
    Q_OBJECT
    QScopedPointer<NotificationTestService> service;
private slots:
    void initTestCase()
    {
        QVERIFY2(qEnvironmentVariable("FERRIC_NOTIFICATION_TEST_BUS") == QStringLiteral("isolated"),
                 "Run through dbus-run-session; this suite must not use the desktop session bus");
        service.reset(new NotificationTestService(QStringLiteral("primary")));
        QVERIFY2(service->start(), "Cannot register isolated fake notification services");
    }
    void init() { service->reset(); }
    void deliveryWaitsForValidAcknowledgement()
    {
        Presenter presenter;
        QObject client;
        QSignalSpy accepted(&presenter, &Presenter::notificationPresented);
        QSignalSpy finished(&presenter, &Presenter::notificationFinished);
        QVERIFY(presenter.enqueue(&client, sample()));
        QTRY_COMPARE(service->pendingCount(), 1);
        QCOMPARE(accepted.size(), 0);
        QCOMPARE(service->delivery(0)["signature"].toString(), QStringLiteral("susssasa{sv}i"));
        QCOMPARE(service->delivery(0)["arguments"].toArray()[3].toString(), sample().title);
        QCOMPARE(service->delivery(0)["arguments"].toArray()[4].toString(), QStringLiteral("&lt;b&gt;Literal&lt;/b&gt; &amp; more"));
        service->acceptNext(42);
        QTRY_COMPARE(accepted.size(), 1);
        QCOMPARE(accepted[0][0].value<QObject *>(), &client);
        QCOMPARE(accepted[0][1].toString(), sample().profileScope);
        QVERIFY(presenter.cancel(&client));
        QTRY_COMPARE(service->closedIds(), QList<uint>{42});
        QCOMPARE(finished.size(), 1);
        QVERIFY(!presenter.cancel(&client));
        QCOMPARE(finished.size(), 1);
    }
    void plainTextDaemonReceivesLiteralBody()
    {
        service->configure(QStringLiteral("capabilities"), QStringList{QStringLiteral("body")});
        Presenter presenter;
        QObject client;
        QVERIFY(presenter.enqueue(&client, sample()));
        QTRY_COMPARE(service->pendingCount(), 1);
        QCOMPARE(service->delivery(0)["arguments"].toArray()[4].toString(), sample().body);
        service->acceptNext(1);
    }
    void invalidRepliesNeverClaimDelivery_data()
    {
        QTest::addColumn<QString>("kind");
        QTest::newRow("zero-id") << QStringLiteral("zero");
        QTest::newRow("wrong-signature") << QStringLiteral("type");
        QTest::newRow("service-error") << QStringLiteral("error");
    }
    void invalidRepliesNeverClaimDelivery()
    {
        QFETCH(QString, kind);
        Presenter presenter;
        QObject client;
        QSignalSpy accepted(&presenter, &Presenter::notificationPresented);
        QSignalSpy unavailable(&presenter, &Presenter::notificationUnavailable);
        QVERIFY(presenter.enqueue(&client, sample()));
        QTRY_COMPARE(service->pendingCount(), 1);
        if (kind == QStringLiteral("zero")) service->acceptNext(0);
        else service->rejectNext(kind);
        QTRY_COMPARE(unavailable.size(), 1);
        QCOMPARE(accepted.size(), 0);
        QVERIFY(!presenter.cancel(&client));
    }
    void cancellingAnInFlightDeliveryClosesTheLateRemoteId()
    {
        Presenter presenter;
        QObject client;
        QSignalSpy accepted(&presenter, &Presenter::notificationPresented);
        QSignalSpy finished(&presenter, &Presenter::notificationFinished);
        QVERIFY(presenter.enqueue(&client, sample()));
        QTRY_COMPARE(service->pendingCount(), 1);
        QVERIFY(presenter.cancel(&client));
        QCOMPARE(finished.size(), 1);
        service->acceptNext(51);
        QTRY_COMPARE(service->closedIds(), QList<uint>{51});
        QCOMPARE(accepted.size(), 0);
        QCOMPARE(finished.size(), 1);
    }
    void destroyedSourceStillRetractsLateDelivery()
    {
        Presenter presenter;
        auto *client = new QObject;
        QSignalSpy accepted(&presenter, &Presenter::notificationPresented);
        QVERIFY(presenter.enqueue(client, sample()));
        QTRY_COMPARE(service->pendingCount(), 1);
        delete client;
        service->acceptNext(61);
        QTRY_COMPARE(service->closedIds(), QList<uint>{61});
        QCOMPARE(accepted.size(), 0);
    }
    void cancelBeforeDiscoveryNeverSends()
    {
        service->configure(QStringLiteral("holdDiscovery"), true);
        Presenter presenter;
        QObject client;
        QVERIFY(presenter.enqueue(&client, sample()));
        QTRY_COMPARE(service->discoveryCount(), 1);
        QVERIFY(presenter.cancel(&client));
        service->replyDiscovery(0);
        QTest::qWait(50);
        QCOMPARE(service->deliveryCount(), 0);
    }
    void timeoutReportsFailureAndRetractsLateDelivery()
    {
        Presenter presenter;
        QObject client;
        QSignalSpy accepted(&presenter, &Presenter::notificationPresented);
        QSignalSpy unavailable(&presenter, &Presenter::notificationUnavailable);
        QVERIFY(presenter.enqueue(&client, sample()));
        QTRY_COMPARE(service->pendingCount(), 1);
        QTRY_COMPARE_WITH_TIMEOUT(unavailable.size(), 1, 7000);
        service->acceptNext(71);
        QTRY_COMPARE(service->closedIds(), QList<uint>{71});
        QCOMPARE(unavailable.size(), 1);
        QCOMPARE(accepted.size(), 0);
    }
    void cancelledRequestsRemainWithinThePendingBudget()
    {
        Presenter presenter;
        for (int i = 0; i < 32; ++i) {
            auto *client = new QObject(&presenter);
            QVERIFY(presenter.enqueue(client, sample()));
        }
        QTRY_COMPARE(service->pendingCount(), 32);
        presenter.closeAll();
        QObject extra;
        QVERIFY(!presenter.enqueue(&extra, sample()));
        for (uint i = 1; i <= 32; ++i)
            service->acceptNext(i);
        QTRY_COMPARE(service->closedIds().size(), 32);
        QVERIFY(presenter.enqueue(&extra, sample()));
        QTRY_COMPARE(service->pendingCount(), 1);
        service->acceptNext(99);
    }
    void continuingActivityCannotPostponeAnOlderDeliveryDeadline()
    {
        Presenter presenter;
        QObject held;
        QSignalSpy unavailable(&presenter, &Presenter::notificationUnavailable);
        QVERIFY(presenter.enqueue(&held, sample()));
        QTRY_COMPARE(service->pendingCount(), 1);
        service->configure(QStringLiteral("automatic"), true);
        int rejected = 0;
        QTimer traffic;
        traffic.setInterval(40);
        connect(&traffic, &QTimer::timeout, &presenter, [&] {
            if (!presenter.enqueue(new QObject(&presenter), sample())) ++rejected;
        });
        traffic.start();
        QTRY_COMPARE_WITH_TIMEOUT(unavailable.size(), 1, 6500);
        traffic.stop();
        QCOMPARE(unavailable[0][0].value<QObject *>(), &held);
        QCOMPARE(rejected, 0);
        service->acceptNext(10000);
        QTRY_VERIFY(service->closedIds().contains(10000));
    }
    void desktopSignalsRequireTheActualSenderAndActiveId()
    {
        Presenter presenter;
        QObject client;
        QSignalSpy accepted(&presenter, &Presenter::notificationPresented);
        QSignalSpy clicked(&presenter, &Presenter::notificationClicked);
        QSignalSpy finished(&presenter, &Presenter::notificationFinished);
        QVERIFY(presenter.enqueue(&client, sample()));
        QTRY_COMPARE(service->pendingCount(), 1);
        service->acceptNext(81);
        QTRY_COMPARE(accepted.size(), 1);
        NotificationTestService stranger(QStringLiteral("stranger"));
        QVERIFY(stranger.start(false));
        stranger.signal(false, QStringLiteral("ActionInvoked"), {uint(81), QStringLiteral("default")});
        stranger.signal(false, QStringLiteral("NotificationClosed"), {uint(81), uint(2)});
        service->signal(false, QStringLiteral("ActionInvoked"), {uint(81), QStringLiteral("unknown")});
        QTest::qWait(50);
        QCOMPARE(clicked.size(), 0);
        QCOMPARE(finished.size(), 0);
        service->signal(false, QStringLiteral("ActionInvoked"), {uint(81), QStringLiteral("default")});
        QTRY_COMPARE(clicked.size(), 1);
        QCOMPARE(clicked[0][0].value<QObject *>(), &client);
        service->signal(false, QStringLiteral("NotificationClosed"), {uint(81), uint(2)});
        QTRY_COMPARE(finished.size(), 1);
        QVERIFY(!presenter.cancel(&client));
        QCOMPARE(service->closedIds().size(), 0);
    }
    void requiredPortalUsesAddAndRemoveInsteadOfDesktopNotify()
    {
        Presenter presenter;
        QObject client;
        QSignalSpy accepted(&presenter, &Presenter::notificationPresented);
        QSignalSpy clicked(&presenter, &Presenter::notificationClicked);
        QVERIFY(presenter.enqueue(&client, sample(Presenter::Transport::Portal)));
        QTRY_COMPARE(service->pendingCount(), 1);
        const auto request = service->delivery(0);
        QCOMPARE(request["member"].toString(), QStringLiteral("AddNotification"));
        QCOMPARE(request["signature"].toString(), QStringLiteral("sa{sv}"));
        const auto contents = request["arguments"].toArray()[1].toObject();
        QCOMPARE(contents.value(QStringLiteral("body")).toString(), sample().body);
        QVERIFY(!contents.contains(QStringLiteral("markup-body")));
        const auto id = request["arguments"].toArray()[0].toString();
        QVERIFY(id.startsWith(QStringLiteral("ferric-")));
        service->acceptNext(0);
        QTRY_COMPARE(accepted.size(), 1);
        service->signal(true, QStringLiteral("ActionInvoked"), {id, QStringLiteral("default"), QVariantList{}});
        QTRY_COMPARE(clicked.size(), 1);
        QVERIFY(presenter.cancel(&client));
        QTRY_COMPARE(service->removedIds(), QStringList{id});
        QCOMPARE(service->closedIds().size(), 0);
    }
    void cancelledPortalSendIsWithdrawnAfterItsLateReply()
    {
        Presenter presenter;
        QObject client;
        QSignalSpy accepted(&presenter, &Presenter::notificationPresented);
        QVERIFY(presenter.enqueue(&client, sample(Presenter::Transport::Portal)));
        QTRY_COMPARE(service->pendingCount(), 1);
        const auto id = service->delivery(0)["arguments"].toArray()[0].toString();
        QVERIFY(presenter.cancel(&client));
        QTRY_COMPARE(service->removedIds().size(), 1);
        service->acceptNext(0);
        QTRY_COMPARE(service->removedIds().size(), 2);
        QCOMPARE(service->removedIds(), (QStringList{id, id}));
        QCOMPARE(accepted.size(), 0);
    }
    void daemonRestartDoesNotCloseAReusedIdInTheNewDaemon()
    {
        Presenter presenter;
        QObject first;
        QObject second;
        QSignalSpy accepted(&presenter, &Presenter::notificationPresented);
        QSignalSpy unavailable(&presenter, &Presenter::notificationUnavailable);
        QVERIFY(presenter.enqueue(&first, sample()));
        QTRY_COMPARE(service->pendingCount(), 1);
        service->releaseDesktop();
        NotificationTestService replacement(QStringLiteral("replacement"));
        QVERIFY(replacement.start(false));
        replacement.claimDesktop();
        QTRY_COMPARE(unavailable.size(), 1);
        QVERIFY(presenter.enqueue(&second, sample()));
        QTRY_COMPARE(replacement.pendingCount(), 1);
        replacement.acceptNext(1);
        QTRY_COMPARE(accepted.size(), 1);
        service->acceptNext(1);
        QTRY_COMPARE(service->closedIds(), QList<uint>{1});
        QCOMPARE(replacement.closedIds().size(), 0);
        QVERIFY(presenter.cancel(&second));
        QTRY_COMPARE(replacement.closedIds(), QList<uint>{1});
        replacement.releaseDesktop();
        service->claimDesktop();
    }
    void activeNotificationsAreBoundedAndEvictedCleanly()
    {
        service->configure(QStringLiteral("automatic"), true);
        Presenter presenter;
        QSignalSpy accepted(&presenter, &Presenter::notificationPresented);
        QSignalSpy finished(&presenter, &Presenter::notificationFinished);
        for (int i = 0; i < 129; ++i) {
            QVERIFY(presenter.enqueue(new QObject(&presenter), sample()));
            QTRY_COMPARE(accepted.size(), i + 1);
        }
        QTRY_COMPARE(service->closedIds(), QList<uint>{1});
        QCOMPARE(finished.size(), 1);
        presenter.closeAll();
        QTRY_COMPARE(service->closedIds().size(), 129);
        QCOMPARE(finished.size(), 129);
    }
    void unavailableServiceReportsFailureWithoutClaimingDelivery()
    {
        service->releaseDesktop();
        Presenter presenter;
        QObject client;
        QSignalSpy accepted(&presenter, &Presenter::notificationPresented);
        QSignalSpy unavailable(&presenter, &Presenter::notificationUnavailable);
        QVERIFY(presenter.enqueue(&client, sample()));
        QTRY_COMPARE(unavailable.size(), 1);
        QCOMPARE(accepted.size(), 0);
        QCOMPARE(service->deliveryCount(), 0);
        service->claimDesktop();
    }
};

QTEST_GUILESS_MAIN(NotificationTransportTests)
#include "tst_notification_transport.moc"
