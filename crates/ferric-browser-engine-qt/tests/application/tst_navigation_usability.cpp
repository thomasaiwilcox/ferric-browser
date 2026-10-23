#include "tst_application.h"
#include "owned_browser_keyboard.h"
#include <QJsonDocument>
#include <QJsonObject>
#include <QtTest>
#include <QClipboard>
#include <QGuiApplication>
#include <QQuickItem>
#include <QDBusInterface>
#include <QDBusReply>
#include <QTcpSocket>
#include <QHostAddress>
#include <QKeyEvent>
#include <QFile>
#include <QDir>
#include <QFileInfo>
#include <QQmlContext>
#include <QPointer>
#include <QSet>
#include <QSignalSpy>
#include <QMetaMethod>
#include <QWebEngineLoadingInfo>

static QQuickItem *findRecoveryItem(QQuickItem *parent, const QString &name) {
    if (parent->objectName() == name) return parent;
    for (auto child : parent->childItems()) {
        if (auto found = findRecoveryItem(child, name)) return found;
    }
    return nullptr;
}

void ApplicationTests::navigationFailureRecovery_data() {
    QTest::addColumn<QString>("hostKind");
    for (const auto kind : {"primary", "secondary", "private", "popup"})
        QTest::newRow(kind) << QString(kind);
}

void ApplicationTests::navigationFailureRecovery() {
    QFETCH(QString, hostKind);
    QTest::failOnWarning(QRegularExpression(".*(TypeError|ReferenceError|Binding loop|invalid context).*"));
    const QDir sourceRoot(QDir(QFileInfo(QString::fromUtf8(__FILE__)).absolutePath()).absoluteFilePath("../.."));
    QStringList paths{"qml/components/FerricNavigationFailure.qml", "qml/components/FerricFormResubmission.qml", "qml/components/FerricNativeTabContent.qml",
        "qml/components/FerricPrimaryBrowserWindow.qml", "qml/scripts/EngineRequests.js"};
    // Every host uses the primary view factory; auxiliary hosts also use the
    // secondary shell. Check each resource before exercising that shell.
    if (hostKind != "primary") paths.append("qml/components/FerricBrowserWindow.qml");
    for (const auto &path : paths) {
        QFile source(sourceRoot.filePath(path)), embedded(":/qt/qml/io/github/ferricbrowser/" + path);
        QVERIFY(source.open(QIODevice::ReadOnly));
        QVERIFY(embedded.open(QIODevice::ReadOnly));
        QVERIFY2(source.readAll() == embedded.readAll(), qPrintable(QString("Rebuild the production engine: stale ") + path));
    }
    const auto initial = text("current_url");
    QString owner = "testWindow", ui = "testWindow.browserUi";
    if (hostKind == "secondary" || hostKind == "private") {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"
            + quoted(initial) + ",windowProfileName:'recovery-fixture',windowPrivateProfile:"
            + (hostKind == "private" ? "true" : "false") + "})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
        owner = "testWindow.browserWindowRegistry[1].host";
        ui = "testWindow.browserWindowRegistry[1].ui";
        pageReceiver = owner + ".activeView";
    } else if (hostKind == "popup") {
        QVERIFY(command("bind p hint --target pointer links"));
        key(Qt::Key_P);
        typeText(hintLabel("Open popup"));
        key(Qt::Key_Return);
        QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(), 1);
        owner = "testWindow.popupWindowRegistry[0].host";
        ui = "testWindow.popupWindowRegistry[0].ui";
        pageReceiver = "testWindow.popupWindowRegistry[0].view";
    }
    QTRY_VERIFY(evaluate(owner + ".profileBootstrapComplete && !" + ui
        + ".profile_bootstrap_pending && !!" + pageReceiver + " && !" + pageReceiver + ".loading").toBool());
    QTRY_VERIFY(page("document.readyState==='complete'").toBool());
    auto host = qobject_cast<QQuickWindow *>(evaluate(owner).value<QObject *>());
    QVERIFY(host);
    host->requestActivate();
    QTRY_VERIFY(host->isActive());
    const auto send = [&](int value) { sendOwnedBrowserKey(host, value); };
    auto surface = host->findChild<QQuickItem *>("navigationFailureSurface");
    QVERIFY(surface);
    auto input = host->findChild<QQuickItem *>("commandLineInput");
    QVERIFY(input);
    const auto originalTabs = evaluate(ui + ".tab_count").toInt();
    bool reject = true;
    int postCount = 0, getCount = 0;
    QByteArray lastTarget, lastPostBody;
    QTcpServer recoveryServer;
    QVERIFY(recoveryServer.listen(QHostAddress::LocalHost, 0));
    connect(&recoveryServer, &QTcpServer::newConnection, &recoveryServer, [&] {
        while (auto socket = recoveryServer.nextPendingConnection()) {
            connect(socket, &QTcpSocket::readyRead, socket, [&, socket, buffer = QByteArray()]() mutable {
                buffer += socket->readAll();
                const auto headerEnd = buffer.indexOf("\r\n\r\n");
                if (headerEnd < 0) return;
                qsizetype bodyLength = 0;
                for (const auto &line : buffer.left(headerEnd).split('\n')) {
                    if (line.toLower().startsWith("content-length:"))
                        bodyLength = line.mid(sizeof("content-length:") - 1).trimmed().toLongLong();
                }
                if (buffer.size() < headerEnd + 4 + bodyLength) return;
                const auto request = buffer;
                buffer.clear();
                const auto parts = request.split(' ');
                if (!parts.value(1).startsWith("/recovery")) {
                    socket->write("HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n");
                    socket->disconnectFromHost(); return;
                }
                if (parts.first() == "POST") { ++postCount; lastPostBody = request.mid(headerEnd + 4, bodyLength); }
                else if (parts.first() == "GET") ++getCount;
                lastTarget = parts.value(1);
                if (reject) { socket->abort(); return; }
                const QByteArray body = "<!doctype html><title>Recovered page</title><script>window.recovered=true</script>";
                socket->write("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: "
                    + QByteArray::number(body.size()) + "\r\n\r\n" + body);
                socket->disconnectFromHost();
            });
            connect(socket, &QTcpSocket::disconnected, socket, &QObject::deleteLater);
        }
    });
    const auto base = QString("http://127.0.0.1:%1/recovery").arg(recoveryServer.serverPort());
    const auto fragmentBase = base + "?value=a;;b&token=kept&label=owned%20space%2F%CE%BB";
    const auto address = fragmentBase + "#fragment";
    const auto open = [&](const QString &url) {
        QVERIFY(evaluate(ui + ".execute_ui_action('browser.url.open'," + quoted(url) + ")").toBool());
        evaluate(owner + (hostKind == "primary" ? ".executePendingEngineAction()" : ".applySwitcherEngineAction()"));
    };
    const auto reload = [&](const QString &options) {
        QVERIFY(evaluate(ui + ".execute_ui_action('browser.tab.reload'," + quoted(options) + ")").toBool());
        evaluate(owner + (hostKind == "primary" ? ".executePendingEngineAction()" : ".applySwitcherEngineAction()"));
    };
    open(address);
    QTRY_VERIFY_WITH_TIMEOUT(evaluate(ui + ".navigation_failure_visible").toBool(), 10000);
    QTRY_VERIFY(surface->isVisible() && surface->isEnabled());
    const auto closeFocused = [&] {
        const auto close = findRecoveryItem(surface, "commandDialogAction-cancel");
        return close && close->hasActiveFocus();
    };
    QTRY_VERIFY(closeFocused());
    auto label = findRecoveryItem(surface, "navigationFailureAddress");
    QVERIFY(label);
    QVERIFY(!label->property("text").toString().contains("token="));
    const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
    if (!artifacts.isEmpty()) QVERIFY(host->grabWindow().save(artifacts + "/navigation-recovery-" + hostKind + ".png"));

    // An explicit retry must reach the engine despite an unchanged URL.
    reject = false;
    send(Qt::Key_R);
    QTRY_VERIFY(!surface->isVisible());
    QTRY_VERIFY_WITH_TIMEOUT(page("window.recovered===true && document.readyState==='complete'").toBool(), 10000);
    QCOMPARE(page("location.href").toString(), address);
    QCOMPARE(lastTarget, QByteArray("/recovery?value=a;;b&token=kept&label=owned%20space%2F%CE%BB"));
    QCOMPARE(postCount, 0);
    const auto sameAddress = base + "?same-address=1&token=kept";
    open(sameAddress);
    QTRY_COMPARE(page("location.href").toString(), sameAddress);
    QTRY_VERIFY(page("window.recovered===true").toBool());
    const auto previousGets = getCount;
    page("window.openMarker=true;true");
    open(sameAddress);
    QTRY_VERIFY2(getCount > previousGets, qPrintable(QString("GETs=%1 before=%2; document=%3; marker=%4")
        .arg(getCount).arg(previousGets).arg(page("location.href").toString()).arg(page("typeof window.openMarker").toString())));
    QTRY_VERIFY(page("window.recovered===true && typeof window.openMarker==='undefined'").toBool());

    // A failed form submission retries with GET and retains the actual URL,
    // including the secret parameter omitted from the displayed safe address.
    reject = true;
    const auto formAddress = base + "?post=1&token=kept#failed-post";
    page("var form=document.createElement('form');form.method='POST';form.action=" + quoted(formAddress)
        + ";form.innerHTML='<input name=payload value=do-not-resend>';document.body.append(form);form.submit();true");
    QTRY_VERIFY_WITH_TIMEOUT(surface->isVisible() && surface->isEnabled(), 10000);
    QTRY_VERIFY(closeFocused());
    QVERIFY(postCount > 0);
    const auto submittedPosts = postCount;
    reject = false;
    send(Qt::Key_R);
    QTRY_VERIFY(!surface->isVisible());
    QTRY_VERIFY_WITH_TIMEOUT(page("window.recovered===true").toBool(), 10000);
    QCOMPARE(postCount, submittedPosts);
    QCOMPARE(page("location.href").toString(), formAddress);

    // A failed response does not prove that the server skipped the form action.
    // Ordinary Reload must ask before it sends that POST again.
    reject = true;
    const auto failedReloadAddress = base + "?failed-reload=1#form";
    page("var form=document.createElement('form');form.method='POST';form.action=" + quoted(failedReloadAddress)
        + ";form.innerHTML='<input name=payload value=failed-repeat>';document.body.append(form);form.submit();true");
    QTRY_VERIFY_WITH_TIMEOUT(surface->isVisible() && surface->isEnabled(), 10000);
    QTRY_VERIFY(closeFocused());
    const auto failedDetail = evaluate(ui + ".navigation_failure_detail").toString();
    const auto failedPosts = postCount;
    reject = false;
    reload("");
    QTest::qWait(150);
    auto failedFormSurface = findRecoveryItem(host->contentItem(), "formResubmissionSurface");
    QVERIFY2(failedFormSurface, qPrintable(QString("Failed form Reload sent POST before confirmation: %1 -> %2")
        .arg(failedPosts).arg(postCount)));
    const auto failedKeepFocused = [&] {
        const auto keep = findRecoveryItem(failedFormSurface, "commandDialogAction-cancel");
        return keep && keep->hasActiveFocus();
    };
    QTRY_VERIFY(failedFormSurface->isVisible() && failedKeepFocused());
    QVERIFY(!surface->isEnabled() && surface->opacity() == 0);
    QCOMPARE(postCount, failedPosts);
    send(Qt::Key_Return);
    QTRY_VERIFY(!failedFormSurface->isVisible());
    QTest::qWait(100);
    QCOMPARE(postCount, failedPosts);
    QCOMPARE(evaluate(ui + ".load_state").toString(), QString("failed"));
    QTRY_VERIFY(surface->isVisible() && closeFocused());
    QVERIFY(surface->isEnabled());
    QCOMPARE(surface->opacity(), qreal(1));
    QCOMPARE(evaluate(ui + ".navigation_failure_detail").toString(), failedDetail);
    send(Qt::Key_Escape);
    QTRY_VERIFY(!surface->isVisible());
    reload("");
    QTRY_VERIFY(failedFormSurface->isVisible() && failedKeepFocused());
    send(Qt::Key_Escape);
    QTRY_VERIFY(!failedFormSurface->isVisible());
    QVERIFY(!surface->isVisible());
    QCOMPARE(postCount, failedPosts);
    QCOMPARE(evaluate(ui + ".load_state").toString(), QString("failed"));
    reload("bypass-cache");
    QTRY_VERIFY(failedFormSurface->isVisible() && failedKeepFocused());
    send(Qt::Key_Y);
    QTRY_COMPARE(postCount, failedPosts + 1);
    QCOMPARE(lastPostBody, QByteArray("payload=failed-repeat"));
    QTRY_VERIFY(page("window.recovered===true").toBool());
    QTRY_COMPARE(evaluate(ui + ".load_state").toString(), QString("complete"));

    const auto successfulPostUrl = base + "?successful-post=1&label=owned%20space%2F%CE%BB#after";
    const auto beforeSuccessfulPost = postCount;
    page("var form=document.createElement('form');form.method='POST';form.action=" + quoted(successfulPostUrl)
        + ";form.innerHTML='<input name=payload value=explicit-post>';document.body.append(form);form.submit();true");
    QTRY_VERIFY(postCount > beforeSuccessfulPost);
    QTRY_COMPARE(page("location.href").toString(), successfulPostUrl);
    QTRY_COMPARE(evaluate(ui + ".load_state").toString(), QString("complete"));
    const auto completedPosts = postCount, beforePostOpen = getCount;
    QCOMPARE(lastPostBody, QByteArray("payload=explicit-post"));
    page("window.openMarker=true;true");
    // Chromium keeps a completed POST document for fragment-only visits.
    // Ferric must finish the request without losing its document method.
    open(successfulPostUrl);
    QTRY_COMPARE(evaluate(ui + ".load_state").toString(), QString("complete"));
    QVERIFY(page("window.openMarker===true").toBool());
    QCOMPARE(getCount, beforePostOpen);
    QCOMPARE(postCount, completedPosts);
    reload("");
    auto formSurface = findRecoveryItem(host->contentItem(), "formResubmissionSurface");
    QVERIFY(formSurface);
    const auto keepFormFocused = [&] {
        const auto keep = findRecoveryItem(formSurface, "commandDialogAction-cancel");
        return keep && keep->hasActiveFocus();
    };
    QTRY_VERIFY(formSurface->isVisible() && keepFormFocused());
    if (!artifacts.isEmpty()) QVERIFY(host->grabWindow().save(artifacts + "/form-resubmission-" + hostKind + ".png"));
    send(Qt::Key_Return);
    QTRY_VERIFY(!formSurface->isVisible());
    QTest::qWait(100);
    QCOMPARE(postCount, completedPosts);
    QVERIFY(page("window.openMarker===true").toBool());
    QCOMPARE(evaluate(ui + ".load_state").toString(), QString("complete"));
    reload("bypass-cache");
    QTRY_VERIFY(formSurface->isVisible() && keepFormFocused());
    send(Qt::Key_Y);
    QTRY_VERIFY(!formSurface->isVisible());
    QTRY_COMPARE(postCount, completedPosts + 1);
    QCOMPARE(lastPostBody, QByteArray("payload=explicit-post"));
    QTRY_VERIFY(page("window.recovered===true && typeof window.openMarker==='undefined'").toBool());
    QTRY_COMPARE(evaluate(ui + ".load_state").toString(), QString("complete"));
    const auto confirmedPosts = postCount;
    // Opening the address without its fragment starts a fresh GET.
    open(base + "?successful-post=1");
    QTRY_VERIFY(getCount > beforePostOpen);
    QTRY_VERIFY(page("window.recovered===true && typeof window.openMarker==='undefined'").toBool());
    QCOMPARE(postCount, confirmedPosts);

    reject = true;
    open(address);
    QTRY_VERIFY_WITH_TIMEOUT(surface->isVisible() && surface->isEnabled(), 10000);
    QTRY_VERIFY(closeFocused());
    send(Qt::Key_E);
    QTRY_COMPARE(evaluate(ui + ".mode").toString(), QString("command"));
    QTRY_VERIFY(input->hasActiveFocus());
    QCOMPARE(input->property("text").toString(), "open \"" + address + "\"");
    QCOMPARE(input->property("cursorPosition").toInt(), input->property("text").toString().size() - 1);
    send(Qt::Key_Backspace);
    send(Qt::Key_T);
    QCOMPARE(input->property("text").toString(), "open \"" + address + "\"");
    QTest::qWait(50);
    QVERIFY(input->hasActiveFocus());
    reject = false;
    send(Qt::Key_Return);
    QTRY_COMPARE(evaluate(ui + ".mode").toString(), QString("normal"));
    QTRY_VERIFY_WITH_TIMEOUT(page("window.recovered===true").toBool(), 10000);
    QCOMPARE(page("location.href").toString(), address);
    QCOMPARE(evaluate(ui + ".tab_count").toInt(), originalTabs);

    reject = true;
    open(base + "?back=1");
    QTRY_VERIFY_WITH_TIMEOUT(surface->isVisible() && surface->isEnabled(), 10000);
    QTRY_VERIFY(closeFocused());
    auto back = findRecoveryItem(surface, "commandDialogAction-back");
    QVERIFY(back && back->isEnabled());
    reject = false;
    send(Qt::Key_B);
    QTRY_VERIFY(!surface->isVisible());
    QTRY_VERIFY_WITH_TIMEOUT(page("window.recovered===true").toBool(), 10000);
    QVERIFY(page("location.href").toString() != base + "?back=1");
    if (hostKind != "primary") QCOMPARE(text("current_url"), initial);

    // Reopening an existing fragment is an engine same-document visit.
    QTRY_COMPARE(page("location.href").toString(), address);
    page("window.openMarker=true;true");
    const auto beforeFragments = getCount;
    const auto anotherFragment = fragmentBase + "#section%20%CE%BB";
    open(anotherFragment);
    QTRY_COMPARE(page("location.href").toString(), anotherFragment);
    QTRY_COMPARE(evaluate(ui + ".load_state").toString(), QString("complete"));
    QVERIFY(page("window.openMarker===true").toBool());
    QCOMPARE(getCount, beforeFragments);
    QVERIFY(evaluate(ui + ".execute_ui_action('browser.tab.back','')").toBool());
    evaluate(owner + (hostKind == "primary" ? ".executePendingEngineAction()" : ".applySwitcherEngineAction()"));
    QTRY_COMPARE(page("location.href").toString(), address);
    open(address);
    QTRY_COMPARE(evaluate(ui + ".load_state").toString(), QString("complete"));
    QVERIFY(page("window.openMarker===true").toBool());
    QCOMPARE(getCount, beforeFragments);
}

