#include "hint_frame_broker.h"

#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QPointer>
#include <QtCore/QTimer>
#include <QtCore/QSet>
#include <QtCore/QUuid>
#include <QtQml/QJSEngine>
#include <QtWebEngineCore/QWebEngineScript>
#include <memory>

namespace {
QString quoted(const QString &value) {
    const auto json = QJsonDocument(QJsonArray{value}).toJson(QJsonDocument::Compact);
    return QString::fromUtf8(json.mid(1, json.size() - 2));
}
QVariant failure(const char *reason) {
    return QVariantMap{{QStringLiteral("error"), QString::fromLatin1(reason)}};
}
}

FerricFrameBroker::FerricFrameBroker(QObject *parent, const QString &documentProperty)
    : QObject(parent), documentProperty_(documentProperty) {}

void FerricFrameBroker::deliver(const QJSValue &callback, const QVariant &value) {
    if (callback.isCallable()) {
        if (auto *engine = qjsEngine(this)) callback.call({engine->toScriptValue(value)});
    }
}

void FerricFrameBroker::evaluate(QWebEngineFrame frame, const QString &script,
                                    Completion completion) {
    if (!frame.isValid() || inFlight_ >= 128) { completion(failure("frame-operation-limit")); return; }
    ++inFlight_;
    auto completed = std::make_shared<bool>(false);
    auto *timer = new QTimer(this);
    timer->setSingleShot(true);
    const QPointer<QTimer> deadline(timer);
    const QPointer<FerricFrameBroker> guard(this);
    const auto generation = generation_;
    auto finish = [guard, deadline, generation, completed, completion](const QVariant &value) {
        if (*completed) return;
        *completed = true;
        if (deadline) { deadline->stop(); deadline->deleteLater(); }
        if (guard) {
            --guard->inFlight_;
            if (guard->generation_ == generation) completion(value);
        }
    };
    connect(timer, &QTimer::timeout, this, [finish] { finish(failure("frame-timeout")); });
    timer->start(1000);
    frame.runJavaScript(script, QWebEngineScript::ApplicationWorld, finish);
}

bool FerricFrameBroker::reachable(const QString &owner) const {
    auto current = frames_.constFind(owner);
    for (int depth = 0; current != frames_.cend() && depth <= 8; ++depth) {
        if (!current->frame.isValid()) return false;
        if (current->parent.isEmpty()) return current->frame.isMainFrame();
        const auto parent = frames_.constFind(current->parent);
        if (parent == frames_.cend() || !parent->frame.isValid()
            || !parent->frame.children().contains(current->frame)) return false;
        current = parent;
    }
    return false;
}

void FerricFrameBroker::attachFrame(const QWebEngineFrame &frame, const QString &parent,
                                       Completion completion) {
    if (!frame.isValid() || frames_.size() >= 64) {
        completion(failure("frame-limit-or-invalid")); return;
    }
    for (auto it = frames_.cbegin(); it != frames_.cend(); ++it) {
        if (it->frame == frame && it->parent == parent) {
            const auto owner = it.key();
            const auto epoch = it->epoch;
            evaluate(frame, QStringLiteral("window.") + documentProperty_ + QStringLiteral("===") + quoted(epoch),
                     [this, frame, parent, owner, completion](const QVariant &value) {
                if (value.metaType().id() == QMetaType::Bool && value.toBool()
                    && reachable(owner)) completion(QVariantMap{{"owner", owner}});
                else {
                    // A new collection may bind the same native frame's new
                    // document. Old owner IDs remain invalid for pending actions.
                    QSet<QString> retired{owner};
                    for (int depth = 0; depth <= 8; ++depth)
                        for (auto it = frames_.cbegin(); it != frames_.cend(); ++it)
                            if (retired.contains(it->parent)) retired.insert(it.key());
                    for (const auto &id : retired) frames_.remove(id);
                    attachFrame(frame, parent, completion);
                }
            });
            return;
        }
    }
    const auto owner = QStringLiteral("frame-") + QUuid::createUuid().toString(QUuid::WithoutBraces);
    const auto epoch = QUuid::createUuid().toString(QUuid::WithoutBraces);
    evaluate(frame, QStringLiteral("(()=>{window.") + documentProperty_ + QStringLiteral("=") + quoted(epoch)
             + QStringLiteral(";return true;})()"),
             [this, frame, parent, owner, epoch, completion](const QVariant &value) {
        if (value.metaType().id() != QMetaType::Bool || !value.toBool() || !frame.isValid()) {
            completion(failure("frame-initialization-failed")); return;
        }
        frames_.insert(owner, Lease{frame, epoch, parent});
        if (!reachable(owner)) { frames_.remove(owner); completion(failure("invalid-frame")); return; }
        emit leasesChanged();
        completion(QVariantMap{{"owner", owner}});
    });
}

