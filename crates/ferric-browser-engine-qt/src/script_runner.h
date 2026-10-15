#ifndef FERRIC_SCRIPT_RUNNER_H
#define FERRIC_SCRIPT_RUNNER_H

#include <QtCore/QHash>
#include <QtCore/QPointer>
#include <QtCore/QTimer>
#include <QtQml/QQmlContext>
#include <QtQml/QJSValue>
#include <QtQml/qqmlregistration.h>
#include <QtWebEngineCore/QWebEngineFrame>

// Only callback lifetime and bounded delivery live here. Scripts, their worlds
// and the interpretation of results remain with the existing browser owner.
class FerricScriptRunner : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(FerricScriptRunner)
public:
    explicit FerricScriptRunner(QObject *parent = nullptr) : QObject(parent) {}
    Q_INVOKABLE bool run(QObject *owner, const QWebEngineFrame &frame,
                         const QString &script, int world, const QJSValue &callback);
    Q_INVOKABLE void cancelFor(QObject *owner);
    Q_INVOKABLE void cancelAll();
private:
    struct Pending {
        QPointer<QObject> owner;
        QPointer<QQmlContext> context;
        QPointer<QTimer> timer;
        QJSValue callback;
    };
    void finish(quint64 id, const QVariant &value);
    QHash<quint64, Pending> pending_;
    quint64 nextId_ = 0;
};
#endif
