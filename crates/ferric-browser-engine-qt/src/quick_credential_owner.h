#pragma once

#include "public_page_owner.h"
#include <QPointer>
#include <QQuickItem>
#include <QWebEngineFrame>

class QWebChannel;
class QWebEngineLoadingInfo;
class QWebEngineScript;
class QWindow;
class FerricCredentialBoundary;

// Native incarnation tracking over a real, application-created Quick view.
// Only documented QML properties/signals and public Core value types cross
// this seam. No Qt private view/page pointer or JavaScript origin claim is used.
class FerricQuickCredentialOwner final : public QObject {
    Q_OBJECT
public:
    using Operation = FerricPublicPageOwner::Operation;
    static FerricQuickCredentialOwner *create(QQuickItem *view);
    ~FerricQuickCredentialOwner() override;
    bool isReady() const;
    Operation captureOperation() const;
    bool isCurrent(const Operation &operation) const;
    QWebEngineFrame mainFrame() const;
    QWindow *presentationWindow() const;
    FerricCredentialBoundary *prepareBoundary(std::function<QString()> contextIdentity);
protected:
    bool eventFilter(QObject *watched, QEvent *event) override;
signals:
    void authorityInvalidated();
private slots:
    void loadingChanged(const QWebEngineLoadingInfo &info);
    void profileChanged();
    void lifecycleChanged();
    void rendererChanged();
    void rendererTerminated();
    void channelChanged();
    void presentationChanged();
private:
    friend class FerricCredentialBoundary;
    explicit FerricQuickCredentialOwner(QQuickItem *view);
    void retire();
    void invalidateDocument();
    void watchWindow();
    QWebChannel *createChannel(QObject *parent);
    bool installChannel(QWebChannel *channel, const QWebEngineScript &script);
    void removeChannel(QWebChannel *channel, const QWebEngineScript &script);
    bool channelMatches(QWebChannel *channel) const;
    QPointer<QQuickItem> view_;
    QPointer<QObject> profile_;
    QPointer<QWindow> window_;
    QPointer<QWebChannel> channel_;
    QList<QMetaObject::Connection> windowConnections_;
    Operation operation_;
    qint64 renderer_ = 0;
    bool retired_ = false;
    bool ready_ = false;
    bool installing_ = false;
};
