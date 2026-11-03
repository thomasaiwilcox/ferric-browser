#include "request_interceptor.h"
#include <QtTest/QtTest>
#include <QQmlEngine>
#include <QQmlComponent>
#include <QQmlContext>

// This isolated test never installs an ABP engine or changes a blocking decision.
extern "C" void ferric_browser_adblock_release(AdblockEngine *) { qFatal("unexpected ABP release"); }
extern "C" bool ferric_browser_adblock_check(const AdblockEngine *, const unsigned char *, size_t,
    const unsigned char *, size_t, const unsigned char *, size_t, const unsigned char *, size_t,
    unsigned int *, unsigned char *, size_t, size_t *) { qFatal("unexpected ABP check"); }

class FerricEvidenceTest : public QObject {
    Q_OBJECT
private slots:
    void coalescesBatches() {
        FerricBrowserRequestInterceptor interceptor;
        QSignalSpy count(&interceptor, &FerricBrowserRequestInterceptor::blockedCountChanged);
        QSignalSpy unknown(&interceptor, &FerricBrowserRequestInterceptor::unknownContextCountChanged);
        QSignalSpy sites(&interceptor, &FerricBrowserRequestInterceptor::blockedSiteCountsChanged);
        QSignalSpy evidence(&interceptor, &FerricBrowserRequestInterceptor::evidenceChanged);
        for (int batch = 0; batch < 100; ++batch) {
            for (int event = 0; event < 100; ++event) interceptor.scheduleEvidenceChanged();
            QTRY_COMPARE(evidence.count(), batch + 1);
        }
        QCOMPARE(count.count() + unknown.count() + sites.count(), 300);
        QCOMPARE(evidence.count(), 100);
        QCOMPARE(interceptor.evidenceRevision(), 100ULL);
    }
    void snapshotsAndClearing() {
        FerricBrowserRequestInterceptor interceptor;
        const QString site = QStringLiteral("example.test");
        QVariantMap decision{{"resource_host", "tracker.test"}, {"matched_rule", "tracker.test"}, {"decision", "blocked"}};
        interceptor.blockedSiteCounts_.insert(site, 42);
        interceptor.blockedRequestExplanations_.insert(site, decision);
        interceptor.requestDecisionLog_.insert(site, QList<QVariantMap>{decision});
        QCOMPARE(interceptor.blockedSiteCount(" EXAMPLE.TEST. "), 42ULL);
        QCOMPARE(interceptor.blockedSiteCount("other.test"), 0ULL);
        const auto explanation = interceptor.blockedRequestExplanationFields(site);
        const auto decisions = interceptor.blockedRequestDecisionFields(site);
        QCOMPARE(explanation.size(), 13);
        QCOMPARE(decisions, explanation);
        interceptor.clearSiteEvidence(site);
        QCOMPARE(interceptor.blockedSiteCount(site), 0ULL);
        QVERIFY(interceptor.blockedRequestDecisionFields(site).isEmpty());
        QCOMPARE(decisions.front(), QStringLiteral("tracker.test"));
        QSignalSpy evidence(&interceptor, &FerricBrowserRequestInterceptor::evidenceChanged);
        QTRY_COMPARE(evidence.count(), 1);
    }
    void qmlCountBindingAndAggregateRefresh() {
        FerricBrowserRequestInterceptor interceptor;
        QQmlEngine engine;
        engine.rootContext()->setContextProperty("interceptor", &interceptor);
        QQmlComponent component(&engine);
        component.setData(R"(
            import QtQml
            QtObject {
                id: root
                property double siteCount: {
                    var revision = interceptor.evidenceRevision
                    return Number(interceptor.blockedSiteCount("example.test"))
                }
                property int refreshes: 0
                property double publishedCount: 0
                property QtObject observer: Connections {
                    target: interceptor
                    function onEvidenceChanged() {
                        root.refreshes++
                        root.publishedCount = Number(interceptor.blockedSiteCount("example.test"))
                    }
                }
            }
        )", QUrl());
        std::unique_ptr<QObject> object(component.create());
        QVERIFY2(object, qPrintable(component.errorString()));
        interceptor.blockedSiteCounts_.insert("example.test", 42);
        interceptor.scheduleEvidenceChanged();
        QTRY_COMPARE(object->property("siteCount").toDouble(), 42.0);
        QCOMPARE(object->property("refreshes").toInt(), 1);
        QCOMPARE(object->property("publishedCount").toDouble(), 42.0);
        interceptor.clearSiteEvidence("example.test");
        QTRY_COMPARE(object->property("siteCount").toDouble(), 0.0);
        QCOMPARE(object->property("refreshes").toInt(), 2);
        QCOMPARE(object->property("publishedCount").toDouble(), 0.0);
    }
    void reentrantEvidenceSchedulesAnotherBatch() {
        FerricBrowserRequestInterceptor interceptor;
        int calls = 0;
        connect(&interceptor, &FerricBrowserRequestInterceptor::evidenceChanged, &interceptor, [&] {
            if (++calls == 1) interceptor.scheduleEvidenceChanged();
        });
        interceptor.scheduleEvidenceChanged();
        QTRY_COMPARE(calls, 2);
    }
};
QTEST_GUILESS_MAIN(FerricEvidenceTest)
#include "tst_evidence.moc"
