#pragma once
#include <QVariant>
#include <memory>
#include <rust/cxx.h>

class BrowserUi;
struct StartupConfiguration;

// Opaque native transport, not a QML type or a source lookup registry.
QVariant ferric_startup_configuration_value(rust::Box<StartupConfiguration> captured);
bool ferric_install_startup_configuration(const QVariant &value, BrowserUi &ui,
                                         const QString &baseJson);
QString ferric_startup_base_json(const QVariant &value);
