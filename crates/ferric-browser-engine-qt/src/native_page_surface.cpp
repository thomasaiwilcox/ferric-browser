#include "native_page_surface.h"
#include "script_runner.h"

#include <QQmlContext>
#include <QFocusEvent>
#include <QQmlEngine>
#include <QWebEngineFindTextResult>
#include <QWebEngineScriptCollection>
#include <QWebEngineSettings>
#include <QWebEngineView>
#include <cmath>

namespace {
constexpr std::array attributes{
    QWebEngineSettings::JavascriptEnabled, QWebEngineSettings::AutoLoadImages,
    QWebEngineSettings::ForceDarkMode, QWebEngineSettings::PlaybackRequiresUserGesture,
    QWebEngineSettings::JavascriptCanOpenWindows, QWebEngineSettings::FocusOnNavigationEnabled};

bool ordinaryWorld(int world)
{
    return world == QWebEngineScript::MainWorld || world == QWebEngineScript::ApplicationWorld;
}

QVariantMap descriptor(const QWebEngineScript &script)
{
    return {{"name", script.name()}, {"sourceCode", script.sourceCode()},
            {"injectionPoint", int(script.injectionPoint())}, {"worldId", script.worldId()},
            {"runsOnSubFrames", script.runsOnSubFrames()}};
}
}

FerricNativePageScripts::FerricNativePageScripts(FerricNativePageSurface *surface)
    : QObject(surface), surface_(surface) {}

QVariantList FerricNativePageScripts::find(const QString &name) const
{
    QVariantList result;
    if (auto *page = surface_->page()) {
        for (const auto &script : page->scripts().find(name)) {
            if (ordinaryWorld(script.worldId())) result.append(descriptor(script));
        }
    }
    return result;
}

bool FerricNativePageScripts::decode(const QJSValue &value, QWebEngineScript *script) const
{
    if (!value.isObject() || !value.property("name").isString()
        || !value.property("sourceCode").isString() || !value.property("worldId").isNumber()
        || !value.property("injectionPoint").isNumber()
        || !value.property("runsOnSubFrames").isBool()) return false;
    const auto world = value.property("worldId").toNumber();
    const auto point = value.property("injectionPoint").toNumber();
    if (!std::isfinite(world) || world < 0 || world > 1 || std::trunc(world) != world
        || !std::isfinite(point) || std::trunc(point) != point
        || point < QWebEngineScript::Deferred || point > QWebEngineScript::DocumentCreation) return false;
    const auto name = value.property("name").toString();
    const auto source = value.property("sourceCode").toString();
    if (name.isEmpty() || name.size() > 256 || source.size() > 1024 * 1024) return false;
    script->setName(name);
    script->setSourceCode(source);
    script->setWorldId(quint32(world));
    script->setInjectionPoint(QWebEngineScript::InjectionPoint(int(point)));
    script->setRunsOnSubFrames(value.property("runsOnSubFrames").toBool());
    return true;
}

bool FerricNativePageScripts::insert(const QJSValue &value)
{
    auto *page = surface_->page();
    QWebEngineScript script;
    if (!page || !decode(value, &script) || page->scripts().count() >= 256) return false;
    page->scripts().insert(script);
    return true;
}

bool FerricNativePageScripts::remove(const QJSValue &value)
{
    auto *page = surface_->page();
    QWebEngineScript script;
    return page && decode(value, &script) && page->scripts().remove(script);
}

void FerricNativePageScripts::clear()
{
    if (auto *page = surface_->page()) {
        for (const auto &script : page->scripts().toList()) {
            if (ordinaryWorld(script.worldId())) page->scripts().remove(script);
        }
    }
}

FerricNativePageSettings::FerricNativePageSettings(QObject *parent) : QObject(parent) {}

void FerricNativePageSettings::bind(QWebEnginePage *page)
{
    page_ = page;
    if (page_) {
        for (std::size_t i = 0; i < attributes.size(); ++i) {
            if (assigned_[i]) page_->settings()->setAttribute(attributes[i], values_[i]);
        }
    }
    emit changed();
}

