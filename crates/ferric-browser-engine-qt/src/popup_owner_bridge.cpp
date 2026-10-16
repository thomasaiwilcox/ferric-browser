#include "popup_owner_bridge.h"
#include "ferric-browser-engine-qt/src/lib.cxxqt.h"
#include <QThread>

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
