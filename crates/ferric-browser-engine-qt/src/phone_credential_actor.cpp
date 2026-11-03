#include "phone_credential_actor.h"
#include "quick_credential_binding.h"
#include "credential_boundary.h"
#include "credential_process.h"
#include "credential_network_policy.h"
#include "ferric-browser-engine-qt/src/bridge.cxxqt.h"

#include <QApplication>
#include <QDialog>
#include <QDialogButtonBox>
#include <QElapsedTimer>
#include <QFileInfo>
#include <QImage>
#include <QLabel>
#include <QPixmap>
#include <QPointer>
#include <QPushButton>
#include <QSet>
#include <QTimer>
#include <QVBoxLayout>
#include <QWindow>

namespace {
using ferric::credentials::CredentialProcess;
using ferric::credentials::CredentialProcessBridge;

bool directNetworkAvailable(FerricQuickCredentialBinding *binding)
{
    const auto *request = binding && binding->boundary() ? binding->boundary()->request() : nullptr;
    return request && FerricCredentialNetworkPolicy::direct(QUrl(request->origin));
}

bool phoneAvailable(FerricQuickCredentialBinding *binding)
{
    return directNetworkAvailable(binding)
        && QFileInfo(QCoreApplication::applicationDirPath() + QStringLiteral("/ferric-phone-auth-helper")).isExecutable();
}

rust::Slice<const std::uint8_t> slice(const QByteArray &bytes)
{
    return {reinterpret_cast<const std::uint8_t *>(bytes.constData()), static_cast<size_t>(bytes.size())};
}
}

struct FerricPhoneCredentialActor::State {
    rust::Box<ferric::credentials::CredentialActor> actor = ferric::credentials::new_credential_actor();
    CredentialProcess process;
    QElapsedTimer clock;
    QTimer monitor;
    QPointer<FerricQuickCredentialBinding> binding;
    QPointer<QDialog> dialog;
    QPointer<QLabel> qr;
    QPointer<QLabel> status;
    QList<QMetaObject::Connection> watches;
    QSet<FerricQuickCredentialBinding *> attached;
    quint64 ceremony = 0;
    quint64 helper = 0;
    bool terminal = false;
    bool terminalSuccess = false;
    unsigned short accountCount = 0;
    bool phone = false;
    bool submitting = false;
    bool delivering = false;
    bool delivered = false;
    quint64 now() const { return static_cast<quint64>(clock.elapsed()); }
};

void FerricPhoneCredentialActor::attach(FerricQuickCredentialBinding *binding)
{
    if (!binding || !qApp || binding->thread() != qApp->thread()) return;
    static QPointer<FerricPhoneCredentialActor> shared;
    if (!shared) shared = new FerricPhoneCredentialActor(qApp);
    if (shared->state_->attached.contains(binding)) return;
    shared->state_->attached.insert(binding);
    const QPointer<FerricPhoneCredentialActor> self(shared);
    connect(binding, &FerricQuickCredentialBinding::requestReady, shared, [self, binding] { if (self) self->begin(binding); });
    connect(binding, &QObject::destroyed, shared, [self, binding] { if (self) self->state_->attached.remove(binding); });
}

