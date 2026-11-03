#include "../credential_https_fixture.h"
#include "quick_credential_owner.h"
#include "credential_boundary.h"
#include "quick_credential_binding.h"
#include "quick_credential_integration.h"
#include "phone_credential_actor.h"
#include "credential_network_policy.h"
#include "profile_lifetime.h"
#include "request_interceptor.h"
#include "popup_owner_bridge.h"

#include <QApplication>
#include <QCloseEvent>
#include <QDialog>
#include <QDialogButtonBox>
#include <QDir>
#include <QLabel>
#include <QNetworkProxy>
#include <QNetworkProxyFactory>
#include <QScopeGuard>
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
#include <utility>

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
    int certificateErrors = 0;
    Q_INVOKABLE void notify() { ++notifications; }
public slots:
    void certificateError(QWebEngineCertificateError error) {
        ++certificateErrors;
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
import io.github.ferricbrowser 1.0
Window {
    id: root
    visible: false; width: 640; height: 420
    property alias view: content
    property alias profilePrototype: prototype
    property var fixtureProfile: null
    property var fixtureTrustFiles: []
    Component.onCompleted: fixtureProfile = prototype.instance()
    WebEngineView {
        // The profile factory must outlive WebContents teardown. Keeping it
        // under the view also gives standalone fixture roots that ordering.
        FerricProfilePrototype { id: prototype; storageName: ""; additionalTrustedCertificateFiles: root.fixtureTrustFiles }
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
    bool nativeProfileFactory = true;

    bool open(std::function<FerricCredentialBoundary *(QQuickItem *)> makeBoundary = {}, bool installBeforeLoad = true, bool trusted = false, int httpStatus = 200) {
        if (!server.start()) return false;
        server.setDocuments("<!doctype html><title>Quick lifetime</title>", "<!doctype html>");
        server.setHttpStatus(httpStatus);
        witness.fixturePort = server.pageUrl().port();
        QQmlComponent component(&engine);
        component.setData(nativeProfileFactory ? QByteArray(source) : QString::fromUtf8(source).replace("FerricProfilePrototype", "WebEngineProfilePrototype").toUtf8(), QUrl("qrc:/quick-lifetime-fixture.qml"));
        root.reset(trusted ? component.createWithInitialProperties({{QStringLiteral("fixtureTrustFiles"), QVariant::fromValue(QList<QUrl>{QUrl::fromLocalFile(server.certificateFile())})}}) : component.create());
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
    void phoneRouteAllowsOnlyTheCapturedOriginsExactLoopbackMapping()
    {
        using namespace FerricCredentialNetworkPolicy;
        const QUrl origin(QStringLiteral("https://passkeys.ferric.test:8443"));
        const QString exact = QStringLiteral("--host-resolver-rules=MAP passkeys.ferric.test 127.0.0.1");
        QVERIFY(argumentsAllowDirect({exact}, origin));
        for (const auto &argument : {QStringLiteral("--host-resolver-rules=MAP * 127.0.0.1"),
                exact + QStringLiteral(", MAP cable.auth.com 127.0.0.1"),
                QStringLiteral("--host-resolver-rules=MAP other.test 127.0.0.1"),
                QStringLiteral("--host-resolver-rules=MAP passkeys.ferric.test 127.0.0.2"),
                QStringLiteral("--host-resolver-rules=MAP passkeys.ferric.test:8443 127.0.0.1"),
                QStringLiteral("--host-rules=MAP * 127.0.0.1"), QStringLiteral("--proxy-server=localhost:9"),
                QStringLiteral("--dns-over-https-servers=https://localhost")})
            QVERIFY(!argumentsAllowDirect({argument}, origin));
        QVERIFY(!argumentsAllowDirect({exact, exact}, origin));
        QVERIFY(!argumentsAllowDirect({QStringLiteral("--host-resolver-rules=MAP cable.auth.com 127.0.0.1")}, QUrl("https://cable.auth.com")));
        QVERIFY(!argumentsAllowDirect({QStringLiteral("--host-resolver-rules=MAP cable.ua5v.com 127.0.0.1")}, QUrl("https://cable.ua5v.com")));
        const QStringList argv{"browser", "--webEngineArgs", exact};
        QCOMPARE(*chromiumArguments(argv, std::nullopt), QStringList{exact});
        QCOMPARE(*chromiumArguments(argv, QString()), QStringList{});
        QCOMPARE(*chromiumArguments(argv, QStringLiteral("--disable-gpu --host-resolver-rules=\"MAP passkeys.ferric.test 127.0.0.1\"")),
                 (QStringList{"--disable-gpu", exact}));
        QVERIFY(!chromiumArguments(argv, QStringLiteral("--host-resolver-rules=\"MAP passkeys.ferric.test 127.0.0.1")));
    }

    void uiDestructionCanSynchronouslyDeleteItsBinding()
    {
        OwnedQuickCase fixture;
        QTemporaryDir storage;
        auto ui = applicationOwner(fixture, storage);
        QVERIFY(ui);
        QPointer<FerricQuickCredentialBinding> binding;
        QVERIFY(fixture.open([&](QQuickItem *view) {
            binding = FerricQuickCredentialBinding::create(view, ui.get(), firstTab(ui.get()));
            return binding ? binding->boundary() : nullptr;
        }));
        QVERIFY(fixture.request());
        QVERIFY(binding->hasCurrentLease());
        connect(binding, &FerricQuickCredentialBinding::leaseInvalidated, fixture.window,
                [&] { delete binding.data(); });
        ui.reset();
        QVERIFY(!binding);
        QVERIFY(!fixture.boundary->request());
        QVERIFY(fixture.owner->isReady());
    }

    void cancellationCanSynchronouslyDeleteItsView_data()
    {
        QTest::addColumn<bool>("alreadyCaptured");
        QTest::newRow("captured") << true;
        QTest::newRow("before-capture") << false;
    }

    void cancellationCanSynchronouslyDeleteItsView()
    {
        QFETCH(bool, alreadyCaptured);
        OwnedQuickCase fixture;
        QVERIFY(fixture.open());
        QPointer<QQuickItem> view(fixture.view);
        QPointer<FerricQuickCredentialOwner> owner(fixture.owner);
        int cancellations = 0;
        connect(fixture.boundary, &FerricCredentialBoundary::cancelRequested, fixture.window, [&] {
            ++cancellations;
            delete view.data();
        });
        if (alreadyCaptured) QVERIFY(fixture.request());
        const QString request = alreadyCaptured ? QString() : QStringLiteral(
            "postMessage({type:'ferric.credentials.request.v1',id:'lifetime',op:'get',options:{challenge:'AQ'}},location.origin);");
        run(fixture.owner->mainFrame(), request + QStringLiteral(
            "postMessage({type:'ferric.credentials.cancel.v1',id:'lifetime'},location.origin)"), 0);
        QTRY_COMPARE(cancellations, 1);
        QVERIFY(!view);
        QVERIFY(!owner);
    }

    void nativeFactoryPublishesUniqueProfilesAndDeclinesDuplicateStorage()
    {
        QQmlEngine engine;
        QQmlComponent component(&engine);
        component.setData("import io.github.ferricbrowser 1.0; FerricProfilePrototype {}", QUrl("qrc:/native-factory-case.qml"));
        std::unique_ptr<QObject> first(component.create()), second(component.create());
        QVERIFY2(first && second, qPrintable(component.errorString()));
        QObject *one = nullptr, *two = nullptr, *again = nullptr;
        QVERIFY(QMetaObject::invokeMethod(first.get(), "instance", Qt::DirectConnection, Q_RETURN_ARG(QObject *, one)));
        QVERIFY(QMetaObject::invokeMethod(second.get(), "instance", Qt::DirectConnection, Q_RETURN_ARG(QObject *, two)));
        QVERIFY(one && two && one != two);
        QVERIFY(QMetaObject::invokeMethod(first.get(), "instance", Qt::DirectConnection, Q_RETURN_ARG(QObject *, again)));
        QCOMPARE(again, one);
        QTemporaryDir root;
        QVERIFY(root.isValid());
        const QVariantMap settings{{"storageName", "native-factory-fixture"}, {"persistentStoragePath", root.filePath("data")}, {"cachePath", root.filePath("cache")}};
        std::unique_ptr<QObject> persistent(component.createWithInitialProperties(settings)), duplicate(component.createWithInitialProperties(settings));
        QVERIFY(persistent && duplicate);
        QObject *stored = nullptr, *declined = nullptr;
        QVERIFY(QMetaObject::invokeMethod(persistent.get(), "instance", Qt::DirectConnection, Q_RETURN_ARG(QObject *, stored)));
        QVERIFY(stored);
        QVERIFY(QMetaObject::invokeMethod(duplicate.get(), "instance", Qt::DirectConnection, Q_RETURN_ARG(QObject *, declined)));
        QVERIFY(!declined);
    }

    void importedProfileWithDestroyedUnobservedViewStaysDenied()
    {
        OwnedQuickCase fixture;
        fixture.nativeProfileFactory = false;
        QVERIFY(fixture.open());
        QVERIFY(fixture.witness.certificateErrors > 0);
        QPointer<QObject> profile(fixture.view->property("profile").value<QObject *>());
        QPointer<QObject> prototype(fixture.root->property("profilePrototype").value<QObject *>());
        QVERIFY(profile && prototype);
        prototype->setParent(fixture.root.get());
        // Qt creates this profile with the prototype's original parent (the
        // view), while also retaining scoped ownership in the prototype.
        // Preserve both before destroying the deliberately unobserved view.
        profile->setParent(prototype);
        QQmlEngine::setObjectOwnership(profile, QQmlEngine::CppOwnership);
        delete fixture.view;
        fixture.view = nullptr; fixture.owner = nullptr; fixture.boundary = nullptr;
        QVERIFY(profile);
        QObject ui;
        FerricProfileLifetime lifetime;
        QVERIFY(lifetime.ownProfile(prototype, profile));
        QQmlComponent component(&fixture.engine);
        component.setData(R"QML(import QtQuick; import QtWebEngine
            Window { width:640; height:420; property var sharedProfile; property alias view:content;
                WebEngineView { id:content; anchors.fill:parent; url:""; profile:sharedProfile }
            })QML", QUrl("qrc:/imported-tls-profile.qml"));
        std::unique_ptr<QObject> root(component.createWithInitialProperties({{QStringLiteral("sharedProfile"), QVariant::fromValue(profile.data())}}));
        QVERIFY2(root, qPrintable(component.errorString()));
        auto *window = qobject_cast<QQuickWindow *>(root.get());
        auto *view = qobject_cast<QQuickItem *>(root->property("view").value<QObject *>());
        QVERIFY(window && view);
        QuickCredentialWitness witness;
        witness.fixturePort = fixture.server.pageUrl().port();
        QVERIFY(connect(view, SIGNAL(certificateError(QWebEngineCertificateError)), &witness, SLOT(certificateError(QWebEngineCertificateError))));
        QVERIFY(lifetime.retainWindow(window, &ui));
        QVERIFY(lifetime.activateWindow(window));
        QVERIFY(lifetime.adoptView(view, window, &ui));
        window->show();
        QVERIFY(QTest::qWaitForWindowExposed(window));
        QVERIFY(view->setProperty("url", fixture.server.pageUrl()));
        QTRY_COMPARE(view->property("title").toString(), QStringLiteral("Quick lifetime"));
        QTRY_VERIFY(!view->property("loading").toBool());
        QCOMPARE(witness.certificateErrors, 0);
        QVERIFY(!lifetime.credentialTlsEligible(view, fixture.server.pageUrl()));
        const auto beforeReload = lifetime.credentialTlsIdentity(view);
        QVERIFY(QMetaObject::invokeMethod(view, "reload"));
        QTRY_VERIFY(lifetime.credentialTlsIdentity(view) != beforeReload);
        QTRY_VERIFY(!view->property("loading").toBool());
        QVERIFY(!lifetime.credentialTlsEligible(view, fixture.server.pageUrl()));
    }

    void nativeTlsProofRequiresObservedTrustedProfile_data()
    {
        QTest::addColumn<bool>("trusted");
        QTest::addColumn<bool>("late");
        QTest::addColumn<int>("httpStatus");
        QTest::newRow("trusted-profile-ca") << true << false << 200;
        QTest::newRow("trusted-http-401") << true << false << 401;
        QTest::newRow("trusted-http-404") << true << false << 404;
        QTest::newRow("accepted-bad-certificate") << false << false << 200;
        QTest::newRow("unobserved-profile") << true << true << 200;
    }

    void nativeTlsProofRequiresObservedTrustedProfile()
    {
        QFETCH(bool, trusted);
        QFETCH(bool, late);
        QFETCH(int, httpStatus);
        OwnedQuickCase fixture;
        QObject ui;
        FerricProfileLifetime lifetime;
        const auto registerView = [&](QQuickItem *view) {
            auto *profile = view->property("profile").value<QObject *>();
            auto *prototype = fixture.root->property("profilePrototype").value<QObject *>();
            return lifetime.ownProfile(prototype, profile) && lifetime.retainWindow(fixture.window, &ui)
                && lifetime.activateWindow(fixture.window) && lifetime.adoptView(view, fixture.window, &ui);
        };
        QVERIFY(fixture.open([&](QQuickItem *view) {
            if (!late && !registerView(view)) return static_cast<FerricCredentialBoundary *>(nullptr);
            return fixture.owner->prepareBoundary([&] { return fixture.context; });
        }, true, trusted, httpStatus));
        if (late) QVERIFY(registerView(fixture.view));
        QVERIFY(!lifetime.credentialTlsIdentity(fixture.view).isEmpty());
        QCOMPARE(fixture.witness.certificateErrors == 0, trusted);
        QVERIFY(run(fixture.owner->mainFrame(), "isSecureContext", 0).toBool());
        const auto requestOrigin = fixture.server.pageUrl().adjusted(QUrl::RemovePath);
        if (trusted && !late) QTRY_VERIFY(lifetime.credentialTlsEligible(fixture.view, requestOrigin));
        else QVERIFY(!lifetime.credentialTlsEligible(fixture.view, requestOrigin));
        QVERIFY(!lifetime.credentialTlsEligible(fixture.view, QUrl("https://other.example")));
        const auto previous = lifetime.credentialTlsIdentity(fixture.view);
        QVERIFY(QMetaObject::invokeMethod(fixture.view, "reload"));
        QTRY_VERIFY(lifetime.credentialTlsIdentity(fixture.view) != previous);
        QTRY_VERIFY(!fixture.view->property("loading").toBool());
        if (trusted && !late) QTRY_VERIFY(lifetime.credentialTlsEligible(fixture.view, requestOrigin));
        else QVERIFY(!lifetime.credentialTlsEligible(fixture.view, requestOrigin));
    }

    void nativeTlsTaintSurvivesSiblingPorts()
    {
        OwnedQuickCase fixture;
        QObject ui;
        FerricProfileLifetime lifetime;
        QVERIFY(fixture.open([&](QQuickItem *view) {
            auto *profile = view->property("profile").value<QObject *>();
            auto *prototype = fixture.root->property("profilePrototype").value<QObject *>();
            if (!lifetime.ownProfile(prototype, profile) || !lifetime.retainWindow(fixture.window, &ui)
                || !lifetime.activateWindow(fixture.window) || !lifetime.adoptView(view, fixture.window, &ui)) return static_cast<FerricCredentialBoundary *>(nullptr);
            return fixture.owner->prepareBoundary([&] { return fixture.context; });
        }));
        QVERIFY(fixture.witness.certificateErrors > 0);
        QVERIFY(!lifetime.credentialTlsEligible(fixture.view, fixture.server.pageUrl()));
        FixtureServer siblingServer;
        QVERIFY(siblingServer.start(&fixture.server));
        siblingServer.setDocuments("<!doctype html><title>Sibling exception reuse</title>", "<!doctype html>");
        QQmlComponent component(&fixture.engine);
        component.setData(R"QML(import QtQuick; import QtWebEngine
            Window { width:640; height:420; property var sharedProfile; property alias view:content;
                WebEngineView { id:content; anchors.fill:parent; url:""; profile:sharedProfile }
            })QML", QUrl("qrc:/tls-sibling.qml"));
        std::unique_ptr<QObject> sibling(component.createWithInitialProperties({{QStringLiteral("sharedProfile"), fixture.view->property("profile")}}));
        QVERIFY2(sibling, qPrintable(component.errorString()));
        auto *window = qobject_cast<QQuickWindow *>(sibling.get());
        auto *view = qobject_cast<QQuickItem *>(sibling->property("view").value<QObject *>());
        QVERIFY(window && view);
        QuickCredentialWitness witness;
        witness.fixturePort = siblingServer.pageUrl().port();
        QVERIFY(connect(view, SIGNAL(certificateError(QWebEngineCertificateError)), &witness, SLOT(certificateError(QWebEngineCertificateError))));
        QVERIFY(lifetime.retainWindow(window, &ui));
        QVERIFY(lifetime.activateWindow(window));
        QVERIFY(lifetime.adoptView(view, window, &ui));
        window->show();
        QVERIFY(QTest::qWaitForWindowExposed(window));
        QVERIFY(view->setProperty("url", siblingServer.pageUrl()));
        QTRY_COMPARE(view->property("title").toString(), QStringLiteral("Sibling exception reuse"));
        QTRY_VERIFY(!view->property("loading").toBool());
        // The same bad certificate is silently reused at a different port.
        QCOMPARE(witness.certificateErrors, 0);
        QVERIFY(!lifetime.credentialTlsEligible(view, siblingServer.pageUrl()));
        QVERIFY(!lifetime.credentialTlsEligible(fixture.view, fixture.server.pageUrl()));
    }

    void phoneTransportAdmissionHonorsInstalledPolicy()
    {
        FerricBrowserRequestInterceptor interceptor;
        const QUrl origin("https://login.example.com");
        QVERIFY(interceptor.phoneTransportAllowed(origin));
        interceptor.setEnabled(true);
        interceptor.setBlockedHosts({"cable.auth.com"});
        QVERIFY(!interceptor.phoneTransportAllowed(origin));
        interceptor.setExceptionHosts({"cable.auth.com"});
        QVERIFY(interceptor.phoneTransportAllowed(origin));
        interceptor.setSecurityDenyHosts({"*.auth.com"});
        QVERIFY(!interceptor.phoneTransportAllowed(origin));
        interceptor.setBypassSites({"login.example.com"});
        QVERIFY(!interceptor.phoneTransportAllowed(origin));
        interceptor.setSecurityDenyHosts({});
        QVERIFY(interceptor.phoneTransportAllowed(origin));
        QVERIFY(!interceptor.phoneTransportAllowed(QUrl("http://login.example.com")));
    }

    void nativeActorOwnsShimProviderCancellationAndDisable()
    {
        OwnedQuickCase fixture;
        QTemporaryDir storage;
        auto ui = applicationOwner(fixture, storage);
        QVERIFY(ui);
        QVERIFY(fixture.open({}, false));
        run(fixture.owner->mainFrame(), R"JS(
            window.actorDraft='keep this document';
            window.nativeStub=function(options) {
                window.nativeReceiverCorrect=this===navigator.credentials;
                window.nativeOptionsCorrect=options===window.actorOptions;
                return Promise.resolve('native-fixture');
            };
            CredentialsContainer.prototype.get=window.nativeStub;
        )JS", 0);
        const auto proxy = QNetworkProxy::applicationProxy();
        const auto systemProxy = QNetworkProxyFactory::usesSystemConfiguration();
        const auto restoreProxy = qScopeGuard([&] {
            if (systemProxy) QNetworkProxyFactory::setUseSystemConfiguration(true);
            else QNetworkProxy::setApplicationProxy(proxy);
        });
        QNetworkProxy::setApplicationProxy(QNetworkProxy(QNetworkProxy::HttpProxy, "127.0.0.1", 9));
        FerricProfileLifetime lifetime;
        FerricBrowserRequestInterceptor interceptor;
        FerricQuickCredentialIntegration integration;
        auto *profile = fixture.view->property("profile").value<QObject *>();
        auto *prototype = fixture.root->property("profilePrototype").value<QObject *>();
        QVERIFY(lifetime.ownProfile(prototype, profile));
        QVERIFY(lifetime.ownResource(&interceptor));
        QVERIFY(interceptor.attach(profile));
        QVERIFY(lifetime.retainWindow(fixture.window, ui.get()));
        QVERIFY(lifetime.activateWindow(fixture.window));
        QString promptIdentity;
        QVERIFY(QMetaObject::invokeMethod(ui.get(), "new_prompt_view_identity", Qt::DirectConnection, Q_RETURN_ARG(QString, promptIdentity)));
        QVERIFY(QMetaObject::invokeMethod(ui.get(), "register_prompt_view_identity", Qt::DirectConnection, Q_ARG(int, 0), Q_ARG(QString, promptIdentity)));
        fixture.view->setProperty("viewUi", QVariant::fromValue(ui.get()));
        fixture.view->setProperty("stableTabId", firstTab(ui.get()));
        fixture.view->setProperty("promptLeaseIdentity", promptIdentity);
        QVERIFY(lifetime.adoptView(fixture.view, fixture.window, ui.get()));
        integration.setLifetime(&lifetime);
        integration.setRequestInterceptor(&interceptor);
        QVERIFY(integration.attachView(fixture.view));
        auto *binding = fixture.view->findChild<FerricQuickCredentialBinding *>(QString(), Qt::FindDirectChildrenOnly);
        QVERIFY(binding);
        fixture.boundary = binding->boundary();
        FerricPhoneCredentialActor::attach(binding);
        FerricPhoneCredentialActor::attach(binding);
        QTRY_VERIFY(!run(fixture.owner->mainFrame(), "navigator.credentials.get===nativeStub", 0).toBool());
        const auto request = [&] {
            fixture.window->requestActivate();
            if (!QTest::qWaitFor([&] { return QGuiApplication::focusWindow() == fixture.window; }, 5000)) return false;
            run(fixture.owner->mainFrame(), R"JS(
                window.actorResult='pending';
                window.actorOptions={publicKey:{challenge:new Uint8Array([1,2,3]),timeout:20000}};
                navigator.credentials.get(actorOptions).then(value=>actorResult=value,error=>actorResult=error.name);
            )JS", 0);
            return true;
        };
        const auto chooser = []() -> QDialog * {
            for (auto *widget : QApplication::topLevelWidgets())
                if (auto *dialog = qobject_cast<QDialog *>(widget); dialog && dialog->isVisible()
                    && dialog->windowTitle() == "Choose an authenticator") return dialog;
            return nullptr;
        };
        QVERIFY(request());
        QTRY_VERIFY(chooser());
        QPointer<QDialog> dialog(chooser());
        QCOMPARE(dialog->windowHandle()->transientParent(), fixture.window);
        auto *phone = dialog->findChild<QPushButton *>("credentialPhoneProvider");
        QVERIFY(phone && !phone->isEnabled());
        dialog->windowHandle()->requestActivate();
        QTRY_COMPARE(QGuiApplication::focusWindow(), dialog->windowHandle());
        auto *buttons = dialog->findChild<QDialogButtonBox *>();
        QVERIFY(buttons && buttons->button(QDialogButtonBox::Cancel));
        QTest::mouseClick(buttons->button(QDialogButtonBox::Cancel), Qt::LeftButton);
        QTRY_COMPARE(run(fixture.owner->mainFrame(), "actorResult", 0).toString(), QStringLiteral("NotAllowedError"));
        QVERIFY(!binding->hasCurrentLease());
        QVERIFY(request());
        QTRY_VERIFY(chooser());
        dialog = chooser();
        dialog->windowHandle()->requestActivate();
        QTRY_COMPARE(QGuiApplication::focusWindow(), dialog->windowHandle());
        QTest::mouseClick(dialog->findChild<QPushButton *>("credentialNativeProvider"), Qt::LeftButton);
        QTRY_COMPARE(run(fixture.owner->mainFrame(), "actorResult", 0).toString(), QStringLiteral("native-fixture"));
        QVERIFY(run(fixture.owner->mainFrame(), "nativeReceiverCorrect&&nativeOptionsCorrect", 0).toBool());
        QVERIFY(request());
        QTRY_VERIFY(chooser());
        integration.setLifetime(nullptr);
        QTRY_COMPARE(run(fixture.owner->mainFrame(), "actorResult", 0).toString(), QStringLiteral("AbortError"));
        QTRY_VERIFY(run(fixture.owner->mainFrame(), "navigator.credentials.get===nativeStub", 0).toBool());
        QTRY_VERIFY(!chooser());
        integration.setLifetime(&lifetime);
        QVERIFY(integration.attachView(fixture.view));
        QTRY_VERIFY(!run(fixture.owner->mainFrame(), "navigator.credentials.get===nativeStub", 0).toBool());
        QCOMPARE(run(fixture.owner->mainFrame(), "actorDraft", 0).toString(), QStringLiteral("keep this document"));
    }

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

struct PhoneActorCase {
    OwnedQuickCase fixture;
    QTemporaryDir storage;
    std::unique_ptr<QObject> ui;
    FerricProfileLifetime lifetime;
    FerricBrowserRequestInterceptor interceptor;
    std::unique_ptr<FerricQuickCredentialIntegration> integration = std::make_unique<FerricQuickCredentialIntegration>();
    QPointer<FerricQuickCredentialBinding> binding;

    bool open() {
        ui = applicationOwner(fixture, storage);
        if (!ui) return false;
        return fixture.open([this](QQuickItem *view) -> FerricCredentialBoundary * {
            auto *profile = view->property("profile").value<QObject *>();
            auto *prototype = fixture.root->property("profilePrototype").value<QObject *>();
            if (!lifetime.ownProfile(prototype, profile) || !lifetime.ownResource(&interceptor)
                || !interceptor.attach(profile) || !lifetime.retainWindow(fixture.window, ui.get())
                || !lifetime.activateWindow(fixture.window)) return nullptr;
            interceptor.setEnabled(true);
            QString identity;
            if (!QMetaObject::invokeMethod(ui.get(), "new_prompt_view_identity", Qt::DirectConnection, Q_RETURN_ARG(QString, identity))
                || !QMetaObject::invokeMethod(ui.get(), "register_prompt_view_identity", Qt::DirectConnection, Q_ARG(int, 0), Q_ARG(QString, identity))) return nullptr;
            view->setProperty("viewUi", QVariant::fromValue(ui.get()));
            view->setProperty("stableTabId", firstTab(ui.get()));
            view->setProperty("promptLeaseIdentity", identity);
            if (!lifetime.adoptView(view, fixture.window, ui.get())) return nullptr;
            integration->setLifetime(&lifetime);
            integration->setRequestInterceptor(&interceptor);
            if (!integration->attachView(view)) return nullptr;
            binding = view->findChild<FerricQuickCredentialBinding *>(QString(), Qt::FindDirectChildrenOnly);
            return binding ? binding->boundary() : nullptr;
        }, true, true);
    }

    bool request(int scenario, int timeout = 10000) {
        fixture.window->requestActivate();
        if (!QTest::qWaitFor([&] { return QGuiApplication::focusWindow() == fixture.window; }, 5000)) return false;
        run(fixture.owner->mainFrame(), QString(R"JS(
            window.phoneDone=false; window.phoneId=''; window.phoneError=''; window.phoneCount=0;
            window.phoneController=new AbortController();
            const create=%1===17 || %1===18;
            const publicKey=create ? {challenge:new Uint8Array([%1]),rp:{id:'localhost',name:'Fixture'},
                user:{id:new Uint8Array([1]),name:'fixture',displayName:'Fixture'},pubKeyCredParams:[{type:'public-key',alg:-7}],
                excludeCredentials:%1===18 ? [{type:'public-key',id:new Uint8Array([1])}] : [],timeout:%2}
                : {challenge:new Uint8Array([%1]),rpId:'localhost',timeout:%2};
            navigator.credentials[create?'create':'get']({publicKey,signal:phoneController.signal})
              .then(value=>{phoneId=value.id;window.phoneIDL=value instanceof PublicKeyCredential
                    && value.response instanceof (create?AuthenticatorAttestationResponse:AuthenticatorAssertionResponse)
                    && value.toJSON().id===value.id
                    && (!create || (value.response.getAuthenticatorData().byteLength===37
                        && value.response.getPublicKey()===null && value.response.getPublicKeyAlgorithm()===-7
                        && value.response.getTransports().join(',')==='hybrid'
                        && value.toJSON().response.attestationObject==='oA'));
                    phoneCount++;phoneDone=true},
                    error=>{phoneError=error.name;phoneCount++;phoneDone=true});
        )JS").arg(scenario).arg(timeout), 0);
        return true;
    }
    QVariant value(const QString &expression) { return run(fixture.owner->mainFrame(), expression, 0); }
};

QDialog *visibleCredentialDialog(const QString &title) {
    for (auto *widget : QApplication::topLevelWidgets())
        if (auto *dialog = qobject_cast<QDialog *>(widget); dialog && dialog->isVisible() && dialog->windowTitle() == title) return dialog;
    return nullptr;
}

QList<QJsonObject> helperReceipts() {
    QFile file(QString::fromLocal8Bit(qgetenv("FERRIC_PHONE_ACTOR_RECEIPT")));
    if (!file.open(QIODevice::ReadOnly)) return {};
    QList<QJsonObject> rows;
    for (const auto &line : file.readAll().split('\n')) {
        const auto document = QJsonDocument::fromJson(line);
        if (document.isObject()) rows.append(document.object());
    }
    return rows;
}

bool helperReceiptsReaped(int offset) {
    const auto rows = helperReceipts();
    for (int index = offset; index < rows.size(); ++index) {
        const auto row = rows[index];
        QFile stat(QStringLiteral("/proc/%1/stat").arg(row.value("pid").toInteger()));
        if (stat.open(QIODevice::ReadOnly) && QString::fromLatin1(stat.readAll().split(' ').value(21)) == row.value("start").toString()) return false;
    }
    return true;
}

class PhoneCredentialActorTests final : public QObject {
    Q_OBJECT
private slots:
    void nativeHelperPipeline_data() {
        QTest::addColumn<int>("scenario");
        for (const auto &[name, scenario] : {std::pair{"success",1}, {"account-choice",2}, {"late-exit",3},
                {"trailing-output",4}, {"nonzero-exit",5}, {"abort",6}, {"stale-id",7}, {"security-error",8},
                {"route-before-started",9}, {"installed-policy",10}, {"rejected-close",11}, {"disable",12}, {"global-busy",13},
                {"late-account-choice",14}, {"duplicate-progress",15}, {"synchronous-cancellation",16},
                {"create-success",17}, {"create-excluded",18}, {"changed-startup-flags",19}})
            QTest::newRow(name) << scenario;
    }

    void nativeHelperPipeline() {
        QFETCH(int, scenario);
        const auto proxy = QNetworkProxy::applicationProxy();
        const auto system = QNetworkProxyFactory::usesSystemConfiguration();
        const auto flags = qgetenv("QTWEBENGINE_CHROMIUM_FLAGS");
        const auto restore = qScopeGuard([&] {
            if (flags.isNull()) qunsetenv("QTWEBENGINE_CHROMIUM_FLAGS");
            else qputenv("QTWEBENGINE_CHROMIUM_FLAGS", flags);
            if (system) QNetworkProxyFactory::setUseSystemConfiguration(true);
            else QNetworkProxy::setApplicationProxy(proxy);
        });
        QNetworkProxy::setApplicationProxy(QNetworkProxy(QNetworkProxy::NoProxy));
        const int offset = helperReceipts().size();
        PhoneActorCase current;
        QVERIFY(current.open());
        QCOMPARE(current.fixture.witness.certificateErrors, 0);
        QTRY_VERIFY(current.lifetime.credentialTlsEligible(current.fixture.view, current.fixture.server.pageUrl()));
        QVERIFY(current.request(scenario, scenario == 3 || scenario == 14 ? 1500 : 10000));
        const auto chooserTitle = QStringLiteral("Choose an authenticator");
        QTRY_VERIFY(visibleCredentialDialog(chooserTitle));
        QPointer<QDialog> chooser(visibleCredentialDialog(chooserTitle));
        auto *phone = chooser->findChild<QPushButton *>("credentialPhoneProvider");
        QVERIFY(phone && phone->isEnabled());
        chooser->windowHandle()->requestActivate();
        QTRY_COMPARE(QGuiApplication::focusWindow(), chooser->windowHandle());
        if (scenario == 9) {
            // Connected after the actor's provider slot: change the route
            // after start(), before the asynchronous started/stdin boundary.
            connect(current.binding->boundary(), &FerricCredentialBoundary::providerChosen, this, [](bool selected) {
                if (selected) QNetworkProxy::setApplicationProxy(QNetworkProxy(QNetworkProxy::HttpProxy, "127.0.0.1", 9));
            });
        }
        QTest::mouseClick(phone, Qt::LeftButton);
        const auto qrTitle = QStringLiteral("Use a phone passkey");
        if (scenario == 2 || scenario == 14) {
            QTRY_VERIFY(visibleCredentialDialog(qrTitle));
            QPointer<QDialog> accounts(visibleCredentialDialog(qrTitle));
            const auto bob = [&]() -> QPushButton * {
                if (!accounts) return nullptr;
                for (auto *button : accounts->findChildren<QPushButton *>()) if (button->text() == "Bob") return button;
                return nullptr;
            };
            QTRY_VERIFY(bob());
            if (scenario == 2) {
                accounts->windowHandle()->requestActivate();
                QTRY_COMPARE(QGuiApplication::focusWindow(), accounts->windowHandle());
                QTest::mouseClick(bob(), Qt::LeftButton);
            }
        } else if (scenario == 3) {
            const auto completing = [&] {
                auto *dialog = visibleCredentialDialog(qrTitle);
                if (dialog) for (auto *label : dialog->findChildren<QLabel *>()) if (label->text() == QStringLiteral("Finishing…")) return true;
                return false;
            };
            QTRY_VERIFY(completing());
        } else if (scenario == 6 || (scenario >= 10 && scenario <= 13) || scenario == 16 || scenario == 19) {
            const auto qrDrawn = [&] {
                auto *dialog = visibleCredentialDialog(qrTitle);
                if (!dialog) return false;
                for (auto *label : dialog->findChildren<QLabel *>()) if (!label->pixmap().isNull()) return true;
                return false;
            };
            QTRY_VERIFY(qrDrawn());
            if (scenario == 19) {
                // Removing an admitted startup mapping cannot retrospectively
                // change Chromium's already-initialized resolver policy.
                if (flags.isNull()) qputenv("QTWEBENGINE_CHROMIUM_FLAGS", "--disable-gpu");
                else qunsetenv("QTWEBENGINE_CHROMIUM_FLAGS");
            }
            else if (scenario == 10) current.interceptor.setSecurityDenyHosts({"cable.auth.com"});
            else if (scenario == 11) {
                QQmlComponent component(&current.fixture.engine);
                component.setData("import QtQuick; Connections { property var host; target:host; function onClosing(event) {event.accepted=false} }", QUrl());
                std::unique_ptr<QObject> reject(component.create());
                QVERIFY(reject && reject->setProperty("host", QVariant::fromValue(static_cast<QObject *>(current.fixture.window))));
                QVERIFY(!current.fixture.window->close());
                QVERIFY(current.fixture.window->isVisible());
            } else if (scenario == 12) current.integration->setLifetime(nullptr);
            else if (scenario == 16) {
                connect(current.binding, &FerricQuickCredentialBinding::leaseInvalidated, current.fixture.window, [&] { current.integration.reset(); });
                current.interceptor.setBlockedHosts({"tracker.invalid"});
                QVERIFY(!current.integration);
            }
            else if (scenario == 13) {
                PhoneActorCase other;
                QVERIFY(other.open());
                QVERIFY(other.request(1));
                QTRY_VERIFY(other.value("phoneDone").toBool());
                QCOMPARE(other.value("phoneError").toString(), QStringLiteral("NotAllowedError"));
                QCOMPARE(other.value("phoneId").toString(), QString());
                current.value("phoneController.abort(new DOMException('fixture abort','AbortError'))");
            } else current.value("phoneController.abort(new DOMException('fixture abort','AbortError'))");
        }
        QTRY_VERIFY_WITH_TIMEOUT(current.value("phoneDone").toBool(), 15000);
        QCOMPARE(current.value("phoneCount").toInt(), 1);
        if (scenario == 1 || scenario == 2 || scenario == 17) {
            QCOMPARE(current.value("phoneId").toString(), scenario == 2 ? QStringLiteral("Zml4dHVyZS0y") : QStringLiteral("Zml4dHVyZS0x"));
            QVERIFY(current.value("phoneIDL").toBool());
            QCOMPARE(current.value("phoneError").toString(), QString());
        } else {
            QCOMPARE(current.value("phoneId").toString(), QString());
            QCOMPARE(current.value("phoneError").toString(), scenario == 18 ? QStringLiteral("InvalidStateError") : scenario == 8 ? QStringLiteral("SecurityError")
                : scenario == 6 || (scenario >= 10 && scenario <= 13) || scenario == 16 ? QStringLiteral("AbortError") : QStringLiteral("NotAllowedError"));
        }
        QTRY_VERIFY_WITH_TIMEOUT(helperReceiptsReaped(offset), 5000);
        QTRY_VERIFY(!visibleCredentialDialog(qrTitle));
        int requests = 0;
        const auto rows = helperReceipts();
        for (int index = offset; index < rows.size(); ++index) if (rows[index].value("kind").toString() == "request") ++requests;
        QCOMPARE(requests, scenario == 9 ? 0 : 1);
    }
};

int main(int argc, char **argv)
{
    QtWebEngineQuick::initialize();
    QApplication application(argc, argv);
    const bool helperFixture = qgetenv("FERRIC_PHONE_ACTOR_FIXTURE") == "1";
    if (!FerricCredentialNetworkPolicy::capture(helperFixture)) return 1;
    if (!cxx_qt_init_crate_ferric_browser_engine_qt()) return 1;
    if (helperFixture) {
        PhoneCredentialActorTests tests;
        return QTest::qExec(&tests, argc, argv);
    }
    QuickCredentialSurfaceTests tests;
    return QTest::qExec(&tests, argc, argv);
}

#include "tst_quick_credential_surface.moc"
