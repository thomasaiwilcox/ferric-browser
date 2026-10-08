#ifndef FERRIC_BROWSER_QML_DIAGNOSTICS_H
#define FERRIC_BROWSER_QML_DIAGNOSTICS_H

#include <QtQml/QQmlApplicationEngine>
#include <QtCore/QUrl>
#include <QtCore/QVariantMap>
#include <QtCore/QString>

void ferric_browser_suppress_qml_warning_stderr(QQmlApplicationEngine &engine);
void ferric_browser_enable_qml_diagnostics(QQmlApplicationEngine &engine);
void ferric_browser_dump_qml_component_errors(
    QQmlApplicationEngine &engine,
    const QUrl &url,
    const QVariantMap &initialProperties);

// Internal native-test observations of the production root. These are not
// registered as QML methods or available to page content.
QString ferric_browser_test_archive_window_state(QQmlApplicationEngine &engine);
bool ferric_browser_test_request_archive_view(QQmlApplicationEngine &engine, const QString &commit);
bool ferric_browser_test_attempt_archive_profile_replacement(QQmlApplicationEngine &engine);
bool ferric_browser_test_close_archive_view(QQmlApplicationEngine &engine);
bool ferric_browser_test_begin_window_shutdown(QQmlApplicationEngine &engine);
bool ferric_browser_test_cancel_quit_and_drain_archive_worker(
    QQmlApplicationEngine &engine);
QString ferric_browser_test_page_capture_state(QQmlApplicationEngine &engine);
QString ferric_browser_test_clean_branch_state(QQmlApplicationEngine &engine);
bool ferric_browser_test_execute_root_command(
    QQmlApplicationEngine &engine, const QString &command);
bool ferric_browser_test_disable_pending_clean_branch_view_factory(
    QQmlApplicationEngine &engine);
bool ferric_browser_test_request_clean_branch_close(QQmlApplicationEngine &engine);
bool ferric_browser_test_confirm_clean_branch_close(QQmlApplicationEngine &engine);
bool ferric_browser_test_confirm_clean_branch_page_close(QQmlApplicationEngine &engine);
bool ferric_browser_test_confirm_clean_branch_popup_page_close(QQmlApplicationEngine &engine);
bool ferric_browser_test_remember_normal_notification_permission(
    QQmlApplicationEngine &engine, const QString &origin);
bool ferric_browser_test_normal_notification_allowed(
    QQmlApplicationEngine &engine, const QString &origin);
bool ferric_browser_test_deny_clean_branch_permission(QQmlApplicationEngine &engine);
bool ferric_browser_test_execute_clean_branch_command(
    QQmlApplicationEngine &engine, const QString &command);
bool ferric_browser_test_first_clean_branch_tab_close_refused(
    QQmlApplicationEngine &engine);
bool ferric_browser_test_disable_clean_branch_factory(QQmlApplicationEngine &engine);
bool ferric_browser_test_click_clean_branch_page(QQmlApplicationEngine &engine);
QString ferric_browser_test_page_capture_public_projections(QQmlApplicationEngine &engine);
bool ferric_browser_test_load_viewless_download_extension(
    QQmlApplicationEngine &engine, const QString &extensionPath);
bool ferric_browser_test_observe_viewless_download(
    QQmlApplicationEngine &engine, const QString &downloadUrl);
bool ferric_browser_test_viewless_download_was_observed(QQmlApplicationEngine &engine);
bool ferric_browser_test_open_viewless_download_extension_popup(
    QQmlApplicationEngine &engine, const QString &downloadUrl);
bool ferric_browser_test_cancel_page_capture_download(
    QQmlApplicationEngine &engine, const QString &operation);
bool ferric_browser_test_expire_page_capture_save(
    QQmlApplicationEngine &engine, const QString &operation);
bool ferric_browser_test_close_first_page_capture_tab(
    QQmlApplicationEngine &engine, const QString &operation);
bool ferric_browser_test_close_second_page_capture_tab(
    QQmlApplicationEngine &engine, const QString &operation);
bool ferric_browser_test_close_completed_page_capture_tab(
    QQmlApplicationEngine &engine, const QString &operation);
