#include "request_interceptor.h"

#include <QtCore/QReadLocker>
#include <QtCore/QWriteLocker>
#include <QtCore/QUrl>
#include <QtWebEngineCore/QWebEngineUrlRequestInfo>

#include <algorithm>
#include <array>
#include <limits>
#include <utility>

namespace {
QString resourceTypeName(const QWebEngineUrlRequestInfo::ResourceType type)
{
    switch (type) {
    case QWebEngineUrlRequestInfo::ResourceTypeMainFrame:
        return QStringLiteral("main-frame");
    case QWebEngineUrlRequestInfo::ResourceTypeSubFrame:
        return QStringLiteral("sub-frame");
    case QWebEngineUrlRequestInfo::ResourceTypeStylesheet:
        return QStringLiteral("stylesheet");
    case QWebEngineUrlRequestInfo::ResourceTypeScript:
        return QStringLiteral("script");
    case QWebEngineUrlRequestInfo::ResourceTypeImage:
        return QStringLiteral("image");
    case QWebEngineUrlRequestInfo::ResourceTypeFontResource:
        return QStringLiteral("font");
    case QWebEngineUrlRequestInfo::ResourceTypeSubResource:
        return QStringLiteral("sub-resource");
    case QWebEngineUrlRequestInfo::ResourceTypeObject:
        return QStringLiteral("object");
    case QWebEngineUrlRequestInfo::ResourceTypeMedia:
        return QStringLiteral("media");
    case QWebEngineUrlRequestInfo::ResourceTypeWorker:
        return QStringLiteral("worker");
    case QWebEngineUrlRequestInfo::ResourceTypeSharedWorker:
        return QStringLiteral("shared-worker");
    case QWebEngineUrlRequestInfo::ResourceTypePrefetch:
        return QStringLiteral("prefetch");
    case QWebEngineUrlRequestInfo::ResourceTypeFavicon:
        return QStringLiteral("favicon");
    case QWebEngineUrlRequestInfo::ResourceTypeXhr:
        return QStringLiteral("xhr");
    case QWebEngineUrlRequestInfo::ResourceTypePing:
        return QStringLiteral("ping");
    case QWebEngineUrlRequestInfo::ResourceTypeServiceWorker:
        return QStringLiteral("service-worker");
    case QWebEngineUrlRequestInfo::ResourceTypeCspReport:
        return QStringLiteral("csp-report");
    case QWebEngineUrlRequestInfo::ResourceTypePluginResource:
        return QStringLiteral("plugin-resource");
    case QWebEngineUrlRequestInfo::ResourceTypeNavigationPreloadMainFrame:
        return QStringLiteral("navigation-preload-main-frame");
    case QWebEngineUrlRequestInfo::ResourceTypeNavigationPreloadSubFrame:
        return QStringLiteral("navigation-preload-sub-frame");
    case QWebEngineUrlRequestInfo::ResourceTypeJson:
        return QStringLiteral("json");
    case QWebEngineUrlRequestInfo::ResourceTypeWebSocket:
        return QStringLiteral("websocket");
    case QWebEngineUrlRequestInfo::ResourceTypeUnknown:
        return QStringLiteral("unknown");
    }
    return QStringLiteral("unknown");
}

QString navigationTypeName(const QWebEngineUrlRequestInfo::NavigationType type)
{
    switch (type) {
    case QWebEngineUrlRequestInfo::NavigationTypeLink:
        return QStringLiteral("link");
    case QWebEngineUrlRequestInfo::NavigationTypeTyped:
        return QStringLiteral("typed");
    case QWebEngineUrlRequestInfo::NavigationTypeFormSubmitted:
        return QStringLiteral("form-submitted");
    case QWebEngineUrlRequestInfo::NavigationTypeBackForward:
        return QStringLiteral("back-forward");
    case QWebEngineUrlRequestInfo::NavigationTypeReload:
        return QStringLiteral("reload");
    case QWebEngineUrlRequestInfo::NavigationTypeOther:
        return QStringLiteral("other");
    case QWebEngineUrlRequestInfo::NavigationTypeRedirect:
        return QStringLiteral("redirect");
    }
    return QStringLiteral("unknown");
}

QString adblockResourceType(const QWebEngineUrlRequestInfo::ResourceType type)
{
    switch (type) {
    case QWebEngineUrlRequestInfo::ResourceTypeMainFrame:
        return QStringLiteral("document");
    case QWebEngineUrlRequestInfo::ResourceTypeSubFrame:
        return QStringLiteral("subdocument");
    case QWebEngineUrlRequestInfo::ResourceTypeStylesheet:
        return QStringLiteral("stylesheet");
    case QWebEngineUrlRequestInfo::ResourceTypeScript:
        return QStringLiteral("script");
    case QWebEngineUrlRequestInfo::ResourceTypeImage:
        return QStringLiteral("image");
    case QWebEngineUrlRequestInfo::ResourceTypeFontResource:
        return QStringLiteral("font");
    case QWebEngineUrlRequestInfo::ResourceTypeObject:
        return QStringLiteral("object");
    case QWebEngineUrlRequestInfo::ResourceTypeMedia:
        return QStringLiteral("media");
    case QWebEngineUrlRequestInfo::ResourceTypeXhr:
        return QStringLiteral("xhr");
    case QWebEngineUrlRequestInfo::ResourceTypePing:
        return QStringLiteral("ping");
    case QWebEngineUrlRequestInfo::ResourceTypeCspReport:
        return QStringLiteral("csp_report");
    case QWebEngineUrlRequestInfo::ResourceTypeWebSocket:
        return QStringLiteral("websocket");
    default:
        return QStringLiteral("other");
    }
}

QVariantMap validatedRuleLists(const QVariantMap &lists)
{
    QVariantMap result;
    for (auto iterator = lists.cbegin(); iterator != lists.cend() && result.size() < 1024;
         ++iterator) {
        auto host = iterator.key().trimmed().toLower();
        const auto source = iterator.value().toString().trimmed();
        const auto encodedSource = source.toUtf8();
        if (host.isEmpty() || host.size() > 253 || host.contains(u'\0')
            || source.isEmpty() || source.size() > 128 || source.contains(u'\0')
            || !std::all_of(encodedSource.cbegin(), encodedSource.cend(), [](const auto byte) {
                   return byte >= 0x20 && byte <= 0x7e;
               })) {
            continue;
        }
        result.insert(host, source);
    }
    return result;
}
}

