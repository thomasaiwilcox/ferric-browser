#include "tst_application.h"
#include "owned_browser_keyboard.h"
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QFont>
#include <QImage>
#include <QQuickItem>
#include <QtWebEngineQuick/QQuickWebEngineProfile>
#include <QtTest>

void ApplicationTests::productPolishPrivacyIdentity_data() {
    QTest::addColumn<QString>("hostKind");
    QTest::addColumn<QString>("privacy");
    for (const auto &kind : {QString("primary"),QString("secondary"),QString("popup")})
        for (const auto &privacy : {QString("regular"),QString("private"),QString("temporary")})
            QTest::newRow(qPrintable(kind+"-"+privacy)) << kind << privacy;
}

void ApplicationTests::productPolishPrivacyIdentity() {
    QFETCH(QString,hostKind); QFETCH(QString,privacy);
    QTest::failOnWarning(QRegularExpression(".*(TypeError|ReferenceError|Binding loop|invalid context).*"));
    const QDir sourceRoot(QDir(QFileInfo(QString::fromUtf8(__FILE__)).absolutePath()).absoluteFilePath("../.."));
    for (const auto &name : {"FerricPrimaryBrowserWindow.qml","FerricBrowserWindow.qml","FerricPopupWindow.qml","FerricPrivacyHeader.qml"}) {
        const auto path=QString("qml/components/")+name;
        QFile source(sourceRoot.filePath(path)),embedded(":/qt/qml/io/github/ferricbrowser/"+path);
        QVERIFY(source.open(QIODevice::ReadOnly)); QVERIFY(embedded.open(QIODevice::ReadOnly));
        QVERIFY2(source.readAll()==embedded.readAll(),qPrintable("Rebuild stale production resource: "+path));
    }
    const auto initial=text("current_url");
    QString owner="testWindow",ui=owner+".browserUi";
    if (hostKind=="secondary") {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"+quoted(initial)
            +",windowProfileName:'identity-fixture',windowProfileLabel:'Identity fixture',windowPrivateProfile:"
            +(privacy=="private"?"true":"false")+",windowEphemeralProfile:"
            +(privacy=="temporary"?"true":"false")+",windowEphemeralInvocationToken:'secondary-identity-fixture'})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
        owner="testWindow.browserWindowRegistry[1].host";ui="testWindow.browserWindowRegistry[1].ui";
    } else if (hostKind=="popup") {
        QVERIFY(command("bind p hint --target pointer links"));
        key(Qt::Key_P);typeText(hintLabel("Open popup"));key(Qt::Key_Return);
        QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(),1);
        owner="testWindow.popupWindowRegistry[0].host";ui="testWindow.popupWindowRegistry[0].ui";
    }
    pageReceiver=owner+".activeWebView()";
    const auto expected=hostKind=="popup"?QUrl(initial).resolved(QUrl("/popup.html")).toString():initial;
    QTRY_VERIFY(evaluate(owner+".profileBootstrapComplete && !"+ui+".profile_bootstrap_pending && !!"+pageReceiver+" && !"+pageReceiver+".loading").toBool());
    QTRY_COMPARE(page("location.href").toString(),expected);
    QTRY_VERIFY(page("document.readyState==='complete'").toBool());
    QTRY_COMPARE(evaluate(ui+".load_state").toString(),"complete");
    auto profile=qobject_cast<QQuickWebEngineProfile *>(evaluate(pageReceiver+".profile").value<QObject *>());
    QVERIFY(profile);
    QCOMPARE(profile->isOffTheRecord(),privacy!="regular");
    auto host=qobject_cast<QQuickWindow *>(evaluate(owner).value<QObject *>());QVERIFY(host);
    host->requestActivate();QTRY_VERIFY(host->isActive());
    evaluate(pageReceiver+".forceActiveFocus()"); // Initial focus only.
    page("document.title='Page title';document.body.innerHTML='<button>needle action</button><p>needle text</p><div style=\"height:2200px\"></div>';true");
    QVERIFY(evaluate(ui+".set_runtime_setting('ui.tabs','never',true)").toBool());
    QVERIFY(evaluate(ui+".set_runtime_setting('ui.statusbar','never',true)").toBool());
    QVERIFY(evaluate(ui+".set_runtime_setting('ui.font_size_pt','26',true)").toBool());
    host->resize(360,640);QTRY_COMPARE(host->width(),360);
    QSignalSpy frame(host,&QQuickWindow::frameSwapped);host->update();QTRY_VERIFY(!frame.isEmpty());
    auto viewportView=qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());QVERIFY(viewportView);
    QTRY_COMPARE(page("innerWidth").toInt(),qRound(viewportView->width()));
    QTRY_COMPARE(page("innerHeight").toInt(),qRound(viewportView->height()));
    const auto artifacts=qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
    if (!artifacts.isEmpty()) {
        QDir().mkpath(artifacts);
        QVERIFY(host->grabWindow().save(artifacts+"/identity-"+hostKind+"-"+privacy+".png"));
    }
    const auto label=privacy=="private"?QString("Private"):privacy=="temporary"?QString("Temporary"):QString();
    auto header=host->findChild<QQuickItem *>("windowPrivacyHeader");
    qInfo()<<"privacy identity"<<hostKind<<privacy<<host->title()<<"header"<<(header && header->isVisible());
    if (label.isEmpty()) {
        QVERIFY(!header || !header->isVisible());
        if (header) QCOMPARE(header->height(),0.0);
        QVERIFY(!host->title().contains("Private") && !host->title().contains("Temporary"));
    } else {
        QVERIFY2(host->title().contains(label),qPrintable(host->title()));
        QVERIFY(header && header->isVisible());
        QCOMPARE(header->property("label").toString(),label);
        QVERIFY(header->height()>0);
        QVERIFY(!header->activeFocusOnTab() && !header->hasActiveFocus());
        auto text=host->findChild<QQuickItem *>("windowPrivacyLabel");QVERIFY(text);
        QCOMPARE(text->property("font").value<QFont>().pointSizeF(),host->property("font").value<QFont>().pointSizeF());
        QVERIFY(text->width()>0);
        QVERIFY(text->width()<=host->width());
        QVERIFY(text->property("contentWidth").toDouble()<=text->width()+1);
    }
    const auto assertIdentity=[&] {
        return label.isEmpty()?(!header || !header->isVisible())
            :(header && header->isVisible() && host->title().contains(label));
    };
    const auto pageFocused=[&] {
        auto view=qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());
        return view && host->activeFocusItem() && (view==host->activeFocusItem() || view->isAncestorOf(host->activeFocusItem()));
    };
    for (const auto &policy : {QString("always"),QString("never"),QString("in-mode")}) {
        QVERIFY(evaluate(ui+".set_runtime_setting('ui.statusbar',"+quoted(policy)+",true)").toBool());
        const auto viewport=page("JSON.stringify([innerWidth,innerHeight])").toString();
        QVERIFY(assertIdentity());
        sendOwnedBrowserKey(host,Qt::Key_Colon,Qt::ShiftModifier,"AC10");
        QTRY_COMPARE(evaluate(ui+".mode").toString(),"command");
        QVERIFY(assertIdentity());
        QCOMPARE(page("JSON.stringify([innerWidth,innerHeight])").toString(),viewport);
        sendOwnedBrowserKey(host,Qt::Key_Escape);
        QTRY_VERIFY(pageFocused());
        sendOwnedBrowserKey(host,Qt::Key_Slash,Qt::NoModifier,"AB10");
        QTRY_COMPARE(evaluate(ui+".mode").toString(),"search");
        for (const auto c : QString("needle"))sendOwnedBrowserKey(host,c.toUpper().unicode());
        QTRY_VERIFY(evaluate(owner+".searchWorkspace.result.found && !"+owner+".searchWorkspace.busy").toBool());
        QVERIFY(assertIdentity());
        QCOMPARE(page("JSON.stringify([innerWidth,innerHeight])").toString(),viewport);
        if (policy=="never" && !artifacts.isEmpty()) {
            frame.clear();host->update();QTRY_VERIFY(!frame.isEmpty());
            QVERIFY(host->grabWindow().save(artifacts+"/identity-"+hostKind+"-"+privacy+"-search.png"));
        }
        sendOwnedBrowserKey(host,Qt::Key_Escape);
        QTRY_VERIFY(pageFocused());
    }
    for (const auto &position : {QString("top"),QString("bottom"),QString("left"),QString("right")}) {
        QVERIFY(evaluate(ui+".set_runtime_setting('ui.tab_position',"+quoted(position)+",true)").toBool());
        QVERIFY(evaluate(ui+".set_runtime_setting('ui.tabs','always',true)").toBool());
        frame.clear();host->update();QTRY_VERIFY(!frame.isEmpty());
        QVERIFY(assertIdentity());
        if (header && header->isVisible()) {
            auto view=qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());QVERIFY(view);
            QVERIFY(view->mapToScene(QPointF()).y()>=header->mapToScene(QPointF(0,header->height())).y());
        }
    }
    page("scrollTo(0,0);true");
    sendOwnedBrowserKey(host,Qt::Key_J);
    QTRY_VERIFY(page("scrollY>0").toBool());
    page("document.title='Changed page title';true");
    if (hostKind=="popup") {
        QTRY_COMPARE(evaluate(pageReceiver+".title").toString(),"Changed page title");
        QTRY_VERIFY2(host->title().contains("Changed page title"),qPrintable(host->title()));
        page("document.title='Unsafe \\u202e page';true");
        QTRY_VERIFY(host->title().contains("Unsafe [bidi] page"));
        QVERIFY(!host->title().contains(QChar(0x202e)));
        if (!label.isEmpty()) QVERIFY(host->title().startsWith(label+" · "));
        const auto scrolled=page("scrollY").toDouble();
        sendOwnedBrowserKey(host,Qt::Key_K);
        QTRY_VERIFY(page("scrollY").toDouble()<scrolled);
    }
    QVERIFY(assertIdentity());
    QCOMPARE(page("location.href").toString(),expected);
}
