#include "qml_diagnostics.h"

#include <algorithm>

#include <QtCore/QCoreApplication>
#include <QtCore/QElapsedTimer>
#include <QtCore/QDebug>
#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QMetaObject>
#include <QtCore/QStringList>
#include <QtCore/QThread>
#include <QtGui/QGuiApplication>
#include <QtGui/QMouseEvent>
#include <QtGui/QWindow>
#include <QtQuick/QQuickItem>
#include <QtQml/QQmlComponent>
#include <QtQml/QQmlError>
#include <QtQml/QQmlEngine>
#include <QtQml/QJSValue>
#include <QtWebEngineCore/qtwebenginecoreglobal.h>

#if QT_CONFIG(webengine_extensions)
#include <QtCore/QUrlQuery>
#include <QtWebEngineCore/QWebEngineDownloadRequest>
#include <QtWebEngineCore/QWebEngineExtensionInfo>
#include <QtWebEngineCore/QWebEngineExtensionManager>
#include <QtWebEngineQuick/QQuickWebEngineDownloadRequest>
#include <QtWebEngineQuick/QQuickWebEngineProfile>
#endif

#include <cstdio>

void ferric_browser_suppress_qml_warning_stderr(QQmlApplicationEngine &engine)
{
    engine.setOutputWarningsToStandardError(false);
}

void ferric_browser_enable_qml_diagnostics(QQmlApplicationEngine &engine)
{
    // QQmlError::toString() can contain source paths and runtime error text.
    // Keep the useful fact that QML warnings occurred, but never echo those
    // details into persistent shell/service logs.
    ferric_browser_suppress_qml_warning_stderr(engine);
    QObject::connect(
        &engine,
        &QQmlEngine::warnings,
        &engine,
        [](const QList<QQmlError> &warnings) {
            if (!warnings.isEmpty()) {
                qWarning().noquote() << "Ferric QML warning details suppressed; count:"
                                     << warnings.size();
            }
        });
}

void ferric_browser_dump_qml_component_errors(
    QQmlApplicationEngine &engine,
    const QUrl &url,
    const QVariantMap &initialProperties)
{
    std::fprintf(stderr, "Ferric QML diagnostic component probe started\n");
    QQmlComponent component(&engine, url, QQmlComponent::PreferSynchronous);
    QObject *root = component.createWithInitialProperties(initialProperties, engine.rootContext());
    const auto errors = component.errors();
    if (!errors.isEmpty()) {
        // Explicit startup diagnostics must not turn local source paths or
        // runtime-provided error text into shell/service log contents.
        std::fprintf(stderr,
                     "Ferric QML component diagnostic details suppressed; error count: %lld\n",
                     static_cast<long long>(errors.size()));
    }
    if (root != nullptr) {
        delete root;
    }
    if (errors.isEmpty() && root == nullptr) {
        std::fprintf(stderr, "Ferric QML component diagnostic: no QQmlError was reported\n");
    } else if (errors.isEmpty()) {
        std::fprintf(stderr, "Ferric QML component diagnostic: root created with the same initial properties\n");
    }
}

namespace {
QObject *archive_test_root(QQmlApplicationEngine &engine)
{
    const auto roots = engine.rootObjects();
    return roots.isEmpty() ? nullptr : roots.first();
}

QObject *archive_test_ui(QObject *root)
{
    return root == nullptr ? nullptr : root->property("primaryBrowserUi").value<QObject *>();
}

QObject *archive_test_secondary_window(
    QQmlApplicationEngine &engine, const QString &startupUrl)
{
    QObject *root = archive_test_root(engine);
    for (QWindow *candidate : QGuiApplication::topLevelWindows()) {
        if (candidate != root
            && candidate->property("windowStartupUrl").toString() == startupUrl)
            return candidate;
    }
    return nullptr;
}

QObject *archive_test_secondary_view(QObject *window)
{
    return window == nullptr ? nullptr : window->property("activeView").value<QObject *>();
}

QObject *clean_branch_test_child(QQmlApplicationEngine &engine)
{
    QObject *root = archive_test_root(engine);
    for (QWindow *candidate : QGuiApplication::topLevelWindows()) {
        if (candidate != root && candidate->property("windowCleanBranch").toBool())
            return candidate;
    }
    return nullptr;
}

bool archive_test_submit_form(QObject *view, const QString &expectedUrl)
{
    if (view == nullptr || view->property("url").toUrl().toString() != expectedUrl)
        return false;
    // This fixed synthetic form submits inside the real renderer. No body or
    // field value crosses the test bridge.
    const QString script = QStringLiteral(
        "(() => { const form = document.querySelector('form'); "
        "if (!form || !['/versioning/form/result', "
        "'/versioning/redirect-303/submit', "
        "'/versioning/redirect-307/submit'].includes(form.getAttribute('action'))) "
        "return false; form.requestSubmit(); return true; })()");
    return QMetaObject::invokeMethod(view, "runJavaScript",
                                     Q_ARG(QString, script), Q_ARG(QJSValue, QJSValue()));
}
}

QString ferric_browser_test_archive_window_state(QQmlApplicationEngine &engine)
{
    QObject *root = archive_test_root(engine);
    QObject *ui = archive_test_ui(root);
    QJsonObject state;
    state.insert("root_ready", root != nullptr);
    state.insert("ui_ready", ui != nullptr);
    if (root != nullptr) {
        state.insert("shutdown_in_progress", root->property("applicationShutdownInProgress").toBool());
        state.insert("shutdown_approved", root->property("shutdownApproved").toBool());
        state.insert("shutdown_stage", root->property("applicationShutdownStage").toString());
        state.insert("shutdown_force_prompt", root->property("applicationShutdownForcePromptVisible").toBool());
        state.insert("archive_close_requested", root->property("archiveShutdownCloseRequested").toBool());
        QObject *host = root->property("versionedArchiveHostObject").value<QObject *>();
        state.insert("archive_host_ready", host != nullptr);
        state.insert("archive_host_active", host != nullptr && host->property("active").toBool());
        state.insert("archive_presented_count", host == nullptr ? 0 : host->property("presentedCount").toInt());
        state.insert("archive_blanked_count", host == nullptr ? 0 : host->property("blankedCount").toInt());
        QObject *item = host == nullptr ? nullptr : host->property("item").value<QObject *>();
        state.insert("archive_host_item_ready", item != nullptr);
        if (item != nullptr) {
            state.insert("archive_close_retry_available", item->property("closeRetryAvailable").toBool());
            state.insert("archive_notice", item->property("archiveNotice").toString());
        }
    }
    if (ui != nullptr) {
        state.insert("profile_bootstrap_pending", ui->property("profile_bootstrap_pending").toBool());
        state.insert("status", ui->property("status_text").toString());
        state.insert("archive_url", ui->property("versioning_archive_view_url").toString());
        state.insert("archive_operation", ui->property("versioning_archive_view_operation").toString());
        bool ready = false;
        if (QMetaObject::invokeMethod(ui, "versioned_archive_shutdown_ready",
                                      Q_RETURN_ARG(bool, ready))) {
            state.insert("archive_shutdown_ready", ready);
        }
    }
    return QString::fromUtf8(QJsonDocument(state).toJson(QJsonDocument::Compact));
}

bool ferric_browser_test_request_archive_view(QQmlApplicationEngine &engine, const QString &commit)
{
    QObject *ui = archive_test_ui(archive_test_root(engine));
    bool accepted = false;
    return ui != nullptr
        && QMetaObject::invokeMethod(ui, "request_versioned_archive_view",
                                     Q_RETURN_ARG(bool, accepted), Q_ARG(QString, commit))
        && accepted;
}

bool ferric_browser_test_attempt_archive_profile_replacement(QQmlApplicationEngine &engine)
{
    QObject *ui = archive_test_ui(archive_test_root(engine));
    const QString label = QStringLiteral("unexpected archive profile switch");
    const QString name = QStringLiteral("unexpected-archive-profile-switch");
    const QString storageBase;
    return ui != nullptr
        && QMetaObject::invokeMethod(ui, "configure_profile",
                                     Q_ARG(bool, false), Q_ARG(bool, true),
                                     Q_ARG(QString, label), Q_ARG(QString, name),
                                     Q_ARG(QString, storageBase));
}

bool ferric_browser_test_close_archive_view(QQmlApplicationEngine &engine)
{
    QObject *root = archive_test_root(engine);
    QObject *host = root == nullptr ? nullptr
        : root->property("versionedArchiveHostObject").value<QObject *>();
    return host != nullptr && QMetaObject::invokeMethod(host, "closeArchive");
}

