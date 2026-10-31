#include "profile_lifetime.h"
#include "profile_tls.h"

#include <QMetaProperty>
#include <QQmlContext>
#include <QThread>
#include <QTimer>
#include <QVariant>
#include <QQuickWindow>
#include <QJSValue>
#include <limits>

#include <algorithm>
#include <utility>

#if QT_VERSION != QT_VERSION_CHECK(6, 11, 2)
#error "Popup opener lifetime selector is qualified only for Qt 6.11.2"
#endif

namespace {
constexpr auto kDelegateClassName =
    "QtWebEngineCore::RenderWidgetHostViewQtDelegateItem";
constexpr int kDiscardedLifecycleState = 2;
QObject *objectProperty(QObject *object, const char *name) {
    if (!object) return nullptr;
    const auto value = object->property(name);
    if (auto *result = value.value<QObject *>()) return result;
    return value.value<QJSValue>().toQObject();
}
}

FerricProfileLifetime::~FerricProfileLifetime() {
    shuttingDown_ = true;
    for (const auto &window : std::as_const(windows_)) {
        disconnect(window.hostDestroyed);
        disconnect(window.uiDestroyed);
    }
    for (auto it = popupLeases_.begin(); it != popupLeases_.end(); ++it)
        removePopupLease(it.value());
    popupLeases_.clear();

    QList<QPair<quint64, QPointer<QQuickItem>>> orderedViews;
    orderedViews.reserve(views_.size());
    for (auto it = views_.begin(); it != views_.end(); ++it) {
        auto &entry = it.value();
        if (!entry.item)
            continue;
        disconnect(entry.item, nullptr, this, nullptr);
        disconnect(entry.lifecycleChanged);
        disconnect(entry.renderProcessPidChanged);
        orderedViews.append({entry.incarnation, entry.item});
    }
    views_.clear();
    std::sort(orderedViews.begin(), orderedViews.end(),
              [](const auto &left, const auto &right) {
                  return left.first > right.first;
              });
    // Popup targets are adopted after their opener. Reverse incarnation order
    // destroys every child WebContents/delegate before its retained opener.
    for (const auto &entry : orderedViews) {
        if (!entry.second)
            continue;
        entry.second->setVisible(false);
        entry.second->setEnabled(false);
        entry.second->setParentItem(nullptr);
        if (entry.second->metaObject()->indexOfProperty("lifecycleState") >= 0)
            entry.second->setProperty("lifecycleState",
                                      kDiscardedLifecycleState);
        delete entry.second.data();
    }
}

bool FerricProfileLifetime::ownResource(QObject *resource) {
    if (drained_ || shuttingDown_ || !resource ||
        resource->thread() != thread())
        return false;
    for (QObject *ancestor = this; ancestor; ancestor = ancestor->parent())
        if (ancestor == resource)
            return false;
    resource->setParent(this);
    return true;
}

bool FerricProfileLifetime::ownProfile(QObject *factory, QObject *profile) {
    if (!factory || !profile || factory == profile || profile == this ||
        profile->thread() != thread())
        return false;
    if (ownedProfile_ && (ownedProfile_ != profile || profile->parent() != factory))
        return false;
    for (QObject *ancestor = factory; ancestor; ancestor = ancestor->parent())
        if (ancestor == profile)
            return false;
    if (!ownResource(factory))
        return false;
    // WebEngineProfilePrototype owns its instance through a scoped pointer.
    // Keep that ownership intact rather than deleting the instance separately.
    profile->setParent(factory);
    ownedProfile_ = profile;
    auto *tls = FerricProfileTls::establish(profile);
    if (tls_ != tls) {
        if (tls_) disconnect(tls_, nullptr, this, nullptr);
        tls_ = tls;
        if (tls_) connect(tls_, &FerricProfileTls::changed, this, &FerricProfileLifetime::credentialTransportChanged);
    }
    const QPointer<FerricProfileLifetime> self(this);
    const auto views = views_.values();
    for (const auto &view : views) {
        if (!self) return false;
        if (self->tls_ && view.item) self->tls_->observeView(view.item);
    }
    return !self.isNull();
}

QString FerricProfileLifetime::credentialTlsIdentity(QQuickItem *view) const {
    return tls_ && currentView(view) ? tls_->identity(view) : QString();
}

bool FerricProfileLifetime::credentialTlsEligible(QQuickItem *view, const QUrl &origin) const {
    return tls_ && currentView(view) && tls_->eligible(view, origin);
}

std::optional<FerricProfileLifetime::CurrentView>
FerricProfileLifetime::currentView(QQuickItem *view) const {
    const QPointer<const FerricProfileLifetime> self(this);
    const QPointer<QQuickItem> guarded(view);
    QObject *profile = nullptr;
    if (!isCurrentView(view, &profile, nullptr) || !self || !guarded || !ownedProfile_
        || profile != ownedProfile_ || !ownedProfile_->parent()
        || ownedProfile_->parent()->parent() != this) return std::nullopt;
    const auto entry = views_.value(view);
    if (!entry.item || entry.retiring || viewTransfers_.contains(view)) return std::nullopt;
    return CurrentView{entry.host, entry.ui, profile, entry.incarnation, entry.windowGeneration};
}

