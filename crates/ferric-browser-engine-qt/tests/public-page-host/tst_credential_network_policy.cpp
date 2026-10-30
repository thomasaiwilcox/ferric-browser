#include "credential_network_policy.h"
#include <QCoreApplication>
#include <QJsonDocument>
#include <QJsonObject>
#include <QNetworkProxy>
#include <QNetworkProxyFactory>
#include <QProcess>
#include <QProcessEnvironment>
#include <QTest>
#include <cstdio>

class CredentialNetworkPolicyTests final : public QObject {
    Q_OBJECT
private slots:
    void startupRoute_data() {
        QTest::addColumn<QString>("scenario");
        QTest::addColumn<bool>("admitted");
        QTest::addColumn<bool>("direct");
        QTest::newRow("system-default") << QStringLiteral("system") << true << false;
        QTest::newRow("explicit-direct") << QStringLiteral("direct") << true << true;
        QTest::newRow("proxy-environment") << QStringLiteral("environment") << false << false;
        QTest::newRow("proxy-flags") << QStringLiteral("flags") << false << false;
        QTest::newRow("malformed-flags") << QStringLiteral("malformed") << false << false;
        QTest::newRow("late-route-change") << QStringLiteral("late") << false << false;
    }
    void startupRoute() {
        QFETCH(QString, scenario);
        QFETCH(bool, admitted);
        QFETCH(bool, direct);
        auto environment = QProcessEnvironment::systemEnvironment();
        // Only an owned, socket-free probe process models direct policy.
        for (const auto *name : {"http_proxy", "https_proxy", "all_proxy", "HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"})
            environment.remove(QString::fromLatin1(name));
        environment.insert("QTWEBENGINE_CHROMIUM_FLAGS", "");
        QProcess process;
        process.setProcessEnvironment(environment);
        process.start(QCoreApplication::applicationFilePath(), {"--probe", scenario});
        QVERIFY(process.waitForFinished(5000));
        QCOMPARE(process.exitStatus(), QProcess::NormalExit);
        QCOMPARE(process.exitCode(), 0);
        const auto result = QJsonDocument::fromJson(process.readAllStandardOutput()).object();
        QCOMPARE(result["admitted"].toBool(), admitted);
        QCOMPARE(result["phone_direct"].toBool(), direct);
        QCOMPARE(result["system"].toBool(), !direct);
        if (direct) QCOMPARE(result["application_proxy"].toInt(), int(QNetworkProxy::NoProxy));
    }
};

int main(int argc, char **argv) {
    QCoreApplication application(argc, argv);
    if (argc == 3 && QByteArray(argv[1]) == "--probe") {
        const QByteArray scenario(argv[2]);
        QNetworkProxyFactory::setUseSystemConfiguration(true);
        if (scenario == "environment") qputenv("https_proxy", "http://127.0.0.1:9");
        if (scenario == "flags") qputenv("QTWEBENGINE_CHROMIUM_FLAGS", "--proxy-server=127.0.0.1:9");
        if (scenario == "malformed") qputenv("QTWEBENGINE_CHROMIUM_FLAGS", "--host-resolver-rules=\"MAP * 127.0.0.1");
        if (scenario == "late") FerricCredentialNetworkPolicy::capture();
        const bool admitted = FerricCredentialNetworkPolicy::capture(scenario != "system");
        const QJsonObject result{{"admitted", admitted},
            {"phone_direct", FerricCredentialNetworkPolicy::direct(QUrl("https://passkeys.ferric.test:8443"))},
            {"system", QNetworkProxyFactory::usesSystemConfiguration()},
            {"application_proxy", int(QNetworkProxy::applicationProxy().type())}};
        const auto encoded = QJsonDocument(result).toJson(QJsonDocument::Compact);
        fwrite(encoded.data(), 1, size_t(encoded.size()), stdout);
        return 0;
    }
    CredentialNetworkPolicyTests tests;
    return QTest::qExec(&tests, argc, argv);
}

#include "tst_credential_network_policy.moc"
