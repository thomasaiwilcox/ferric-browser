#include "quick_credential_integration.h"
#include "quick_credential_binding.h"
#include "phone_credential_actor.h"
#include "credential_boundary.h"
#include "request_interceptor.h"
#include <limits>
#include "ferric-browser-engine-qt/src/bridge.cxxqt.h"
#include <QJSValue>
#include <QMetaProperty>
#include <QMetaMethod>
#include <QThread>
#include <QTimer>
#include <utility>

namespace {
QObject *objectProperty(QObject *object, const char *name) {
    if (!object) return nullptr;
    const auto value = object->property(name);
    if (auto *result = value.value<QObject *>()) return result;
    return value.value<QJSValue>().toQObject();
}
QMetaObject::Connection watchProperty(QObject *source, const char *name, QObject *receiver) {
    const int index = source->metaObject()->indexOfProperty(name);
    if (index < 0) return {};
    const auto property = source->metaObject()->property(index);
    const int slot = receiver->metaObject()->indexOfSlot("reconcile()");
    return property.hasNotifySignal() && slot >= 0
        ? QObject::connect(source, property.notifySignal(), receiver, receiver->metaObject()->method(slot))
        : QMetaObject::Connection();
}
}

FerricQuickCredentialIntegration::~FerricQuickCredentialIntegration() { clear(); }

void FerricQuickCredentialIntegration::clear() {
    const auto entries = std::exchange(entries_, {});
    for (const auto &entry : entries) {
        for (const auto &watch : entry.watches) disconnect(watch);
        for (const auto &watch : entry.uiWatches) disconnect(watch);
        delete entry.binding.data();
    }
}

void FerricQuickCredentialIntegration::setLifetime(FerricProfileLifetime *lifetime) {
    if (lifetime_ == lifetime) return;
    const QPointer<FerricQuickCredentialIntegration> self(this);
    clear();
    if (!self) return;
    for (const auto &watch : std::as_const(lifetimeWatches_)) disconnect(watch);
    lifetimeWatches_.clear();
    lifetime_.clear();
    profileSession_.clear();
    if (lifetime && lifetime->thread() == thread()) {
        lifetime_ = lifetime;
        lifetimeWatches_.append(connect(lifetime, &FerricProfileLifetime::consumersChanged,
                                       this, &FerricQuickCredentialIntegration::reconcile));
        lifetimeWatches_.append(connect(lifetime, &FerricProfileLifetime::viewRetiring,
                                       this, &FerricQuickCredentialIntegration::reconcile));
        lifetimeWatches_.append(connect(lifetime, &FerricProfileLifetime::credentialTransportChanged,
                                       this, &FerricQuickCredentialIntegration::policyChanged));
        lifetimeWatches_.append(connect(lifetime, &QObject::destroyed, this, [self] {
            if (!self) return;
            self->clear();
            if (self) { self->lifetime_.clear(); self->profileSession_.clear(); }
        }));
    }
    emit lifetimeChanged();
}

void FerricQuickCredentialIntegration::setRequestInterceptor(QObject *interceptor) {
    if (interceptor_ == interceptor) return;
    for (const auto &watch : std::as_const(interceptorWatches_)) disconnect(watch);
    interceptorWatches_.clear();
    interceptor_.clear();
    if (interceptorIncarnation_ != 0)
        interceptorIncarnation_ = interceptorIncarnation_ == std::numeric_limits<quint64>::max() ? 0 : interceptorIncarnation_ + 1;
    if (auto *native = qobject_cast<FerricBrowserRequestInterceptor *>(interceptor);
            native && native->thread() == thread()) {
        interceptor_ = native;
        interceptorWatches_.append(connect(native, &FerricBrowserRequestInterceptor::policyChanged,
                                           this, &FerricQuickCredentialIntegration::policyChanged));
        interceptorWatches_.append(connect(native, &QObject::destroyed, this, [this] {
            interceptor_.clear(); policyChanged();
        }));
    }
    const QPointer<FerricQuickCredentialIntegration> self(this);
    policyChanged();
    if (self) emit self->requestInterceptorChanged();
}

QString FerricQuickCredentialIntegration::profilePolicyIdentity(QObject *profile) const {
    const auto *native = qobject_cast<FerricBrowserRequestInterceptor *>(interceptor_.data());
    if (!native || !interceptorIncarnation_ || !native->attachedToProfile(profile)) return {};
    const auto generation = native->policyGeneration();
    return generation ? QString::number(interceptorIncarnation_) + QLatin1Char(':') + QString::number(generation) : QString();
}

void FerricQuickCredentialIntegration::policyChanged() {
    const QPointer<FerricQuickCredentialIntegration> self(this);
    const auto entries = entries_.values();
    for (const auto &entry : entries) {
        if (!self) return;
        if (entry.binding) entry.binding->applicationChanged();
    }
    if (self) self->reconcile();
}

bool FerricQuickCredentialIntegration::associationCurrent(QQuickItem *view,
    const FerricProfileLifetime::CurrentView &owner, const QString &tab) const {
    if (!lifetime_ || !view || QThread::currentThread() != thread()
        || !lifetime_->matchesCurrentView(view, owner) || !owner.ui || !owner.host
        || objectProperty(view, "viewUi") != owner.ui || objectProperty(view, "viewHost") != owner.host
        || view->property("stableTabId").toString() != tab || view->property("viewRetired").toBool()
        || profilePolicyIdentity(owner.profile).isEmpty()) return false;
    const auto *ui = qobject_cast<BrowserUi *>(owner.ui.data());
    return ui && !profileSession_.isEmpty() && ui->page_host_profile_id() == profileSession_
        && ferric::credentials::credential_view_association_current(*ui, tab.toStdString(),
            view->property("tabIndex").toInt(), view->property("promptLeaseIdentity").toString().toStdString());
}

