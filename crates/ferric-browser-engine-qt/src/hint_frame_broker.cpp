#include "hint_frame_broker.h"

#include <QtCore/QJsonArray>
#include <QtCore/QJsonDocument>
#include <QtCore/QPointer>
#include <QtCore/QTimer>
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

FerricHintFrameBroker::FerricHintFrameBroker(QObject *parent) : QObject(parent) {}

void FerricHintFrameBroker::deliver(const QJSValue &callback, const QVariant &value) {
    if (callback.isCallable()) {
        if (auto *engine = qjsEngine(this)) callback.call({engine->toScriptValue(value)});
    }
}

void FerricHintFrameBroker::evaluate(QWebEngineFrame frame, const QString &script,
                                    Completion completion) {
    if (!frame.isValid() || inFlight_ >= 128) { completion(failure("frame-operation-limit")); return; }
    ++inFlight_;
    auto completed = std::make_shared<bool>(false);
    const QPointer<FerricHintFrameBroker> guard(this);
    const auto generation = generation_;
    auto finish = [guard, generation, completed, completion](const QVariant &value) {
        if (*completed) return;
        *completed = true;
        if (guard) {
            --guard->inFlight_;
            if (guard->generation_ == generation) completion(value);
        }
    };
    QTimer::singleShot(1000, this, [finish] { finish(failure("frame-timeout")); });
    frame.runJavaScript(script, QWebEngineScript::ApplicationWorld, finish);
}

bool FerricHintFrameBroker::reachable(const QString &owner) const {
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

void FerricHintFrameBroker::attachFrame(const QWebEngineFrame &frame, const QString &parent,
                                       Completion completion) {
    if (!frame.isValid() || frames_.size() >= 64) {
        completion(failure("frame-limit-or-invalid")); return;
    }
    for (auto it = frames_.cbegin(); it != frames_.cend(); ++it) {
        if (it->frame == frame && it->parent == parent) {
            const auto owner = it.key();
            const auto epoch = it->epoch;
            evaluate(frame, QStringLiteral("window.__ferric_hintFrameDocument===") + quoted(epoch),
                     [this, owner, completion](const QVariant &value) {
                if (value.metaType().id() == QMetaType::Bool && value.toBool()
                    && reachable(owner)) completion(QVariantMap{{"owner", owner}});
                else completion(failure("stale-frame-document"));
            });
            return;
        }
    }
    const auto owner = QStringLiteral("frame-") + QUuid::createUuid().toString(QUuid::WithoutBraces);
    const auto epoch = QUuid::createUuid().toString(QUuid::WithoutBraces);
    evaluate(frame, QStringLiteral("(()=>{window.__ferric_hintFrameDocument=") + quoted(epoch)
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

void FerricHintFrameBroker::attach(const QWebEngineFrame &frame, const QJSValue &callback) {
    if (!frame.isMainFrame()) { deliver(callback, failure("main-frame-required")); return; }
    attachFrame(frame, {}, [this, callback](const QVariant &value) { deliver(callback, value); });
}

void FerricHintFrameBroker::focusedChild(const QString &parent, const QJSValue &callback) {
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

void FerricHintFrameBroker::run(const QString &owner, const QString &script, const QJSValue &callback) {
    execute(owner, script, callback, false);
}

void FerricHintFrameBroker::dispatch(const QString &owner, const QString &script, const QJSValue &callback) {
    execute(owner, script, callback, true);
}

void FerricHintFrameBroker::execute(const QString &owner, const QString &script,
                                   const QJSValue &callback, bool action) {
    if (!reachable(owner) || script.size() > 1024 * 1024) {
        deliver(callback, failure("invalid-frame-operation")); return;
    }
    const auto lease = frames_.value(owner);
    const auto wrapped = QStringLiteral("(()=>{if(window.__ferric_hintFrameDocument!==")
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

void FerricHintFrameBroker::clear() {
    ++generation_;
    frames_.clear();
    inspected_.clear();
    emit leasesChanged();
}
