#include "public_page_host.h"

#include "public_page_owner.h"
#include "credential_boundary.h"

#include <QQuickWidget>
#include <QVBoxLayout>
#include <QApplication>
#include <QMetaObject>
#include <QThread>
#include <QWebEnginePage>
#include <QWebEngineNavigationRequest>
#include <QDialog>
#include <QWebEngineProfile>
#include <QWebEngineView>
#include <QQmlContext>
#include <QHash>
#include <QStringList>
#include <QUuid>
#include <QUrl>
#include <QVariantMap>

#include <utility>
#include <limits>

namespace {

struct PageHostRegistry {
    QHash<QString, QPointer<FerricPublicPageHost>> hosts;
    QHash<QString, QPointer<FerricPublicPageOwner>> owners;
    QHash<QString, QPointer<FerricPublicPageHost>> owner_hosts;
    QHash<QString, QString> owner_profiles;
    QHash<QString, QPointer<QObject>> owner_uis;
    QHash<QString, QPointer<QWebEngineProfile>> profiles_by_runtime_id;
    QHash<QWebEngineProfile *, QString> runtime_ids_by_profile;
};

PageHostRegistry &pageHostRegistry()
{
    static PageHostRegistry registry;
    return registry;
}

QString opaqueToken()
{
    return QUuid::createUuid().toString(QUuid::WithoutBraces);
}

bool bindRuntimeProfile(const QString &profileId, QWebEngineProfile *profile)
{
    if (profileId.isEmpty() || !profile) {
        return false;
    }
    auto &registry = pageHostRegistry();
    const QPointer<QWebEngineProfile> existingProfile = registry.profiles_by_runtime_id.value(profileId);
    const QString existingId = registry.runtime_ids_by_profile.value(profile);
    if ((existingProfile && existingProfile != profile)
        || (!existingId.isEmpty() && existingId != profileId)) {
        return false;
    }
    if (!existingProfile) {
    // A new native profile may reuse the runtime UUID only after the old
    // profile's owner bindings have been retired on their BrowserUi objects.
    QStringList retiredOwnerTokens;
    for (auto it = registry.owner_profiles.cbegin(); it != registry.owner_profiles.cend(); ++it) {
        if (it.value() == profileId) {
            retiredOwnerTokens.append(it.key());
        }
    }
    for (const QString &ownerToken : retiredOwnerTokens) {
        const QPointer<QObject> browserUi = registry.owner_uis.value(ownerToken);
        if (browserUi && QThread::currentThread() == browserUi->thread()) {
            bool released = false;
            QMetaObject::invokeMethod(
                browserUi,
                "release_page_host_owner",
                Qt::DirectConnection,
                Q_RETURN_ARG(bool, released),
                Q_ARG(QString, profileId),
                Q_ARG(QString, ownerToken));
        }
        registry.owners.remove(ownerToken);
        registry.owner_hosts.remove(ownerToken);
        registry.owner_profiles.remove(ownerToken);
        registry.owner_uis.remove(ownerToken);
    }

    // A destroyed profile can leave a raw reverse-map key behind until its
    // callback runs. Do not let that stale identity validate a later binding
    // that reuses the runtime UUID.
    for (auto it = registry.runtime_ids_by_profile.begin();
         it != registry.runtime_ids_by_profile.end();) {
        if (it.value() == profileId && it.key() != profile) {
            it = registry.runtime_ids_by_profile.erase(it);
        } else {
            ++it;
        }
    }
    registry.profiles_by_runtime_id.insert(profileId, profile);
        registry.runtime_ids_by_profile.insert(profile, profileId);
        QObject::connect(profile, &QObject::destroyed, [profileId, profile]() {
            auto &registry = pageHostRegistry();
            QStringList ownerTokens;
            for (auto it = registry.owner_profiles.cbegin();
                 it != registry.owner_profiles.cend();
                 ++it) {
                if (registry.runtime_ids_by_profile.value(profile) == profileId
                    && it.value() == profileId) {
                    ownerTokens.append(it.key());
                }
            }
            for (const QString &ownerToken : ownerTokens) {
                const QPointer<QObject> browserUi = registry.owner_uis.value(ownerToken);
                if (browserUi && QThread::currentThread() == browserUi->thread()) {
                    bool released = false;
                    QMetaObject::invokeMethod(
                        browserUi,
                        "release_page_host_owner",
                        Qt::DirectConnection,
                        Q_RETURN_ARG(bool, released),
                        Q_ARG(QString, profileId),
                        Q_ARG(QString, ownerToken));
                }
                registry.owners.remove(ownerToken);
                registry.owner_hosts.remove(ownerToken);
                registry.owner_profiles.remove(ownerToken);
                registry.owner_uis.remove(ownerToken);
            }
            if (registry.runtime_ids_by_profile.value(profile) == profileId) {
                registry.profiles_by_runtime_id.remove(profileId);
            }
            if (registry.runtime_ids_by_profile.value(profile) == profileId) {
                registry.runtime_ids_by_profile.remove(profile);
            }
        });
    }
    return true;
}

} // namespace

