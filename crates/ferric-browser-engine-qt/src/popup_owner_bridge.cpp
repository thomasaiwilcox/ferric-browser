#include "popup_owner_bridge.h"
#include "ferric-browser-engine-qt/src/bridge.cxxqt.h"
#include <QThread>
#include <QWindow>
#include <QJSValue>

namespace {
QObject *objectProperty(QObject *object, const char *name) {
    if (!object) return nullptr;
    const auto value = object->property(name);
    if (auto *result = value.value<QObject *>()) return result;
    return value.value<QJSValue>().toQObject();
}
QWindow *hostWindow(QObject *object) {
    while (object && !qobject_cast<QWindow *>(object)) object = object->parent();
    return qobject_cast<QWindow *>(object);
}
QObject *hostUi(QObject *host) {
    auto *ui = objectProperty(host, "windowUi");
    return ui ? ui : objectProperty(host, "browserUi");
}
QObject *hostProfile(QObject *host) {
    auto *profile = objectProperty(host, "windowWebEngineProfile");
    return profile ? profile : objectProperty(host, "browserProfile");
}
}

bool FerricPopupOwnerBridge::initialize(QObject *source, QObject *destination,
                                        const QString &token) {
    if (!source || !destination || source == destination ||
        source->thread() != QThread::currentThread() ||
        destination->thread() != QThread::currentThread())
        return false;
    auto *opener = qobject_cast<BrowserUi *>(source);
    auto *popup = qobject_cast<BrowserUi *>(destination);
    return opener && popup && opener->handoff_popup_owner(*popup, token);
}

bool FerricPopupOwnerBridge::inheritProfile(QObject *source, QObject *destination,
                                           const QString &profileName) {
    if (!source || !destination || source == destination ||
        source->thread() != QThread::currentThread() ||
        destination->thread() != QThread::currentThread())
        return false;
    auto *owner = qobject_cast<BrowserUi *>(source);
    auto *child = qobject_cast<BrowserUi *>(destination);
    return owner && child && owner->initialize_window_owner(*child, profileName);
}

bool FerricPopupOwnerBridge::sharesPermissionSession(QObject *left,
                                                    QObject *right) const {
    if (!left || !right || left->thread() != QThread::currentThread() ||
        right->thread() != QThread::currentThread())
        return false;
    const auto *source = qobject_cast<BrowserUi *>(left);
    const auto *target = qobject_cast<BrowserUi *>(right);
    return source && target && source->shares_permission_session(*target);
}

bool FerricPopupOwnerBridge::preparePageWindow(QObject *source, QObject *destination,
                                               const QString &tabId) {
    if (!source || !destination || source == destination
        || source->thread() != QThread::currentThread()
        || destination->thread() != QThread::currentThread()) return false;
    auto *owner = qobject_cast<BrowserUi *>(source);
    auto *child = qobject_cast<BrowserUi *>(destination);
    auto *host = hostWindow(this);
    auto *sourceHost = objectProperty(host, "windowPreparedPageFocusSource");
    auto *profile = hostProfile(sourceHost);
    if (preparedSource_ || !owner || !child || !host || hostUi(host) != child
        || !qobject_cast<QWindow *>(sourceHost) || hostUi(sourceHost) != owner
        || !profile || hostProfile(host) != profile
        || objectProperty(host, "activeView")) return false;
    if (!owner->initialize_page_owner(*child, tabId)) return false;
    preparedSource_ = owner;
    preparedDestination_ = child;
    preparedHost_ = sourceHost;
    preparedProfile_ = profile;
    return true;
}

bool FerricPopupOwnerBridge::commitPageWindow() {
    auto *host = hostWindow(this);
    auto *source = qobject_cast<BrowserUi *>(preparedSource_.data());
    auto *destination = qobject_cast<BrowserUi *>(preparedDestination_.data());
    if (preparedCommitted_ || !host || !source || !destination || !preparedHost_ || !preparedProfile_
        || hostUi(host) != destination || hostUi(preparedHost_) != source
        || source->thread() != QThread::currentThread() || destination->thread() != QThread::currentThread()
        || hostProfile(preparedHost_) != preparedProfile_ || hostProfile(host) != preparedProfile_
        || preparedHost_->property("runtimeReleased").toBool()
        || host->property("runtimeReleased").toBool() || objectProperty(host, "activeView")) return false;
    preparedCommitted_ = source->commit_page_owner(*destination);
    return preparedCommitted_;
}

QString FerricPopupOwnerBridge::activatePreparedPage() {
    auto *host = hostWindow(this);
    auto *destination = qobject_cast<BrowserUi *>(preparedDestination_.data());
    auto *view = objectProperty(host, "activeView");
    if (!preparedCommitted_) return QStringLiteral("Page opening cancelled because this window changed. Open the page again.");
    preparedCommitted_ = false;
    if (!host || !destination || !view || !preparedProfile_
        || destination->thread() != QThread::currentThread() || hostUi(host) != destination
        || hostProfile(host) != preparedProfile_ || objectProperty(view, "profile") != preparedProfile_
        || objectProperty(view, "viewUi") != destination
        || objectProperty(view, "viewHost") != host
        || view->property("stableTabId").toString() != destination->tab_id_for_index(0)
        || host->property("runtimeReleased").toBool()) return QStringLiteral("Page opening cancelled because this window changed. Open the page again.");
    return destination->activate_prepared_page();
}