QString ferric_browser_test_clean_branch_state(QQmlApplicationEngine &engine)
{
    QObject *root = archive_test_root(engine);
    QObject *child = clean_branch_test_child(engine);
    QObject *childUi = child == nullptr ? nullptr
        : child->property("windowBranchUi").value<QObject *>();
    QJsonObject state;
    state.insert("root_ready", root != nullptr);
    state.insert("child_present", child != nullptr);
    state.insert("child_published", child != nullptr
        && child->property("windowCleanBranchPublished").toBool());
    state.insert("child_native_bound", child != nullptr
        && child->property("windowCleanBranchNativeBound").toBool());
    state.insert("child_visible", child != nullptr
        && qobject_cast<QWindow *>(child)->isVisible());
    state.insert("child_shutdown_approved", child != nullptr
        && child->property("shutdownApproved").toBool());
    state.insert("child_shutdown_in_progress", child != nullptr
        && child->property("applicationShutdownInProgress").toBool());
    state.insert("child_shutdown_stage", child == nullptr
        ? QString() : child->property("applicationShutdownStage").toString());
    state.insert("child_status", childUi == nullptr
        ? QString() : childUi->property("status_text").toString());
    state.insert("parent_permission_prompt", root != nullptr
        && root->property("permissionPromptVisible").toBool());
    state.insert("child_permission_prompt", child != nullptr
        && child->property("permissionPromptVisible").toBool());
    state.insert("child_permission_name", child == nullptr
        ? QString() : child->property("pendingPermissionName").toString());
    state.insert("child_permission_owner_exact", child != nullptr && childUi != nullptr
        && child->property("pendingPermissionUi").value<QObject *>() == childUi);
    state.insert("child_ephemeral", child != nullptr
        && child->property("ephemeralProfile").toBool());
    state.insert("child_tab_count", childUi == nullptr
        ? 0 : childUi->property("tab_count").toInt());
    QJsonArray childTabIds;
    if (childUi != nullptr) {
        const int count = std::min(childUi->property("tab_count").toInt(), 128);
        for (int index = 0; index < count; ++index) {
            QString id;
            if (!QMetaObject::invokeMethod(childUi, "tab_id_for_index",
                                           Q_RETURN_ARG(QString, id), Q_ARG(int, index)))
                break;
            childTabIds.append(id);
        }
    }
    state.insert("child_tab_ids", childTabIds);
    state.insert("child_current_url", childUi == nullptr
        ? QString() : childUi->property("current_url").toString());
    state.insert("child_load_state", childUi == nullptr
        ? QString() : childUi->property("load_state").toString());
    QObject *childView = nullptr;
    if (child != nullptr) {
        const QVariant views = child->property("tabViews");
        if (views.metaType() == QMetaType::fromType<QJSValue>())
            childView = views.value<QJSValue>().property(0).toQObject();
        else if (!views.toList().isEmpty())
            childView = views.toList().first().value<QObject *>();
    }
    state.insert("child_view_url", childView == nullptr
        ? QString() : childView->property("url").toUrl().toString());
    state.insert("child_view_progress", childView == nullptr
        ? 0 : childView->property("loadProgress").toInt());
    state.insert("close_prompt", child != nullptr
        && child->property("shutdownConfirmationVisible").toBool());
    state.insert("page_prompt", child != nullptr
        && child->property("shutdownPagePromptVisible").toBool());
    QJsonArray outcomes;
    if (child != nullptr) {
        const QVariant raw = child->property("windowBranchPageOutcomes");
        const QVariantList values = raw.metaType() == QMetaType::fromType<QJSValue>()
            ? raw.value<QJSValue>().toVariant().toList() : raw.toList();
        for (const QVariant &value : values) {
            const QString outcome = value.toString();
            outcomes.append(outcome == "pending" || outcome == "loaded"
                || outcome == "failed" ? outcome : "unknown");
        }
    }
    state.insert("outcomes", outcomes);
    QJsonArray starts;
    if (child != nullptr) {
        const QVariant raw = child->property("windowBranchPageNavigationStarted");
        const QVariantList values = raw.metaType() == QMetaType::fromType<QJSValue>()
            ? raw.value<QJSValue>().toVariant().toList() : raw.toList();
        for (const QVariant &value : values)
            starts.append(value.toBool());
    }
    state.insert("planned_starts", starts);
    int popupCount = 0;
    int allBranchPopupCount = 0;
    bool popupOwnersExact = true;
    QJsonArray popupStates;
    const QString token = child == nullptr ? QString()
        : child->property("windowBoundBranchToken").toString();
    QObject *profile = child == nullptr ? nullptr
        : child->property("windowBranchProfile").value<QObject *>();
    QObject *interceptor = child == nullptr ? nullptr
        : child->property("primaryRequestInterceptor").value<QObject *>();
    for (QWindow *candidate : QGuiApplication::topLevelWindows()) {
        const QString popupToken = candidate->property("popupBranchToken").toString();
        if (popupToken.isEmpty())
            continue;
        ++allBranchPopupCount;
        if (child != nullptr && popupToken == token) {
            ++popupCount;
            QJsonObject popupState;
            popupState.insert("visible", candidate->isVisible());
            popupState.insert("approved", candidate->property("windowShutdownApproved").toBool());
            popupState.insert("page_prompt", candidate->property("windowShutdownPagePromptVisible").toBool());
            popupState.insert("page_prompt_reason", candidate->property("windowShutdownPagePromptReason").toString());
            popupState.insert("work_prompt", candidate->property("windowShutdownPromptVisible").toBool());
            popupState.insert("status", candidate->property("statusUi").value<QObject *>() == nullptr
                ? QString() : candidate->property("statusUi").value<QObject *>()->property("status_text").toString());
            popupStates.append(popupState);
            popupOwnersExact &= candidate->property("popupProfile").value<QObject *>() == profile
                && candidate->property("popupPermissionUi").value<QObject *>() == childUi
                && candidate->property("popupRequestInterceptor").value<QObject *>() == interceptor;
        }
    }
    state.insert("popup_count", popupCount);
    state.insert("all_branch_popup_count", allBranchPopupCount);
    state.insert("popup_states", popupStates);
    state.insert("popup_owners_exact", popupOwnersExact);
    return QString::fromUtf8(QJsonDocument(state).toJson(QJsonDocument::Compact));
}

bool ferric_browser_test_execute_root_command(
    QQmlApplicationEngine &engine, const QString &command)
{
    QObject *root = archive_test_root(engine);
    QObject *ui = archive_test_ui(root);
    bool accepted = false;
    return root != nullptr && ui != nullptr
        && QMetaObject::invokeMethod(ui, "execute_command",
                                     Q_RETURN_ARG(bool, accepted), Q_ARG(QString, command))
        && accepted && QMetaObject::invokeMethod(root, "processRuntimeWork");
}

bool ferric_browser_test_execute_clean_branch_command(
    QQmlApplicationEngine &engine, const QString &command)
{
    QObject *child = clean_branch_test_child(engine);
    QObject *ui = child == nullptr ? nullptr
        : child->property("windowBranchUi").value<QObject *>();
    bool accepted = false;
    return child != nullptr && ui != nullptr
        && QMetaObject::invokeMethod(ui, "execute_command",
                                     Q_RETURN_ARG(bool, accepted), Q_ARG(QString, command))
        && accepted && QMetaObject::invokeMethod(child, "executePendingEngineAction");
}

bool ferric_browser_test_first_clean_branch_tab_close_refused(
    QQmlApplicationEngine &engine)
{
    QObject *child = clean_branch_test_child(engine);
    QVariant closed;
    return child != nullptr && child->property("windowCleanBranchPublished").toBool()
        && QMetaObject::invokeMethod(child, "closeTabAtIndex",
                                     Q_RETURN_ARG(QVariant, closed),
                                     Q_ARG(QVariant, QVariant(0)))
        && !closed.toBool();
}

bool ferric_browser_test_disable_clean_branch_factory(QQmlApplicationEngine &engine)
{
    QObject *root = archive_test_root(engine);
    return root != nullptr
        && root->setProperty("tabCapableWindowFactory", QVariant::fromValue<QObject *>(nullptr))
        && root->property("tabCapableWindowFactory").value<QObject *>() == nullptr;
}