FerricPublicPageHost::FerricPublicPageHost(
    QObject *browserUi,
    QWebEngineProfile *profile,
    FerricPublicPageOwner *initialOwner,
    QWidget *parent,
    QString initialTabId)
    : QWidget(parent),
      m_profile(profile),
      m_chromeView(new QQuickWidget(this)),
      m_pageView(new QWebEngineView(this)),
      m_layout(new QVBoxLayout(this)),
      m_hostToken(opaqueToken())
{
    m_browserUi = browserUi;
    setObjectName(QStringLiteral("ferricPublicPageHost"));
    setFocusPolicy(Qt::StrongFocus);

    m_layout->setContentsMargins(0, 0, 0, 0);
    m_layout->setSpacing(0);
    m_layout->addWidget(m_chromeView);
    m_layout->addWidget(m_pageView, 1);

    m_chromeView->setObjectName(QStringLiteral("ferricWidgetChrome"));
    m_chromeView->setResizeMode(QQuickWidget::SizeRootObjectToView);
    m_chromeView->setFocusPolicy(Qt::StrongFocus);
    m_chromeView->setInitialProperties(
        {{QStringLiteral("browserUi"), QVariant::fromValue(browserUi)}});
    m_chromeView->setSource(
        QUrl(QStringLiteral(
            "qrc:/qt/qml/io/github/ferricbrowser/qml/components/FerricWidgetChrome.qml")));
    m_chromeView->setMinimumHeight(44);
    m_chromeView->setMaximumHeight(44);

    m_pageView->setObjectName(QStringLiteral("ferricHostedPageView"));
    m_pageView->setFocusPolicy(Qt::StrongFocus);
    m_pageView->setEnabled(false);
    // Bind the native profile pointer to the profile UUID owned by this Rust
    // BrowserUi instead of accepting a caller-supplied profile string.
    if (browserUi) {
        QMetaObject::invokeMethod(
            browserUi,
            "page_host_profile_id",
            Qt::DirectConnection,
            Q_RETURN_ARG(QString, m_profileId));
    }
    if (!bindRuntimeProfile(m_profileId, m_profile)) {
        m_profileId.clear();
    }
    if (!m_profileId.isEmpty()) {
        pageHostRegistry().hosts.insert(m_hostToken, this);
    }
    if (initialOwner && !registerInitialPage(initialOwner, initialTabId)) {
        m_profileId.clear();
    }
}

FerricPublicPageHost::~FerricPublicPageHost()
{
    auto &registry = pageHostRegistry();
    registry.hosts.remove(m_hostToken);
    if (!m_pageOwnerToken.isEmpty() && registry.owner_hosts.value(m_pageOwnerToken) == this) {
        if (m_pageOwner) m_pageOwner->invalidatePresentation();
        registry.owner_hosts.remove(m_pageOwnerToken);
    }
}

bool FerricPublicPageHost::isReady() const
{
    return m_chromeView && m_chromeView->status() == QQuickWidget::Ready
        && m_pageView && m_profile && !m_profileId.isEmpty();
}

