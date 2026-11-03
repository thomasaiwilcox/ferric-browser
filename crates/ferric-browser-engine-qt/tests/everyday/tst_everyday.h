#pragma once

#include <QObject>

class EverydayHarnessTests final : public QObject {
    Q_OBJECT

private slots:
    void fixtureServerSupportsDisposableHttpAndHttps();
};
