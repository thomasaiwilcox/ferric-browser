#pragma once

#include <QtCore/QtGlobal>
#include <QtCore/QString>
#include <functional>
#include <memory>
#include <utility>

// Native-only scalar reservation. The counter distinguishes graph ticks from
// document dispatch units; neither measures carrier parsing or string work.
// Shared ownership keeps settlement alive independently of user callbacks.
class FerricHintWorkTicket final {
public:
    static constexpr quint32 limit = 100000;
    enum class Counter { Graph, Document };
    enum class Outcome { Known, NoScript, Unknown };
    using Completion = std::function<bool(Outcome, quint32)>;

    explicit FerricHintWorkTicket(Completion completion, Counter counter = Counter::Graph)
        : counter(counter), completion_(std::move(completion)) {}
    FerricHintWorkTicket(const FerricHintWorkTicket &) = delete;
    FerricHintWorkTicket &operator=(const FerricHintWorkTicket &) = delete;
    ~FerricHintWorkTicket() { settle(Outcome::Unknown); }

    bool settle(Outcome outcome, quint32 steps = 0) {
        auto completion = std::exchange(completion_, {});
        return completion && completion(outcome, steps);
    }
    const Counter counter;
private:
    Completion completion_;
};

using FerricHintGraphAdmission = std::function<std::shared_ptr<FerricHintWorkTicket>(quint8)>;
using FerricHintDocumentAdmission = std::function<std::shared_ptr<FerricHintWorkTicket>(const QString &, quint8, quint8)>;