FerricBrowserRequestInterceptor::FerricBrowserRequestInterceptor(QObject *parent)
    : QWebEngineUrlRequestInterceptor(parent), policy_(std::make_shared<const PolicySnapshot>())
{
}

FerricBrowserRequestInterceptor::~FerricBrowserRequestInterceptor()
{
    detachProfiles();
}

QStringList FerricBrowserRequestInterceptor::blockedHosts() const
{
    QReadLocker locker(&policyLock_);
    return policy_->blockedHosts;
}

void FerricBrowserRequestInterceptor::setBlockedHosts(const QStringList &hosts)
{
    const auto next = validatedHosts(hosts);
    {
        QWriteLocker locker(&policyLock_);
        if (policy_->blockedHosts == next) {
            return;
        }
        auto replacement = *policy_;
        replacement.blockedHosts = next;
        policy_ = std::make_shared<const PolicySnapshot>(replacement);
    }
    emit blockedHostsChanged();
}

QStringList FerricBrowserRequestInterceptor::exceptionHosts() const
{
    QReadLocker locker(&policyLock_);
    return policy_->exceptionHosts;
}

void FerricBrowserRequestInterceptor::setExceptionHosts(const QStringList &hosts)
{
    const auto next = validatedHosts(hosts);
    {
        QWriteLocker locker(&policyLock_);
        if (policy_->exceptionHosts == next) {
            return;
        }
        auto replacement = *policy_;
        replacement.exceptionHosts = next;
        policy_ = std::make_shared<const PolicySnapshot>(replacement);
    }
    emit exceptionHostsChanged();
}

