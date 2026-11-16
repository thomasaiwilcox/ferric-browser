#pragma once

#include <QtCore/QString>
class QWindow;

// Test source input only. Production destinations stay inside the Qt adapter.
bool sendNativeProbeKey(QWindow *window, int key, QString *error,
    Qt::KeyboardModifiers modifiers = Qt::NoModifier, const QString &position = {});