bool FerricPublicPageHost::registerInitialPage(
    FerricPublicPageOwner *owner,
    const QString &tabId)
{
    if (m_profileId.isEmpty() || !owner || owner->isClosed() || !owner->page() || !owner->profile()) {
        return false;
    }
    if (owner->page()->profile() != owner->profile()) {
        return false;
    }
    if (m_pageOwner && m_pageOwner != owner) {
        return false;
    }
    if (m_profile != owner->profile()) {
        return false;
    }
    if (m_pageView->page() != owner->page()) {
        m_pageView->setPage(owner->page());
    }

    m_pageOwner = owner;
    if (m_pageOwnerToken.isEmpty()) {
        m_pageOwnerToken = opaqueToken();
    }
    auto &registry = pageHostRegistry();
    registry.owners.insert(m_pageOwnerToken, owner);
    registry.owner_hosts.insert(m_pageOwnerToken, this);
    registry.owner_profiles.insert(m_pageOwnerToken, m_profileId);
    registry.owner_uis.insert(m_pageOwnerToken, m_browserUi);
    const QString ownerToken = m_pageOwnerToken;
    const QString profileId = m_profileId;
    const QPointer<QObject> browserUi = m_browserUi;
    QObject::connect(owner, &QObject::destroyed, [ownerToken, profileId, browserUi]() {
        auto &registry = pageHostRegistry();
        if (browserUi && QThread::currentThread() == browserUi->thread()) {
            bool released = false;
            QMetaObject::invokeMethod(
                browserUi,
                "release_page_host_owner",
                Qt::DirectConnection,
                Q_RETURN_ARG(bool, released),
                Q_ARG(QString, profileId),
                Q_ARG(QString, ownerToken));
        }
        registry.owners.remove(ownerToken);
        registry.owner_hosts.remove(ownerToken);
        registry.owner_profiles.remove(ownerToken);
        registry.owner_uis.remove(ownerToken);
    });
    if (!m_browserUi || QThread::currentThread() != m_browserUi->thread()) {
        return false;
    }
    QString ownerTabId = tabId;
    if (ownerTabId.isEmpty()) {
        const int activeIndex = m_browserUi->property("active_tab_index").toInt();
        const bool tabResolved = QMetaObject::invokeMethod(
            m_browserUi,
            "tab_id_for_index",
            Qt::DirectConnection,
            Q_RETURN_ARG(QString, ownerTabId),
            Q_ARG(int, activeIndex));
        if (!tabResolved || ownerTabId.isEmpty()) {
            return false;
        }
    }
    bool rustRegistered = false;
    const bool registrationInvoked = QMetaObject::invokeMethod(
        m_browserUi,
        "register_page_host_owner",
        Qt::DirectConnection,
        Q_RETURN_ARG(bool, rustRegistered),
        Q_ARG(QString, ownerTabId),
        Q_ARG(QString, m_profileId),
        Q_ARG(QString, m_hostToken),
        Q_ARG(QString, m_pageOwnerToken));
    if (!registrationInvoked || !rustRegistered) {
        auto &registry = pageHostRegistry();
        registry.owners.remove(m_pageOwnerToken);
        registry.owner_hosts.remove(m_pageOwnerToken);
        registry.owner_profiles.remove(m_pageOwnerToken);
        registry.owner_uis.remove(m_pageOwnerToken);
        m_pageOwner.clear();
        m_pageOwnerToken.clear();
        return false;
    }

    m_tabId = ownerTabId;
    m_pageView->setEnabled(true);
    bindPageSignals();
    m_detached = false;
    if (m_browserUi->property("feature_phone_passkeys_requested").toString() == "on")
        prepareCredentialBoundary();
    return m_pageView->page() == owner->page();
}