void ApplicationTests::productPolishStopState_data() {
    QTest::addColumn<QString>("hostKind");
    QTest::addColumn<QString>("phase");
    for (const auto &host : {QString("primary"),QString("secondary"),QString("private"),QString("popup")})
        for (const auto &phase : {QString("complete"),QString("failed"),QString("pending-get"),QString("pending-post"),QString("streaming-get"),QString("streaming-post"),QString("queued")})
            QTest::newRow(qPrintable(host+"-"+phase))<<host<<phase;
}

void ApplicationTests::productPolishStopState() {
    QFETCH(QString,hostKind);QFETCH(QString,phase);
    QString owner,ui;QQuickWindow *host=nullptr;
    preparePageSearchHost(hostKind,owner,ui,host);QVERIFY(host);
    const auto original=page("location.href").toString();
    page("window.stopStateMarker='finished document';true");
    auto view=qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());QVERIFY(view);
    qRegisterMetaType<QWebEngineLoadingInfo>();
    QMetaMethod loadingSignal;
    for(int i=0;i<view->metaObject()->methodCount();++i) {
        const auto candidate=view->metaObject()->method(i);
        if(candidate.methodType()==QMetaMethod::Signal && candidate.name()=="loadingChanged") {loadingSignal=candidate;break;}
    }
    QVERIFY(loadingSignal.isValid());
    QSignalSpy events(view,loadingSignal);QVERIFY(events.isValid());
    QTcpServer server;QList<QPointer<QTcpSocket>> responses;
    int requests=0,posts=0;
    const bool active=phase!="complete" && phase!="failed";
    const bool post=phase.endsWith("-post");
    if (phase!="complete") {
        QVERIFY(server.listen(QHostAddress::LocalHost,0));
        connect(&server,&QTcpServer::newConnection,&server,[&]{
            while (auto socket=server.nextPendingConnection()) {
                connect(socket,&QTcpSocket::readyRead,socket,[&,socket,buffer=QByteArray(),answered=false]() mutable {
                    if(answered)return;
                    buffer+=socket->readAll();const auto end=buffer.indexOf("\r\n\r\n");if(end<0)return;
                    qsizetype length=0;
                    for(const auto &line:buffer.left(end).split('\n'))
                        if(line.toLower().startsWith("content-length:"))length=line.mid(sizeof("content-length:")-1).trimmed().toLongLong();
                    if(buffer.size()<end+4+length)return;
                    answered=true;++requests;if(buffer.startsWith("POST "))++posts;
                    if(phase=="failed") {socket->abort();return;}
                    responses.append(socket);
                    if(phase.startsWith("streaming")) {
                        const QByteArray body="<!doctype html><title>Unfinished response</title><script>window.stopStateMarker='streamed document';window.stopStateStreaming=true</script><p>Unfinished response</p>"+QByteArray(8192,' ');
                        socket->write("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: "+QByteArray::number(body.size()+8192)+"\r\n\r\n"+body);
                        socket->flush();
                    }
                });
                connect(socket,&QTcpSocket::disconnected,socket,&QObject::deleteLater);
            }
        });
    }
    const auto destination=QString("http://127.0.0.1:%1/stop-state").arg(server.serverPort());
    if(phase=="failed" || phase=="queued") {
        QVERIFY(evaluate(ui+".execute_command("+quoted("open "+destination)+")").toBool());
    } else if(active) {
        if(post)page("var f=document.createElement('form');f.method='POST';f.action="+quoted(destination)+";f.innerHTML='<input name=payload value=once>';document.body.append(f);f.submit();true");
        else page("location.href="+quoted(destination)+";true");
    }
    if(phase=="failed") {
        QTRY_COMPARE(evaluate(ui+".load_state").toString(),"failed");
        QTRY_VERIFY(!view->property("loading").toBool());
        QTRY_VERIFY(evaluate(ui+".navigation_failure_visible").toBool());
        sendOwnedBrowserKey(host,Qt::Key_Escape);
        QTRY_VERIFY(!evaluate(ui+".navigation_failure_visible").toBool());
    } else if(active && phase!="queued") {
        QTRY_COMPARE(requests,1);
        QTRY_VERIFY(view->property("loading").toBool());
        QTRY_COMPARE(evaluate(ui+".load_state").toString(),"provisional");
        if(phase.startsWith("streaming"))QTRY_VERIFY(page("window.stopStateStreaming===true").toBool());
        else QCOMPARE(page("window.stopStateMarker").toString(),"finished document");
        sendOwnedBrowserKey(host,Qt::Key_1,Qt::AltModifier);
        QCOMPARE(evaluate(ui+".load_state").toString(),"provisional");
        QVERIFY(view->property("loading").toBool());
    }
    const auto projection=[&] {
        return evaluate("JSON.parse("+ui+".tab_transfer_payload("+ui+".tab_id_for_index("+ui+".active_tab_index)))").toMap();
    };
    const auto target=evaluate("JSON.stringify("+ui+".navigation_target_for_index("+ui+".active_tab_index))").toString();
    const auto before=projection().value("loading").toInt();
    const auto beforeUrl=evaluate(ui+".current_url").toString();
    const auto eventsBeforeStop=events.count();
    sendOwnedBrowserKey(host,Qt::Key_S);
    QTRY_VERIFY(!view->property("loading").toBool());
    const auto after=projection().value("loading").toInt();
    const auto beforeSelection=evaluate(ui+".load_state").toString();
    sendOwnedBrowserKey(host,Qt::Key_1,Qt::AltModifier);
    qInfo()<<"Stop state facts"<<hostKind<<phase<<"before"<<before<<"after"<<after
        <<"before selection"<<beforeSelection<<"after selection"<<evaluate(ui+".load_state")
        <<"native loading"<<view->property("loading")<<"events"<<events.count()
        <<"URL before"<<beforeUrl<<"after"<<evaluate(ui+".current_url")<<"native"<<view->property("url");
    for(const auto &arguments:events) {
        const auto info=arguments[0].value<QWebEngineLoadingInfo>();
        qInfo()<<"Stop native event"<<int(info.status())<<info.url()<<info.errorCode();
    }
    const auto expected=active?QString("cancelled"):phase=="failed"?QString("failed"):QString("complete");
    QCOMPARE(evaluate(ui+".load_state").toString(),expected);
    QCOMPARE(before,active?1:phase=="failed"?4:3);
    QCOMPARE(after,active?5:phase=="failed"?4:3);
    QCOMPARE(evaluate(ui+".current_url").toString(),view->property("url").toUrl().toString());
    if(!active)QCOMPARE(events.count(),eventsBeforeStop);
    QCOMPARE(evaluate("JSON.stringify("+ui+".navigation_target_for_index("+ui+".active_tab_index))").toString(),target);
    if(phase=="complete") {
        QCOMPARE(page("window.stopStateMarker").toString(),"finished document");
        QCOMPARE(page("location.href").toString(),original);
    } else if(active && phase!="queued") {
        QCOMPARE(requests,1);QCOMPARE(posts,post?1:0);
        QCOMPARE(page("window.stopStateMarker").toString(),phase.startsWith("streaming")?"streamed document":"finished document");
        QVERIFY(!evaluate(ui+".navigation_failure_visible").toBool());
    }
    QTRY_VERIFY(host->activeFocusItem() && (host->activeFocusItem()==view || view->isAncestorOf(host->activeFocusItem())));
}

