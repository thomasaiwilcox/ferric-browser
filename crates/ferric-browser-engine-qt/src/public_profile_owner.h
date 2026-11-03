#pragma once

#include <QObject>
#include <QPointer>
#include <QSet>
#include <QString>
#include <QtWebEngineCore/QWebEngineProfile>

class FerricBrowserRequestInterceptor;
class FerricPublicPageOwner;

// Owns one real WebEngine profile and keeps its request policy attached until
// all page owners have been synchronously stopped and destroyed.
class FerricPublicProfileOwner final : public QObject {
  Q_OBJECT

public:
  struct Configuration {
    // Empty names are exclusively in-memory. Persistent storage is supplied
    // by the existing profile application policy, before any page is created.
    QString storageName;
    QString persistentStoragePath;
    QString cachePath;
  };
  explicit FerricPublicProfileOwner(
      FerricBrowserRequestInterceptor *interceptor, QObject *parent = nullptr);
  FerricPublicProfileOwner(FerricBrowserRequestInterceptor *interceptor,
                           const Configuration &configuration,
                           QObject *parent = nullptr);
  ~FerricPublicProfileOwner() override;

  QWebEngineProfile *profile() const { return profile_; }
  bool isReady() const;
  bool isShuttingDown() const { return shuttingDown_; }
  int pageCount() const { return pages_.size(); }
  quint64 generation() const { return generation_; }
  FerricPublicPageOwner *createPage(QObject *host);
  bool shutdown();

signals:
  void pageCountChanged();
  void drained();

private:
  friend class FerricPublicPageOwner;
  void pageClosed(FerricPublicPageOwner *page);
  void pageOwnerDestroyed(FerricPublicPageOwner *page);

  QPointer<QWebEngineProfile> profile_;
  QPointer<FerricBrowserRequestInterceptor> interceptor_;
  QSet<FerricPublicPageOwner *> pages_;
  quint64 generation_ = 0;
  bool shuttingDown_ = false;
  bool drainedEmitted_ = false;
};
