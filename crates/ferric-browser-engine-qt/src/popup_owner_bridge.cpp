#include "popup_owner_bridge.h"
#include "ferric-browser-engine-qt/src/bridge.cxxqt.h"
#include <QThread>
#include <QWindow>
#include <QJSValue>
#include <QWebEngineNewWindowRequest>
#include <QCoreApplication>
#include <QTimer>
#include <QSignalBlocker>

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
bool publishPreparedProperties(QPointer<BrowserUi> child);
}

struct FerricPopupOwnerBridge::PreparedWindow {
    QPointer<BrowserUi> source, destination;
    QPointer<QObject> sourceHost, host, profile, root;
    QString tab;
    bool published = false;
    bool corePublished = false;
    bool failed = false;
};

bool FerricPopupOwnerBridge::initialize(QObject *source, QObject *destination,
                                        const QString &token) {
    if (!source || !destination || source == destination ||
        source->thread() != QThread::currentThread() ||
        destination->thread() != QThread::currentThread())
        return false;
    auto *opener = qobject_cast<BrowserUi *>(source);
    auto *popup = qobject_cast<BrowserUi *>(destination);
    auto *host = hostWindow(this);
    auto *request = qobject_cast<QWebEngineNewWindowRequest *>(objectProperty(host, "windowPopupRequest"));
    const auto joinGroup = request && (request->destination() == QWebEngineNewWindowRequest::InNewTab
        || request->destination() == QWebEngineNewWindowRequest::InNewBackgroundTab);
    return opener && popup && host && hostUi(host) == popup && request
        && opener->handoff_popup_owner(*popup, token, joinGroup);
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
    const QPointer<FerricPopupOwnerBridge> self(this);
    const auto state = std::make_shared<PreparedWindow>();
    state->source = qobject_cast<BrowserUi *>(source);
    state->destination = qobject_cast<BrowserUi *>(destination);
    state->host = hostWindow(this);
    state->tab = tabId;
    state->root = objectProperty(state->host,"rootWindow");
    auto alive = [&] { return self && state->source && state->destination && state->host; };
    if (!alive() || prepared_ || source == destination
        || source->thread() != QThread::currentThread()
        || destination->thread() != QThread::currentThread()) return false;
    state->sourceHost = objectProperty(state->host,"windowPreparedPageFocusSource");
    if (!alive() || !qobject_cast<QWindow *>(state->sourceHost.data())
        || hostUi(state->sourceHost) != state->source || !alive()
        || hostUi(state->host) != state->destination || !alive()) return false;
    state->profile = hostProfile(state->sourceHost);
    if (!alive() || !state->profile || hostProfile(state->host) != state->profile
        || !alive() || objectProperty(state->host,"activeView") || !alive()) return false;
    bool initialized = false;
    {
        QSignalBlocker noSourceNotifications(state->source);
        QSignalBlocker noNotifications(state->destination);
        initialized = state->source->initialize_page_owner(*state->destination,tabId);
        if (!state->destination) noNotifications.dismiss();
        if (!state->source) noSourceNotifications.dismiss();
    }
    if (!alive() || !initialized) return false;
    self->prepared_ = state;
    // Initialization runs in Component.onCompleted. As with live transfers,
    // neither Rust references nor the construction signal may span publication.
    QTimer::singleShot(0,QCoreApplication::instance(),[self,state] {
        auto current = [&] { return self && state->source && state->sourceHost
            && state->destination && state->host && state->profile; };
        if (!current() || !publishPreparedProperties(state->destination) || !current()) {
            if (state->host) QMetaObject::invokeMethod(state->host,"rejectPopupAdmission",Qt::DirectConnection);
            if (state->source) state->source->runtime_work_available();
            return;
        }
        state->published = true;
        state->host->setProperty("windowPreparedPagePublished",true);
    });
    return true;
}

void FerricPopupOwnerBridge::failPreparedPublication(const std::shared_ptr<PreparedWindow> &state) {
    if (!state || state->failed) return;
    state->failed = true;
    if (state->root) QMetaObject::invokeMethod(state->root,"failPreparedPagePublication",Qt::DirectConnection,
        Q_ARG(QVariant,QVariant(state->tab)));
}

