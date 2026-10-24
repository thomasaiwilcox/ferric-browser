"""Run portal regressions against owned D-Bus and Wayland services only."""

from contextlib import ExitStack
from pathlib import Path
import os
import select
import subprocess
import sys
import tempfile


def start(stack, command, env):
    process = subprocess.Popen(command, env=env, stdout=subprocess.PIPE, text=True)

    def stop():
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=3)
        process.stdout.close()

    stack.callback(stop)
    readable, _, _ = select.select([process.stdout], [], [], 5)
    if not readable or process.stdout.readline().strip() != "ready":
        raise RuntimeError("Owned portal fixture did not become ready")
    return process


def main():
    if os.environ.get("FERRIC_PORTAL_TEST_BUS") != "isolated":
        raise RuntimeError("Use the CTest entry, which creates an isolated bus")
    build = Path(sys.argv[1]).resolve()
    test = sys.argv[2]
    if test not in {"request", "file_chooser", "browser", "command_editing", "keyboard_chrome", "chrome_overlay", "navigation_recovery", "keyboard_guide", "page_search", "privacy_identity", "live_titles", "switcher_navigation", "switcher_live_updates", "tab_recovery", "stop_state", "form_reload", "streaming_history"}:
        raise ValueError("Unknown portal test")
    with tempfile.TemporaryDirectory(prefix="ferric-portal-") as temporary, ExitStack() as stack:
        env = os.environ.copy()
        for variable, folder in [
            ("XDG_RUNTIME_DIR", "runtime"),
            ("XDG_CONFIG_HOME", "config"),
            ("XDG_CACHE_HOME", "cache"),
            ("XDG_DATA_HOME", "data"),
            ("XDG_STATE_HOME", "state"),
        ]:
            path = Path(temporary) / folder
            path.mkdir(mode=0o700)
            env[variable] = str(path)
        env.update(QT_QPA_PLATFORMTHEME="", QT_QUICK_BACKEND="software")
        if test in {"file_chooser", "browser", "command_editing", "keyboard_chrome", "chrome_overlay", "navigation_recovery", "keyboard_guide", "page_search", "privacy_identity", "live_titles", "switcher_navigation", "switcher_live_updates", "tab_recovery", "stop_state", "form_reload", "streaming_history"}:
            env.update(WAYLAND_DISPLAY="ferric-portal-test", QT_QPA_PLATFORM="wayland",
                QT_WAYLAND_DISABLE_WINDOWDECORATION="1")
            compositor_env = dict(env, QT_QPA_PLATFORM="offscreen",
                FERRIC_PARENT_DISPLAY=env["WAYLAND_DISPLAY"])
            if test in {"browser", "command_editing", "keyboard_chrome", "chrome_overlay", "navigation_recovery", "keyboard_guide", "page_search", "privacy_identity", "live_titles", "switcher_navigation", "switcher_live_updates", "tab_recovery", "stop_state", "form_reload", "streaming_history"}:
                lifetime = "480000" if test == "tab_recovery" else "360000" if test in {"chrome_overlay", "streaming_history"} else "300000" if test in {"stop_state", "form_reload"} else "180000"
                env["FERRIC_PORTAL_FIXTURE_LIFETIME_MS"] = lifetime
                compositor_env["FERRIC_PORTAL_FIXTURE_LIFETIME_MS"] = lifetime
            start(stack, [str(build / "portal_compositor")], compositor_env)
        if test in {"chrome_overlay", "navigation_recovery", "keyboard_guide", "page_search", "privacy_identity", "live_titles", "switcher_navigation", "switcher_live_updates", "tab_recovery", "stop_state", "form_reload", "streaming_history"}:
            executable = build.parent / "application_tests"
            if test == "tab_recovery":
                arguments = sys.argv[3:] or ["productPolishTabRecovery", "productPolishTabRecoveryCheckpoint"]
            elif test == "switcher_live_updates":
                arguments = sys.argv[3:] or ["productPolishSwitcherLiveUpdates"]
            elif test == "switcher_navigation":
                arguments = sys.argv[3:] or ["productPolishSwitcherNavigation"]
            elif test == "live_titles":
                arguments = sys.argv[3:] or ["productPolishLiveTitles"]
            elif test == "privacy_identity":
                arguments = sys.argv[3:] or ["productPolishPrivacyIdentity"]
            elif test == "page_search":
                arguments = sys.argv[3:] or ["pageSearchFollowing", "pageSearchFeedback", "pageSearchEditorPreedit", "pageSearchDirection"]
            else:
                arguments = sys.argv[3:] or [{"chrome_overlay": "bottomChromeOverlay", "navigation_recovery": "navigationFailureRecovery", "keyboard_guide": "keyboardGuideOwner", "stop_state": "productPolishStopState", "form_reload": "streamingFormReload", "streaming_history": "streamingHistoryTraversal"}[test]]
        elif test in {"command_editing", "keyboard_chrome"}:
            executable = build.parent / "switcher_tests"
            if test == "keyboard_chrome":
                env["FERRIC_SWITCHER_COMPOSITOR_KEYS"] = "1"
                arguments = sys.argv[3:] or [
                    "switcherPaging", "switcherDeferredPaging", "libraryPaging", "libraryDrafts",
                    "journeyDestination", "journeyGraphKeyboard", "managerFormsKeyboard", "favouriteEditorKeyboard", "favouriteRequestsKeyboard", "settingsDrafts", "downloadPaging", "switcherActionFocus",
                    "commandCompletionKeyboard", "commandHistoryEditing", "commandModifiedArrows", "normalCtrlPUsesConfiguredBinding"]
            else:
                arguments = sys.argv[3:] or ["commandCompletionKeyboard",
                    "commandCompletionEditing", "commandCompletionPreedit", "commandHistoryEditing", "commandModifiedArrows"]
        else:
            start(stack, [str(build / "portal_service")], env)
            executable = build / f"portal_{test}_tests"
            arguments = sys.argv[3:]
        timeout = 75
        if test in {"keyboard_chrome", "page_search", "privacy_identity", "live_titles", "switcher_navigation", "switcher_live_updates"}:
            timeout = 180
        elif test in {"browser", "command_editing", "navigation_recovery", "keyboard_guide"}:
            timeout = 150
        elif test == "chrome_overlay":
            timeout = 300
        elif test == "tab_recovery":
            timeout = 450
        elif test == "streaming_history":
            timeout = 330
        elif test in {"stop_state", "form_reload"}:
            timeout = 270
        result = subprocess.run([str(executable), *arguments], env=env, timeout=timeout)
        return result.returncode


if __name__ == "__main__":
    raise SystemExit(main())
