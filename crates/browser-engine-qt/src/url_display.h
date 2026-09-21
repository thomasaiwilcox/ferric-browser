#ifndef RUSTBROWSER_URL_DISPLAY_H
#define RUSTBROWSER_URL_DISPLAY_H

#include <QtCore/QString>

QString rustbrowser_to_ascii_host(const QString &host);
QString rustbrowser_canonicalize_url(const QString &value);
QString rustbrowser_read_clipboard();
QString rustbrowser_read_primary_selection();
bool rustbrowser_primary_selection_available();
bool rustbrowser_write_clipboard(const QString &value, bool primary);
QString rustbrowser_qt_platform_name();
QString rustbrowser_qt_quick_graphics_api();
QString rustbrowser_qt_webengine_version();
QString rustbrowser_qt_chromium_version();
QString rustbrowser_qt_chromium_security_patch_version();
void rustbrowser_set_desktop_identity();
void rustbrowser_enable_software_rendering();
void rustbrowser_register_internal_scheme();

#endif
