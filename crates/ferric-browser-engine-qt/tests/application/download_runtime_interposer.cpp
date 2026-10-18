// Test-only ABI version mismatch. The production guard must reject admission
// before creating a controller or inspecting any private Qt request state.
const char *qWebEngineVersion() noexcept {
    return "0.0.0-owned-runtime-mismatch";
}
