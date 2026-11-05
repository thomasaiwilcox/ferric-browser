#include "xdg-foreign-unstable-v2-server.h"
#include "xdg-toplevel-tag-v1-server.h"
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
#include <utility>

// Test-only compositor: real wl_surface/xdg_toplevel resources, with a small
// xdg_foreign_v2 exporter. Seat events target only its owned client surfaces;
// this process has no connection to the user's display or input devices.
class OwnedCompositor final : public QWaylandCompositor {
protected:
  QWaylandSeat *createSeat() override {
    auto seat = QWaylandCompositor::createSeat();
    // QWaylandSeat's compositor argument is not a QObject parent. Explicit
    // ownership releases the seat/keymap before the display's teardown.
    seat->setParent(this);
    return seat;
  }
};
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
  QVariantList protocol;
  bool holdExports = false;
  QList<QPair<wl_resource *, QString>> heldExports;
  QHash<int, uint32_t> modifierMasks;
  QHash<QString, uint32_t> physicalKeys;
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
  for (const auto &position : {"AB10", "AC10"}) {
    const auto code = xkb_keymap_key_by_name(keymap, position);
    if (code != XKB_KEYCODE_INVALID && code >= 8)
      state->physicalKeys.insert(QString::fromLatin1(position), code);
  }
  xkb_keymap_unref(keymap);
  return state->modifierMasks.size() == modifiers.size()
      && state->physicalKeys.contains("AB10") && state->physicalKeys.contains("AC10");
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
        data->state->heldExports.removeIf([resource](const auto &item) { return item.first == resource; });
        auto row = data->state->exports[data->index].toMap();
        row["destroyed"] = true;
        data->state->exports[data->index] = row;
        delete data;
      });
  if (state->holdExports) {
    state->heldExports.append({exported, handle});
    // Remove dead resources before the test releases delayed replies.
    // The protocol logger independently retains their destruction receipt.
  } else zxdg_exported_v2_send_handle(exported, handle.toUtf8().constData());
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
void setTag(wl_client *, wl_resource *, wl_resource *, const char *) {}
const struct xdg_toplevel_tag_manager_v1_interface tagImplementation = {
    destroyResource, setTag, setTag};