void FerricFrameBroker::attach(const QWebEngineFrame &frame, const QJSValue &callback) {
    if (!frame.isMainFrame()) { deliver(callback, failure("main-frame-required")); return; }
    attachFrame(frame, {}, [this, callback](const QVariant &value) { deliver(callback, value); });
}

void FerricFrameBroker::focusedChild(const QString &parent, const QJSValue &callback) {
    if (!reachable(parent)) { deliver(callback, failure("invalid-parent-frame")); return; }
    const auto children = frames_.value(parent).frame.children();
    if (children.isEmpty() || children.size() > 64) {
        deliver(callback, failure("no-unique-focused-child")); return;
    }
    for (const auto &child : children) {
        if (!inspected_.contains(child)) {
            if (inspected_.size() >= 64) { deliver(callback, failure("frame-enumeration-limit")); return; }
            inspected_.append(child);
        }
    }
    struct Scan { int remaining; QList<QWebEngineFrame> focused; bool failed = false; };
    auto scan = std::make_shared<Scan>();
    scan->remaining = children.size();
    for (const auto &child : children) {
        evaluate(child, QStringLiteral("document.hasFocus()"),
                 [this, scan, child, parent, callback](const QVariant &value) {
            if (value.metaType().id() != QMetaType::Bool) scan->failed = true;
            else if (value.toBool()) scan->focused.append(child);
            if (--scan->remaining != 0) return;
            if (scan->failed || scan->focused.size() != 1 || !reachable(parent)) {
                deliver(callback, failure("no-unique-focused-child")); return;
            }
            attachFrame(scan->focused.first(), parent,
                        [this, callback](const QVariant &result) { deliver(callback, result); });
        });
    }
}

void FerricFrameBroker::run(const QString &owner, const QString &script, const QJSValue &callback) {
    execute(owner, script, callback, false);
}

void FerricFrameBroker::dispatch(const QString &owner, const QString &script, const QJSValue &callback) {
    execute(owner, script, callback, true);
}

void FerricFrameBroker::execute(const QString &owner, const QString &script,
                                   const QJSValue &callback, bool action) {
    if (!reachable(owner) || script.size() > 1024 * 1024) {
        deliver(callback, failure("invalid-frame-operation")); return;
    }
    const auto lease = frames_.value(owner);
    const auto wrapped = QStringLiteral("(()=>{if(window.") + documentProperty_ + QStringLiteral("!==")
        + quoted(lease.epoch) + QStringLiteral(")return {error:'stale-frame-document'};return (")
        + script + QStringLiteral(");})()");
    evaluate(lease.frame, wrapped, [this, owner, callback, action](const QVariant &value) {
        // An acknowledged action may intentionally dismiss/remove its frame.
        // Only the exact successful boolean completion is accepted after removal;
        // collection and preflight must still reject any detached owner.
        const bool acknowledged = action && value.metaType().id() == QMetaType::Bool && value.toBool();
        deliver(callback, reachable(owner) || acknowledged ? value : failure("invalid-frame"));
    });
}

void FerricFrameBroker::clear() {
    ++generation_;
    frames_.clear();
    inspected_.clear();
    emit leasesChanged();
}
