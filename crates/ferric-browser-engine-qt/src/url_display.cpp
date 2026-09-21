#include "url_display.h"

#include <QtWebEngineCore/qtwebenginecoreversion.h>
#include <QtWebEngineCore/qtwebenginecoreglobal.h>
#include <QtWebEngineCore/QWebEngineUrlScheme>

#include <algorithm>
#include <QtCore/QByteArray>
#include <QtCore/QUrl>
#include <QtGui/QClipboard>
#include <QtGui/QGuiApplication>
#include <QtQuick/QQuickWindow>
#include <QtQuick/QSGRendererInterface>

QString ferric_browser_to_ascii_host(const QString &host)
{
    if (host.isEmpty() || host.contains(u':')) {
        return host;
    }
    const auto ascii = QUrl::toAce(host, QUrl::IgnoreIDNWhitelist);
    return ascii.isEmpty() ? host : QString::fromLatin1(ascii);
}

QString ferric_browser_canonicalize_url(const QString &value)
{
    QUrl url(value, QUrl::StrictMode);
    if (!url.isValid() || url.scheme().isEmpty()) {
        return {};
    }
    const auto scheme = url.scheme().toLower();
    if (scheme != QStringLiteral("http") && scheme != QStringLiteral("https")
        && scheme != QStringLiteral("file") && scheme != QStringLiteral("about")) {
        return {};
    }
    url.setScheme(scheme);
    if (scheme == QStringLiteral("http") || scheme == QStringLiteral("https")) {
        if (url.host().isEmpty()) {
            return {};
        }
        const auto defaultPort = scheme == QStringLiteral("http") ? 80 : 443;
        if (url.port() == defaultPort) {
            url.setPort(-1);
        }
    }
    return url.toString(QUrl::FullyEncoded);
}

QString ferric_browser_read_clipboard()
{
    const auto *application = qobject_cast<const QGuiApplication *>(QCoreApplication::instance());
    if (application == nullptr || application->clipboard() == nullptr) {
        return {};
    }
    return application->clipboard()->text(QClipboard::Clipboard);
}

QString ferric_browser_read_primary_selection()
{
    const auto *application = qobject_cast<const QGuiApplication *>(QCoreApplication::instance());
    if (application == nullptr || application->clipboard() == nullptr
        || !application->clipboard()->supportsSelection()) {
        return {};
    }
    return application->clipboard()->text(QClipboard::Selection);
}

bool ferric_browser_primary_selection_available()
{
    const auto *application = qobject_cast<const QGuiApplication *>(QCoreApplication::instance());
    return application != nullptr && application->clipboard() != nullptr
        && application->clipboard()->supportsSelection();
}

bool ferric_browser_write_clipboard(const QString &value, bool primary)
{
    const auto *application = qobject_cast<const QGuiApplication *>(QCoreApplication::instance());
    if (application == nullptr || application->clipboard() == nullptr) {
        return false;
    }
    auto *clipboard = application->clipboard();
    const auto mode = primary ? QClipboard::Selection : QClipboard::Clipboard;
    if (primary && !clipboard->supportsSelection()) {
        return false;
    }
    clipboard->setText(value, mode);
    return true;
}

QString ferric_browser_qt_platform_name()
{
    return QGuiApplication::platformName();
}

QString ferric_browser_qt_quick_graphics_api()
{
    switch (QQuickWindow::graphicsApi()) {
    case QSGRendererInterface::Software:
        return QStringLiteral("software");
    case QSGRendererInterface::OpenVG:
        return QStringLiteral("openvg");
    case QSGRendererInterface::OpenGL:
        return QStringLiteral("opengl");
    case QSGRendererInterface::Direct3D11:
        return QStringLiteral("direct3d11");
    case QSGRendererInterface::Vulkan:
        return QStringLiteral("vulkan");
    case QSGRendererInterface::Metal:
        return QStringLiteral("metal");
    case QSGRendererInterface::Null:
        return QStringLiteral("null");
    case QSGRendererInterface::Direct3D12:
        return QStringLiteral("direct3d12");
    case QSGRendererInterface::Unknown:
        return QStringLiteral("unknown");
    }
    return QStringLiteral("unknown");
}

QString ferric_browser_qt_webengine_version()
{
    return QStringLiteral(QTWEBENGINECORE_VERSION_STR);
}

QString ferric_browser_qt_chromium_version()
{
    return QString::fromLatin1(qWebEngineChromiumVersion());
}

QString ferric_browser_qt_chromium_security_patch_version()
{
    return QString::fromLatin1(qWebEngineChromiumSecurityPatchVersion());
}

void ferric_browser_set_desktop_identity()
{
    // Keep Qt's native Wayland app_id aligned with the installed desktop file.
    // The extension is intentionally omitted: Qt expects the desktop-file
    // basename, while the desktop entry itself remains the .desktop file.
    QGuiApplication::setDesktopFileName(QStringLiteral("io.github.ferricbrowser.FerricBrowser"));
    QGuiApplication::setApplicationDisplayName(QStringLiteral("FerricBrowser"));
}

void ferric_browser_enable_software_rendering()
{
    auto flags = qgetenv("QTWEBENGINE_CHROMIUM_FLAGS");
    const auto append_flag = [&flags](const QByteArray &flag) {
        const auto tokens = flags.split(' ');
        if (std::any_of(tokens.cbegin(), tokens.cend(), [&flag](const QByteArray &token) {
                return token == flag;
            })) {
            return;
        }
        if (!flags.isEmpty()) {
            flags.append(' ');
        }
        flags.append(flag);
    };
    append_flag("--disable-gpu");
    append_flag("--disable-gpu-compositing");
    qputenv("QTWEBENGINE_CHROMIUM_FLAGS", flags);
    qputenv("QT_QUICK_BACKEND", "software");
}

void ferric_browser_register_internal_scheme()
{
    // Reserve the internal-document namespace before any WebEngine profile or
    // view exists. No URL handler is installed: management UI remains native
    // QML, and an rb:// navigation cannot reach privileged application code.
    static const bool registered = [] {
        QWebEngineUrlScheme scheme("rb");
        scheme.setSyntax(QWebEngineUrlScheme::Syntax::Host);
        scheme.setFlags(QWebEngineUrlScheme::SecureScheme);
        QWebEngineUrlScheme::registerScheme(scheme);
        return true;
    }();
    Q_UNUSED(registered);
}