bool FerricPopupOwnerBridge::commitPageWindow() {
    const auto state = prepared_;
    const QPointer<FerricPopupOwnerBridge> self(this);
    auto alive = [&] { return self && state && state->source && state->destination
        && state->sourceHost && state->host && state->profile; };
    if (!alive() || !state->published || preparedCommitted_
        || hostUi(state->host) != state->destination || !alive()
        || hostUi(state->sourceHost) != state->source || !alive()
        || state->source->thread() != QThread::currentThread()
        || state->destination->thread() != QThread::currentThread()
        || hostProfile(state->sourceHost) != state->profile || !alive()
        || hostProfile(state->host) != state->profile || !alive()
        || state->sourceHost->property("runtimeReleased").toBool() || !alive()
        || state->host->property("runtimeReleased").toBool() || !alive()
        || objectProperty(state->host,"activeView") || !alive()) return false;
    bool committed = false;
    {
        QSignalBlocker sourceNotifications(state->source), destinationNotifications(state->destination);
        committed = state->source->commit_page_owner(*state->destination);
        if (!state->destination) destinationNotifications.dismiss();
        if (!state->source) sourceNotifications.dismiss();
    }
    if (!committed) return false;
    if (!self || !state->destination || !state->host) return true;
    self->preparedCommitted_ = true;
    QTimer::singleShot(15000,QCoreApplication::instance(),[state] {
        // The coordinator ignores already registered records and keeps a valid
        // visible owner on a registration retry; only this preparation is named.
        failPreparedPublication(state);
    });
    // Neither Core owner notifies while Rust holds both references. Publication
    // is independently guarded, so source retirement cannot erase the child.
    QTimer::singleShot(0,QCoreApplication::instance(),[self,state] {
        if (state->source) {
            bool changed = false;
            {
                QSignalBlocker notifications(state->source);
                changed = state->source->publish_prepared_page_source(state->tab);
                if (!state->source) notifications.dismiss();
            }
            if (changed && state->source) publishPreparedProperties(state->source);
            if (state->source) state->source->runtime_work_available();
        }
        if (!self || !state->destination || !state->host || !state->profile) {
            failPreparedPublication(state); return;
        }
        bool published = false;
        {
            QSignalBlocker notifications(state->destination);
            published = state->destination->publish_prepared_page_owner();
            if (!state->destination) notifications.dismiss();
        }
        if (!published && self && state->destination && state->host && state->profile
            && hostUi(state->host) == state->destination && state->destination && state->host
            && hostProfile(state->host) == state->profile && state->destination && state->host) {
            {
                QSignalBlocker notifications(state->destination);
                published = state->destination->recover_prepared_page_owner();
                if (!state->destination) notifications.dismiss();
            }
            if (published && self && state->host) {
                self->preparedCommitted_ = false;
            }
        }
        if (!published || !state->destination || !publishPreparedProperties(state->destination)
            || !self || !state->destination || !state->host || !state->profile) {
            failPreparedPublication(state); return;
        }
        state->corePublished = true;
        state->host->setProperty("windowPreparedPageCorePublished",true);
        if (state->destination) state->destination->runtime_work_available();
    });
    return true;
}

QString FerricPopupOwnerBridge::activatePreparedPage() {
    const auto state = prepared_;
    auto *host = hostWindow(this);
    auto *destination = state ? state->destination.data() : nullptr;
    auto *view = objectProperty(host, "activeView");
    if (!state || !state->published || !state->corePublished || !preparedCommitted_) return QStringLiteral("Page opening cancelled because this window changed. Open the page again.");
    preparedCommitted_ = false;
    if (!host || !destination || !view || !state->profile
        || destination->thread() != QThread::currentThread() || hostUi(host) != destination
        || hostProfile(host) != state->profile || objectProperty(view, "profile") != state->profile
        || objectProperty(view, "viewUi") != destination
        || objectProperty(view, "viewHost") != host
        || view->property("stableTabId").toString() != destination->tab_id_for_index(0)
        || host->property("runtimeReleased").toBool()) return QStringLiteral("Page opening cancelled because this window changed. Open the page again.");
    QString result;
    {
        QSignalBlocker notifications(destination);
        result = destination->activate_prepared_page();
        if (!state->destination) notifications.dismiss();
    }
    QTimer::singleShot(0,QCoreApplication::instance(),[state] {
        if (state->destination) publishPreparedProperties(state->destination);
        if (state->root && state->destination && state->host) {
            const auto intent=state->destination->created_page_presentation_intent();
            if (!intent.isEmpty() && state->root && state->destination && state->host)
                QMetaObject::invokeMethod(state->root,"presentCreatedPageDecision",Qt::DirectConnection,
                    Q_ARG(QVariant,QVariant::fromValue(static_cast<QObject *>(state->destination.data()))),
                    Q_ARG(QVariant,QVariant::fromValue(state->host.data())),Q_ARG(QVariant,intent));
        }
        if (state->destination) state->destination->runtime_work_available();
    });
    return result;
}

QString FerricPopupOwnerBridge::createdPagePresentationIntent() const {
    const auto state=prepared_;
    return state && state->destination ? state->destination->created_page_presentation_intent() : QString();
}

