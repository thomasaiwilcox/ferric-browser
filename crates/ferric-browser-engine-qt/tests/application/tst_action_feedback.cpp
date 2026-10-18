#include "tst_application.h"
#include <QClipboard>
#include <QGuiApplication>
#include <QRegularExpression>
#include <QFile>
#include <QFileInfo>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonArray>
#include <QPointer>
#include <QTcpSocket>
#include <QtTest>

void ApplicationTests::prepareActionSelection() {
    evaluate(pageReceiver + ".forceActiveFocus()");
    page("(()=>{const e=document.createElement('p');e.textContent='Action selection';e.tabIndex=0;"
         "document.body.prepend(e);e.focus();const r=document.createRange();r.selectNodeContents(e);"
         "getSelection().removeAllRanges();getSelection().addRange(r);return true})()");
    // The DOM setup replaces the focus/selection metadata from fixture startup.
    // Await its fresh capture rather than accepting an older non-null lease.
    scriptDone = false;
    evaluate("(()=>{const source=testWindow.selectionProviderFor(" + pageReceiver + ");"
             "source.invalidate();source.capture(function(value){testProbe.accept(value!==null)})})()");
    QTRY_VERIFY(scriptDone);
    QVERIFY(scriptResult.toBool());
}

void ApplicationTests::productPolishSecondarySelectionCommand() {
    const auto original = text("current_url");
    evaluate("testWindow.browserWindowFactory.createObject(null,{windowStartupUrl:"
        + quoted(original) + ",windowProfileName:'selection-owner',windowProfileLabel:'Selection owner'})");
    QTRY_COMPARE(evaluate("testWindow.browserWindowRegistry.length").toInt(), 2);
    const auto owner = QString("testWindow.browserWindowRegistry[1]");
    pageReceiver = owner + ".view";
    QTRY_VERIFY_WITH_TIMEOUT(evaluate(owner + ".host.profileBootstrapComplete && !" + owner +
        ".ui.profile_bootstrap_pending && !!" + pageReceiver + " && " + pageReceiver +
        ".ownsTab() && !" + pageReceiver + ".loading && " + pageReceiver +
        ".url.toString()===" + quoted(original) + " && " + pageReceiver +
        ".mainFrame && " + pageReceiver + ".mainFrame.isValid").toBool(), 15000);
    QTRY_VERIFY_WITH_TIMEOUT(page("document.readyState === 'complete' && "
        "document.getElementById('button') !== null").toBool(), 15000);
    evaluate(owner + ".host.requestActivate()");
    prepareActionSelection(); if (QTest::currentTestFailed()) return;
    QGuiApplication::clipboard()->setText("before secondary action");
    evaluate(owner + ".ui.enter_command()");
    evaluate(owner + ".host.submitInteractiveCommand('yank selection')");
    QTRY_COMPARE(evaluate(owner + ".ui.mode").toString(), QString("normal"));
    QTRY_COMPARE(QGuiApplication::clipboard()->text(), QString("Action selection"));
    QTRY_COMPARE(evaluate(owner + ".ui.status_text").toString(), QString("Selection copied"));
    QCOMPARE(text("current_url"), original);
}

void ApplicationTests::productPolishActionFeedback_data() {
    QTest::addColumn<bool>("selection");
    QTest::addColumn<bool>("fromCommand");
    QTest::newRow("url-ui") << false << false;
    QTest::newRow("url-command") << false << true;
    QTest::newRow("selection-ui") << true << false;
    QTest::newRow("selection-command") << true << true;
}