bool FerricProfileLifetime::matchesCurrentView(QQuickItem *view,
                                               const CurrentView &expected) const {
    const auto current = currentView(view);
    return current && current->host == expected.host && current->ui == expected.ui
        && current->profile == expected.profile && current->incarnation == expected.incarnation
        && current->windowGeneration == expected.windowGeneration;
}

bool FerricProfileLifetime::retainWindow(QObject *host, QObject *ui) {
    if (drained_ || shuttingDown_ || !host || !ui ||
        host->thread() != thread() || ui->thread() != thread())
        return false;
    for (const auto &window : std::as_const(windows_))
        if (window.key == host)
            return window.host && window.ui == ui;
    Window entry{host, host, ui, {}, {}, false, ++nextGeneration_};
    entry.hostDestroyed = connect(host, &QObject::destroyed, this,
                                  [this, host] { releaseWindow(host); });
    entry.uiDestroyed = connect(ui, &QObject::destroyed, this,
                                [this, host] { releaseWindow(host); });
    windows_.append(entry);
    consumed_ = true;
    refresh();
    return true;
}

bool FerricProfileLifetime::activateWindow(QObject *host) {
    if (drained_ || shuttingDown_)
        return false;
    for (auto &entry : windows_) {
        if (entry.key != host || !entry.host || !entry.ui)
            continue;
        entry.ready = true;
        refresh();
        return true;
    }
    return false;
}

bool FerricProfileLifetime::adoptView(QQuickItem *view, QObject *host,
                                      QObject *ui) {
    if (drained_ || shuttingDown_ || !view || view->thread() != thread()
        || viewTransfers_.contains(view))
        return false;
    auto owner = std::find_if(
        windows_.begin(), windows_.end(), [host, ui](const Window &entry) {
            return entry.host == host && entry.ui == ui && entry.ready;
        });
    if (owner == windows_.end())
        return false;
    auto current = views_.find(view);
    if (current != views_.end() && current->retiring)
        return false;
    if (current != views_.end()) {
        const bool sameOwner = current->hostKey == host && current->ui == ui &&
                               current->windowGeneration == owner->generation;
        if (sameOwner) {
            if (tls_) tls_->observeView(view);
            return true;
        }
        for (const auto &lease : std::as_const(popupLeases_))
            if (lease.source == view || lease.target == view)
                return false;
    }
    // QObject ownership and visual parenting are independent in Qt Quick.
    for (QObject *ancestor = this; ancestor; ancestor = ancestor->parent())
        if (ancestor == view)
            return false;
    View entry;
    entry.item = view;
    entry.hostKey = host;
    entry.host = host;
    entry.ui = ui;
    entry.incarnation = ++nextGeneration_;
    entry.windowGeneration = owner->generation;
    if (current == views_.end()) {
        connect(view, &QObject::destroyed, this, [this, view] {
            const auto found = views_.find(view);
            if (found == views_.end())
                return;
            const quint64 incarnation = found->incarnation;
            views_.erase(found);
            const auto leases = popupLeases_.keys();
            for (const QString &id : leases) {
                auto lease = popupLeases_.find(id);
                if (lease == popupLeases_.end())
                    continue;
                const quint64 leaseGeneration = lease->generation;
                if (lease->sourceKey == view &&
                    lease->sourceIncarnation == incarnation)
                    lease->sourceDestroyed = true;
                if (lease->targetKey == view &&
                    lease->targetIncarnation == incarnation) {
                    lease->targetRetiring = true;
                    lease->targetDestroyed = true;
                }
                if (!lease->adoptionStarted &&
                    (lease->sourceDestroyed || lease->targetDestroyed)) {
                    cancelPopupOpenerLease(id);
                    continue;
                }
                if (lease->targetDestroyed && lease->adoptionStarted &&
                    !lease->sawDelegate) {
                    // A started openIn can be retired before Quick attaches a
                    // native item. Settle after QObject destruction finishes.
                    QTimer::singleShot(0, this, [this, id, leaseGeneration] {
                        maybeReleasePopupLease(id, leaseGeneration);
                    });
                    continue;
                }
                maybeReleasePopupLease(id, leaseGeneration);
            }
            refresh();
        });
    }
    view->setParent(this);
    views_.insert(view, entry);
    const QPointer<FerricProfileLifetime> self(this);
    const QPointer<QQuickItem> observed(view);
    refresh();
    if (self && observed && self->tls_) self->tls_->observeView(observed);
    return self && observed;
}