bool FerricQuickCredentialIntegration::attachView(QQuickItem *view) {
    if (!view || !lifetime_ || QThread::currentThread() != thread() || view->thread() != thread()) return false;
    const QPointer<FerricQuickCredentialIntegration> alive(this);
    const QPointer<QQuickItem> liveView(view);
    const auto current = lifetime_->currentView(view);
    if (!current) return false;
    auto *ui = qobject_cast<BrowserUi *>(current->ui.data());
    auto *policy = qobject_cast<BrowserUi *>(lifetime_->policyUi());
    if (!ui || !policy) return false;
    const auto session = policy->page_host_profile_id();
    if (profileSession_.isEmpty() && !session.isEmpty()) profileSession_ = session;
    if (session != profileSession_ || ui->page_host_profile_id() != profileSession_) return false;
    auto found = entries_.find(view);
    const auto tab = view->property("stableTabId").toString();
    if (found == entries_.end()) {
        Entry entry;
        entry.view = view;
        entry.watches.append(connect(view, &QObject::destroyed, this, [this, view] {
            auto found = entries_.find(view);
            if (found == entries_.end()) return;
            for (const auto &watch : found->watches) disconnect(watch);
            for (const auto &watch : found->uiWatches) disconnect(watch);
            // The view owns its binding and is already destroying children.
            entries_.erase(found);
        }));
        const auto presentationChanged = [this] {
            reconcile();
            // Native owner observers must also finish updating their actual
            // window before a replacement association can be constructed.
            QTimer::singleShot(0, this, &FerricQuickCredentialIntegration::reconcile);
        };
        entry.watches.append(connect(view, &QQuickItem::windowChanged, this, presentationChanged));
        entry.watches.append(connect(view, &QQuickItem::parentChanged, this, presentationChanged));
        for (const auto *property : {"stableTabId", "tabIndex", "viewUi", "viewHost", "profile", "viewRetired"})
            entry.watches.append(watchProperty(view, property, this));
        found = entries_.insert(view, entry);
    }
    if (found->watchedUi != ui) {
        for (const auto &watch : found->uiWatches) disconnect(watch);
        found->uiWatches.clear();
        found->watchedUi = ui;
        for (const auto *property : {"active_tab_index", "profile_bootstrap_pending", "feature_phone_passkeys_requested"})
            found->uiWatches.append(watchProperty(ui, property, this));
    }
    if (found->binding && (!associationCurrent(view, found->owner, found->tab)
        || !found->binding->isReady())) {
        auto *retired = found->binding.data();
        found->binding.clear();
        delete retired;
        if (!alive || !liveView || !lifetime_ || !current->ui || !lifetime_->matchesCurrentView(view, *current)) return false;
        found = entries_.find(view);
        if (found == entries_.end()) return false;
    }
    found->owner = *current;
    found->tab = tab;
    if (!associationCurrent(view, *current, tab)) return false;
    if (!found->binding) {
        const QPointer<FerricQuickCredentialIntegration> self(this);
        const QPointer<QQuickItem> item(view);
        found->binding = FerricQuickCredentialBinding::create(view, ui, tab,
            [self, item, expected = *current, tab] {
                return self && item && self->associationCurrent(item, expected, tab);
            }, [self, item, profile = current->profile] {
                if (!self || !profile || !item || !self->lifetime_) return QString();
                const auto policy = self->profilePolicyIdentity(profile);
                const auto tls = self->lifetime_->credentialTlsIdentity(item);
                return policy.isEmpty() || tls.isEmpty() ? QString() : policy + QLatin1Char('/') + tls;
            }, [self, item, profile = current->profile](const QUrl &origin) {
                const auto *interceptor = self ? qobject_cast<FerricBrowserRequestInterceptor *>(self->interceptor_.data()) : nullptr;
                return interceptor && profile && item && self->lifetime_ && interceptor->attachedToProfile(profile)
                    && self->lifetime_->credentialTlsEligible(item, origin)
                    && interceptor->phoneTransportAllowed(origin);
            });
        if (found->binding) {
            FerricPhoneCredentialActor::attach(found->binding);
            if (!found->binding->boundary()->enablePhoneShim()) {
                delete found->binding.data();
                found->binding.clear();
            }
        }
    }
    return found->binding && found->binding->isReady();
}

void FerricQuickCredentialIntegration::reconcile() {
    if (reconciling_) return;
    const QPointer<FerricQuickCredentialIntegration> self(this);
    reconciling_ = true;
    const auto views = entries_.keys();
    for (auto *view : views) {
        if (!self) return;
        auto found = entries_.find(view);
        if (found == entries_.end() || !found->view) continue;
        const QPointer<QQuickItem> item(found->view);
        if (found->binding && !associationCurrent(view, found->owner, found->tab)) {
            auto *retired = found->binding.data();
            found->binding.clear();
            delete retired;
        }
        if (self && item) self->attachView(item);
    }
    if (self) self->reconciling_ = false;
}
