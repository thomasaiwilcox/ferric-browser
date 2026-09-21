#include "blocklist_updater.h"

#include <QtCore/QDir>
#include <QtCore/QFile>
#include <QtCore/QCryptographicHash>
#include <QtCore/QTimer>
#include <QtCore/QUrl>
#include <QtNetwork/QNetworkRequest>

namespace {
constexpr qsizetype kMaximumBodyBytes = 32 * 1024 * 1024;
constexpr qsizetype kMaximumTotalBodyBytes = 128 * 1024 * 1024;
constexpr qsizetype kMaximumMetadataBytes = 8 * 1024;
}

RustBrowserBlocklistUpdater::RustBrowserBlocklistUpdater(QObject *parent) : QObject(parent)
{
    status_ = QStringLiteral("Idle");
}

bool RustBrowserBlocklistUpdater::busy() const
{
    return busy_;
}

QString RustBrowserBlocklistUpdater::status() const
{
    return status_;
}

bool RustBrowserBlocklistUpdater::update(const QStringList &listIds, const QString &storageBasePath)
{
    if (busy_) {
        return false;
    }
    QStringList unique;
    for (const auto &listId : listIds) {
        if (unique.contains(listId)) {
            continue;
        }
        if (unique.size() >= 64) {
            break;
        }
        const auto source = sourceFor(listId);
        if (!source.isValid()) {
            continue;
        }
        unique.append(listId);
    }
    if (unique.isEmpty()) {
        setStatus(QStringLiteral("No supported blocklists configured"));
        emit updateFinished(false, status_);
        return false;
    }
    storageBasePath_ = storageBasePath;
    failed_.clear();
    remaining_ = unique.size();
    busy_ = true;
    emit busyChanged();
    setStatus(QStringLiteral("Updating blocklists…"));

    for (const auto &listId : unique) {
        const auto url = sourceFor(listId);
        const auto cacheId = cacheIdFor(listId);
        QNetworkRequest request(url);
        request.setAttribute(QNetworkRequest::RedirectPolicyAttribute,
                             QNetworkRequest::NoLessSafeRedirectPolicy);
        const auto metadataPath = QDir(storageBasePath_)
                                       .filePath(QStringLiteral("cache/blocklists/%1.meta").arg(cacheId));
        QFile metadata(metadataPath);
        if (metadata.open(QIODevice::ReadOnly) && metadata.size() <= kMaximumMetadataBytes) {
            const auto lines = QString::fromUtf8(metadata.readAll()).split(u'\n');
            if (!lines.value(0).trimmed().isEmpty()) {
                request.setRawHeader("If-None-Match", lines.value(0).trimmed().toUtf8());
            }
            if (!lines.value(1).trimmed().isEmpty()) {
                request.setRawHeader("If-Modified-Since", lines.value(1).trimmed().toUtf8());
            }
        }
        auto *reply = manager_.get(request);
        pending_.insert(reply, Pending{listId, cacheId, {}});
        connect(reply, &QNetworkReply::readyRead, this, [this, reply]() {
            auto iterator = pending_.find(reply);
            if (iterator == pending_.end()) {
                return;
            }
            iterator->body.append(reply->readAll());
            if (iterator->body.size() > kMaximumBodyBytes
                || totalBodyBytes() > kMaximumTotalBodyBytes) {
                reply->abort();
            }
        });
        connect(reply, &QNetworkReply::finished, this, [this, reply]() { finishOne(reply); });
        QTimer::singleShot(30'000, this, [this, reply]() {
            if (pending_.contains(reply)) {
                reply->abort();
            }
        });
    }
    return true;
}

