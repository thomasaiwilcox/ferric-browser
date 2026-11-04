#include "request_interceptor.h"
#include <QCoreApplication>
#include <QElapsedTimer>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <algorithm>
#include <cstdio>

// No ABP engine is installed by this isolated hostname benchmark.
extern "C" void ferric_browser_adblock_release(AdblockEngine *) { qFatal("unexpected ABP release"); }
extern "C" bool ferric_browser_adblock_check(const AdblockEngine *, const unsigned char *, size_t,
    const unsigned char *, size_t, const unsigned char *, size_t, const unsigned char *, size_t,
    unsigned int *, unsigned char *, size_t, size_t *) { qFatal("unexpected ABP check"); }

class FerricEvidenceTest {
public:
    static int run() {
        QStringList excessive;
        for (int index = 0; index < 10000; ++index) excessive.append(QStringLiteral("host%1.test").arg(index));
        if (FerricBrowserRequestInterceptor::validatedHosts(excessive).size() != 1024) return 1;
        QJsonArray workloads;
        for (int count : {0, 8, 64, 1024}) {
            for (bool wildcard : {false, true}) {
                QStringList patterns;
                for (int index = 0; index < count; ++index)
                    patterns.append((wildcard ? QStringLiteral("*.host%1.test") : QStringLiteral("host%1.test")).arg(index));
                const auto host = QStringLiteral("unmatched.example.test");
                QJsonArray samples;
                int matches = 0;
                for (int sample = -10; sample < 100; ++sample) {
                    QElapsedTimer timer; timer.start();
                    for (int request = 0; request < 100; ++request) {
                        matches += std::any_of(patterns.cbegin(), patterns.cend(), [&](const QString &pattern) {
                            return FerricBrowserRequestInterceptor::hostMatches(host, pattern);
                        }) ? 1 : 0;
                    }
                    if (sample >= 0) samples.append(static_cast<double>(timer.nsecsElapsed()) / 100.0);
                }
                if (matches != 0) return 1;
                workloads.append(QJsonObject{{"rules",count},{"wildcards",wildcard},{"samples_ns_per_request",samples}});
            }
        }
        const QJsonDocument document(QJsonObject{{"production_limit",1024},{"warmups",10},{"samples",100},
            {"requests_per_sample",100},{"scenario","worst-case miss"},{"workloads",workloads}});
        std::puts(document.toJson(QJsonDocument::Compact).constData());
        return 0;
    }
};
int main(int argc, char **argv) { QCoreApplication app(argc, argv); return FerricEvidenceTest::run(); }
