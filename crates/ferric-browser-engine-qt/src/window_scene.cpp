#include "window_scene.h"

#include <QCloseEvent>
#include <QCoreApplication>
#include <QQmlEngine>
#include <QQuickItem>
#include <QQuickWindow>
#include <QResizeEvent>
#include <QSet>

namespace {
QSet<QQuickWindow *> &presentedWindows()
{
    static QSet<QQuickWindow *> windows;
    return windows;
}
}

FerricWindowScene::FerricWindowScene(QQuickWindow *logicalWindow, QQmlEngine *engine,
                                     QWidget *host)
    : QQuickWidget(engine, host), host_(host)
{
    if (!logicalWindow || !engine || !host || logicalWindow->isVisible()
        || qmlEngine(logicalWindow) != engine || presentedWindows().contains(logicalWindow)) {
        return;
    }
    logicalWindow_ = logicalWindow;
    sourceRoot_ = logicalWindow->contentItem();
    presentedWindows().insert(logicalWindow);
    sceneRoot_ = new QQuickItem;
    sceneRoot_->setObjectName(QStringLiteral("ferricWindowSceneRoot"));
    setResizeMode(SizeRootObjectToView);
    setFocusPolicy(Qt::StrongFocus);
    setContent(QUrl(), nullptr, sceneRoot_);
    adoptChildren();
    childrenChanged_ = connect(sourceRoot_, &QQuickItem::childrenChanged,
                                this, &FerricWindowScene::adoptChildren);
    connect(logicalWindow, &QObject::destroyed, this, [this, logicalWindow] {
        presentedWindows().remove(logicalWindow);
        logicalWindow_.clear();
        sourceRoot_.clear();
        setEnabled(false);
    });
    connect(quickWindow(), &QQuickWindow::activeFocusItemChanged,
            this, &FerricWindowScene::activeFocusItemChanged);
    titleChanged_ = connect(logicalWindow, &QWindow::windowTitleChanged, host, &QWidget::setWindowTitle);
    host->setWindowTitle(logicalWindow->title());
    host->installEventFilter(this);
    resize(logicalWindow->size());
}

FerricWindowScene::~FerricWindowScene()
{
    release();
}

bool FerricWindowScene::isReady() const
{
    // setContent() installs an existing item without a QQmlComponent. Qt's
    // source-loading status stays Null in that supported form.
    return logicalWindow_ && sourceRoot_ && sceneRoot_ && rootObject() == sceneRoot_
        && errors().isEmpty();
}

bool FerricWindowScene::isHostActive() const
{
    return host_ && host_->isActiveWindow();
}

QQuickItem *FerricWindowScene::activeFocusItem() const
{
    return quickWindow()->activeFocusItem();
}

QWindow *FerricWindowScene::nativeWindow() const
{
    return host_ ? host_->windowHandle() : nullptr;
}

void FerricWindowScene::adoptChildren()
{
    if (moving_ || !sourceRoot_ || !sceneRoot_) return;
    moving_ = true;
    const auto children = sourceRoot_->childItems();
    for (auto *child : children) child->setParentItem(sceneRoot_);
    moving_ = false;
}

void FerricWindowScene::release()
{
    disconnect(childrenChanged_);
    disconnect(titleChanged_);
    if (host_) host_->removeEventFilter(this);
    if (sourceRoot_ && sceneRoot_) {
        const auto children = sceneRoot_->childItems();
        for (auto *child : children) child->setParentItem(sourceRoot_);
    }
    if (logicalWindow_) presentedWindows().remove(logicalWindow_);
    logicalWindow_.clear();
    sourceRoot_.clear();
}

void FerricWindowScene::requestClose()
{
    if (host_) host_->close();
}

bool FerricWindowScene::eventFilter(QObject *watched, QEvent *event)
{
    if (watched != host_ || !logicalWindow_) return false;
    switch (event->type()) {
    case QEvent::Close: {
        if (closing_) {
            event->ignore();
            return true;
        }
        closing_ = true;
        QCloseEvent close;
        QCoreApplication::sendEvent(logicalWindow_, &close);
        closing_ = false;
        if (!close.isAccepted()) {
            event->ignore();
            return true;
        }
        break;
    }
    case QEvent::WindowActivate:
    case QEvent::WindowDeactivate:
        emit hostActiveChanged();
        break;
    case QEvent::Show:
    case QEvent::WinIdChange:
        emit nativeWindowChanged();
        break;
    default:
        break;
    }
    return false;
}

void FerricWindowScene::resizeEvent(QResizeEvent *event)
{
    QQuickWidget::resizeEvent(event);
    if (logicalWindow_) logicalWindow_->resize(event->size());
}
