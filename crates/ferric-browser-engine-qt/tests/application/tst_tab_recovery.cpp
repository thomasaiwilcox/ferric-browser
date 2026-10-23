#include "tst_application.h"
#include "owned_browser_keyboard.h"
#include <QDir>
#include <QDirIterator>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QQuickItem>
#include <QPointer>
#include <QSet>
#include <QScopeGuard>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QtTest>
#include <dlfcn.h>

void ApplicationTests::restartRecoveryFixture(const QString &url, const QString &checkpointPath,
                                            const QByteArray &checkpointBytes) {
    QVariantMap properties;
    for (const auto name : {"temporaryProfile", "ephemeralProfile", "storageBasePath",
            "softwareRendering", "nativeWayland", "startupConfigJson", "startupConfigPath",
            "startupConfigBaseJson", "startupCliOverridesJson", "startupProfileOverridesJson",
            "userscriptsOff", "width", "height"})
        properties[name] = window->property(name);
    properties["startupUrl"] = url;
    properties["safeMode"] = false;
    properties["recoveryAvailable"] = true;
    engine.reset();
    window = nullptr;
    QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
    QTest::qWait(20);
    if (!checkpointPath.isEmpty()) {
        QFile checkpoint(checkpointPath);
        QVERIFY(checkpoint.open(QIODevice::WriteOnly | QIODevice::Truncate));
        QCOMPARE(checkpoint.write(checkpointBytes), checkpointBytes.size());
    }
    engine = std::make_unique<QQmlApplicationEngine>();
    engine->setInitialProperties(properties);
    engine->load(QUrl("qrc:/qt/qml/io/github/ferricbrowser/qml/Main.qml"));
    QCOMPARE(engine->rootObjects().size(), 1);
    window = qobject_cast<QQuickWindow *>(engine->rootObjects().first());
    QVERIFY(window);
    engine->rootContext()->setContextProperty("testWindow", window);
    engine->rootContext()->setContextProperty("testProbe", this);
    pageReceiver = "testWindow.activeWebView()";
    QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.profileBootstrapComplete").toBool(), 10000);
    QTRY_VERIFY(evaluate("!!testWindow.activeWebView()").toBool());
    QTRY_COMPARE_WITH_TIMEOUT(text("load_state"), QString("complete"), 15000);
    window->show();
    window->requestActivate();
    QTRY_VERIFY(window->isActive());
    auto banner = window->findChild<QQuickItem *>("recoveryBanner");
    QVERIFY(banner && banner->isVisible());
}

void ApplicationTests::productPolishSessionRecoveryFailure_data() {
    QTest::addColumn<QString>("decision");
    QTest::addColumn<bool>("empty");
    QTest::newRow("escape-after-failure") << QString("escape") << false;
    QTest::newRow("alt-d-after-failure") << QString("dismiss") << false;
    QTest::newRow("retry-after-repair") << QString("retry") << false;
    QTest::newRow("escape-without-saved-tabs") << QString("escape") << true;
    QTest::newRow("retry-after-saved-tabs-appear") << QString("retry") << true;
}

