#include <QCoreApplication>
#include <QDBusConnection>
#include <QDBusContext>
#include <QDBusMessage>
#include <QDBusObjectPath>
#include <QDBusServiceWatcher>
#include <QTextStream>
#include <QTimer>
#include <QVariantMap>

struct State {
  QVariantList requests;
  QVariantMap settings;
  QHash<QString, QDBusMessage> held;
  uint version = 4;
  bool nameDropped = false;
};
void respond(const QVariantMap &request, uint code,
             const QVariantMap &results) {
  auto signal = QDBusMessage::createTargetedSignal(
      request.value("sender").toString(), request.value("path").toString(),
      "org.freedesktop.portal.Request", "Response");
  signal.setArguments({code, results});
  if (!QDBusConnection::sessionBus().send(signal))
    qFatal("owned response send failed");
}
class Request final : public QObject {
  Q_OBJECT
  Q_CLASSINFO("D-Bus Interface", "org.freedesktop.portal.Request")
public:
  Request(State *state, int index, QObject *parent)
      : QObject(parent), state(state), index(index) {}
public slots:
  void Close() {
    auto row = state->requests[index].toMap();
    row["close_count"] = row.value("close_count").toInt() + 1;
    state->requests[index] = row;
  }

private:
  State *state;
  int index;
};
class Chooser final : public QObject, protected QDBusContext {
  Q_OBJECT
  Q_CLASSINFO("D-Bus Interface", "org.freedesktop.portal.FileChooser")
  Q_PROPERTY(uint version READ version)
public:
  explicit Chooser(State *state) : state(state) {}
  uint version() const { return state->version; }
public slots:
  QDBusObjectPath OpenFile(const QString &parent, const QString &title,
                           const QVariantMap &options) {
    return open("OpenFile", parent, title, options);
  }
  QDBusObjectPath SaveFile(const QString &parent, const QString &title,
                           const QVariantMap &options) {
    return open("SaveFile", parent, title, options);
  }

private:
  QDBusObjectPath open(const QString &method, const QString &parent,
                       const QString &title, const QVariantMap &options) {
    const QString sender = message().service();
    QString component = sender.mid(1);
    component.replace('.', '_');
    auto path = "/org/freedesktop/portal/desktop/request/" + component + "/" +
                options.value("handle_token").toString();
    if (state->settings.value("alternate").toBool())
      path += "_actual";
    const int index = state->requests.size();
    QVariantMap row{{"path", path},     {"sender", sender},
                    {"method", method}, {"parent", parent},
                    {"title", title},   {"options", options},
                    {"close_count", 0}, {"caller_gone", false}};
    state->requests.append(row);
    if (!QDBusConnection::sessionBus().registerObject(
            path, new Request(state, index, this),
            QDBusConnection::ExportAllSlots))
      qFatal("owned request registration failed");
    auto watch = new QDBusServiceWatcher(
        sender, QDBusConnection::sessionBus(),
        QDBusServiceWatcher::WatchForUnregistration, this);
    connect(watch, &QDBusServiceWatcher::serviceUnregistered, this,
            [this, index] {
              auto row = state->requests[index].toMap();
              row["caller_gone"] = true;
              state->requests[index] = row;
            });
    if (state->settings.value("early").toBool())
      respond(row, 0, {{"uris", QStringList{"file:///tmp/owned-early.txt"}}});
    if (state->settings.value("hold").toBool()) {
      setDelayedReply(true);
      state->held.insert(path, message());
    }
    return QDBusObjectPath(path);
  }
  State *state;
};
class Control final : public QObject {
  Q_OBJECT
  Q_CLASSINFO("D-Bus Interface", "org.ferricbrowser.PortalFixture")
public:
  explicit Control(State *state) : state(state) {}
public slots:
  QVariantList Requests() { return state->requests; }
  void Configure(const QVariantMap &settings) {
    state->settings = settings;
    state->version = settings.value("version", 4).toUInt();
  }
  void Respond(const QString &path, uint code, const QVariantMap &results) {
    for (const auto &item : state->requests) {
      const auto row = item.toMap();
      if (row.value("path") == path) {
        respond(row, code, results);
        return;
      }
    }
    qFatal("unknown owned response path");
  }
  void Release(const QString &path, bool error) {
    if (!state->held.contains(path))
      qFatal("no held owned reply");
    const auto request = state->held.take(path);
    const auto reply =
        error ? request.createErrorReply("org.freedesktop.portal.Error.Failed",
                                         "Owned delayed error")
              : request.createReply(QVariant::fromValue(QDBusObjectPath(path)));
    if (!QDBusConnection::sessionBus().send(reply))
      qFatal("owned reply send failed");
  }
  void DropName() {
    if (!QDBusConnection::sessionBus().unregisterService(
            "org.freedesktop.portal.Desktop"))
      qFatal("could not drop owned service");
    state->nameDropped = true;
  }
  void RestoreName() {
    if (!state->nameDropped)
      return;
    if (!QDBusConnection::sessionBus().registerService(
            "org.freedesktop.portal.Desktop"))
      qFatal("could not restore owned service");
    state->nameDropped = false;
  }

private:
  State *state;
};
int main(int argc, char **argv) {
  QCoreApplication app(argc, argv);
  if (qEnvironmentVariable("FERRIC_PORTAL_TEST_BUS") != "isolated")
    return 2;
  State state;
  Chooser chooser(&state);
  Control control(&state);
  auto bus = QDBusConnection::sessionBus();
  if (!bus.registerService("org.freedesktop.portal.Desktop") ||
      !bus.registerObject("/org/freedesktop/portal/desktop", &chooser,
                          QDBusConnection::ExportAllSlots |
                              QDBusConnection::ExportAllProperties) ||
      !bus.registerObject("/org/ferricbrowser/PortalFixture", &control,
                          QDBusConnection::ExportAllSlots))
    return 3;
  QTextStream(stdout) << "ready" << Qt::endl;
  const int lifetime = qBound(
      90000, qEnvironmentVariableIntValue("FERRIC_PORTAL_FIXTURE_LIFETIME_MS"),
      180000);
  QTimer::singleShot(lifetime, &app, &QCoreApplication::quit);
  return app.exec();
}
#include "service.moc"
