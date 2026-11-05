#ifndef FERRIC_NATIVE_WAYLAND_H
#define FERRIC_NATIVE_WAYLAND_H

#include <QObject>
#include <QPointer>
#include <QString>
#include <QWindow>
#include <memory>

enum class FerricExportStatus { Success, Unsupported, Invalidated, Cancelled, Failed };
struct FerricExportResult {
    FerricExportStatus status = FerricExportStatus::Failed;
    QString handle;
    quint64 generation = 0;
};
Q_DECLARE_METATYPE(FerricExportResult)

// A consumer owns its subscription. Deleting it detaches only that consumer;
// the generation's shared export remains alive until native invalidation.
class FerricWaylandExport final : public QObject {
    Q_OBJECT
public:
    explicit FerricWaylandExport(quint64 generation, QObject *parent)
        : QObject(parent), generation_(generation) {}
    void cancel() { finish({FerricExportStatus::Cancelled, {}, generation_}); }
    bool finished() const { return finished_; }
    void finish(const FerricExportResult &result) {
        if (finished_) return;
        finished_ = true;
        emit completed(result);
    }
signals:
    void completed(const FerricExportResult &result);
private:
    bool finished_ = false;
    quint64 generation_;
};

// Prepare before QWindow::create/show to tag the first role before its first
// commit. Logical preparation survives hide/show and platform-window teardown.
class FerricWaylandWindow final : public QObject {
    Q_OBJECT
public:
    static FerricWaylandWindow *prepare(QWindow *window, const QString &tag = {});
    ~FerricWaylandWindow() override;
    quint64 generation() const;
    bool tagSubmitted() const;
    FerricWaylandExport *exportToplevel(QObject *consumer);
signals:
    void invalidated(quint64 generation);
    void changed();
protected:
    bool eventFilter(QObject *watched, QEvent *event) override;
private:
    friend void ferric_shutdown_wayland();
    explicit FerricWaylandWindow(QWindow *window, const QString &tag);
    class State;
    std::unique_ptr<State> state_;
};

// GUI-thread only. The application explicitly retires this context before
// QApplication tears down its platform integration and Wayland connection.
void ferric_initialize_wayland();
void ferric_shutdown_wayland();
#endif