bool FerricPopupOwnerBridge::presentCreatedPage() {
    const auto state=prepared_;
    const QPointer<FerricPopupOwnerBridge> self(this);
    const QPointer<QWindow> window(state ? qobject_cast<QWindow *>(state->host.data()) : nullptr);
    auto alive=[&] { return self && state && state->destination && state->host && state->profile && window; };
    if (!alive() || hostUi(state->host) != state->destination || !alive()
        || hostProfile(state->host) != state->profile || !alive()) return false;
    const QPointer<QObject> view(objectProperty(state->host,"activeView"));
    if (!alive() || !view || objectProperty(view,"viewUi") != state->destination || !alive() || !view
        || objectProperty(view,"viewHost") != state->host || !alive() || !view
        || objectProperty(view,"profile") != state->profile || !alive() || !view
        || !state->destination->take_created_page_presentation()) return false;
    // The QML caller holds the shared activation batch across this real show.
    // No Qt callback follows the final pure generation/Target consume first.
    window->show();
    return true;
}

#include "profile_lifetime.h"
#include "ferric-browser-engine-qt/src/bridge.cxx.h"
#include <QMetaProperty>
#include <QSignalBlocker>
#include <QQuickWindow>
#include <QCoreApplication>
#include <QTimer>

struct FerricPopupOwnerBridge::LivePreparation {
    QPointer<BrowserUi> source, destination;
    QPointer<QObject> sourceHost, host, profile;
    QString nonce;
    bool consumed = false;
    bool published = false;
};

namespace {
// Bootstrap a dormant child without notifying QML while Rust owns direct
// references. Publish the settled values only after those references unwind.
// Every notification is followed by a fresh QObject liveness check.
bool publishPreparedProperties(QPointer<BrowserUi> child) {
    if (!child) return false;
    const auto *meta = child->metaObject();
    QList<QMetaMethod> notifications;
    for (int index = QObject::staticMetaObject.propertyCount(); index < meta->propertyCount(); ++index) {
        const auto signal = meta->property(index).notifySignal();
        if (signal.isValid() && signal.parameterCount() == 0 && !notifications.contains(signal)) notifications.append(signal);
    }
    for (const auto &signal : notifications) {
        if (!child) return false;
        signal.invoke(child, Qt::DirectConnection);
    }
    return child;
}
bool invokeView(QObject *host, const char *method, QQuickItem *view) {
    QVariant result;
    return host && QMetaObject::invokeMethod(host, method, Qt::DirectConnection,
        Q_RETURN_ARG(QVariant, result), Q_ARG(QVariant, QVariant::fromValue(static_cast<QObject *>(view))))
        && result.toBool();
}
}

void ferric_queue_page_creation_publication(const BrowserUi &ui) {
    const QPointer<BrowserUi> owner(const_cast<BrowserUi *>(&ui));
    QTimer::singleShot(0,QCoreApplication::instance(),[owner] {
        if (!owner) return;
        bool changed = false;
        {
            QSignalBlocker notifications(owner);
            changed = owner->publish_page_creation();
            if (!owner) notifications.dismiss();
        }
        if (changed && owner) publishPreparedProperties(owner);
        if (owner) owner->runtime_work_available();
    });
}

bool FerricPopupOwnerBridge::prepareLivePageWindow(QObject *source, QObject *destination, const QString &nonce) {
    const QPointer<FerricPopupOwnerBridge> self(this);
    const auto state = std::make_shared<LivePreparation>();
    state->source = qobject_cast<BrowserUi *>(source);
    state->destination = qobject_cast<BrowserUi *>(destination);
    state->host = hostWindow(this);
    state->nonce = nonce;
    auto alive = [&] { return self && state->source && state->destination && state->host; };
    if (!alive() || live_ || source == destination || source->thread() != QThread::currentThread()
        || destination->thread() != QThread::currentThread()) return false;
    state->sourceHost = objectProperty(state->host, "windowLivePageSourceHost");
    if (!alive() || !qobject_cast<QQuickWindow *>(state->sourceHost.data())) return false;
    auto *sourceUi = hostUi(state->sourceHost);
    if (!alive() || !state->sourceHost || sourceUi != source) return false;
    auto *destinationUi = hostUi(state->host);
    if (!alive() || destinationUi != destination) return false;
    state->profile = hostProfile(state->sourceHost);
    if (!alive() || !state->sourceHost || !state->profile) return false;
    auto *profile = hostProfile(state->host);
    if (!alive() || profile != state->profile) return false;
    auto *view = objectProperty(state->host, "activeView");
    if (!alive() || view) return false;
    bool initialized = false;
    {
        const QSignalBlocker noNotifications(state->destination);
        initialized = state->source->initialize_live_page_owner(*state->destination, nonce);
    }
    if (!alive() || !initialized) return false;
    self->live_ = state;
    // Component.onCompleted is a QML signal handler. Qt aborts if one of its
    // notifications deletes that handler's window synchronously. Publish only
    // after construction unwinds, still outside every direct Rust reference.
    QTimer::singleShot(0,QCoreApplication::instance(),[self,state] {
        auto current = [&] { return self && state->source && state->sourceHost
            && state->destination && state->host && state->profile; };
        if (!current() || !publishPreparedProperties(state->destination) || !current()) {
            if (state->source) state->source->cancel_live_page_move(state->nonce);
            if (state->host) QMetaObject::invokeMethod(state->host,"rejectPopupAdmission",Qt::DirectConnection);
            return;
        }
        state->published = true;
        state->host->setProperty("windowLivePagePublished",true);
    });
    return true;
}

