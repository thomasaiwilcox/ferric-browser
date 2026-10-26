#include "public_profile_owner.h"

#include "public_page_owner.h"
#include "request_interceptor.h"

#include <QList>
#include <QPointer>
#include <QThread>
#include <QtWebEngineCore/QWebEngineProfile>

#include <atomic>
#include <utility>

namespace {
std::atomic<quint64> nextProfileGeneration{1};
}

FerricPublicProfileOwner::FerricPublicProfileOwner(
    FerricBrowserRequestInterceptor *interceptor, QObject *parent)
    : QObject(parent), interceptor_(interceptor),
      generation_(
          nextProfileGeneration.fetch_add(1, std::memory_order_relaxed)) {
  if (interceptor == nullptr || interceptor->thread() != thread()) {
    return;
  }

  auto *profile = new QWebEngineProfile(this);
  if (!interceptor->attach(profile)) {
    delete profile;
    return;
  }
  profile_ = profile;

  connect(interceptor, &QObject::destroyed, this, [this] { shutdown(); });
  connect(profile, &QObject::destroyed, this, [this] {
    profile_ = nullptr;
    shutdown();
  });
}

FerricPublicProfileOwner::~FerricPublicProfileOwner() { shutdown(); }

bool FerricPublicProfileOwner::isReady() const {
  return profile_ && interceptor_ && !shuttingDown_;
}

FerricPublicPageOwner *FerricPublicProfileOwner::createPage(QObject *host) {
  if (!isReady() || host == nullptr || host->thread() != thread() ||
      QThread::currentThread() != thread()) {
    return nullptr;
  }

  auto *page = new FerricPublicPageOwner(this, profile_, host, host);
  if (page->page() == nullptr) {
    delete page;
    return nullptr;
  }
  pages_.insert(page);
  connect(page, &QObject::destroyed, this,
          [this, page] { pageOwnerDestroyed(page); });
  emit pageCountChanged();
  return page;
}

bool FerricPublicProfileOwner::shutdown() {
  if (shuttingDown_) {
    return pages_.isEmpty() && profile_.isNull();
  }
  shuttingDown_ = true;

  QList<QPointer<FerricPublicPageOwner>> pages;
  pages.reserve(pages_.size());
  for (FerricPublicPageOwner *page : std::as_const(pages_)) {
    pages.append(page);
  }
  for (const auto &page : std::as_const(pages)) {
    if (page) {
      page->close();
    }
  }

  if (!pages_.isEmpty()) {
    return false;
  }
  if (profile_ != nullptr) {
    if (interceptor_ != nullptr) {
      interceptor_->detach(profile_);
    }
    QWebEngineProfile *const profile = profile_.data();
    profile_ = nullptr;
    delete profile;
  }
  if (!drainedEmitted_) {
    drainedEmitted_ = true;
    emit drained();
  }
  return true;
}

void FerricPublicProfileOwner::pageClosed(FerricPublicPageOwner *page) {
  if (pages_.remove(page) > 0) {
    emit pageCountChanged();
  }
}

void FerricPublicProfileOwner::pageOwnerDestroyed(FerricPublicPageOwner *page) {
  if (pages_.remove(page) > 0) {
    emit pageCountChanged();
  }
}
