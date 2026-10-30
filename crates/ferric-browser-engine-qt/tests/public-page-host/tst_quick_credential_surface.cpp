#include "../credential_https_fixture.h"
#include "quick_credential_owner.h"
#include "credential_boundary.h"
#include "quick_credential_binding.h"
#include "quick_credential_integration.h"
#include "profile_lifetime.h"
#include "request_interceptor.h"
#include "popup_owner_bridge.h"

#include <QApplication>
#include <QCloseEvent>
#include <QDialog>
#include <QPushButton>
#include <QSignalSpy>
#include <QJsonDocument>
#include <QJsonObject>
#include <QQmlComponent>
#include <QQmlContext>
#include <QQmlEngine>
#include <QQmlProperty>
#include <QQuickItem>
#include <QQuickWindow>
#include <QTest>
#include <QWebChannel>
#include <QWebEngineCertificateError>
#include <QWebEngineFrame>
#include <QWebEngineLoadingInfo>
#include <QWebEngineScript>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>
#include <QtWebEngineQuick/QQuickWebEngineProfile>
#include <memory>
#include <signal.h>

extern "C" bool cxx_qt_init_crate_ferric_browser_engine_qt();

class FerricCredentialBoundaryTestAccess {
public:
    static bool beginNotificationChallenge(FerricCredentialBoundary *boundary) {
        boundary->challenge();
        return boundary->challenging_;
    }
};

class QuickCredentialWitness final : public QObject {
    Q_OBJECT
public:
    int notifications = 0;
    quint64 documentGeneration = 0;
    QWebEngineFrame frame;
    int fixturePort = 0;
    Q_INVOKABLE void notify() { ++notifications; }
public slots:
    void certificateError(QWebEngineCertificateError error) {
        if (error.url().host() == "localhost" && error.url().port() == fixturePort)
            error.acceptCertificate();
        else error.rejectCertificate();
    }
    void nativeLoadingChanged(const QWebEngineLoadingInfo &info) {
        if (info.status() == QWebEngineLoadingInfo::LoadStartedStatus) ++documentGeneration;
    }
};

// The same native Ui-to-interceptor property projection used by profile
// resources, with a distinct source Ui from the requesting page's owner.
class ProfilePolicyProjection final : public QObject {
    Q_OBJECT
public:
    ProfilePolicyProjection(QObject *ui, FerricBrowserRequestInterceptor *interceptor)
        : QObject(ui), ui_(ui), interceptor_(interceptor) {
        connect(ui, SIGNAL(blocking_hostsChanged()), this, SLOT(refresh()));
    }
private slots:
    void refresh() {
        if (ui_ && interceptor_) interceptor_->setBlockedHosts(ui_->property("blocking_hosts").toStringList());
    }
private:
    QPointer<QObject> ui_;
    QPointer<FerricBrowserRequestInterceptor> interceptor_;
};

namespace {
QVariant run(QWebEngineFrame frame, const QString &script, quint32 world)
{
    struct Result { QVariant value; bool done = false; };
    const auto result = std::make_shared<Result>();
    frame.runJavaScript(script, world, [result](const QVariant &value) {
        result->value = value; result->done = true;
    });
    if (!QTest::qWaitFor([&] { return result->done; }, 5000)) return {};
    return result->value;
}

QString read(const QString &name)
{
    QFile file(QStringLiteral(FERRIC_CREDENTIAL_SCRIPTS) + '/' + name);
    if (!file.open(QIODevice::ReadOnly)) return {};
    return QString::fromUtf8(file.readAll());
}

constexpr auto source = R"QML(
import QtQuick
import QtWebEngine
Window {
    id: root
    visible: false; width: 640; height: 420
    property alias view: content
    property alias profilePrototype: prototype
    property var fixtureProfile: null
    Component.onCompleted: fixtureProfile = prototype.instance()
    WebEngineView {
        // The profile factory must outlive WebContents teardown. Keeping it
        // under the view also gives standalone fixture roots that ordering.
        WebEngineProfilePrototype { id: prototype; storageName: "" }
        id: content
        anchors.fill: parent
        url: ""
        profile: root.fixtureProfile
        property var viewUi: null
        property var viewHost: root
        property string stableTabId: ""
        property int tabIndex: 0
        property string promptLeaseIdentity: ""
        property bool viewRetired: false
    }
}
)QML";

struct OwnedQuickCase {
    FixtureServer server;
    QuickCredentialWitness witness;
    QQmlEngine engine;
    QString context = "native-quick-lifetime-fixture";
    std::unique_ptr<QObject> root;
    QQuickWindow *window = nullptr;
    QQuickItem *view = nullptr;
    FerricQuickCredentialOwner *owner = nullptr;
    FerricCredentialBoundary *boundary = nullptr;