bool FerricProfileLifetime::isCurrentView(QQuickItem *view,
                                          QObject **profile,
                                          QQmlEngine **engine) const {
    const QPointer<const FerricProfileLifetime> self(this);
    const QPointer<QQuickItem> guarded(view);
    if (!view || view->thread() != thread() || QThread::currentThread() != thread())
        return false;
    const auto it = views_.constFind(view);
    if (it == views_.cend() || viewTransfers_.contains(view) || it->retiring || !it->item || !it->host ||
        !it->ui)
        return false;
    auto *hostWindow = qobject_cast<QQuickWindow *>(it->host.data());
    if (!hostWindow || view->window() != hostWindow)
        return false;
    const auto owner = std::find_if(
        windows_.cbegin(), windows_.cend(), [&it](const Window &window) {
            return window.key == it->hostKey && window.host == it->host &&
                   window.ui == it->ui && window.ready &&
                   window.generation == it->windowGeneration;
        });
    if (owner == windows_.cend())
        return false;
    const CurrentView expected{it->host, it->ui, {}, it->incarnation, it->windowGeneration};
    const QVariant profileValue = view->property("profile");
    if (!self || !guarded) return false;
    QObject *viewProfile = profileValue.value<QObject *>();
    QQmlContext *context = QQmlEngine::contextForObject(view);
    QQmlEngine *viewEngine = context ? context->engine() : nullptr;
    if (!viewProfile || !viewEngine || viewEngine->thread() != thread())
        return false;
    const auto current = views_.constFind(view);
    if (current == views_.cend() || current->retiring || viewTransfers_.contains(view)
        || current->host != expected.host || current->ui != expected.ui
        || current->incarnation != expected.incarnation || current->windowGeneration != expected.windowGeneration
        || guarded->window() != qobject_cast<QQuickWindow *>(expected.host.data()))
        return false;
    if (profile)
        *profile = viewProfile;
    if (engine)
        *engine = viewEngine;
    return true;
}

QString FerricProfileLifetime::acquirePopupOpenerLease(QQuickItem *source,
                                                       QQuickItem *target) {
    if (QThread::currentThread() != thread())
        return {};
    if (shuttingDown_ || drained_ || !source || !target || source == target)
        return {};
    QObject *sourceProfile = nullptr;
    QObject *targetProfile = nullptr;
    QQmlEngine *sourceEngine = nullptr;
    QQmlEngine *targetEngine = nullptr;
    if (!isCurrentView(source, &sourceProfile, &sourceEngine) ||
        !isCurrentView(target, &targetProfile, &targetEngine) ||
        sourceProfile != targetProfile || sourceEngine != targetEngine)
        return {};
    const auto sourceEntry = views_.constFind(source);
    const auto targetEntry = views_.constFind(target);
    if (sourceEntry == views_.cend() || targetEntry == views_.cend() ||
        targetEntry->incarnation <= sourceEntry->incarnation)
        return {};
    if (QString::fromLatin1(QT_VERSION_STR) != QStringLiteral("6.11.2"))
        return {};
    const QString id = QStringLiteral("popup-opener-%1")
                           .arg(++nextPopupLeaseId_);
    PopupLease lease;
    lease.id = id;
    lease.generation = ++nextGeneration_;
    lease.sourceKey = source;
    lease.targetKey = target;
    lease.source = source;
    lease.target = target;
    lease.profile = sourceProfile;
    lease.engine = sourceEngine;
    lease.sourceIncarnation = sourceEntry->incarnation;
    lease.targetIncarnation = targetEntry->incarnation;
    popupLeases_.insert(id, lease);
    reconcilePopupLease(id, lease.generation);
    return id;
}

bool FerricProfileLifetime::beginPopupOpenerLease(const QString &leaseId) {
    if (QThread::currentThread() != thread() || shuttingDown_ || drained_)
        return false;
    auto it = popupLeases_.find(leaseId);
    if (it == popupLeases_.end())
        return false;
    if (it->adoptionStarted)
        return true;
    if (it->targetRetiring || it->targetDestroyed ||
        !popupLeaseIsCurrent(it.value()))
        return false;
    it->adoptionStarted = true;
    return true;
}

bool FerricProfileLifetime::confirmPopupOpenerLease(const QString &leaseId) {
    if (QThread::currentThread() != thread() || shuttingDown_ || drained_)
        return false;
    auto it = popupLeases_.find(leaseId);
    if (it == popupLeases_.end() || !it->adoptionStarted)
        return false;
    if (it->adoptionConfirmed)
        return true;
    if (it->targetRetiring || it->targetDestroyed ||
        !popupLeaseIsCurrent(it.value()))
        return false;
    it->adoptionConfirmed = true;
    reconcilePopupLease(leaseId, it->generation);
    return true;
}

bool FerricProfileLifetime::cancelPopupOpenerLease(const QString &leaseId) {
    if (QThread::currentThread() != thread())
        return false;
    auto it = popupLeases_.find(leaseId);
    if (it == popupLeases_.end() || it->adoptionStarted)
        return false;
    PopupLease cancelled = it.value();
    removePopupLease(it.value());
    popupLeases_.erase(it);
    emit popupOpenerLeaseReleased(leaseId);
    maybeDeleteRetiredView(cancelled.source.data());
    refresh();
    return true;
}

int FerricProfileLifetime::popupOpenerLeaseDelegateCount(
    const QString &leaseId) const {
    const auto it = popupLeases_.constFind(leaseId);
    return it == popupLeases_.cend() ? 0 : it->delegates.size();
}