QStringList FerricBrowserRequestInterceptor::bypassSites() const
{
    QReadLocker locker(&policyLock_);
    return policy_->bypassSites;
}

void FerricBrowserRequestInterceptor::setBypassSites(const QStringList &hosts)
{
    const auto next = validatedHosts(hosts);
    {
        QWriteLocker locker(&policyLock_);
        if (policy_->bypassSites == next) {
            return;
        }
        auto replacement = *policy_;
        replacement.bypassSites = next;
        policy_ = std::make_shared<const PolicySnapshot>(replacement);
    }
    emit bypassSitesChanged();
}

QStringList FerricBrowserRequestInterceptor::securityDenyHosts() const
{
    QReadLocker locker(&policyLock_);
    return policy_->securityDenyHosts;
}

void FerricBrowserRequestInterceptor::setSecurityDenyHosts(const QStringList &hosts)
{
    const auto next = validatedHosts(hosts);
    {
        QWriteLocker locker(&policyLock_);
        if (policy_->securityDenyHosts == next) {
            return;
        }
        auto replacement = *policy_;
        replacement.securityDenyHosts = next;
        policy_ = std::make_shared<const PolicySnapshot>(replacement);
    }
    emit securityDenyHostsChanged();
}

QVariantMap FerricBrowserRequestInterceptor::blockedRuleLists() const
{
    QReadLocker locker(&policyLock_);
    return policy_->blockedRuleLists;
}

void FerricBrowserRequestInterceptor::setBlockedRuleLists(const QVariantMap &lists)
{
    const auto next = validatedRuleLists(lists);
    {
        QWriteLocker locker(&policyLock_);
        if (policy_->blockedRuleLists == next) {
            return;
        }
        auto replacement = *policy_;
        replacement.blockedRuleLists = next;
        policy_ = std::make_shared<const PolicySnapshot>(replacement);
    }
    emit blockedRuleListsChanged();
}

QVariantMap FerricBrowserRequestInterceptor::exceptionRuleLists() const
{
    QReadLocker locker(&policyLock_);
    return policy_->exceptionRuleLists;
}

void FerricBrowserRequestInterceptor::setExceptionRuleLists(const QVariantMap &lists)
{
    const auto next = validatedRuleLists(lists);
    {
        QWriteLocker locker(&policyLock_);
        if (policy_->exceptionRuleLists == next) {
            return;
        }
        auto replacement = *policy_;
        replacement.exceptionRuleLists = next;
        policy_ = std::make_shared<const PolicySnapshot>(replacement);
    }
    emit exceptionRuleListsChanged();
}

QStringList FerricBrowserRequestInterceptor::adblockSourceIds() const
{
    QReadLocker locker(&policyLock_);
    return policy_->adblockSourceIds;
}

void FerricBrowserRequestInterceptor::setAdblockSourceIds(const QStringList &ids)
{
    {
        QWriteLocker locker(&policyLock_);
        if (policy_->adblockSourceIds == ids) {
            return;
        }
        auto replacement = *policy_;
        replacement.adblockSourceIds = ids;
        policy_ = std::make_shared<const PolicySnapshot>(replacement);
    }
    emit adblockSourceIdsChanged();
}

qulonglong FerricBrowserRequestInterceptor::adblockEngineHandle() const
{
    QReadLocker locker(&policyLock_);
    return reinterpret_cast<qulonglong>(policy_->adblockEngine.get());
}

void FerricBrowserRequestInterceptor::setAdblockEngineHandle(const qulonglong handle)
{
    std::shared_ptr<AdblockEngine> next;
    if (handle != 0) {
        auto *raw = reinterpret_cast<AdblockEngine *>(handle);
        next = std::shared_ptr<AdblockEngine>(raw, [](AdblockEngine *engine) {
            ferric_browser_adblock_release(engine);
        });
    }
    {
        QWriteLocker locker(&policyLock_);
        auto replacement = *policy_;
        replacement.adblockEngine = std::move(next);
        policy_ = std::make_shared<const PolicySnapshot>(replacement);
    }
    emit adblockEngineHandleChanged();
}

