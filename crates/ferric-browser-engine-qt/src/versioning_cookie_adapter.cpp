#include "versioning_cookie_adapter.h"

#include <QtCore/QThread>

#include <limits>
#include <utility>

FerricVersioningCookieAdapter::~FerricVersioningCookieAdapter()
{
    detach();
}

bool FerricVersioningCookieAdapter::attach(QQuickWebEngineProfile *profile)
{
    if (!profile || QThread::currentThread() != m_context.thread()
        || profile->thread() != m_context.thread())
        return false;
    auto *store = profile->cookieStore();
    if (!store || store->thread() != m_context.thread())
        return false;

    detach();
    if (m_epoch == std::numeric_limits<quint64>::max())
        return false;
    ++m_epoch;
    m_sequence = 0;
    m_invalidated = false;
    m_overflowed = false;
    m_profile = profile;
    m_store = store;
    m_attached = true;
    const auto boundEpoch = m_epoch;

    // The receiver context marshals callbacks onto this adapter's thread.
    // The callback only copies bounded typed fields into a private queue.
    m_added = QObject::connect(store, &QWebEngineCookieStore::cookieAdded, &m_context,
                               [this, boundEpoch](const QNetworkCookie &cookie) {
                                   enqueue(Kind::ObservedAdded, cookie, boundEpoch);
                               });
    m_removed = QObject::connect(store, &QWebEngineCookieStore::cookieRemoved, &m_context,
                                 [this, boundEpoch](const QNetworkCookie &cookie) {
                                     enqueue(Kind::ObservedRemoved, cookie, boundEpoch);
                                 });
    m_profileDestroyed = QObject::connect(profile, &QObject::destroyed, &m_context,
                                          [this, boundEpoch] {
                                              if (m_epoch == boundEpoch)
                                                  detach();
                                          });
    m_storeDestroyed = QObject::connect(store, &QObject::destroyed, &m_context,
                                        [this, boundEpoch] {
                                            if (m_epoch == boundEpoch)
                                                detach();
                                        });
    return true;
}

void FerricVersioningCookieAdapter::detach()
{
    if (QThread::currentThread() != m_context.thread())
        return;
    QObject::disconnect(m_added);
    QObject::disconnect(m_removed);
    QObject::disconnect(m_profileDestroyed);
    QObject::disconnect(m_storeDestroyed);
    m_added = {};
    m_removed = {};
    m_profileDestroyed = {};
    m_storeDestroyed = {};
    m_profile = nullptr;
    m_store = nullptr;
    m_queue.clear();
    m_sequence = 0;
    m_invalidated = false;
    m_overflowed = false;
    if (std::exchange(m_attached, false)
        && m_epoch != std::numeric_limits<quint64>::max())
        ++m_epoch;
}

bool FerricVersioningCookieAdapter::requestEnumeration()
{
    if (QThread::currentThread() != m_context.thread() || !attached() || m_invalidated)
        return false;
    m_store->loadAllCookies();
    // This means only that the void-returning request was dispatched. Qt has
    // no public enumeration-complete signal or consistency token here.
    return true;
}

std::vector<FerricVersioningCookieAdapter::Observation>
FerricVersioningCookieAdapter::takeObservations()
{
    if (QThread::currentThread() != m_context.thread() || m_invalidated)
        return {};
    std::vector<Observation> result;
    result.reserve(m_queue.size());
    while (!m_queue.empty()) {
        result.push_back(std::move(m_queue.front()));
        m_queue.pop_front();
    }
    return result;
}

void FerricVersioningCookieAdapter::invalidate(bool overflow)
{
    m_invalidated = true;
    m_overflowed = overflow;
    m_queue.clear();
}

void FerricVersioningCookieAdapter::enqueue(Kind kind, const QNetworkCookie &cookie,
                                            quint64 sourceEpoch)
{
    if (sourceEpoch != m_epoch || !attached() || m_invalidated)
        return;
    if (m_queue.size() >= maxQueued() || m_sequence == std::numeric_limits<quint64>::max()) {
        invalidate(true);
        return;
    }
    if (cookie.name().size() > 1024 || cookie.value().size() > 16 * 1024
        || cookie.domain().size() > 253 || cookie.path().size() > 4096) {
        invalidate(false);
        return;
    }
    Observation item;
    item.epoch = m_epoch;
    item.sequence = ++m_sequence;
    item.kind = kind;
    item.name = cookie.name();
    item.value = cookie.value();
    item.domain = cookie.domain();
    item.path = cookie.path();
    item.expiration = cookie.expirationDate();
    item.sameSite = cookie.sameSitePolicy();
    item.session = cookie.isSessionCookie();
    item.secure = cookie.isSecure();
    item.httpOnly = cookie.isHttpOnly();
    m_queue.push_back(std::move(item));
}