void ApplicationTests::productPolishSessionRecoveryFailure() {
    QFETCH(QString, decision);
    QFETCH(bool, empty);
    const auto saveReady = [&] { return evaluate("testWindow.browserUi.flush_durable_state()").toBool(); };
    QTRY_VERIFY_WITH_TIMEOUT(saveReady(), 5000);
    const auto root = directory.filePath("durable-test/productPolishSessionRecoveryFailure/"
        + QString::fromLatin1(QTest::currentDataTag()));
    QDirIterator paths(root, {"current*.json"}, QDir::Files, QDirIterator::Subdirectories);
    QVERIFY(paths.hasNext());
    const auto path = paths.next();
    QVERIFY(!paths.hasNext());
    QFile checkpoint(path);
    QVERIFY(checkpoint.open(QIODevice::ReadOnly));
    const auto validSnapshot = checkpoint.readAll();
    checkpoint.close();
    const auto startup = text("current_url");
    restartRecoveryFixture(startup);
    QVERIFY(window);
    const auto originalId = text("tab_id_for_index(0)");
    const auto originalView = evaluate("testWindow.activeWebView()").value<QObject *>();
    QVERIFY(originalView);
    page("window.recoveryDraft='keep my work';true");
    // The marker only decides whether to show this surface. Exercise the real
    // dialog, storage worker and key routing with an unreadable generation.
    QVERIFY(window->setProperty("recoveryAvailable", true));
    const auto previousPath = QFileInfo(path).path() + "/" + QFileInfo(path).completeBaseName() + ".previous";
    QVERIFY(QFile::remove(previousPath) || !QFile::exists(previousPath));
    if (empty) {
        auto snapshot = QJsonDocument::fromJson(validSnapshot).object();
        auto windows = snapshot["windows"].toArray();
        auto emptyWindow = windows[0].toObject();
        emptyWindow["tabs"] = QJsonArray{};
        emptyWindow["selected_tab"] = QJsonValue();
        snapshot["windows"] = QJsonArray{emptyWindow};
        snapshot["closed_tabs"] = QJsonArray{};
        QVERIFY(checkpoint.open(QIODevice::WriteOnly | QIODevice::Truncate));
        QVERIFY(checkpoint.write(QJsonDocument(snapshot).toJson(QJsonDocument::Compact)) > 0);
        checkpoint.close();
    } else {
        QVERIFY(checkpoint.open(QIODevice::WriteOnly | QIODevice::Truncate));
        QCOMPARE(checkpoint.write("not a saved session"), qint64(19));
        checkpoint.close();
    }
    auto banner = window->findChild<QQuickItem *>("recoveryBanner");
    QVERIFY(banner);
    QTRY_VERIFY(banner->isVisible());
    sendOwnedBrowserKey(window, Qt::Key_Y);
    QTRY_VERIFY_WITH_TIMEOUT(text("status_text").startsWith(empty
        ? "Recovery session is empty" : "Recovery rejected:"), 5000);
    QVERIFY(!evaluate("testWindow.browserUi.session_recovery_pending").toBool());
    QVERIFY(!text("session_recovery_error").isEmpty());
    QCOMPARE(banner->property("message").toString(), text("session_recovery_error"));
    QCOMPARE(evaluate("testWindow.activeWebView()").value<QObject *>(), originalView);
    QCOMPARE(page("window.recoveryDraft").toString(), "keep my work");
    QCOMPARE(text("tab_id_for_index(0)"), originalId);
    if (decision == "retry") {
        QVERIFY(checkpoint.open(QIODevice::WriteOnly | QIODevice::Truncate));
        QCOMPARE(checkpoint.write(validSnapshot), qint64(validSnapshot.size()));
        checkpoint.close();
        sendOwnedBrowserKey(window, Qt::Key_R, Qt::AltModifier);
        QTRY_VERIFY_WITH_TIMEOUT(!window->property("recoveryAvailable").toBool(), 5000);
        QTRY_VERIFY(evaluate("!!testWindow.activeWebView()").toBool());
        QTRY_COMPARE(text("load_state"), "complete");
    } else {
        sendOwnedBrowserKey(window, decision == "escape" ? Qt::Key_Escape : Qt::Key_D,
            decision == "escape" ? Qt::NoModifier : Qt::AltModifier);
        QTRY_VERIFY_WITH_TIMEOUT(!window->property("recoveryAvailable").toBool(), 5000);
        QCOMPARE(evaluate("testWindow.activeWebView()").value<QObject *>(), originalView);
        QCOMPARE(page("window.recoveryDraft").toString(), "keep my work");
        QCOMPARE(text("tab_id_for_index(0)"), originalId);
    }
    auto view = qobject_cast<QQuickItem *>(evaluate("testWindow.activeWebView()").value<QObject *>());
    QVERIFY(view);
    QTRY_VERIFY(window->activeFocusItem() && (window->activeFocusItem() == view
        || view->isAncestorOf(window->activeFocusItem())));
}

void ApplicationTests::productPolishSessionRecoveryCancellation_data() {
    QTest::addColumn<int>("dismissKey");
    QTest::addColumn<int>("dismissModifiers");
    QTest::newRow("escape-while-reading") << int(Qt::Key_Escape) << int(Qt::NoModifier);
    QTest::newRow("n-while-reading") << int(Qt::Key_N) << int(Qt::NoModifier);
    QTest::newRow("alt-d-while-reading") << int(Qt::Key_D) << int(Qt::AltModifier);
}