void bindTags(wl_client *client, void *, uint32_t version, uint32_t id) {
  auto resource = wl_resource_create(client, &xdg_toplevel_tag_manager_v1_interface, qMin(version, 1U), id);
  wl_resource_set_implementation(resource, &tagImplementation, nullptr, nullptr);
}
void recordProtocol(void *data, wl_protocol_logger_type direction, const wl_protocol_logger_message *message) {
  if (direction != WL_PROTOCOL_LOGGER_REQUEST) return;
  auto state = static_cast<State *>(data);
  const QByteArray interface(wl_resource_get_class(message->resource));
  const QByteArray request(message->message->name);
  if (interface != "wl_surface" && interface != "xdg_surface" && interface != "xdg_toplevel"
      && interface != "xdg_toplevel_tag_manager_v1" && interface != "wl_compositor") return;
  if (request != "commit" && request != "attach" && request != "destroy" && request != "get_toplevel"
      && request != "set_toplevel_tag" && request != "create_surface") return;
  pid_t pid; uid_t uid; gid_t gid;
  wl_client_get_credentials(wl_resource_get_client(message->resource), &pid, &uid, &gid);
  QVariantMap row{{"interface", QString::fromLatin1(interface)}, {"request", QString::fromLatin1(request)},
                  {"object", wl_resource_get_id(message->resource)}, {"pid", qlonglong(pid)}};
  if (request == "create_surface" || request == "get_toplevel") row["new_id"] = message->arguments[0].n;
  if (request == "get_toplevel") {
    auto xdg = QWaylandXdgSurface::fromResource(message->resource);
    if (xdg) row["surface"] = wl_resource_get_id(xdg->surface()->resource());
  }
  if (request == "set_toplevel_tag") {
    row["role"] = wl_resource_get_id(reinterpret_cast<wl_resource *>(message->arguments[0].o));
    row["tag"] = QString::fromUtf8(message->arguments[1].s);
  }
  if (request == "attach") row["buffer"] = message->arguments[0].o != nullptr;
  state->protocol.append(row);
}
class Control final : public QObject {
  Q_OBJECT
  Q_CLASSINFO("D-Bus Interface", "org.ferricbrowser.ParentFixture")
public:
  explicit Control(State *state) : state(state) {}
public slots:
  QVariantList Exports() { return state->exports; }
  void Quit() { QTimer::singleShot(0, qApp, &QCoreApplication::quit); }
  QVariantList Protocol() { return state->protocol; }
  QVariantMap Resources() {
    int exports = 0, roles = 0;
    for (const auto &entry : state->exports)
      if (!entry.toMap()["destroyed"].toBool()) ++exports;
    for (const auto &top : state->tops) if (top) ++roles;
    return {{"exports", exports}, {"roles", roles},
            {"views", state->compositor->findChildren<QWaylandView *>().size()}};
  }
  void HoldExports(bool hold) { state->holdExports = hold; }
  void ReleaseExports() {
    const auto pending = std::exchange(state->heldExports, {});
    for (const auto &item : pending) zxdg_exported_v2_send_handle(item.first, item.second.toUtf8().constData());
  }
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
  bool Move(const QString &id, double x, double y) {
    auto seat = state->compositor->defaultSeat();
    if (!seat)
      return false;
    for (const auto &window : state->windows) {
      if (window.id == id && window.surface && window.view && window.surface->hasContent()) {
        seat->sendMouseMoveEvent(window.view, QPointF(x, y), QPointF(x, y));
        return true;
      }
    }
    return false;
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
    return sendKey(id, key, modifiers, false);
  }
  bool KeyPosition(const QString &id, const QString &position, int modifiers) {
    if (!state->physicalKeys.contains(position)) return false;
    return sendKey(id, state->physicalKeys.value(position), modifiers, true);
  }

private:
  bool sendKey(const QString &id, int key, int modifiers, bool physical) {
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
      if (!physical && !seat->keyboard()->keyToScanCode(key)) return false;
      for (const auto &modifier : modifierKeys)
        if ((modifiers & modifier.first) && !seat->keyboard()->keyToScanCode(modifier.second)) return false;
      for (const auto &modifier : modifierKeys)
        if (modifiers & modifier.first) seat->sendKeyEvent(modifier.second, true);
      sendModifiers(state, window.surface, modifiers);
      if (physical) {
        seat->sendKeyPressEvent(key);
        seat->sendKeyReleaseEvent(key);
      } else {
        seat->sendKeyEvent(key, true);
        seat->sendKeyEvent(key, false);
      }
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
  OwnedCompositor compositor;
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
        QObject::connect(top, &QObject::destroyed, &compositor,
            [&, surface, guardedView = QPointer<QWaylandView>(view)] {
              if (!state.tops.value(surface)) state.tops.remove(surface);
              if (guardedView) delete guardedView.data();
              state.windows.removeIf([](const Window &window) { return !window.top || !window.surface; });
            });
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
  auto logger = wl_display_add_protocol_logger(compositor.display(), recordProtocol, &state);
  if (!initializeModifiers(&state)) return 5;
  if (!qEnvironmentVariableIsSet("FERRIC_FIXTURE_NO_EXPORTER") && !wl_global_create(compositor.display(), &zxdg_exporter_v2_interface, 1,
                        &state, bindExporter))
    return 3;
  if (!qEnvironmentVariableIsSet("FERRIC_FIXTURE_NO_TAGS") && !wl_global_create(compositor.display(), &xdg_toplevel_tag_manager_v1_interface, 1, &state, bindTags)) return 3;
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
      480000);
  QTimer::singleShot(lifetime, &app, &QCoreApplication::quit);
  const int result = app.exec();
  frames.stop();
  // Release client-owned protocol resources while State and the display are
  // still alive, then drain Qt's deferred shell/surface object destruction.
  for (auto client : compositor.clients()) compositor.destroyClient(client);
  QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
  wl_protocol_logger_destroy(logger);
  return result;
}
#include "compositor.moc"