bool FerricBrowserRequestInterceptor::enabled() const
{
    QReadLocker locker(&policyLock_);
    return enabled_;
}

void FerricBrowserRequestInterceptor::setEnabled(const bool enabled)
{
    {
        QWriteLocker locker(&policyLock_);
        if (enabled_ == enabled) {
            return;
        }
        enabled_ = enabled;
    }
    emit enabledChanged();
}

qulonglong FerricBrowserRequestInterceptor::blockedCount() const
{
    return blockedCount_.load(std::memory_order_relaxed);
}

qulonglong FerricBrowserRequestInterceptor::unknownContextCount() const
{
    return unknownContextCount_.load(std::memory_order_relaxed);
}

QVariantMap FerricBrowserRequestInterceptor::blockedSiteCounts() const
{
    QReadLocker locker(&countsLock_);
    QVariantMap result;
    for (auto iterator = blockedSiteCounts_.cbegin(); iterator != blockedSiteCounts_.cend(); ++iterator) {
        result.insert(iterator.key(), QVariant::fromValue(iterator.value()));
    }
    return result;
}

QVariantMap FerricBrowserRequestInterceptor::blockedRequestExplanation(const QString &site) const
{
    const auto normalizedSite = normalizedHost(site);
    if (normalizedSite.isEmpty()) {
        return {};
    }
    QReadLocker locker(&countsLock_);
    return blockedRequestExplanations_.value(normalizedSite);
}

QVariantList FerricBrowserRequestInterceptor::blockedRequestDecisions(const QString &site) const
{
    const auto normalizedSite = normalizedHost(site);
    if (normalizedSite.isEmpty()) {
        return {};
    }
    QReadLocker locker(&countsLock_);
    QVariantList result;
    const auto decisions = requestDecisionLog_.value(normalizedSite);
    result.reserve(decisions.size());
    for (const auto &decision : decisions) {
        result.append(decision);
    }
    return result;
}

void FerricBrowserRequestInterceptor::clearSiteEvidence(const QString &site)
{
    const auto normalizedSite = normalizedHost(site);
    if (normalizedSite.isEmpty()) {
        return;
    }
    bool changed = false;
    {
        QWriteLocker locker(&countsLock_);
        changed = blockedSiteCounts_.remove(normalizedSite) > 0;
        changed = blockedRequestExplanations_.remove(normalizedSite) > 0 || changed;
        changed = requestDecisionLog_.remove(normalizedSite) > 0 || changed;
    }
    if (changed) {
        scheduleEvidenceChanged();
    }
}

bool FerricBrowserRequestInterceptor::attach(QObject *profile)
{
    auto *quickProfile = qobject_cast<QQuickWebEngineProfile *>(profile);
    if (quickProfile == nullptr) {
        return false;
    }
    if (!profiles_.contains(quickProfile)) {
        profiles_.append(QPointer<QQuickWebEngineProfile>(quickProfile));
    }
    quickProfile->setUrlRequestInterceptor(this);
    return true;
}

bool FerricBrowserRequestInterceptor::detach(QObject *profile)
{
    auto *quickProfile = qobject_cast<QQuickWebEngineProfile *>(profile);
    if (quickProfile == nullptr) {
        return false;
    }
    quickProfile->setUrlRequestInterceptor(nullptr);
    profiles_.removeAll(QPointer<QQuickWebEngineProfile>(quickProfile));
    return true;
}

