#pragma once

#include <QObject>
#include <memory>

class FerricQuickCredentialBinding;

// One QApplication-owned actor, shared by every ordinary/private profile and
// native page window. No QML registration, invokables, or page-supplied owners.
class FerricPhoneCredentialActor final : public QObject {
public:
    static void attach(FerricQuickCredentialBinding *binding);
    ~FerricPhoneCredentialActor() override;
private:
    explicit FerricPhoneCredentialActor(QObject *parent);
    void begin(FerricQuickCredentialBinding *binding);
    void provider(bool phone);
    void acceptCredential(unsigned short index);
    void showPhone();
    void showAccounts(unsigned short count);
    void fail();
    void reset();
    bool current() const;
    struct State;
    std::unique_ptr<State> state_;
};