bool FerricPublicPageHost::applyNativeRequest(const QVariant &request)
{
    const QVariantMap captured = request.toMap();
    const QString capturedTab = captured.value(QStringLiteral("tabId")).toString();
    const QString capturedProfile = captured.value(QStringLiteral("profileId")).toString();
    const QString capturedGeneration = captured.value(QStringLiteral("generation")).toString();
    const QString capturedDocument = captured.value(QStringLiteral("documentId")).toString();
    const QString capturedOwner = captured.value(QStringLiteral("pageOwnerToken")).toString();
    if (!m_browserUi || QThread::currentThread() != m_browserUi->thread()
        || capturedTab.isEmpty() || capturedProfile.isEmpty() || capturedGeneration.isEmpty()
        || capturedDocument.isEmpty() || capturedOwner.isEmpty()) {
        return false;
    }

    QString currentProfile;
    bool targetCurrent = false;
    bool ownerMatchesTarget = false;
    const bool profileRead = QMetaObject::invokeMethod(
        m_browserUi,
        "page_host_profile_id",
        Qt::DirectConnection,
        Q_RETURN_ARG(QString, currentProfile));
    const bool targetRead = QMetaObject::invokeMethod(
        m_browserUi,
        "request_target_current",
        Qt::DirectConnection,
        Q_RETURN_ARG(bool, targetCurrent),
        Q_ARG(QString, capturedTab),
        Q_ARG(QString, capturedGeneration),
        Q_ARG(QString, capturedDocument));
    const bool ownerBindingRead = QMetaObject::invokeMethod(
        m_browserUi,
        "page_host_owner_matches_target",
        Qt::DirectConnection,
        Q_RETURN_ARG(bool, ownerMatchesTarget),
        Q_ARG(QString, capturedOwner),
        Q_ARG(QString, capturedProfile),
        Q_ARG(QString, capturedTab),
        Q_ARG(QString, capturedGeneration));
    if (!profileRead || currentProfile != capturedProfile || !targetRead || !targetCurrent
        || !ownerBindingRead || !ownerMatchesTarget) {
        return false;
    }
    const QVariantMap row = request.toMap();
    if (row.value(QStringLiteral("kind")).toString() != QStringLiteral("reparent-view")
        || row.value(QStringLiteral("windowId")).toString().isEmpty()
        || row.value(QStringLiteral("tabId")).toString().isEmpty()
        || row.value(QStringLiteral("generation")).toString().isEmpty()
        || row.value(QStringLiteral("documentId")).toString().isEmpty()
        || row.value(QStringLiteral("profileId")).toString() != m_profileId) {
        return false;
    }

    const QString hostToken = row.value(QStringLiteral("hostToken")).toString();
    const QString pageOwnerToken = row.value(QStringLiteral("pageOwnerToken")).toString();
    const QString operation = row.value(QStringLiteral("operation")).toString();
    const bool attaching = operation == QStringLiteral("attach");
    if ((!attaching && operation != QStringLiteral("detach"))
        || hostToken != m_hostToken
        || !ferric_public_page_host_tokens_live(
            hostToken, pageOwnerToken, m_profileId, attaching)) {
        return false;
    }

    auto &registry = pageHostRegistry();
    FerricPublicPageOwner *owner = registry.owners.value(pageOwnerToken).data();
    if (!owner || owner->isClosed() || !owner->page() || owner->profile() != m_profile) {
        return false;
    }

    if (!attaching) {
        if (m_pageOwner != owner || !m_pageView || m_pageView->page() != owner->page()) {
            return false;
        }
        QWidget *focusWidget = QApplication::focusWidget();
        m_pageViewHadFocus = m_pageView->hasFocus()
            || (focusWidget && m_pageView->isAncestorOf(focusWidget));
        m_layout->removeWidget(m_pageView);
        m_pageView->hide();
        m_detached = true;
        owner->invalidatePresentation();
        return true;
    }

    QWebEngineView *view = nullptr;
    bool restoreFocus = false;
    FerricPublicPageHost *source = registry.owner_hosts.value(pageOwnerToken).data();
    if (source && source != this) {
        view = source->takePageViewForTransfer(owner);
        restoreFocus = source->m_pageViewHadFocus;
    } else if (source == this && m_pageOwner == owner && m_pageView) {
        if (!m_detached) {
            return true;
        }
        view = m_pageView;
        restoreFocus = m_pageViewHadFocus;
        m_layout->addWidget(view, 1);
        view->show();
        m_detached = false;
        if (restoreFocus) {
            view->setFocus(Qt::OtherFocusReason);
        }
        return true;
    } else if (!source) {
        view = new QWebEngineView(this);
        view->setPage(owner->page());
    }

    return view && acceptTransferredPage(owner, view, pageOwnerToken, restoreFocus);
}

QWebEngineView *FerricPublicPageHost::takePageViewForTransfer(FerricPublicPageOwner *owner)
{
    if (!owner || m_pageOwner != owner || !m_pageView || m_pageView->page() != owner->page()) {
        return nullptr;
    }
    QWebEngineView *view = m_pageView;
    QWidget *focusWidget = QApplication::focusWidget();
    if (!m_detached) {
        m_pageViewHadFocus = view->hasFocus()
            || (focusWidget && view->isAncestorOf(focusWidget));
    }
    m_layout->removeWidget(view);
    view->hide();
    view->setParent(nullptr);
    for (const auto &connection : std::as_const(m_pageConnections)) QObject::disconnect(connection);
    m_pageConnections.clear();
    owner->invalidatePresentation();
    m_pageView = nullptr;
    m_pageOwner = nullptr;
    m_pageOwnerToken.clear();
    m_detached = true;
    return view;
}