void FerricBrowserRequestInterceptor::interceptRequest(QWebEngineUrlRequestInfo &info)
{
    std::shared_ptr<const PolicySnapshot> policy;
    bool enabled = false;
    {
        QReadLocker locker(&policyLock_);
        enabled = enabled_;
        policy = policy_;
    }
    if (!enabled || (policy->blockedHosts.isEmpty() && policy->securityDenyHosts.isEmpty()
                     && !policy->adblockEngine)) {
        return;
    }

    const auto host = normalizedHost(info.requestUrl().host());
    if (host.isEmpty()) {
        return;
    }
    const auto resourceType = info.resourceType();
    const auto navigationType = info.navigationType();
    const bool isTopLevel = resourceType == QWebEngineUrlRequestInfo::ResourceTypeMainFrame;
    const bool isSubFrame = resourceType == QWebEngineUrlRequestInfo::ResourceTypeSubFrame;
    const bool isRedirect = navigationType == QWebEngineUrlRequestInfo::NavigationTypeRedirect;
    const auto firstPartyHost = normalizedHost(info.firstPartyUrl().host());
    const auto initiatorHost = normalizedHost(info.initiator().host());
    const auto siteHost = firstPartyHost.isEmpty() && isTopLevel ? host : firstPartyHost;
    const bool contextKnown = resourceType != QWebEngineUrlRequestInfo::ResourceTypeUnknown
        && !siteHost.isEmpty() && (isTopLevel || !initiatorHost.isEmpty());
    if (!contextKnown) {
        unknownContextCount_.fetch_add(1, std::memory_order_relaxed);
        scheduleEvidenceChanged();
    }
    const auto addContext = [&](QVariantMap &decision) {
        decision.insert(QStringLiteral("first_party_host"), siteHost);
        decision.insert(QStringLiteral("initiator_host"), initiatorHost);
        decision.insert(QStringLiteral("resource_type"), resourceTypeName(resourceType));
        decision.insert(QStringLiteral("navigation_type"), navigationTypeName(navigationType));
        decision.insert(QStringLiteral("is_top_level"), isTopLevel);
        decision.insert(QStringLiteral("is_subframe"), isSubFrame);
        decision.insert(QStringLiteral("is_redirect"), isRedirect);
        decision.insert(QStringLiteral("context_known"), contextKnown);
    };
    const auto siteBypassed = std::any_of(
        policy->bypassSites.cbegin(), policy->bypassSites.cend(), [&siteHost](const auto &pattern) {
            return !siteHost.isEmpty() && hostMatches(siteHost, pattern);
        });
    const auto securityDenyRule = std::find_if(
        policy->securityDenyHosts.cbegin(), policy->securityDenyHosts.cend(), [&host](const auto &pattern) {
            return hostMatches(host, pattern);
        });
    if (securityDenyRule != policy->securityDenyHosts.cend()) {
        info.block(true);
        blockedCount_.fetch_add(1, std::memory_order_relaxed);
        QVariantMap decision;
        decision.insert(QStringLiteral("resource_host"), host);
        addContext(decision);
        decision.insert(QStringLiteral("matched_rule"), *securityDenyRule);
        decision.insert(QStringLiteral("list_id"), QStringLiteral("security-policy"));
        decision.insert(QStringLiteral("decision"), QStringLiteral("blocked"));
        decision.insert(QStringLiteral("reason"), QStringLiteral("security deny rule"));
        if (!siteHost.isEmpty()) {
            recordDecision(siteHost, decision);
        }
        scheduleEvidenceChanged();
        return;
    }
    if (siteBypassed) {
        return;
    }
    if (policy->adblockEngine) {
        const auto url = info.requestUrl().toString(QUrl::FullyEncoded).toUtf8();
        const auto sourceUrl = info.firstPartyUrl().toString(QUrl::FullyEncoded).toUtf8();
        const auto type = adblockResourceType(resourceType).toUtf8();
        const auto method = info.requestMethod();
        unsigned int sourceIndex = std::numeric_limits<unsigned int>::max();
        std::array<unsigned char, 1024> matchedRule{};
        size_t matchedRuleLength = 0;
        const bool blocked = ferric_browser_adblock_check(
            policy->adblockEngine.get(),
            reinterpret_cast<const unsigned char *>(url.constData()),
            static_cast<size_t>(url.size()),
            reinterpret_cast<const unsigned char *>(sourceUrl.constData()),
            static_cast<size_t>(sourceUrl.size()),
            reinterpret_cast<const unsigned char *>(type.constData()),
            static_cast<size_t>(type.size()),
            reinterpret_cast<const unsigned char *>(method.constData()),
            static_cast<size_t>(method.size()),
            &sourceIndex,
            matchedRule.data(),
            matchedRule.size(),
            &matchedRuleLength);
        if (blocked) {
            info.block(true);
            blockedCount_.fetch_add(1, std::memory_order_relaxed);
            QVariantMap decision;
            decision.insert(QStringLiteral("resource_host"), host);
            addContext(decision);
            const auto matchedRuleText = matchedRuleLength > 0
                ? QString::fromUtf8(reinterpret_cast<const char *>(matchedRule.data()),
                                   static_cast<qsizetype>(matchedRuleLength))
                : QStringLiteral("adblock-network-rule");
            decision.insert(QStringLiteral("matched_rule"), matchedRuleText);
            if (sourceIndex < static_cast<unsigned int>(policy->adblockSourceIds.size())) {
                decision.insert(QStringLiteral("list_id"), policy->adblockSourceIds.at(static_cast<int>(sourceIndex)));
            }
            decision.insert(QStringLiteral("decision"), QStringLiteral("blocked"));
            decision.insert(QStringLiteral("reason"), QStringLiteral("ABP-compatible network rule"));
            if (!siteHost.isEmpty()) {
                recordDecision(siteHost, decision);
            }
            scheduleEvidenceChanged();
            return;
        }
    }
    const auto matchingException = std::find_if(
        policy->exceptionHosts.cbegin(), policy->exceptionHosts.cend(), [&host](const auto &pattern) {
            return hostMatches(host, pattern);
        });
    const bool excepted = matchingException != policy->exceptionHosts.cend();
    const auto matchingRule = std::find_if(
        policy->blockedHosts.cbegin(), policy->blockedHosts.cend(), [&host](const auto &pattern) {
            return hostMatches(host, pattern);
        });
    if (excepted && matchingRule != policy->blockedHosts.cend()) {
        QVariantMap decision;
        decision.insert(QStringLiteral("resource_host"), host);
        addContext(decision);
        decision.insert(QStringLiteral("matched_rule"), *matchingRule);
        decision.insert(QStringLiteral("list_id"),
                       policy->blockedRuleLists.value(*matchingRule, QStringLiteral("unknown")));
        decision.insert(QStringLiteral("exception_rule"), *matchingException);
        decision.insert(
            QStringLiteral("exception_list_id"),
            policy->exceptionRuleLists.value(*matchingException, QStringLiteral("unknown")));
        decision.insert(QStringLiteral("decision"), QStringLiteral("allowed-by-exception"));
        decision.insert(QStringLiteral("reason"),
                        QStringLiteral("matched host exception rule"));
        if (!siteHost.isEmpty()) {
            recordDecision(siteHost, decision);
        }
    }
    if (!excepted && matchingRule != policy->blockedHosts.cend()) {
        info.block(true);
        blockedCount_.fetch_add(1, std::memory_order_relaxed);
        {
            QWriteLocker locker(&countsLock_);
            if (!siteHost.isEmpty()
                && (blockedSiteCounts_.contains(siteHost) || blockedSiteCounts_.size() < 1024)) {
                const auto count = blockedSiteCounts_.value(siteHost);
                blockedSiteCounts_.insert(siteHost, count + 1);
            }
            if (!siteHost.isEmpty()
                && (blockedRequestExplanations_.contains(siteHost)
                    || blockedRequestExplanations_.size() < 1024)) {
                QVariantMap explanation;
                explanation.insert(QStringLiteral("resource_host"), host);
                addContext(explanation);
                explanation.insert(QStringLiteral("matched_rule"), *matchingRule);
                explanation.insert(
                    QStringLiteral("list_id"),
                    policy->blockedRuleLists.value(*matchingRule, QStringLiteral("unknown")));
                explanation.insert(QStringLiteral("reason"),
                                   QStringLiteral("matched host blocking rule"));
                blockedRequestExplanations_.insert(siteHost, explanation);
            }
        }
        QVariantMap decision;
        decision.insert(QStringLiteral("resource_host"), host);
        addContext(decision);
        decision.insert(QStringLiteral("matched_rule"), *matchingRule);
        decision.insert(QStringLiteral("list_id"),
                       policy->blockedRuleLists.value(*matchingRule, QStringLiteral("unknown")));
        decision.insert(QStringLiteral("decision"), QStringLiteral("blocked"));
        decision.insert(QStringLiteral("reason"),
                        QStringLiteral("matched host blocking rule"));
        if (!siteHost.isEmpty()) {
            recordDecision(siteHost, decision);
        }
        scheduleEvidenceChanged();
    }
}

