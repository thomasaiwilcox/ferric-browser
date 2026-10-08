#ifndef FERRIC_VERSIONING_COOKIE_ADAPTER_H
#define FERRIC_VERSIONING_COOKIE_ADAPTER_H

#include <QtCore/QByteArray>
#include <QtCore/QDateTime>
#include <QtCore/QMetaObject>
#include <QtCore/QObject>
#include <QtCore/QPointer>
#include <QtCore/QString>
#include <QtNetwork/QNetworkCookie>
#include <QtWebEngineCore/QWebEngineCookieStore>
#include <QtWebEngineQuick/QQuickWebEngineProfile>

#include <cstddef>
#include <deque>
#include <vector>

// C++-only diagnostic observation boundary. This type is deliberately not a
// QObject/QML model: cookie bytes never enter a QVariant, signal payload,
// ordinary QML property, log or IPC projection. All calls use its QObject
// context's thread. No observation is proof of enumeration completeness.
class FerricVersioningCookieAdapter final {
public:
    enum class Kind { ObservedAdded, ObservedRemoved };

    struct Observation {
        quint64 epoch = 0;
        quint64 sequence = 0;
        Kind kind = Kind::ObservedAdded;
        QByteArray name;
        QByteArray value;
        QString domain;
        QString path;
        QDateTime expiration;
        QNetworkCookie::SameSite sameSite = QNetworkCookie::SameSite::Default;
        bool session = true;
        bool secure = false;
        bool httpOnly = false;
    };

    FerricVersioningCookieAdapter() = default;
    ~FerricVersioningCookieAdapter();
    FerricVersioningCookieAdapter(const FerricVersioningCookieAdapter &) = delete;
    FerricVersioningCookieAdapter &operator=(const FerricVersioningCookieAdapter &) = delete;

    static constexpr std::size_t maxQueued() { return 64; }
    bool attach(QQuickWebEngineProfile *profile);
    void detach();
    bool requestEnumeration();
    std::vector<Observation> takeObservations();

    quint64 epoch() const { return m_epoch; }
    std::size_t queuedCount() const { return m_queue.size(); }
    bool attached() const { return m_attached && m_profile && m_store; }
    bool invalidated() const { return m_invalidated; }
    bool overflowed() const { return m_overflowed; }

private:
    void enqueue(Kind kind, const QNetworkCookie &cookie, quint64 sourceEpoch);
    void invalidate(bool overflow);

    QObject m_context;
    QPointer<QQuickWebEngineProfile> m_profile;
    QPointer<QWebEngineCookieStore> m_store;
    QMetaObject::Connection m_added;
    QMetaObject::Connection m_removed;
    QMetaObject::Connection m_profileDestroyed;
    QMetaObject::Connection m_storeDestroyed;
    std::deque<Observation> m_queue;
    quint64 m_epoch = 0;
    quint64 m_sequence = 0;
    bool m_attached = false;
    bool m_invalidated = false;
    bool m_overflowed = false;
};

#endif