QUrl RustBrowserBlocklistUpdater::sourceFor(const QString &source)
{
    if (source == QStringLiteral("easylist")) {
        return QUrl(QStringLiteral("https://easylist.to/easylist/easylist.txt"));
    }
    if (source == QStringLiteral("easyprivacy")) {
        return QUrl(QStringLiteral("https://easylist.to/easylist/easyprivacy.txt"));
    }
    if (!validListId(source) && source.size() <= 2048) {
        const QUrl url(source);
        if (url.isValid() && url.scheme().compare(QStringLiteral("https"), Qt::CaseInsensitive) == 0
            && !url.host().isEmpty() && url.userInfo().isEmpty() && url.fragment().isEmpty()
            && source.toUtf8().size() <= 2048
            && url.toString(QUrl::FullyEncoded) == source) {
            return url;
        }
    }
    return {};
}

QString RustBrowserBlocklistUpdater::cacheIdFor(const QString &source)
{
    if (validListId(source)) {
        return source;
    }
    const auto encoded = sourceFor(source).toString(QUrl::FullyEncoded).toUtf8();
    const auto digest = QCryptographicHash::hash(encoded, QCryptographicHash::Sha256).toHex();
    return QStringLiteral("url-") + QString::fromLatin1(digest.left(60));
}

bool RustBrowserBlocklistUpdater::validListId(const QString &listId)
{
    if (listId.isEmpty() || listId.size() > 64) {
        return false;
    }
    for (const auto character : listId) {
        if (!((character >= u'a' && character <= u'z')
              || (character >= u'0' && character <= u'9') || character == u'-')) {
            return false;
        }
    }
    return true;
}

void RustBrowserBlocklistUpdater::finishOne(QNetworkReply *reply)
{
    const auto iterator = pending_.find(reply);
    if (iterator == pending_.end()) {
        reply->deleteLater();
        return;
    }
    const auto pending = iterator.value();
    const auto source = pending.source;
    const auto cacheId = pending.cacheId;
    auto body = pending.body;
    body.append(reply->readAll());
    const auto statusCode = reply->attribute(QNetworkRequest::HttpStatusCodeAttribute).toInt();
    const auto finalUrl = reply->url();
    const auto expectedUrl = sourceFor(source);
    const bool validTransport = finalUrl.scheme() == QStringLiteral("https")
        && finalUrl.host().compare(expectedUrl.host(), Qt::CaseInsensitive) == 0
        && (statusCode == 0 || (statusCode >= 200 && statusCode < 300) || statusCode == 304)
        && body.size() <= kMaximumBodyBytes;
    if (reply->error() != QNetworkReply::NoError || !validTransport) {
        failed_.append(cacheId);
    } else if (statusCode != 304 && body.isEmpty()) {
        failed_.append(cacheId);
    } else if (statusCode == 304) {
        setStatus(QStringLiteral("Blocklist unchanged: %1").arg(cacheId));
    } else {
        const auto etag = QString::fromUtf8(reply->rawHeader("ETag"));
        const auto lastModified = QString::fromUtf8(reply->rawHeader("Last-Modified"));
        emit listReady(cacheId, QString::fromUtf8(body), etag, lastModified);
    }
    pending_.erase(iterator);
    reply->deleteLater();
    --remaining_;
    if (remaining_ == 0) {
        finishUpdate();
    }
}

qsizetype RustBrowserBlocklistUpdater::totalBodyBytes() const
{
    qsizetype total = 0;
    for (const auto &pending : pending_) {
        total += pending.body.size();
    }
    return total;
}

void RustBrowserBlocklistUpdater::setStatus(const QString &status)
{
    if (status_ == status) {
        return;
    }
    status_ = status;
    emit statusChanged();
}

void RustBrowserBlocklistUpdater::finishUpdate()
{
    busy_ = false;
    emit busyChanged();
    if (failed_.isEmpty()) {
        setStatus(QStringLiteral("Blocklist update complete"));
        emit updateFinished(true, status_);
    } else {
        setStatus(QStringLiteral("Blocklist update incomplete; last-known-good lists retained"));
        emit updateFinished(false, status_);
    }
}