FerricPhoneCredentialActor::FerricPhoneCredentialActor(QObject *parent)
    : QObject(parent), state_(std::make_unique<State>())
{
    state_->clock.start();
    state_->monitor.setInterval(100);
    connect(&state_->monitor, &QTimer::timeout, this, [this] {
        if (!current() || (state_->phone && !directNetworkAvailable(state_->binding))) fail();
    });
    connect(qApp, &QCoreApplication::aboutToQuit, this, &FerricPhoneCredentialActor::reset);
    CredentialProcess::Events events;
    events.started = [this](quint64 token) {
        if (!state_->submitting && token != state_->helper) { state_->process.cancel(token); return; }
        // Capture the real token before rejection: cancellation must prevent
        // the supervisor's started callback from submitting stdin even when
        // started is delivered synchronously inside start().
        state_->helper = token;
        if (!current() || !directNetworkAvailable(state_->binding) || !state_->binding->phoneNetworkAllowed()
            || !state_->actor->helper_started(state_->ceremony, state_->now())) { fail(); return; }
    };
    events.admitFrame = [this](quint64 token, const QByteArray &bytes) -> int {
        if (!current() || token != state_->helper || !directNetworkAvailable(state_->binding) || !state_->binding->phoneNetworkAllowed()) return 0;
        auto update = state_->actor->frame(state_->ceremony, slice(bytes), state_->now());
        if (update.kind == 0) return 0;
        if (update.kind == 1) {
            if (!state_->qr || update.modules.size() != static_cast<size_t>(update.width) * update.width) return 0;
            const int border = 4;
            QImage image(update.width + border * 2, update.width + border * 2, QImage::Format_RGB32);
            image.fill(Qt::white);
            for (int y = 0; y < update.width; ++y)
                for (int x = 0; x < update.width; ++x)
                    if (update.modules[static_cast<size_t>(y) * update.width + x]) image.setPixelColor(x + border, y + border, Qt::black);
            state_->qr->setPixmap(QPixmap::fromImage(image.scaled(370, 370, Qt::KeepAspectRatio, Qt::FastTransformation)));
            if (state_->status) state_->status->setText(tr("Scan with your phone's camera, then follow the instructions on your phone."));
            return 1;
        }
        if (update.kind == 2) {
            if (state_->status) state_->status->setText(update.payload == "submitted" ? tr("Continue on your phone.") : tr("Finishing…"));
            return 1;
        }
        state_->terminal = true;
        state_->terminalSuccess = update.kind == 3;
        state_->accountCount = update.width;
        // Do not deliver until the process exits cleanly. Trailing output,
        // truncated frames or a crash must invalidate an apparent result.
        return 2;
    };
    events.finished = [this](quint64 token, CredentialProcess::Failure outcome, int) {
        if (token != state_->helper && !state_->submitting) return;
        state_->helper = 0;
        if (outcome != CredentialProcess::Failure::None || !state_->terminal) { fail(); return; }
        const auto ceremony = state_->ceremony;
        QTimer::singleShot(0, this, [this, ceremony] {
            if (state_->ceremony != ceremony || !current()) return;
            if (!state_->terminalSuccess || state_->accountCount == 1) acceptCredential(0);
            else showAccounts(state_->accountCount);
        });
    };
    CredentialProcessBridge::install(state_->process, std::move(events), {});
}

FerricPhoneCredentialActor::~FerricPhoneCredentialActor() { reset(); }

bool FerricPhoneCredentialActor::current() const
{
    return state_->ceremony && state_->binding && state_->binding->hasCurrentLease()
        && state_->actor->current(state_->ceremony, state_->now())
        && (!state_->phone || state_->binding->phoneNetworkAllowed());
}

void FerricPhoneCredentialActor::begin(FerricQuickCredentialBinding *binding)
{
    QPointer<FerricCredentialBoundary> boundary(binding->boundary());
    if (!boundary || !binding->hasCurrentLease()) return;
    if (state_->ceremony) { boundary->complete("{\"error\":\"NotAllowedError\"}"); return; }
    const auto token = binding->admitActor(*state_->actor, state_->now());
    if (!token) { boundary->complete("{\"error\":\"TypeError\"}"); return; }
    state_->binding = binding;
    state_->ceremony = token;
    state_->terminal = false;
    state_->watches.append(connect(binding, &FerricQuickCredentialBinding::leaseInvalidated, this, &FerricPhoneCredentialActor::reset));
    state_->watches.append(connect(binding, &QObject::destroyed, this, &FerricPhoneCredentialActor::reset));
    state_->watches.append(connect(binding, &FerricQuickCredentialBinding::responseSettled, this, [this, token](bool delivered) {
        if (state_->ceremony == token) state_->delivered = delivered;
    }));
    state_->watches.append(connect(boundary, &FerricCredentialBoundary::cancelRequested, this, &FerricPhoneCredentialActor::reset));
    state_->watches.append(connect(boundary, &FerricCredentialBoundary::providerChosen, this, &FerricPhoneCredentialActor::provider));
    state_->monitor.start();
    if (!boundary->showProviderChoice(phoneAvailable(binding) && binding->phoneNetworkAllowed())) fail();
}

void FerricPhoneCredentialActor::provider(bool phone)
{
    if (!current()) { fail(); return; }
    if (!phone) {
        QPointer<FerricCredentialBoundary> boundary(state_->binding->boundary());
        reset();
        if (boundary) boundary->complete("{\"native\":true}");
        return;
    }
    auto request = state_->actor->helper_request(state_->ceremony, directNetworkAvailable(state_->binding) && state_->binding->phoneNetworkAllowed(), state_->now());
    if (request.empty()) { fail(); return; }
    state_->phone = true;
    showPhone();
    if (!current()) { fail(); return; }
    const auto ceremony = state_->ceremony;
    state_->submitting = true;
    auto token = CredentialProcessBridge::start(state_->process,
        QByteArray(reinterpret_cast<const char *>(request.data()), static_cast<qsizetype>(request.size())));
    state_->submitting = false;
    if (state_->ceremony != ceremony) { if (token) state_->process.cancel(token); return; }
    if (!token) { fail(); return; }
    // QProcess::started is asynchronous; the callback assigns the helper
    // token before accepting its first frame.
    state_->helper = token;
}