bool ferric_browser_test_disable_pending_clean_branch_view_factory(
    QQmlApplicationEngine &engine)
{
    QObject *child = clean_branch_test_child(engine);
    return child != nullptr
        && child->property("windowCleanBranchNativeBound").toBool()
        && !child->property("windowCleanBranchPublished").toBool()
        && !child->property("webViewComponentObject").isNull()
        && child->setProperty("webViewComponentObject", QVariant::fromValue<QObject *>(nullptr))
        && child->property("webViewComponentObject").value<QObject *>() == nullptr;
}

bool ferric_browser_test_request_clean_branch_close(QQmlApplicationEngine &engine)
{
    auto *child = qobject_cast<QWindow *>(clean_branch_test_child(engine));
    if (child == nullptr)
        return false;
    child->close();
    return true;
}

bool ferric_browser_test_confirm_clean_branch_close(QQmlApplicationEngine &engine)
{
    QObject *child = clean_branch_test_child(engine);
    return child != nullptr && child->property("shutdownConfirmationVisible").toBool()
        && QMetaObject::invokeMethod(child, "continueQuitRequest");
}

bool ferric_browser_test_confirm_clean_branch_page_close(QQmlApplicationEngine &engine)
{
    QObject *child = clean_branch_test_child(engine);
    return child != nullptr && child->property("shutdownPagePromptVisible").toBool()
        && QMetaObject::invokeMethod(child, "confirmShutdownPagePrompt");
}

bool ferric_browser_test_confirm_clean_branch_popup_page_close(
    QQmlApplicationEngine &engine)
{
    QObject *child = clean_branch_test_child(engine);
    if (child == nullptr)
        return false;
    const QString token = child->property("windowBoundBranchToken").toString();
    for (QWindow *candidate : QGuiApplication::topLevelWindows()) {
        if (candidate->property("popupBranchToken").toString() != token
            || !candidate->property("windowShutdownPagePromptVisible").toBool())
            continue;
        return QMetaObject::invokeMethod(candidate, "confirmShutdownPagePrompt");
    }
    return false;
}

bool ferric_browser_test_remember_normal_notification_permission(
    QQmlApplicationEngine &engine, const QString &origin)
{
    QObject *ui = archive_test_ui(archive_test_root(engine));
    bool accepted = false;
    return ui != nullptr
        && QMetaObject::invokeMethod(ui, "remember_permission",
                                     Q_RETURN_ARG(bool, accepted),
                                     Q_ARG(QString, origin),
                                     Q_ARG(QString, QStringLiteral("notifications")),
                                     Q_ARG(QString, QStringLiteral("allow")),
                                     Q_ARG(QString, QStringLiteral("site")))
        && accepted;
}

bool ferric_browser_test_normal_notification_allowed(
    QQmlApplicationEngine &engine, const QString &origin)
{
    QObject *ui = archive_test_ui(archive_test_root(engine));
    QString decision;
    return ui != nullptr
        && QMetaObject::invokeMethod(ui, "permission_decision",
                                     Q_RETURN_ARG(QString, decision),
                                     Q_ARG(QString, origin),
                                     Q_ARG(QString, QStringLiteral("notifications")))
        && decision == QStringLiteral("allow");
}

bool ferric_browser_test_deny_clean_branch_permission(QQmlApplicationEngine &engine)
{
    QObject *child = clean_branch_test_child(engine);
    return child != nullptr && child->property("permissionPromptVisible").toBool()
        && QMetaObject::invokeMethod(child, "decidePermission",
                                     Q_ARG(QVariant, QVariant(false)),
                                     Q_ARG(QVariant, QVariant(QString())));
}

bool ferric_browser_test_click_clean_branch_page(QQmlApplicationEngine &engine)
{
    auto *child = qobject_cast<QWindow *>(clean_branch_test_child(engine));
    if (child == nullptr || !child->isVisible())
        return false;
    const QVariant views = child->property("tabViews");
    QObject *first = views.metaType() == QMetaType::fromType<QJSValue>()
        ? views.value<QJSValue>().property(0).toQObject()
        : (views.toList().isEmpty() ? nullptr : views.toList().first().value<QObject *>());
    auto *view = qobject_cast<QQuickItem *>(first);
    if (view == nullptr || !view->isVisible())
        return false;
    const QPointF local = view->mapToScene(QPointF(40, 20));
    const QPointF global = child->mapToGlobal(local.toPoint());
    QMouseEvent press(QEvent::MouseButtonPress, local, local, global,
                      Qt::LeftButton, Qt::LeftButton, Qt::NoModifier);
    QCoreApplication::sendEvent(child, &press);
    QMouseEvent release(QEvent::MouseButtonRelease, local, local, global,
                        Qt::LeftButton, Qt::NoButton, Qt::NoModifier);
    QCoreApplication::sendEvent(child, &release);
    return true;
}

QString ferric_browser_test_page_capture_state(QQmlApplicationEngine &engine)
{
    QObject *root = archive_test_root(engine);
    QObject *ui = archive_test_ui(root);
    QJsonObject state;
    state.insert("root_ready", root != nullptr);
    state.insert("ui_ready", ui != nullptr);
    if (root != nullptr) {
        state.insert("profile_bootstrap_complete",
                     root->property("profileBootstrapComplete").toBool());
        state.insert("shutdown_approved", root->property("shutdownApproved").toBool());
        state.insert("shutdown_in_progress",
                     root->property("applicationShutdownInProgress").toBool());
        state.insert("shutdown_waiting_for_work",
                     root->property("shutdownWaitingForDownloadFinishes").toBool());
        state.insert("shutdown_prompt_visible",
                     root->property("shutdownPromptVisible").toBool());
        state.insert("restore_editable_dialog_visible",
                     root->property("versioningRestoreEditableDialogVisible").toBool());
        QObject *restoreHost = root->property("versioningRestoreCoordinatorObject").value<QObject *>();
        QObject *restoreCoordinator = restoreHost == nullptr ? nullptr
            : restoreHost->property("item").value<QObject *>();
        state.insert("restore_preview_refillable_count", restoreCoordinator == nullptr
            ? 0 : restoreCoordinator->property("previewRefillableCount").toInt());
        const QVariant captureValue = root->property("nativePageCaptureSave");
        state.insert("native_capture_active", !captureValue.isNull());
        const QVariantMap capture = captureValue.metaType() == QMetaType::fromType<QJSValue>()
            ? captureValue.value<QJSValue>().toVariant().toMap()
            : captureValue.toMap();
        if (!capture.isEmpty()) {
            state.insert("collection_finished", capture.value("collectionFinished").toBool());
            state.insert("collector_timed_out", capture.value("collectionTimedOut").toBool());
            state.insert("sidecar_accepted", capture.value("sidecarAccepted").toBool());
            state.insert("media_key_status", capture.value("mediaKeyStatus").toString());
            state.insert("media_key_wait_attempts",
                         capture.value("mediaKeyWaitAttempts").toInt());
            state.insert("media_requested", capture.value("mediaRequested").toBool());
            state.insert("collector_started", capture.value("collectorStarted").toBool());
            state.insert("collector_start_failed",
                         capture.value("collectorStartFailed").toBool());
            state.insert("collector_entry_point_available",
                         capture.value("collectorEntryPointAvailable").toBool());
            state.insert("collector_start_error_name",
                         capture.value("collectorStartErrorName").toString());
            state.insert("collector_start_error_line",
                         capture.value("collectorStartErrorLine").toInt());
            state.insert("collector_reference_error_symbol",
                         capture.value("collectorReferenceErrorSymbol").toString());
            state.insert("collector_start_duration_ms",
                         capture.value("collectorStartDurationMs").toInt());
            state.insert("collector_callback_received",
                         capture.value("collectorCallbackReceived").toBool());
            state.insert("collector_result_type",
                         capture.value("collectorResultType").toString());
            state.insert("collector_result_length",
                         capture.value("collectorResultLength").toInt());
            state.insert("save_requested", !capture.value("path").toString().isEmpty());
            state.insert("download_started", !capture.value("downloadId").toString().isEmpty());
            state.insert("download_finished", capture.value("downloadFinished").toBool());
            int captureTabIndex = -1;
            const QString captureTabId = capture.value("tabId").toString();
            const int tabCount = ui == nullptr ? 0 : ui->property("tab_count").toInt();
            for (int index = 0; index < tabCount && !captureTabId.isEmpty(); ++index) {
                QString candidateTabId;
                if (QMetaObject::invokeMethod(ui, "tab_id_for_index",
                                              Q_RETURN_ARG(QString, candidateTabId),
                                              Q_ARG(int, index))
                    && candidateTabId == captureTabId) {
                    captureTabIndex = index;
                    break;
                }
            }
            state.insert("capture_tab_index", captureTabIndex);
        }
        QVariant activeView;
        if (QMetaObject::invokeMethod(root, "activeWebView", Q_RETURN_ARG(QVariant, activeView))) {
            QObject *view = activeView.value<QObject *>();
            state.insert("view_ready", view != nullptr);
            if (view != nullptr) {
                state.insert("view_url", view->property("url").toUrl().toString());
                state.insert("view_title", view->property("title").toString());
                state.insert("form_navigation_pending",
                             view->property("versioningFormNavigationPending").toBool());
                state.insert("url_binding_suspended",
                             view->property("versioningUrlBindingSuspended").toBool());
            }
        }
    }
    if (ui != nullptr) {
        state.insert("profile_bootstrap_pending", ui->property("profile_bootstrap_pending").toBool());
        state.insert("load_state", ui->property("load_state").toString());
        state.insert("requested_url", ui->property("initial_url").toString());
        state.insert("current_url", ui->property("current_url").toString());
        const int tabCount = ui->property("tab_count").toInt();
        state.insert("tab_count", tabCount);
        state.insert("active_tab_index", ui->property("active_tab_index").toInt());
        QJsonArray lazyRestorePages;
        const int lazyRestoreProbeCount = tabCount < 128 ? tabCount : 128;
        for (int index = 0; index < lazyRestoreProbeCount; ++index) {
            bool lazy = false;
            if (!QMetaObject::invokeMethod(ui, "has_lazy_restore_page",
                                          Q_RETURN_ARG(bool, lazy), Q_ARG(int, index))) {
                break;
            }
            lazyRestorePages.append(lazy);
        }
        state.insert("lazy_restore_pages", lazyRestorePages);
        state.insert("lazy_restore_pages_truncated", lazyRestoreProbeCount < tabCount);
        state.insert("view_alive", ui->property("view_alive").toBool());
        state.insert("status", ui->property("status_text").toString());
        bool captureRequestActive = false;
        if (QMetaObject::invokeMethod(ui, "has_native_page_capture_request",
                                      Q_RETURN_ARG(bool, captureRequestActive)))
            state.insert("capture_request_active", captureRequestActive);
        bool pendingVersioningWork = false;
        if (QMetaObject::invokeMethod(ui, "has_pending_versioning_object_work",
                                      Q_RETURN_ARG(bool, pendingVersioningWork)))
            state.insert("pending_versioning_object_work", pendingVersioningWork);
    }
    return QString::fromUtf8(QJsonDocument(state).toJson(QJsonDocument::Compact));
}

