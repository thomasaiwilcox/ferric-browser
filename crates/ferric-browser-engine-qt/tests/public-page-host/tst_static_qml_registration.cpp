#include <QByteArray>
#include <QDebug>
#include <QGuiApplication>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQmlError>
#include <QStringList>
#include <QTest>
#include <QUrl>

#include <memory>

extern "C" bool cxx_qt_init_crate_ferric_browser_engine_qt();

static QString componentErrors(const QQmlComponent &component)
{
    QStringList messages;
    for (const QQmlError &error : component.errors()) {
        messages.append(error.toString());
    }
    return messages.join(QLatin1Char('\n'));
}

class StaticQmlRegistrationTest final : public QObject {
    Q_OBJECT

private slots:
    void createsRegisteredBrowserUi()
    {
        QQmlEngine engine;
        QQmlComponent component(&engine);
        component.setData(
            QByteArrayLiteral(
                "import QtQml\n"
                "import io.github.ferricbrowser 1.0\n"
                "BrowserUi {}\n"),
            QUrl(QStringLiteral("qrc:/tests/static-qml-registration.qml")));

        const QString errors = componentErrors(component);
        QVERIFY2(component.isReady(), qPrintable(errors));

        std::unique_ptr<QObject> browserUi(component.create());
        QVERIFY2(browserUi.get() != nullptr, qPrintable(componentErrors(component)));
    }
};

int main(int argc, char *argv[])
{
    QGuiApplication application(argc, argv);
    if (!cxx_qt_init_crate_ferric_browser_engine_qt()) {
        qCritical() << "CXX-Qt crate initialization failed before QML module use";
        return 1;
    }

    StaticQmlRegistrationTest test;
    return QTest::qExec(&test, argc, argv);
}

#include "tst_static_qml_registration.moc"