void FerricPhoneCredentialActor::showPhone()
{
    auto *dialog = new QDialog;
    dialog->setAttribute(Qt::WA_DeleteOnClose);
    dialog->setWindowTitle(tr("Use a phone passkey"));
    dialog->setWindowModality(Qt::WindowModal);
    auto *layout = new QVBoxLayout(dialog);
    auto *origin = new QLabel(state_->binding->boundary()->request()->origin, dialog);
    origin->setTextFormat(Qt::PlainText);
    layout->addWidget(origin);
    auto *status = new QLabel(tr("Preparing a private connection to your phone…"), dialog);
    status->setTextFormat(Qt::PlainText);
    status->setWordWrap(true);
    layout->addWidget(status);
    auto *qr = new QLabel(dialog);
    qr->setMinimumSize(370, 370);
    qr->setAlignment(Qt::AlignCenter);
    layout->addWidget(qr);
    auto *buttons = new QDialogButtonBox(QDialogButtonBox::Cancel, dialog);
    layout->addWidget(buttons);
    connect(buttons, &QDialogButtonBox::rejected, dialog, &QDialog::reject);
    connect(dialog, &QDialog::rejected, this, &FerricPhoneCredentialActor::fail);
    state_->dialog = dialog;
    state_->status = status;
    state_->qr = qr;
    dialog->winId();
    if (auto *window = state_->binding->presentationWindow()) dialog->windowHandle()->setTransientParent(window);
    dialog->show();
    dialog->raise();
    dialog->activateWindow();
}

void FerricPhoneCredentialActor::showAccounts(unsigned short count)
{
    if (!current() || !state_->dialog || !count || count > 16) { fail(); return; }
    if (state_->qr) state_->qr->hide();
    if (state_->status) state_->status->setText(tr("Choose a passkey returned by your phone."));
    const auto ceremony = state_->ceremony;
    QPointer<QDialog> dialog(state_->dialog);
    for (unsigned short index = 0; index < count; ++index) {
        const auto label = state_->actor->account_label(ceremony, index);
        if (label.empty()) { fail(); return; }
        auto *button = new QPushButton(QString::fromUtf8(label.data(), static_cast<qsizetype>(label.size())).replace('&', QStringLiteral("&&")), dialog);
        dialog->layout()->addWidget(button);
        connect(button, &QPushButton::clicked, this, [this, dialog, ceremony, index] {
            if (dialog && dialog == state_->dialog && state_->ceremony == ceremony
                && dialog->isVisible() && QGuiApplication::focusWindow() == dialog->windowHandle()) acceptCredential(index);
        });
    }
}

void FerricPhoneCredentialActor::acceptCredential(unsigned short index)
{
    if (state_->delivering) return;
    if (!current()) { fail(); return; }
    const auto response = state_->actor->response(state_->ceremony, index, state_->now());
    if (response.empty()) { fail(); return; }
    QPointer<FerricCredentialBoundary> boundary(state_->binding->boundary());
    state_->delivering = true;
    // Keep admission reserved until the boundary acknowledges or invalidates
    // delivery; its leaseInvalidated signal releases the actor exactly once.
    if (state_->dialog) { state_->dialog->blockSignals(true); state_->dialog->close(); state_->dialog.clear(); }
    const auto ceremony = state_->ceremony;
    const QPointer<FerricPhoneCredentialActor> self(this);
    if (!boundary || !boundary->complete(QByteArray(response.data(), static_cast<qsizetype>(response.size())),
        [self, ceremony] { return self && self->state_->ceremony == ceremony && self->current() && directNetworkAvailable(self->state_->binding); })) fail();
}

void FerricPhoneCredentialActor::fail()
{
    QPointer<FerricCredentialBoundary> boundary(state_->binding ? state_->binding->boundary() : nullptr);
    reset();
    if (boundary && !boundary->complete("{\"error\":\"NotAllowedError\"}")) boundary->invalidate();
}

void FerricPhoneCredentialActor::reset()
{
    const auto helper = state_->helper;
    state_->helper = 0;
    state_->monitor.stop();
    for (const auto &watch : std::as_const(state_->watches)) disconnect(watch);
    state_->watches.clear();
    if (state_->dialog) { state_->dialog->blockSignals(true); state_->dialog->close(); }
    state_->dialog.clear();
    state_->qr.clear();
    state_->status.clear();
    state_->binding.clear();
    if (state_->ceremony) state_->actor->finish(state_->ceremony, state_->now(), state_->delivered);
    state_->ceremony = 0;
    state_->terminal = false;
    state_->terminalSuccess = false;
    state_->accountCount = 0;
    state_->phone = false;
    state_->submitting = false;
    state_->delivering = false;
    state_->delivered = false;
    if (helper) state_->process.cancel(helper);
}