    bool open(std::function<FerricCredentialBoundary *(QQuickItem *)> makeBoundary = {}, bool installBeforeLoad = true) {
        if (!server.start()) return false;
        server.setDocuments("<!doctype html><title>Quick lifetime</title>", "<!doctype html>");
        witness.fixturePort = server.pageUrl().port();
        QQmlComponent component(&engine);
        component.setData(source, QUrl("qrc:/quick-lifetime-fixture.qml"));
        root.reset(component.create());
        if (!root) return false;
        window = qobject_cast<QQuickWindow *>(root.get());
        view = qobject_cast<QQuickItem *>(root->property("view").value<QObject *>());
        if (!window || !view || !QObject::connect(view, SIGNAL(certificateError(QWebEngineCertificateError)),
                &witness, SLOT(certificateError(QWebEngineCertificateError)))) return false;
        owner = FerricQuickCredentialOwner::create(view);
        if (!owner) return false;
        if (installBeforeLoad) {
            boundary = makeBoundary ? makeBoundary(view) : owner->prepareBoundary([this] { return context; });
            if (!boundary) return false;
        }
        window->show();
        if (!QTest::qWaitForWindowExposed(window) || !QTest::qWaitForWindowActive(window)) return false;
        if (!view->setProperty("url", server.pageUrl())) return false;
        return QTest::qWaitFor([this] {
            return view->property("title").toString() == "Quick lifetime" && !view->property("loading").toBool();
        }, 15000);
    }

    bool request() {
        run(owner->mainFrame(), "postMessage({type:'ferric.credentials.request.v1',id:'lifetime',op:'get',options:{challenge:'AQ'}},location.origin)", 0);
        return QTest::qWaitFor([this] { return boundary->request() != nullptr; }, 5000);
    }
};

std::unique_ptr<QObject> applicationOwner(OwnedQuickCase &fixture, QTemporaryDir &storage, bool durable = false)
{
    if (!storage.isValid()) return {};
    QQmlComponent component(&fixture.engine);
    component.setData("import io.github.ferricbrowser 1.0\nBrowserUi {}", QUrl());
    std::unique_ptr<QObject> ui(component.create());
    if (!ui) return {};
    const auto config = QStringLiteral(R"JSON({"schema_version":3,"passkeys":{"phone":"on"}})JSON");
    ui->setProperty("config_json", config);
    ui->setProperty("config_base_json", config);
    if (durable) {
        bool accepted = false;
        const auto contexts = QStringLiteral(R"JSON({"schema_version":3,"contexts":[{"name":"work","label":"Work","profile":"credential-fixture"}]})JSON");
        if (!QMetaObject::invokeMethod(ui.get(), "set_contexts_configuration", Qt::DirectConnection,
            Q_RETURN_ARG(bool, accepted), Q_ARG(QString, contexts)) || !accepted) return {};
    }
    if (!QMetaObject::invokeMethod(ui.get(), "configure_profile", Qt::DirectConnection,
        Q_ARG(bool, false), Q_ARG(bool, !durable), Q_ARG(QString, QStringLiteral("Credential fixture")),
        Q_ARG(QString, QStringLiteral("credential-fixture")), Q_ARG(QString, storage.path()))) return {};
    if (!QTest::qWaitFor([&] {
        QMetaObject::invokeMethod(ui.get(), "poll_config", Qt::DirectConnection);
        return !ui->property("profile_bootstrap_pending").toBool() && ui->property("tab_count").toInt() > 0;
    }, 15000)) return {};
    return ui;
}

QString firstTab(QObject *ui) {
    QString tab;
    QMetaObject::invokeMethod(ui, "tab_id_for_index", Qt::DirectConnection, Q_RETURN_ARG(QString, tab), Q_ARG(int, 0));
    return tab;
}

bool isOwnedProcess(qint64 pid)
{
    const auto process = QCoreApplication::applicationPid();
    if (pid <= 1 || pid == process) return false;
    for (int depth = 0; depth < 64 && pid > 1; ++depth) {
        if (pid == process) return true;
        QFile status(QString("/proc/%1/status").arg(pid));
        if (!status.open(QIODevice::ReadOnly)) return false;
        qint64 parent = 0;
        for (const auto &line : status.readAll().split('\n')) {
            if (line.startsWith("PPid:")) parent = line.mid(5).trimmed().toLongLong();
        }
        pid = parent;
    }
    return false;
}
}

class QuickCredentialSurfaceTests final : public QObject {
    Q_OBJECT
private slots:
    void closeCancellationCanDestroyTheOwner_data()
    {
        QTest::addColumn<bool>("destroyHost");
        QTest::newRow("view-owner") << false;
        QTest::newRow("whole-host") << true;
    }

    void closeCancellationCanDestroyTheOwner()
    {
        QFETCH(bool, destroyHost);
        OwnedQuickCase fixture;
        QVERIFY(fixture.open());
        QVERIFY(fixture.request());
        const QPointer<FerricQuickCredentialOwner> owner(fixture.owner);
        const QPointer<QQuickWindow> host(fixture.window);
        bool cancelled = false;
        connect(fixture.boundary, &FerricCredentialBoundary::cancelled, fixture.window, [&] {
            cancelled = true;
            if (destroyHost) fixture.root.reset();
            else delete fixture.view;
        });
        QCloseEvent close;
        QVERIFY(QCoreApplication::sendEvent(fixture.window, &close));
        QVERIFY(cancelled);
        QVERIFY(owner.isNull());
        QCOMPARE(host.isNull(), destroyHost);
        if (host) QVERIFY(host->isVisible());
    }