bool ferric_browser_test_close_during_capture_install_is_deferred(
    QQmlApplicationEngine &engine, const QString &operation);
bool ferric_browser_test_delay_page_capture_renderer(
    QQmlApplicationEngine &engine, const QString &expectedUrl);
bool ferric_browser_test_submit_page_capture_form(
    QQmlApplicationEngine &engine, const QString &expectedUrl);
bool ferric_browser_test_prepare_page_capture_geometry(
    QQmlApplicationEngine &engine, const QString &expectedUrl);
bool ferric_browser_test_prepare_page_capture_selection(
    QQmlApplicationEngine &engine, const QString &expectedUrl);
bool ferric_browser_test_prepare_page_capture_refill_draft(
    QQmlApplicationEngine &engine, const QString &expectedUrl);
bool ferric_browser_test_prepare_page_capture_media_position(
    QQmlApplicationEngine &engine, const QString &expectedUrl);
bool ferric_browser_test_remove_page_capture_frame(
    QQmlApplicationEngine &engine, const QString &expectedUrl);
bool ferric_browser_test_reload_page_capture_frame(
    QQmlApplicationEngine &engine, const QString &expectedUrl);
bool ferric_browser_test_probe_restored_page_geometry(
    QQmlApplicationEngine &engine, const QString &expectedUrl);
bool ferric_browser_test_confirm_restore_editable_review(QQmlApplicationEngine &engine);
bool ferric_browser_test_probe_blank_restore_refill_draft(
    QQmlApplicationEngine &engine, const QString &expectedUrl);
bool ferric_browser_test_probe_restored_refill_draft(
    QQmlApplicationEngine &engine, const QString &expectedUrl);
bool ferric_browser_test_probe_restored_media_position(
    QQmlApplicationEngine &engine, const QString &expectedUrl);
bool ferric_browser_test_open_secondary_page_capture_form(
    QQmlApplicationEngine &engine, const QString &startupUrl);
QString ferric_browser_test_secondary_page_capture_state(
    QQmlApplicationEngine &engine, const QString &startupUrl);
bool ferric_browser_test_submit_secondary_page_capture_form(
    QQmlApplicationEngine &engine, const QString &startupUrl);
bool ferric_browser_test_navigate_secondary_page_capture(
    QQmlApplicationEngine &engine, const QString &startupUrl, const QString &url);
bool ferric_browser_test_request_secondary_page_capture_close(
    QQmlApplicationEngine &engine, const QString &startupUrl);
bool ferric_browser_test_continue_secondary_page_capture_close(
    QQmlApplicationEngine &engine, const QString &startupUrl);
bool ferric_browser_test_confirm_secondary_page_capture_close(
    QQmlApplicationEngine &engine, const QString &startupUrl);
bool ferric_browser_test_retry_secondary_page_capture_close(
    QQmlApplicationEngine &engine, const QString &startupUrl);
bool ferric_browser_test_navigate_page_capture_source(
    QQmlApplicationEngine &engine, const QString &url);
bool ferric_browser_test_navigate_first_page_capture_tab(
    QQmlApplicationEngine &engine, const QString &url);
bool ferric_browser_test_navigate_second_page_capture_tab(
    QQmlApplicationEngine &engine, const QString &url);
bool ferric_browser_test_open_second_page_capture_tab(
    QQmlApplicationEngine &engine, const QString &url);
bool ferric_browser_test_select_page_capture_tab(
    QQmlApplicationEngine &engine, int index);
bool ferric_browser_test_request_page_capture(QQmlApplicationEngine &engine,
                                              bool include_editable,
                                              bool include_selection);
bool ferric_browser_test_request_window_page_capture(QQmlApplicationEngine &engine,
                                                     bool include_editable,
                                                     bool include_selection,
                                                     bool allow_partial_pages);
QString ferric_browser_test_page_capture_disclosure_operation(QQmlApplicationEngine &engine);
bool ferric_browser_test_accept_page_capture(
    QQmlApplicationEngine &engine, const QString &operation);
bool ferric_browser_test_decline_page_capture(
    QQmlApplicationEngine &engine, const QString &operation);

#endif
