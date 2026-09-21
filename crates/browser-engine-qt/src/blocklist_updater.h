#ifndef RUSTBROWSER_BLOCKLIST_UPDATER_H
#define RUSTBROWSER_BLOCKLIST_UPDATER_H

#include <QtCore/QByteArray>
#include <QtCore/QHash>
#include <QtCore/QObject>
#include <QtCore/QString>
#include <QtCore/QStringList>
#include <QtCore/QUrl>
#include <QtNetwork/QNetworkAccessManager>
#include <QtNetwork/QNetworkReply>
#include <QtQml/qqmlregistration.h>

class RustBrowserBlocklistUpdater : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(BlocklistUpdater)
    Q_PROPERTY(bool busy READ busy NOTIFY busyChanged)
    Q_PROPERTY(QString status READ status NOTIFY statusChanged)

public:
    explicit RustBrowserBlocklistUpdater(QObject *parent = nullptr);

    bool busy() const;
    QString status() const;

    Q_INVOKABLE bool update(const QStringList &listIds, const QString &storageBasePath);

signals:
    void busyChanged();
    void statusChanged();
    void listReady(const QString &listId, const QString &content, const QString &etag,
                   const QString &lastModified);
    void updateFinished(bool success, const QString &message);

private:
    struct Pending {
        QString source;
        QString cacheId;
        QByteArray body;
    };

    static QUrl sourceFor(const QString &source);
    static QString cacheIdFor(const QString &source);
    static bool validListId(const QString &listId);
    qsizetype totalBodyBytes() const;
    void finishOne(QNetworkReply *reply);
    void setStatus(const QString &status);
    void finishUpdate();

    QNetworkAccessManager manager_;
    QHash<QNetworkReply *, Pending> pending_;
    QString storageBasePath_;
    QStringList failed_;
    int remaining_ = 0;
    bool busy_ = false;
    QString status_;
};

#endif