void ApplicationTests::productPolishSessionRecoveryCancellation() {
    QFETCH(int, dismissKey);
    QFETCH(int, dismissModifiers);
    const auto arm = reinterpret_cast<void (*)(const char *)>(dlsym(RTLD_DEFAULT, "ferric_session_read_arm"));
    const auto entered = reinterpret_cast<bool (*)()>(dlsym(RTLD_DEFAULT, "ferric_session_read_entered"));
    const auto release = reinterpret_cast<void (*)()>(dlsym(RTLD_DEFAULT, "ferric_session_read_release"));
    if (!arm || !entered || !release)
        QSKIP("Use application_session_recovery with its owned read barrier");
    const auto releaseOnExit = qScopeGuard([&] { release(); });
    const auto saveReady = [&] { return evaluate("testWindow.browserUi.flush_durable_state()").toBool(); };
    QTRY_VERIFY_WITH_TIMEOUT(saveReady(), 5000);
    const auto root = directory.filePath("durable-test/productPolishSessionRecoveryCancellation/"
        + QString::fromLatin1(QTest::currentDataTag()));
    QDirIterator paths(root, {"current*.json"}, QDir::Files, QDirIterator::Subdirectories);
    QVERIFY(paths.hasNext());
    const auto path = paths.next();
    QVERIFY(!paths.hasNext());
    QFile checkpoint(path);
    QVERIFY(checkpoint.open(QIODevice::ReadOnly));
    const auto snapshot = checkpoint.readAll();
    checkpoint.close();
    const auto startup = text("current_url");
    restartRecoveryFixture(startup);
    QVERIFY(window);
    const auto originalId = text("tab_id_for_index(0)");
    const auto originalView = evaluate("testWindow.activeWebView()").value<QObject *>();
    QVERIFY(originalView);
    page("window.recoveryDraft='keep while reading';true");
    // A subsequent named preview proves the cancelled operation has left the
    // single-flight restore lane, without sleeps or production test hooks.
    QFile named(QFileInfo(path).path() + "/recovery-proof.json");
    QVERIFY(named.open(QIODevice::WriteOnly));
    QCOMPARE(named.write(snapshot), qint64(snapshot.size()));
    named.close();
    const auto previewBefore = text("session_preview");
    QVERIFY(window->setProperty("recoveryAvailable", true));
    arm(QFile::encodeName(path).constData());
    sendOwnedBrowserKey(window, Qt::Key_Y);
    QTRY_VERIFY_WITH_TIMEOUT(entered(), 5000);
    QVERIFY(evaluate("testWindow.browserUi.session_recovery_pending").toBool());
    // Repeated requests must not create a second recovery after cancellation.
    sendOwnedBrowserKey(window, Qt::Key_Y);
    sendOwnedBrowserKey(window, Qt::Key_R, Qt::AltModifier);
    sendOwnedBrowserKey(window, dismissKey, Qt::KeyboardModifiers(dismissModifiers));
    QTRY_VERIFY_WITH_TIMEOUT(!window->property("recoveryAvailable").toBool(), 3000);
    QVERIFY(!evaluate("testWindow.browserUi.session_recovery_pending").toBool());
    QVERIFY(!text("status_text").startsWith("Loading recovery"));
    QCOMPARE(evaluate("testWindow.activeWebView()").value<QObject *>(), originalView);
    QCOMPARE(page("window.recoveryDraft").toString(), "keep while reading");
    // Drain unrelated receipts while the restore is held. Then poll without
    // pumping GUI timers, so this fixture observes the discarded completion
    // before the normal 50 ms library timer can consume it.
    while (evaluate("testWindow.browserUi.poll_storage_library()").toBool()) {}
    release();
    QElapsedTimer completionDeadline;
    completionDeadline.start();
    bool completed = false;
    while (!completed && completionDeadline.elapsed() < 5000) {
        completed = evaluate("testWindow.browserUi.poll_storage_library()").toBool();
        if (!completed) QThread::msleep(2);
    }
    QVERIFY(completed);
    QCOMPARE(text("session_preview"), previewBefore);
    QVERIFY(text("session_restore_values").isEmpty());
    QTRY_VERIFY_WITH_TIMEOUT(evaluate("testWindow.browserUi.request_session_preview('recovery-proof')").toBool(), 5000);
    QTRY_COMPARE_WITH_TIMEOUT(text("status_text"), QString("Session preview ready"), 5000);
    QCOMPARE(text("tab_count").toInt(), 1);
    QCOMPARE(text("tab_id_for_index(0)"), originalId);
    QCOMPARE(evaluate("testWindow.activeWebView()").value<QObject *>(), originalView);
    QCOMPARE(page("window.recoveryDraft").toString(), "keep while reading");
    QVERIFY(text("session_preview") != previewBefore);
    auto view = qobject_cast<QQuickItem *>(originalView);
    QVERIFY(view);
    QTRY_VERIFY(window->activeFocusItem() && (window->activeFocusItem() == view
        || view->isAncestorOf(window->activeFocusItem())));
}

void ApplicationTests::productPolishStartupRecovery_data() {
    QTest::addColumn<bool>("nativeStart");
    QTest::addColumn<bool>("extraTab");
    QTest::addColumn<bool>("duringRead");
    QTest::newRow("untouched-start-page") << true << false << false;
    QTest::newRow("incoming-link") << false << false << false;
    QTest::newRow("incoming-tabs-and-closed-work") << false << true << false;
    QTest::newRow("new-tab-during-read") << true << false << true;
}

