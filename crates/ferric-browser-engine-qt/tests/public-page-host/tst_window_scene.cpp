#include "window_scene.h"

#include <QApplication>
#include <QImage>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQuickItem>
#include <QQuickWindow>
#include <QSignalSpy>
#include <QTest>
#include <QVBoxLayout>
#include <memory>

namespace {
constexpr auto source = R"QML(
import QtQuick
import QtQuick.Controls
ApplicationWindow {
    id: logical
    visible: false
    width: 520; height: 360
    title: "Shared browser workspace"
    property int closeRequests: 0
    property int clicks: 0
    property bool allowClose: false
    onClosing: function(event) { closeRequests++; event.accepted = allowClose }
    header: Rectangle { height: 32; color: "#e05632" }
    footer: Rectangle { height: 24; color: "#327be0" }
    Rectangle {
        objectName: "workspace"
        anchors.fill: parent
        color: "#24536d"
        Column {
            x: 24; y: 24; spacing: 12
            TextField { objectName: "editor"; width: 240 }
            Button { objectName: "openDialog"; text: "Open dialog"; onClicked: { logical.clicks++; prompt.open() } }
        }
    }
    Dialog {
        id: prompt
        objectName: "prompt"
        parent: logical.contentItem
        anchors.centerIn: parent
        title: "Owned prompt"
        modal: true
        closePolicy: Popup.CloseOnEscape
        standardButtons: Dialog.Cancel
    }
}
)QML";

std::unique_ptr<QQuickWindow> createWindow(QQmlEngine &engine)
{
    QQmlComponent component(&engine);
    component.setData(source, QUrl(QStringLiteral("qrc:/window-scene-test.qml")));
    if (component.isError()) qWarning() << component.errors();
    return std::unique_ptr<QQuickWindow>(qobject_cast<QQuickWindow *>(component.create()));
}

void click(FerricWindowScene &scene, QQuickItem *item)
{
    const auto point = item->mapToScene(QPointF(item->width() / 2, item->height() / 2));
    QTest::mouseClick(&scene, Qt::LeftButton, Qt::NoModifier, point.toPoint());
}
}

class WindowSceneTests final : public QObject {
    Q_OBJECT
private slots:
    void sharedWorkspaceAndLifetime()
    {
        QQmlEngine engine;
        auto logical = createWindow(engine);
        QVERIFY(logical);
        QVERIFY(!logical->isVisible());
        auto *editor = logical->findChild<QQuickItem *>(QStringLiteral("editor"));
        auto *button = logical->findChild<QQuickItem *>(QStringLiteral("openDialog"));
        auto *prompt = logical->findChild<QObject *>(QStringLiteral("prompt"));
        QVERIFY(editor && button && prompt);
        const QPointer<QObject> originalParent = editor->parent();
        const QPointer<QQuickItem> workspace = logical->findChild<QQuickItem *>(QStringLiteral("workspace"));
        QSignalSpy destroyed(workspace, &QObject::destroyed);
        QWidget host;
        auto *layout = new QVBoxLayout(&host);
        layout->setContentsMargins(0, 0, 0, 0);
        auto *scene = new FerricWindowScene(logical.get(), &engine, &host);
        layout->addWidget(scene);
        QVERIFY(scene->isReady());
        host.resize(520, 360);
        host.show();
        QVERIFY(QTest::qWaitForWindowExposed(&host));
        QVERIFY(QTest::qWaitForWindowActive(&host));
        QVERIFY(scene->nativeWindow() == host.windowHandle());
        QVERIFY(scene->isHostActive());
        QVERIFY(!logical->isVisible());
        QCOMPARE(editor->parent(), originalParent.data());
        QCOMPARE(editor->window(), scene->quickWindow());
        QTRY_COMPARE(scene->grabFramebuffer().pixelColor(8, 8), QColor("#e05632"));
        QCOMPARE(scene->grabFramebuffer().pixelColor(8, scene->height() - 8), QColor("#327be0"));

        click(*scene, editor);
        QTRY_COMPARE(scene->activeFocusItem(), editor);
        QTest::keyClicks(scene, "window mode");
        QCOMPARE(editor->property("text").toString(), QStringLiteral("window mode"));
        click(*scene, button);
        QTRY_COMPARE(logical->property("clicks").toInt(), 1);
        QTRY_VERIFY(prompt->property("visible").toBool());
        QTest::keyClick(scene, Qt::Key_Escape);
        QTRY_VERIFY(!prompt->property("visible").toBool());
        QVERIFY(!logical->isVisible());

        // A dynamically created overlay is moved too, without changing its
        // application/profile QObject ownership.
        auto *later = new QQuickItem(logical->contentItem());
        later->setParentItem(logical->contentItem());
        QTRY_COMPARE(later->window(), scene->quickWindow());
        QCOMPARE(later->parent(), logical->contentItem());
        host.resize(640, 420);
        QTRY_COMPARE(logical->size(), scene->size());

        // Close veto is the existing QML policy, even though its Window has
        // never been mapped. A rejected close leaves the native host usable.
        QVERIFY(!host.close());
        QCOMPARE(logical->property("closeRequests").toInt(), 1);
        QVERIFY(host.isVisible());
        logical->setProperty("allowClose", true);
        scene->requestClose();
        QVERIFY(!host.isVisible());
        QCOMPARE(logical->property("closeRequests").toInt(), 2);
        delete scene;
        QCOMPARE(destroyed.count(), 0);
        QVERIFY(workspace);
        QCOMPARE(editor->window(), logical.get());
        QCOMPARE(later->parentItem(), logical->contentItem());

        // The surviving contexts can be presented again after first-host
        // teardown. They are not implicitly owned by a disposable widget.
        scene = new FerricWindowScene(logical.get(), &engine, &host);
        layout->addWidget(scene);
        QVERIFY(scene->isReady());
        QCOMPARE(editor->window(), scene->quickWindow());
        QCOMPARE(editor->property("text").toString(), QStringLiteral("window mode"));
        delete scene;
        QCOMPARE(destroyed.count(), 0);
    }

    void rejectsIncompatibleAndDuplicatePresentation()
    {
        QQmlEngine engine;
        QQmlEngine otherEngine;
        auto logical = createWindow(engine);
        QVERIFY(logical);
        QWidget host;
        FerricWindowScene wrongEngine(logical.get(), &otherEngine, &host);
        QVERIFY(!wrongEngine.isReady());
        FerricWindowScene first(logical.get(), &engine, &host);
        QVERIFY(first.isReady());
        FerricWindowScene second(logical.get(), &engine, &host);
        QVERIFY(!second.isReady());
        first.release();
        QVERIFY(!first.isReady());
        FerricWindowScene next(logical.get(), &engine, &host);
        QVERIFY(next.isReady());
        logical.reset();
        QVERIFY(!next.isReady());
        QVERIFY(!next.isEnabled());
    }
};

int main(int argc, char **argv)
{
    QApplication application(argc, argv);
    WindowSceneTests tests;
    return QTest::qExec(&tests, argc, argv);
}

#include "tst_window_scene.moc"
