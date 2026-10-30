#include "credential_boundary.h"
#include "quick_credential_owner.h"
#include <QWindow>

#include <QDialog>
#include <QDialogButtonBox>
#include <QGuiApplication>
#include <QFile>
#include <QJsonDocument>
#include <QJsonArray>
#include <QJsonObject>
#include <QLabel>
#include <QPushButton>
#include <QThread>
#include <QUuid>
#include <QUrl>
#include <QVBoxLayout>
#include <QWebChannel>
#include <QWebEngineFrame>
#include <QWebEngineScript>
#include <QWebEngineScriptCollection>
#include <QWidget>
#include <limits>

namespace {
constexpr auto ClientResource = ":/qt/qml/io/github/ferricbrowser/qml/scripts/credential/qwebchannel.js";
constexpr auto MediatorResource = ":/qt/qml/io/github/ferricbrowser/qml/scripts/credential/mediator.js";
QString readResource(const char *path) {
    QFile file(QString::fromLatin1(path));
    return file.open(QIODevice::ReadOnly) ? QString::fromUtf8(file.readAll()) : QString();
}
QString literal(const QString &value) {
    QJsonArray values; values.append(value);
    const auto encoded = QJsonDocument(values).toJson(QJsonDocument::Compact);
    return QString::fromUtf8(encoded.mid(1, encoded.size() - 2));
}
QString mediatorSource(const QString &client, const QString &mediator) {
    // The creation script and enabling the feature in an existing document
    // can meet. Only one receiver may own this private world/document.
    return QStringLiteral("(function(){if(Object.prototype.hasOwnProperty.call(globalThis,'__ferricCredentialMediator'))return true;"
                          "if(!window.isSecureContext||location.protocol!=='https:'||typeof qt==='undefined'||!qt.webChannelTransport)return false;\n")
        + client + QLatin1Char('\n') + mediator
        + QStringLiteral("\nreturn typeof globalThis.__ferricCredentialMediator==='object';})();");
}
}

FerricCredentialNotification::FerricCredentialNotification(FerricCredentialBoundary *boundary)
    : QObject(boundary), boundary_(boundary) {}
void FerricCredentialNotification::notify() { if (boundary_) boundary_->notification(); }

FerricCredentialBoundary::FerricCredentialBoundary(FerricPublicPageOwner *owner,
                                                   QWidget *dialogParent, std::function<QString()> contextIdentity)
    : QObject(owner), owner_(owner), dialogParent_(dialogParent), contextIdentity_(std::move(contextIdentity)) {
    if (!owner || !owner->page() || QThread::currentThread() != owner->thread()
        || owner->page()->webChannel()) return;
    const auto client = readResource(ClientResource);
    const auto mediator = readResource(MediatorResource);
    if (client.isEmpty() || mediator.isEmpty()) return;
    channel_ = new QWebChannel(this);
    notification_ = new FerricCredentialNotification(this);
    channel_->registerObject(QStringLiteral("credentialNotification"), notification_);
    owner->page()->setWebChannel(channel_, World);
    QWebEngineScript script;
    script.setName(QStringLiteral("ferric-credential-mediator-v1"));
    script.setWorldId(World);
    script.setInjectionPoint(QWebEngineScript::DocumentCreation);
    script.setRunsOnSubFrames(false);
    script.setSourceCode(mediatorSource(client, mediator));
    owner->page()->scripts().insert(script);
    mediatorScript_ = script;
    connect(owner, &FerricPublicPageOwner::authorityInvalidated, this, &FerricCredentialBoundary::invalidate);
    if (dialogParent) parentDestroyed_ = connect(dialogParent, &QObject::destroyed, this, &FerricCredentialBoundary::invalidate);
    initializeTimers();
    ready_ = true;
}

void FerricCredentialBoundary::initializeTimers() {
    coalesce_.setSingleShot(true);
    coalesce_.setInterval(25);
    connect(&coalesce_, &QTimer::timeout, this, &FerricCredentialBoundary::challenge);
    deadline_.setSingleShot(true);
    deadline_.setInterval(180000);
    connect(&deadline_, &QTimer::timeout, this, [this] {
        emit cancelRequested();
        if (!complete("{\"error\":\"NotAllowedError\"}")) invalidate();
    });
    deliveryDeadline_.setSingleShot(true);
    deliveryDeadline_.setInterval(1000);
    connect(&deliveryDeadline_, &QTimer::timeout, this, [this] {
        invalidate();
        emit responseCompleted(false);
    });
}