QString ferric_browser_test_page_capture_public_projections(QQmlApplicationEngine &engine)
{
    QObject *ui = archive_test_ui(archive_test_root(engine));
    QString diagnostics;
    QString commandHistory;
    const QString emptyDraft;
    if (ui == nullptr
        || !QMetaObject::invokeMethod(ui, "diagnostics_json",
                                      Q_RETURN_ARG(QString, diagnostics))
        || !QMetaObject::invokeMethod(ui, "command_history_previous",
                                      Q_RETURN_ARG(QString, commandHistory),
                                      Q_ARG(QString, emptyDraft)))
        return {};
    QJsonObject result;
    result.insert("diagnostics", diagnostics);
    result.insert("command_history_previous", commandHistory);
    return QString::fromUtf8(QJsonDocument(result).toJson(QJsonDocument::Compact));
}

bool ferric_browser_test_load_viewless_download_extension(
    QQmlApplicationEngine &engine, const QString &extensionPath)
{
#if QT_CONFIG(webengine_extensions)
    QObject *root = archive_test_root(engine);
    auto *profile = root == nullptr
        ? nullptr
        : qobject_cast<QQuickWebEngineProfile *>(
              root->property("browserProfile").value<QObject *>());
    auto *manager = profile == nullptr ? nullptr : profile->extensionManager();
    if (manager == nullptr || extensionPath.isEmpty())
        return false;

    bool loadFinished = false;
    bool loadSucceeded = false;
    const auto loadConnection = QObject::connect(
        manager,
        &QWebEngineExtensionManager::loadFinished,
        root,
        [&](const QWebEngineExtensionInfo &extension) {
            if (extension.name() == u"Ferric Viewless Download Probe") {
                loadFinished = true;
                loadSucceeded = extension.isLoaded();
            }
        });
    manager->loadExtension(extensionPath);
    QElapsedTimer elapsed;
    elapsed.start();
    while (!loadFinished && elapsed.elapsed() < 15000) {
        QCoreApplication::processEvents();
        QThread::msleep(10);
    }
    QObject::disconnect(loadConnection);
    if (!loadFinished || !loadSucceeded)
        return false;

    QWebEngineExtensionInfo probeExtension;
    for (const auto &extension : manager->extensions()) {
        if (extension.name() == u"Ferric Viewless Download Probe")
            probeExtension = extension;
    }
    if (!probeExtension.isLoaded() || probeExtension.id().isEmpty())
        return false;
    manager->setExtensionEnabled(probeExtension, true);

    elapsed.restart();
    while (elapsed.elapsed() < 15000) {
        QCoreApplication::processEvents();
        for (const auto &extension : manager->extensions()) {
            if (extension.id() == probeExtension.id())
                return extension.isEnabled();
        }
        QThread::msleep(10);
    }
    return false;
#else
    Q_UNUSED(engine);
    Q_UNUSED(extensionPath);
    return false;
#endif
}

bool ferric_browser_test_observe_viewless_download(
    QQmlApplicationEngine &engine, const QString &downloadUrl)
{
#if QT_CONFIG(webengine_extensions)
    QObject *root = archive_test_root(engine);
    auto *profile = root == nullptr
        ? nullptr
        : qobject_cast<QQuickWebEngineProfile *>(
              root->property("browserProfile").value<QObject *>());
    if (root == nullptr || profile == nullptr || downloadUrl.isEmpty())
        return false;

    root->setProperty("ferricTestViewlessDownloadCount", 0);
    root->setProperty("ferricTestViewlessDownloadNoPage", false);
    QObject::connect(
        profile,
        &QQuickWebEngineProfile::downloadRequested,
        root,
        [root, downloadUrl](QQuickWebEngineDownloadRequest *download) {
            if (download == nullptr || download->url().toString() != downloadUrl
                || download->isSavePageDownload())
                return;
            const int count = root->property("ferricTestViewlessDownloadCount").toInt();
            root->setProperty("ferricTestViewlessDownloadCount", count + 1);
            root->setProperty("ferricTestViewlessDownloadNoPage", download->page() == nullptr);
        });
    return true;
#else
    Q_UNUSED(engine);
    Q_UNUSED(downloadUrl);
    return false;
#endif
}

bool ferric_browser_test_viewless_download_was_observed(QQmlApplicationEngine &engine)
{
#if QT_CONFIG(webengine_extensions)
    QObject *root = archive_test_root(engine);
    return root != nullptr
        && root->property("ferricTestViewlessDownloadCount").toInt() == 1
        && root->property("ferricTestViewlessDownloadNoPage").toBool();
#else
    Q_UNUSED(engine);
    return false;
#endif
}

bool ferric_browser_test_open_viewless_download_extension_popup(
    QQmlApplicationEngine &engine, const QString &downloadUrl)
{
#if QT_CONFIG(webengine_extensions)
    QObject *root = archive_test_root(engine);
    auto *profile = root == nullptr
        ? nullptr
        : qobject_cast<QQuickWebEngineProfile *>(
              root->property("browserProfile").value<QObject *>());
    auto *manager = profile == nullptr ? nullptr : profile->extensionManager();
    if (root == nullptr || manager == nullptr || downloadUrl.isEmpty())
        return false;

    QWebEngineExtensionInfo probeExtension;
    for (const auto &extension : manager->extensions()) {
        if (extension.name() == u"Ferric Viewless Download Probe"
            && extension.isLoaded() && extension.isEnabled()) {
            probeExtension = extension;
        }
    }
    QUrl popupUrl = probeExtension.actionPopupUrl();
    if (!popupUrl.isValid() || popupUrl.scheme() != u"chrome-extension")
        return false;
    QUrlQuery query;
    query.addQueryItem(QStringLiteral("downloadUrl"), downloadUrl);
    popupUrl.setQuery(query);

    QVariant activeView;
    if (!QMetaObject::invokeMethod(
            root, "activeWebView", Q_RETURN_ARG(QVariant, activeView))) {
        return false;
    }
    QObject *view = activeView.value<QObject *>();
    return view != nullptr && view->setProperty("url", popupUrl);
#else
    Q_UNUSED(engine);
    Q_UNUSED(downloadUrl);
    return false;
#endif
}