void ApplicationTests::productPolishStartupRecovery() {
    QFETCH(bool, nativeStart); QFETCH(bool, extraTab); QFETCH(bool, duringRead);
    const auto arm = reinterpret_cast<void (*)(const char *)>(dlsym(RTLD_DEFAULT, "ferric_session_read_arm"));
    const auto entered = reinterpret_cast<bool (*)()>(dlsym(RTLD_DEFAULT, "ferric_session_read_entered"));
    const auto release = reinterpret_cast<void (*)()>(dlsym(RTLD_DEFAULT, "ferric_session_read_release"));
    if (duringRead && (!arm || !entered || !release))
        QSKIP("Use application_startup_recovery with its owned read barrier");
    const auto releaseOnExit = qScopeGuard([&] { if (release) release(); });
    const auto origin = QString("http://127.0.0.1:%1").arg(server.serverPort());
    for (const auto name : {"saved-a", "saved-b", "saved-closed", "current-a", "current-b", "current-closed"})
        frameFixtures[QString("/%1.html").arg(name).toUtf8()] = QString("<!doctype html><title>%1</title><input id='draft'><p>%1</p>").arg(name).toUtf8();
    const auto oldA = origin + "/saved-a.html?keep=yes";
    const auto oldB = origin + "/saved-b.html";
    const auto oldClosed = origin + "/saved-closed.html";
    const auto newA = origin + "/current-a.html";
    const auto newB = origin + "/current-b.html";
    const auto newClosed = origin + "/current-closed.html";
    const auto navigate = [&](const QString &url, bool newTab) {
        QVERIFY(command((newTab ? "tab-open " : "open ") + url));
        evaluate("testWindow.executePendingEngineAction()");
        QTRY_VERIFY(evaluate("!!testWindow.activeWebView()").toBool());
        QTRY_COMPARE(page("location.href").toString(), url);
        QTRY_COMPARE(text("load_state"), QString("complete"));
    };
    const auto saveReady = [&] { return evaluate("testWindow.browserUi.flush_durable_state()").toBool(); };
    navigate(oldA, false);
    QCOMPARE(evaluate("testWindow.browserUi.set_tab_pinned(0,true)").toInt(), 0);
    QVERIFY(evaluate("testWindow.browserUi.set_tab_muted(0,true)").toBool());
    QVERIFY(command("zoom 1.25")); evaluate("testWindow.executePendingEngineAction()");
    navigate(oldClosed, true);
    QVERIFY(command("tab-close")); evaluate("testWindow.executePendingEngineAction()");
    QTRY_COMPARE(text("tab_count").toInt(), 1);
    navigate(oldB, true);
    QTRY_VERIFY_WITH_TIMEOUT(saveReady(), 5000);
    const auto root = directory.filePath("durable-test/productPolishStartupRecovery/"
        + QString::fromLatin1(QTest::currentDataTag()));
    QDirIterator paths(root, {"current*.json"}, QDir::Files, QDirIterator::Subdirectories);
    QVERIFY(paths.hasNext()); const auto oldPath = paths.next(); QVERIFY(!paths.hasNext());
    restartRecoveryFixture(nativeStart ? "rb://start" : newA);
    QVERIFY(window);
    const auto initialId = text("tab_id_for_index(0)");
    struct CurrentPage { QString id, url, draft; QPointer<QObject> view; };
    QList<CurrentPage> current;
    const auto remember = [&](const QString &url) {
        const auto id = text("tab_id_for_index(testWindow.browserUi.active_tab_index)");
        const auto draft = "keep " + url;
        page("document.getElementById('draft').value=" + quoted(draft) + ";true");
        current.append({id, url, draft, evaluate("testWindow.activeWebView()").value<QObject *>()});
    };
    if (!nativeStart) remember(newA);
    if (extraTab) {
        navigate(newClosed, true);
        QVERIFY(command("tab-close")); evaluate("testWindow.executePendingEngineAction()");
        QTRY_COMPARE(text("tab_count").toInt(), 1);
        navigate(newB, true); remember(newB);
    }
    // An explicit checkpoint while the decision is deferred must never become
    // a recovery source for this run.
    QTRY_VERIFY_WITH_TIMEOUT(saveReady(), 5000);
    if (duringRead) arm(QFile::encodeName(oldPath).constData());
    sendOwnedBrowserKey(window, Qt::Key_Y);
    if (duringRead) {
        QTRY_VERIFY_WITH_TIMEOUT(entered(), 5000);
        navigate(newA, true); remember(newA);
        release();
    }
    QTRY_VERIFY_WITH_TIMEOUT(!window->property("recoveryAvailable").toBool(), 10000);
    QTRY_COMPARE(text("tab_count").toInt(), nativeStart && !duringRead ? 2 : current.size() + 2 + (duringRead ? 1 : 0));
    QTRY_VERIFY(evaluate("!!testWindow.activeWebView()").toBool());
    QTRY_COMPARE(page("location.href").toString(), oldB);
    QTRY_COMPARE(text("load_state"), QString("complete"));
    for (const auto &kept : current) {
        const auto index = evaluate("testWindow.browserUi.tab_index_for_id(" + quoted(kept.id) + ")").toInt();
        QVERIFY(index >= 0 && kept.view);
        pageReceiver = "testWindow.tabViewAt(" + QString::number(index) + ")";
        QCOMPARE(evaluate(pageReceiver).value<QObject *>(), kept.view.data());
        QCOMPARE(page("location.href").toString(), kept.url);
        QCOMPARE(page("document.getElementById('draft').value").toString(), kept.draft);
    }
    pageReceiver = "testWindow.activeWebView()";
    const auto savedIndex = evaluate("(function(){for(var i=0;i<testWindow.tabsModel.count;i++)if(testWindow.tabsModel.get(i).url===" + quoted(oldA) + ")return i;return -1})()").toInt();
    QVERIFY(savedIndex >= 0);
    QVERIFY(!evaluate("testWindow.tabsModel.get(" + QString::number(savedIndex) + ").loaded").toBool());
    const auto savedId = text("tab_id_for_index(" + QString::number(savedIndex) + ")");
    if (nativeStart && !duringRead)
        QVERIFY(evaluate("testWindow.browserUi.tab_index_for_id(" + quoted(initialId) + ")").toInt() >= 0);
    QVERIFY(evaluate("testWindow.browserUi.save_named_session('lazy-after-recovery')").toBool());
    QTRY_COMPARE(text("session_save_state"), QString("succeeded"));
    QDirIterator saves(root, {"lazy-after-recovery.json"}, QDir::Files, QDirIterator::Subdirectories);
    QVERIFY(saves.hasNext()); QFile saved(saves.next()); QVERIFY(saved.open(QIODevice::ReadOnly));
    const auto tabs = QJsonDocument::fromJson(saved.readAll()).object()["windows"].toArray()[0].toObject()["tabs"].toArray();
    QSet<QString> addresses; bool metadata = false;
    for (const auto value : tabs) {
        const auto tab = value.toObject(); addresses.insert(tab["safe_restore_url"].toString());
        if (tab["safe_restore_url"].toString() == oldA) {
            metadata = true; QVERIFY(tab["pinned"].toBool()); QVERIFY(tab["muted"].toBool());
            QCOMPARE(tab["zoom"].toDouble(), 1.25);
        }
    }
    QVERIFY(metadata && addresses.contains(oldA) && addresses.contains(oldB));
    for (const auto &kept : current) QVERIFY(addresses.contains(kept.url));
    // Select the previously unvisited tab through its real configured key.
    sendOwnedBrowserKey(window, Qt::Key_1 + savedIndex, Qt::AltModifier);
    QTRY_VERIFY(evaluate("!!testWindow.activeWebView()").toBool());
    QTRY_COMPARE(page("location.href").toString(), oldA);
    QCOMPARE(text("tab_id_for_index(testWindow.browserUi.active_tab_index)"), savedId);
    QTRY_COMPARE(text("load_state"), QString("complete"));
    if (extraTab) {
        sendOwnedBrowserKey(window, Qt::Key_U);
        QTRY_VERIFY(evaluate("!!testWindow.activeWebView()").toBool());
        QTRY_COMPARE(page("location.href").toString(), newClosed);
    }
    sendOwnedBrowserKey(window, Qt::Key_U);
    QTRY_VERIFY(evaluate("!!testWindow.activeWebView()").toBool());
    QTRY_COMPARE(page("location.href").toString(), oldClosed);
}

