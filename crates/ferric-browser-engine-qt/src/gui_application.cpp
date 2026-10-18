#include "gui_application.h"
#include <QApplication>
#include <QDir>
#include <QFileInfo>
#include <QSettings>
#include <QTemporaryDir>
#include <vector>

namespace {
// Qt keeps references to argc and argv until application destruction.
// The storage base is constructed first and destroyed after QApplication.
struct ApplicationArguments {
    explicit ApplicationArguments(const QVector<QByteArray> &values) : values(values) {
        if (this->values.isEmpty()) this->values.append(QByteArray("ferric-browser"));
        count = static_cast<int>(this->values.size());
        pointers.reserve(this->values.size() + 1);
        for (auto &value : this->values) pointers.push_back(value.data());
        pointers.push_back(nullptr);
    }
    QVector<QByteArray> values;
    std::vector<char *> pointers;
    int count = 0;
};
// Keep toolkit-owned UI preferences out of desktop/profile state. Activate
// after platform-theme initialization, and retain the scope through QWidget
// teardown. Browser configuration continues to use the Rust configuration model.
struct SessionUiPreferences {
    bool activate() {
        QSettings previous(QSettings::NativeFormat, QSettings::UserScope, QStringLiteral("QtProject"));
        previousPath = QFileInfo(previous.fileName()).absolutePath();
        root = std::make_unique<QTemporaryDir>(QDir::tempPath() + QStringLiteral("/ferric-ui-XXXXXX"));
        if (!root->isValid()) return false;
        QSettings::setPath(QSettings::NativeFormat, QSettings::UserScope, root->path());
        active = true;
        QSettings current(QSettings::NativeFormat, QSettings::UserScope, QStringLiteral("QtProject"));
        return QFileInfo(current.fileName()).absolutePath() == root->path();
    }
    ~SessionUiPreferences() {
        if (active) QSettings::setPath(QSettings::NativeFormat, QSettings::UserScope, previousPath);
    }
    QString previousPath;
    std::unique_ptr<QTemporaryDir> root;
    bool active = false;
};
class FerricApplication final : private ApplicationArguments, private SessionUiPreferences, public QApplication {
public:
    explicit FerricApplication(const QVector<QByteArray> &arguments)
        : ApplicationArguments(arguments), QApplication(count, pointers.data()), preferencesReady(activate()) {}
    const bool preferencesReady;
};
}

std::unique_ptr<QGuiApplication> ferric_browser_gui_application(const QVector<QByteArray> &arguments) {
    auto application = std::make_unique<FerricApplication>(arguments);
    if (!application->preferencesReady) return {};
    return application;
}

bool ferric_browser_gui_preferences_ready() {
    auto application = dynamic_cast<FerricApplication *>(QCoreApplication::instance());
    return application && application->preferencesReady;
}