bool ferric_browser_test_cancel_page_capture_download(
    QQmlApplicationEngine &engine, const QString &operation)
{
    QObject *root = archive_test_root(engine);
    if (root == nullptr)
        return false;
    const QVariant captureValue = root->property("nativePageCaptureSave");
    if (captureValue.metaType() != QMetaType::fromType<QJSValue>())
        return false;
    const QJSValue capture = captureValue.value<QJSValue>();
    if (capture.property("operationId").toString() != operation
        || !capture.property("sidecarAccepted").toBool()
        || capture.property("downloadFinished").toBool())
        return false;
    const QString id = capture.property("downloadId").toString();
    const QString path = capture.property("path").toString();
    QObject *download = capture.property("download").toQObject();
    if (id.isEmpty() || path.isEmpty() || download == nullptr
        || download->property("id").toString() != id
        || !download->property("isSavePageDownload").toBool()
        || download->property("downloadDirectory").toString() + QStringLiteral("/")
            + download->property("downloadFileName").toString() != path)
        return false;
    return QMetaObject::invokeMethod(download, "cancel");
}

bool ferric_browser_test_expire_page_capture_save(
    QQmlApplicationEngine &engine, const QString &operation)
{
    QObject *root = archive_test_root(engine);
    if (root == nullptr)
        return false;
    const QVariant captureValue = root->property("nativePageCaptureSave");
    if (captureValue.metaType() != QMetaType::fromType<QJSValue>())
        return false;
    const QJSValue capture = captureValue.value<QJSValue>();
    if (capture.property("operationId").toString() != operation
        || capture.property("path").toString().isEmpty()
        || capture.property("downloadFinished").toBool())
        return false;
    QVariant expired;
    return QMetaObject::invokeMethod(root, "expireNativePageCaptureSave",
                                     Q_RETURN_ARG(QVariant, expired)) && expired.toBool();
}

bool ferric_browser_test_close_first_page_capture_tab(
    QQmlApplicationEngine &engine, const QString &operation)
{
    QObject *root = archive_test_root(engine);
    QObject *ui = archive_test_ui(root);
    if (root == nullptr || ui == nullptr)
        return false;
    const QVariant captureValue = root->property("nativePageCaptureSave");
    if (captureValue.metaType() != QMetaType::fromType<QJSValue>())
        return false;
    const QJSValue capture = captureValue.value<QJSValue>();
    QString firstTab;
    if (capture.property("operationId").toString() != operation
        || capture.property("path").toString().isEmpty()
        || capture.property("downloadFinished").toBool()
        || !QMetaObject::invokeMethod(ui, "tab_id_for_index",
                                      Q_RETURN_ARG(QString, firstTab), Q_ARG(int, 0))
        || capture.property("tabId").toString() != firstTab)
        return false;
    QVariant closed;
    return QMetaObject::invokeMethod(root, "closeTabAtIndex",
                                     Q_RETURN_ARG(QVariant, closed),
                                     Q_ARG(QVariant, QVariant(0))) && closed.toBool();
}

bool ferric_browser_test_close_second_page_capture_tab(
    QQmlApplicationEngine &engine, const QString &operation)
{
    QObject *root = archive_test_root(engine);
    QObject *ui = archive_test_ui(root);
    if (root == nullptr || ui == nullptr || ui->property("tab_count").toInt() != 2)
        return false;
    const QVariant captureValue = root->property("nativePageCaptureSave");
    if (captureValue.metaType() != QMetaType::fromType<QJSValue>())
        return false;
    const QJSValue capture = captureValue.value<QJSValue>();
    QString secondTab;
    if (capture.property("operationId").toString() != operation
        || capture.property("path").toString().isEmpty()
        || capture.property("downloadFinished").toBool()
        || !QMetaObject::invokeMethod(ui, "tab_id_for_index",
                                      Q_RETURN_ARG(QString, secondTab), Q_ARG(int, 1))
        || secondTab.isEmpty() || capture.property("tabId").toString() != secondTab)
        return false;
    QVariant closed;
    return QMetaObject::invokeMethod(root, "closeTabAtIndex",
                                     Q_RETURN_ARG(QVariant, closed),
                                     Q_ARG(QVariant, QVariant(1))) && closed.toBool();
}

bool ferric_browser_test_close_completed_page_capture_tab(
    QQmlApplicationEngine &engine, const QString &operation)
{
    QObject *root = archive_test_root(engine);
    QObject *ui = archive_test_ui(root);
    if (root == nullptr || ui == nullptr)
        return false;
    const QVariant captureValue = root->property("nativePageCaptureSave");
    if (captureValue.metaType() != QMetaType::fromType<QJSValue>())
        return false;
    const QJSValue capture = captureValue.value<QJSValue>();
    QString firstTab;
    if (capture.property("operationId").toString() != operation
        || capture.property("path").toString().isEmpty()
        || capture.property("downloadFinished").toBool()
        || !QMetaObject::invokeMethod(ui, "tab_id_for_index",
                                      Q_RETURN_ARG(QString, firstTab), Q_ARG(int, 0))
        || capture.property("tabId").toString() == firstTab)
        return false;
    QVariant closed;
    return QMetaObject::invokeMethod(root, "closeTabAtIndex",
                                     Q_RETURN_ARG(QVariant, closed),
                                     Q_ARG(QVariant, QVariant(0))) && closed.toBool();
}

bool ferric_browser_test_close_during_capture_install_is_deferred(
    QQmlApplicationEngine &engine, const QString &operation)
{
    QObject *root = archive_test_root(engine);
    QObject *ui = archive_test_ui(root);
    if (root == nullptr || ui == nullptr || ui->property("tab_count").toInt() != 2)
        return false;
    const QVariant captureValue = root->property("nativePageCaptureSave");
    if (captureValue.metaType() != QMetaType::fromType<QJSValue>())
        return false;
    const QJSValue capture = captureValue.value<QJSValue>();
    QString firstTab;
    if (capture.property("operationId").toString() != operation
        || capture.property("path").toString().isEmpty()
        || !capture.property("downloadFinished").toBool()
        || !QMetaObject::invokeMethod(ui, "tab_id_for_index",
                                      Q_RETURN_ARG(QString, firstTab), Q_ARG(int, 0))
        || capture.property("tabId").toString() != firstTab)
        return false;
    QVariant closed;
    if (!QMetaObject::invokeMethod(root, "closeTabAtIndex",
                                   Q_RETURN_ARG(QVariant, closed),
                                   Q_ARG(QVariant, QVariant(0))) || closed.toBool())
        return false;
    const QVariant retainedCapture = root->property("nativePageCaptureSave");
    if (retainedCapture.metaType() != QMetaType::fromType<QJSValue>()
        || retainedCapture.value<QJSValue>().property("operationId").toString() != operation
        || retainedCapture.value<QJSValue>().property("path").toString()
            != capture.property("path").toString()
        || ui->property("tab_count").toInt() != 2)
        return false;
    return ui->property("status_text").toString()
        == QStringLiteral("Tab close is waiting for native archive callback cleanup");
}

bool ferric_browser_test_delay_page_capture_renderer(
    QQmlApplicationEngine &engine, const QString &expectedUrl)
{
    QObject *root = archive_test_root(engine);
    QVariant activeView;
    if (root == nullptr
        || !QMetaObject::invokeMethod(root, "activeWebView", Q_RETURN_ARG(QVariant, activeView)))
        return false;
    QObject *view = activeView.value<QObject *>();
    if (view == nullptr || view->property("url").toUrl().toString() != expectedUrl)
        return false;
    // Queue a bounded renderer task ahead of the collector's separate
    // ApplicationWorld request. The GUI-side collection deadline remains live.
    const QString script = QStringLiteral(
        "(() => { const started = performance.now(); "
        "while (performance.now() - started < 3200) {} return true; })()");
    return QMetaObject::invokeMethod(view, "runJavaScript",
                                     Q_ARG(QString, script), Q_ARG(QJSValue, QJSValue()));
}