void FerricBrowserRequestInterceptor::recordDecision(const QString &site,
                                                   const QVariantMap &decision)
{
    constexpr qsizetype maxSites = 256;
    constexpr qsizetype maxDecisionsPerSite = 100;
    QWriteLocker locker(&countsLock_);
    if (!requestDecisionLog_.contains(site) && requestDecisionLog_.size() >= maxSites) {
        return;
    }
    auto &decisions = requestDecisionLog_[site];
    while (decisions.size() >= maxDecisionsPerSite) {
        decisions.removeFirst();
    }
    decisions.append(decision);
}

void FerricBrowserRequestInterceptor::scheduleEvidenceChanged()
{
    if (evidenceSignalPending_.exchange(true, std::memory_order_acq_rel)) {
        return;
    }
    QMetaObject::invokeMethod(this, [this]() {
        evidenceSignalPending_.store(false, std::memory_order_release);
        emit blockedCountChanged();
        emit unknownContextCountChanged();
        emit blockedSiteCountsChanged();
    }, Qt::QueuedConnection);
}

QString FerricBrowserRequestInterceptor::normalizedHost(const QString &host)
{
    auto normalized = host.trimmed().toLower();
    if (normalized.isEmpty() || normalized.size() > 253 || normalized.contains(u'\0')) {
        return {};
    }
    while (normalized.endsWith(u'.')) {
        normalized.chop(1);
    }
    return normalized;
}

