#include "tst_application.h"
#include "ferric-browser-engine-qt/src/bridge.cxxqt.h"
#include <QElapsedTimer>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QQmlContext>
#include <QScopeGuard>
#include <QtTest>
#ifdef Q_OS_UNIX
#include <sys/stat.h>
#endif

void ApplicationTests::profileMetadataWaitsForEngineValidation_data() {
    QTest::addColumn<QString>("outcome");
    for (const auto &value : {QString("accept"), QString("accept-idle"), QString("reject"), QString("replace")})
        QTest::newRow(qPrintable(value)) << value;
}

void ApplicationTests::profileMetadataWaitsForEngineValidation() {
    QFETCH(QString, outcome);
    auto *validator = window->findChild<QObject *>("selectorValidationSurface");
    QVERIFY(validator);
    QTRY_VERIFY(validator->property("documentReady").toBool());
    QVERIFY(validator->setProperty("documentReady", false));
    const auto restoreValidator = qScopeGuard([&] { validator->setProperty("documentReady", true); });
    const auto base = evaluate("testWindow.storageBasePath").toString();
    const auto snapshot = [&] {
        QVariantList result;
        for (const auto &name : {QString("profiles.json"), QString("contexts.json")}) {
            const auto path = base + "/state/" + name;
            QFile file(path);
            const bool exists = file.exists();
            QVariantMap row{{"name", name}, {"exists", exists}, {"readable", true}};
            if (exists) {
                const bool readable = file.open(QIODevice::ReadOnly);
                row["readable"] = readable;
                if (readable) row["bytes"] = file.readAll();
#ifdef Q_OS_UNIX
                struct stat info {};
                const bool statOk = ::stat(path.toLocal8Bit().constData(), &info) == 0;
                row["statOk"] = statOk;
                if (statOk) {
                    row["device"] = qulonglong(info.st_dev);
                    row["inode"] = qulonglong(info.st_ino);
                    row["mtime"] = qlonglong(info.st_mtime);
#ifdef Q_OS_LINUX
                    row["mtimeNanos"] = qlonglong(info.st_mtim.tv_nsec);
#endif
                }
#endif
            }
            result.append(row);
        }
        return result;
    };
    const auto before = snapshot();
    for (const auto &row : before) {
        QVERIFY(row.toMap().value("readable").toBool());
        if (row.toMap().value("exists").toBool()) QVERIFY(row.toMap().value("statOk", true).toBool());
    }
    const auto oldConfig = text("config_json");
    const auto selector = outcome == "reject" ? QString("a[") : QString("#button");
    const auto config = QJsonObject{{"schema_version", 3}, {"ui", QJsonObject{{"font_size_pt", 19}}},
        {"hints", QJsonObject{{"selectors", QJsonObject{{"links", QJsonObject{
            {"mode", "replace"}, {"selectors", QJsonArray{selector}}}}}}}}};
    evaluate("testWindow.browserUi.config_base_json=" + quoted(QString::fromUtf8(QJsonDocument(config).toJson(QJsonDocument::Compact))));
    evaluate("testWindow.browserUi.contexts_json=" + quoted(R"JSON({"schema_version":3,"contexts":[{"name":"metadata-held-context","label":"Metadata held context","profile":"metadata-waiting"}]})JSON"));
    evaluate("testWindow.browserUi.configure_profile(false,false,'Metadata waiting','metadata-waiting',testWindow.storageBasePath)");
    QTRY_VERIFY_WITH_TIMEOUT(!text("selector_validation_request").isEmpty(), 6000);
    const auto generation = QJsonDocument::fromJson(text("selector_validation_request").toUtf8()).object()["generation"].toString();
    QVERIFY(!generation.isEmpty());
    QCOMPARE(snapshot(), before);
    QCOMPARE(text("config_json"), oldConfig);
    QVERIFY(evaluate("testWindow.browserUi.profile_bootstrap_pending").toBool());
    // Keep the parser barrier closed across real generic polls and event turns.
    // A later poll must not admit metadata merely because the worker is ready.
    for (int turn = 0; turn != 3; ++turn) {
        evaluate("testWindow.browserUi.poll_config()");
        QCoreApplication::processEvents();
        QVERIFY(!validator->property("documentReady").toBool());
        QCOMPARE(QJsonDocument::fromJson(text("selector_validation_request").toUtf8())
                     .object()["generation"].toString(), generation);
        QVERIFY(evaluate("testWindow.browserUi.profile_bootstrap_pending").toBool());
        QCOMPARE(snapshot(), before);
        QCOMPARE(text("config_json"), oldConfig);
    }
    if (outcome == "replace") {
        evaluate("testWindow.browserUi.config_base_json='{" "\"schema_version\":3,\"ui\":{\"font_size_pt\":22}}'");
        evaluate("testWindow.browserUi.contexts_json='{" "\"schema_version\":3}'");
        evaluate("testWindow.browserUi.configure_profile(false,false,'Metadata newer','metadata-newer',testWindow.storageBasePath)");
        QTRY_VERIFY_WITH_TIMEOUT(!evaluate("testWindow.browserUi.profile_bootstrap_pending").toBool(), 10000);
        QVERIFY(!evaluate("testWindow.browserUi.complete_selector_validation(" + quoted(generation) + ",'{\"ok\":true}')").toBool());
    } else {
        engine->rootContext()->setContextProperty("metadataHeldValidator", validator);
        QSignalSpy runtimeWake(evaluate("testWindow.browserUi").value<QObject *>(), SIGNAL(runtime_work_available()));
        QVERIFY(runtimeWake.isValid());
        if (outcome == "accept-idle") {
            evaluate("testWindow.runtime_runtimeWorkTimer.stop()");
            QVERIFY(!evaluate("testWindow.runtime_runtimeWorkTimer.running").toBool());
        }
        evaluate("metadataHeldValidator.documentReady=true;metadataHeldValidator.validateRequest(JSON.parse(testWindow.browserUi.selector_validation_request))");
        QTRY_VERIFY_WITH_TIMEOUT(!evaluate("testWindow.browserUi.profile_bootstrap_pending").toBool(), 10000);
        if (outcome == "accept-idle") QVERIFY(!runtimeWake.isEmpty());
    }
    QFile profiles(base + "/state/profiles.json");
    QVERIFY(profiles.open(QIODevice::ReadOnly));
    const auto profileRows = QJsonDocument::fromJson(profiles.readAll()).array();
    QStringList names;
    for (const auto &row : profileRows) names.append(row.toObject()["name"].toString());
    QCOMPARE(names.contains("metadata-waiting"), outcome != "replace");
    QCOMPARE(names.contains("metadata-newer"), outcome == "replace");
    QFile contexts(base + "/state/contexts.json");
    const auto contextBytes = contexts.open(QIODevice::ReadOnly) ? contexts.readAll() : QByteArray();
    QCOMPARE(contextBytes.contains("metadata-held-context"), outcome != "replace");
    const auto active = QJsonDocument::fromJson(text("config_json").toUtf8()).object();
    QCOMPARE(active["ui"].toObject()["font_size_pt"].toDouble(), outcome == "replace" ? 22.0 : outcome == "reject" ? 10.0 : 19.0);
    QVERIFY(text("selector_validation_request").isEmpty());
}

