#include "profile_lifetime.h"

#include <QMetaProperty>
#include <QQmlContext>
#include <QThread>
#include <QTimer>
#include <QVariant>
#include <QQuickWindow>

#include <algorithm>
#include <utility>

#if QT_VERSION != QT_VERSION_CHECK(6, 11, 2)
#error "Popup opener lifetime selector is qualified only for Qt 6.11.2"
#endif

namespace {
constexpr auto kDelegateClassName =
    "QtWebEngineCore::RenderWidgetHostViewQtDelegateItem";
constexpr int kDiscardedLifecycleState = 2;
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
    for (QObject *ancestor = factory; ancestor; ancestor = ancestor->parent())
        if (ancestor == profile)
            return false;
    if (!ownResource(factory))
        return false;
    // WebEngineProfilePrototype owns its instance through a scoped pointer.
    // Keep that ownership intact rather than deleting the instance separately.
    profile->setParent(factory);
    return true;
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
    if (drained_ || shuttingDown_ || !view || view->thread() != thread())
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
        if (sameOwner)
            return true;
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
    refresh();
    return true;
}

bool FerricProfileLifetime::isCurrentView(QQuickItem *view,
                                          QObject **profile,
                                          QQmlEngine **engine) const {
    if (!view || view->thread() != thread() || QThread::currentThread() != thread())
        return false;
    const auto it = views_.constFind(view);
    if (it == views_.cend() || it->retiring || !it->item || !it->host ||
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
    const QVariant profileValue = view->property("profile");
    QObject *viewProfile = profileValue.value<QObject *>();
    QQmlContext *context = QQmlEngine::contextForObject(view);
    QQmlEngine *viewEngine = context ? context->engine() : nullptr;
    if (!viewProfile || !viewEngine || viewEngine->thread() != thread())
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