void ApplicationTests::productPolishActionFeedback() {
    QFETCH(bool, selection);
    QFETCH(bool, fromCommand);
    QTest::failOnWarning(QRegularExpression("TypeError|ReferenceError|Binding loop"));
    const auto expected = selection ? QString("Action selection") : text("current_url");
    if (selection) {
        prepareActionSelection(); if (QTest::currentTestFailed()) return;
    }
    QGuiApplication::clipboard()->setText("before action");
    evaluate("testWindow.browserUi.status_text='Previous notice'");
    if (fromCommand) {
        interactiveCommand(selection ? "action selection copy" : "action url copy");
        QTRY_COMPARE(text("mode"), QString("normal"));
    } else {
        QVERIFY(evaluate("testWindow.browserUi.execute_ui_action("
            + quoted(selection ? "browser.selection.copy" : "browser.url.copy") + ",'')").toBool());
    }
    QTRY_COMPARE(QGuiApplication::clipboard()->text(), expected);
    QTRY_COMPARE(text("status_text"), selection ? QString("Selection copied") : QString("URL copied"));
}

void ApplicationTests::actionAvailabilityFeedback() {
    // This fixture uses a temporary profile: session actions must give usable
    // guidance rather than the discovery protocol's capability reason.
    QVERIFY(!evaluate("testWindow.browserUi.execute_ui_action('browser.session.list','')").toBool());
    QCOMPARE(text("status_text"), QString("Use a regular profile with available storage for this action."));
}

void ApplicationTests::productPolishRegisteredActionFeedback_data() {
    productPolishActionFeedback_data();
}

void ApplicationTests::productPolishRegisteredActionFeedback() {
    QFETCH(bool, selection);
    QFETCH(bool, fromCommand);
    const auto contextPath = directory.filePath("action-context.json");
    QFile::remove(contextPath);
    const QString subject = selection ? "selection" : "url";
    const QString label = selection ? "Inspect selection" : "Inspect URL";
    const auto id = "userscript.feedback_fixture." + subject;
    const QJsonArray argv{"-c", "cat > \"$1\"; printf '%s' '{\"type\":\"message\",\"text\":\"Fixture done\"}'",
        "fixture", contextPath};
    QFile manifest(directory.filePath("feedback_fixture.toml"));
    QVERIFY(manifest.open(QIODevice::WriteOnly));
    manifest.write("schema_version=1\nname=\"feedback_fixture\"\nexecutable=\"/bin/sh\"\nargv="
        + QJsonDocument(argv).toJson(QJsonDocument::Compact)
        + "\nallowed_results=[\"message\"]\ncontext_fields=[" + quoted(subject).toUtf8()
        + "]\n[[actions]]\nid=" + quoted(id).toUtf8() + "\nsubject=" + quoted(subject).toUtf8()
        + "\nverb=\"inspect-feedback\"\nlabel=" + quoted(label).toUtf8()
        + "\nrequired_fields=[" + quoted(subject).toUtf8() + "]\n");
    manifest.close();
    QTRY_VERIFY(!evaluate("testWindow.browserUi.profile_bootstrap_pending || testWindow.browserUi.userscript_inventory_pending").toBool());
    QVERIFY(evaluate("testWindow.browserUi.install_userscript_manifest(" + quoted(manifest.fileName()) + ")").toBool());
    QTRY_COMPARE(text("userscript_install_state"), QString("installed"));
    QVERIFY(evaluate("testWindow.browserUi.select_userscript_action_subject(" + quoted(subject) + ")").toBool());
    QVERIFY(evaluate("testWindow.browserUi.userscript_action_ids.indexOf(" + quoted(id) + ")>=0").toBool());
    QVERIFY(evaluate("testWindow.browserUi.userscript_action_labels.indexOf(" + quoted(label) + ")>=0").toBool());
    if (selection) {
        prepareActionSelection(); if (QTest::currentTestFailed()) return;
    }
    if (fromCommand) {
        interactiveCommand("action " + subject + " inspect-feedback");
        QTRY_COMPARE(text("mode"), QString("normal"));
    } else {
        QVERIFY(evaluate("testWindow.browserUi.execute_ui_action(" + quoted(id) + ","
            + quoted(selection ? "Action selection" : "") + ")").toBool());
    }
    QTRY_COMPARE(text("status_text"), "Started: " + label);
    QJsonObject input;
    QTRY_VERIFY(([&] {
        QFile context(contextPath);
        if (!context.open(QIODevice::ReadOnly)) return false;
        input = QJsonDocument::fromJson(context.readAll()).object();
        return input.contains("context") && input.contains("invocation_id");
    })());
    QVERIFY(input.value("invocation_id").toString().startsWith("op-"));
    QCOMPARE(input.value("context").toObject().value(subject).toString(), selection ? QString("Action selection") : text("current_url"));
}

