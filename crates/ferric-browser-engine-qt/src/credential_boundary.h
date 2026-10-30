#pragma once

#include "public_page_owner.h"
#include <QByteArray>
#include <QPointer>
#include <QTimer>
#include <QWebEngineFrame>
#include <QWebEngineScript>
#include <functional>

class QDialog;
class QWebChannel;
class QWidget;
class QWindow;
class FerricQuickCredentialOwner;
class FerricCredentialBoundary;

// This is the entire remote native surface. A notification carries no origin,
// frame, target, options, authority or result destination.
class FerricCredentialNotification final : public QObject {
    Q_OBJECT
public:
    explicit FerricCredentialNotification(FerricCredentialBoundary *boundary);
    Q_INVOKABLE void notify();
private:
    QPointer<FerricCredentialBoundary> boundary_;
};

// Native adapter for one actual public-page owner. It captures requests and
// presents native provider choice; it cannot start an authenticator/helper.
class FerricCredentialBoundary final : public QObject {
    Q_OBJECT
public:
    static constexpr quint32 World = 42;
    struct CapturedRequest {
        FerricPublicPageOwner::Operation owner;
        QString documentNonce;
        QString requestNonce;
        QString id;
        QString origin;
        QString operation;
        QByteArray options;
        QString contextIdentity;
    };
    ~FerricCredentialBoundary() override;
    bool isReady() const;
    void bindPresentation(QWidget *dialogParent, std::function<QString()> contextIdentity);
    void bindPresentation(QWindow *dialogParent, std::function<QString()> contextIdentity);
    const CapturedRequest *request() const;
    // Caller supplies already-established provider eligibility. This adapter
    // never invents phone capability or invokes either provider itself.
    bool showProviderChoice(bool phoneAvailable);
    bool complete(const QByteArray &response, std::function<bool()> deliveryAdmission = {});
    bool enablePhoneShim();
    void disablePhoneShim();
    void invalidate();
    void notification();
signals:
    void requestCaptured();
    void providerChosen(bool phone);
    void cancelled();
    void cancelRequested();
    void responseCompleted(bool delivered);
private:
    friend class FerricPublicPageHost;
    friend class FerricQuickCredentialOwner;
    friend class FerricCredentialBoundaryTestAccess;
    explicit FerricCredentialBoundary(FerricPublicPageOwner *owner,
                                      QWidget *dialogParent, std::function<QString()> contextIdentity);
    explicit FerricCredentialBoundary(FerricQuickCredentialOwner *owner,
                                      QWindow *dialogParent, std::function<QString()> contextIdentity);
    void initializeTimers();
    void ensureMediatorInCurrentDocument(int attempts = 5);
    FerricPublicPageOwner::Operation captureOperation() const;
    bool operationCurrent(const FerricPublicPageOwner::Operation &operation) const;
    QWebEngineFrame mainFrame() const;
    void challenge();
    bool current() const;
    QPointer<FerricPublicPageOwner> owner_;
    QPointer<FerricQuickCredentialOwner> quickOwner_;
    QPointer<QWidget> dialogParent_;
    QPointer<QWindow> dialogWindow_;
    QPointer<QDialog> dialog_;
    QWebChannel *channel_ = nullptr;
    FerricCredentialNotification *notification_ = nullptr;
    QWebEngineScript mediatorScript_;
    QWebEngineScript shimScript_;
    QWebEngineFrame capturedFrame_;
    std::function<QString()> contextIdentity_;
    QTimer coalesce_;
    QTimer deadline_;
    QTimer deliveryDeadline_;
    QMetaObject::Connection parentDestroyed_;
    CapturedRequest request_;
    quint64 challengeSequence_ = 0;
    bool ready_ = false;
    bool captured_ = false;
    bool challenging_ = false;
    bool completing_ = false;
    bool notificationPending_ = false;
    bool providerConfirmed_ = false;
};