FerricCredentialBoundary::FerricCredentialBoundary(FerricQuickCredentialOwner *owner,
                                                   QWindow *dialogParent, std::function<QString()> contextIdentity)
    : QObject(owner), quickOwner_(owner), dialogWindow_(dialogParent), contextIdentity_(std::move(contextIdentity)) {
    if (!owner || !owner->isReady()) return;
    const auto client = readResource(ClientResource);
    const auto mediator = readResource(MediatorResource);
    if (client.isEmpty() || mediator.isEmpty()) return;
    channel_ = owner->createChannel(this);
    if (!channel_) return;
    notification_ = new FerricCredentialNotification(this);
    channel_->registerObject(QStringLiteral("credentialNotification"), notification_);
    mediatorScript_.setName(QStringLiteral("ferric-credential-mediator-v1"));
    mediatorScript_.setWorldId(World);
    mediatorScript_.setInjectionPoint(QWebEngineScript::DocumentCreation);
    mediatorScript_.setRunsOnSubFrames(false);
    mediatorScript_.setSourceCode(mediatorSource(client, mediator));
    if (!owner->installChannel(channel_, mediatorScript_)) return;
    connect(owner, &FerricQuickCredentialOwner::authorityInvalidated, this, &FerricCredentialBoundary::invalidate);
    if (dialogParent) parentDestroyed_ = connect(dialogParent, &QObject::destroyed, this, &FerricCredentialBoundary::invalidate);
    initializeTimers();
    ready_ = true;
}
FerricCredentialBoundary::~FerricCredentialBoundary() {
    invalidate();
    if (quickOwner_) quickOwner_->removeChannel(channel_, mediatorScript_);
    if (owner_ && owner_->page() && owner_->page()->webChannel() == channel_) {
        owner_->page()->setWebChannel(nullptr, World);
        auto scripts = owner_->page()->scripts().find(QStringLiteral("ferric-credential-mediator-v1"));
        for (const auto &script : scripts) owner_->page()->scripts().remove(script);
    }
}