void ApplicationTests::productPolishHintActionFeedback() {
    const auto contextPath = directory.filePath("hint-action-context.json");
    const auto releasePath = directory.filePath("hint-action-release");
    QFile::remove(contextPath); QFile::remove(releasePath);
    const QString id = "userscript.hint_feedback.inspect";
    const QString label = "Inspect hinted link";
    // Hold the actual producer until the initial notice has been observed.
    // A fast result message must not race the feedback assertion.
    const QJsonArray argv{"-c", "cat > \"$1\"; while [ ! -f \"$2\" ]; do sleep 0.05; done; "
        "printf '%s' '{\"type\":\"message\",\"text\":\"Fixture done\"}'",
        "fixture", contextPath, releasePath};
    QFile manifest(directory.filePath("hint_feedback.toml"));
    QVERIFY(manifest.open(QIODevice::WriteOnly));
    manifest.write("schema_version=1\nname=\"hint_feedback\"\nexecutable=\"/bin/sh\"\nargv="
        + QJsonDocument(argv).toJson(QJsonDocument::Compact)
        + "\ntimeout_seconds=10\nallowed_results=[\"message\"]\ncontext_fields=[\"hint_url\"]\n"
        "[[actions]]\nid=" + quoted(id).toUtf8()
        + "\nsubject=\"link\"\nverb=\"inspect-hint\"\nlabel=" + quoted(label).toUtf8()
        + "\nrequired_fields=[\"hint_url\"]\n");
    manifest.close();
    QTRY_VERIFY(!evaluate("testWindow.browserUi.profile_bootstrap_pending || testWindow.browserUi.userscript_inventory_pending").toBool());
    QVERIFY(evaluate("testWindow.browserUi.install_userscript_manifest(" + quoted(manifest.fileName()) + ")").toBool());
    QTRY_COMPARE(text("userscript_install_state"), QString("installed"));
    const auto expected = page("document.getElementById('same').href.split('#')[0]").toString();
    QVERIFY(command("set --temp hints.auto_follow=never"));
    QVERIFY(command("hint --target choose links"));
    const auto hint = hintLabel("Same document");
    QVERIFY(!hint.isEmpty());
    typeText(hint); key(Qt::Key_Return);
    int index = -1;
    QTRY_VERIFY((index = evaluate("testWindow.contextMenuItems.findIndex(function(item){return item.actionId === "
        + quoted(id) + " && item.enabled})").toInt()) >= 0);
    auto menu = window->findChild<QObject *>("contextMenuPopup");
    QVERIFY(menu);
    QTRY_VERIFY(menu->property("opened").toBool());
    QTRY_COMPARE(menu->property("currentIndex").toInt(), 0);
    const auto count = evaluate("testWindow.contextMenuItems.length").toInt();
    // Arrow navigation skips disabled entries; observe the actual menu index.
    for (int i = 0; i < count && menu->property("currentIndex").toInt() != index; ++i)
        key(Qt::Key_Down);
    QCOMPARE(menu->property("currentIndex").toInt(), index);
    key(Qt::Key_Return);
    QTRY_COMPARE(text("status_text"), "Started: " + label);
    QJsonObject input;
    QTRY_VERIFY(([&] {
        QFile context(contextPath);
        if (!context.open(QIODevice::ReadOnly)) return false;
        input = QJsonDocument::fromJson(context.readAll()).object();
        return input.contains("context") && input.contains("invocation_id");
    })());
    QCOMPARE(input.value("context").toObject().value("hint_url").toString(), expected);
    QVERIFY(input.value("invocation_id").toString().startsWith("op-"));
    QVERIFY(!text("status_text").contains(id));
    QFile release(releasePath); QVERIFY(release.open(QIODevice::WriteOnly)); release.close();
}