void ApplicationTests::streamingHistoryTraversal_data() {
    QTest::addColumn<QString>("hostKind");
    QTest::addColumn<bool>("post");
    QTest::addColumn<QString>("phase");
    for (const auto host : {"primary", "secondary", "private", "popup"})
        for (const auto post : {false, true})
            for (const auto phase : {"streaming", "count-two", "pending", "native-start", "same-url", "encoded", "encoded-pending", "coalesced", "completed-same-url"})
                QTest::newRow(qPrintable(QString(host) + (post ? "-post-" : "-get-") + phase)) << QString(host) << post << QString(phase);
}

void ApplicationTests::streamingHistoryTraversal() {
    QFETCH(QString, hostKind);
    QFETCH(bool, post);
    QFETCH(QString, phase);
    QTest::failOnWarning(QRegularExpression(".*(TypeError|ReferenceError|Binding loop|invalid context).*"));
    const QDir sources(QDir(QFileInfo(QString::fromUtf8(__FILE__)).absolutePath()).absoluteFilePath("../.."));
    for (const auto &path : {QString("qml/components/FerricNativeTabContent.qml"),
            QString("qml/components/FerricPrimaryBrowserWindow.qml"), QString("qml/components/FerricRuntimeController.qml")}) {
        QFile source(sources.filePath(path)), embedded(":/qt/qml/io/github/ferricbrowser/" + path);
        QVERIFY(source.open(QIODevice::ReadOnly)); QVERIFY(embedded.open(QIODevice::ReadOnly));
        QVERIFY2(source.readAll() == embedded.readAll(), qPrintable("Rebuild production engine: stale " + path));
    }
    QString owner="testWindow", ui="testWindow.browserUi";
    const auto initial=text("current_url");
    if (hostKind=="secondary" || hostKind=="private") {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:" + quoted(initial)
            + ",windowProfileName:'history-stream-fixture',windowPrivateProfile:" + (hostKind=="private" ? "true" : "false") + "})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
        owner="testWindow.browserWindowRegistry[1].host";ui="testWindow.browserWindowRegistry[1].ui";
        pageReceiver=owner+".activeView";
    } else if (hostKind=="popup") {
        QVERIFY(command("bind p hint --target pointer links"));
        key(Qt::Key_P);typeText(hintLabel("Open popup"));key(Qt::Key_Return);
        QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(),1);
        owner="testWindow.popupWindowRegistry[0].host";ui="testWindow.popupWindowRegistry[0].ui";
        pageReceiver="testWindow.popupWindowRegistry[0].view";
    }
    QTRY_VERIFY(evaluate(owner+".profileBootstrapComplete && !"+ui+".profile_bootstrap_pending && !!"+pageReceiver).toBool());
    QTRY_VERIFY(page("document.readyState==='complete' && location.protocol==='http:'").toBool());
    QTRY_COMPARE(evaluate(ui+".load_state").toString(),QString("complete"));
    const auto startupUrl=page("location.href").toString();
    const auto historyCurrent=[&]{
        return evaluate("(function(){try{var h=JSON.parse("+ui+".content_history_export("+pageReceiver+".tabIndex));return h.entries[h.cursor].url==="+pageReceiver+".url.toString()}catch(error){return false}})()").toBool();
    };
    QTRY_VERIFY(historyCurrent());
    auto host=qobject_cast<QQuickWindow *>(evaluate(owner).value<QObject *>());
    auto view=qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());
    QVERIFY(host && view);host->requestActivate();QTRY_VERIFY(host->isActive());
    qRegisterMetaType<QWebEngineLoadingInfo>();
    QMetaMethod loadingSignal;
    for(int row=0;row<view->metaObject()->methodCount();++row) {
        const auto method=view->metaObject()->method(row);
        if(method.methodType()==QMetaMethod::Signal && method.name()=="loadingChanged") { loadingSignal=method;break; }
    }
    QVERIFY(loadingSignal.isValid());
    QSignalSpy loadingEvents(view,loadingSignal);
    QVERIFY(loadingEvents.isValid());
    const auto loadDiagnostic=[&]{
        QStringList events;
        for(const auto &arguments:loadingEvents) {
            if(arguments.isEmpty() || !arguments[0].canConvert<QWebEngineLoadingInfo>()) { events << "unknown load event";continue; }
            const auto info=arguments[0].value<QWebEngineLoadingInfo>();
            events << QString("status=%1 url=%2 code=%3 domain=%4 errorPage=%5 download=%6")
                .arg(int(info.status())).arg(info.url().toString()).arg(info.errorCode())
                .arg(int(info.errorDomain())).arg(info.isErrorPage()).arg(info.isDownload());
        }
        return "History load: state="+evaluate(ui+".load_state").toString()
            +" current="+evaluate(ui+".current_url").toString()
            +" native="+view->property("url").toUrl().toString()
            +" loading="+QString::number(view->property("loading").toBool())
            +" index="+QString::number(view->property("engineHistoryIndex").toInt())
            +" events=["+events.join("; ")+"]";
    };

    const auto dispatch=[&](const QString &value){
        QVERIFY(evaluate(ui+".execute_command("+quoted(value)+")").toBool());
        evaluate(owner+(hostKind=="primary" ? ".executePendingEngineAction()" : ".applySwitcherEngineAction()"));
    };
    const auto readJourney=[&](const QString &address, QString &node){
        evaluate(ui+".library_values=''");
        dispatch("journey --current");
        QTRY_COMPARE(evaluate(ui+".library_values").toString().split('\t').value(2),address);
        node=evaluate(ui+".library_values").toString().split('\t').value(0);
        sendOwnedBrowserKey(host,Qt::Key_Escape);
    };
    QString startupJourney;
    readJourney(startupUrl,startupJourney);QVERIFY(!startupJourney.isEmpty());
    const bool encoded=phase.startsWith("encoded");
    const bool pendingHeaders=phase=="pending" || phase=="encoded-pending";
    const auto formPath=encoded ? QString("/form?label=owned%20space%2F%CE%BB") : QString("/form");
    const auto streamPath=(phase=="same-url" ? QString("/form") : QString("/stream"))
        +(encoded ? QString("?label=owned%20space%2F%CE%BB") : QString());
    int streams=0, posts=0, commits=0, formResponses=0;
    bool startStreaming=false;
    QPointer<QTcpSocket> response;
    QTcpServer ownedServer;
    QVERIFY(ownedServer.listen(QHostAddress::LocalHost,0));
    connect(&ownedServer,&QTcpServer::newConnection,&ownedServer,[&]{
        while (auto socket=ownedServer.nextPendingConnection()) {
            connect(socket,&QTcpSocket::readyRead,socket,[&,socket,buffer=QByteArray(),answered=false]() mutable {
                if (answered) return;
                buffer+=socket->readAll();const auto end=buffer.indexOf("\r\n\r\n");if(end<0)return;
                qsizetype length=0;
                for(const auto &line:buffer.left(end).split('\n'))
                    if(line.toLower().startsWith("content-length:"))length=line.mid(sizeof("content-length:")-1).trimmed().toLongLong();
                if(buffer.size()<end+4+length)return;
                answered=true;
                const auto parts=buffer.left(buffer.indexOf("\r\n")).split(' ');
                const auto target=parts.value(1);
                if(parts.value(0)=="POST")++posts;
                if(startStreaming && streams==0 && target==streamPath.toUtf8()) {
                    ++streams;
                    response=socket;
                    if(pendingHeaders)return;
                    const QByteArray body="<!doctype html><title>Streaming history</title><script>window.streamingHistory=true;fetch('/committed',{cache:'no-store'})</script><div style='height:3000px'>Streaming history</div>"+QByteArray(8192,' ');
                    socket->write("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: "+QByteArray::number(body.size()+8192)+"\r\n\r\n"+body);
                    socket->flush();return;
                }
                if(target=="/committed")++commits;
                const QByteArray body=target.startsWith("/form")
                    ? "<!doctype html><title>Previous page</title><p id=previous>Previous page</p><script>window.ownedHistoryResponse="+QByteArray::number(++formResponses)+"</script>"
                    : "OK";
                socket->write("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: "+QByteArray::number(body.size())+"\r\n\r\n"+body);
                socket->disconnectFromHost();
            });
            connect(socket,&QTcpSocket::disconnected,socket,&QObject::deleteLater);
        }
    });
    const auto base=QString("http://127.0.0.1:%1").arg(ownedServer.serverPort());
    const int repeats=phase=="pending" ? qBound(1,qEnvironmentVariableIntValue("FERRIC_HISTORY_PENDING_REPEATS"),32) : 1;
    for(int repetition=0;repetition<repeats;++repetition) {
        startStreaming=false;streams=posts=commits=formResponses=0;response=nullptr;
        loadingEvents.clear();
        if(phase=="native-start") {
            dispatch("open rb://start");
            QTRY_COMPARE(evaluate(pageReceiver+".nativeUrl").toString(),QString("rb://start"));
        }
        dispatch("open "+base+formPath);
        QTRY_VERIFY(page("!!document.getElementById('previous') && document.readyState==='complete'").toBool());
        QTRY_COMPARE(evaluate(ui+".load_state").toString(),QString("complete"));
        QTRY_VERIFY(historyCurrent());
        QString formJourney;
        readJourney(base+formPath,formJourney);QVERIFY(!formJourney.isEmpty());
        if(phase=="completed-same-url") {
            dispatch("open "+base+"/bridge");
            QTRY_COMPARE(page("location.href").toString(),base+"/bridge");
            QTRY_COMPARE(evaluate(ui+".load_state").toString(),QString("complete"));
            if(post)page("var f=document.createElement('form');f.method='POST';f.action="+quoted(base+formPath)+";document.body.append(f);f.submit();true");
            else dispatch("open "+base+formPath);
            QTRY_COMPARE(page("location.href").toString(),base+formPath);
            QTRY_COMPARE(evaluate(ui+".load_state").toString(),QString("complete"));
            QTRY_COMPARE(page("window.ownedHistoryResponse").toInt(),2);
            QString newerJourney;
            readJourney(base+formPath,newerJourney);
            QVERIFY(newerJourney!=formJourney);
            dispatch("back --count 2");
            QTRY_COMPARE(page("window.ownedHistoryResponse").toInt(),3);
            QTRY_COMPARE(page("location.href").toString(),base+formPath);
            QTRY_COMPARE(evaluate(ui+".load_state").toString(),QString("complete"));
            QString currentJourney;
            readJourney(base+formPath,currentJourney);
            QCOMPARE(currentJourney,formJourney);
            QCOMPARE(posts,post ? 1 : 0);
            QVERIFY(!evaluate(ui+".navigation_failure_visible").toBool());
            return;
        }
        if(phase=="same-url") {
            dispatch("open "+base+"/bridge");
            QTRY_COMPARE(page("location.href").toString(),base+"/bridge");
            QTRY_VERIFY(page("document.readyState==='complete'").toBool());
            QTRY_COMPARE(evaluate(ui+".load_state").toString(),QString("complete"));
            QTRY_VERIFY(historyCurrent());
        }
        startStreaming=true;
        const auto streamUrl=base+streamPath;
        if(post)page("var f=document.createElement('form');f.method='POST';f.action="+quoted(streamUrl)+";f.innerHTML='<input name=payload value=history>';document.body.append(f);f.submit();true");
        else dispatch("open "+streamUrl);
        QTRY_COMPARE(streams,1);
        if(pendingHeaders) {
            QCOMPARE(commits,0);
            QTRY_VERIFY(page("!!document.getElementById('previous')").toBool());
        } else {
            QTRY_COMPARE(commits,1);
            QTRY_VERIFY(page("window.streamingHistory===true").toBool());
        }
        QVERIFY(response);QVERIFY(view->property("loading").toBool());
        evaluate(pageReceiver+".forceActiveFocus()");
        if(phase=="coalesced") {
            // Both commands arrive before the owned JavaScript probe can reply.
            dispatch("back");dispatch("back");
        } else {
            if(phase=="count-two" || phase=="native-start" || phase=="same-url")sendOwnedBrowserKey(host,Qt::Key_2);
            sendOwnedBrowserKey(host,Qt::Key_H,Qt::ShiftModifier);
        }
        if(phase=="native-start") {
            QTRY_COMPARE(evaluate(pageReceiver+".nativeUrl").toString(),QString("rb://start"));
            QTRY_COMPARE(evaluate(ui+".current_url").toString(),QString("rb://start"));
        } else {
            const auto destination=phase=="count-two" || pendingHeaders || phase=="coalesced" ? startupUrl : base+formPath;
            QTRY_COMPARE_WITH_TIMEOUT(page("location.href").toString(),destination,5000);
            QTRY_VERIFY(page("document.readyState==='complete'").toBool());
            if(phase!="count-two" && !pendingHeaders && phase!="coalesced")QTRY_VERIFY(page("!!document.getElementById('previous')").toBool());
            QTRY_VERIFY(historyCurrent());
        }
        bool reportedCancellation=false;
        const auto observedLoadState=[&]{
            const auto state=evaluate(ui+".load_state").toString();
            if(state=="cancelled" && !reportedCancellation) {
                reportedCancellation=true;
                qInfo() << "History cancellation" << loadDiagnostic();
            }
            return state;
        };
        QTRY_COMPARE(observedLoadState(),QString("complete"));
        if(phase!="native-start") {
            const auto destination=phase=="count-two" || pendingHeaders || phase=="coalesced" ? startupUrl : base+formPath;
            QString currentJourney;
            readJourney(destination,currentJourney);
            QCOMPARE(currentJourney,phase=="count-two" || pendingHeaders || phase=="coalesced" ? startupJourney : formJourney);
        }
        if(phase=="coalesced") {
            dispatch("open "+base+"/fresh");
            QTRY_COMPARE(page("location.href").toString(),base+"/fresh");
            QTRY_COMPARE(evaluate(ui+".load_state").toString(),QString("complete"));
            QTRY_VERIFY(historyCurrent());
            evaluate(pageReceiver+".forceActiveFocus()");
            sendOwnedBrowserKey(host,Qt::Key_H,Qt::ShiftModifier);
            QTRY_COMPARE(page("location.href").toString(),startupUrl);
            QTRY_COMPARE(evaluate(ui+".load_state").toString(),QString("complete"));
            QString currentJourney;
            readJourney(startupUrl,currentJourney);
            QCOMPARE(currentJourney,startupJourney);
        }
        QCOMPARE(streams,1);QCOMPARE(posts,post ? 1 : 0);
        QVERIFY(!evaluate(ui+".navigation_failure_visible").toBool());
    }
}

