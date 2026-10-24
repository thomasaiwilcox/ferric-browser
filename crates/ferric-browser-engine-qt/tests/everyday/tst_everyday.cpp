#include "tst_everyday.h"

#include <QProcess>
#include <QtTest>

void EverydayHarnessTests::fixtureServerSupportsDisposableHttpAndHttps() {
    const auto script = qEnvironmentVariable("FERRIC_EVERYDAY_FIXTURE_SERVER");
    QVERIFY2(!script.isEmpty(), "fixture script path must be supplied by CTest");
    QProcess process;
    process.setProgram(qEnvironmentVariable("Python3_EXECUTABLE", "python3"));
    process.setArguments({script, "--self-test"});
    process.setProcessChannelMode(QProcess::MergedChannels);
    process.start();
    QVERIFY2(process.waitForStarted(), qPrintable(process.errorString()));
    QVERIFY2(process.waitForFinished(15000), "fixture self-test exceeded its deadline");
    const auto output = process.readAll();
    QCOMPARE(process.exitStatus(), QProcess::NormalExit);
    QVERIFY2(process.exitCode() == 0, output.constData());
    QVERIFY2(output.contains("HTTP and HTTPS disposable fixture checks passed"), output.constData());
}

QTEST_GUILESS_MAIN(EverydayHarnessTests)
#include "tst_everyday.moc"