bool ferric_browser_test_submit_page_capture_form(
    QQmlApplicationEngine &engine, const QString &expectedUrl)
{
    QObject *root = archive_test_root(engine);
    QVariant activeView;
    if (root == nullptr
        || !QMetaObject::invokeMethod(root, "activeWebView", Q_RETURN_ARG(QVariant, activeView)))
        return false;
    return archive_test_submit_form(activeView.value<QObject *>(), expectedUrl);
}

bool ferric_browser_test_prepare_page_capture_geometry(
    QQmlApplicationEngine &engine, const QString &expectedUrl)
{
    QObject *root = archive_test_root(engine);
    QVariant activeView;
    if (root == nullptr
        || !expectedUrl.contains(QStringLiteral("/versioning/document"))
        || !QMetaObject::invokeMethod(root, "activeWebView", Q_RETURN_ARG(QVariant, activeView)))
        return false;
    QObject *view = activeView.value<QObject *>();
    if (view == nullptr || view->property("url").toUrl().toString() != expectedUrl)
        return false;
    const QString script = QStringLiteral(
        "(() => { const outer=document.getElementById('outer'); "
        "const inner=document.getElementById('inner'); "
        "if (!outer || !inner) return false; "
        "outer.scrollTop=55; inner.scrollTop=75; "
        "document.title='Versioning geometry prepared'; return true; })()");
    return QMetaObject::invokeMethod(view, "runJavaScript",
                                     Q_ARG(QString, script), Q_ARG(QJSValue, QJSValue()));
}

bool ferric_browser_test_prepare_page_capture_selection(
    QQmlApplicationEngine &engine, const QString &expectedUrl)
{
    QObject *root = archive_test_root(engine);
    QVariant activeView;
    if (root == nullptr
        || !expectedUrl.contains(QStringLiteral("/versioning/editor"))
        || !QMetaObject::invokeMethod(root, "activeWebView", Q_RETURN_ARG(QVariant, activeView)))
        return false;
    QObject *view = activeView.value<QObject *>();
    if (view == nullptr || view->property("url").toUrl().toString() != expectedUrl)
        return false;
    const QString script = QStringLiteral(
        "(() => { const editor=document.getElementById('draft'); "
        "if (!editor || !editor.firstChild || editor.firstChild.length < 12) return false; "
        "editor.focus(); const range=document.createRange(); "
        "range.setStart(editor.firstChild,3); range.setEnd(editor.firstChild,12); "
        "const selection=window.getSelection(); selection.removeAllRanges(); "
        "selection.addRange(range); "
        "document.title='Versioning selection prepared'; return true; })()");
    return QMetaObject::invokeMethod(view, "runJavaScript",
                                     Q_ARG(QString, script), Q_ARG(QJSValue, QJSValue()));
}

bool ferric_browser_test_prepare_page_capture_refill_draft(
    QQmlApplicationEngine &engine, const QString &expectedUrl)
{
    QObject *root = archive_test_root(engine);
    QVariant activeView;
    if (root == nullptr
        || !expectedUrl.contains(QStringLiteral("/versioning/editor-refill"))
        || !QMetaObject::invokeMethod(root, "activeWebView", Q_RETURN_ARG(QVariant, activeView)))
        return false;
    QObject *view = activeView.value<QObject *>();
    if (view == nullptr || view->property("url").toUrl().toString() != expectedUrl)
        return false;
    const QString script = QStringLiteral(
        "(() => { const input=document.getElementById('refill-draft'); "
        "if (!input || input.value !== '') return false; "
        "input.value='synthetic refill draft'; "
        "document.title='Versioning refill prepared'; return true; })()");
    return QMetaObject::invokeMethod(view, "runJavaScript",
                                     Q_ARG(QString, script), Q_ARG(QJSValue, QJSValue()));
}

bool ferric_browser_test_prepare_page_capture_media_position(
    QQmlApplicationEngine &engine, const QString &expectedUrl)
{
    QObject *root = archive_test_root(engine);
    QVariant activeView;
    if (root == nullptr
        || !expectedUrl.contains(QStringLiteral("/versioning/media"))
        || !QMetaObject::invokeMethod(root, "activeWebView", Q_RETURN_ARG(QVariant, activeView)))
        return false;
    QObject *view = activeView.value<QObject *>();
    if (view == nullptr || view->property("url").toUrl().toString() != expectedUrl)
        return false;
    const QString script = QStringLiteral(
        "(() => { const media=document.getElementById('sample'); "
        "if (!media) { document.title='Versioning media missing'; return false; } "
        "if (!Number.isFinite(media.duration)) { "
        "document.title='Versioning media duration unavailable'; return false; } "
        "if (media.duration<0.8) { "
        "document.title='Versioning media too short'; return false; } "
        "if (!media.paused) { document.title='Versioning media playing'; return false; } "
        "media.currentTime=0.4; "
        "if (Math.abs(media.currentTime-0.4)>=0.1) { "
        "document.title='Versioning media seek refused'; return false; } "
        "if (window.mediaActionCounts.play!==0 || window.mediaActionCounts.pause!==0) { "
        "document.title='Versioning media action observed'; return false; } "
        "document.title='Versioning media prepared'; return true; })()");
    return QMetaObject::invokeMethod(view, "runJavaScript",
                                     Q_ARG(QString, script), Q_ARG(QJSValue, QJSValue()));
}

bool ferric_browser_test_remove_page_capture_frame(
    QQmlApplicationEngine &engine, const QString &expectedUrl)
{
    QObject *root = archive_test_root(engine);
    QVariant activeView;
    if (root == nullptr
        || !expectedUrl.contains(QStringLiteral("/versioning/frames"))
        || !QMetaObject::invokeMethod(root, "activeWebView", Q_RETURN_ARG(QVariant, activeView)))
        return false;
    QObject *view = activeView.value<QObject *>();
    if (view == nullptr || view->property("url").toUrl().toString() != expectedUrl)
        return false;
    const QString script = QStringLiteral(
        "(() => { const frame=document.getElementById('same'); "
        "if (!frame) return false; frame.remove(); "
        "document.title='Versioning frame removed'; return true; })()");
    return QMetaObject::invokeMethod(view, "runJavaScript",
                                     Q_ARG(QString, script), Q_ARG(QJSValue, QJSValue()));
}

bool ferric_browser_test_reload_page_capture_frame(
    QQmlApplicationEngine &engine, const QString &expectedUrl)
{
    QObject *root = archive_test_root(engine);
    QVariant activeView;
    if (root == nullptr
        || !expectedUrl.contains(QStringLiteral("/versioning/frames"))
        || !QMetaObject::invokeMethod(root, "activeWebView", Q_RETURN_ARG(QVariant, activeView)))
        return false;
    QObject *view = activeView.value<QObject *>();
    if (view == nullptr || view->property("url").toUrl().toString() != expectedUrl)
        return false;
    const QString script = QStringLiteral(
        "(() => { const frame=document.getElementById('same'); "
        "if (!frame) return false; "
        "frame.addEventListener('load', () => { "
        "document.title='Versioning frame reloaded'; }, {once:true}); "
        "frame.src=frame.src; return true; })()");
    return QMetaObject::invokeMethod(view, "runJavaScript",
                                     Q_ARG(QString, script), Q_ARG(QJSValue, QJSValue()));
}

bool ferric_browser_test_probe_restored_page_geometry(
    QQmlApplicationEngine &engine, const QString &expectedUrl)
{
    QObject *root = archive_test_root(engine);
    QVariant activeView;
    if (root == nullptr
        || !expectedUrl.contains(QStringLiteral("/versioning/document"))
        || !QMetaObject::invokeMethod(root, "activeWebView", Q_RETURN_ARG(QVariant, activeView)))
        return false;
    QObject *view = activeView.value<QObject *>();
    if (view == nullptr || view->property("url").toUrl().toString() != expectedUrl)
        return false;
    const QString script = QStringLiteral(
        "(() => { const outer=document.getElementById('outer'); "
        "const inner=document.getElementById('inner'); "
        "if (!outer || !inner) return false; "
        "if (Math.abs(outer.scrollTop-55)<2 && Math.abs(inner.scrollTop-75)<2) "
        "document.title='Versioning geometry restored'; return true; })()");
    return QMetaObject::invokeMethod(view, "runJavaScript",
                                     Q_ARG(QString, script), Q_ARG(QJSValue, QJSValue()));
}

