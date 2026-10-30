#include "quick_credential_binding.h"
#include "quick_credential_owner.h"
#include "credential_boundary.h"
#include "ferric-browser-engine-qt/src/bridge.cxxqt.h"

#include <QGuiApplication>
#include <QQuickItem>
#include <QThread>
#include <QWindow>
#include <optional>
#include <utility>

namespace {
using ferric::credentials::NativeCredentialFacts;
NativeCredentialFacts facts(const FerricCredentialBoundary::CapturedRequest &request,
                            QQuickItem *view, QWindow *window)
{
    NativeCredentialFacts result;
    result.origin = request.origin.toStdString();
    result.profile_generation = request.owner.profileGeneration;
    result.page_generation = request.owner.pageGeneration;
    result.document_generation = request.owner.documentGeneration;
    result.host_generation = request.owner.hostGeneration;
    result.presentation_generation = request.owner.presentationGeneration;
    result.foreground = view && view->isVisible() && window && window->isVisible()
        && QGuiApplication::focusWindow() == window;
    return result;
}
}

struct FerricQuickCredentialBinding::State {
    std::optional<rust::Box<ferric::credentials::CredentialLeaseCapture>> lease;
    std::optional<NativeCredentialFacts> admittedFacts;
};

FerricQuickCredentialBinding *FerricQuickCredentialBinding::create(
    QQuickItem *view, QObject *ui, const QString &tabId, std::function<bool()> associationCurrent, std::function<QString()> profilePolicyIdentity)
{
    if (!view || !ui || QThread::currentThread() != view->thread()
        || ui->thread() != view->thread() || !qobject_cast<BrowserUi *>(ui)
        || tabId.isEmpty()) return nullptr;
    // Do not allow two adapters to assign different Core identities to a view.
    if (view->findChild<FerricQuickCredentialBinding *>(QString(), Qt::FindDirectChildrenOnly))
        return nullptr;
    auto *binding = new FerricQuickCredentialBinding(view, ui, tabId, std::move(associationCurrent), std::move(profilePolicyIdentity));
    if (binding->isReady()) return binding;
    delete binding;
    return nullptr;
}

FerricQuickCredentialBinding::FerricQuickCredentialBinding(QQuickItem *view,
    QObject *ui, const QString &tabId, std::function<bool()> associationCurrent, std::function<QString()> profilePolicyIdentity)
    : QObject(view), state_(std::make_unique<State>()), view_(view), ui_(ui), tabId_(tabId),
      associationCurrent_(std::move(associationCurrent)), profilePolicyIdentity_(std::move(profilePolicyIdentity))
{
    owner_ = FerricQuickCredentialOwner::create(view);
    if (!owner_) return;
    const QPointer<FerricQuickCredentialBinding> self(this);
    auto context = [self]() -> QString {
        if (!self || !self->ui_ || (self->associationCurrent_ && !self->associationCurrent_())) return {};
        auto *ui = qobject_cast<BrowserUi *>(self->ui_.data());
        if (!ui) return {};
        const auto core = QString::fromStdString(std::string(ferric::credentials::credential_context_identity(
                        *ui, self->tabId_.toStdString())));
        if (core.isEmpty() || !self->profilePolicyIdentity_) return core;
        const auto policy = self->profilePolicyIdentity_();
        return policy.isEmpty() ? QString() : core + QLatin1Char('/') + policy;
    };
    if (context().isEmpty()) return;
    boundary_ = owner_->prepareBoundary(context);
    if (!boundary_) return;
    connect(boundary_, &FerricCredentialBoundary::requestCaptured, this, &FerricQuickCredentialBinding::capture);
    connect(boundary_, &FerricCredentialBoundary::cancelled, this, &FerricQuickCredentialBinding::clearLease);
    connect(boundary_, &FerricCredentialBoundary::responseCompleted, this, &FerricQuickCredentialBinding::clearLease);
    connect(ui, &QObject::destroyed, this, [this] {
        ui_.clear();
        if (boundary_) boundary_->invalidate();
        clearLease();
    });
    // Direct invalidation after application ownership/selection changes. The
    // full context and lease are also revalidated at every use.
    connect(ui, SIGNAL(active_tab_indexChanged()), this, SLOT(applicationChanged()));
    connect(ui, SIGNAL(tab_countChanged()), this, SLOT(applicationChanged()));
    connect(ui, SIGNAL(context_nameChanged()), this, SLOT(applicationChanged()));
    connect(ui, SIGNAL(config_jsonChanged()), this, SLOT(applicationChanged()));
    connect(ui, SIGNAL(blocking_policy_readyChanged()), this, SLOT(applicationChanged()));
    connect(ui, SIGNAL(blocking_bypass_sitesChanged()), this, SLOT(applicationChanged()));
    connect(ui, SIGNAL(runtime_work_available()), this, SLOT(applicationChanged()));
}

FerricQuickCredentialBinding::~FerricQuickCredentialBinding()
{
    if (boundary_) boundary_->invalidate();
}

bool FerricQuickCredentialBinding::isReady() const
{
    return ui_ && view_ && (!associationCurrent_ || associationCurrent_())
        && owner_ && owner_->isReady() && boundary_ && boundary_->isReady();
}

FerricCredentialBoundary *FerricQuickCredentialBinding::boundary() const { return boundary_.data(); }

bool FerricQuickCredentialBinding::hasCurrentLease() const
{
    if (!isReady() || !state_->lease || !boundary_->request()) return false;
    auto *ui = qobject_cast<BrowserUi *>(ui_.data());
    // Foreground is the verified admission fact. An owned chooser/QR window
    // takes input focus during the ceremony; that does not alter the captured
    // document. Native selection independently requires exact dialog focus.
    return ui && state_->admittedFacts && (*state_->lease)->is_current(
        *ui, tabId_.toStdString(), *state_->admittedFacts);
}

void FerricQuickCredentialBinding::capture()
{
    clearLease();
    if (!isReady() || !boundary_->request()) return;
    auto *ui = qobject_cast<BrowserUi *>(ui_.data());
    if (!ui) return;
    state_->admittedFacts.emplace(facts(*boundary_->request(), view_, owner_->presentationWindow()));
    state_->lease.emplace(ferric::credentials::capture_native_credential_lease(
        *ui, tabId_.toStdString(), *state_->admittedFacts));
    if (!(*state_->lease)->is_valid()) {
        state_->lease.reset();
        state_->admittedFacts.reset();
        boundary_->invalidate();
        return;
    }
    emit requestReady();
}

void FerricQuickCredentialBinding::applicationChanged()
{
    if (state_->lease && !hasCurrentLease()) {
        boundary_->invalidate();
        clearLease();
    }
}

void FerricQuickCredentialBinding::clearLease()
{
    if (!state_->lease) return;
    state_->lease.reset();
    state_->admittedFacts.reset();
    emit leaseInvalidated();
}