bool FerricPublicPageHost::acceptTransferredPage(
    FerricPublicPageOwner *owner,
    QWebEngineView *view,
    const QString &pageOwnerToken,
    bool restoreFocus)
{
    if (!owner || owner->isClosed() || owner->profile() != m_profile
        || !owner->page() || !view || view->page() != owner->page()) {
        return false;
    }
    if (m_pageOwner && m_pageOwner != owner) {
        return false;
    }
    if (m_pageView && m_pageView != view) {
        m_layout->removeWidget(m_pageView);
        delete m_pageView;
    }
    view->setParent(this);
    view->setFocusPolicy(Qt::StrongFocus);
    view->setEnabled(true);
    m_layout->addWidget(view, 1);
    view->show();
    m_pageView = view;
    m_pageOwner = owner;
    m_pageOwnerToken = pageOwnerToken;
    m_detached = false;
    m_pageViewHadFocus = restoreFocus;
    auto *source = pageHostRegistry().owner_hosts.value(pageOwnerToken).data();
    if (source && source != this) m_tabId = source->m_tabId;
    pageHostRegistry().owner_hosts.insert(pageOwnerToken, this);
    owner->invalidatePresentation();
    bindPageSignals();
    if (owner->findChild<FerricCredentialBoundary *>()) prepareCredentialBoundary();
    if (restoreFocus) {
        view->setFocus(Qt::OtherFocusReason);
    }
    return true;
}

FerricPublicPageOwner *FerricPublicPageHost::pageOwner() const
{
    return m_pageOwner.data();
}

QWebEngineView *FerricPublicPageHost::pageView() const
{
    return m_pageView;
}

QQuickWidget *FerricPublicPageHost::chromeView() const
{
    return m_chromeView;
}

QWidget *FerricPublicPageHost::nativeDialogParent() const
{
    return m_pageView;
}

QString FerricPublicPageHost::hostToken() const
{
    return m_hostToken;
}

QString FerricPublicPageHost::pageOwnerToken() const
{
    return m_pageOwnerToken;
}

bool ferric_public_page_host_tokens_live(
    const QString &hostToken,
    const QString &pageOwnerToken,
    const QString &profileId,
    bool attach)
{
    auto &registry = pageHostRegistry();
    FerricPublicPageHost *host = registry.hosts.value(hostToken).data();
    FerricPublicPageOwner *owner = registry.owners.value(pageOwnerToken).data();
    if (!host || !owner || owner->isClosed() || !owner->page() || !owner->profile()
        || host->m_profile != owner->profile() || host->m_profileId != profileId
        || registry.owner_profiles.value(pageOwnerToken) != profileId) {
        return false;
    }
    if (attach) {
        return !host->m_pageOwner || host->m_pageOwner == owner;
    }
    return registry.owner_hosts.value(pageOwnerToken) == host
        && host->m_pageOwner == owner && host->m_pageOwnerToken == pageOwnerToken;
}


int FerricPublicPageHost::currentTabIndex() const
{
    int index = -1;
    if (m_browserUi) {
        QMetaObject::invokeMethod(m_browserUi, "tab_index_for_id", Qt::DirectConnection,
                                  Q_RETURN_ARG(int, index), Q_ARG(QString, m_tabId));
    }
    return index;
}