QObject *FerricProfileLifetime::ownerUi(QQuickItem *view) const {
    if (viewTransfers_.contains(view)) return nullptr;
    const auto it = views_.constFind(view);
    if (it == views_.cend() || it->retiring || !it->item || !it->host ||
        !it->ui)
        return nullptr;
    for (const auto &window : windows_)
        if (window.key == it->hostKey && window.host && window.ui == it->ui &&
            window.ready && window.generation == it->windowGeneration)
            return it->ui;
    return nullptr;
}

QObject *FerricProfileLifetime::ownerHost(QQuickItem *view) const {
    return ownerUi(view) ? views_.value(view).host.data() : nullptr;
}

void FerricProfileLifetime::releaseWindow(QObject *host) {
    bool found = false;
    for (auto it = windows_.begin(); it != windows_.end();) {
        if (it->key != host) {
            ++it;
            continue;
        }
        disconnect(it->hostDestroyed);
        disconnect(it->uiDestroyed);
        it = windows_.erase(it);
        found = true;
    }
    if (!found)
        return;
    const auto retiring = views_.values();
    for (const auto &entry : retiring)
        if (entry.hostKey == host && entry.item)
            releaseView(entry.item);
    refresh();
}

int FerricProfileLifetime::outgoingPopupLeaseCount(QQuickItem *view) const {
    int count = 0;
    for (const auto &lease : popupLeases_)
        if (lease.source == view)
            ++count;
    return count;
}

bool FerricProfileLifetime::isPopupDelegate(QObject *object) const {
    if (!object || QString::fromLatin1(QT_VERSION_STR) !=
                       QStringLiteral("6.11.2"))
        return false;
    const QByteArray expected(kDelegateClassName);
    return QByteArray(object->metaObject()->className()) == expected &&
           object->inherits(expected.constData());
}

void FerricProfileLifetime::reconcilePopupLease(const QString &leaseId,
                                                quint64 generation) {
    auto it = popupLeases_.find(leaseId);
    if (it == popupLeases_.end() || it->generation != generation ||
        !popupLeaseIsCurrent(it.value()))
        return;
    const auto targetEntry = views_.constFind(it->target.data());
    if (targetEntry == views_.cend() ||
        targetEntry->incarnation != it->targetIncarnation)
        return;

    QSet<QQuickItem *> currentItems;
    QList<QQuickItem *> pending{it->target.data()};
    while (!pending.isEmpty()) {
        QQuickItem *item = pending.takeLast();
        if (!item || currentItems.contains(item))
            continue;
        currentItems.insert(item);
        if (isPopupDelegate(item)) {
            if (!it->delegates.contains(item)) {
                it->sawDelegate = true;
                DelegateWatch watch;
                watch.item = item;
                watch.destroyed = connect(
                    item, &QObject::destroyed, this,
                    [this, leaseId, generation, targetIncarnation =
                                                   it->targetIncarnation,
                     item] {
                        onPopupDelegateDestroyed(leaseId, generation,
                                                 targetIncarnation, item);
                    });
                it->delegates.insert(item, watch);
            }
        }
        if (!it->childWatches.contains(item)) {
            ChildWatch watch;
            watch.item = item;
            watch.childrenChanged = connect(
                item, &QQuickItem::childrenChanged, this,
                [this, leaseId, generation] {
                    reconcilePopupLease(leaseId, generation);
                });
            it->childWatches.insert(item, watch);
        }
        pending.append(item->childItems());
    }
    for (auto watch = it->childWatches.begin();
         watch != it->childWatches.end();) {
        if (currentItems.contains(watch.key())) {
            ++watch;
            continue;
        }
        disconnect(watch->childrenChanged);
        watch = it->childWatches.erase(watch);
    }
    maybeReleasePopupLease(leaseId, generation);
}

void FerricProfileLifetime::onPopupDelegateDestroyed(
    const QString &leaseId, quint64 generation, quint64 targetIncarnation,
    QObject *delegate) {
    auto it = popupLeases_.find(leaseId);
    if (it == popupLeases_.end() || it->generation != generation ||
        it->targetIncarnation != targetIncarnation)
        return;
    auto native = it->delegates.find(delegate);
    if (native == it->delegates.end())
        return;
    disconnect(native->destroyed);
    it->delegates.erase(native);
    maybeReleasePopupLease(leaseId, generation);
}

bool FerricProfileLifetime::isDiscardedAndRendererFree(
    QQuickItem *view) const {
    if (!view || view->metaObject()->indexOfProperty("lifecycleState") < 0 ||
        view->metaObject()->indexOfProperty("renderProcessPid") < 0)
        return false;
    bool pidOk = false;
    const qlonglong pid = view->property("renderProcessPid").toLongLong(&pidOk);
    return view->property("lifecycleState").toInt() ==
               kDiscardedLifecycleState &&
           pidOk && pid == 0;
}