void ApplicationTests::productPolishRapidDownloadRejection() {
    QTcpServer heldServer;
    QVERIFY(heldServer.listen(QHostAddress::LocalHost, 0));
    QPointer<QTcpSocket> held;
    int requests = 0;
    connect(&heldServer, &QTcpServer::newConnection, &heldServer, [&] {
        while (auto socket = heldServer.nextPendingConnection()) {
            connect(socket, &QTcpSocket::readyRead, socket, [&, socket, bytes = QByteArray{}]() mutable {
                bytes += socket->readAll();
                if (!bytes.contains("\r\n\r\n")) return;
                if (bytes.startsWith("GET /held.bin ")) { held = socket; ++requests; }
                bytes.clear();
                // Deliberately send no headers: admission is still preparing.
            });
            connect(socket, &QTcpSocket::disconnected, socket, &QObject::deleteLater);
        }
    });
    const auto url = QString("http://127.0.0.1:%1/held.bin").arg(heldServer.serverPort());
    QVERIFY(page("(()=>{const a=document.createElement('a');a.href=" + quoted(url)
        + ";a.textContent='Retry this download';document.body.prepend(a);return true})()").toBool());
    QVERIFY(command("download " + url));
    QTRY_COMPARE(requests, 1);
    QVERIFY(held);
    QVERIFY(command("set --temp hints.auto_follow=never"));
    QVERIFY(command("hint --rapid --target download links"));
    const auto hint = hintLabel("Retry this download"); QVERIFY(!hint.isEmpty());
    typeText(hint); key(Qt::Key_Return);
    QTRY_COMPARE(text("status_text"), QString("Another download is being prepared. Wait for it or cancel the request in Downloads."));
    QCOMPARE(text("mode"), QString("hint"));
    QCOMPARE(hintLabel("Retry this download"), hint);
    QCOMPARE(requests, 1);
    evaluate("testWindow.browserUi.cancel_download_request(testWindow.browserUi.download_request_token)");
    const QString noHelpers = "testWindow.downloadWorkspaceObject.children.filter(function(item){return item.helpers!==undefined})[0].ownedHelpers.length===0";
    QTRY_VERIFY(evaluate(noHelpers).toBool());
    // The stream must abort the actual held connection before the server sends
    // any headers; no late native offer is part of the stream contract.
    QTRY_VERIFY(held.isNull() || held->state() == QAbstractSocket::UnconnectedState);
    QCOMPARE(evaluate("testWindow.browserUi.download_rows().length").toInt(), 0);
    // Preserve the selection so Enter retries it after the request is cancelled.
    key(Qt::Key_Return);
    QTRY_COMPARE(requests, 2);
    QCOMPARE(text("mode"), QString("hint"));
    evaluate("testWindow.browserUi.cancel_download_request(testWindow.browserUi.download_request_token)");
    QTRY_VERIFY(evaluate(noHelpers).toBool());
    QTRY_VERIFY(held.isNull() || held->state() == QAbstractSocket::UnconnectedState);
    key(Qt::Key_Escape);
    // Normal page downloads must also abort when their source is retired.
    // The separate download-source suite qualifies detached late offers.
    const auto original = text("current_url");
    QVERIFY(command("tab-open " + original));
    QTRY_VERIFY(evaluate("!!testWindow.activeWebView() && !testWindow.activeWebView().loading && testWindow.activeWebView().mainFrame.isValid && String(testWindow.activeWebView().url)===" + quoted(original)).toBool());
    QTRY_VERIFY(page("location.href===" + quoted(original) + " && document.readyState==='complete'").toBool());
    QPointer<QObject> nativeSource = evaluate("testWindow.activeWebView()").value<QObject *>();
    QVERIFY(nativeSource);
    QVERIFY(page("(()=>{const a=document.createElement('a');a.href=" + quoted(url)
        + ";a.download='native.bin';document.body.append(a);a.click();return true})()").toBool());
    QTRY_COMPARE(requests, 3);
    QVERIFY(command("tab-close"));
    QTRY_VERIFY(nativeSource.isNull());
    QTRY_VERIFY(held.isNull() || held->state() == QAbstractSocket::UnconnectedState);
    QCOMPARE(evaluate("testWindow.browserUi.download_rows().length").toInt(), 0);

}