    void closeAttemptRevokesEvenWhenTheWindowRemainsOpen()
    {
        OwnedQuickCase fixture;
        QVERIFY(fixture.open());
        QQmlComponent closing(&fixture.engine);
        closing.setData("import QtQuick; Connections { property var host; target: host; function onClosing(event) { event.accepted = false } }", QUrl());
        std::unique_ptr<QObject> guard(closing.create());
        QVERIFY2(guard, qPrintable(closing.errorString()));
        QVERIFY(guard->setProperty("host", QVariant::fromValue(static_cast<QObject *>(fixture.window))));
        QVERIFY(fixture.request());
        const auto captured = fixture.owner->captureOperation();
        QSignalSpy cancelled(fixture.boundary, &FerricCredentialBoundary::cancelled);
        QVERIFY(!fixture.window->close());
        QVERIFY(fixture.window->isVisible());
        QCOMPARE(cancelled.count(), 1);
        QVERIFY(!fixture.owner->isCurrent(captured));
        QVERIFY(!fixture.boundary->request());
        QVERIFY(!fixture.boundary->complete("{\"error\":\"AbortError\"}"));
        // Rejecting the close keeps a usable page, with a fresh ceremony.
        QVERIFY(fixture.request());
        QVERIFY(fixture.boundary->request());
    }

    void nativeCaptureBindsTheActualApplicationTabAndProfileSession()
    {
        OwnedQuickCase fixture;
        QTemporaryDir storage;
        QVERIFY(storage.isValid());
        QQmlComponent component(&fixture.engine);
        component.setData("import io.github.ferricbrowser 1.0\nBrowserUi {}", QUrl());
        std::unique_ptr<QObject> ui(component.create());
        QVERIFY2(ui, qPrintable(component.errorString()));
        QVERIFY(QMetaObject::invokeMethod(ui.get(), "configure_profile", Qt::DirectConnection,
            Q_ARG(bool, false), Q_ARG(bool, true), Q_ARG(QString, QStringLiteral("Credential fixture")),
            Q_ARG(QString, QStringLiteral("credential-fixture")), Q_ARG(QString, storage.path())));
        QTRY_VERIFY_WITH_TIMEOUT(([&] {
            QMetaObject::invokeMethod(ui.get(), "poll_config", Qt::DirectConnection);
            return !ui->property("profile_bootstrap_pending").toBool()
                && ui->property("tab_count").toInt() > 0;
        })(), 15000);
        QString tab;
        QVERIFY(QMetaObject::invokeMethod(ui.get(), "tab_id_for_index", Qt::DirectConnection,
            Q_RETURN_ARG(QString, tab), Q_ARG(int, 0)));
        QVERIFY(!tab.isEmpty());
        FerricQuickCredentialBinding *binding = nullptr;
        bool foreignRejected = false;
        bool staleTabRejected = false;
        QVERIFY(fixture.open([&](QQuickItem *view) {
            QObject foreignUi;
            foreignRejected = !FerricQuickCredentialBinding::create(view, &foreignUi, tab);
            staleTabRejected = !FerricQuickCredentialBinding::create(view, ui.get(), QStringLiteral("invalid-tab"));
            binding = FerricQuickCredentialBinding::create(view, ui.get(), tab);
            return binding ? binding->boundary() : nullptr;
        }));
        QVERIFY(foreignRejected);
        QVERIFY(staleTabRejected);
        QVERIFY(binding && binding->isReady());
        QVERIFY(!FerricQuickCredentialBinding::create(fixture.view, ui.get(), tab));
        QSignalSpy captured(binding, &FerricQuickCredentialBinding::requestReady);
        QSignalSpy invalidated(binding, &FerricQuickCredentialBinding::leaseInvalidated);
        QVERIFY(fixture.request());
        QCOMPARE(captured.count(), 1);
        QVERIFY(binding->hasCurrentLease());
        int newIndex = -1;
        QVERIFY(QMetaObject::invokeMethod(ui.get(), "new_tab", Qt::DirectConnection, Q_RETURN_ARG(int, newIndex)));
        QVERIFY(newIndex > 0);
        QVERIFY(!binding->hasCurrentLease());
        QCOMPARE(invalidated.count(), 1);
        QVERIFY(!fixture.boundary->request());
        QVERIFY(!fixture.boundary->complete("{\"native\":true}"));
        bool selected = false;
        QVERIFY(QMetaObject::invokeMethod(ui.get(), "select_tab", Qt::DirectConnection,
            Q_RETURN_ARG(bool, selected), Q_ARG(int, 0)));
        QVERIFY(selected);
        // The old request was aborted in its captured frame. A fresh request
        // can obtain a new actual lease after selection returns to this tab.
        run(fixture.owner->mainFrame(), "true", FerricCredentialBoundary::World);
        QVERIFY(fixture.request());
        QCOMPARE(captured.count(), 2);
        QVERIFY(binding->hasCurrentLease());
        ui.reset();
        QVERIFY(!binding->isReady());
        QVERIFY(!binding->hasCurrentLease());
        QCOMPARE(invalidated.count(), 2);
        QVERIFY(!fixture.boundary->request());
    }

