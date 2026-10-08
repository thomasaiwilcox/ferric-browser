import QtQuick

Loader {
    id: consentHost
    required property var browserWindow

    active: browserWindow.nativePageCaptureDisclosure.length === 2
            || browserWindow.nativeStashDisclosure.length === 2

    sourceComponent: Component {
        FerricVersioningConsentDialog {
            id: consentDialog
            browserWindow: consentHost.browserWindow
            pageCaptureMode: browserWindow.nativePageCaptureDisclosure.length === 2
            operationId: pageCaptureMode
                         ? browserWindow.nativePageCaptureDisclosure[0]
                         : browserWindow.nativeStashDisclosure.length === 2
                           ? browserWindow.nativeStashDisclosure[0] : ""
            disclosureText: pageCaptureMode
                            ? browserWindow.nativePageCaptureDisclosure[1]
                            : browserWindow.nativeStashDisclosure.length === 2
                              ? browserWindow.nativeStashDisclosure[1] : ""
            // Stash approval still requires a qualified fresh activity observation.
            acceptEnabled: pageCaptureMode

            onAcceptRequested: function(operationId) {
                var host = consentHost.browserWindow
                if (!consentDialog.pageCaptureMode
                        || host.nativePageCaptureDisclosure.length !== 2
                        || host.nativePageCaptureDisclosure[0] !== operationId) return
                // A refused Rust decision may have consumed the preview. Always
                // refresh from Rust; this local list is only presentation state.
                try {
                    host.browserUi.accept_native_page_capture_disclosure(operationId)
                } finally {
                    if (host.nativePageCaptureDisclosure.length === 2
                            && host.nativePageCaptureDisclosure[0] === operationId)
                        host.nativePageCaptureDisclosure = []
                    host.scheduleRuntimeWork(0)
                }
            }
            onDeclineRequested: function(operationId) {
                var host = consentHost.browserWindow
                if (consentDialog.pageCaptureMode) {
                    if (host.nativePageCaptureDisclosure.length !== 2
                            || host.nativePageCaptureDisclosure[0] !== operationId) return
                    try {
                        host.browserUi.decline_native_page_capture_disclosure(operationId)
                    } finally {
                        if (host.nativePageCaptureDisclosure.length === 2
                                && host.nativePageCaptureDisclosure[0] === operationId)
                            host.nativePageCaptureDisclosure = []
                        host.scheduleRuntimeWork(0)
                    }
                } else if (host.browserUi.decline_native_stash_disclosure(operationId)) {
                    host.nativeStashDisclosure = []
                    host.scheduleRuntimeWork(0)
                }
            }
        }
    }
}