void ApplicationTests::productPolishExternalActionFeedback_data() {
    QTest::addColumn<bool>("selection");
    QTest::addColumn<bool>("fromCommand");
    QTest::addColumn<bool>("detached");
    for (bool selection : {false,true})
        for (bool fromCommand : {false,true})
            for (bool detached : {false,true}) {
                const auto name = QByteArray(selection ? "selection" : "url")
                    + (detached ? "-detached" : "-tracked") + (fromCommand ? "-command" : "-ui");
                QTest::newRow(name.constData()) << selection << fromCommand << detached;
            }
}

void ApplicationTests::productPolishExternalActionFeedback() {
    QFETCH(bool, selection);
    QFETCH(bool, fromCommand);
    QFETCH(bool, detached);
    Q_UNUSED(detached);
    const auto pidPath = directory.filePath("action-child.pid");
    const auto subjectPath = directory.filePath("action-subject.txt");
    QFile::remove(pidPath); QFile::remove(subjectPath);
    if (selection) {
        prepareActionSelection(); if (QTest::currentTestFailed()) return;
    }
    if (fromCommand) {
        interactiveCommand(selection ? "send fixture --selection" : "send fixture --url");
        QTRY_COMPARE(text("mode"), QString("normal"));
    } else {
        QVERIFY(evaluate("testWindow.browserUi.execute_ui_action("
            + quoted(selection ? "external.fixture.selection.send" : "external.fixture.url.send") + ",'')").toBool());
    }
    QTRY_COMPARE(text("status_text"), QString("Started: fixture"));
    QTRY_VERIFY(QFileInfo(subjectPath).size() > 0);
    QFile subject(subjectPath); QVERIFY(subject.open(QIODevice::ReadOnly));
    QCOMPARE(QString::fromUtf8(subject.readAll()), selection ? QString("Action selection") : text("current_url"));
    QFile pidFile(pidPath); QVERIFY(pidFile.open(QIODevice::ReadOnly));
    const auto pid = pidFile.readAll().toLongLong(); QVERIFY(pid > 1);
    // A completed detached launch must not leave a zombie in the long-running
    // browser. Only observe the fixture PID; never signal a desktop process.
    QTRY_VERIFY(!QFileInfo::exists(QString("/proc/%1").arg(pid)));
}

void ApplicationTests::productPolishConfigurationFeedback_data() {
    QTest::addColumn<QString>("request");
    QTest::addColumn<QString>("expected");
    const auto missing =
        QString("Unknown setting: no.such.setting. Use :settings to find available settings.");
    QTest::newRow("missing-setting") << QStringLiteral("get no.such.setting") << missing;
    QTest::newRow("stop-chain")
        << QStringLiteral("get no.such.setting ;; open https://should-not-open.invalid/")
        << missing;
    QTest::newRow("invalid-site-url")
        << QStringLiteral("get downloads.ask_destination --url file:///tmp/not-a-site")
        << QStringLiteral("Use an HTTP or HTTPS page address with --url.");
}

void ApplicationTests::productPolishConfigurationFeedback() {
    QFETCH(QString, request);
    QFETCH(QString, expected);
    QTest::failOnWarning(QRegularExpression("TypeError|ReferenceError|Binding loop"));
    const auto original = text("current_url");
    const auto folder = evaluate("testWindow.browserUi.default_download_directory()").toString();
    QVERIFY(!folder.isEmpty());
    interactiveCommand(request);
    QTRY_COMPARE(text("status_text"), expected);
    QCOMPARE(text("current_url"), original);
    QCOMPARE(evaluate("testWindow.browserUi.default_download_directory()").toString(), folder);
    QVERIFY(!text("status_text").contains("E_ENGINE"));
    QVERIFY(!text("status_text").contains("diagnostic_context"));
}
