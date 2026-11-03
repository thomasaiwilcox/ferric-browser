#pragma once
#include <QObject>
#include <QFile>
#include <QProcess>
#include <QSslServer>
#include <QSslSocket>
#include <QSslKey>
#include <QSslCertificate>
#include <QSslConfiguration>
#include <QTcpSocket>
#include <QHostAddress>
#include <QTemporaryDir>
#include <QUrl>

class FixtureServer final : public QObject {
public:
  bool start(const FixtureServer *reuse = nullptr) {
    if (!temp_.isValid())
      return false;
    const auto keyPath = temp_.filePath(QStringLiteral("key.pem"));
    const auto certPath = temp_.filePath(QStringLiteral("cert.pem"));
    auto config = reuse ? reuse->server_.sslConfiguration() : QSslConfiguration::defaultConfiguration();
    if (!reuse) {
    QProcess openssl;
    openssl.start(QStringLiteral(FERRIC_TEST_OPENSSL),
                  {QStringLiteral("req"), QStringLiteral("-x509"),
                   QStringLiteral("-newkey"), QStringLiteral("rsa:2048"),
                   QStringLiteral("-nodes"), QStringLiteral("-days"),
                   QStringLiteral("1"), QStringLiteral("-subj"),
                   QStringLiteral("/CN=localhost"), QStringLiteral("-addext"),
                   QStringLiteral("subjectAltName=DNS:localhost"),
                   QStringLiteral("-keyout"), keyPath, QStringLiteral("-out"),
                   certPath});
    if (!openssl.waitForFinished(10000) || openssl.exitCode() != 0)
      return false;
    QFile key(keyPath), cert(certPath);
    if (!key.open(QIODevice::ReadOnly) || !cert.open(QIODevice::ReadOnly))
      return false;
    config.setLocalCertificate(QSslCertificate(cert.readAll()));
    config.setPrivateKey(QSslKey(key.readAll(), QSsl::Rsa));
    certificateFile_ = certPath;
    } else certificateFile_ = reuse->certificateFile_;
    config.setPeerVerifyMode(QSslSocket::VerifyNone);
    server_.setSslConfiguration(config);
    connect(&server_, &QTcpServer::pendingConnectionAvailable, this, [this] {
      while (auto *socket = server_.nextPendingConnection()) {
        const auto serveRequest = [this, socket] {
          if (socket->property("served").toBool()) return;
          auto bytes =
              socket->property("request").toByteArray() + socket->readAll();
          socket->setProperty("request", bytes);
          if (!bytes.contains("\r\n\r\n"))
            return;
          socket->setProperty("served", true);
          requests_.append(bytes.split(' ').value(0) + " " + bytes.split(' ').value(1));
          const auto path = bytes.split(' ').value(1);
          const auto body = path == "/frame" ? frame_ : page_;
          socket->write(
              "HTTP/1.1 " + QByteArray::number(httpStatus_) + " Fixture\r\nContent-Type: text/html; "
              "charset=utf-8\r\nConnection: close\r\n" + credentialPolicy_ + "Content-Length: " +
              QByteArray::number(body.size()) + "\r\n\r\n" + body);
          socket->disconnectFromHost();
        };
        connect(socket, &QTcpSocket::readyRead, socket, serveRequest);
        connect(socket, &QTcpSocket::disconnected, socket, &QObject::deleteLater);
        if (socket->bytesAvailable() > 0)
          serveRequest();
      }
    });
    return server_.listen(QHostAddress::LocalHost);
  }
  int requestCount(const QByteArray &method, const QByteArray &path) const {
    return requests_.count(method + " " + path);
  }
  QString certificateFile() const { return certificateFile_; }
  void setHttpStatus(int status) { httpStatus_ = status; }
  void setDocuments(QByteArray page, QByteArray frame) {
    page_ = std::move(page);
    frame_ = std::move(frame);
  }
  void setCredentialPolicyAllowed(bool allowed) {
    credentialPolicy_ = allowed ? QByteArray() : QByteArray(
        "Permissions-Policy: publickey-credentials-get=(), publickey-credentials-create=()\r\n");
  }
  QUrl pageUrl() const {
    return QUrl(
        QStringLiteral("https://localhost:%1/page").arg(server_.serverPort()));
  }

private:
  QTemporaryDir temp_;
  QString certificateFile_;
  int httpStatus_ = 200;
  QSslServer server_;
  QList<QByteArray> requests_;
  QByteArray page_;
  QByteArray frame_;
  QByteArray credentialPolicy_;
};