void ApplicationTests::streamingFormReload_data() {
    QTest::addColumn<QString>("hostKind");
    QTest::addColumn<QString>("phase");
    for (const auto host : {"primary", "secondary", "private", "popup"})
        for (const auto phase : {"pending", "streaming", "stopped", "finishes", "beforeunload", "beforeunload-fast", "beforeunload-complete", "beforeunload-open", "beforeunload-back", "beforeunload-open-complete", "beforeunload-back-complete"})
            QTest::newRow(qPrintable(QString(host) + "-" + phase)) << QString(host) << QString(phase);
}

void ApplicationTests::streamingFormReload() {
    QFETCH(QString, hostKind);
    QFETCH(QString, phase);
    const auto hasBeforeUnload=phase.startsWith("beforeunload");
    const auto completedBeforeUnload=hasBeforeUnload && phase.endsWith("-complete");
    const auto artifacts=qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
    QTest::failOnWarning(QRegularExpression(".*(TypeError|ReferenceError|Binding loop|invalid context).*"));
    const QDir sources(QDir(QFileInfo(QString::fromUtf8(__FILE__)).absolutePath()).absoluteFilePath("../.."));
    for (const auto &path : {QString("qml/components/FerricNativeTabContent.qml"),
            QString("qml/components/FerricFormResubmission.qml"), QString("qml/components/FerricPrimaryBrowserWindow.qml"),
            QString("qml/components/FerricWindowRequestsController.qml"), QString("qml/components/FerricBrowserWindow.qml")}) {
        QFile source(sources.filePath(path)), embedded(":/qt/qml/io/github/ferricbrowser/" + path);
        QVERIFY(source.open(QIODevice::ReadOnly)); QVERIFY(embedded.open(QIODevice::ReadOnly));
        QVERIFY2(source.readAll() == embedded.readAll(), qPrintable("Rebuild production engine: stale " + path));
    }
    QString owner="testWindow", ui="testWindow.browserUi";
    const auto initial=text("current_url");
    if (hostKind=="secondary" || hostKind=="private") {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:" + quoted(initial)
            + ",windowProfileName:'stream-fixture',windowPrivateProfile:" + (hostKind=="private" ? "true" : "false") + "})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
        owner="testWindow.browserWindowRegistry[1].host";ui="testWindow.browserWindowRegistry[1].ui";
        pageReceiver=owner+".activeView";
    } else if (hostKind=="popup") {
        QVERIFY(command("bind p hint --target pointer links"));
        key(Qt::Key_P);typeText(hintLabel("Open popup"));key(Qt::Key_Return);
        QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(),1);
        owner="testWindow.popupWindowRegistry[0].host";ui="testWindow.popupWindowRegistry[0].ui";
        pageReceiver="testWindow.popupWindowRegistry[0].view";
    }
    QTRY_VERIFY(evaluate(owner+".profileBootstrapComplete && !"+ui+".profile_bootstrap_pending && !!"+pageReceiver+" && !"+pageReceiver+".loading").toBool());
    const auto historyAtCurrentDocument=[&]{
        return evaluate("(function(){try{var h=JSON.parse("+ui+".content_history_export("+pageReceiver+".tabIndex));return h.entries[h.cursor].url==="+pageReceiver+".url.toString()}catch(error){return false}})()").toBool();
    };
    // A new view starts with loading=false before its startup request runs.
    // Establish the visits required by Back before driving this journey.
    QTRY_VERIFY(page("document.readyState==='complete' && location.protocol==='http:' && location.hostname==='127.0.0.1'").toBool());
    QTRY_COMPARE(evaluate(ui+".load_state").toString(),QString("complete"));
    QTRY_VERIFY(historyAtCurrentDocument());
    auto host=qobject_cast<QQuickWindow *>(evaluate(owner).value<QObject *>());
    auto view=qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());
    QVERIFY(host && view);host->requestActivate();QTRY_VERIFY(host->isActive());
    const auto send=[&](int value){sendOwnedBrowserKey(host,value);};
    const auto dispatch=[&](const QString &command){
        QVERIFY(evaluate(ui+".execute_command("+quoted(command)+")").toBool());
        evaluate(owner+(hostKind=="primary" ? ".executePendingEngineAction()" : ".applySwitcherEngineAction()"));
    };
    int posts=0, gets=0;
    QByteArray lastBody;
    QSet<int> committed;
    QHash<int,QPointer<QTcpSocket>> held;
    QHash<int,QByteArray> tails;
    QTcpServer ownedServer;
    QVERIFY(ownedServer.listen(QHostAddress::LocalHost,0));
    connect(&ownedServer,&QTcpServer::newConnection,&ownedServer,[&]{
        while (auto socket=ownedServer.nextPendingConnection()) {
            connect(socket,&QTcpSocket::readyRead,socket,[&,socket,buffer=QByteArray(),answered=false]() mutable {
                if (answered) return;
                buffer+=socket->readAll();const auto end=buffer.indexOf("\r\n\r\n");if (end<0) return;
                qsizetype size=0;
                for (const auto &line:buffer.left(end).split('\n'))
                    if (line.toLower().startsWith("content-length:")) size=line.mid(sizeof("content-length:")-1).trimmed().toLongLong();
                if (buffer.size()<end+4+size) return;
                answered=true;
                const auto parts=buffer.left(buffer.indexOf("\r\n")).split(' ');
                const auto target=parts.value(1);
                if (parts.value(0)=="POST") {
                    ++posts;lastBody=buffer.mid(end+4,size);held.insert(posts,socket);
                    if (phase=="pending") return;
                    const auto beforeUnload=hasBeforeUnload
                        ? QByteArray("<button id=edit>Activate page</button><script>addEventListener('beforeunload',function(e){e.preventDefault();e.returnValue='stay'})</script>") : QByteArray();
                    const QByteArray prefix="<!doctype html><meta charset=utf-8><title>Streaming form</title>"
                        "<body><script>window.streamPostSerial="+QByteArray::number(posts)
                        +";fetch('/committed?serial="+QByteArray::number(posts)+"',{cache:'no-store'})</script>"
                        +beforeUnload+"<div style='height:3000px'>Owned form response</div>"+QByteArray(8192,' ');
                    const QByteArray tail=QByteArray(8192,' ')+"</body>";
                    tails.insert(posts,tail);
                    socket->write("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: "
                        +QByteArray::number(prefix.size()+tail.size())+"\r\n\r\n"+prefix);
                    socket->flush();return;
                }
                if (target.startsWith("/committed?serial=")) committed.insert(target.mid(sizeof("/committed?serial=")-1).toInt());
                QByteArray body="OK";
                if (target.startsWith("/form")) {
                    ++gets;body="<!doctype html><title>Owned form</title><script>window.formGetRevision="+QByteArray::number(gets)+"</script>";
                }
                socket->write("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: "
                    +QByteArray::number(body.size())+"\r\n\r\n"+body);
                socket->disconnectFromHost();
            });
            connect(socket,&QTcpSocket::disconnected,socket,&QObject::deleteLater);
        }
    });
    const auto base=QString("http://127.0.0.1:%1").arg(ownedServer.serverPort());
    const auto release=[&](int serial){
        auto socket=held.value(serial);QVERIFY(socket);QVERIFY(tails.contains(serial));
        socket->write(tails.take(serial));socket->disconnectFromHost();
    };
    dispatch("open "+base+"/form");
    QTRY_VERIFY(page("window.formGetRevision===1 && document.readyState==='complete'").toBool());
    QTRY_COMPARE(evaluate(ui+".load_state").toString(),QString("complete"));
    QTRY_VERIFY(historyAtCurrentDocument());
    evaluate(pageReceiver+".forceActiveFocus()");
    const auto tabs=evaluate(ui+".tab_count").toInt();
    page("var f=document.createElement('form');f.method='POST';f.action="+quoted(base+"/posted"+(hasBeforeUnload ? "#stay" : ""))
        +";f.innerHTML='<input name=payload value=owned-fixture>';document.body.append(f);f.submit();true");
    QTRY_COMPARE(posts,1);
    QTRY_VERIFY(view->property("loading").toBool());
    const auto reload=[&](bool bypass){
        QVERIFY(evaluate(ui+".execute_ui_action('browser.tab.reload',"+quoted(bypass ? "bypass-cache" : "")+")").toBool());
        evaluate(owner+(hostKind=="primary" ? ".executePendingEngineAction()" : ".applySwitcherEngineAction()"));
    };
    const auto reloadState=[&]{
        return evaluate("JSON.stringify((function(){var v="+pageReceiver+";for(var i=0;i<v.children.length;i++){var c=v.children[i];if(typeof c.interceptReloadNavigation==='function')return {state:c.engineLoadState,loading:v.loading,attempt:!!c.reloadAttempt,verified:!!c.verifiedFormReload,verifiedCurrent:c.reloadOwnerCurrent(c.verifiedFormReload),serial:c.navigationSerial,canBack:v.contentCanGoBack,nativeBack:v.canGoBack,engineIndex:v.engineHistoryIndex,status:c.browserUi.status_text,history:c.browserUi.content_history_export(v.tabIndex)}}return null})())").toString();
    };
    if (phase=="pending") {
        reload(false);
        QTRY_COMPARE(gets,2);
        QTRY_VERIFY(page("window.formGetRevision===2 && document.readyState==='complete'").toBool());
        QCOMPARE(posts,1);
        auto prompt=findRecoveryItem(host->contentItem(),"formResubmissionSurface");
        QVERIFY(!prompt || !prompt->isVisible());
        return;
    }
    QTRY_VERIFY(committed.contains(1));
    QTRY_COMPARE(page("window.streamPostSerial").toInt(),1);
    if (phase=="stopped") {
        dispatch("stop");QTRY_VERIFY(!view->property("loading").toBool());
    }
    if (completedBeforeUnload) {
        release(1);
        QTRY_COMPARE(evaluate(ui+".load_state").toString(),QString("complete"));
        QTRY_VERIFY(!view->property("loading").toBool());
    }
    if (hasBeforeUnload) {
        const auto retainedUrl=page("location.href").toString();
        const auto point=page("var r=document.getElementById('edit').getBoundingClientRect();({x:r.x+r.width/2,y:r.y+r.height/2})").toMap();
        const auto position=view->mapToScene(QPointF(point["x"].toDouble(),point["y"].toDouble())).toPoint();
        QTest::mouseClick(host,Qt::LeftButton,Qt::NoModifier,position);
        QTRY_VERIFY(page("navigator.userActivation.hasBeenActive").toBool());
        if (phase.startsWith("beforeunload-open")) dispatch("open "+base+"/other");
        else if (phase.startsWith("beforeunload-back")) {
            QVERIFY2(view->property("contentCanGoBack").toBool(),qPrintable(reloadState()));
            dispatch("back");
        }
        else reload(false);
        QTRY_VERIFY2(evaluate("!!testWindow.pendingPageDialogRequest").toBool()
            || (findRecoveryItem(host->contentItem(),"formResubmissionSurface")
                && findRecoveryItem(host->contentItem(),"formResubmissionSurface")->isVisible()),qPrintable(reloadState()));
        if (!evaluate("!!testWindow.pendingPageDialogRequest").toBool()) {
            qInfo() << "BeforeUnload follows form approval";
            send(Qt::Key_Y);
            QTRY_VERIFY(evaluate("!!testWindow.pendingPageDialogRequest").toBool());
        }
        send(Qt::Key_Escape);
        QTRY_VERIFY(!evaluate("!!testWindow.pendingPageDialogRequest").toBool());
        QCOMPARE(posts,1);
        if (phase!="beforeunload-fast") {
            QTRY_COMPARE(evaluate(ui+".load_state").toString(),QString(completedBeforeUnload ? "complete" : "cancelled"));
            QVERIFY(!evaluate(ui+".navigation_failure_visible").toBool());
            QCOMPARE(page("location.href").toString(),retainedUrl);
            QTRY_COMPARE(evaluate(ui+".current_url").toString(),retainedUrl);
            const auto stayScroll=page("window.scrollY").toDouble();send(Qt::Key_J);
            QTRY_VERIFY(page("window.scrollY").toDouble()>stayScroll);
            if (!artifacts.isEmpty()) QVERIFY(host->grabWindow().save(artifacts+"/stay-"+hostKind+"-"+phase+".png"));
        }
    }
    const auto loadingBeforeDecision=view->property("loading").toBool();
    reload(false);
    if (hasBeforeUnload) {
        QTRY_VERIFY2(evaluate("!!testWindow.pendingPageDialogRequest").toBool()
            || (findRecoveryItem(host->contentItem(),"formResubmissionSurface")
                && findRecoveryItem(host->contentItem(),"formResubmissionSurface")->isVisible()),qPrintable(reloadState()));
        if (evaluate("!!testWindow.pendingPageDialogRequest").toBool()) {
            QTRY_VERIFY(findRecoveryItem(host->contentItem(),"pageDialogCancel")
                && findRecoveryItem(host->contentItem(),"pageDialogCancel")->hasActiveFocus());
            send(Qt::Key_Tab);
            QTRY_VERIFY2(findRecoveryItem(host->contentItem(),"pageDialogAccept")
                && findRecoveryItem(host->contentItem(),"pageDialogAccept")->hasActiveFocus(),
                qPrintable(evaluate(owner+".activeFocusItem ? "+owner+".activeFocusItem.objectName : 'no focus'").toString()));
            send(Qt::Key_Return);
        }
    }
    QPointer<QQuickItem> prompt;
    QTRY_VERIFY((prompt=findRecoveryItem(host->contentItem(),"formResubmissionSurface")) && prompt->isVisible());
    const auto keepFocused=[&]{auto keep=findRecoveryItem(prompt,"commandDialogAction-cancel");return keep && keep->hasActiveFocus();};
    QTRY_VERIFY(keepFocused());
    QCOMPARE(posts,1);
    QCOMPARE(page("window.streamPostSerial").toInt(),1);
    QCOMPARE(view->property("loading").toBool(),loadingBeforeDecision);
    const auto stateBeforeKeep=evaluate(ui+".load_state").toString();
    if (hasBeforeUnload) {
        QCOMPARE(stateBeforeKeep,QString(completedBeforeUnload ? "complete" : "cancelled"));
        QVERIFY(!evaluate(ui+".navigation_failure_visible").toBool());
    }
    if (!artifacts.isEmpty()) QVERIFY(host->grabWindow().save(artifacts+"/form-reload-"+hostKind+"-"+phase+".png"));
    if (phase=="finishes") {
        release(1);
        QTRY_COMPARE(evaluate(ui+".load_state").toString(),QString("complete"));
        QVERIFY(prompt->isVisible());
        reload(false);
        QTRY_COMPARE(evaluate(ui+".load_state").toString(),QString("complete"));
        send(Qt::Key_Return);
        QTRY_VERIFY(!prompt->isVisible());
        QCOMPARE(evaluate(ui+".load_state").toString(),QString("complete"));
        QCOMPARE(posts,1);
    } else {
        send(Qt::Key_Return);
        QTRY_VERIFY(!prompt->isVisible());QTest::qWait(100);QCOMPARE(posts,1);
        QCOMPARE(view->property("loading").toBool(),loadingBeforeDecision);
        QCOMPARE(evaluate(ui+".load_state").toString(),stateBeforeKeep);
    }
    if (hasBeforeUnload) QVERIFY(!evaluate(ui+".navigation_failure_visible").toBool());
    const auto before=page("window.scrollY").toDouble();send(Qt::Key_J);
    QTRY_VERIFY(page("window.scrollY").toDouble()>before);
    if (phase=="finishes" || hasBeforeUnload) return;
    reload(true);
    QTRY_VERIFY(prompt->isVisible() && keepFocused());
    send(Qt::Key_Y);
    QTRY_COMPARE(posts,2);
    QCOMPARE(lastBody,QByteArray("payload=owned-fixture"));
    QTRY_VERIFY(committed.contains(2));
    QTRY_COMPARE(page("window.streamPostSerial").toInt(),2);
    QTest::qWait(100);QCOMPARE(posts,2);
    release(2);
    QTRY_COMPARE(evaluate(ui+".load_state").toString(),QString("complete"));
    QCOMPARE(evaluate(ui+".tab_count").toInt(),tabs);
}

