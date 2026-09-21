#ifndef FERRIC_BROWSER_LINK_RULE_UPDATER_H
#define FERRIC_BROWSER_LINK_RULE_UPDATER_H

#include <QtCore/QByteArray>
#include <QtCore/QObject>
#include <QtCore/QString>
#include <QtCore/QUrl>
#include <QtNetwork/QNetworkAccessManager>
#include <QtNetwork/QNetworkReply>
#include <QtQml/qqmlregistration.h>

class FerricBrowserLinkRuleUpdater : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(LinkRuleUpdater)
    Q_PROPERTY(bool busy READ busy NOTIFY busyChanged)
    Q_PROPERTY(QString status READ status NOTIFY statusChanged)

public:
    explicit FerricBrowserLinkRuleUpdater(QObject *parent = nullptr);

    bool busy() const;
    QString status() const;

    Q_INVOKABLE bool update(const QString &source, const QString &expectedChecksum,
                            const QString &storageBasePath);

signals:
    void busyChanged();
    void statusChanged();
    void ruleReady(const QString &content, const QString &expectedChecksum,
                   const QString &etag, const QString &lastModified);
    void updateFinished(bool success, const QString &message);

private:
    static QUrl sourceFor(const QString &source);
    static bool validChecksum(const QString &checksum);
    void finish(QNetworkReply *reply);
    void setStatus(const QString &status);

    QNetworkAccessManager manager_;
    QNetworkReply *reply_ = nullptr;
    QByteArray body_;
    QString source_;
    QString expectedChecksum_;
    QString storageBasePath_;
    QString status_;
    bool busy_ = false;
};

#endif