void FerricProfileLifetime::maybeReleasePopupLease(const QString &leaseId,
                                                   quint64 generation) {
    auto it = popupLeases_.find(leaseId);
    if (it == popupLeases_.end() || it->generation != generation ||
        !it->targetRetiring || (!it->sawDelegate && !it->targetDestroyed) ||
        !it->delegates.isEmpty())
        return;
    QQuickItem *source = it->source.data();
    QQuickItem *target = it->target.data();
    const auto sourceEntry = views_.constFind(it->sourceKey);
    if (source) {
        QQmlContext *sourceContext = QQmlEngine::contextForObject(source);
        if (sourceEntry == views_.cend() ||
            sourceEntry->incarnation != it->sourceIncarnation ||
            source != it->sourceKey ||
            source->property("profile").value<QObject *>() != it->profile ||
            (!sourceEntry->retiring &&
             (!sourceContext || sourceContext->engine() != it->engine)) ||
            (sourceEntry->retiring && !isDiscardedAndRendererFree(source)))
            return;
    } else if (!it->sourceDestroyed) {
        return;
    }
    const auto targetEntry = views_.constFind(it->targetKey);
    if (target) {
        QQmlContext *targetContext = QQmlEngine::contextForObject(target);
        if (targetEntry == views_.cend() ||
            targetEntry->incarnation != it->targetIncarnation ||
            target != it->targetKey || !targetEntry->retiring ||
            target->property("profile").value<QObject *>() != it->profile ||
            (!targetEntry->retiring &&
             (!targetContext || targetContext->engine() != it->engine)))
            return;
    } else if (!it->targetDestroyed) {
        return;
    }
    PopupLease retired = it.value();
    removePopupLease(it.value());
    popupLeases_.erase(it);
    emit popupOpenerLeaseReleased(leaseId);
    maybeDeleteRetiredView(retired.source.data());
    refresh();
}

void FerricProfileLifetime::removePopupLease(PopupLease &lease) {
    for (auto it = lease.childWatches.begin();
         it != lease.childWatches.end(); ++it)
        disconnect(it->childrenChanged);
    for (auto it = lease.delegates.begin(); it != lease.delegates.end(); ++it)
        disconnect(it->destroyed);
    lease.childWatches.clear();
    lease.delegates.clear();
}

bool FerricProfileLifetime::popupLeaseIsCurrent(
    const PopupLease &lease) const {
    if (!lease.source || !lease.target || !lease.profile || !lease.engine)
        return false;
    const auto source = views_.constFind(lease.source.data());
    const auto target = views_.constFind(lease.target.data());
    if (source == views_.cend() || target == views_.cend() ||
        source->incarnation != lease.sourceIncarnation ||
        target->incarnation != lease.targetIncarnation)
        return false;
    const auto currentProfile = [](QQuickItem *view) -> QObject * {
        return view->property("profile").value<QObject *>();
    };
    const auto currentEngine = [](QQuickItem *view) -> QQmlEngine * {
        QQmlContext *context = QQmlEngine::contextForObject(view);
        return context ? context->engine() : nullptr;
    };
    const bool sourceRetiring = source->retiring;
    const bool targetRetiring = target->retiring;
    return currentProfile(lease.source.data()) == lease.profile &&
           currentProfile(lease.target.data()) == lease.profile &&
           (sourceRetiring || currentEngine(lease.source.data()) == lease.engine) &&
           (targetRetiring || currentEngine(lease.target.data()) == lease.engine);
}

void FerricProfileLifetime::watchRetiredViewState(QQuickItem *view) {
    auto it = views_.find(view);
    if (it == views_.end())
        return;
    const int slotIndex = metaObject()->indexOfSlot("onRetiredViewStateChanged()");
    if (slotIndex < 0)
        return;
    const QMetaMethod slot = metaObject()->method(slotIndex);
    const int lifecycleIndex = view->metaObject()->indexOfProperty("lifecycleState");
    const int pidIndex = view->metaObject()->indexOfProperty("renderProcessPid");
    if (lifecycleIndex >= 0) {
        const QMetaMethod signal = view->metaObject()->property(lifecycleIndex).notifySignal();
        if (signal.isValid())
            it->lifecycleChanged = QObject::connect(
                view, signal, this, slot, Qt::DirectConnection);
    }
    if (pidIndex >= 0) {
        const QMetaMethod signal = view->metaObject()->property(pidIndex).notifySignal();
        if (signal.isValid())
            it->renderProcessPidChanged = QObject::connect(
                view, signal, this, slot, Qt::DirectConnection);
    }
}

void FerricProfileLifetime::onRetiredViewStateChanged() {
    auto *view = qobject_cast<QQuickItem *>(sender());
    if (!view || shuttingDown_)
        return;
    const auto ids = popupLeases_.keys();
    for (const QString &id : ids) {
        const auto it = popupLeases_.constFind(id);
        if (it != popupLeases_.cend() &&
            (it->source == view || it->target == view))
            maybeReleasePopupLease(id, it->generation);
    }
    maybeDeleteRetiredView(view);
}

