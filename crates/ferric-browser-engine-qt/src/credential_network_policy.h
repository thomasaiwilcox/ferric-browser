#ifndef FERRIC_CREDENTIAL_NETWORK_POLICY_H
#define FERRIC_CREDENTIAL_NETWORK_POLICY_H
#include <QStringList>
#include <QUrl>
#include <optional>

namespace FerricCredentialNetworkPolicy {
// Called by native bootstrap before any WebEngine profile is created.
// Explicit direct mode applies to all Qt/Chromium traffic in this process.
// It is a startup choice, never a provider-specific policy bypass.
bool capture(bool explicitDirect = false);
bool direct(const QUrl &requestingOrigin);

// Pure policy edges, shared with native regression tests. No QML exposure.
std::optional<QStringList> chromiumArguments(const QStringList &arguments,
                                           const std::optional<QString> &environment);
bool argumentsAllowDirect(const QStringList &arguments, const QUrl &requestingOrigin);
}
#endif