bool FerricPublicPageHost::requestIsCurrent(const QVariantMap &row) const
{
    if (!m_browserUi || !m_pageOwner || !m_pageOwner->page() || m_detached
        || !m_pageView || m_pageView->page() != m_pageOwner->page()
        || QThread::currentThread() != thread()
        || pageHostRegistry().owner_hosts.value(m_pageOwnerToken) != this
        || row.value("tabId").toString() != m_tabId
        || row.value("nativeHost").toString() != m_hostToken
        || row.value("nativeOwner").toString() != m_pageOwnerToken
        || row.value("nativeDocument").toString() != QString::number(m_pageOwner->documentGeneration())
        || m_pageOwner->profile() != m_profile
        || row.value("nativeHostGeneration").toString() != QString::number(m_pageOwner->hostGeneration())
        || row.value("nativePageGeneration").toString() != QString::number(m_pageOwner->pageGeneration())
        || row.value("nativePresentationGeneration").toString() != QString::number(m_pageOwner->presentationGeneration())) return false;
    QString profile;
    bool current = false;
    bool ownerMatches = false;
    QMetaObject::invokeMethod(m_browserUi, "page_host_profile_id", Qt::DirectConnection,
                              Q_RETURN_ARG(QString, profile));
    const QString tab = row.value("tabId").toString();
    const QString generation = row.value("generation").toString();
    const QString document = row.value("documentId").toString();
    QMetaObject::invokeMethod(m_browserUi, "request_target_current", Qt::DirectConnection,
                              Q_RETURN_ARG(bool, current), Q_ARG(QString, tab),
                              Q_ARG(QString, generation), Q_ARG(QString, document));
    QMetaObject::invokeMethod(m_browserUi, "page_host_owner_matches_target", Qt::DirectConnection,
                              Q_RETURN_ARG(bool, ownerMatches), Q_ARG(QString, m_pageOwnerToken),
                              Q_ARG(QString, m_profileId), Q_ARG(QString, tab), Q_ARG(QString, generation));
    return profile == m_profileId && current && ownerMatches && currentTabIndex() >= 0;
}

QVariantMap FerricPublicPageHost::ownerStamp(QVariantMap row) const
{
    row.insert("nativeTarget", true);
    row.insert("nativeHost", m_hostToken);
    row.insert("nativeOwner", m_pageOwnerToken);
    row.insert("nativeDocument", m_pageOwner ? QString::number(m_pageOwner->documentGeneration()) : QString());
    row.insert("nativeHostGeneration", m_pageOwner ? QString::number(m_pageOwner->hostGeneration()) : QString());
    row.insert("nativePageGeneration", m_pageOwner ? QString::number(m_pageOwner->pageGeneration()) : QString());
    row.insert("nativePresentationGeneration", m_pageOwner ? QString::number(m_pageOwner->presentationGeneration()) : QString());
    return row;
}

QVariant FerricPublicPageHost::captureRequest(const QVariant &request) const
{
    auto row = ownerStamp(request.toMap());
    if (m_nextRequestId == std::numeric_limits<quint64>::max()) return row;
    row.insert("nativeRequest", QString::number(++m_nextRequestId));
    return row;
}

QVariant ferric_public_page_capture_request(const QString &ownerToken, const QVariant &request)
{
    const auto host = pageHostRegistry().owner_hosts.value(ownerToken);
    if (host) return host->captureRequest(request);
    QVariantMap row = request.toMap();
    row.insert("nativeTarget", true);
    return row;
}

QVariant ferric_public_page_for_request(const QVariant &request)
{
    const auto row = request.toMap();
    if (row.value("kind").toString() == "reparent-view") {
        const QString operation = row.value("operation").toString();
        const auto host = pageHostRegistry().hosts.value(row.value("hostToken").toString());
        const bool live = (operation == "attach" || operation == "detach")
            && ferric_public_page_host_tokens_live(row.value("hostToken").toString(),
                row.value("pageOwnerToken").toString(), row.value("profileId").toString(), operation == "attach");
        return QVariant::fromValue(static_cast<QObject *>(host && live ? host.data() : nullptr));
    }
    const auto host = pageHostRegistry().hosts.value(row.value("nativeHost").toString());
    // Request envelopes contain only values. Resolve the QObject immediately
    // before dispatch, so retaining a request cannot retain a dangling pointer.
    return QVariant::fromValue(static_cast<QObject *>(
        host && host->requestIsCurrent(row) ? host.data() : nullptr));
}

bool FerricPublicPageHost::executeCaptured(const QVariant &request)
{
    const QVariantMap row = request.toMap();
    bool serialValid = false;
    const auto serial = row.value("nativeRequest").toString().toULongLong(&serialValid);
    if (!serialValid || serial <= m_lastExecutedRequest || serial > m_nextRequestId
        || !requestIsCurrent(row)) return false;
    // Consume before any reentrant Qt call, including validation failures.
    m_lastExecutedRequest = serial;
    return executePageCommand(row);
}