void ApplicationTests::productPolishTabRecovery_data() {
    QTest::addColumn<QString>("hostKind");
    QTest::addColumn<QString>("attribute");
    QTest::addColumn<bool>("modifiedShortcut");
    for (const auto &host : {QString("primary"), QString("secondary"), QString("private"),
                             QString("temporary"), QString("popup")}) {
        for (const auto &attribute : {QString("position"), QString("pinned"),
                                      QString("muted"), QString("zoom")}) {
            for (const bool modified : {false, true}) {
                const auto tag = host + "-" + attribute + (modified ? "-ctrl-shift-t" : "-u");
                QTest::newRow(qPrintable(tag)) << host << attribute << modified;
            }
        }
    }
}

void ApplicationTests::productPolishTabRecovery() {
    QFETCH(QString, hostKind);
    QFETCH(QString, attribute);
    QFETCH(bool, modifiedShortcut);
    QString owner, ui;
    QQuickWindow *host = nullptr;
    preparePageSearchHost(hostKind == "temporary" ? "primary" : hostKind, owner, ui, host);
    QVERIFY(host);
    if (hostKind == "temporary")
        QVERIFY(evaluate(owner + ".ephemeralProfile").toBool());
    const auto drain = [&] {
        evaluate(owner + ((hostKind == "primary" || hostKind == "temporary")
            ? ".executePendingEngineAction()" : ".applySwitcherEngineAction()"));
    };
    frameFixtures["/recover-b.html"] = "<!doctype html><title>Recover middle tab</title><p>Middle tab</p>";
    frameFixtures["/recover-c.html"] = "<!doctype html><title>Keep right tab</title><p>Right tab</p>";
    const auto base = QUrl(page("location.href").toString());
    const auto middleUrl = base.resolved(QUrl("/recover-b.html")).toString();
    const auto rightUrl = base.resolved(QUrl("/recover-c.html")).toString();
    const auto leftId = evaluate(ui + ".tab_id_for_index(0)").toString();
    QVERIFY(evaluate(ui + ".set_tab_pinned(0,true)").toInt() >= 0);
    QVERIFY(evaluate(ui + ".execute_command(" + quoted("tab-open " + middleUrl) + ")").toBool());
    drain();
    QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), 2);
    QTRY_VERIFY(evaluate("!!" + pageReceiver).toBool());
    QTRY_COMPARE(page("location.href").toString(), middleUrl);
    QTRY_COMPARE(evaluate(ui + ".load_state").toString(), "complete");
    const auto closedId = evaluate(ui + ".tab_id_for_index(1)").toString();
    QVERIFY(!closedId.isEmpty());
    const bool pinned = attribute == "pinned";
    const bool muted = attribute == "muted";
    const int zoomHundredths = attribute == "zoom" ? 125 : 100;
    if (pinned)
        QCOMPARE(evaluate(ui + ".set_tab_pinned(1,true)").toInt(), 1);
    if (muted)
        QVERIFY(evaluate(ui + ".set_tab_muted(1,true)").toBool());
    if (zoomHundredths != 100)
        QVERIFY(evaluate(ui + ".execute_command('zoom 1.25')").toBool());
    drain();
    QTRY_COMPARE(evaluate(pageReceiver + ".audioMuted").toBool(), muted);
    QTRY_COMPARE(qRound(evaluate(pageReceiver + ".zoomFactor").toDouble() * 100), zoomHundredths);
    QVERIFY(evaluate(ui + ".execute_command(" + quoted("tab-open " + rightUrl) + ")").toBool());
    drain();
    QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), 3);
    QTRY_VERIFY(evaluate("!!" + pageReceiver).toBool());
    QTRY_COMPARE(page("location.href").toString(), rightUrl);
    QTRY_COMPARE(evaluate(ui + ".load_state").toString(), "complete");
    const auto rightId = evaluate(ui + ".tab_id_for_index(2)").toString();
    sendOwnedBrowserKey(host, Qt::Key_2, Qt::AltModifier);
    QTRY_VERIFY(evaluate("!!" + pageReceiver).toBool());
    QTRY_COMPARE(page("location.href").toString(), middleUrl);
    sendOwnedBrowserKey(host, Qt::Key_Escape);
    auto view = qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());
    QVERIFY(view);
    QTRY_VERIFY(host->activeFocusItem() && (host->activeFocusItem() == view || view->isAncestorOf(host->activeFocusItem())));
    page("window.closedRecoveryOldDocument=true;true");

    // Pinned tabs require an explicit target; the ordinary close binding
    // deliberately protects them. All reopening uses the configured keys.
    if (pinned) {
        QVERIFY(evaluate(ui + ".execute_command(" + quoted("tab-close --id " + closedId) + ")").toBool());
        drain();
    } else
        sendOwnedBrowserKey(host, Qt::Key_D);
    QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), 2);
    QCOMPARE(evaluate(ui + ".tab_index_for_id(" + quoted(closedId) + ")").toInt(), -1);
    if (modifiedShortcut)
        sendOwnedBrowserKey(host, Qt::Key_T, Qt::ControlModifier | Qt::ShiftModifier);
    else
        sendOwnedBrowserKey(host, Qt::Key_U);
    QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), 3);
    QTRY_VERIFY(evaluate("!!" + pageReceiver).toBool());
    QTRY_COMPARE(page("location.href").toString(), middleUrl);
    QTRY_COMPARE(evaluate(ui + ".load_state").toString(), "complete");
    const auto restoredIndex = evaluate(ui + ".active_tab_index").toInt();
    const auto restoredId = evaluate(ui + ".tab_id_for_index(" + QString::number(restoredIndex) + ")").toString();
    const auto restored = evaluate("JSON.parse(" + ui + ".tab_transfer_payload(" + quoted(restoredId) + "))").toMap();
    qInfo() << "Closed-tab recovery" << hostKind << attribute << modifiedShortcut
            << "index" << restoredIndex << "pinned" << restored.value("pinned")
            << "muted" << restored.value("muted") << "zoom" << restored.value("zoom_hundredths")
            << "native mute" << evaluate(pageReceiver + ".audioMuted")
            << "native zoom" << evaluate(pageReceiver + ".zoomFactor");
    if (attribute == "pinned")
        QCOMPARE(restored.value("pinned").toBool(), pinned);
    else if (attribute == "muted")
        QCOMPARE(restored.value("muted").toBool(), muted);
    else if (attribute == "zoom")
        QCOMPARE(restored.value("zoom_hundredths").toInt(), zoomHundredths);
    QCOMPARE(restoredIndex, 1);
    QCOMPARE(restored.value("pinned").toBool(), pinned);
    QCOMPARE(restored.value("muted").toBool(), muted);
    QCOMPARE(restored.value("zoom_hundredths").toInt(), zoomHundredths);
    QCOMPARE(evaluate(pageReceiver + ".audioMuted").toBool(), muted);
    QCOMPARE(qRound(evaluate(pageReceiver + ".zoomFactor").toDouble() * 100), zoomHundredths);
    QCOMPARE(evaluate(ui + ".tab_id_for_index(0)").toString(), leftId);
    QCOMPARE(evaluate(ui + ".tab_id_for_index(2)").toString(), rightId);
    QVERIFY(restoredId != closedId);
    QVERIFY(!page("window.closedRecoveryOldDocument===true").toBool());
    view = qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());
    QVERIFY(view);
    QTRY_VERIFY(host->activeFocusItem() && (host->activeFocusItem() == view || view->isAncestorOf(host->activeFocusItem())));
}

