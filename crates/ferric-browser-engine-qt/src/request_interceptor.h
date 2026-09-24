#ifndef FERRIC_BROWSER_REQUEST_INTERCEPTOR_H
#define FERRIC_BROWSER_REQUEST_INTERCEPTOR_H

#include <QtCore/QPointer>
#include <QtCore/QHash>
#include <QtCore/QReadWriteLock>
#include <QtCore/QList>
#include <QtCore/QStringList>
#include <QtCore/QVariantMap>
#include <QtCore/QVariantList>
#include <QtQml/qqmlregistration.h>
#include <QtWebEngineCore/QWebEngineUrlRequestInterceptor>
#include <QtWebEngineQuick/QQuickWebEngineProfile>

#include <memory>
#include <atomic>

struct AdblockEngine;
extern "C" void ferric_browser_adblock_release(AdblockEngine *handle);
extern "C" bool ferric_browser_adblock_check(const AdblockEngine *handle,
                                            const unsigned char *url,
                                            size_t urlLength,
                                            const unsigned char *sourceUrl,
                                            size_t sourceUrlLength,
                                            const unsigned char *requestType,
                                            size_t requestTypeLength,
                                            const unsigned char *method,
                                            size_t methodLength,
                                            unsigned int *sourceIndex,
                                            unsigned char *matchedRule,
                                            size_t matchedRuleCapacity,
                                            size_t *matchedRuleLength);

class FerricBrowserRequestInterceptor : public QWebEngineUrlRequestInterceptor {
    Q_OBJECT
    QML_NAMED_ELEMENT(RequestInterceptor)
    Q_PROPERTY(QStringList blockedHosts READ blockedHosts WRITE setBlockedHosts NOTIFY blockedHostsChanged)
    Q_PROPERTY(QStringList exceptionHosts READ exceptionHosts WRITE setExceptionHosts NOTIFY exceptionHostsChanged)
    Q_PROPERTY(QStringList bypassSites READ bypassSites WRITE setBypassSites NOTIFY bypassSitesChanged)
    Q_PROPERTY(QStringList securityDenyHosts READ securityDenyHosts WRITE setSecurityDenyHosts NOTIFY securityDenyHostsChanged)
    Q_PROPERTY(QStringList blockedRuleHosts READ blockedRuleHosts WRITE setBlockedRuleHosts NOTIFY blockedRuleHostsChanged)
    Q_PROPERTY(QStringList blockedRuleListIds READ blockedRuleListIds WRITE setBlockedRuleListIds NOTIFY blockedRuleListIdsChanged)
    Q_PROPERTY(QStringList exceptionRuleHosts READ exceptionRuleHosts WRITE setExceptionRuleHosts NOTIFY exceptionRuleHostsChanged)
    Q_PROPERTY(QStringList exceptionRuleListIds READ exceptionRuleListIds WRITE setExceptionRuleListIds NOTIFY exceptionRuleListIdsChanged)
    Q_PROPERTY(QStringList adblockSourceIds READ adblockSourceIds WRITE setAdblockSourceIds NOTIFY adblockSourceIdsChanged)
    Q_PROPERTY(qulonglong adblockEngineHandle READ adblockEngineHandle WRITE setAdblockEngineHandle NOTIFY adblockEngineHandleChanged)
    Q_PROPERTY(bool enabled READ enabled WRITE setEnabled NOTIFY enabledChanged)
    Q_PROPERTY(qulonglong blockedCount READ blockedCount NOTIFY blockedCountChanged)
    Q_PROPERTY(qulonglong unknownContextCount READ unknownContextCount NOTIFY unknownContextCountChanged)
    Q_PROPERTY(QVariantMap blockedSiteCounts READ blockedSiteCounts NOTIFY blockedSiteCountsChanged)

public:
    explicit FerricBrowserRequestInterceptor(QObject *parent = nullptr);
    ~FerricBrowserRequestInterceptor() override;