void FerricPublicPageHost::bindPageSignals()
{
    for (const auto &connection : std::as_const(m_pageConnections)) QObject::disconnect(connection);
    m_pageConnections.clear();
    auto *page = m_pageOwner->page();
    m_pageConnections.append(connect(m_pageOwner, &FerricPublicPageOwner::authorityInvalidated, this, [this] {
        if (m_formReloadDialog) m_formReloadDialog->reject();
        if (m_pageOwner && m_pageOwner->isClosed() && m_pageView) {
            // A retired owner must not leave an interactive view that can
            // lazily acquire Qt's unrelated default page/profile.
            m_pageView->setEnabled(false);
            m_pageView->hide();
        }
    }));
    m_pageConnections.append(connect(page, &QWebEnginePage::navigationRequested, this,
        [this](QWebEngineNavigationRequest &request) {
            if (request.isMainFrame()) m_documentHasFormData = request.hasFormData();
        }));
    m_pageConnections.append(connect(page, &QWebEnginePage::loadStarted, this, [this] {
        if (!m_pageOwner) return;
        m_engineLoadState = QStringLiteral("provisional");
        // The owner advances its native document generation before this slot.
        m_navigationRequest.insert("nativeDocument", QString::number(m_pageOwner->documentGeneration()));
        if (!requestIsCurrent(m_navigationRequest)) return;
        m_navigationDocument = m_pageOwner->documentGeneration();
        const int index = currentTabIndex();
        const QString url = m_navigationRequest.value("url").toString();
        QMetaObject::invokeMethod(m_browserUi, "navigation_started_for", Qt::DirectConnection,
                                  Q_ARG(int, index), Q_ARG(QString, url));
    }));
    m_pageConnections.append(connect(page, &QWebEnginePage::loadFinished, this, [this, page](bool ok) {
        m_engineLoadState = ok ? QStringLiteral("complete") : QStringLiteral("failed");
        if (!requestIsCurrent(m_navigationRequest)
            || m_navigationDocument != m_pageOwner->documentGeneration()) return;
        const int index = currentTabIndex();
        const QString url = page->url().toString();
        const QString title = page->title();
        if (ok) {
            QMetaObject::invokeMethod(m_browserUi, "navigation_committed_for", Qt::DirectConnection,
                                      Q_ARG(int, index), Q_ARG(QString, url), Q_ARG(QString, title));
            QMetaObject::invokeMethod(m_browserUi, "navigation_completed_for", Qt::DirectConnection,
                                      Q_ARG(int, index));
        } else {
            QMetaObject::invokeMethod(m_browserUi, "navigation_failed_for", Qt::DirectConnection,
                                      Q_ARG(int, index));
        }
    }));
}

QString FerricPublicPageHost::credentialContextIdentity() const {
    if (!m_browserUi || !m_pageOwner || !m_pageOwner->page() || m_detached
        || !m_pageView || m_pageView->page() != m_pageOwner->page()
        || pageHostRegistry().owner_hosts.value(m_pageOwnerToken) != this) return {};
    QVariant target;
    const int index = currentTabIndex();
    if (index < 0 || !QMetaObject::invokeMethod(m_browserUi, "navigation_target_for_index", Qt::DirectConnection,
            Q_RETURN_ARG(QVariant, target), Q_ARG(int, index))) return {};
    const auto row = ownerStamp(target.toMap());
    if (!requestIsCurrent(row)) return {};
    return QStringList{m_profileId, m_hostToken, m_pageOwnerToken,
        row.value("tabId").toString(), row.value("generation").toString(), row.value("documentId").toString()}.join(QLatin1Char('/'));
}

FerricCredentialBoundary *FerricPublicPageHost::prepareCredentialBoundary() {
    if (credentialContextIdentity().isEmpty()) return nullptr;
    QPointer<FerricPublicPageHost> guard(this);
    auto contextIdentity = [guard] { return guard ? guard->credentialContextIdentity() : QString(); };
    auto *boundary = m_pageOwner->findChild<FerricCredentialBoundary *>(QString(), Qt::FindDirectChildrenOnly);
    if (boundary) boundary->bindPresentation(m_pageView, std::move(contextIdentity));
    else boundary = new FerricCredentialBoundary(m_pageOwner, m_pageView, std::move(contextIdentity));
    return boundary->isReady() ? boundary : nullptr;
}