void ApplicationTests::keyboardGuideOwner_data() {
    navigationFailureRecovery_data();
}

void ApplicationTests::keyboardGuideOwner() {
    QFETCH(QString, hostKind);
    QTest::failOnWarning(QRegularExpression(".*(TypeError|ReferenceError|Binding loop|invalid context).*"));
    const QDir sourceRoot(QDir(QFileInfo(QString::fromUtf8(__FILE__)).absolutePath()).absoluteFilePath("../.."));
    const QStringList paths{"qml/components/FerricBindingHelp.qml", "qml/components/FerricGuide.qml",
        "qml/components/FerricFocusOverlayController.qml",
        "qml/components/FerricPrimaryBrowserWindow.qml", "qml/components/FerricBrowserWindow.qml", "qml/components/FerricPageServices.qml"};
    for (const auto &path : paths) {
        QFile source(sourceRoot.filePath(path)), embedded(":/qt/qml/io/github/ferricbrowser/" + path);
        QVERIFY(source.open(QIODevice::ReadOnly));
        QVERIFY(embedded.open(QIODevice::ReadOnly));
        QVERIFY2(source.readAll() == embedded.readAll(), qPrintable(QString("Rebuild the production engine: stale ") + path));
    }
    QString owner = "testWindow", ui = "testWindow.browserUi";
    const auto initial = text("current_url");
    if (hostKind == "secondary" || hostKind == "private") {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"
            + quoted(initial) + ",windowProfileName:'guide-fixture',windowPrivateProfile:"
            + (hostKind == "private" ? "true" : "false") + "})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
        owner = "testWindow.browserWindowRegistry[1].host";
        ui = "testWindow.browserWindowRegistry[1].ui";
        pageReceiver = owner + ".activeView";
    } else if (hostKind == "popup") {
        QVERIFY(command("bind p hint --target pointer links"));
        key(Qt::Key_P); typeText(hintLabel("Open popup")); key(Qt::Key_Return);
        QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(), 1);
        owner = "testWindow.popupWindowRegistry[0].host";
        ui = "testWindow.popupWindowRegistry[0].ui";
        pageReceiver = "testWindow.popupWindowRegistry[0].view";
    }
    QTRY_VERIFY(evaluate(owner + ".profileBootstrapComplete && !" + ui
        + ".profile_bootstrap_pending && !!" + pageReceiver + " && !" + pageReceiver + ".loading").toBool());
    QTRY_VERIFY(page("document.readyState==='complete'").toBool());
    if (hostKind != "primary") evaluate("testWindow.showBindingHelp('primary draft')");
    auto host = qobject_cast<QQuickWindow *>(evaluate(owner).value<QObject *>());
    QVERIFY(host);
    host->requestActivate(); QTRY_VERIFY(host->isActive());
    evaluate(pageReceiver + ".forceActiveFocus()");
    const auto show = [&](const QString &command) {
        QVERIFY(evaluate(ui + ".execute_command(" + quoted(command) + ")").toBool());
        evaluate(owner + (hostKind == "primary" ? ".executePendingEngineAction()" : ".applySwitcherEngineAction()"));
    };
    QVERIFY(evaluate(ui + ".execute_command('bind x zoom out')").toBool());
    evaluate(ui + ".enter_command()");
    auto commandInput = findRecoveryItem(host->contentItem(), "commandLineInput");
    QVERIFY(commandInput); QTRY_VERIFY(commandInput->hasActiveFocus());
    for (auto letter : {Qt::Key_B, Qt::Key_I, Qt::Key_N, Qt::Key_D, Qt::Key_I, Qt::Key_N,
            Qt::Key_G, Qt::Key_Minus, Qt::Key_L, Qt::Key_I, Qt::Key_S, Qt::Key_T}) sendOwnedBrowserKey(host, letter);
    sendOwnedBrowserKey(host, Qt::Key_Return);
    QQuickItem *guide = nullptr;
    QTRY_VERIFY2((guide = findRecoveryItem(host->contentItem(), "ferricGuide")),
        qPrintable(evaluate(ui + ".status_text").toString()));
    engine->rootContext()->setContextProperty("testGuide", guide);
    QTRY_VERIFY(guide->isVisible());
    QVERIFY(evaluate("testGuide.browserUi === " + ui).toBool());
    auto search = qobject_cast<QQuickItem *>(guide->property("searchInput").value<QObject *>());
    QVERIFY(search); QTRY_VERIFY(search->hasActiveFocus());
    for (auto letter : {Qt::Key_Z, Qt::Key_O, Qt::Key_O, Qt::Key_M}) sendOwnedBrowserKey(host, letter);
    QTRY_COMPARE(guide->property("searchText").toString(), QString("zoom"));
    QTRY_COMPARE(evaluate("testGuide.currentRow && testGuide.currentRow.identity").toString(), QString("zoom"));
    QVERIFY(evaluate("testGuide.rows.every(function(row){return !row.heading})").toBool());
    QVERIFY(guide->property("navigationHelp").toString().contains("Type to search"));
    QVERIFY(!guide->property("navigationHelp").toString().contains("j/k"));
    const auto artifacts = qEnvironmentVariable("FERRIC_TEST_ARTIFACT_DIR");
    if (!artifacts.isEmpty()) QVERIFY(host->grabWindow().save(artifacts + "/keyboard-guide-" + hostKind + ".png"));
    if (hostKind != "primary") {
        QVERIFY(evaluate("testWindow.bindingHelpVisible").toBool());
        QCOMPARE(evaluate("testWindow.bindingHelpSearch").toString(), QString("primary draft"));
    }
    sendOwnedBrowserKey(host, Qt::Key_Escape);
    QTRY_COMPARE(guide->property("searchText").toString(), QString(""));
    QVERIFY(guide->isVisible());
    sendOwnedBrowserKey(host, Qt::Key_Escape);
    QTRY_VERIFY(!guide->isVisible());
    const auto beforeScroll = page("window.scrollY").toDouble();
    sendOwnedBrowserKey(host, Qt::Key_J);
    QTRY_VERIFY(page("window.scrollY").toDouble() > beforeScroll);
    sendOwnedBrowserKey(host, Qt::Key_G);
    sendOwnedBrowserKey(host, Qt::Key_Question, Qt::ShiftModifier, "AB10");
    QTRY_VERIFY(guide->isVisible() && search->hasActiveFocus());
    sendOwnedBrowserKey(host, Qt::Key_Escape);
    QTRY_VERIFY(!guide->isVisible());
    show("binding-explain gg");
    QTRY_VERIFY(guide->isVisible() && search->hasActiveFocus());
    QTRY_COMPARE(evaluate("testGuide.currentRow && testGuide.currentRow.identity").toString(), QString("scroll-to"));
    sendOwnedBrowserKey(host, Qt::Key_Down);
    QVERIFY(guide->property("navigationHelp").toString().contains("j/k browse"));
    sendOwnedBrowserKey(host, Qt::Key_Return);
    QTRY_VERIFY(guide->property("navigationHelp").toString().contains("j/k scroll"));
    if (!artifacts.isEmpty()) QVERIFY(host->grabWindow().save(artifacts + "/keyboard-guide-reader-" + hostKind + ".png"));
    const auto beforeGuideSearchScroll = page("window.scrollY").toDouble();
    const auto beforeGuideSearchMode = evaluate(ui + ".mode").toString();
    const auto beforeGuidePageSearchText = evaluate(ui + ".search_text").toString();
    sendOwnedBrowserKey(host, Qt::Key_Slash, Qt::NoModifier, "AB10");
    QTRY_VERIFY(search->hasActiveFocus() && search->isVisible());
    QCOMPARE(search->property("selectedText").toString(), QString("gg"));
    QVERIFY(!guide->property("readerActive").toBool());
    for (auto letter : {Qt::Key_Z, Qt::Key_O, Qt::Key_O, Qt::Key_M}) sendOwnedBrowserKey(host, letter);
    QTRY_COMPARE(guide->property("searchText").toString(), QString("zoom"));
    QTRY_COMPARE(evaluate("testGuide.currentRow && testGuide.currentRow.identity").toString(), QString("zoom"));
    QCOMPARE(page("window.scrollY").toDouble(), beforeGuideSearchScroll);
    QCOMPARE(evaluate(ui + ".mode").toString(), beforeGuideSearchMode);
    QCOMPARE(evaluate(ui + ".search_text").toString(), beforeGuidePageSearchText);
    sendOwnedBrowserKey(host, Qt::Key_Down);
    sendOwnedBrowserKey(host, Qt::Key_Slash, Qt::NoModifier, "AB10");
    QTRY_VERIFY(search->hasActiveFocus());
    QCOMPARE(search->property("selectedText").toString(), QString("zoom"));
    sendOwnedBrowserKey(host, Qt::Key_Return);
    sendOwnedBrowserKey(host, Qt::Key_Return);
    QTRY_VERIFY(guide->property("navigationHelp").toString().contains("j/k scroll"));
    if (!artifacts.isEmpty()) QVERIFY(host->grabWindow().save(artifacts + "/keyboard-guide-zoom-reader-" + hostKind + ".png"));
    auto articleScroll = findRecoveryItem(guide, "ferricGuideReaderScroll");
    QVERIFY(articleScroll);
    auto articleViewport = articleScroll->property("contentItem").value<QObject *>();
    QVERIFY(articleViewport);
    const auto articleFacts = [&] {
        const auto focus = host->activeFocusItem();
        auto pane = findRecoveryItem(guide, "ferricGuideArticle");
        return QJsonDocument(QJsonObject{
            {"host", hostKind}, {"contentY", articleViewport->property("contentY").toDouble()},
            {"contentHeight", articleViewport->property("contentHeight").toDouble()},
            {"height", articleViewport->property("height").toDouble()},
            {"width", articleViewport->property("width").toDouble()},
            {"articleFocus", pane && pane->hasActiveFocus()},
            {"focus", focus ? QString::fromLatin1(focus->metaObject()->className()) + ":" + focus->objectName() : "none"},
            {"inputActive", guide->property("inputActive").toBool()},
            {"readerActive", guide->property("readerActive").toBool()},
            {"compact", guide->property("compact").toBool()},
            {"mode", evaluate(ui + ".mode").toString()}
        }).toJson(QJsonDocument::Compact);
    };
    const auto beforeArticleFacts = articleFacts();
    const auto beforeArticlePage = articleViewport->property("contentY").toDouble();
    sendOwnedBrowserKey(host, Qt::Key_PageDown);
    QTRY_VERIFY2(articleViewport->property("contentY").toDouble() > beforeArticlePage,
        qPrintable(QString::fromUtf8("before=" + beforeArticleFacts + " after=" + articleFacts())));
    QCOMPARE(evaluate("testGuide.currentRow.identity").toString(), QString("zoom"));
    QCOMPARE(page("window.scrollY").toDouble(), beforeGuideSearchScroll);
    if (!artifacts.isEmpty()) QVERIFY(host->grabWindow().save(artifacts + "/keyboard-guide-zoom-body-" + hostKind + ".png"));
    sendOwnedBrowserKey(host, Qt::Key_Escape);
    QTRY_VERIFY(guide->property("navigationHelp").toString().contains("j/k browse"));
    sendOwnedBrowserKey(host, Qt::Key_Escape);
    QTRY_VERIFY(!guide->isVisible());
    show("binding-explain x");
    QTRY_VERIFY(guide->isVisible() && search->hasActiveFocus());
    QTRY_COMPARE(guide->property("searchText").toString(), QString("x"));
    QCOMPARE(search->property("text").toString(), QString("x"));
    QTRY_COMPARE(evaluate("testGuide.currentRow && testGuide.currentRow.identity").toString(), QString("zoom"));
    QVERIFY(evaluate("testGuide.currentRow.bindings.some(function(binding){return binding.keys==='x'})"
        " && testGuide.currentRow.bindings.some(function(binding){return binding.keys==='+'})").toBool());
    QVERIFY(evaluate("testGuide.browserUi === " + ui).toBool());
    if (hostKind != "primary") {
        QCOMPARE(evaluate("testWindow.bindingHelpSearch").toString(), QString("primary draft"));
        QCOMPARE(text("current_url"), initial);
    }
}

