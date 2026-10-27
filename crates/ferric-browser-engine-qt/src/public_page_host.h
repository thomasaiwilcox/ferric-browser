#ifndef FERRIC_BROWSER_ENGINE_QT_PUBLIC_PAGE_HOST_H
#define FERRIC_BROWSER_ENGINE_QT_PUBLIC_PAGE_HOST_H

#include <QPointer>
#include <QString>
#include <QVariant>
#include <QWidget>

class QQuickWidget;
class QVBoxLayout;
class QWebEngineProfile;
class QWebEngineView;

class FerricPublicPageOwner;

// A disposable Widgets presentation for a page whose profile/page owner has a
// longer lifetime. Destroying this widget never closes the page owner.
class FerricPublicPageHost final : public QWidget {
public:
    // initialOwner is bootstrap-only. Every later view move must consume a
    // current Rust ReparentView request through applyNativeRequest().
    explicit FerricPublicPageHost(
        QObject *browserUi,
        QWebEngineProfile *profile,
        FerricPublicPageOwner *initialOwner = nullptr,
        QWidget *parent = nullptr,
        // Empty captures the current Rust-selected tab; Rust captures Target.
        QString initialTabId = {});
    ~FerricPublicPageHost() override;

    bool isReady() const;
    bool applyNativeRequest(const QVariant &request);
    FerricPublicPageOwner *pageOwner() const;
    QWebEngineView *pageView() const;
    QQuickWidget *chromeView() const;
    QWidget *nativeDialogParent() const;
    QString hostToken() const;
    QString pageOwnerToken() const;

private:
    friend bool ferric_public_page_host_tokens_live(
        const QString &hostToken,
        const QString &pageOwnerToken,
        const QString &profileId,
        bool attach);

    bool registerInitialPage(FerricPublicPageOwner *owner, const QString &tabId);
    QWebEngineView *takePageViewForTransfer(FerricPublicPageOwner *owner);
    bool acceptTransferredPage(
        FerricPublicPageOwner *owner,
        QWebEngineView *view,
        const QString &pageOwnerToken,
        bool restoreFocus);

    QPointer<QObject> m_browserUi;
    QPointer<FerricPublicPageOwner> m_pageOwner;
    QPointer<QWebEngineProfile> m_profile;
    QQuickWidget *m_chromeView = nullptr;
    QWebEngineView *m_pageView = nullptr;
    QVBoxLayout *m_layout = nullptr;
    QString m_hostToken;
    QString m_pageOwnerToken;
    QString m_profileId;
    bool m_detached = false;
    bool m_pageViewHadFocus = false;
};

bool ferric_public_page_host_tokens_live(
    const QString &hostToken,
    const QString &pageOwnerToken,
    const QString &profileId,
    bool attach);

#endif // FERRIC_BROWSER_ENGINE_QT_PUBLIC_PAGE_HOST_H
