# S24 accessibility capability and coexistence report

**Decision: no-go for a production keyboard semantic provider in this slice.** The public Qt Quick path can expose a named button role and press action through AT-SPI, and a dynamically destroyed target is invalidated in the tested case. The same client receives no component extents for that QML button. The loaded Qt WebEngine document exposes no child controls through AT-SPI in this Qt 6.11.2 Wayland run, despite the fixture containing ordinary controls, a labelled canvas, and a closed-root control. The probe therefore does not provide the public names, roles, bounds and actions needed for a bounded browser target provider.

## Tested route

The opt-in probe ran `qml_tests` on the native Wayland QPA plugin with `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1` and `QTWEBENGINE_ENABLE_LINUX_ACCESSIBILITY=1`. The probe created an isolated session D-Bus and AT-SPI bus under a temporary runtime directory, enabled `IsEnabled` and `ScreenReaderEnabled` only on that bus, and ran an AT-SPI client against it. GSettings used the in-memory backend. The live desktop AT-SPI flags and the two corresponding desktop preferences were `false` before and after the run.

The QML test verified that the local HTML document loaded and returned its expected heading. The AT-SPI tree exposed the test window, a named QML `button`, and its `Press`/`SetFocus` actions. The AT-SPI client invoked `Press`; the fixture counter reached exactly one. After the object was destroyed, the captured AT-SPI object reported `DEFUNCT`, disappeared from a fresh tree walk, and rejected a second `Press`.

The QML button had no AT-SPI Component interface or extents. The WebEngine subtree consisted only of an unnamed `document web` node with no children: the expected HTML button, canvas, and closed-root button names were absent. This is an observed limitation of this tested Qt/WebEngine/QPA configuration, not a universal claim about other platforms or versions.

## Assistive-technology coexistence

The installed AT-SPI library and registry were exercised by a small public AT-SPI client. Orca and Accerciser are not installed, so the probe could not verify spoken output, focus announcements, navigation, or coexistence with an actual screen reader. No screen-reader pass is claimed. This missing evidence is part of the no-go decision rather than a reason to add an unqualified production path.

## Public API context

Qt Quick documents the `Accessible` QML type for names, descriptions, roles, and invocable actions on QML items. The test confirms that route for the synthetic QML button. [Qt's Qt Quick accessibility guide](https://doc.qt.io/qt-6.11/accessible-qtquick.html) describes those properties and action handlers.

Qt WebEngine documents that page accessibility requires Qt Core accessibility support and that the platform plugin must be notified that accessibility is active; on Linux, `QTWEBENGINE_ENABLE_LINUX_ACCESSIBILITY=0` can disable it. The probe enabled the provider path on a private AT-SPI session but observed no semantic child tree. [Qt WebEngine's platform notes](https://doc.qt.io/qt-6.11/qtwebengine-platform-notes.html) state those activation conditions. Qt's public `QAccessible` documentation describes the Linux AT-SPI status flags and the `QT_LINUX_ACCESSIBILITY_ALWAYS_ON` alternative. [QAccessible, Qt 6.11](https://doc.qt.io/qt-6.11/qaccessible.html).

## T035 boundary and remaining qualification

The C22 checks `cargo xtask check` and `cargo xtask test engine` passed. The latter passed 442 engine-crate tests, 144 QML/WebEngine tests (including the T035 cross-origin and closed-root cases), and native CTest 1/1. Those WebEngine tests continue to expose only the parent-visible surface and do not claim child semantic access. The S23 public frame probe separately records why general cross-origin semantic targeting has no-go status. Native Wayland physical keyboard input for T035 was not run: the existing virtual-keyboard injector is unavailable in this compositor path. The S24 provider probe itself did run under Wayland, but its AT-SPI actions are not a substitute for physical keyboard or screen-reader qualification.
