import QtQuick
import QtWebEngine
import io.github.ferricbrowser 1.0

// Isolated, untrusted presentation for one explicitly authorised MHTML file.
// This component deliberately has no Ferric page bridge, WebChannel, injected
// scripts, normal-profile reference, or privileged internal-page content.
Item {
    id: archiveHost

    required property string authorizedArchiveUrl
    property bool archiveReady: false
    readonly property string archiveTitle: sanitizeArchiveTitle(archiveView.title)
    readonly property int rejectedNavigationCount: rejectedNavigations
    property int rejectedNavigations: 0
    property bool initialNavigationPending: false
    property bool closingToBlank: false
    property bool blankWaitExpired: false

    signal archiveLoaded(string title)
    signal archiveLoadFailed()
    signal archiveBlanked()
    signal archiveBlankingTimedOut()

    function sanitizeArchiveTitle(value) {
        // The archive document controls its title. Keep it inert, bounded,
        // and resistant to directional-control spoofing in the browser chrome.
        let title = String(value || "").slice(0, 512)
        if (title.length > 0) {
            const lastCodeUnit = title.charCodeAt(title.length - 1)
            if (lastCodeUnit >= 0xD800 && lastCodeUnit <= 0xDBFF)
                title = title.slice(0, -1) + "\uFFFD"
        }
        title = title
                .replace(/[\u061c\u200e-\u200f\u202a-\u202e\u2066-\u2069]/g, "[bidi]")
                .replace(/[\u0000-\u001f\u007f-\u009f]/g, "\uFFFD")
        if (title.length > 512)
            title = title.slice(0, 512)
        if (title.length > 0) {
            const finalCodeUnit = title.charCodeAt(title.length - 1)
            if (finalCodeUnit >= 0xD800 && finalCodeUnit <= 0xDBFF)
                title = title.slice(0, -1) + "\uFFFD"
        }
        return title
    }

    Timer {
        id: blankingTimer
        interval: 5000
        repeat: false
        onTriggered: {
            if (archiveHost.closingToBlank && archiveHost.blankWaitExpired === false) {
                archiveHost.blankWaitExpired = true
                archiveHost.archiveBlankingTimedOut()
            }
        }
    }

    WebEngineProfile {
        id: archiveProfile
        offTheRecord: true
        storageName: ""
        persistentStoragePath: ""
        cachePath: ""
        persistentCookiesPolicy: WebEngineProfile.NoPersistentCookies
        httpCacheType: WebEngineProfile.MemoryHttpCache
        persistentPermissionsPolicy: WebEngineProfile.AskEveryTime
    }

    VersioningArchiveInterceptor {
        id: archiveInterceptor
    }

    Connections {
        target: archiveProfile
        function onDownloadRequested(download) {
            download.cancel()
        }
    }

    WebEngineView {
        id: archiveView
        anchors.fill: parent
        profile: archiveProfile
        activeFocusOnPress: true
        // Keep keyboard navigation inside the archived document after the
        // authorised local representation loads, without enabling its scripts.
        settings.focusOnNavigationEnabled: true
        settings.javascriptEnabled: false
        settings.javascriptCanOpenWindows: false
        settings.pluginsEnabled: false
        settings.localContentCanAccessRemoteUrls: false
        settings.localContentCanAccessFileUrls: false
        settings.dnsPrefetchEnabled: false
        settings.hyperlinkAuditingEnabled: false
        settings.localStorageEnabled: false
        settings.webGLEnabled: false
        settings.pdfViewerEnabled: false
        settings.fullScreenSupportEnabled: false
        settings.navigateOnDropEnabled: false

        onLoadingChanged: function(request) {
            if (archiveHost.closingToBlank) {
                if (request.status === WebEngineView.LoadSucceededStatus
                        && request.url.toString() === "about:blank") {
                    archiveHost.closingToBlank = false
                    archiveHost.blankWaitExpired = false
                    blankingTimer.stop()
                    archiveView.visible = false
                    archiveHost.archiveBlanked()
                }
                return
            }
            if (request.status === WebEngineView.LoadSucceededStatus
                    && archiveHost.isExactAuthorizedArchiveUrl(request.url)) {
                archiveHost.archiveLoaded(archiveHost.archiveTitle)
            } else if (request.status === WebEngineView.LoadFailedStatus) {
                archiveHost.archiveLoadFailed()
                archiveHost.closeArchive()
            }
        }

        onNavigationRequested: function(request) {
            if (archiveHost.closingToBlank && request.isMainFrame
                    && request.url.toString() === "about:blank") {
                return
            }
            if (archiveHost.initialNavigationPending
                    && request.isMainFrame
                    && archiveHost.isExactAuthorizedArchiveUrl(request.url)
                    && !request.hasFormData) {
                archiveHost.initialNavigationPending = false
                return
            }
            archiveHost.rejectedNavigations++
            request.action = WebEngineNavigationRequest.IgnoreRequest
        }

        onNewWindowRequested: function(request) {
            // Not accepting the public request leaves it denied by WebEngine.
            archiveHost.rejectedNavigations++
        }

        onPermissionRequested: function(permission) {
            permission.deny()
        }

        onFileDialogRequested: function(request) {
            request.accepted = true
            request.dialogReject()
        }

        onAuthenticationDialogRequested: function(request) {
            request.accepted = true
            request.dialogReject()
        }

        onFullScreenRequested: function(request) {
            request.reject()
        }
    }

    function openAuthorizedArchive() {
        // The host may have queued this call just before shutdown began.
        // Never let a deferred open re-arm the viewer after blanking starts.
        if (closingToBlank)
            return false
        if (!archiveReady || authorizedArchiveUrl.length === 0
                || !archiveInterceptor.authorizeArchiveUrl(authorizedArchiveUrl)) {
            closeArchive()
            archiveLoadFailed()
            return false
        }
        archiveHost.closingToBlank = false
        initialNavigationPending = true
        archiveView.visible = true
        archiveView.url = authorizedArchiveUrl
        return true
    }

    function isExactAuthorizedArchiveUrl(candidate) {
        try {
            return new URL(String(candidate)).href
                    === new URL(authorizedArchiveUrl).href
        } catch (error) {
            return false
        }
    }

    function closeArchive() {
        if (closingToBlank && !blankWaitExpired)
            return
        blankWaitExpired = false
        initialNavigationPending = false
        closingToBlank = true
        archiveInterceptor.clearAuthorization()
        archiveView.stop()
        if (archiveView.url.toString() === "about:blank")
            archiveView.reload()
        else
            archiveView.url = "about:blank"
        blankingTimer.restart()
    }

    Component.onCompleted: {
        archiveReady = archiveProfile.offTheRecord
                && archiveInterceptor.attach(archiveProfile)
        if (!archiveReady)
            archiveLoadFailed()
    }
}