// Use only public UI operations and actual worker completions.
void ApplicationTests::inheritedProfileReentryKeepsConfiguration() {
    auto *source = qobject_cast<BrowserUi *>(evaluate("testWindow.browserUi").value<QObject *>());
    QVERIFY(source);
    QVERIFY(source->set_runtime_setting(QString("content.zoom"), QString("1.25"), true));
    const auto zoom = [](BrowserUi &owner) {
        owner.poll_config();
        return QJsonDocument::fromJson(owner.property("config_json").toString().toUtf8())
            .object()["content"].toObject()["zoom"].toDouble();
    };
    QTRY_COMPARE(zoom(*source), 1.25);
    const auto profile = evaluate("testWindow.browserWindowRegistry[0].profileName").toString();
    QVERIFY(!profile.isEmpty());
    BrowserUi child;
    bool called = false, reloadAccepted = false, reloadCompleted = false;
    bool pendingBeforeReload = false, unreadyPolicyBeforeReload = false;
    bool readyDuringNotification = false, editAccepted = false, editCompleted = false;
    const auto connection = connect(&child, &BrowserUi::start_page_profile_revisionChanged,
        &child, [&] {
            if (called) return;
            called = true;
            pendingBeforeReload = child.property("profile_bootstrap_pending").toBool();
            unreadyPolicyBeforeReload = !child.property("blocking_policy_ready").toBool();
            if (!pendingBeforeReload || !unreadyPolicyBeforeReload) return;
            reloadAccepted = child.reload_blocking_policy();
            if (!reloadAccepted) return;
            QElapsedTimer deadline;
            deadline.start();
            while (deadline.elapsed() < 6000 && !child.property("blocking_policy_ready").toBool()) {
                child.poll_config();
                QTest::qWait(1);
            }
            reloadCompleted = child.property("blocking_policy_ready").toBool();
            readyDuringNotification = !child.property("profile_bootstrap_pending").toBool();
            if (!readyDuringNotification) return;
            editAccepted = child.set_runtime_setting(QString("content.zoom"), QString("1.5"), true);
            deadline.restart();
            while (editAccepted && deadline.elapsed() < 6000 && zoom(child) != 1.5) QTest::qWait(1);
            editCompleted = editAccepted && zoom(child) == 1.5;
        }, Qt::DirectConnection);
    QVERIFY(source->initialize_window_owner(child, profile));
    disconnect(connection);
    qInfo() << "inheritance reentry" << called << pendingBeforeReload << unreadyPolicyBeforeReload
            << reloadAccepted << reloadCompleted << readyDuringNotification << editAccepted << editCompleted;
    QVERIFY(called);
    QVERIFY(pendingBeforeReload);
    QVERIFY(unreadyPolicyBeforeReload);
    if (reloadAccepted) QVERIFY(reloadCompleted);
    if (readyDuringNotification) {
        QVERIFY(editAccepted);
        QVERIFY(editCompleted);
        // A completed edit may not be overwritten by the older finish call.
        QCOMPARE(zoom(child), 1.5);
    }
    QTRY_VERIFY_WITH_TIMEOUT((child.poll_config(), !child.property("profile_bootstrap_pending").toBool()), 6000);
    if (!readyDuringNotification) {
        QVERIFY(child.set_runtime_setting(QString("content.zoom"), QString("1.5"), true));
        QTRY_COMPARE(zoom(child), 1.5);
    }
    // Subsequent real temporary composition must retain the published zoom.
    QVERIFY(child.set_runtime_setting(QString("content.images"), QString("false"), true));
    const auto images = [&] {
        child.poll_config();
        return QJsonDocument::fromJson(child.property("config_json").toString().toUtf8())
            .object()["content"].toObject()["images"].toBool();
    };
    QTRY_COMPARE(images(), false);
    QCOMPARE(zoom(child), 1.5);
}

