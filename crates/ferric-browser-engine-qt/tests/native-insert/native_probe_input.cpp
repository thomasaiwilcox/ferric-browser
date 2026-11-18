#include "native_probe_input.h"
#include <QtCore/QCoreApplication>
#include <QtDBus/QDBusInterface>
#include <QtDBus/QDBusReply>
#include <QtGui/QGuiApplication>
#include <QtGui/QWindow>

#ifdef FERRIC_NATIVE_PROBE_XTEST
#include <X11/Xlib.h>
#include <X11/XKBlib.h>
#include <X11/keysym.h>
#include <X11/extensions/XTest.h>
#endif

bool sendNativeProbeKey(QWindow *window, int key, QString *error,
    Qt::KeyboardModifiers modifiers, const QString &position)
{
    if (!window || QGuiApplication::focusWindow() != window) {
        *error = QStringLiteral("Owned probe window does not hold Qt focus");
        return false;
    }
    if (QGuiApplication::platformName().startsWith(QStringLiteral("wayland"))) {
        if (qEnvironmentVariable("FERRIC_PORTAL_TEST_BUS") != QStringLiteral("isolated")) {
            *error = QStringLiteral("Wayland probe requires the isolated owned compositor");
            return false;
        }
        QDBusInterface compositor(QStringLiteral("org.ferricbrowser.ParentFixture"),
            QStringLiteral("/org/ferricbrowser/ParentFixture"),
            QStringLiteral("org.ferricbrowser.ParentFixture"), QDBusConnection::sessionBus());
        QDBusReply<QVariantList> windows = compositor.call(QStringLiteral("Windows"));
        QStringList matches;
        if (windows.isValid()) {
            for (const auto &item : windows.value()) {
                const auto candidate = qdbus_cast<QVariantMap>(item);
                if (candidate.value("title").toString() == window->title()
                    && candidate.value("pid").toLongLong() == QCoreApplication::applicationPid()
                    && candidate.value("mapped").toBool())
                    matches.append(candidate.value("id").toString());
            }
        }
        if (matches.size() != 1 || matches.first().isEmpty()) {
            *error = QStringLiteral("Owned compositor surface is missing or ambiguous");
            return false;
        }
        QDBusReply<bool> sent = compositor.call(position.isEmpty() ? QStringLiteral("Key") : QStringLiteral("KeyPosition"),
            matches.first(), position.isEmpty() ? QVariant(key) : QVariant(position), int(modifiers));
        if (sent.isValid() && sent.value()) return true;
        *error = QStringLiteral("Owned compositor rejected source key");
        return false;
    }
#ifdef FERRIC_NATIVE_PROBE_XTEST
    if (QGuiApplication::platformName() == QStringLiteral("xcb")
        && key >= Qt::Key_A && key <= Qt::Key_Z && modifiers == Qt::NoModifier && position.isEmpty()) {
        auto *display = XOpenDisplay(nullptr);
        if (!display) { *error = QStringLiteral("Cannot open X11 display"); return false; }
        struct Close { Display *display; ~Close() { XCloseDisplay(display); } } close{display};
        // Keep other clients from changing native focus between verification
        // and the complete source gesture. Release before any Qt event wait.
        XGrabServer(display);
        struct Ungrab {
            Display *display;
            ~Ungrab() { XUngrabServer(display); XSync(display, False); }
        } ungrab{display};
        Window focused = 0;
        int revert = 0, event = 0, detail = 0, major = 0, minor = 0;
        XGetInputFocus(display, &focused, &revert);
        if (focused != window->winId() || !XTestQueryExtension(display, &event, &detail, &major, &minor)) {
            *error = QStringLiteral("Owned X11 window lacks native focus or XTEST is absent");
            return false;
        }
        const auto code = XKeysymToKeycode(display, static_cast<KeySym>('a' + key - Qt::Key_A));
        char held[32]{};
        XQueryKeymap(display, held);
        if (!code || (held[code / 8] & (1 << (code % 8)))) {
            *error = QStringLiteral("X11 source key is unavailable or already physically held");
            return false;
        }
        XkbStateRec state{};
        // NumLock does not modify alphabetic input. Read its actual XKB mask
        // rather than clearing a user's lock or assuming a fixed Mod2 bit.
        const auto numLock = XkbKeysymToModifiers(display, XK_Num_Lock);
        if (XkbGetState(display, XkbUseCoreKbd, &state) != Success || (state.mods & ~numLock)) {
            *error = QStringLiteral("X11 probe requires an unmodified native keyboard state");
            return false;
        }
        // Queue the complete unmodified gesture to our focused test window.
        // Never use XTEST as a production destination/remapping mechanism.
        const bool down = XTestFakeKeyEvent(display, code, True, CurrentTime);
        const bool up = XTestFakeKeyEvent(display, code, False, CurrentTime);
        XSync(display, False);
        if (down && up) return true;
        *error = QStringLiteral("XTEST rejected source gesture");
        return false;
    }
#endif
    *error = QStringLiteral("No native source-input driver for this backend");
    return false;
}
