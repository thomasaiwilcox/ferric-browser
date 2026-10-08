#ifndef FERRIC_BROWSER_VERSIONING_ARCHIVE_INTERCEPTOR_H
#define FERRIC_BROWSER_VERSIONING_ARCHIVE_INTERCEPTOR_H

#include <QtCore/QPointer>
#include <QtCore/QReadWriteLock>
#include <QtCore/QString>
#include <QtQml/qqmlregistration.h>
#include <QtWebEngineCore/QWebEngineUrlRequestInfo>
#include <QtWebEngineCore/QWebEngineUrlRequestInterceptor>
#include <QtWebEngineQuick/QQuickWebEngineProfile>

class FerricVersioningArchiveInterceptor : public QWebEngineUrlRequestInterceptor {
    Q_OBJECT
    QML_NAMED_ELEMENT(VersioningArchiveInterceptor)

public:
    explicit FerricVersioningArchiveInterceptor(QObject *parent = nullptr);
    ~FerricVersioningArchiveInterceptor() override;

    Q_INVOKABLE bool attach(QObject *profile);
    Q_INVOKABLE bool authorizeArchiveUrl(const QString &url);
    Q_INVOKABLE void clearAuthorization();

    void interceptRequest(QWebEngineUrlRequestInfo &info) override;
    // Fixed-category, saturating in-memory diagnostics for native
    // qualification only; request URLs and payloads are never retained.
    quint64 blockedRequestCount(QWebEngineUrlRequestInfo::ResourceType type) const;

private:
    void detachProfiles();

    mutable QReadWriteLock authorizationLock_;
    QString authorizedArchiveUrl_;
    bool initialMainFramePending_{false};
    bool blankMainFramePending_{false};
    QList<QPointer<QQuickWebEngineProfile>> profiles_;
    quint64 blockedMainFrameRequests_{0};
    quint64 blockedSubFrameRequests_{0};
    quint64 blockedStylesheetRequests_{0};
    quint64 blockedImageRequests_{0};
    quint64 blockedOtherRequests_{0};
};

#endif