    void backgroundDocumentHasMediatorBeforeSelectionWithoutReload()
    {
        OwnedQuickCase fixture;
        QTemporaryDir storage;
        auto ui = applicationOwner(fixture, storage);
        QVERIFY(ui);
        const auto tab = firstTab(ui.get());
        int second = -1;
        QVERIFY(QMetaObject::invokeMethod(ui.get(), "new_tab", Qt::DirectConnection, Q_RETURN_ARG(int, second)));
        QCOMPARE(second, 1);
        FerricQuickCredentialBinding *binding = nullptr;
        QVERIFY(fixture.open([&](QQuickItem *view) {
            binding = FerricQuickCredentialBinding::create(view, ui.get(), tab);
            return binding ? binding->boundary() : nullptr;
        }));
        QCOMPARE(run(fixture.owner->mainFrame(), "typeof __ferricCredentialMediator", 42).toString(), QStringLiteral("object"));
        run(fixture.owner->mainFrame(), "window.backgroundDraft='preserved';postMessage({type:'ferric.credentials.request.v1',id:'background',op:'get',options:{challenge:'AQ'}},location.origin)", 0);
        QSignalSpy admitted(binding, &FerricQuickCredentialBinding::requestReady);
        QTest::qWait(100);
        QCOMPARE(admitted.count(), 0);
        QVERIFY(!binding->hasCurrentLease());
        QTRY_COMPARE(run(fixture.owner->mainFrame(), "__ferricCredentialMediator.challenge('empty')", 42).toString(), QString());
        const auto operation = fixture.owner->captureOperation();
        bool selected = false;
        QVERIFY(QMetaObject::invokeMethod(ui.get(), "select_tab", Qt::DirectConnection,
            Q_RETURN_ARG(bool, selected), Q_ARG(int, 0)));
        QVERIFY(selected);
        QVERIFY(fixture.request());
        QCOMPARE(admitted.count(), 1);
        QVERIFY(binding->hasCurrentLease());
        QVERIFY(fixture.owner->isCurrent(operation));
        QCOMPARE(run(fixture.owner->mainFrame(), "window.backgroundDraft", 0).toString(), QStringLiteral("preserved"));
    }

    void lateBindingInstallsMediatorInTheExistingMainDocument()
    {
        OwnedQuickCase fixture;
        QTemporaryDir storage;
        auto ui = applicationOwner(fixture, storage);
        QVERIFY(ui);
        QVERIFY(fixture.open({}, false));
        QCOMPARE(run(fixture.owner->mainFrame(), "typeof __ferricCredentialMediator", 42).toString(), QStringLiteral("undefined"));
        const auto operation = fixture.owner->captureOperation();
        run(fixture.owner->mainFrame(), "window.lateBindingDraft='preserved'", 0);
        auto *binding = FerricQuickCredentialBinding::create(fixture.view, ui.get(), firstTab(ui.get()));
        QVERIFY(binding);
        fixture.boundary = binding->boundary();
        QTRY_COMPARE(run(fixture.owner->mainFrame(), "typeof __ferricCredentialMediator", 42).toString(), QStringLiteral("object"));
        QVERIFY(fixture.request());
        QVERIFY(binding->hasCurrentLease());
        QVERIFY(fixture.owner->isCurrent(operation));
        QCOMPARE(run(fixture.owner->mainFrame(), "window.lateBindingDraft", 0).toString(), QStringLiteral("preserved"));
    }

    void contextAndInstalledPolicyChangesRevokeCapturedCredentials()
    {
        OwnedQuickCase fixture;
        QTemporaryDir storage;
        auto ui = applicationOwner(fixture, storage, true);
        QVERIFY(ui);
        FerricQuickCredentialBinding *binding = nullptr;
        QVERIFY(fixture.open([&](QQuickItem *view) {
            binding = FerricQuickCredentialBinding::create(view, ui.get(), firstTab(ui.get()));
            return binding ? binding->boundary() : nullptr;
        }));
        QSignalSpy invalidated(binding, &FerricQuickCredentialBinding::leaseInvalidated);
        QVERIFY(fixture.request());
        bool accepted = false;
        QVERIFY(QMetaObject::invokeMethod(ui.get(), "assign_window_context", Qt::DirectConnection,
            Q_RETURN_ARG(bool, accepted), Q_ARG(QString, QStringLiteral("work"))));
        QVERIFY(accepted);
        QVERIFY(!binding->hasCurrentLease());
        QCOMPARE(invalidated.count(), 1);
        QVERIFY(!fixture.boundary->request());
        run(fixture.owner->mainFrame(), "true", 42);
        QVERIFY(fixture.request());
        QVERIFY(QMetaObject::invokeMethod(ui.get(), "reload_blocking_policy", Qt::DirectConnection, Q_RETURN_ARG(bool, accepted)));
        QVERIFY(accepted);
        QTRY_VERIFY(([&] {
            QMetaObject::invokeMethod(ui.get(), "poll_config", Qt::DirectConnection);
            return !binding->hasCurrentLease();
        })());
        QCOMPARE(invalidated.count(), 2);
        QVERIFY(!fixture.boundary->request());
    }