void ApplicationTests::navigationJourneys_data() {
    QTest::addColumn<QString>("method");
    for (const auto name : {"open", "link", "hint", "push-state", "same-url-state", "mouse-back"})
        QTest::newRow(name) << QString(name);
}
void ApplicationTests::navigationJourneys() {
    QFETCH(QString, method);
    QTest::failOnWarning(QRegularExpression(".*(TypeError|ReferenceError|invalid context).*"));
    const auto first = text("current_url");
    const auto second = first + "?navigation-second=1";
    const auto third = first + "?navigation-third=1";
    if (method == "open" || method == "mouse-back") {
        interactiveCommand("open " + second);
    } else if (method == "push-state" || method == "same-url-state") {
        page("history.pushState({position:1},''," + quoted(method == "same-url-state" ? first : second) + ");true");
    } else {
        page("document.getElementById('new').href=" + quoted(second) + ";true");
        if (method == "hint") {
            QVERIFY(command("bind x hint"));
            key(Qt::Key_X);
            QTRY_COMPARE(text("mode"),"hint");
            QCOMPARE(text("hint_family"),"actions");
            key(Qt::Key_Escape);
            QVERIFY(command("set --temp hints.auto_follow=never"));
            key(Qt::Key_F);
            const auto label = hintLabel("New document");
            QVERIFY(!label.isEmpty());
            typeText(label);
            key(Qt::Key_Return);
        } else page("document.getElementById('new').click();true");
    }
    const auto destination = method == "same-url-state" ? first : second;
    QTRY_VERIFY(page("location.href===" + quoted(destination) + " && document.readyState==='complete'").toBool());
    QTRY_VERIFY(evaluate("testWindow.activeWebView().contentCanGoBack").toBool());
    if (method == "push-state" || method == "same-url-state") {
        page("history.pushState({position:2},''," + quoted(third) + ");true");
    } else {
        page("document.getElementById('new').href=" + quoted(third) + ";document.getElementById('new').click();true");
    }
    QTRY_VERIFY(page("location.href===" + quoted(third) + " && document.readyState==='complete'").toBool());
    QTRY_VERIFY(evaluate("testWindow.activeWebView().contentCanGoBack").toBool());
    qInfo() << "navigation history" << method << evaluate("testWindow.browserUi.content_history_export(0)");
    if (method == "mouse-back") QTest::mouseClick(window, Qt::BackButton, Qt::NoModifier, QPoint(window->width()/2,window->height()/2));
    else key(Qt::Key_H, Qt::ShiftModifier);
    QTRY_COMPARE_WITH_TIMEOUT(page("location.href").toString(), destination, 5000);
    if (method == "same-url-state") QCOMPARE(page("history.state.position").toInt(),1);
    interactiveCommand("back");
    QTRY_COMPARE_WITH_TIMEOUT(page("location.href").toString(), first, 5000);
    if (method == "same-url-state") QVERIFY(page("history.state===null").toBool());
    key(Qt::Key_2);
    QVERIFY2(text("binding_overlay").startsWith("count"),qPrintable("Count prefix was lost: "+text("mode")+" "+text("load_state")+" "+text("status_text")));
    key(Qt::Key_L, Qt::ShiftModifier);
    QTRY_VERIFY2_WITH_TIMEOUT(page("location.href").toString()==third,
        qPrintable("Counted Forward: "+text("mode")+" "+text("load_state")+" "+text("status_text")
            +" history="+evaluate("testWindow.browserUi.content_history_export(0)").toString()),5000);
    if (method == "open") {
        page("window.reloadMarker='must disappear';true");
        key(Qt::Key_R);
        QTRY_VERIFY(page("typeof window.reloadMarker==='undefined' && document.readyState==='complete'").toBool());
        QCOMPARE(page("location.href").toString(),third);
    }
}

