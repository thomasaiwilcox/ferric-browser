#include "profile_lifetime.h"
#include <QTimer>
#include <algorithm>
#include <utility>

FerricProfileLifetime::~FerricProfileLifetime() {
    // A profile must outlive all of its pages, including during application
    // disposal before queued view deletions have been delivered.
    shuttingDown_ = true;
    for (const auto &entry : std::as_const(windows_)) {
        disconnect(entry.hostDestroyed);
        disconnect(entry.uiDestroyed);
    }
    const auto views = views_.values();
    views_.clear();
    for (const auto &entry : views) {
        if (!entry.item)
            continue;
        disconnect(entry.item, nullptr, this, nullptr);
        delete entry.item.data();
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
            return window.ui == ui;
    Window entry{host, host, ui, {}, {}};
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
    if (views_.contains(view) && views_.value(view).retiring)
        return false;
    const auto owner = std::find_if(
        windows_.cbegin(), windows_.cend(), [host, ui](const Window &entry) {
            return entry.host == host && entry.ui == ui && entry.ready;
        });
    if (owner == windows_.cend())
        return false;
    // QObject ownership and visual parenting are independent in Qt Quick.
    // Leave the StackLayout parentItem unchanged while owning the live object.
    for (QObject *ancestor = this; ancestor; ancestor = ancestor->parent())
        if (ancestor == view)
            return false;
    if (!views_.contains(view)) {
        connect(view, &QObject::destroyed, this, [this, view] {
            views_.remove(view);
            refresh();
        });
    }
    view->setParent(this);
    views_.insert(view, View{view, host, host, ui});
    refresh();
    return true;
}

QObject *FerricProfileLifetime::ownerUi(QQuickItem *view) const {
    const auto it = views_.constFind(view);
    if (it == views_.cend() || it->retiring || !it->item || !it->host ||
        !it->ui)
        return nullptr;
    for (const auto &window : windows_)
        if (window.key == it->hostKey && window.host && window.ui == it->ui &&
            window.ready)
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
    // Moved views already name their destination. Retire only views still
    // owned by the released host, then wait for their actual destruction.
    const auto retiring = views_.values();
    for (const auto &entry : retiring)
        if (entry.hostKey == host && entry.item)
            releaseView(entry.item);
    refresh();
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
    guard->deleteLater();
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
        !views_.isEmpty())
        return;
    drainQueued_ = true;
    QTimer::singleShot(0, this, [this] {
        drainQueued_ = false;
        if (windows_.isEmpty() && views_.isEmpty()) {
            drained_ = true;
            emit drained();
        }
    });
}
