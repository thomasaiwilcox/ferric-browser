#include "link_rule_updater.h"

#include <QtCore/QDir>
#include <QtCore/QFile>
#include <QtCore/QTimer>
#include <QtNetwork/QNetworkRequest>

namespace {
constexpr qsizetype kMaximumBodyBytes = 1024 * 1024;
constexpr qsizetype kMaximumMetadataBytes = 8 * 1024;
}

RustBrowserLinkRuleUpdater::RustBrowserLinkRuleUpdater(QObject *parent) : QObject(parent)
{
    status_ = QStringLiteral("Idle");
}

bool RustBrowserLinkRuleUpdater::busy() const
{
    return busy_;
}

QString RustBrowserLinkRuleUpdater::status() const
{
    return status_;
}

bool RustBrowserLinkRuleUpdater::update(const QString &source, const QString &expectedChecksum,
                                        const QString &storageBasePath)
{
    if (busy_) {
        return false;
    }
    const auto url = sourceFor(source);
    if (!url.isValid() || !validChecksum(expectedChecksum) || storageBasePath.isEmpty()) {
        setStatus(QStringLiteral("Clean-link update source or checksum is invalid"));
        emit updateFinished(false, status_);
        return false;
    }
    source_ = source;
    expectedChecksum_ = expectedChecksum.toLower();
    storageBasePath_ = storageBasePath;
    body_.clear();
    busy_ = true;
    emit busyChanged();
    setStatus(QStringLiteral("Updating clean-link rules…"));

    QNetworkRequest request(url);
    request.setAttribute(QNetworkRequest::RedirectPolicyAttribute,
                         QNetworkRequest::NoLessSafeRedirectPolicy);
    const auto metadataPath = QDir(storageBasePath_)
                                  .filePath(QStringLiteral("cache/link-cleaning/accepted.meta"));
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
    reply_ = manager_.get(request);
    connect(reply_, &QNetworkReply::readyRead, this, [this]() {
        if (!reply_) {
            return;
        }
        body_.append(reply_->readAll());
        if (body_.size() > kMaximumBodyBytes) {
            reply_->abort();
        }
    });
    connect(reply_, &QNetworkReply::finished, this, [this]() {
        if (reply_) {
            finish(reply_);
        }
    });
    QTimer::singleShot(30'000, this, [this]() {
        if (reply_) {
            reply_->abort();
        }
    });
    return true;
}

QUrl RustBrowserLinkRuleUpdater::sourceFor(const QString &source)
{
    if (source.size() < 10 || source.size() > 2048 || source.contains(QChar::fromLatin1('@'))
        || source.contains(QChar::fromLatin1('#')) || source.contains(QChar::fromLatin1('?'))) {
        return {};
    }
    const QUrl url(source);
    if (!url.isValid() || url.scheme().compare(QStringLiteral("https"), Qt::CaseInsensitive) != 0
        || url.host().isEmpty() || !url.userInfo().isEmpty()
        || url.toString(QUrl::FullyEncoded) != source) {
        return {};
    }
    return url;
}

bool RustBrowserLinkRuleUpdater::validChecksum(const QString &checksum)
{
    if (checksum.size() != 64) {
        return false;
    }
    for (const auto character : checksum) {
        if (!((character >= u'a' && character <= u'f')
              || (character >= u'0' && character <= u'9'))) {
            return false;
        }
    }
    return true;
}

void RustBrowserLinkRuleUpdater::finish(QNetworkReply *reply)
{
    body_.append(reply->readAll());
    const auto statusCode = reply->attribute(QNetworkRequest::HttpStatusCodeAttribute).toInt();
    const auto finalUrl = reply->url();
    const auto expectedUrl = sourceFor(source_);
    const bool validTransport = finalUrl.scheme().compare(QStringLiteral("https"), Qt::CaseInsensitive) == 0
        && finalUrl.host().compare(expectedUrl.host(), Qt::CaseInsensitive) == 0
        && ((statusCode >= 200 && statusCode < 300 && !body_.isEmpty()) || statusCode == 304)
        && body_.size() <= kMaximumBodyBytes;
    const bool success = reply->error() == QNetworkReply::NoError && validTransport;
    if (success && statusCode == 304) {
        setStatus(QStringLiteral("Clean-link rules unchanged"));
    } else if (success) {
        const auto etag = QString::fromUtf8(reply->rawHeader("ETag"));
        const auto lastModified = QString::fromUtf8(reply->rawHeader("Last-Modified"));
        emit ruleReady(QString::fromUtf8(body_), expectedChecksum_, etag, lastModified);
        setStatus(QStringLiteral("Clean-link rule candidate downloaded"));
    } else {
        setStatus(QStringLiteral("Clean-link update failed; last accepted rules retained"));
    }
    reply->deleteLater();
    reply_ = nullptr;
    body_.clear();
    busy_ = false;
    emit busyChanged();
    emit updateFinished(success, status_);
}

void RustBrowserLinkRuleUpdater::setStatus(const QString &status)
{
    if (status_ == status) {
        return;
    }
    status_ = status;
    emit statusChanged();
}