bool FerricNativePageSettings::get(std::size_t index) const
{
    return page_ ? page_->settings()->testAttribute(attributes[index]) : values_[index];
}

void FerricNativePageSettings::set(std::size_t index, bool value)
{
    const bool previous = get(index);
    assigned_[index] = true;
    values_[index] = value;
    if (page_) page_->settings()->setAttribute(attributes[index], value);
    if (previous != value) emit changed();
}

FerricNativePageSurface::FerricNativePageSurface(QQuickItem *parent)
    : QQuickItem(parent), settings_(this), scripts_(this), runner_(new FerricScriptRunner(this)) {}

bool FerricNativePageSurface::bind(FerricPublicPageOwner *owner, QWebEngineView *view)
{
    if (bound_ || !owner || !view || owner->thread() != thread() || view->thread() != thread()
        || !owner->isCurrent(owner->captureOperation()) || view->page() != owner->page()) return false;
    bound_ = true;
    owner_ = owner;
    view_ = view;
    settings_.bind(page());
    auto *native = page();
    connect(native, &QWebEnginePage::urlChanged, this, &FerricNativePageSurface::urlChanged);
    connect(native, &QWebEnginePage::titleChanged, this, &FerricNativePageSurface::titleChanged);
    connect(native, &QWebEnginePage::audioMutedChanged, this, &FerricNativePageSurface::audioMutedChanged);
    connect(native, &QWebEnginePage::recentlyAudibleChanged, this, &FerricNativePageSurface::recentlyAudibleChanged);
    connect(native, &QWebEnginePage::zoomFactorChanged, this, &FerricNativePageSurface::zoomFactorChanged);
    connect(native, &QWebEnginePage::lifecycleStateChanged, this, &FerricNativePageSurface::lifecycleStateChanged);
    connect(native, &QWebEnginePage::renderProcessPidChanged, this, &FerricNativePageSurface::renderProcessPidChanged);
    connect(native, &QWebEnginePage::recommendedStateChanged, this, &FerricNativePageSurface::pageStateChanged);
    connect(native, &QWebEnginePage::loadingChanged, this, [this](const QWebEngineLoadingInfo &info) {
        emit pageStateChanged();
        emit loadingChanged(info);
    });
    connect(owner, &FerricPublicPageOwner::authorityInvalidated, this, [this] {
        runner_->cancelFor(this);
        findCallback_ = {};
        ++findSerial_;
        if (!page()) { settings_.bind(nullptr); setEnabled(false); }
        emit authorityInvalidated();
        emit pageStateChanged();
    });
    emit pageStateChanged();
    return true;
}

QWebEnginePage *FerricNativePageSurface::page() const
{
    if (!owner_ || !view_ || !owner_->isCurrent(owner_->captureOperation())) return nullptr;
    return view_->page() == owner_->page() ? owner_->page() : nullptr;
}

QWebEngineView *FerricNativePageSurface::pageView() const { return page() ? view_.data() : nullptr; }
QUrl FerricNativePageSurface::url() const { return page() ? page()->url() : QUrl(); }
void FerricNativePageSurface::setUrl(const QUrl &url) { if (page() && url.isValid() && !url.isRelative()) page()->setUrl(url); }
QString FerricNativePageSurface::title() const { return page() ? page()->title() : QString(); }
bool FerricNativePageSurface::loading() const { return page() && page()->isLoading(); }
bool FerricNativePageSurface::audioMuted() const { return page() && page()->isAudioMuted(); }
void FerricNativePageSurface::setAudioMuted(bool value) { if (page()) page()->setAudioMuted(value); }
bool FerricNativePageSurface::recentlyAudible() const { return page() && page()->recentlyAudible(); }
qreal FerricNativePageSurface::zoomFactor() const { return page() ? page()->zoomFactor() : 1.0; }
void FerricNativePageSurface::setZoomFactor(qreal zoom) { if (page() && std::isfinite(zoom) && zoom >= .25 && zoom <= 5) page()->setZoomFactor(zoom); }
int FerricNativePageSurface::lifecycleState() const { return page() ? int(page()->lifecycleState()) : int(QWebEnginePage::LifecycleState::Discarded); }
void FerricNativePageSurface::setLifecycleState(int state) { if (page() && state >= 0 && state <= 2) page()->setLifecycleState(QWebEnginePage::LifecycleState(state)); }
qint64 FerricNativePageSurface::renderProcessPid() const { return page() ? page()->renderProcessPid() : 0; }
int FerricNativePageSurface::recommendedState() const { return page() ? int(page()->recommendedState()) : int(QWebEnginePage::LifecycleState::Discarded); }
QWebEngineFrame FerricNativePageSurface::mainFrame() const { return page() ? page()->mainFrame() : QWebEngineFrame(); }
QWebEngineHistory *FerricNativePageSurface::history() const { return page() ? page()->history() : nullptr; }