    void siblingPolicyChangeRevokesTheSharedProfilesCapturedRequest()
    {
        OwnedQuickCase fixture;
        QTemporaryDir storage;
        auto requester = applicationOwner(fixture, storage);
        QVERIFY(requester);
        QQmlComponent component(&fixture.engine);
        component.setData("import io.github.ferricbrowser 1.0\nBrowserUi {}", QUrl());
        std::unique_ptr<QObject> policyUi(component.create());
        QVERIFY(policyUi);
        FerricPopupOwnerBridge bridge;
        QVERIFY(bridge.inheritProfile(requester.get(), policyUi.get(), QStringLiteral("credential-fixture")));
        QTRY_VERIFY(([&] {
            QMetaObject::invokeMethod(policyUi.get(), "poll_config", Qt::DirectConnection);
            return !policyUi->property("profile_bootstrap_pending").toBool();
        })());
        QVERIFY(fixture.open({}, false));
        auto *profile = fixture.view->property("profile").value<QObject *>();
        auto *prototype = fixture.root->property("profilePrototype").value<QObject *>();
        QVERIFY(profile && prototype);
        FerricProfileLifetime lifetime;
        FerricBrowserRequestInterceptor interceptor;
        FerricQuickCredentialIntegration integration;
        QQuickWindow policyWindow;
        QVERIFY(lifetime.ownProfile(prototype, profile));
        QVERIFY(lifetime.ownResource(&interceptor));
        QVERIFY(interceptor.attach(profile));
        QVERIFY(lifetime.retainWindow(&policyWindow, policyUi.get()));
        QVERIFY(lifetime.activateWindow(&policyWindow));
        QVERIFY(lifetime.retainWindow(fixture.window, requester.get()));
        QVERIFY(lifetime.activateWindow(fixture.window));
        QCOMPARE(lifetime.policyUi(), policyUi.get());
        const auto tab = firstTab(requester.get());
        QString promptIdentity;
        QVERIFY(QMetaObject::invokeMethod(requester.get(), "new_prompt_view_identity", Qt::DirectConnection,
            Q_RETURN_ARG(QString, promptIdentity)));
        QVERIFY(QMetaObject::invokeMethod(requester.get(), "register_prompt_view_identity", Qt::DirectConnection,
            Q_ARG(int, 0), Q_ARG(QString, promptIdentity)));
        fixture.view->setProperty("viewUi", QVariant::fromValue(requester.get()));
        fixture.view->setProperty("stableTabId", tab);
        fixture.view->setProperty("promptLeaseIdentity", promptIdentity);
        QVERIFY(lifetime.adoptView(fixture.view, fixture.window, requester.get()));
        integration.setLifetime(&lifetime);
        integration.setRequestInterceptor(&interceptor);
        QVERIFY(integration.attachView(fixture.view));
        auto *binding = fixture.view->findChild<FerricQuickCredentialBinding *>(QString(), Qt::FindDirectChildrenOnly);
        QVERIFY(binding);
        fixture.boundary = binding->boundary();
        QSignalSpy invalidated(binding, &FerricQuickCredentialBinding::leaseInvalidated);
        QVERIFY(fixture.request());
        QVERIFY(binding->hasCurrentLease());
        const auto operation = fixture.owner->captureOperation();
        const auto generation = interceptor.policyGeneration();
        // The selected profile-policy owner changes the installed interceptor;
        // the requesting Ui's own policy epoch does not change in this call.
        ProfilePolicyProjection projection(policyUi.get(), &interceptor);
        QVERIFY(policyUi->setProperty("blocking_hosts", QStringList{QStringLiteral("blocked-fixture.invalid")}));
        QVERIFY(interceptor.policyGeneration() > generation);
        QVERIFY(!binding->hasCurrentLease());
        QCOMPARE(invalidated.count(), 1);
        QVERIFY(!fixture.boundary->request());
        QVERIFY(fixture.owner->isCurrent(operation));
        run(fixture.owner->mainFrame(), "true", 42);
        QVERIFY(fixture.request());
        QVERIFY(binding->hasCurrentLease());
    }

    void interceptorReplacementRequiresDetachAndStaleOwnerCannotClearIt()
    {
        OwnedQuickCase fixture;
        QVERIFY(fixture.open());
        auto *profile = fixture.view->property("profile").value<QObject *>();
        QVERIFY(profile);
        auto original = std::make_unique<FerricBrowserRequestInterceptor>();
        FerricBrowserRequestInterceptor replacement;
        QVERIFY(original->attach(profile));
        QVERIFY(!replacement.attach(profile));
        QVERIFY(original->attachedToProfile(profile));
        QVERIFY(!replacement.attachedToProfile(profile));
        QVERIFY(original->detach(profile));
        QVERIFY(replacement.attach(profile));
        QVERIFY(!original->attachedToProfile(profile));
        QVERIFY(!original->detach(profile));
        original.reset();
        QVERIFY(replacement.attachedToProfile(profile));
        replacement.setBlockedHosts({QStringLiteral("localhost")});
        replacement.setEnabled(true);
        QVERIFY(QMetaObject::invokeMethod(fixture.view, "reload"));
        QTRY_VERIFY(replacement.blockedCount() > 0);
        QVERIFY(replacement.detach(profile));
        QVERIFY(!replacement.attachedToProfile(profile));
    }

    void productionQuickRendererTerminationRevokesThePendingDocument()
    {
        OwnedQuickCase fixture;
        QVERIFY(fixture.open());
        QVERIFY(fixture.request());
        const auto operation = fixture.owner->captureOperation();
        const auto pid = fixture.view->property("renderProcessPid").toLongLong();
        QVERIFY(isOwnedProcess(pid));
        QSignalSpy cancelled(fixture.boundary, &FerricCredentialBoundary::cancelled);
        QCOMPARE(::kill(static_cast<pid_t>(pid), SIGKILL), 0);
        QTRY_VERIFY(!fixture.owner->isCurrent(operation));
        QTRY_VERIFY(cancelled.count() > 0);
        QVERIFY(!fixture.boundary->request());
        QVERIFY(!fixture.boundary->complete("{\"error\":\"AbortError\"}"));
    }