void ApplicationTests::productPolishTabRecoveryCheckpoint_data() {
    QTest::addColumn<bool>("legacy");
    QTest::newRow("current") << false;
    QTest::newRow("legacy-v2") << true;
}

void ApplicationTests::productPolishTabRecoveryCheckpoint() {
    QFETCH(bool, legacy);
    QString owner, ui;
    QQuickWindow *host = nullptr;
    preparePageSearchHost("primary", owner, ui, host);
    QVERIFY(host);
    const auto drain = [&] { evaluate(owner + ".executePendingEngineAction()"); };
    const auto saveReady = [&] {
        const auto saved = evaluate(ui + ".flush_durable_state()").toBool();
        if (!saved) qInfo() << "Checkpoint not ready" << evaluate(ui + ".status_text");
        return saved;
    };
    frameFixtures["/recover-saved.html"] = "<!doctype html><title>Saved closed tab</title><p>Saved tab</p>";
    frameFixtures["/recover-right.html"] = "<!doctype html><title>Saved right tab</title><p>Right tab</p>";
    const auto base = QUrl(page("location.href").toString());
    const auto savedUrl = base.resolved(QUrl("/recover-saved.html")).toString();
    const auto rightUrl = base.resolved(QUrl("/recover-right.html")).toString();
    QCOMPARE(evaluate(ui + ".set_tab_pinned(0,true)").toInt(), 0);
    QVERIFY(evaluate(ui + ".execute_command(" + quoted("tab-open " + savedUrl) + ")").toBool());
    drain();
    QTRY_VERIFY(evaluate("!!" + pageReceiver).toBool());
    QTRY_COMPARE(page("location.href").toString(), savedUrl);
    QTRY_COMPARE(evaluate(ui + ".load_state").toString(), "complete");
    const auto closedId = evaluate(ui + ".tab_id_for_index(1)").toString();
    QCOMPARE(evaluate(ui + ".set_tab_pinned(1,true)").toInt(), 1);
    QVERIFY(evaluate(ui + ".set_tab_muted(1,true)").toBool());
    drain();
    QVERIFY(evaluate(ui + ".execute_command('zoom 1.75')").toBool());
    drain();
    QTRY_COMPARE(qRound(evaluate(pageReceiver + ".zoomFactor").toDouble() * 100), 175);
    QVERIFY(evaluate(ui + ".execute_command(" + quoted("tab-open " + rightUrl) + ")").toBool());
    drain();
    QTRY_VERIFY(evaluate("!!" + pageReceiver).toBool());
    QTRY_COMPARE(page("location.href").toString(), rightUrl);
    QTRY_COMPARE(evaluate(ui + ".load_state").toString(), "complete");
    QVERIFY(evaluate(ui + ".execute_command(" + quoted("tab-close --id " + closedId) + ")").toBool());
    drain();
    QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), 2);
    QTRY_VERIFY_WITH_TIMEOUT(saveReady(), 5000);
    const auto root = directory.filePath("durable-test/productPolishTabRecoveryCheckpoint/"
        + QString::fromLatin1(QTest::currentDataTag()));
    QDirIterator paths(root, {"current*.json"}, QDir::Files, QDirIterator::Subdirectories);
    QVERIFY(paths.hasNext());
    const auto checkpointPath = paths.next();
    QVERIFY(!paths.hasNext());
    QFile checkpoint(checkpointPath);
    QVERIFY(checkpoint.open(QIODevice::ReadOnly));
    auto snapshot = QJsonDocument::fromJson(checkpoint.readAll()).object();
    checkpoint.close();
    QCOMPARE(snapshot.value("schema_version").toInt(), 4);
    auto closed = snapshot.value("closed_tabs").toArray();
    QCOMPARE(closed.size(), 1);
    const auto settings = closed[0].toObject();
    QCOMPARE(settings.value("url").toString(), savedUrl);
    QCOMPARE(settings.value("group_index").toInt(), 1);
    QCOMPARE(settings.value("pinned").toBool(), true);
    QCOMPARE(settings.value("muted").toBool(), true);
    QCOMPARE(settings.value("zoom_hundredths").toInt(), 175);

    // Consume the live descriptor, flush its removal, then recover only from
    // the saved on-disk generation through the production storage worker.
    sendOwnedBrowserKey(host, Qt::Key_U);
    QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), 3);
    QTRY_VERIFY(evaluate("!!" + pageReceiver).toBool());
    QTRY_COMPARE(page("location.href").toString(), savedUrl);
    QTRY_COMPARE(evaluate(ui + ".load_state").toString(), "complete");
    QVERIFY(!evaluate(ui + ".execute_command('tab-undo')").toBool());
    // Startup recovery begins with one live tab. Retire the extra fixture
    // tabs; closing the left neighbor last also makes the live undo record
    // differ from the on-disk record that recovery must reload.
    const auto reopenedId = evaluate(ui + ".tab_id_for_index(" + ui + ".active_tab_index)").toString();
    QVERIFY(evaluate(ui + ".execute_command(" + quoted("tab-close --id " + reopenedId) + ")").toBool());
    drain();
    QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), 2);
    const auto leftId = evaluate(ui + ".tab_id_for_index(0)").toString();
    QVERIFY(evaluate(ui + ".execute_command(" + quoted("tab-close --id " + leftId) + ")").toBool());
    drain();
    QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), 1);
    QTRY_VERIFY(evaluate("!!" + pageReceiver).toBool());
    QTRY_COMPARE(page("location.href").toString(), rightUrl);
    QTRY_VERIFY_WITH_TIMEOUT(saveReady(), 5000);
    if (legacy) {
        snapshot["schema_version"] = 2;
        snapshot.remove("recovered_session_ids");
        auto old = closed[0].toObject();
        for (const auto &key : {"group_index", "pinned", "muted", "zoom_hundredths"}) old.remove(key);
        snapshot["closed_tabs"] = QJsonArray{old};
    }
    const auto bytes = QJsonDocument(snapshot).toJson(QJsonDocument::Compact);
    // Install the saved generation after the warm owner has stopped writing,
    // before the new profile captures its recovery sources.
    restartRecoveryFixture("rb://start", checkpointPath, bytes);
    QVERIFY(window);
    host = window;
    sendOwnedBrowserKey(host, Qt::Key_Y);
    QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), 2);
    QTRY_VERIFY(evaluate("!!" + pageReceiver).toBool());
    QTRY_COMPARE(page("location.href").toString(), rightUrl);
    QTRY_COMPARE(evaluate(ui + ".load_state").toString(), "complete");
    qInfo() << "Recovered neighbors" << evaluate(owner + ".tabsModel.get(0).url")
            << evaluate(owner + ".tabsModel.get(1).url");
    QCOMPARE(evaluate(owner + ".tabsModel.get(0).url").toString(), base.toString());
    QCOMPARE(evaluate(owner + ".tabsModel.get(1).url").toString(), rightUrl);
    QVERIFY(!evaluate(owner + ".tabsModel.get(0).loaded").toBool());
    sendOwnedBrowserKey(host, Qt::Key_1, Qt::AltModifier);
    QTRY_VERIFY(evaluate("!!" + pageReceiver).toBool());
    QTRY_COMPARE(page("location.href").toString(), base.toString());
    QTRY_COMPARE(evaluate(ui + ".load_state").toString(), "complete");
    sendOwnedBrowserKey(host, Qt::Key_2, Qt::AltModifier);
    QTRY_VERIFY(evaluate("!!" + pageReceiver).toBool());
    QTRY_COMPARE(page("location.href").toString(), rightUrl);
    sendOwnedBrowserKey(host, Qt::Key_U);
    QTRY_COMPARE(evaluate(ui + ".tab_count").toInt(), 3);
    QTRY_VERIFY(evaluate("!!" + pageReceiver).toBool());
    QTRY_COMPARE(page("location.href").toString(), savedUrl);
    QTRY_COMPARE(evaluate(ui + ".load_state").toString(), "complete");
    const auto restoredIndex = evaluate(ui + ".active_tab_index").toInt();
    const auto restored = evaluate("JSON.parse(" + ui + ".tab_transfer_payload("
        + ui + ".tab_id_for_index(" + ui + ".active_tab_index)))").toMap();
    QCOMPARE(restoredIndex, legacy ? 2 : 1);
    QCOMPARE(restored.value("pinned").toBool(), !legacy);
    QCOMPARE(restored.value("muted").toBool(), !legacy);
    QCOMPARE(restored.value("zoom_hundredths").toInt(), legacy ? 100 : 175);
    QCOMPARE(evaluate(pageReceiver + ".audioMuted").toBool(), !legacy);
    QCOMPARE(qRound(evaluate(pageReceiver + ".zoomFactor").toDouble() * 100), legacy ? 100 : 175);
    auto view = qobject_cast<QQuickItem *>(evaluate(pageReceiver).value<QObject *>());
    QVERIFY(view);
    QTRY_VERIFY(host->activeFocusItem() && (host->activeFocusItem() == view || view->isAncestorOf(host->activeFocusItem())));
}
