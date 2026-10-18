#include "../../src/profile_lifetime.h"
#include <QtTest>

static bool retain(FerricProfileLifetime &lifetime, QObject *host,
                   QObject *ui) {
    return lifetime.retainWindow(host, ui) && lifetime.activateWindow(host);
}

class LifetimeTests : public QObject {
    Q_OBJECT
  private slots:
    void pendingWindowCannotSupplyPolicyOrOwnAView() {
        FerricProfileLifetime lifetime;
        QObject host, ui;
        QQuickItem view;
        QVERIFY(lifetime.retainWindow(&host, &ui));
        QCOMPARE(lifetime.policyUi(), nullptr);
        QVERIFY(!lifetime.adoptView(&view, &host, &ui));
        QVERIFY(lifetime.activateWindow(&host));
        QCOMPARE(lifetime.policyUi(), &ui);
    }
    void disposalDestroysPagesBeforeTheirProfile() {
        auto *lifetime = new FerricProfileLifetime;
        auto *profile = new QObject;
        QPointer<QObject> profileGuard = profile;
        QVERIFY(lifetime->ownResource(profile));
        QObject host, ui;
        QVERIFY(retain(*lifetime, &host, &ui));
        auto *view = new QQuickItem;
        bool profileAliveWhenPageDied = false;
        connect(view, &QObject::destroyed, &host,
                [&] { profileAliveWhenPageDied = bool(profileGuard); });
        QVERIFY(lifetime->adoptView(view, &host, &ui));
        delete lifetime;
        QVERIFY(profileAliveWhenPageDied);
        QVERIFY(profileGuard.isNull());
    }
    void profileFactoryRetainsOwnershipUntilPagesAreDestroyed() {
        bool profileAliveAtFactoryDestruction = false;
        struct Factory : QObject {
            QPointer<QObject> instance;
            bool &observed;
            Factory(QObject *profile, bool &alive)
                : instance(profile), observed(alive) {}
            ~Factory() override {
                observed = bool(instance);
                if (instance)
                    delete instance.data();
            }
        };
        auto *lifetime = new FerricProfileLifetime;
        auto *profile = new QObject;
        auto *factory = new Factory(profile, profileAliveAtFactoryDestruction);
        QPointer<QObject> profileGuard = profile;
        QVERIFY(lifetime->ownProfile(factory, profile));
        QCOMPARE(profile->parent(), factory);
        QCOMPARE(factory->parent(), lifetime);
        QObject host, ui;
        QVERIFY(retain(*lifetime, &host, &ui));
        auto *view = new QQuickItem;
        bool pageSawItsProfile = false;
        connect(view, &QObject::destroyed, &host,
                [&] { pageSawItsProfile = bool(profileGuard); });
        QVERIFY(lifetime->adoptView(view, &host, &ui));
        delete lifetime;
        QVERIFY(pageSawItsProfile);
        QVERIFY(profileAliveAtFactoryDestruction);
        QVERIFY(profileGuard.isNull());
    }
    void transferredViewSurvivesOriginalQObjectDestruction() {
        FerricProfileLifetime lifetime;
        auto *source = new QObject;
        auto *sourceUi = new QObject(source);
        QObject destination;
        QObject destinationUi(&destination);
        QQuickItem sourceVisual, destinationVisual;
        auto *view = new QQuickItem(&sourceVisual);
        QPointer<QQuickItem> guard = view;
        QVERIFY(retain(lifetime, source, sourceUi));
        QVERIFY(retain(lifetime, &destination, &destinationUi));
        QVERIFY(lifetime.adoptView(view, source, sourceUi));
        QCOMPARE(view->parent(), &lifetime);
        QCOMPARE(view->parentItem(), &sourceVisual);
        view->setParentItem(&destinationVisual);
        QVERIFY(lifetime.adoptView(view, &destination, &destinationUi));
        QCOMPARE(lifetime.policyUi(), sourceUi);
        QCOMPARE(lifetime.ownerUi(view), &destinationUi);
        delete source;
        QVERIFY(guard);
        QCOMPARE(lifetime.policyUi(), &destinationUi);
        QCOMPARE(view->parentItem(), &destinationVisual);
        lifetime.releaseWindow(&destination);
        QCOMPARE(lifetime.ownerUi(view), nullptr);
        QSignalSpy drained(&lifetime, &FerricProfileLifetime::drained);
        QCOMPARE(lifetime.viewCount(), 1);
        QCOMPARE(drained.count(), 0);
        QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
        QVERIFY(guard.isNull());
        QTRY_COMPARE(drained.count(), 1);
        QCOMPARE(lifetime.viewCount(), 0);
        QVERIFY(!retain(lifetime, &destination, &destinationUi));
    }
    void finalHostDestructionRetiresItsOwnedView() {
        FerricProfileLifetime lifetime;
        auto *host = new QObject;
        auto *ui = new QObject(host);
        auto *view = new QQuickItem;
        QPointer<QQuickItem> guard = view;
        QVERIFY(retain(lifetime, host, ui));
        QVERIFY(lifetime.adoptView(view, host, ui));
        delete host;
        QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
        QVERIFY(guard.isNull());
    }
    void pendingFinalReleaseCanAcceptANewConsumer() {
        FerricProfileLifetime lifetime;
        QObject first, firstUi, second, secondUi;
        QVERIFY(retain(lifetime, &first, &firstUi));
        QSignalSpy drained(&lifetime, &FerricProfileLifetime::drained);
        lifetime.releaseWindow(&first);
        QVERIFY(retain(lifetime, &second, &secondUi));
        QTest::qWait(20);
        QCOMPARE(drained.count(), 0);
        QCOMPARE(lifetime.policyUi(), &secondUi);
        lifetime.releaseWindow(&second);
        QTRY_COMPARE(drained.count(), 1);
    }
    void retiredViewCannotBeResurrectedBeforeDeferredDeletion() {
        FerricProfileLifetime lifetime;
        QObject first, firstUi, second, secondUi;
        QVERIFY(retain(lifetime, &first, &firstUi));
        QVERIFY(retain(lifetime, &second, &secondUi));
        auto *view = new QQuickItem;
        QVERIFY(lifetime.adoptView(view, &first, &firstUi));
        lifetime.releaseWindow(&first);
        QVERIFY(!lifetime.adoptView(view, &second, &secondUi));
        QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
        QCOMPARE(lifetime.viewCount(), 0);
    }
    void destroyingTrackerCancelsConsumerCallbacks() {
        QObject host, ui;
        auto *lifetime = new FerricProfileLifetime;
        QVERIFY(retain(*lifetime, &host, &ui));
        auto *view = new QQuickItem;
        QPointer<QQuickItem> guard = view;
        QVERIFY(lifetime->adoptView(view, &host, &ui));
        delete lifetime;
        QVERIFY(guard.isNull());
        QTest::qWait(20);
    }
    void destroyedUiPublishesTheClearedPolicyOwner() {
        FerricProfileLifetime lifetime;
        QObject host;
        auto *ui = new QObject;
        QVERIFY(retain(lifetime, &host, ui));
        QSignalSpy changed(&lifetime, &FerricProfileLifetime::policyUiChanged);
        delete ui;
        QCOMPARE(lifetime.policyUi(), nullptr);
        QCOMPARE(changed.count(), 1);
        QCOMPARE(lifetime.windowCount(), 0);
    }
};
QTEST_MAIN(LifetimeTests)
#include "tst_lifetime.moc"