    QStringList blockedHosts() const;
    void setBlockedHosts(const QStringList &hosts);

    QStringList exceptionHosts() const;
    void setExceptionHosts(const QStringList &hosts);

    QStringList bypassSites() const;
    void setBypassSites(const QStringList &hosts);

    QStringList securityDenyHosts() const;
    void setSecurityDenyHosts(const QStringList &hosts);

    QStringList blockedRuleHosts() const;
    void setBlockedRuleHosts(const QStringList &hosts);
    QStringList blockedRuleListIds() const;
    void setBlockedRuleListIds(const QStringList &ids);

    QStringList exceptionRuleHosts() const;
    void setExceptionRuleHosts(const QStringList &hosts);
    QStringList exceptionRuleListIds() const;
    void setExceptionRuleListIds(const QStringList &ids);

    QStringList adblockSourceIds() const;
    void setAdblockSourceIds(const QStringList &ids);

    qulonglong adblockEngineHandle() const;
    void setAdblockEngineHandle(qulonglong handle);

    bool enabled() const;
    void setEnabled(bool enabled);

    qulonglong blockedCount() const;
    qulonglong unknownContextCount() const;
    QVariantMap blockedSiteCounts() const;
    // The QML bridge consumes fixed-width rows rather than JSON.  Keep the
    // QVariant containers private to this native adapter.
    Q_INVOKABLE QStringList blockedRequestExplanationFields(const QString &site) const;
    Q_INVOKABLE QStringList blockedRequestDecisionFields(const QString &site) const;
    Q_INVOKABLE void clearSiteEvidence(const QString &site);

    Q_INVOKABLE bool attach(QObject *profile);
    Q_INVOKABLE bool detach(QObject *profile);

    void interceptRequest(QWebEngineUrlRequestInfo &info) override;

    signals:
    void blockedHostsChanged();
    void exceptionHostsChanged();
    void bypassSitesChanged();
    void securityDenyHostsChanged();
    void blockedRuleHostsChanged();
    void blockedRuleListIdsChanged();
    void exceptionRuleHostsChanged();
    void exceptionRuleListIdsChanged();
    void adblockSourceIdsChanged();
    void adblockEngineHandleChanged();
    void enabledChanged();
    void blockedCountChanged();
    void unknownContextCountChanged();
    void blockedSiteCountsChanged();

private:
    struct PolicySnapshot {
        QStringList blockedHosts;
        QStringList exceptionHosts;
        QStringList bypassSites;
        QStringList securityDenyHosts;
        QVariantMap blockedRuleLists;
        QVariantMap exceptionRuleLists;
        QStringList adblockSourceIds;
        std::shared_ptr<AdblockEngine> adblockEngine;
    };

    static QString normalizedHost(const QString &host);
    static bool hostMatches(const QString &host, const QString &pattern);
    static QStringList validatedHosts(const QStringList &hosts);
    void updateBlockedRuleLists();
    void updateExceptionRuleLists();
    void recordDecision(const QString &site, const QVariantMap &decision);
    void scheduleEvidenceChanged();
    void detachProfiles();

    mutable QReadWriteLock policyLock_;
    std::shared_ptr<const PolicySnapshot> policy_;
    QStringList blockedRuleHosts_;
    QStringList blockedRuleListIds_;
    QStringList exceptionRuleHosts_;
    QStringList exceptionRuleListIds_;
    bool enabled_ = false;
    std::atomic<qulonglong> blockedCount_{0};
    std::atomic<qulonglong> unknownContextCount_{0};
    mutable QReadWriteLock countsLock_;
    QHash<QString, qulonglong> blockedSiteCounts_;
    QHash<QString, QVariantMap> blockedRequestExplanations_;
    QHash<QString, QList<QVariantMap>> requestDecisionLog_;
    std::atomic_bool evidenceSignalPending_{false};
    QList<QPointer<QQuickWebEngineProfile>> profiles_;
};

#endif
