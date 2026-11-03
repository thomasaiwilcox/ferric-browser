#pragma once

#include <QMetaObject>
#include <QObject>
#include <QPointer>
#include <QString>
#include <QVariant>
#include <QtWebEngineCore/QWebEnginePage>
#include <QtWebEngineCore/QWebEngineProfile>

#include <functional>

class FerricPublicProfileOwner;
class QWebEngineNewWindowRequest;

class FerricPublicPageOwner final : public QObject {
  Q_OBJECT

public:
  struct Operation {
    quint64 profileGeneration = 0;
    quint64 pageGeneration = 0;
    quint64 hostGeneration = 0;
    quint64 documentGeneration = 0;
    quint64 presentationGeneration = 0;
  };
  using JavaScriptCallback = std::function<void(const QVariant &)>;

  ~FerricPublicPageOwner() override;

  QWebEnginePage *page() const { return page_; }
  QWebEngineProfile *profile() const { return profile_; }
  bool isClosed() const { return closed_; }
  quint64 hostGeneration() const { return hostGeneration_; }
  quint64 presentationGeneration() const { return presentationGeneration_; }
  quint64 pageGeneration() const { return pageGeneration_; }
  quint64 documentGeneration() const { return documentGeneration_; }

  Operation captureOperation() const;
  bool isCurrent(const Operation &operation) const;
  bool runJavaScript(const QString &script, JavaScriptCallback callback);
  bool queueIfCurrent(const Operation &operation,
                      std::function<void()> callback);
  bool adoptNewWindowFrom(FerricPublicPageOwner *source,
                          QWebEngineNewWindowRequest &request);
  bool close();
  void invalidatePresentation();

signals:
  void documentChanged(quint64 generation);
  void closed();
  void authorityInvalidated();

private:
  friend class FerricPublicProfileOwner;
  FerricPublicPageOwner(FerricPublicProfileOwner *profileOwner,
                        QWebEngineProfile *profile, QObject *host,
                        QObject *parent);

  void invalidateDocument();
  QPointer<FerricPublicProfileOwner> profileOwner_;
  QPointer<QWebEngineProfile> profile_;
  QPointer<QObject> host_;
  QPointer<QWebEnginePage> page_;
  QMetaObject::Connection hostDestroyed_;
  quint64 profileGeneration_ = 0;
  quint64 pageGeneration_ = 0;
  quint64 hostGeneration_ = 0;
  quint64 documentGeneration_ = 0;
  quint64 presentationGeneration_ = 1;
  bool closed_ = false;
  bool popupAdopted_ = false;
};