void ApplicationTests::urlCopyStatusBar_data() {
    QTest::addColumn<bool>("secondary");
    QTest::newRow("primary-single-tab") << false;
    QTest::newRow("secondary-single-tab") << true;
}
void ApplicationTests::urlCopyStatusBar() {
    QFETCH(bool, secondary);
    const auto initial = text("current_url");
    QString ui = "testWindow.browserUi";
    QString owner = "testWindow";
    auto hostWindow = window;
    auto expected = initial;
    if (secondary) {
        expected += "?copy-owner=secondary";
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"
            + quoted(expected) + ",windowProfileName:'copy-status',windowProfileLabel:'Copy status'})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(),2);
        owner = "testWindow.browserWindowRegistry[1].host";
        ui = "testWindow.browserWindowRegistry[1].ui";
        QTRY_VERIFY(evaluate(owner + ".profileBootstrapComplete && !" + ui + ".profile_bootstrap_pending").toBool());
        hostWindow = qobject_cast<QQuickWindow *>(evaluate(owner).value<QObject *>());
        QVERIFY(hostWindow);
        QTRY_COMPARE(evaluate(ui + ".current_url").toString(),expected);
        QTRY_VERIFY(evaluate(owner + ".activeView && " + owner + ".activeView.ownsTab() && !" + owner + ".activeView.loading && String(" + owner + ".activeView.url)===" + quoted(expected)).toBool());
        pageReceiver = owner + ".activeView";
        QTRY_VERIFY(page("location.href===" + quoted(expected) + " && document.readyState==='complete'").toBool());
        QTRY_COMPARE(evaluate(ui + ".load_state").toString(),"complete");
    }
    QCOMPARE(evaluate(ui + ".tab_count").toInt(),1);
    auto button = hostWindow->findChild<QQuickItem *>("copyCurrentUrlButton");
    QVERIFY(button);
    // The default in-mode policy deliberately hides all status chrome in
    // Normal mode. Showing a copy control must not override that preference.
    QVERIFY(evaluate(ui + ".set_runtime_setting('ui.statusbar','in-mode',true)").toBool());
    QTRY_VERIFY(!button->isVisible());
    QVERIFY(evaluate(ui + ".set_runtime_setting('ui.statusbar','always',true)").toBool());
    QTRY_VERIFY(!evaluate(owner + ".tabStripVisible").toBool());
    QTRY_VERIFY(button->isVisible() && button->isEnabled());
    hostWindow->requestActivate();
    QTRY_VERIFY(hostWindow->isActive());
    QGuiApplication::clipboard()->setText("before status copy");
    const auto point = button->mapToScene(QPointF(button->width()/2,button->height()/2)).toPoint();
    QTest::mouseClick(hostWindow,Qt::LeftButton,Qt::NoModifier,point);
    QTRY_COMPARE(QGuiApplication::clipboard()->text(),expected);
    QTRY_COMPARE(evaluate(ui + ".status_text").toString(),"URL copied");
    auto notice = window->findChild<QQuickItem *>("copyNotice");
    auto bar = hostWindow->findChild<QQuickItem *>("browserStatusBar");
    QVERIFY(notice);
    QVERIFY(bar);
    QTRY_VERIFY(notice->isVisible());
    QVERIFY(notice->mapToScene(QPointF(0, notice->height())).y() <= bar->mapToScene(QPointF()).y() - 8);
    if (secondary) QCOMPARE(text("current_url"),initial);
}

void ApplicationTests::bottomChromeOverlay_data() {
    QTest::addColumn<QString>("hostKind");
    QTest::addColumn<QString>("tabPosition");
    for (const auto host : {"primary", "secondary", "popup"}) {
        for (const auto position : {"top", "bottom", "left"}) {
            const auto name = QString(host) + "-" + position;
            QTest::newRow(qPrintable(name)) << QString(host) << QString(position);
        }
    }
}