void FerricProfileLifetime::maybeDeleteRetiredView(QQuickItem *view) {
    auto it = views_.find(view);
    if (it == views_.end() || !it->retiring || it->deleteQueued ||
        outgoingPopupLeaseCount(view) != 0)
        return;
    if (it->requireDiscarded && !isDiscardedAndRendererFree(view))
        return;
    it->deleteQueued = true;
    view->deleteLater();
}

void FerricProfileLifetime::releaseView(QQuickItem *view) {
    if (auto transfer = viewTransfers_.value(view)) {
        transfer->invalid = true;
        transfer->retire = true;
        return;
    }
    auto it = views_.find(view);
    if (it == views_.end() || it->retiring)
        return;
    it->retiring = true;
    const QPointer<QQuickItem> guard = view;
    emit viewRetiring(view);
    if (!guard)
        return;
    guard->setVisible(false);
    guard->setEnabled(false);
    guard->setParentItem(nullptr);
    const bool hasOutgoingLeases = outgoingPopupLeaseCount(view) != 0;
    it = views_.find(view);
    if (it == views_.end())
        return;
    it->requireDiscarded = hasOutgoingLeases;
    if (hasOutgoingLeases) {
        watchRetiredViewState(view);
        if (view->metaObject()->indexOfProperty("lifecycleState") >= 0)
            view->setProperty("lifecycleState", kDiscardedLifecycleState);
    } else if (view->metaObject()->indexOfProperty("lifecycleState") >= 0) {
        view->setProperty("lifecycleState", kDiscardedLifecycleState);
    }
    const auto leaseIds = popupLeases_.keys();
    for (const QString &id : leaseIds) {
        auto lease = popupLeases_.find(id);
        if (lease == popupLeases_.end() || lease->target != view)
            continue;
        lease->targetRetiring = true;
        const quint64 generation = lease->generation;
        reconcilePopupLease(id, generation);
    }
    maybeDeleteRetiredView(view);
    refresh();
}

FerricProfileLifetime::ViewTransfer::~ViewTransfer() {
    // The state is pinned independently of this caller-owned handle. A callback
    // may delete the adapter/handle while a native transaction method runs.
    const auto pinned = state;
    if (pinned && pinned->lifetime && !pinned->ended)
        pinned->lifetime->cancelViewTransfer(pinned, false);
}

bool FerricProfileLifetime::transferWindowsCurrent(const ViewTransferState &transfer) const {
    if (transfer.ended || transfer.invalid || drained_ || shuttingDown_
        || !transfer.view || !transfer.source.host || !transfer.source.ui
        || !transfer.source.profile || transfer.source.profile != ownedProfile_
        || !transfer.destinationHost || !transfer.destinationUi
        || viewTransfers_.value(transfer.key).get() != &transfer) return false;
    auto has = [this](QObject *host, QObject *ui, quint64 generation) {
        return std::any_of(windows_.cbegin(), windows_.cend(), [&](const Window &entry) {
            return entry.host == host && entry.ui == ui && entry.ready
                && entry.generation == generation;
        });
    };
    return has(transfer.source.host, transfer.source.ui, transfer.source.windowGeneration)
        && has(transfer.destinationHost, transfer.destinationUi, transfer.destinationGeneration);
}

std::unique_ptr<FerricProfileLifetime::ViewTransfer>
FerricProfileLifetime::beginViewTransfer(QQuickItem *item, QObject *host, QObject *ui, const std::function<bool()> &sourceProof) {
    const QPointer<FerricProfileLifetime> self(this);
    const QPointer<QQuickItem> view(item);
    const QPointer<QObject> destinationHost(host), destinationUi(ui);
    auto alive = [&] { return self && view && destinationHost && destinationUi; };
    if (!alive() || QThread::currentThread() != thread()
        || host->thread() != thread() || ui->thread() != thread()
        || viewTransfers_.contains(item)) return {};
    const auto source = currentView(view);
    if (!alive() || !source || source->host == host || source->ui == ui) return {};
    const auto sourceHost = objectProperty(view, "viewHost");
    if (!alive() || sourceHost != source->host) return {};
    const auto sourceUi = objectProperty(view, "viewUi");
    if (!alive() || sourceUi != source->ui) return {};
    const auto loading = view->property("loading").toBool();
    if (!alive() || loading) return {};
    const auto retired = view->property("viewRetired").toBool();
    if (!alive() || retired) return {};
    // Recheck the raw registry after all callback-capable property evaluation.
    const auto entry = views_.constFind(item);
    if (entry == views_.cend() || entry->retiring || entry->incarnation != source->incarnation
        || entry->windowGeneration != source->windowGeneration || entry->host != source->host
        || entry->ui != source->ui || viewTransfers_.contains(item)
        || view->window() != qobject_cast<QQuickWindow *>(source->host.data())) return {};
    const auto destination = std::find_if(windows_.cbegin(), windows_.cend(), [&](const Window &owner) {
        return owner.host == host && owner.ui == ui && owner.ready;
    });
    if (destination == windows_.cend()) return {};
    for (const auto &lease : std::as_const(popupLeases_))
        if (lease.source == view || lease.target == view) return {};
    for (const char *name : {"url", "loading", "renderProcessPid", "lifecycleState", "profile"})
        if (view->metaObject()->indexOfProperty(name) < 0) return {};
    auto transfer = std::unique_ptr<ViewTransfer>(new ViewTransfer);
    const auto state = std::make_shared<ViewTransferState>();
    transfer->state = state;
    state->key = item;
    state->lifetime = this;
    state->view = view;
    state->source = *source;
    state->destinationHost = host;
    state->destinationUi = ui;
    state->destinationGeneration = destination->generation;
    const auto slot = metaObject()->method(metaObject()->indexOfSlot("invalidateViewTransfer()"));
    for (const char *name : {"url", "loading", "renderProcessPid", "lifecycleState", "profile"}) {
        const auto property = view->metaObject()->property(view->metaObject()->indexOfProperty(name));
        if (property.hasNotifySignal())
            state->watches.append(connect(view, property.notifySignal(), this, slot));
    }
    for (const char *name : {"stableTabId", "promptLeaseIdentity"}) {
        const int index = view->metaObject()->indexOfProperty(name);
        if (index >= 0) {
            const auto property = view->metaObject()->property(index);
            if (property.hasNotifySignal()) state->watches.append(connect(view, property.notifySignal(), this, slot));
        }
    }
    viewTransfers_.insert(item, state);
    emit credentialTransportChanged();
    if (!self) return {};
    if (!self->transferWindowsCurrent(*state)) {
        self->cancelViewTransfer(*transfer, true, sourceProof ? sourceProof : [] { return false; });
        return {};
    }
    return transfer;
}

