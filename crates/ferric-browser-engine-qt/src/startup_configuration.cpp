#include "startup_configuration.h"
#include "ferric-browser-engine-qt/src/bridge.cxxqt.h"
#include <utility>

namespace {
struct StartupConfigurationHandle final {
  explicit StartupConfigurationHandle(rust::Box<StartupConfiguration> value)
      : captured(std::move(value)) {}
  rust::Box<StartupConfiguration> captured;
};
using SharedStartupConfiguration = std::shared_ptr<StartupConfigurationHandle>;
}
Q_DECLARE_METATYPE(SharedStartupConfiguration)

QVariant ferric_startup_configuration_value(rust::Box<StartupConfiguration> captured) {
  return QVariant::fromValue(std::make_shared<StartupConfigurationHandle>(std::move(captured)));
}

bool ferric_install_startup_configuration(const QVariant &value, BrowserUi &ui,
                                         const QString &baseJson) {
  if (value.metaType() != QMetaType::fromType<SharedStartupConfiguration>())
    return false;
  const auto handle = value.value<SharedStartupConfiguration>();
  return handle && install_startup_configuration(*handle->captured, ui, baseJson);
}

QString ferric_startup_base_json(const QVariant &value) {
  if (value.metaType() != QMetaType::fromType<SharedStartupConfiguration>())
    return {};
  const auto handle = value.value<SharedStartupConfiguration>();
  return handle ? startup_base_json(*handle->captured) : QString();
}
