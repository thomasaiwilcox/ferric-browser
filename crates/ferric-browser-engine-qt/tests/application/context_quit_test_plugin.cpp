// Native input for the public context CLI fixture's explicit Quit decision.
// This is a standalone test plugin, never linked into or shipped with Ferric.
#include <QDir>
#include <QFile>
#include <QGenericPlugin>
#include <QGuiApplication>
#include <QJsonDocument>
#include <QJsonObject>
#include <QPointer>
#include <QQuickItem>
#include <QQuickWindow>
#include <QTimer>
#include <QtTest>

class OwnedQuitInput final : public QObject {
    const QString root;
    QTimer timer;
    QPointer<QQuickItem> pendingDialog;
    QMetaObject::Connection frameConnection;
    bool presented = false;
    bool armed = false;
    bool decisionSent = false;
    QPointer<QQuickWindow> confirmedWindow;
    QByteArray previousState;

    static QQuickItem *find(QQuickItem *parent, const QString &name) {
        if (parent->isVisible() && parent->objectName() == name)
            return parent;
        for (auto child : parent->childItems())
            if (auto match = find(child, name))
                return match;
        return nullptr;
    }

    void recordOutcome() {
        QJsonObject state{{"windowAlive", !confirmedWindow.isNull()}};
        if (confirmedWindow) {
            for (const auto name :
                 {"shutdownConfirmationVisible", "shutdownPromptVisible",
                  "shutdownPagePromptVisible", "applicationShutdownInProgress",
                  "applicationShutdownForcePromptVisible", "applicationShutdownStage"})
                state[name] = QJsonValue::fromVariant(confirmedWindow->property(name));
            auto ui = confirmedWindow->property("primaryBrowserUi").value<QObject *>();
            if (ui)
                state["status"] = QJsonValue::fromVariant(ui->property("status_text"));
        }
        const auto bytes = QJsonDocument(state).toJson(QJsonDocument::Compact);
        if (bytes != previousState) {
            previousState = bytes;
            QFile file(root + "/quit-observed-state.json");
            if (file.open(QIODevice::WriteOnly | QIODevice::Truncate))
                file.write(bytes);
        }
    }

    void poll() {
        if (decisionSent) {
            recordOutcome();
            return;
        }
        if (QGuiApplication::platformName() != "offscreen")
            return;
        QFile authorization(root + "/confirm-quit");
        if (!authorization.open(QIODevice::ReadOnly) ||
            authorization.readAll() != "Confirm this fixture's browser quit.\n")
            return;
        if (!armed) {
            qInfo("Owned context Quit input armed");
            armed = true;
        }
        QJsonObject observed{{"platform", QGuiApplication::platformName()},
                             {"windowCount", QGuiApplication::allWindows().size()},
                             {"dialogFound", false}};
        for (auto candidate : QGuiApplication::allWindows()) {
            auto window = qobject_cast<QQuickWindow *>(candidate);
            if (!window || !window->isVisible())
                continue;
            auto dialog = find(window->contentItem(), "shutdownDecisionDialog");
            if (!dialog || dialog->property("title").toString() != "Quit Ferric Browser?" ||
                dialog->property("commandText").toString() != ":quit")
                continue;
            observed["dialogFound"] = true;
            observed["presented"] = presented;
            observed["windowActive"] = window->isActive();
            auto choice = find(dialog, "shutdownProceedButton");
            if (!choice || !choice->isEnabled() ||
                choice->property("text").toString() != "Quit browser")
                continue;
            if (pendingDialog != dialog) {
                QObject::disconnect(frameConnection);
                pendingDialog = dialog;
                presented = false;
                frameConnection = connect(window, &QQuickWindow::frameSwapped, this,
                                          [this] { presented = true; });
                window->requestUpdate();
                return;
            }
            if (!presented)
                return;
            const auto bounds = choice->mapRectToScene(choice->boundingRect());
            if (choice->width() <= 0 || choice->height() <= 0 ||
                !QRectF(0, 0, window->width(), window->height()).contains(bounds))
                return;
            window->requestActivate();
            if (!window->isActive())
                return;
            authorization.close();
            if (!QFile::remove(root + "/confirm-quit"))
                qFatal("Owned Quit authorization could not be consumed");
            confirmedWindow = window;
            decisionSent = true;
            QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier, bounds.center().toPoint());
            QFile evidence(root + "/quit-confirmed.json");
            if (!evidence.open(QIODevice::WriteOnly | QIODevice::NewOnly))
                qFatal("Owned Quit evidence could not be created");
            evidence.write(QJsonDocument(QJsonObject{{"input", "native mouse click"},
                                                     {"choice", "Quit browser"},
                                                     {"presented", true}})
                               .toJson());
            evidence.close();
            recordOutcome();
            return;
        }
        const auto state = QJsonDocument(observed).toJson(QJsonDocument::Compact);
        if (state != previousState) {
            previousState = state;
            qInfo().noquote() << "Owned context Quit state" << state;
            QFile facts(root + "/quit-helper-state.json");
            if (facts.open(QIODevice::WriteOnly | QIODevice::Truncate))
                facts.write(state);
        }
    }

  public:
    explicit OwnedQuitInput(QString path) : root(std::move(path)) {
        qInfo("Owned context Quit input helper loaded");
        timer.setInterval(25);
        connect(&timer, &QTimer::timeout, this, [this] { poll(); });
        timer.start();
    }
};

class ContextQuitTestPlugin final : public QGenericPlugin {
    Q_OBJECT
    Q_PLUGIN_METADATA(IID "org.qt-project.Qt.QGenericPluginFactoryInterface" FILE
                          "context_quit_test_plugin.json")

  public:
    QObject *create(const QString &name, const QString &) override {
        qInfo() << "Owned context Quit plugin requested" << name << QGuiApplication::platformName();
        if (name != "ferricownedquitfixture" ||
            qEnvironmentVariable("QT_QPA_PLATFORM") != "offscreen")
            return nullptr;
        const auto root = qEnvironmentVariable("FERRIC_OWNED_QUIT_ROOT");
        QFile marker(root + "/.ferric-owned-fixture");
        if (root.isEmpty() || !QDir(root).isAbsolute() || !marker.open(QIODevice::ReadOnly) ||
            marker.readAll() != "Owned context-deletion test only.\n")
            return nullptr;
        return new OwnedQuitInput(root);
    }
};
#include "context_quit_test_plugin.moc"
