import QtQuick
import io.github.ferricbrowser 1.0

// The application entry point deliberately owns composition only. Browser
// lifecycle, QtWebEngine coordination, and visual chrome live with the
// primary window component rather than accumulating in the module root.
FerricPrimaryBrowserWindow {
}
