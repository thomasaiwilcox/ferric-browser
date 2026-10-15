#include "script_runner.h"
#include <QtQml/QJSEngine>
#include <QtQml/QQmlEngine>
#include <utility>

bool FerricScriptRunner::run(QObject *owner, const QWebEngineFrame &frame,
                              const QString &script, int world, const QJSValue &callback) {
    auto *context = owner ? qmlContext(owner) : nullptr;
    if (!context || !context->isValid() || !frame.isValid()
            || !callback.isCallable() || world < 0 || world > 256 || pending_.size() >= 128)
        return false;
    const auto id = ++nextId_;
    auto *timer = new QTimer(this);
    timer->setSingleShot(true);
    pending_.insert(id, Pending{owner, context, timer, callback});
    connect(timer, &QTimer::timeout, this, [this, id] { finish(id, {}); });
    timer->start(2500);
    const QPointer<FerricScriptRunner> guard(this);
    // No QJSValue or raw view pointer survives in the engine callback.
    auto destination = frame;
    destination.runJavaScript(script, static_cast<quint32>(world), [guard, id](const QVariant &value) {
        if (guard) guard->finish(id, value);
    });
    return true;
}

void FerricScriptRunner::finish(quint64 id, const QVariant &value) {
    auto it = pending_.find(id);
    if (it == pending_.end()) return;
    const auto pending = it.value();
    pending_.erase(it);
    if (pending.timer) { pending.timer->stop(); pending.timer->deleteLater(); }
    if (!pending.owner || !pending.context || !pending.context->isValid()) return;
    if (auto *engine = qjsEngine(this))
        pending.callback.call({engine->toScriptValue(value)});
}

void FerricScriptRunner::cancelFor(QObject *owner) {
    for (auto it = pending_.begin(); it != pending_.end();) {
        if (it->owner != owner) { ++it; continue; }
        if (it->timer) { it->timer->stop(); it->timer->deleteLater(); }
        it = pending_.erase(it);
    }
}

void FerricScriptRunner::cancelAll() {
    for (const auto &pending : std::as_const(pending_))
        if (pending.timer) { pending.timer->stop(); pending.timer->deleteLater(); }
    pending_.clear();
}