bool FerricPopupOwnerBridge::commitLivePageWindow() {
    const auto state = live_;
    const QPointer<FerricPopupOwnerBridge> self(this);
    auto alive = [&] { return self && state && state->source && state->destination
        && state->sourceHost && state->host && state->profile; };
    if (!alive() || !state->published || state->consumed) return false;
    state->consumed = true;
    if (state->source->thread() != QThread::currentThread() || state->destination->thread() != QThread::currentThread()) return false;
    const QPointer<QQuickItem> view(qobject_cast<QQuickItem *>(objectProperty(state->sourceHost, "activeView")));
    if (!alive() || !view) return false;
    const QPointer<FerricProfileLifetime> lifetime(qobject_cast<FerricProfileLifetime *>(view->parent()));
    if (!lifetime) return false;
    const auto current = lifetime->currentView(view);
    if (!alive() || !view || !lifetime || !current || current->host != state->sourceHost
        || current->ui != state->source || current->profile != state->profile) return false;
    const auto tab = view->property("stableTabId").toString();
    if (!alive() || !view || !lifetime) return false;
    const auto prompt = view->property("promptLeaseIdentity").toString();
    if (!alive() || !view || !lifetime) return false;
    const auto index = view->property("tabIndex").toInt();
    if (!alive() || !view || !lifetime
        || !state->source->validate_live_page_owner(*state->destination, state->nonce, tab, prompt, index)) return false;
    const auto rollback = ferric::page_windows::capture_page_rollback(*state->source,state->nonce);
    auto transfer = lifetime->beginViewTransfer(view, state->host, state->destination, [&] {
        return state->source && rollback->source_current(*state->source);
    });
    if (!view || !lifetime || !transfer) return false;
    const bool staged = alive() && invokeView(state->host, "stageLivePageView", view);
    bool committed = false;
    if (staged && alive() && view && lifetime
        && lifetime->pinViewTransferIdentity(*transfer,tab,prompt,0)
        && alive() && view && lifetime && lifetime->associateViewTransfer(*transfer)) {
        // Pure Rust validation/import/release immediately follows final native
        // association. No property read or QML callback belongs in this gap.
        if (alive() && view && lifetime) committed = state->source->commit_live_page_owner(*state->destination, state->nonce);
    }
    if (!committed) {
        if (state->sourceHost && state->source && view) invokeView(state->sourceHost, "restoreStagedLiveView", view);
        if (lifetime) lifetime->cancelViewTransfer(*transfer, state->sourceHost && state->source && view, [&] {
            return state->source && rollback->source_current(*state->source);
        });
        if (state->sourceHost && state->source && view) invokeView(state->sourceHost, "finishStagedLiveRollback", view);
        return false;
    }
    // The destination now owns the receipt and continuation. Source retirement
    // is independent after commit and must never reject this surviving page.
    auto destinationAlive = [&] { return state->destination && state->host && state->profile && view && lifetime; };
    const bool nativeCompleted = lifetime && lifetime->completeViewTransfer(*transfer);
    if (state->source) state->source->publish_live_page_source(state->nonce);
    if (!destinationAlive() || !nativeCompleted
        || !state->destination->ready_live_page_owner(state->nonce)) {
        if (state->destination) state->destination->fail_live_page_owner(state->nonce);
        if (state->host) invokeView(state->host,"abandonEmptyLivePage",view);
        return true;
    }
    const bool registered = invokeView(state->host, "finishLivePageView", view);
    if (!destinationAlive()) return true;
    if (!state->destination->publish_live_page_owner(state->nonce,registered)) {
        state->destination->fail_live_page_owner(state->nonce);
    }
    return true;
}
