#include "tst_application.h"
#include <QFile>
#include <QElapsedTimer>
#include <QJsonArray>
#include <QJsonDocument>
#include <QtTest>

void ApplicationTests::hintPageKeyAcrossTabs()
{
    QFile fixture(QFINDTESTDATA("../fixtures/site-policy/hint-media-tabs.html"));
    QVERIFY(fixture.open(QIODevice::ReadOnly));
    const auto html=fixture.readAll();
    frameFixtures["/media-a"]=html;
    frameFixtures["/media-b"]=html;
    const auto origin=QStringLiteral("http://127.0.0.1:%1").arg(server.serverPort());
    QVERIFY(command("open "+origin+"/media-a"));
    QTRY_VERIFY(evaluate("!!testWindow.activeWebView()").toBool());
    QTRY_VERIFY(page("document.querySelector('video')?.readyState===4").toBool());
    const auto first=evaluate("testWindow.browserUi.tab_id_for_index(0)").toString();
    QVERIFY(command("open --target tab "+origin+"/media-b"));
    QTRY_VERIFY(evaluate("!!testWindow.activeWebView() && String(testWindow.activeWebView().url)==="+quoted(origin+"/media-b")).toBool());
    QTRY_VERIFY(page("location.pathname==='/media-b'&&document.querySelector('video')?.readyState===4").toBool());
    const auto second=evaluate("testWindow.browserUi.tab_id_for_index(1)").toString();
    QVERIFY(!first.isEmpty()&&!second.isEmpty()&&first!=second);

    int firstEvents=0,secondEvents=0;
    for(const auto &tab:{first,second,first}) {
        QVERIFY(command("tab-focus "+tab));
        QTRY_VERIFY(evaluate("!!testWindow.activeWebView()").toBool());
        QTRY_VERIFY(page("document.hasFocus()").toBool());
        auto &expectedEvents=tab==first?firstEvents:secondEvents;
        QCOMPARE(page("window.keys.length").toInt(),expectedEvents);
        QVERIFY(page("document.querySelector('video').paused").toBool());
        for(const bool paused:{false,true}) {
            // Use the actual default f binding and automatic focus handoff.
            // Explicitly probing the receiver here would hide tab-owner bugs.
            QElapsedTimer collectionClock;collectionClock.start();
            key(Qt::Key_F);
            QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.browserUi.mode==='hint' && testWindow.browserUi.hint_visible && !testWindow.hintRefreshPending").toBool(),5000);
            QTRY_VERIFY(evaluate("JSON.parse(testWindow.browserUi.page_key_route('p')).allowed===true").toBool());
            qInfo()<<"HINT_MEDIA_TAB_MEASUREMENT"<<(tab==first?"A":"B")
                <<"publication_ms"<<collectionClock.elapsed();
            key(Qt::Key_P);
            QTRY_COMPARE(page("document.querySelector('video').paused").toBool(),paused);
            QTRY_COMPARE(text("mode"),QStringLiteral("normal"));
            expectedEvents+=2;
            QCOMPARE(page("window.keys.length").toInt(),expectedEvents);
            QCOMPARE(page("JSON.stringify(window.keys.slice(-2))").toString(),
                QStringLiteral("[{\"type\":\"keydown\",\"key\":\"k\",\"trusted\":true},{\"type\":\"keyup\",\"key\":\"k\",\"trusted\":true}]"));
            key(Qt::Key_P);
            QTest::qWait(100);
            QCOMPARE(page("window.keys.length").toInt(),expectedEvents);
            QCOMPARE(page("document.querySelector('video').paused").toBool(),paused);
            key(Qt::Key_Escape);
            // Normal p is the pp/pP browser prefix. Let this independent
            // assertion's chord expire before the next f journey.
            QTest::qWait(1100);
        }
    }
    // Switching back also detects an event accidentally sent to the inactive
    // page; both documents and their media remain alive throughout the test.
    for(const auto &tab:{second,first}) {
        QVERIFY(command("tab-focus "+tab));
        QTRY_VERIFY(evaluate("!!testWindow.activeWebView()").toBool());
        QCOMPARE(page("window.keys.length").toInt(),tab==first?firstEvents:secondEvents);
        QVERIFY(page("document.querySelector('video').paused").toBool());
    }
}