void FerricProfileLifetime::invalidateViewTransfer() {
    auto *view = qobject_cast<QQuickItem *>(sender());
    if (auto transfer = viewTransfers_.value(view)) transfer->invalid = true;
}

bool FerricProfileLifetime::pinViewTransferIdentity(ViewTransfer &handle, const QString &tab, const QString &prompt, int index) {
    const auto transfer = handle.state;
    const QPointer<FerricProfileLifetime> self(this);
    if (!transfer || !transferWindowsCurrent(*transfer)) return false;
    const QPointer<QQuickItem> view = transfer->view;
    const auto slot = metaObject()->method(metaObject()->indexOfSlot("invalidateViewTransfer()"));
    const int propertyIndex = view->metaObject()->indexOfProperty("tabIndex");
    if (propertyIndex < 0) return false;
    const auto property = view->metaObject()->property(propertyIndex);
    if (!property.hasNotifySignal()) return false;
    transfer->watches.append(connect(view, property.notifySignal(), this, slot));
    auto current = [&] { return self && view && self->transferWindowsCurrent(*transfer); };
    const auto actualTab = view->property("stableTabId").toString();
    if (!current() || actualTab != tab) return false;
    const auto actualPrompt = view->property("promptLeaseIdentity").toString();
    if (!current() || actualPrompt != prompt) return false;
    const auto actualIndex = view->property("tabIndex").toInt();
    return current() && actualIndex == index;
}

bool FerricProfileLifetime::associateViewTransfer(ViewTransfer &handle) {
    const auto transfer = handle.state;
    const QPointer<FerricProfileLifetime> self(this);
    if (!transfer || QThread::currentThread() != thread() || transfer->associated
        || !transferWindowsCurrent(*transfer)) return false;
    const QPointer<QQuickItem> view = transfer->view;
    auto current = [&] { return self && view && self->transferWindowsCurrent(*transfer); };
    if (view->window() != qobject_cast<QQuickWindow *>(transfer->destinationHost.data())) return false;
    const auto host = objectProperty(view, "viewHost");
    if (!current() || host != transfer->destinationHost) return false;
    const auto ui = objectProperty(view, "viewUi");
    if (!current() || ui != transfer->destinationUi) return false;
    const auto profile = objectProperty(view, "profile");
    if (!current() || profile != transfer->source.profile) return false;
    const auto loading = view->property("loading").toBool();
    if (!current() || loading) return false;
    const auto retired = view->property("viewRetired").toBool();
    if (!current() || retired || nextGeneration_ == std::numeric_limits<quint64>::max()
        || view->window() != qobject_cast<QQuickWindow *>(transfer->destinationHost.data())) return false;
    // No QObject access follows this scalar/native comparison, before the
    // caller's private Rust Core commit in the same native entry point.
    auto entry = views_.find(transfer->key);
    if (entry == views_.end() || entry->retiring || entry->incarnation != transfer->source.incarnation
        || entry->windowGeneration != transfer->source.windowGeneration
        || entry->host != transfer->source.host || entry->ui != transfer->source.ui) return false;
    entry->hostKey = transfer->destinationHost;
    entry->host = transfer->destinationHost;
    entry->ui = transfer->destinationUi;
    entry->windowGeneration = transfer->destinationGeneration;
    entry->incarnation = ++nextGeneration_;
    transfer->stagedIncarnation = entry->incarnation;
    transfer->associated = true;
    return true;
}

