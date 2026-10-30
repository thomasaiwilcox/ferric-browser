#include "page_window_identity.h"

#include <KWaylandExtras>
#include <QGuiApplication>
#include <QThread>
#include <QUuid>

FerricPageWindowIdentity::FerricPageWindowIdentity(QObject *parent)
    : QObject(parent), tag_(QStringLiteral("ferric-page-")
        + QUuid::createUuid().toString(QUuid::WithoutBraces)) {}

QWindow *FerricPageWindowIdentity::window() const { return window_; }
QString FerricPageWindowIdentity::tag() const { return tag_; }
bool FerricPageWindowIdentity::prepared() const { return prepared_ && window_; }

void FerricPageWindowIdentity::setWindow(QWindow *window)
{
    // A marker is never reassigned to a replacement window, and an already
    // mapped surface cannot acquire retrospective grouping authority.
    if (window_ || prepared_ || !window || window->isVisible()
        || window->thread() != QThread::currentThread()) return;
    window_ = window;
    if (QGuiApplication::platformName() == QStringLiteral("wayland"))
        KWaylandExtras::setXdgToplevelTag(window, tag_);
    prepared_ = true;
    connect(window, &QObject::destroyed, this, [this] {
        window_.clear();
        emit windowChanged();
        emit preparedChanged();
    });
    emit windowChanged();
    emit preparedChanged();
}