bool FerricBrowserRequestInterceptor::hostMatches(const QString &host, const QString &pattern)
{
    if (pattern.startsWith(u"*.")) {
        const auto suffix = pattern.mid(2);
        return host != suffix && host.endsWith(QStringLiteral(".") + suffix);
    }
    return host == pattern;
}

QStringList FerricBrowserRequestInterceptor::validatedHosts(const QStringList &hosts)
{
    QStringList result;
    result.reserve(static_cast<qsizetype>(std::min(hosts.size(), qsizetype(1024))));
    for (const auto &host : hosts) {
        if (result.size() >= 1024) {
            break;
        }
        auto normalized = normalizedHost(host);
        if (normalized.size() > 253 || normalized.isEmpty() || normalized.contains(u'/')
            || normalized.contains(u'\\') || normalized.contains(u'@')
            || normalized.contains(u'\0') || normalized.contains(u"..")) {
            continue;
        }
        if (normalized.startsWith(u"*.") && normalized.size() <= 2) {
            continue;
        }
        const auto wildcardCount = normalized.count(u'*');
        if (wildcardCount > 1 || (wildcardCount == 1 && !normalized.startsWith(u"*."))) {
            continue;
        }
        if (!result.contains(normalized)) {
            result.append(normalized);
        }
    }
    return result;
}

void FerricBrowserRequestInterceptor::detachProfiles()
{
    for (const auto &profile : std::as_const(profiles_)) {
        if (profile != nullptr) {
            profile->setUrlRequestInterceptor(nullptr);
        }
    }
    profiles_.clear();
}