bool ferric_browser_test_confirm_restore_editable_review(QQmlApplicationEngine &engine)
{
    QObject *root = archive_test_root(engine);
    if (root == nullptr || !root->property("versioningRestoreEditableDialogVisible").toBool())
        return false;
    QObject *host = root->property("versioningRestoreCoordinatorObject").value<QObject *>();
    QObject *coordinator = host == nullptr ? nullptr : host->property("item").value<QObject *>();
    return coordinator != nullptr
        && coordinator->property("previewRefillableCount").toInt() == 1
        && QMetaObject::invokeMethod(coordinator, "finishEditableReview",
                                     Q_ARG(QVariant, QVariant(true)));
}

bool ferric_browser_test_probe_blank_restore_refill_draft(
    QQmlApplicationEngine &engine, const QString &expectedUrl)
{
    QObject *root = archive_test_root(engine);
    QVariant activeView;
    if (root == nullptr
        || !expectedUrl.contains(QStringLiteral("/versioning/editor-refill"))
        || !QMetaObject::invokeMethod(root, "activeWebView", Q_RETURN_ARG(QVariant, activeView)))
        return false;
    QObject *view = activeView.value<QObject *>();
    if (view == nullptr || view->property("url").toUrl().toString() != expectedUrl)
        return false;
    const QString script = QStringLiteral(
        "(() => { const input=document.getElementById('refill-draft'); "
        "if (input && input.value==='') "
        "document.title='Versioning refill awaiting review'; return true; })()");
    return QMetaObject::invokeMethod(view, "runJavaScript",
                                     Q_ARG(QString, script), Q_ARG(QJSValue, QJSValue()));
}

bool ferric_browser_test_probe_restored_refill_draft(
    QQmlApplicationEngine &engine, const QString &expectedUrl)
{
    QObject *root = archive_test_root(engine);
    QVariant activeView;
    if (root == nullptr
        || !expectedUrl.contains(QStringLiteral("/versioning/editor-refill"))
        || !QMetaObject::invokeMethod(root, "activeWebView", Q_RETURN_ARG(QVariant, activeView)))
        return false;
    QObject *view = activeView.value<QObject *>();
    if (view == nullptr || view->property("url").toUrl().toString() != expectedUrl)
        return false;
    const QString script = QStringLiteral(
        "(() => { const input=document.getElementById('refill-draft'); "
        "const events=window.refillDraftEvents; "
        "if (!input || !events) return false; "
        "if (input.value==='synthetic refill draft' "
        "&& events.input===0 && events.change===0 "
        "&& events.focus===0 && events.blur===0 "
        "&& window.refillAutosaveAttempted===false) "
        "document.title='Versioning refill restored'; return true; })()");
    return QMetaObject::invokeMethod(view, "runJavaScript",
                                     Q_ARG(QString, script), Q_ARG(QJSValue, QJSValue()));
}

bool ferric_browser_test_probe_restored_media_position(
    QQmlApplicationEngine &engine, const QString &expectedUrl)
{
    QObject *root = archive_test_root(engine);
    QVariant activeView;
    if (root == nullptr
        || !expectedUrl.contains(QStringLiteral("/versioning/media"))
        || !QMetaObject::invokeMethod(root, "activeWebView", Q_RETURN_ARG(QVariant, activeView)))
        return false;
    QObject *view = activeView.value<QObject *>();
    if (view == nullptr || view->property("url").toUrl().toString() != expectedUrl)
        return false;
    const QString script = QStringLiteral(
        "(() => { const media=document.getElementById('sample'); "
        "const counts=window.mediaActionCounts; "
        "if (media && counts && Number.isFinite(media.duration) "
        "&& Math.abs(media.currentTime-0.4)<0.1 && media.paused "
        "&& counts.play===0 && counts.pause===0) "
        "document.title='Versioning media restored'; return true; })()");
    return QMetaObject::invokeMethod(view, "runJavaScript",
                                     Q_ARG(QString, script), Q_ARG(QJSValue, QJSValue()));
}


bool ferric_browser_test_open_secondary_page_capture_form(
    QQmlApplicationEngine &engine, const QString &startupUrl)
{
    QObject *root = archive_test_root(engine);
    QObject *ui = archive_test_ui(root);
    if (root == nullptr || ui == nullptr
        || archive_test_secondary_window(engine, startupUrl) != nullptr)
        return false;
    const QVariant action(QStringLiteral("reopen\t") + startupUrl);
    return QMetaObject::invokeMethod(
        root, "openReopenedWindow",
        Q_ARG(QVariant, QVariant::fromValue(ui)),
        Q_ARG(QVariant, QVariant::fromValue(root)),
        Q_ARG(QVariant, action));
}

QString ferric_browser_test_secondary_page_capture_state(
    QQmlApplicationEngine &engine, const QString &startupUrl)
{
    QObject *window = archive_test_secondary_window(engine, startupUrl);
    QObject *view = archive_test_secondary_view(window);
    QObject *ui = view == nullptr ? nullptr : view->property("viewUi").value<QObject *>();
    QJsonObject state;
    state.insert("window_ready", window != nullptr);
    state.insert("window_visible", window != nullptr && window->property("visible").toBool());
    state.insert("shutdown_approved",
                 window != nullptr && window->property("windowShutdownApproved").toBool());
    if (window != nullptr) {
        state.insert("page_probe_generation",
                     window->property("windowShutdownPageProbeGeneration").toInt());
        state.insert("page_probe_snapshot_active",
                     !window->property("windowShutdownPagePromptSnapshot").isNull());
        state.insert("close_confirmation_visible",
                     window->property("windowCloseConfirmationVisible").toBool());
        state.insert("shutdown_prompt_visible",
                     window->property("windowShutdownPromptVisible").toBool());
        state.insert("page_prompt_visible",
                     window->property("windowShutdownPagePromptVisible").toBool());
        state.insert("storage_prompt_visible",
                     window->property("windowShutdownStoragePromptVisible").toBool());
    }
    state.insert("view_ready", view != nullptr);
    state.insert("ui_ready", ui != nullptr);
    if (view != nullptr) {
        state.insert("view_url", view->property("url").toUrl().toString());
        state.insert("form_navigation_pending",
                     view->property("versioningFormNavigationPending").toBool());
        state.insert("url_binding_suspended",
                     view->property("versioningUrlBindingSuspended").toBool());
    }
    if (ui != nullptr) {
        state.insert("load_state", ui->property("load_state").toString());
        state.insert("current_url", ui->property("current_url").toString());
        state.insert("requested_url", ui->property("initial_url").toString());
        state.insert("status", ui->property("status_text").toString());
        state.insert("profile_bootstrap_pending",
                     ui->property("profile_bootstrap_pending").toBool());
    }
    return QString::fromUtf8(QJsonDocument(state).toJson(QJsonDocument::Compact));
}

bool ferric_browser_test_submit_secondary_page_capture_form(
    QQmlApplicationEngine &engine, const QString &startupUrl)
{
    return archive_test_submit_form(
        archive_test_secondary_view(archive_test_secondary_window(engine, startupUrl)),
        startupUrl);
}

bool ferric_browser_test_navigate_secondary_page_capture(
    QQmlApplicationEngine &engine, const QString &startupUrl, const QString &url)
{
    QObject *view = archive_test_secondary_view(
        archive_test_secondary_window(engine, startupUrl));
    QObject *ui = view == nullptr ? nullptr : view->property("viewUi").value<QObject *>();
    return ui != nullptr && QMetaObject::invokeMethod(ui, "navigate", Q_ARG(QString, url));
}

bool ferric_browser_test_request_secondary_page_capture_close(
    QQmlApplicationEngine &engine, const QString &startupUrl)
{
    auto *window = qobject_cast<QWindow *>(archive_test_secondary_window(engine, startupUrl));
    if (window == nullptr)
        return false;
    window->close();
    return true;
}

bool ferric_browser_test_continue_secondary_page_capture_close(
    QQmlApplicationEngine &engine, const QString &startupUrl)
{
    QObject *window = archive_test_secondary_window(engine, startupUrl);
    return window != nullptr && QMetaObject::invokeMethod(window, "continueQuitRequest");
}

bool ferric_browser_test_confirm_secondary_page_capture_close(
    QQmlApplicationEngine &engine, const QString &startupUrl)
{
    QObject *window = archive_test_secondary_window(engine, startupUrl);
    return window != nullptr && QMetaObject::invokeMethod(window, "confirmShutdownPagePrompt");
}

bool ferric_browser_test_retry_secondary_page_capture_close(
    QQmlApplicationEngine &engine, const QString &startupUrl)
{
    QObject *window = archive_test_secondary_window(engine, startupUrl);
    if (window == nullptr || !window->property("windowShutdownStoragePromptVisible").toBool())
        return false;
    window->setProperty("windowShutdownStoragePromptVisible", false);
    return QMetaObject::invokeMethod(window, "finalizeQuit");
}

