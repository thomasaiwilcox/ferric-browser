#include "foreign-server.h"
#include <QBuffer>
#include <QDBusConnection>
#include <QEventLoop>
#include <QGuiApplication>
#include <QHash>
#include <QImage>
#include <QPointer>
#include <QTextStream>
#include <QTimer>
#include <QUuid>
#include <QVariantMap>
#include <QWaylandClient>
#include <QWaylandCompositor>
#include <QWaylandKeyboard>
#include <QWaylandKeymap>
#include <QWaylandOutput>
#include <QWaylandOutputMode>
#include <QWaylandSeat>
#include <QWaylandSurface>
#include <QWaylandSurfaceGrabber>
#include <QWaylandView>
#include <QWaylandXdgShell>
#include <wayland-server-core.h>
#include <wayland-server-protocol.h>
#include <xkbcommon/xkbcommon.h>
#include <xkbcommon/xkbcommon-names.h>
#include <cstring>

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
  QHash<int, uint32_t> modifierMasks;
};
bool initializeModifiers(State *state) {
  auto settings = state->compositor->defaultSeat()->keymap();
  settings->setLayout("us");
  QCoreApplication::processEvents();
  const auto rules = settings->rules().toLocal8Bit();
  const auto model = settings->model().toLocal8Bit();
  const auto layout = settings->layout().toLocal8Bit();
  const auto variant = settings->variant().toLocal8Bit();
  const auto options = settings->options().toLocal8Bit();
  const xkb_rule_names names{rules.constData(), model.constData(), layout.constData(),
                             variant.constData(), options.constData()};
  auto context = xkb_context_new(XKB_CONTEXT_NO_FLAGS);
  if (!context) return false;
  auto keymap = xkb_keymap_new_from_names(context, &names, XKB_KEYMAP_COMPILE_NO_FLAGS);
  xkb_context_unref(context);
  if (!keymap) return false;
  const QList<QPair<int, const char *>> modifiers{
      {Qt::ShiftModifier, XKB_MOD_NAME_SHIFT}, {Qt::ControlModifier, XKB_MOD_NAME_CTRL},
      {Qt::AltModifier, XKB_MOD_NAME_ALT}, {Qt::MetaModifier, XKB_MOD_NAME_LOGO}};
  for (const auto &modifier : modifiers) {
    auto index = xkb_keymap_mod_get_index(keymap, modifier.second);
    if (index < 32) state->modifierMasks.insert(modifier.first, uint32_t(1) << index);
  }
  xkb_keymap_unref(keymap);
  return state->modifierMasks.size() == modifiers.size();
}
void sendModifiers(State *state, QWaylandSurface *surface, int modifiers) {
  struct Update { uint32_t serial, mask; } update{state->compositor->nextSerial(), 0};
  for (auto modifier = state->modifierMasks.cbegin(); modifier != state->modifierMasks.cend(); ++modifier)
    if (modifiers & modifier.key()) update.mask |= modifier.value();
  // This fixture has one seat. Keep protocol modifiers explicit, including
  // release, instead of relying on Qt's synthetic-key modifier repair cache.
  wl_client_for_each_resource(surface->client()->client(), [](wl_resource *resource, void *data) {
    auto update = static_cast<Update *>(data);
    if (std::strcmp(wl_resource_get_class(resource), "wl_keyboard") == 0)
      wl_keyboard_send_modifiers(resource, update->serial, update->mask, 0, 0, 0);
    return WL_ITERATOR_CONTINUE;
  }, &update);
}
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
  QByteArray Capture(const QString &id) {
    for (const auto &window : state->windows) {
      if (window.id != id || !window.surface || !window.surface->hasContent())
        continue;
      QWaylandSurfaceGrabber grabber(window.surface);
      QEventLoop ready;
      QImage image;
      bool complete = false;
      connect(&grabber, &QWaylandSurfaceGrabber::success, &ready, [&](const QImage &result) {
        image = result;
        complete = true;
        ready.quit();
      });
      connect(&grabber, &QWaylandSurfaceGrabber::failed, &ready, [&] {
        complete = true;
        ready.quit();
      });
      grabber.grab();
      if (!complete) {
        QTimer::singleShot(1000, &ready, &QEventLoop::quit);
        ready.exec();
      }
      QByteArray png;
      QBuffer buffer(&png);
      if (!image.isNull() && buffer.open(QIODevice::WriteOnly)) image.save(&buffer, "PNG");
      return png;
    }
    return {};
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
    return Key(id, Qt::Key_Escape, 0);
  }
  bool Key(const QString &id, int key, int modifiers) {
    if (modifiers & ~(Qt::ShiftModifier | Qt::ControlModifier | Qt::AltModifier | Qt::MetaModifier))
      return false;
    auto seat = state->compositor->defaultSeat();
    if (!seat)
      return false;
    for (const auto &window : state->windows) {
      if (window.id != id || !window.surface || !window.surface->hasContent())
        continue;
      seat->setKeyboardFocus(window.surface);
      const QList<QPair<int, int>> modifierKeys{
          {Qt::ShiftModifier, Qt::Key_Shift}, {Qt::ControlModifier, Qt::Key_Control},
          {Qt::AltModifier, Qt::Key_Alt}, {Qt::MetaModifier, Qt::Key_Meta}};
      if (!seat->keyboard()->keyToScanCode(key)) return false;
      for (const auto &modifier : modifierKeys)
        if ((modifiers & modifier.first) && !seat->keyboard()->keyToScanCode(modifier.second)) return false;
      for (const auto &modifier : modifierKeys)
        if (modifiers & modifier.first) seat->sendKeyEvent(modifier.second, true);
      sendModifiers(state, window.surface, modifiers);
      seat->sendKeyEvent(key, true);
      seat->sendKeyEvent(key, false);
      for (auto modifier = modifierKeys.crbegin(); modifier != modifierKeys.crend(); ++modifier)
        if (modifiers & modifier->first) seat->sendKeyEvent(modifier->second, false);
      sendModifiers(state, window.surface, 0);
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
  if (!initializeModifiers(&state)) return 5;
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
