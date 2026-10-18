#include "foreign-server.h"
#include <QDBusConnection>
#include <QGuiApplication>
#include <QHash>
#include <QPointer>
#include <QTextStream>
#include <QTimer>
#include <QUuid>
#include <QVariantMap>
#include <QWaylandClient>
#include <QWaylandCompositor>
#include <QWaylandOutput>
#include <QWaylandOutputMode>
#include <QWaylandSeat>
#include <QWaylandSurface>
#include <QWaylandView>
#include <QWaylandXdgShell>
#include <wayland-server-core.h>

// Test-only compositor: real wl_surface/xdg_toplevel resources, with a small
// xdg_foreign_v2 exporter. Seat events target only its owned client surfaces;
// this process has no connection to the user's display or input devices.
struct Window {
  QString id;
  QPointer<QWaylandSurface> surface;
  QPointer<QWaylandXdgToplevel> top;
  QPointer<QWaylandView> view;
};
struct State {
  QHash<QWaylandSurface *, QPointer<QWaylandXdgToplevel>> tops;
  QList<Window> windows;
  QWaylandCompositor *compositor = nullptr;
  QVariantList exports;
};
struct Export {
  State *state;
  int index;
};
void destroyResource(wl_client *, wl_resource *resource) {
  wl_resource_destroy(resource);
}
const struct zxdg_exported_v2_interface exportedImplementation = {
    destroyResource};
void exportToplevel(wl_client *client, wl_resource *resource, uint32_t id,
                    wl_resource *surfaceResource) {
  auto state = static_cast<State *>(wl_resource_get_user_data(resource));
  auto surface = QWaylandSurface::fromResource(surfaceResource);
  auto top = state->tops.value(surface);
  if (!surface || !top || wl_resource_get_client(surfaceResource) != client) {
    wl_resource_post_error(resource, 0,
                           "Owned fixture requires a real client xdg_toplevel");
    return;
  }
  auto exported =
      wl_resource_create(client, &zxdg_exported_v2_interface, 1, id);
  if (!exported) {
    wl_client_post_no_memory(client);
    return;
  }
  const auto handle =
      "ferric_fixture_" + QUuid::createUuid().toString(QUuid::Id128);
  const int index = state->exports.size();
  QString windowId;
  for (const auto &window : state->windows)
    if (window.surface == surface)
      windowId = window.id;
  state->exports.append(QVariantMap{{"handle", handle},
                                    {"window_id", windowId},
                                    {"title", top->title()},
                                    {"pid", surface->client()->processId()},
                                    {"destroyed", false}});
  wl_resource_set_implementation(
      exported, &exportedImplementation, new Export{state, index},
      [](wl_resource *resource) {
        auto data = static_cast<Export *>(wl_resource_get_user_data(resource));
        auto row = data->state->exports[data->index].toMap();
        row["destroyed"] = true;
        data->state->exports[data->index] = row;
        delete data;
      });
  zxdg_exported_v2_send_handle(exported, handle.toUtf8().constData());
}
const struct zxdg_exporter_v2_interface exporterImplementation = {
    destroyResource, exportToplevel};