bool ferric_browser_test_navigate_page_capture_source(
    QQmlApplicationEngine &engine, const QString &url)
{
    QObject *root = archive_test_root(engine);
    QObject *ui = archive_test_ui(root);
    // Mirror the ordinary QML navigation handoff: the reducer accepts the
    // URL, then the active tab model applies it to the live WebEngineView.
    return ui != nullptr && root != nullptr
        && QMetaObject::invokeMethod(ui, "navigate", Q_ARG(QString, url))
        && QMetaObject::invokeMethod(root, "updateActiveTabUrl");
}

bool ferric_browser_test_navigate_first_page_capture_tab(
    QQmlApplicationEngine &engine, const QString &url)
{
    QObject *root = archive_test_root(engine);
    QObject *ui = archive_test_ui(root);
    bool selected = false;
    return ui != nullptr && root != nullptr
        && QMetaObject::invokeMethod(ui, "select_tab", Q_RETURN_ARG(bool, selected), Q_ARG(int, 0))
        && selected
        && QMetaObject::invokeMethod(root, "syncTabModel")
        && QMetaObject::invokeMethod(ui, "navigate", Q_ARG(QString, url))
        && QMetaObject::invokeMethod(root, "updateActiveTabUrl");
}

bool ferric_browser_test_navigate_second_page_capture_tab(
    QQmlApplicationEngine &engine, const QString &url)
{
    QObject *root = archive_test_root(engine);
    QObject *ui = archive_test_ui(root);
    bool selected = false;
    return ui != nullptr && root != nullptr
        && QMetaObject::invokeMethod(ui, "select_tab", Q_RETURN_ARG(bool, selected), Q_ARG(int, 1))
        && selected
        && QMetaObject::invokeMethod(root, "syncTabModel")
        && QMetaObject::invokeMethod(ui, "navigate", Q_ARG(QString, url))
        && QMetaObject::invokeMethod(root, "updateActiveTabUrl");
}

bool ferric_browser_test_open_second_page_capture_tab(
    QQmlApplicationEngine &engine, const QString &url)
{
    QObject *root = archive_test_root(engine);
    QObject *ui = archive_test_ui(root);
    int index = -1;
    return ui != nullptr && root != nullptr
        && QMetaObject::invokeMethod(ui, "new_tab", Q_RETURN_ARG(int, index))
        && index == 1
        && QMetaObject::invokeMethod(root, "syncTabModel")
        && QMetaObject::invokeMethod(ui, "navigate", Q_ARG(QString, url))
        && QMetaObject::invokeMethod(root, "updateActiveTabUrl");
}

bool ferric_browser_test_select_page_capture_tab(
    QQmlApplicationEngine &engine, int index)
{
    QObject *root = archive_test_root(engine);
    QObject *ui = archive_test_ui(root);
    bool selected = false;
    return ui != nullptr && root != nullptr
        && QMetaObject::invokeMethod(ui, "select_tab", Q_RETURN_ARG(bool, selected),
                                     Q_ARG(int, index))
        && selected
        && QMetaObject::invokeMethod(root, "syncTabModel")
        && QMetaObject::invokeMethod(root, "executePendingEngineAction")
        && QMetaObject::invokeMethod(root, "updateActiveTabUrl");
}

bool ferric_browser_test_request_page_capture(QQmlApplicationEngine &engine,
                                              bool include_editable,
                                              bool include_selection)
{
    QObject *ui = archive_test_ui(archive_test_root(engine));
    bool accepted = false;
    return ui != nullptr
        && QMetaObject::invokeMethod(ui, "request_native_page_capture_consent",
                                     Q_RETURN_ARG(bool, accepted), Q_ARG(bool, include_editable),
                                     Q_ARG(bool, include_selection))
        && accepted;
}

bool ferric_browser_test_request_window_page_capture(QQmlApplicationEngine &engine,
                                                     bool include_editable,
                                                     bool include_selection,
                                                     bool allow_partial_pages)
{
    QObject *ui = archive_test_ui(archive_test_root(engine));
    bool accepted = false;
    return ui != nullptr
        && QMetaObject::invokeMethod(ui, "request_native_window_page_capture_consent",
                                     Q_RETURN_ARG(bool, accepted), Q_ARG(bool, include_editable),
                                     Q_ARG(bool, include_selection),
                                     Q_ARG(bool, allow_partial_pages))
        && accepted;
}

QString ferric_browser_test_page_capture_disclosure_operation(QQmlApplicationEngine &engine)
{
    QObject *ui = archive_test_ui(archive_test_root(engine));
    QStringList disclosure;
    if (ui == nullptr
        || !QMetaObject::invokeMethod(ui, "pending_native_page_capture_disclosure",
                                      Q_RETURN_ARG(QStringList, disclosure))
        || disclosure.size() != 2)
        return QString();
    return disclosure.first();
}

bool ferric_browser_test_accept_page_capture(
    QQmlApplicationEngine &engine, const QString &operation)
{
    QObject *ui = archive_test_ui(archive_test_root(engine));
    bool accepted = false;
    return ui != nullptr
        && QMetaObject::invokeMethod(ui, "accept_native_page_capture_disclosure",
                                     Q_RETURN_ARG(bool, accepted), Q_ARG(QString, operation))
        && accepted;
}

bool ferric_browser_test_decline_page_capture(
    QQmlApplicationEngine &engine, const QString &operation)
{
    QObject *ui = archive_test_ui(archive_test_root(engine));
    bool declined = false;
    return ui != nullptr
        && QMetaObject::invokeMethod(ui, "decline_native_page_capture_disclosure",
                                     Q_RETURN_ARG(bool, declined), Q_ARG(QString, operation))
        && declined;
}

bool ferric_browser_test_begin_window_shutdown(QQmlApplicationEngine &engine)
{
    QObject *root = archive_test_root(engine);
    return root != nullptr && QMetaObject::invokeMethod(root, "beginApplicationShutdown");
}

bool ferric_browser_test_cancel_quit_and_drain_archive_worker(
    QQmlApplicationEngine &engine)
{
    const auto failed = [](const char *stage) {
        std::fprintf(stderr,
                     "FERRIC_NATIVE_ARCHIVE_SHUTDOWN_PROBE_FAILED stage=%s\n",
                     stage);
        return false;
    };
    QObject *root = archive_test_root(engine);
    QObject *ui = archive_test_ui(root);
    bool pendingVersioningWork = false;
    if (root == nullptr || ui == nullptr
        || !QMetaObject::invokeMethod(ui, "has_pending_versioning_object_work",
                                      Q_RETURN_ARG(bool, pendingVersioningWork))
        || !pendingVersioningWork
        || root->property("shutdownConfirmationVisible").toBool()
        || root->property("shutdownPromptVisible").toBool())
        return failed("preconditions");
    const QString command = QStringLiteral(":quit");
    // QML JavaScript function parameters are exposed through the meta-object as
    // QVariant, even when the call-site value is a QString.
    if (!QMetaObject::invokeMethod(root, "beginQuitRequest",
                                   Q_ARG(QVariant, QVariant(command))))
        return failed("begin-invocation");
    if (!root->property("shutdownConfirmationVisible").toBool())
        return failed("confirmation-not-visible");
    if (root->property("shutdownConfirmationCommand").toString() != command)
        return failed("confirmation-command");
    if (!QMetaObject::invokeMethod(root, "continueQuitRequest"))
        return failed("continue-invocation");
    if (root->property("shutdownConfirmationVisible").toBool())
        return failed("confirmation-still-visible");
    if (!root->property("shutdownPromptVisible").toBool())
        return failed("prompt-not-visible");
    if (!QMetaObject::invokeMethod(root, "cancelDownloadsAndQuit"))
        return failed("cancel-and-quit-invocation");
    bool captureRequestActive = true;
    pendingVersioningWork = false;
    return root->property("shutdownWaitingForDownloadFinishes").toBool()
        && root->property("shutdownPromptVisible").toBool()
        && QMetaObject::invokeMethod(ui, "has_native_page_capture_request",
                                     Q_RETURN_ARG(bool, captureRequestActive))
        && !captureRequestActive
        && QMetaObject::invokeMethod(ui, "has_pending_versioning_object_work",
                                     Q_RETURN_ARG(bool, pendingVersioningWork))
        && pendingVersioningWork
        ? true
        : failed("worker-drain-wait");
}