    void productionQuickProfileReplacementRetiresItsNativeBinding()
    {
        OwnedQuickCase fixture;
        QVERIFY(fixture.open());
        QVERIFY(fixture.request());
        const auto operation = fixture.owner->captureOperation();
        QSignalSpy cancelled(fixture.boundary, &FerricCredentialBoundary::cancelled);
        // Keep both the original and replacement profiles alive until after
        // the view's native WebContents has been destroyed.
        auto *replacement = new QQuickWebEngineProfile(fixture.view);
        QVERIFY(replacement);
        QVERIFY(replacement->isOffTheRecord());
        QVERIFY(replacement != fixture.view->property("profile").value<QObject *>());
        QVERIFY(QQmlProperty::write(fixture.view, "profile", QVariant::fromValue(replacement)));
        QCOMPARE(fixture.view->property("profile").value<QObject *>(), static_cast<QObject *>(replacement));
        QVERIFY(!fixture.owner->isCurrent(operation));
        QVERIFY(!fixture.owner->isReady());
        QVERIFY(cancelled.count() > 0);
        QVERIFY(!fixture.boundary->request());
        QVERIFY(!fixture.boundary->complete("{\"error\":\"AbortError\"}"));
    }

    void productionBoundaryUsesQuickOwnershipAndOwnedChoice()
    {
        FixtureServer server;
        QVERIFY(server.start());
        server.setDocuments("<!doctype html><title>Owned Quick boundary</title><iframe src='/frame'></iframe>",
                            "<!doctype html><title>Hostile frame</title>");
        QuickCredentialWitness witness;
        witness.fixturePort = server.pageUrl().port();
        QQmlEngine engine;
        QQmlComponent component(&engine);
        component.setData(source, QUrl("qrc:/owned-quick-boundary.qml"));
        std::unique_ptr<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        auto *window = qobject_cast<QQuickWindow *>(root.get());
        auto *view = qobject_cast<QQuickItem *>(root->property("view").value<QObject *>());
        QVERIFY(window && view);
        QVERIFY(connect(view, SIGNAL(certificateError(QWebEngineCertificateError)),
                        &witness, SLOT(certificateError(QWebEngineCertificateError))));
        auto *owner = FerricQuickCredentialOwner::create(view);
        QVERIFY(owner);
        QVERIFY(owner->presentationWindow());
        QCOMPARE(FerricQuickCredentialOwner::create(view), owner);
        QString context = "actual-quick-owner-fixture";
        auto *boundary = owner->prepareBoundary([&] { return context; });
        QVERIFY(boundary && boundary->isReady());
        QSignalSpy captured(boundary, &FerricCredentialBoundary::requestCaptured);
        QSignalSpy chosen(boundary, &FerricCredentialBoundary::providerChosen);
        QSignalSpy completed(boundary, &FerricCredentialBoundary::responseCompleted);
        window->show();
        QVERIFY(QTest::qWaitForWindowExposed(window));
        QVERIFY(QTest::qWaitForWindowActive(window));
        QVERIFY(view->setProperty("url", server.pageUrl()));
        QTRY_COMPARE_WITH_TIMEOUT(view->property("title").toString(), QStringLiteral("Owned Quick boundary"), 15000);
        QTRY_VERIFY(!view->property("loading").toBool());
        QVERIFY(owner->mainFrame().isMainFrame());
        run(owner->mainFrame(), "postMessage({type:'ferric.credentials.request.v1',id:'owned',op:'get',options:{challenge:'AA'}},location.origin)", 0);
        QTRY_COMPARE(captured.count(), 1);
        QVERIFY(boundary->request());
        QCOMPARE(boundary->request()->origin, server.pageUrl().adjusted(QUrl::RemovePath).toString());
        QVERIFY(!boundary->complete("{\"credential\":{}}"));
        QVERIFY(!boundary->complete("{\"native\":true}"));
        QCOMPARE(chosen.count(), 0);
        QVERIFY(boundary->showProviderChoice(true));
        QDialog *dialog = nullptr;
        for (auto *widget : QApplication::topLevelWidgets()) {
            if (widget->objectName() == "ferricCredentialProviderChoice") dialog = qobject_cast<QDialog *>(widget);
        }
        QVERIFY(dialog);
        QCOMPARE(dialog->windowHandle()->transientParent(), window);
        QVERIFY(QTest::qWaitForWindowActive(dialog));
        auto *phone = dialog->findChild<QPushButton *>("credentialPhoneProvider");
        QVERIFY(phone);
        QVERIFY(boundary->request());
        QVERIFY(dialog->isVisible());
        QVERIFY(dialog->windowHandle()->isActive());
        QVERIFY(phone->isEnabled());
        QTRY_COMPARE(QGuiApplication::focusWindow(), dialog->windowHandle());
        QTest::mouseClick(dialog->windowHandle(), Qt::LeftButton, Qt::NoModifier,
                         phone->mapTo(dialog, phone->rect().center()));
        QTRY_COMPARE(chosen.count(), 1);
        QVERIFY(chosen.first().first().toBool());
        // Terminate with an error: this proof never connects an authenticator.
        // A real asynchronous notification challenge is still in flight when
        // completion takes ownership. Its stale callback cannot poison the
        // next ceremony, and notifications arriving during delivery are held.
        QVERIFY(FerricCredentialBoundaryTestAccess::beginNotificationChallenge(boundary));
        QVERIFY(boundary->complete("{\"error\":\"NotAllowedError\"}"));
        boundary->notification();
        QTRY_COMPARE(completed.count(), 1);
        QVERIFY(completed.first().first().toBool());

        // Closing an owned chooser with Escape never chooses a provider.
        run(owner->mainFrame(), "postMessage({type:'ferric.credentials.request.v1',id:'cancel',op:'get',options:{challenge:'AA'}},location.origin)", 0);
        QTRY_COMPARE(captured.count(), 2);
        window->requestActivate();
        QTRY_COMPARE(QGuiApplication::focusWindow(), window);
        QVERIFY(boundary->showProviderChoice(true));
        QPointer<QDialog> cancelDialog;
        for (auto *widget : QApplication::topLevelWidgets()) {
            if (widget->objectName() == "ferricCredentialProviderChoice" && widget->isVisible())
                cancelDialog = qobject_cast<QDialog *>(widget);
        }
        QVERIFY(cancelDialog);
        QTRY_COMPARE(QGuiApplication::focusWindow(), cancelDialog->windowHandle());
        QTest::keyClick(cancelDialog->windowHandle(), Qt::Key_Escape);
        QTRY_COMPARE(completed.count(), 2);
        QCOMPARE(chosen.count(), 1);
        QVERIFY(!boundary->request());

        run(owner->mainFrame(), "postMessage({type:'ferric.credentials.request.v1',id:'stale',op:'get',options:{challenge:'AQ'}},location.origin)", 0);
        QTRY_COMPARE(captured.count(), 3);
        const auto operation = owner->captureOperation();
        const auto nonce = boundary->request()->documentNonce;
        QVERIFY(QMetaObject::invokeMethod(view, "reload"));
        QTRY_VERIFY(!owner->isCurrent(operation));
        QVERIFY(!boundary->request());
        QVERIFY(!boundary->complete("{\"error\":\"AbortError\"}"));
        QTRY_VERIFY(!view->property("loading").toBool());
        run(owner->mainFrame(), "postMessage({type:'ferric.credentials.request.v1',id:'fresh',op:'get',options:{challenge:'Ag'}},location.origin)", 0);
        QTRY_COMPARE(captured.count(), 4);
        QVERIFY(boundary->request());
        QVERIFY(nonce != boundary->request()->documentNonce);
        context = "retired-core-document-fixture";
        QVERIFY(!boundary->request());
        QVERIFY(!boundary->complete("{\"error\":\"AbortError\"}"));
        boundary->invalidate();
        QCOMPARE(run(owner->mainFrame(), "__ferricCredentialMediator.challenge('empty')", 42).toString(), QString());

        run(owner->mainFrame(), "postMessage({type:'ferric.credentials.request.v1',id:'hide',op:'get',options:{challenge:'Aw'}},location.origin)", 0);
        QTRY_COMPARE(captured.count(), 5);
        window->hide();
        QVERIFY(!boundary->request());
        QVERIFY(!boundary->complete("{\"error\":\"AbortError\"}"));

        window->show();
        window->requestActivate();
        QVERIFY(QTest::qWaitForWindowExposed(window));
        server.setCredentialPolicyAllowed(false);
        const auto beforePolicy = owner->captureOperation();
        QVERIFY(QMetaObject::invokeMethod(view, "reload"));
        QTRY_VERIFY(!owner->isCurrent(beforePolicy));
        QTRY_VERIFY(!view->property("loading").toBool());
        run(owner->mainFrame(), "postMessage({type:'ferric.credentials.request.v1',id:'denied',op:'get',options:{challenge:'BA'}},location.origin)", 0);
        QTest::qWait(100);
        const auto policy = QJsonDocument::fromJson(run(owner->mainFrame(),
            "__ferricCredentialMediator.challenge('denied-policy')", 42).toString().toUtf8()).object();
        QVERIFY(policy.contains("policyAllowed"));
        QVERIFY(!policy.value("policyAllowed").toBool());
        QCOMPARE(captured.count(), 5);
        QVERIFY(!boundary->request());

        server.setCredentialPolicyAllowed(true);
        const auto beforeAllowed = owner->captureOperation();
        QVERIFY(QMetaObject::invokeMethod(view, "reload"));
        QTRY_VERIFY(!owner->isCurrent(beforeAllowed));
        QTRY_VERIFY(!view->property("loading").toBool());
        run(owner->mainFrame(), "postMessage({type:'ferric.credentials.request.v1',id:'channel',op:'get',options:{challenge:'BQ'}},location.origin)", 0);
        QTRY_COMPARE(captured.count(), 6);
        QQmlComponent replacementComponent(&engine);
        replacementComponent.setData("import QtWebChannel; WebChannel {}", QUrl("qrc:/replacement-channel.qml"));
        auto *replacement = replacementComponent.create();
        QVERIFY(replacement);
        replacement->setParent(root.get());
        QVERIFY(QQmlProperty::write(view, "webChannel", QVariant::fromValue(replacement)));
        QVERIFY(!owner->isReady());
        QVERIFY(!boundary->request());
        QVERIFY(!boundary->complete("{\"error\":\"AbortError\"}"));
        const QPointer<FerricQuickCredentialOwner> lifetime(owner);
        root.reset();
        QVERIFY(lifetime.isNull());
    }

