#include <QtQuickTest/quicktest.h>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>

int main(int argc, char **argv)
{
    // WebEngine must configure shared OpenGL contexts before Qt Quick Test
    // constructs its QGuiApplication, including when tests run offscreen.
    QtWebEngineQuick::initialize();
    return quick_test_main(argc, argv, "ferric_qml_tests", nullptr);
}