void ApplicationTests::bottomChromeOverlay() {
    QFETCH(QString, hostKind);
    QFETCH(QString, tabPosition);
    QTest::failOnWarning(QRegularExpression(".*(TypeError|ReferenceError|Binding loop).*"));
    QString owner = "testWindow", ui = "testWindow.browserUi";
    auto host = window;
    if (hostKind == "secondary") {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"
            + quoted(text("current_url")) + ",windowProfileName:'overlay-fixture'})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
        owner = "testWindow.browserWindowRegistry[1].host";
        ui = "testWindow.browserWindowRegistry[1].ui";
        pageReceiver = owner + ".activeView";
    } else if (hostKind == "popup") {
        QVERIFY(command("bind p hint --target pointer links"));
        key(Qt::Key_P);
        typeText(hintLabel("Open popup"));
        key(Qt::Key_Return);
        QTRY_COMPARE(evaluate("testWindow.popupWindowRegistry.length").toInt(), 1);
        owner = "testWindow.popupWindowRegistry[0].host";
        ui = "testWindow.popupWindowRegistry[0].ui";
        pageReceiver = "testWindow.popupWindowRegistry[0].view";
    }
    QTRY_VERIFY(evaluate(owner + ".profileBootstrapComplete && !" + ui
        + ".profile_bootstrap_pending && !!" + pageReceiver + " && !" + pageReceiver + ".loading").toBool());
    host = qobject_cast<QQuickWindow *>(evaluate(owner).value<QObject *>());
    QVERIFY(host);
    QTRY_VERIFY(page("document.readyState==='complete' && !!document.getElementById('button')").toBool());
    QVERIFY(evaluate(ui + ".set_runtime_setting('ui.tabs','always',true)").toBool());
    QVERIFY(evaluate(ui + ".set_runtime_setting('ui.tab_position'," + quoted(tabPosition) + ",true)").toBool());
    evaluate(owner + ".commandNoticeVisible=false");
    auto notice = host->findChild<QQuickItem *>("profileHealthNotice");
    QVERIFY(notice);
    if (tabPosition == "bottom") {
        QVERIFY(notice->setProperty("noticeText", "Overlay layout fixture: persistent profile notice"));
        QTRY_VERIFY(notice->height() > 0);
    }
    auto view = qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());
    QVERIFY(view);
    page("document.body.style.minHeight='4000px';"
         "window.overlayFooter=document.createElement('div');"
         "overlayFooter.style.cssText='position:fixed;bottom:0;left:0;width:100%;height:20px';"
         "document.body.append(overlayFooter);scrollTo(0,300);true");
    QTRY_COMPARE(page("scrollY").toInt(), 300);
    QTest::qWait(100);
    const auto geometry = QRectF(view->position(), view->size());
    const auto noticeY = notice->y();
    const QString snapshot = "JSON.stringify([innerWidth,innerHeight,scrollX,scrollY,overlayFooter.getBoundingClientRect().y])";
    const auto before = page(snapshot).toString();
    page("window.overlayResizes=0;addEventListener('resize',()=>++window.overlayResizes);true");
    QSignalSpy heightChanges(view, &QQuickItem::heightChanged);
    QSignalSpy widthChanges(view, &QQuickItem::widthChanged);
    const auto verifyStable = [&] {
        QTest::qWait(50);
        QCOMPARE(QRectF(view->position(), view->size()), geometry);
        QCOMPARE(notice->y(), noticeY);
        QCOMPARE(page(snapshot).toString(), before);
        QCOMPARE(page("overlayResizes").toInt(), 0);
        QCOMPARE(heightChanges.count(), 0);
        QCOMPARE(widthChanges.count(), 0);
    };
    for (const auto mode : {"insert", "command", "search"}) {
        const auto enter = QString(mode) == "search" ? "enter_search(false)"
                                                    : QString("enter_") + mode + "()";
        evaluate(ui + "." + enter);
        QTRY_COMPARE(evaluate(ui + ".mode").toString(), QString(mode));
        verifyStable();
        evaluate(ui + ".escape()");
        QTRY_COMPARE(evaluate(ui + ".mode").toString(), QString("normal"));
        verifyStable();
    }
    evaluate(owner + ".commandNoticeVisible=true");
    verifyStable();
    evaluate(owner + ".commandNoticeVisible=false");
    for (const auto policy : {"always", "never", "in-mode"}) {
        QVERIFY(evaluate(ui + ".set_runtime_setting('ui.statusbar','" + policy + "',true)").toBool());
        verifyStable();
    }
    QVERIFY(evaluate(ui + ".set_runtime_setting('ui.statusbar','always',true)").toBool());
    auto bar = host->findChild<QQuickItem *>("browserStatusBar");
    QVERIFY(bar);
    QTRY_VERIFY(bar->isVisible());
    host->requestActivate();
    QTRY_VERIFY(host->isActive());
    const auto clicks = page("window.allClicks||0").toInt();
    // Stay inside the page, above its scrollbars, and away from responsive chrome controls.
    const auto point = QPoint(qRound(view->mapToScene(QPointF(20, 0)).x()),
        qRound(bar->mapToScene(QPointF(0, bar->height()/4)).y()));
    auto copyButton = host->findChild<QQuickItem *>("copyCurrentUrlButton");
    QVERIFY(copyButton);
    QVERIFY(!copyButton->contains(copyButton->mapFromScene(point)));
    const auto click = [&] {
        if (!QGuiApplication::platformName().startsWith("wayland")
                || qEnvironmentVariable("FERRIC_PORTAL_TEST_BUS") != "isolated") {
            QTest::mouseClick(host, Qt::LeftButton, Qt::NoModifier, point);
            return;
        }
        // WebEngine needs real seat pointer focus on Wayland, not a synthetic QWindow event.
        host->setTitle("Ferric overlay pointer fixture");
        QDBusInterface compositor("org.ferricbrowser.ParentFixture",
            "/org/ferricbrowser/ParentFixture", "org.ferricbrowser.ParentFixture",
            QDBusConnection::sessionBus());
        QVERIFY(compositor.isValid());
        QString windowId;
        QTRY_VERIFY(([&] {
            QDBusReply<QVariantList> reply = compositor.call("Windows");
            if (!reply.isValid()) return false;
            for (const auto &item : reply.value()) {
                const auto candidate = qdbus_cast<QVariantMap>(item);
                if (candidate["title"].toString() == host->title()
                        && candidate["pid"].toLongLong() == QCoreApplication::applicationPid()) {
                    windowId = candidate["id"].toString();
                    return !windowId.isEmpty();
                }
            }
            return false;
        })());
        // Refresh native hover after the overlay disappears beneath a stationary pointer.
        QDBusReply<bool> moved = compositor.call("Move", windowId, double(point.x()-2), double(point.y()-2));
        QVERIFY(moved.isValid() && moved.value());
        QTest::qWait(50);
        QDBusReply<bool> sent = compositor.call("Click", windowId, double(point.x()), double(point.y()));
        QVERIFY(sent.isValid() && sent.value());
    };
    click();
    QTest::qWait(100);
    QCOMPARE(page("window.allClicks||0").toInt(), clicks);
    QVERIFY(evaluate(ui + ".set_runtime_setting('ui.statusbar','never',true)").toBool());
    QTRY_VERIFY(!bar->isVisible());
    click();
    QTRY_COMPARE(page("window.allClicks||0").toInt(), clicks + 1);
    verifyStable();
}

void ApplicationTests::hintNavigationLifecycle_data() {
    QTest::addColumn<bool>("secondary");
    QTest::addColumn<QString>("method");
    for (const auto secondary : {false, true}) {
        for (const auto method : {"location", "busy-location", "form", "same-document", "replacement", "background"}) {
            const auto name = QString(secondary ? "secondary-" : "primary-") + method;
            QTest::newRow(qPrintable(name)) << secondary << QString(method);
        }
    }
}

void ApplicationTests::hintNavigationLifecycle() {
    QFETCH(bool, secondary);
    QFETCH(QString, method);
    QTest::failOnWarning(QRegularExpression(".*(invalid context|TypeError|ReferenceError|Binding loop).*"));
    const auto first = text("current_url");
    const auto destination = first + "?hint-destination=1";
    QString owner = "testWindow";
    QString ui = "testWindow.browserUi";
    auto ownerWindow = window;
    if (secondary) {
        evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"
            + quoted(first) + ",windowProfileName:'hint-navigation',windowProfileLabel:'Hint navigation'})");
        QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
        owner = "testWindow.browserWindowRegistry[1].host";
        ui = "testWindow.browserWindowRegistry[1].ui";
        QTRY_VERIFY(evaluate(owner + ".profileBootstrapComplete && !" + ui + ".profile_bootstrap_pending").toBool());
        ownerWindow = qobject_cast<QQuickWindow *>(evaluate(owner).value<QObject *>());
        QVERIFY(ownerWindow);
        pageReceiver = owner + ".activeView";
        QTRY_VERIFY(evaluate(pageReceiver + ".ownsTab() && !" + pageReceiver
            + ".loading && " + pageReceiver + ".mainFrame.isValid && String("
            + pageReceiver + ".url)===" + quoted(first)).toBool());
        QTRY_VERIFY(page("location.href===" + quoted(first) + " && document.readyState==='complete'").toBool());
        QTRY_COMPARE(evaluate(ui + ".load_state").toString(),"complete");
    }
    if (method == "background") {
        QVERIFY(evaluate(ui + ".execute_command(" + quoted("tab-open --background " + destination) + ")").toBool());
        QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), 2);
        QTRY_VERIFY(evaluate(owner + ".tabViewAt(1) && !" + owner + ".tabViewAt(1).loading").toBool());
    }
    if (method == "form") {
        page("document.body.innerHTML='<form method=get><input type=hidden name=hint-destination value=1><button>Continue</button></form>';document.querySelector('form').action=" + quoted(first) + ";true");
    } else {
        auto handler = method == "same-document"
            ? "history.pushState({hint:true},''," + quoted(destination) + ")"
            : "location.href=" + quoted(destination);
        if (method == "busy-location")
            handler += ";const until=performance.now()+200;while(performance.now()<until){}";
        page("document.body.innerHTML='<button id=next>Continue</button>';document.getElementById('next').addEventListener('click',()=>{" + handler + "});true");
    }
    QVERIFY(evaluate(ui + ".execute_command('set --temp hints.auto_follow=never')").toBool());
    ownerWindow->requestActivate();
    QTRY_VERIFY(ownerWindow->isActive());
    evaluate(pageReceiver + ".forceActiveFocus()");
    QTest::keyClick(ownerWindow, Qt::Key_F);
    const auto label = hintLabel("Continue", ui);
    QVERIFY(!label.isEmpty());
    if (method == "background") {
        const auto backgroundDestination = first + "?background-finished=1";
        evaluate(owner + ".tabViewAt(1).url=" + quoted(backgroundDestination));
        const auto activeReceiver = pageReceiver;
        pageReceiver = owner + ".tabViewAt(1)";
        QTRY_VERIFY(page("location.href===" + quoted(backgroundDestination) + " && document.readyState==='complete'").toBool());
        pageReceiver = activeReceiver;
        QCOMPARE(evaluate(ui + ".mode").toString(), "hint");
        QVERIFY(evaluate(ui + ".hint_visible").toBool());
        QCOMPARE(hintLabel("Continue", ui), label);
    } else {
        if (method != "replacement") {
            for (const auto character : label)
                QTest::keyClick(ownerWindow, Qt::Key(Qt::Key_A + character.toLower().unicode() - 'a'));
        }
        if (method == "replacement") {
            // Document replacement can retire an activation callback before
            // it returns; navigation owns cancellation independently of it.
            evaluate(pageReceiver + ".url=" + quoted(destination));
        } else QTest::keyClick(ownerWindow, Qt::Key_Return);
        QTRY_VERIFY(page("location.href===" + quoted(destination) + " && document.readyState==='complete'").toBool());
        QTRY_COMPARE(evaluate(ui + ".mode").toString(), "normal");
        QVERIFY(!evaluate(ui + ".hint_visible").toBool());
        QVERIFY(!evaluate(owner + ".hintWorkspace.hintRefreshPending").toBool());
        QTest::keyClick(ownerWindow, Qt::Key_F);
        QVERIFY(!hintLabel(method == "same-document" ? "Continue" : "New document", ui).isEmpty());
    }
    QTest::keyClick(ownerWindow, Qt::Key_Escape);
    QTRY_COMPARE(evaluate(ui + ".mode").toString(), "normal");
    if (secondary) {
        QCOMPARE(text("mode"), "normal");
        QCOMPARE(text("current_url"), first);
    }
}