void ApplicationTests::releasedProfileSelectorCannotRestoreResources() {
    for (bool afterDeadline : {false, true}) {
        BrowserUi owner;
        const auto base = directory.filePath(afterDeadline ? "released-expired" : "released-callback");
        owner.configure_profile(true, false, QString("Released old"), QString("released-old"), base);
        const auto ready = [&] {
            owner.poll_config();
            return !owner.property("profile_bootstrap_pending").toBool();
        };
        QTRY_VERIFY_WITH_TIMEOUT(ready(), 6000);
        QVERIFY(owner.property("view_alive").toBool());
        QVERIFY(owner.property("tab_count").toInt() > 0);
        QVERIFY(owner.setProperty("config_base_json", QString::fromUtf8(
            R"JSON({"schema_version":3,"ui":{"font_size_pt":19},"hints":{"selectors":{"links":{"mode":"replace","selectors":["#released-profile"]}}}})JSON")));
        owner.configure_profile(true, false, QString("Released pending"), QString("released-pending"), base);
        const auto held = [&] {
            owner.poll_config();
            return !owner.property("selector_validation_request").toString().isEmpty();
        };
        QTRY_VERIFY_WITH_TIMEOUT(held(), 6000);
        const auto generation = QJsonDocument::fromJson(owner.property("selector_validation_request").toString().toUtf8())
            .object()["generation"].toString();
        QVERIFY(!generation.isEmpty());
        // This owner has no validation receiver: the real production request
        // remains held until a callback or the actual deadline handles it.
        QVERIFY(owner.release_transient_resources());
        QVERIFY(!owner.property("view_alive").toBool());
        QCOMPARE(owner.property("tab_count").toInt(), 0);
        if (afterDeadline) blockEventLoopForSelectorDeadline();
        QVERIFY(!owner.complete_selector_validation(generation, QString("{\"ok\":true}")));
        const auto workDrained = [&] {
            owner.poll_config();
            // Long process-wide page-group maintenance can remain enabled by
            // the separate production QML fixture; no owner work may stay busy.
            const auto delay = owner.maintenance_delay_ms();
            return delay == -1 || delay >= 1000;
        };
        QTRY_VERIFY_WITH_TIMEOUT(workDrained(), 3000);
        QVERIFY(owner.property("selector_validation_request").toString().isEmpty());
        QVERIFY(!owner.property("view_alive").toBool());
        QCOMPARE(owner.property("tab_count").toInt(), 0);
        QVERIFY(owner.property("core_window_id").toString().isEmpty());
        QVERIFY(!owner.complete_selector_validation(generation, QString("{\"ok\":true}")));
        qInfo() << "released profile stayed retired" << "after deadline" << afterDeadline;
    }
}