void FerricCredentialBoundary::ensureMediatorInCurrentDocument(int attempts) {
    if (!isReady() || attempts <= 0) return;
    const auto operation = captureOperation();
    auto frame = mainFrame();
    if (!frame.isValid() || !frame.isMainFrame() || frame.url().scheme() != QLatin1String("https")) return;
    const QPointer<FerricCredentialBoundary> self(this);
    frame.runJavaScript(mediatorScript_.sourceCode(), World,
        [self, operation, attempts](const QVariant &result) {
            if (!self || !self->operationCurrent(operation) || result.toBool() || attempts <= 1) return;
            QTimer::singleShot(25, self, [self, operation, attempts] {
                if (self && self->operationCurrent(operation))
                    self->ensureMediatorInCurrentDocument(attempts - 1);
            });
        });
}
bool FerricCredentialBoundary::isReady() const {
    return ready_ && ((owner_ && owner_->page()) || (quickOwner_ && quickOwner_->channelMatches(channel_)));
}
FerricPublicPageOwner::Operation FerricCredentialBoundary::captureOperation() const {
    return owner_ ? owner_->captureOperation() : quickOwner_ ? quickOwner_->captureOperation() : FerricPublicPageOwner::Operation{};
}
bool FerricCredentialBoundary::operationCurrent(const FerricPublicPageOwner::Operation &operation) const {
    return owner_ ? owner_->isCurrent(operation) : quickOwner_ && quickOwner_->isCurrent(operation);
}
QWebEngineFrame FerricCredentialBoundary::mainFrame() const {
    return owner_ && owner_->page() ? owner_->page()->mainFrame() : quickOwner_ ? quickOwner_->mainFrame() : QWebEngineFrame();
}
bool FerricCredentialBoundary::current() const {
    return captured_ && operationCurrent(request_.owner) && contextIdentity_ && contextIdentity_() == request_.contextIdentity;
}
const FerricCredentialBoundary::CapturedRequest *FerricCredentialBoundary::request() const {
    return current() ? &request_ : nullptr;
}
void FerricCredentialBoundary::notification() {
    if (!isReady()) return;
    if (completing_ || challenging_) { notificationPending_ = true; return; }
    if (!coalesce_.isActive()) coalesce_.start();
}
void FerricCredentialBoundary::challenge() {
    if (!isReady() || !contextIdentity_ || contextIdentity_().isEmpty() || challenging_ || challengeSequence_ == std::numeric_limits<quint64>::max()) return;
    auto frame = mainFrame();
    if (!frame.isValid() || !frame.isMainFrame()) return;
    const auto operation = captureOperation();
    const auto context = contextIdentity_();
    const auto challengeId = QUuid::createUuid().toString(QUuid::WithoutBraces);
    const auto sequence = ++challengeSequence_;
    challenging_ = true;
    QPointer<FerricCredentialBoundary> guard(this);
    frame.runJavaScript(QStringLiteral("globalThis.__ferricCredentialMediator ? "
        "globalThis.__ferricCredentialMediator.challenge(%1) : ''").arg(literal(challengeId)), World,
        [guard, operation, context, challengeId, sequence](const QVariant &result) {
        if (!guard || sequence != guard->challengeSequence_) return;
        guard->challenging_ = false;
        if (guard->notificationPending_) {
            guard->notificationPending_ = false;
            guard->coalesce_.start();
        }
        if (!guard->operationCurrent(operation)
            || !guard->contextIdentity_ || guard->contextIdentity_() != context) return;
        const auto encoded = result.toString().toUtf8();
        if (encoded.isEmpty() || encoded.size() > 1048576) return;
        QJsonParseError error;
        const auto document = QJsonDocument::fromJson(encoded, &error);
        if (error.error != QJsonParseError::NoError || !document.isObject()) return;
        const auto row = document.object();
        const auto origin = row.value("origin").toString();
        const QUrl url(origin, QUrl::StrictMode);
        if (row.value("challenge").toString() != challengeId
            || !row.value("secureContext").toBool() || !row.value("policyAllowed").toBool()
            || url.scheme() != "https" || url.host().isEmpty() || !url.userInfo().isEmpty()
            || !url.path().isEmpty() || url.hasQuery() || url.hasFragment()) return;
        const auto op = row.value("op").toString();
        const auto id = row.value("id").toString();
        const auto documentNonce = row.value("documentNonce").toString();
        const auto requestNonce = row.value("requestNonce").toString();
        if ((op != "get" && op != "create") || id.isEmpty() || id.size() > 128
            || QUuid(documentNonce).isNull() || QUuid(requestNonce).isNull()
            || !row.value("options").isObject()) return;
        const bool cancelled = row.value("cancelled").toBool();
        if (guard->captured_) {
            if (cancelled && guard->request_.requestNonce == requestNonce) {
                emit guard->cancelRequested();
                if (!guard->complete("{\"error\":\"AbortError\"}")) guard->invalidate();
            }
            return;
        }
        guard->request_ = {operation, documentNonce, requestNonce, id, origin, op,
                           QJsonDocument(row.value("options").toObject()).toJson(QJsonDocument::Compact), context};
        guard->capturedFrame_ = guard->mainFrame();
        guard->captured_ = true;
        if (cancelled) {
            emit guard->cancelRequested();
            if (!guard->complete("{\"error\":\"AbortError\"}")) guard->invalidate();
            return;
        }
        guard->deadline_.start();
        emit guard->requestCaptured();
    });
}
bool FerricCredentialBoundary::showProviderChoice(bool phoneAvailable) {
    const bool widgetActive = dialogParent_ && dialogParent_->isVisible() && dialogParent_->window()->isActiveWindow();
    const bool quickActive = dialogWindow_ && dialogWindow_->isVisible() && dialogWindow_->isActive();
    QPointer<QWindow> requestWindow = dialogWindow_ ? dialogWindow_.data() :
        dialogParent_ ? dialogParent_->window()->windowHandle() : nullptr;
    if (!current() || providerConfirmed_ || dialog_ || (!widgetActive && !quickActive)
        || !requestWindow || QGuiApplication::focusWindow() != requestWindow) return false;
    auto *dialog = new QDialog(dialogParent_);
    dialog_ = dialog;
    dialog->setObjectName(QStringLiteral("ferricCredentialProviderChoice"));
    dialog->setAttribute(Qt::WA_DeleteOnClose);
    dialog->setWindowModality(Qt::WindowModal);
    if (dialogWindow_) {
        dialog->winId();
        dialog->windowHandle()->setTransientParent(dialogWindow_);
    }
    dialog->setWindowTitle(QStringLiteral("Choose an authenticator"));
    auto *layout = new QVBoxLayout(dialog);
    auto *site = new QLabel(request_.origin, dialog);
    site->setTextFormat(Qt::PlainText);
    site->setAccessibleName(QStringLiteral("Requesting site"));
    layout->addWidget(site);
    auto *buttons = new QDialogButtonBox(dialog);
    auto *native = buttons->addButton(QStringLiteral("Security key or native authenticator"), QDialogButtonBox::AcceptRole);
    native->setObjectName(QStringLiteral("credentialNativeProvider"));
    auto *phone = buttons->addButton(QStringLiteral("Use a phone"), QDialogButtonBox::ActionRole);
    phone->setObjectName(QStringLiteral("credentialPhoneProvider"));
    phone->setEnabled(phoneAvailable);
    auto *cancel = buttons->addButton(QDialogButtonBox::Cancel);
    cancel->setDefault(true);
    layout->addWidget(buttons);
    auto choose = [this, dialog](bool phoneSelected) {
        const auto *nativeWindow = dialog->windowHandle();
        if (!current() || !dialog->isVisible() || !nativeWindow || !nativeWindow->isActive()
            || QGuiApplication::focusWindow() != nativeWindow) { dialog->reject(); return; }
        providerConfirmed_ = true;
        dialog->accept();
        emit providerChosen(phoneSelected);
    };
    connect(native, &QPushButton::clicked, this, [choose] { choose(false); });
    connect(phone, &QPushButton::clicked, this, [choose] { choose(true); });
    connect(cancel, &QPushButton::clicked, dialog, &QDialog::reject);
    connect(dialog, &QDialog::rejected, this, [this] {
        emit cancelRequested();
        if (!complete("{\"error\":\"NotAllowedError\"}")) invalidate();
    });
    dialog->open();
    QTimer::singleShot(0, dialog, [requestWindow, dialog] {
        // QWindow::isActive also admits an active transient ancestor. Actual
        // input focus must move to this chooser before it accepts a decision.
        // Request activation only while the captured requesting window owns
        // focus; a later desktop switch must not pull focus back.
        if (dialog->isVisible() && requestWindow && QGuiApplication::focusWindow() == requestWindow)
            dialog->windowHandle()->requestActivate();
    });
    return true;
}
bool FerricCredentialBoundary::complete(const QByteArray &response) {
    if (!current() || completing_ || response.size() > 1048576
        || challengeSequence_ == std::numeric_limits<quint64>::max()) return false;
    QJsonParseError error;
    const auto parsed = QJsonDocument::fromJson(response, &error);
    if (error.error != QJsonParseError::NoError || !parsed.isObject()
        || (!providerConfirmed_ && !parsed.object().value("error").isString())) return false;
    // Completion owns the next challenge sequence. Retire an in-flight
    // notification probe; its stale callback must not leave challenging_ set.
    coalesce_.stop();
    notificationPending_ = false;
    challenging_ = false;
    completing_ = true;
    deliveryDeadline_.start();
    const auto nonce = request_.requestNonce;
    const auto operation = request_.owner;
    const auto docNonce = request_.documentNonce;
    const auto origin = request_.origin;
    const auto challengeId = QUuid::createUuid().toString(QUuid::WithoutBraces);
    const auto sequence = ++challengeSequence_;
    QPointer<FerricCredentialBoundary> guard(this);
    mainFrame().runJavaScript(QStringLiteral(
        "globalThis.__ferricCredentialMediator ? globalThis.__ferricCredentialMediator.challenge(%1) : ''")
        .arg(literal(challengeId)), World,
        [guard, operation, nonce, docNonce, origin, challengeId, sequence, response](const QVariant &value) {
        if (!guard || sequence != guard->challengeSequence_ || !guard->current()
            || !guard->operationCurrent(operation)) return;
        const auto bytes = value.toString().toUtf8();
        if (bytes.size() > 1048576) return;
        const auto row = QJsonDocument::fromJson(bytes).object();
        if (row.value("challenge").toString() != challengeId || row.value("documentNonce").toString() != docNonce
            || row.value("requestNonce").toString() != nonce || row.value("origin").toString() != origin
            || !row.value("secureContext").toBool() || !row.value("policyAllowed").toBool()) return;
        guard->mainFrame().runJavaScript(QStringLiteral(
            "globalThis.__ferricCredentialMediator.complete(%1, %2, %3, JSON.parse(%4))")
            .arg(literal(nonce), literal(docNonce), literal(origin), literal(QString::fromUtf8(response))), World,
            [guard, operation, sequence](const QVariant &ack) {
                if (!guard || sequence != guard->challengeSequence_ || !guard->current()
                    || !guard->operationCurrent(operation)) return;
                const bool delivered = ack.toBool();
                if (!delivered) { guard->invalidate(); emit guard->responseCompleted(false); return; }
                guard->captured_ = false;
                guard->completing_ = false;
                guard->challenging_ = false;
                guard->providerConfirmed_ = false;
                guard->deadline_.stop(); guard->deliveryDeadline_.stop();
                if (guard->dialog_) guard->dialog_->close();
                guard->request_ = {};
                guard->capturedFrame_ = {};
                if (guard->notificationPending_) {
                    guard->notificationPending_ = false;
                    guard->coalesce_.start();
                }
                emit guard->responseCompleted(true);
            });
    });
    return true;
}
void FerricCredentialBoundary::invalidate() {
    if (challengeSequence_ == std::numeric_limits<quint64>::max()) ready_ = false;
    else ++challengeSequence_;
    notificationPending_ = false;
    coalesce_.stop(); deadline_.stop(); deliveryDeadline_.stop();
    challenging_ = false;
    completing_ = false;
    providerConfirmed_ = false;
    const bool wasCaptured = captured_;
    auto revokedFrame = capturedFrame_.isValid() ? capturedFrame_ : mainFrame();
    if (captured_ && revokedFrame.isValid()) {
        // Revocation may leave the same document alive (transfer or a failed
        // navigation). Release only its exact private pending request, with an
        // error; a replacement document cannot match these nonces.
        revokedFrame.runJavaScript(QStringLiteral(
            "globalThis.__ferricCredentialMediator && globalThis.__ferricCredentialMediator.complete(%1, %2, %3, {error:'AbortError'})")
            .arg(literal(request_.requestNonce), literal(request_.documentNonce), literal(request_.origin)), World);
    }
    captured_ = false;
    request_ = {};
    capturedFrame_ = {};
    if (dialog_) { dialog_->blockSignals(true); dialog_->close(); }
    if (wasCaptured) emit cancelled();
}

void FerricCredentialBoundary::bindPresentation(QWidget *parent, std::function<QString()> contextIdentity) {
    invalidate();
    QObject::disconnect(parentDestroyed_);
    dialogParent_ = parent;
    dialogWindow_.clear();
    if (parent) parentDestroyed_ = connect(parent, &QObject::destroyed, this, &FerricCredentialBoundary::invalidate);
    contextIdentity_ = std::move(contextIdentity);
}

void FerricCredentialBoundary::bindPresentation(QWindow *parent, std::function<QString()> contextIdentity) {
    invalidate();
    QObject::disconnect(parentDestroyed_);
    dialogParent_.clear();
    dialogWindow_ = parent;
    if (parent) parentDestroyed_ = connect(parent, &QObject::destroyed, this, &FerricCredentialBoundary::invalidate);
    contextIdentity_ = std::move(contextIdentity);
}
