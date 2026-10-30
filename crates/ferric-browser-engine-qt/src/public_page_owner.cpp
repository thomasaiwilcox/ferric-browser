#include "public_page_owner.h"

#include "public_profile_owner.h"

#include <QMetaObject>
#include <QThread>
#include <QtWebEngineCore/QWebEngineNewWindowRequest>
#include <QtWebEngineCore/QWebEnginePage>
#include <QtWebEngineCore/QWebEngineProfile>

#include <atomic>
#include <limits>
#include <utility>

namespace {
std::atomic<quint64> nextPageGeneration{1};
std::atomic<quint64> nextHostGeneration{1};
} // namespace

FerricPublicPageOwner::FerricPublicPageOwner(
    FerricPublicProfileOwner *profileOwner, QWebEngineProfile *profile,
    QObject *host, QObject *parent)
    : QObject(parent), profileOwner_(profileOwner), profile_(profile),
      host_(host),
      profileGeneration_(profileOwner == nullptr ? 0
                                                 : profileOwner->generation()),
      pageGeneration_(
          nextPageGeneration.fetch_add(1, std::memory_order_relaxed)),
      hostGeneration_(
          nextHostGeneration.fetch_add(1, std::memory_order_relaxed)) {
  if (profileOwner_ == nullptr || profile_ == nullptr || host_ == nullptr ||
      QThread::currentThread() != thread() || host->thread() != thread()) {
    return;
  }

  page_ = new QWebEnginePage(profile_, this);
  connect(page_, &QWebEnginePage::loadStarted, this, &FerricPublicPageOwner::invalidateDocument);
  connect(page_, &QWebEnginePage::renderProcessTerminated, this,
          [this] { invalidateDocument(); });
  connect(page_, &QWebEnginePage::lifecycleStateChanged, this,
          [this](QWebEnginePage::LifecycleState state) {
            if (state == QWebEnginePage::LifecycleState::Discarded) invalidateDocument();
          });
  hostDestroyed_ =
      connect(host, &QObject::destroyed, this, [this] { close(); });
}

FerricPublicPageOwner::~FerricPublicPageOwner() {
  close();
  if (profileOwner_ != nullptr) {
    profileOwner_->pageOwnerDestroyed(this);
  }
}

FerricPublicPageOwner::Operation
FerricPublicPageOwner::captureOperation() const {
  return {profileGeneration_, pageGeneration_, hostGeneration_,
          documentGeneration_, presentationGeneration_};
}

bool FerricPublicPageOwner::isCurrent(const Operation &operation) const {
  return !closed_ && page_ && profile_ && host_ && profileOwner_ &&
         profileOwner_->isReady() && QThread::currentThread() == thread() &&
         operation.profileGeneration == profileGeneration_ &&
         operation.profileGeneration == profileOwner_->generation() &&
         operation.pageGeneration == pageGeneration_ &&
         operation.hostGeneration == hostGeneration_ &&
         operation.documentGeneration == documentGeneration_ &&
         operation.presentationGeneration == presentationGeneration_;
}

bool FerricPublicPageOwner::runJavaScript(const QString &script,
                                          JavaScriptCallback callback) {
  if (!page_ || closed_ || !callback || QThread::currentThread() != thread()) {
    return false;
  }
  const Operation operation = captureOperation();
  QPointer<FerricPublicPageOwner> guard(this);
  page_->runJavaScript(script, [guard, operation,
                                callback = std::move(callback)](
                                   const QVariant &result) mutable {
    if (!guard || !guard->isCurrent(operation)) {
      return;
    }
    guard->queueIfCurrent(operation, [callback = std::move(callback),
                                      result]() mutable { callback(result); });
  });
  return true;
}

bool FerricPublicPageOwner::queueIfCurrent(const Operation &operation,
                                           std::function<void()> callback) {
  if (!isCurrent(operation) || !callback) {
    return false;
  }
  QPointer<FerricPublicPageOwner> guard(this);
  return QMetaObject::invokeMethod(
      this,
      [guard, operation, callback = std::move(callback)]() mutable {
        if (guard && guard->isCurrent(operation)) {
          callback();
        }
      },
      Qt::QueuedConnection);
}

bool FerricPublicPageOwner::adoptNewWindowFrom(
    FerricPublicPageOwner *source, QWebEngineNewWindowRequest &request) {
  if (closed_ || popupAdopted_ || page_ == nullptr || source == nullptr ||
      source == this || !source->isCurrent(source->captureOperation()) ||
      !profile_ || source->profile_ != profile_ ||
      source->profileOwner_ != profileOwner_ || page_->isLoading() ||
      documentGeneration_ != 0 || QThread::currentThread() != thread()) {
    return false;
  }
  popupAdopted_ = true;
  page_->acceptAsNewWindow(request);
  return true;
}

bool FerricPublicPageOwner::close() {
  if (closed_) {
    return false;
  }
  closed_ = true;
  ++documentGeneration_;
  emit authorityInvalidated();
  QObject::disconnect(hostDestroyed_);

  QWebEnginePage *const page = page_.data();
  page_ = nullptr;
  if (page != nullptr) {
    page->triggerAction(QWebEnginePage::Stop);
    delete page;
  }

  QPointer<FerricPublicPageOwner> guard(this);
  if (profileOwner_ != nullptr) {
    profileOwner_->pageClosed(this);
  }
  if (guard) {
    emit closed();
  }
  return true;
}

void FerricPublicPageOwner::invalidateDocument() {
  if (closed_) return;
  if (documentGeneration_ == std::numeric_limits<quint64>::max()) { close(); return; }
  ++documentGeneration_;
  emit documentChanged(documentGeneration_);
  emit authorityInvalidated();
}

void FerricPublicPageOwner::invalidatePresentation() {
  if (closed_) return;
  if (presentationGeneration_ == std::numeric_limits<quint64>::max()) { close(); return; }
  ++presentationGeneration_;
  emit authorityInvalidated();
}