    void publicQuickSurfaceChallengesTheActualMainFrame()
    {
        FixtureServer server;
        QVERIFY(server.start());
        server.setDocuments("<!doctype html><title>Quick credential boundary</title><iframe src='/frame'></iframe>",
                            "<!doctype html><title>Hostile child</title>");
        QuickCredentialWitness witness;
        witness.fixturePort = server.pageUrl().port();
        QQmlEngine engine;
        std::unique_ptr<QObject> channelObject;
        QQmlComponent component(&engine);
        component.setData(source, QUrl("qrc:/quick-credential-fixture.qml"));
        const auto mediator = read("qwebchannel.js") + '\n' + read("mediator.js");
        QVERIFY(!mediator.isEmpty());
        std::unique_ptr<QObject> root(component.create());
        QVERIFY2(root, qPrintable(component.errorString()));
        auto *window = qobject_cast<QQuickWindow *>(root.get());
        auto *view = root->property("view").value<QObject *>();
        QQmlComponent channelComponent(&engine);
        channelComponent.setData("import QtWebChannel; WebChannel {}", QUrl("qrc:/native-owned-channel.qml"));
        channelObject.reset(channelComponent.create());
        auto *channel = qobject_cast<QWebChannel *>(channelObject.get());
        QVERIFY(window && view && channel);
        channel->registerObject("credentialNotification", &witness);
        QCOMPARE(channel->registeredObjects().size(), 1);
        QVERIFY(view->setProperty("webChannelWorld", 42u));
        QVERIFY(QQmlProperty::write(view, "webChannel", QVariant::fromValue(channelObject.get())));
        auto *scripts = view->property("userScripts").value<QObject *>();
        QVERIFY(scripts);
        QWebEngineScript script;
        script.setName("credential-private-fixture");
        script.setWorldId(42);
        script.setInjectionPoint(QWebEngineScript::DocumentCreation);
        script.setRunsOnSubFrames(false);
        script.setSourceCode(mediator);
        QVERIFY(QMetaObject::invokeMethod(scripts, "insert", Qt::DirectConnection, Q_ARG(QWebEngineScript, script)));
        QVERIFY(connect(view, SIGNAL(loadingChanged(QWebEngineLoadingInfo)),
                        &witness, SLOT(nativeLoadingChanged(QWebEngineLoadingInfo))));
        QVERIFY(connect(view, SIGNAL(certificateError(QWebEngineCertificateError)),
                        &witness, SLOT(certificateError(QWebEngineCertificateError))));
        window->show();
        QVERIFY(QTest::qWaitForWindowExposed(window));
        QVERIFY(view->setProperty("url", server.pageUrl()));
        QTRY_COMPARE_WITH_TIMEOUT(view->property("title").toString(), QStringLiteral("Quick credential boundary"), 15000);
        auto nativeProperty = view->property("mainFrame");
        QVERIFY(nativeProperty.canConvert<QWebEngineFrame>());
        witness.frame = nativeProperty.value<QWebEngineFrame>();
        QVERIFY(witness.frame.isValid());
        QVERIFY(witness.frame.isMainFrame());
        QVERIFY(witness.documentGeneration > 0);
        QCOMPARE(run(witness.frame, "typeof qt", 0).toString(), QStringLiteral("undefined"));
        QCOMPARE(run(witness.frame, "typeof qt", 1).toString(), QStringLiteral("undefined"));
        run(witness.frame, "window.qt={webChannelTransport:{send(){}}};postMessage({type:'ferric.credentials.request.v1',id:'quick',op:'get',options:{challenge:'AA'}},location.origin)", 0);
        QTRY_COMPARE(witness.notifications, 1);
        const auto first = QJsonDocument::fromJson(run(witness.frame,
            "__ferricCredentialMediator.challenge('native-challenge')", 42).toString().toUtf8()).object();
        QCOMPARE(first.value("challenge").toString(), QStringLiteral("native-challenge"));
        QCOMPARE(first.value("origin").toString(), server.pageUrl().adjusted(QUrl::RemovePath).toString());
        QVERIFY(first.value("secureContext").toBool());
        QVERIFY(first.value("policyAllowed").toBool());
        QVERIFY(!first.value("documentNonce").toString().isEmpty());
        QVERIFY(!witness.frame.children().isEmpty());
        run(witness.frame.children().first(),
            "parent.postMessage({type:'ferric.credentials.request.v1',id:'child',op:'get',options:{}},'*')", 0);
        QTest::qWait(50);
        QCOMPARE(witness.notifications, 1);
        const auto generation = witness.documentGeneration;
        QVERIFY(QMetaObject::invokeMethod(view, "reload"));
        QTRY_VERIFY(witness.documentGeneration > generation);
        QTRY_VERIFY(!view->property("loading").toBool());
        nativeProperty = view->property("mainFrame");
        QVERIFY(nativeProperty.canConvert<QWebEngineFrame>());
        witness.frame = nativeProperty.value<QWebEngineFrame>();
        QVERIFY(witness.frame.isMainFrame());
        run(witness.frame, "postMessage({type:'ferric.credentials.request.v1',id:'fresh',op:'get',options:{challenge:'AQ'}},location.origin)", 0);
        QTRY_COMPARE(witness.notifications, 2);
        const auto fresh = QJsonDocument::fromJson(run(witness.frame,
            "__ferricCredentialMediator.challenge('fresh')", 42).toString().toUtf8()).object();
        QVERIFY(first.value("documentNonce") != fresh.value("documentNonce"));
        QVERIFY(first.value("requestNonce") != fresh.value("requestNonce"));
        QCOMPARE(run(witness.frame,
            QString("__ferricCredentialMediator.complete('%1','%2','%3',{error:'AbortError'})")
                .arg(first.value("requestNonce").toString(), first.value("documentNonce").toString(), first.value("origin").toString()), 42).toBool(), false);
    }
};

int main(int argc, char **argv)
{
    QtWebEngineQuick::initialize();
    QApplication application(argc, argv);
    if (!cxx_qt_init_crate_ferric_browser_engine_qt()) return 1;
    QuickCredentialSurfaceTests tests;
    return QTest::qExec(&tests, argc, argv);
}

#include "tst_quick_credential_surface.moc"