void FerricNativePageSurface::deliver(const FerricPublicPageOwner::Operation &operation,
                                     const QJSValue &callback, const QVariant &value)
{
    auto *context = qmlContext(this);
    if (!owner_ || !owner_->isCurrent(operation) || !context || !context->isValid() || !callback.isCallable()) return;
    if (auto *engine = qjsEngine(this)) callback.call({engine->toScriptValue(value)});
}

void FerricNativePageSurface::runJavaScript(const QString &script, int world, const QJSValue &callback)
{
    if (!page() || !ordinaryWorld(world) || script.size() > 1024 * 1024) return;
    if (callback.isCallable()) {
        if (!qmlContext(runner_)) QQmlEngine::setContextForObject(runner_, qmlContext(this));
        runner_->run(this, page()->mainFrame(), script, world, callback);
    } else {
        page()->mainFrame().runJavaScript(script, quint32(world));
    }
}

void FerricNativePageSurface::triggerWebAction(int action)
{
    if (page() && action >= QWebEnginePage::Back && action < QWebEnginePage::WebActionCount)
        page()->triggerAction(QWebEnginePage::WebAction(action));
}
void FerricNativePageSurface::goBack() { goBackOrForward(-1); }
void FerricNativePageSurface::goForward() { goBackOrForward(1); }
void FerricNativePageSurface::goBackOrForward(int offset)
{
    if (!page() || !offset || offset < -100 || offset > 100) return;
    auto *cursor = page()->history();
    const auto item = cursor->itemAt(cursor->currentItemIndex() + offset);
    if (item.isValid()) cursor->goToItem(item);
}
void FerricNativePageSurface::reload() { triggerWebAction(QWebEnginePage::Reload); }
void FerricNativePageSurface::stop() { triggerWebAction(QWebEnginePage::Stop); }

void FerricNativePageSurface::focusInEvent(QFocusEvent *event)
{
    QQuickItem::focusInEvent(event);
    if (auto *view = pageView(); view && view->isVisible() && view->isEnabled() && view->isActiveWindow())
        view->setFocus(event->reason());
}

void FerricNativePageSurface::findText(const QString &query, int flags, const QJSValue &callback)
{
    if (!page() || query.size() > 65536 || (flags & ~int(QWebEnginePage::FindBackward | QWebEnginePage::FindCaseSensitively))) return;
    const auto operation = owner_->captureOperation();
    const auto serial = ++findSerial_;
    findCallback_ = callback;
    QPointer<FerricNativePageSurface> guard(this);
    page()->findText(query, QWebEnginePage::FindFlags(flags), [guard, operation, serial](const QWebEngineFindTextResult &result) {
        if (!guard || !guard->owner_ || !guard->owner_->isCurrent(operation) || serial != guard->findSerial_) return;
        const auto callback = guard->findCallback_;
        guard->findCallback_ = {};
        emit guard->findTextFinished(result);
        if (guard) guard->deliver(operation, callback, result.numberOfMatches());
    });
}
