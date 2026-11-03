#include <KWaylandExtras>

#include <QGuiApplication>
#include <QEvent>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSocketNotifier>
#include <QTimer>
#include <QRasterWindow>
#include <QPainter>

#include <unistd.h>

#include <cstdio>
#include <map>
#include <memory>
#include <string>

namespace {

class PaintedWindow final : public QRasterWindow {
 public:
  int focusIn = 0, focusOut = 0, activated = 0, deactivated = 0;

 protected:
  bool event(QEvent *event) override {
    switch (event->type()) {
      case QEvent::FocusIn: ++focusIn; break;
      case QEvent::FocusOut: ++focusOut; break;
      case QEvent::WindowActivate: ++activated; break;
      case QEvent::WindowDeactivate: ++deactivated; break;
      default: break;
    }
    return QRasterWindow::event(event);
  }
  void paintEvent(QPaintEvent *) override {
    QPainter painter(this);
    painter.fillRect(QRect(QPoint(0, 0), size()), QColor(38, 52, 67));
  }
};

void emitEvent(QJsonObject event) {
  event.insert(QStringLiteral("pid"), static_cast<qint64>(::getpid()));
  const QByteArray bytes = QJsonDocument(event).toJson(QJsonDocument::Compact);
  std::fwrite(bytes.constData(), 1, static_cast<size_t>(bytes.size()), stdout);
  std::fputc('\n', stdout);
  std::fflush(stdout);
}

class Fixture final : public QObject {
  Q_OBJECT

 public:
  explicit Fixture(QObject *parent = nullptr) : QObject(parent) {
    connect(qGuiApp, &QGuiApplication::focusWindowChanged, this,
            [this](QWindow *) { ++m_focusChanges; });
    m_stdin = new QSocketNotifier(STDIN_FILENO, QSocketNotifier::Read, this);
    connect(m_stdin, &QSocketNotifier::activated, this,
            [this](QSocketDescriptor, QSocketNotifier::Type) { readCommands(); });
  }

 private:
  void readCommands() {
    char chunk[4096];
    const ssize_t count = ::read(STDIN_FILENO, chunk, sizeof(chunk));
    if (count <= 0) {
      m_stdin->setEnabled(false);
      QGuiApplication::quit();
      return;
    }
    m_input.append(chunk, static_cast<size_t>(count));
    if (m_input.size() > 65536) {
      QGuiApplication::quit();
      return;
    }
    for (;;) {
      const size_t newline = m_input.find('\n');
      if (newline == std::string::npos)
        break;
      const QByteArray line(m_input.data(), static_cast<qsizetype>(newline));
      m_input.erase(0, newline + 1);
      const QJsonDocument document = QJsonDocument::fromJson(line);
      if (!document.isObject()) {
        emitEvent({{QStringLiteral("event"), QStringLiteral("error")},
                   {QStringLiteral("reason"), QStringLiteral("invalid-command")}});
        continue;
      }
      const QJsonObject command = document.object();
      const QString operation = command.value(QStringLiteral("op")).toString();
      if (operation == QStringLiteral("create")) {
        createWindow(command);
      } else if (operation == QStringLiteral("destroy")) {
        destroyWindow(command.value(QStringLiteral("key")).toString());
      } else if (operation == QStringLiteral("focus-snapshot")) {
        focusSnapshot(command.value(QStringLiteral("key")).toString());
      } else if (operation == QStringLiteral("exit")) {
        QGuiApplication::quit();
      } else {
        emitEvent({{QStringLiteral("event"), QStringLiteral("error")},
                   {QStringLiteral("reason"), QStringLiteral("unknown-operation")}});
      }
    }
  }

  void createWindow(const QJsonObject &command) {
    const QString key = command.value(QStringLiteral("key")).toString();
    const QString tag = command.value(QStringLiteral("tag")).toString();
    const int delayMs = command.value(QStringLiteral("delay_ms")).toInt();
    if (key.isEmpty() || tag.isEmpty() || m_windows.find(key) != m_windows.end() ||
        delayMs < 0 || delayMs > 5000) {
      emitEvent({{QStringLiteral("event"), QStringLiteral("error")},
                 {QStringLiteral("reason"), QStringLiteral("invalid-window-request")}});
      return;
    }

    auto window = std::make_unique<PaintedWindow>();
    window->setFlags(Qt::Window);
    window->setTitle(QStringLiteral("Ferric W00 test-owned window"));
    window->resize(480, 320);
    KWaylandExtras::setXdgToplevelTag(window.get(), tag);
    PaintedWindow *const rawWindow = window.get();
    m_windows.emplace(key, std::move(window));
    QTimer::singleShot(delayMs, rawWindow, [this, key, rawWindow] {
      if (m_windows.find(key) == m_windows.end())
        return;
      rawWindow->show();
      rawWindow->update();
      emitEvent({{QStringLiteral("event"), QStringLiteral("shown")},
                 {QStringLiteral("key"), key}});
    });
  }

  void destroyWindow(const QString &key) {
    const auto it = m_windows.find(key);
    if (it == m_windows.end()) {
      emitEvent({{QStringLiteral("event"), QStringLiteral("error")},
                 {QStringLiteral("reason"), QStringLiteral("unknown-window")}});
      return;
    }
    it->second->close();
    m_windows.erase(it);
    emitEvent({{QStringLiteral("event"), QStringLiteral("destroyed")},
               {QStringLiteral("key"), key}});
  }

  void focusSnapshot(const QString &key) {
    const auto it = m_windows.find(key);
    if (it == m_windows.end()) {
      emitEvent({{QStringLiteral("event"), QStringLiteral("error")},
                 {QStringLiteral("reason"), QStringLiteral("unknown-window")}});
      return;
    }
    const auto *window = it->second.get();
    emitEvent({{QStringLiteral("event"), QStringLiteral("focus-snapshot")},
               {QStringLiteral("key"), key},
               {QStringLiteral("active"), window->isActive()},
               {QStringLiteral("focus_window"), qGuiApp->focusWindow() == window},
               {QStringLiteral("focus_in"), window->focusIn},
               {QStringLiteral("focus_out"), window->focusOut},
               {QStringLiteral("activated"), window->activated},
               {QStringLiteral("deactivated"), window->deactivated},
               {QStringLiteral("native_focus_changes"), m_focusChanges}});
  }

  QSocketNotifier *m_stdin = nullptr;
  int m_focusChanges = 0;
  std::string m_input;
  std::map<QString, std::unique_ptr<PaintedWindow>> m_windows;
};

}  // namespace

int main(int argc, char **argv) {
  QGuiApplication app(argc, argv);
  QGuiApplication::setQuitOnLastWindowClosed(false);
  const QString appId = qEnvironmentVariable("FERRIC_W00_APP_ID",
                                             QStringLiteral("ferric-w00-proof"));
  if (appId != QStringLiteral("ferric-w00-proof") &&
      appId != QStringLiteral("ferric-w00-neighbor-proof"))
    return 2;
  QGuiApplication::setDesktopFileName(appId);
  Fixture fixture;
  emitEvent({{QStringLiteral("event"), QStringLiteral("ready")},
             {QStringLiteral("platform"), QGuiApplication::platformName()}});
  return app.exec();
}

#include "tst_compositor_identity.moc"