void bindExporter(wl_client *client, void *data, uint32_t version,
                  uint32_t id) {
  auto resource = wl_resource_create(client, &zxdg_exporter_v2_interface,
                                     qMin(version, 1U), id);
  if (!resource) {
    wl_client_post_no_memory(client);
    return;
  }
  wl_resource_set_implementation(resource, &exporterImplementation, data,
                                 nullptr);
}
class Control final : public QObject {
  Q_OBJECT
  Q_CLASSINFO("D-Bus Interface", "org.ferricbrowser.ParentFixture")
public:
  explicit Control(State *state) : state(state) {}
public slots:
  QVariantList Exports() { return state->exports; }
  QVariantList Windows() {
    QVariantList result;
    for (const auto &window : state->windows)
      if (window.surface && window.top && window.view)
        result.append(
            QVariantMap{{"id", window.id},
                        {"title", window.top->title()},
                        {"pid", window.surface->client()->processId()},
                        {"mapped", window.surface->hasContent()}});
    return result;
  }
  bool Click(const QString &id, double x, double y) {
    auto seat = state->compositor->defaultSeat();
    if (!seat)
      return false;
    for (const auto &window : state->windows) {
      if (window.id != id || !window.surface || !window.view ||
          !window.surface->hasContent())
        continue;
      seat->setKeyboardFocus(window.surface);
      seat->sendMouseMoveEvent(window.view, QPointF(x, y), QPointF(x, y));
      seat->sendMousePressEvent(Qt::LeftButton);
      seat->sendMouseReleaseEvent(Qt::LeftButton);
      return true;
    }
    return false;
  }
  bool Escape(const QString &id) {
    auto seat = state->compositor->defaultSeat();
    if (!seat)
      return false;
    for (const auto &window : state->windows) {
      if (window.id != id || !window.surface || !window.surface->hasContent())
        continue;
      seat->setKeyboardFocus(window.surface);
      seat->sendKeyEvent(Qt::Key_Escape, true);
      seat->sendKeyEvent(Qt::Key_Escape, false);
      return true;
    }
    return false;
  }

private:
  State *state;
};
int main(int argc, char **argv) {
  QGuiApplication app(argc, argv);
  if (qEnvironmentVariable("FERRIC_PORTAL_TEST_BUS") != "isolated")
    return 2;
  State state;
  QWaylandCompositor compositor;
  state.compositor = &compositor;
  compositor.setSocketName(qgetenv("FERRIC_PARENT_DISPLAY"));
  compositor.setUseHardwareIntegrationExtension(false);
  QWaylandXdgShell shell(&compositor);
  QWaylandOutput output(&compositor, nullptr);
  QObject::connect(
      &shell, &QWaylandXdgShell::toplevelCreated, &compositor,
      [&](QWaylandXdgToplevel *top, QWaylandXdgSurface *xdg) {
        state.tops.insert(xdg->surface(), top);
        auto surface = xdg->surface();
        auto view = new QWaylandView(nullptr, &compositor);
        view->setSurface(surface);
        view->setOutput(&output);
        view->setPrimary();
        state.windows.append(Window{QUuid::createUuid().toString(QUuid::Id128),
                                    surface, top, view});
        QObject::connect(
            surface, &QWaylandSurface::hasContentChanged, &compositor,
            [&, surface] {
              if (surface->hasContent() && compositor.defaultSeat())
                compositor.defaultSeat()->setKeyboardFocus(surface);
            });
        QObject::connect(
            xdg->surface(), &QObject::destroyed, &compositor,
            [&, surface = xdg->surface()] { state.tops.remove(surface); });
        top->sendConfigure(QSize(640, 480),
                           QList<QWaylandXdgToplevel::State>{
                               QWaylandXdgToplevel::ActivatedState});
      });
  const QWaylandOutputMode mode(QSize(1280, 800), 60000);
  output.addMode(mode, true);
  output.setCurrentMode(mode);
  compositor.setDefaultOutput(&output);
  compositor.create();
  if (!wl_global_create(compositor.display(), &zxdg_exporter_v2_interface, 1,
                        &state, bindExporter))
    return 3;
  Control control(&state);
  auto bus = QDBusConnection::sessionBus();
  if (!bus.registerService("org.ferricbrowser.ParentFixture") ||
      !bus.registerObject("/org/ferricbrowser/ParentFixture", &control,
                          QDBusConnection::ExportAllSlots))
    return 4;
  QTimer frames;
  QObject::connect(&frames, &QTimer::timeout, &output, [&] {
    for (const auto &window : state.windows)
      if (window.view && window.surface)
        window.view->advance();
    output.frameStarted();
    output.sendFrameCallbacks();
  });
  frames.start(16);
  QTextStream(stdout) << "ready" << Qt::endl;
  const int lifetime = qBound(
      60000, qEnvironmentVariableIntValue("FERRIC_PORTAL_FIXTURE_LIFETIME_MS"),
      180000);
  QTimer::singleShot(lifetime, &app, &QCoreApplication::quit);
  return app.exec();
}
#include "compositor.moc"
