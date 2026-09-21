#ifndef FERRIC_BROWSER_URL_DISPLAY_H
#define FERRIC_BROWSER_URL_DISPLAY_H

#include <QtCore/QString>

QString ferric_browser_to_ascii_host(const QString &host);
QString ferric_browser_canonicalize_url(const QString &value);
QString ferric_browser_read_clipboard();
QString ferric_browser_read_primary_selection();
bool ferric_browser_primary_selection_available();
bool ferric_browser_write_clipboard(const QString &value, bool primary);
QString ferric_browser_qt_platform_name();
QString ferric_browser_qt_quick_graphics_api();
QString ferric_browser_qt_webengine_version();
QString ferric_browser_qt_chromium_version();
QString ferric_browser_qt_chromium_security_patch_version();
void ferric_browser_set_desktop_identity();
void ferric_browser_enable_software_rendering();
void ferric_browser_register_internal_scheme();

#endif