void FerricProfileLifetime::removeViewTransfer(ViewTransferState &transfer) {
    for (const auto &watch : std::as_const(transfer.watches)) disconnect(watch);
    transfer.watches.clear();
    if (viewTransfers_.value(transfer.key).get() == &transfer) viewTransfers_.remove(transfer.key);
    transfer.ended = true;
}

bool FerricProfileLifetime::completeViewTransfer(ViewTransfer &handle) {
    const auto transfer = handle.state;
    if (!transfer || !transfer->associated || !transferWindowsCurrent(*transfer)) return false;
    const auto entry = views_.constFind(transfer->key);
    if (entry == views_.cend() || entry->retiring || entry->incarnation != transfer->stagedIncarnation) return false;
    const QPointer<FerricProfileLifetime> self(this);
    const QPointer<QQuickItem> view = transfer->view;
    removeViewTransfer(*transfer);
    if (self->tls_ && view) self->tls_->observeView(view);
    if (self) emit self->credentialTransportChanged();
    return self && view;
}

bool FerricProfileLifetime::cancelViewTransfer(ViewTransfer &handle, bool restoreSource, const std::function<bool()> &sourceProof) {
    const auto transfer = handle.state;
    return cancelViewTransfer(transfer, restoreSource, sourceProof);
}

bool FerricProfileLifetime::cancelViewTransfer(const std::shared_ptr<ViewTransferState> &transfer, bool restoreSource, const std::function<bool()> &sourceProof) {
    // Pin by value across property/signal callbacks, even if the handle is freed.
    const auto pinned = transfer;
    const QPointer<FerricProfileLifetime> self(this);
    if (!pinned || pinned->ended || pinned->lifetime != this) return false;
    const QPointer<QQuickItem> view = pinned->view;
    auto sourceCurrent = [&] {
        if (!self || !view || pinned->ended || self->viewTransfers_.value(pinned->key) != pinned) return false;
        return std::any_of(self->windows_.cbegin(), self->windows_.cend(), [&](const Window &owner) {
            return owner.host && owner.host == pinned->source.host && owner.ui && owner.ui == pinned->source.ui
                && owner.ready && owner.generation == pinned->source.windowGeneration;
        });
    };
    bool routed = restoreSource && sourceCurrent()
        && view->window() == qobject_cast<QQuickWindow *>(pinned->source.host.data());
    if (routed) {
        const auto host = objectProperty(view, "viewHost");
        routed = sourceCurrent() && host == pinned->source.host;
    }
    if (routed) {
        const auto ui = objectProperty(view, "viewUi");
        routed = sourceCurrent() && ui == pinned->source.ui;
    }
    if (routed) {
        const auto profile = objectProperty(view, "profile");
        routed = sourceCurrent() && profile == pinned->source.profile;
    }
    if (!self || pinned->ended) return false;
    auto entry = views_.find(pinned->key);
    // The consumer supplies a pure Core comparison after the last property
    // evaluation; no QML callback can slip between that proof and routing.
    const bool proven = routed && (!sourceProof || sourceProof());
    const bool restored = proven && sourceCurrent()
        && view->window() == qobject_cast<QQuickWindow *>(pinned->source.host.data())
        && !pinned->retire && !shuttingDown_
        && nextGeneration_ != std::numeric_limits<quint64>::max()
        && entry != views_.end() && !entry->retiring
        && entry->incarnation == (pinned->associated ? pinned->stagedIncarnation : pinned->source.incarnation);
    if (restored) {
        entry->hostKey = pinned->source.host;
        entry->host = pinned->source.host;
        entry->ui = pinned->source.ui;
        entry->windowGeneration = pinned->source.windowGeneration;
        entry->incarnation = ++nextGeneration_;
    }
    removeViewTransfer(*pinned);
    if (!restored && view) releaseView(view);
    if (restored && self && self->tls_ && view) self->tls_->observeView(view);
    if (restored && self) emit self->credentialTransportChanged();
    return restored && self && view;
}

void FerricProfileLifetime::shutdown() {
    shuttingDown_ = true;
    const auto windows = windows_;
    for (const auto &entry : windows)
        releaseWindow(entry.key);
}

void FerricProfileLifetime::refresh() {
    QObject *next = nullptr;
    for (const auto &entry : std::as_const(windows_)) {
        if (entry.host && entry.ui && entry.ready) {
            next = entry.ui;
            break;
        }
    }
    if (policyUiKey_ != next) {
        policyUi_ = next;
        policyUiKey_ = next;
        emit policyUiChanged();
    }
    emit consumersChanged();
    if (!consumed_ || drained_ || drainQueued_ || !windows_.isEmpty() ||
        !views_.isEmpty() || !popupLeases_.isEmpty())
        return;
    drainQueued_ = true;
    QTimer::singleShot(0, this, [this] {
        drainQueued_ = false;
        if (windows_.isEmpty() && views_.isEmpty() && popupLeases_.isEmpty()) {
            drained_ = true;
            emit drained();
        }
    });
}
