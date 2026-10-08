import QtQuick
import QtTest
import "../../qml/scripts/ProfileDownloadRouting.js" as ProfileDownloadRouting

TestCase {
    name: "ProfileDownloadRouting"
    when: windowShown
    width: 320
    height: 240

    function source(view) {
        return {
            tabViews: [view],
            activeDownloads: ({}),
            pendingDownloadRequests: ({}),
            downloadCompletionRequests: ({}),
            nativePageCaptureSave: null
        }
    }

    function owner(download, view, sourceHost, restoredHost,
                   browserEntries, popupEntries, profile) {
        return ProfileDownloadRouting.resolve(
            download, view, sourceHost, restoredHost,
            browserEntries, popupEntries, profile)
    }

    function test_source_view_and_retained_request() {
        var profile = {}
        var view = {}
        var host = source(view)
        var download = { id: 7 }
        compare(owner(download, view, host, null, [], [], profile), host)
        host.activeDownloads["7"] = download
        compare(owner(download, null, host, null, [], [], profile), host)
        compare(owner({ id: 7 }, null, host, null, [], [], profile), null)
        var late = { id: 8 }
        Object.defineProperty(late, "view", { get: function() {
            throw new Error("destroyed view getter must not run")
        } })
        host.activeDownloads["8"] = late
        compare(owner(late, null, host, null, [], [], profile), host)
        compare(owner({ id: 8 }, null, host, null, [], [], profile), null)
    }

    function test_finished_request_stays_outstanding_until_profile_callback_is_handled() {
        var finishedBeforeSignalDelivery = { id: 19, isFinished: true }
        var retained = { "19": finishedBeforeSignalDelivery }
        verify(ProfileDownloadRouting.hasOutstandingDownloadRequests(retained))

        var profile = {}
        var host = source({})
        host.downloadCompletionRequests["19"] = finishedBeforeSignalDelivery
        compare(owner(finishedBeforeSignalDelivery, null, host, null, [], [], profile), host)

        delete retained["19"]
        delete host.downloadCompletionRequests["19"]
        verify(!ProfileDownloadRouting.hasOutstandingDownloadRequests(retained))
        compare(owner(finishedBeforeSignalDelivery, null, host, null, [], [], profile), null)
    }

    function test_shutdown_treats_unknown_download_ledgers_as_outstanding() {
        verify(!ProfileDownloadRouting.hasOutstandingDownloadRequests({}))
        verify(ProfileDownloadRouting.hasOutstandingDownloadRequests(null))
        verify(ProfileDownloadRouting.hasOutstandingDownloadRequests([]))
        verify(ProfileDownloadRouting.hasOutstandingDownloadRequests(7))
        verify(ProfileDownloadRouting.hasOutstandingDownloadRequests({ "17": false }))

        var throwingLedger = {}
        Object.defineProperty(throwingLedger, "17", {
            enumerable: true,
            get: function() { throw new Error("terminal owner lookup failed") }
        })
        verify(ProfileDownloadRouting.hasOutstandingDownloadRequests(throwingLedger))
    }

    function test_ordinary_request_is_retained_before_shutdown_cancellation() {
        var request = { id: 20, isSavePageDownload: false, cancellations: 0 }
        Object.defineProperty(request, "view", { get: function() {
            throw new Error("shutdown cancellation must not inspect Qt's view getter")
        } })
        var host = source({})
        host.shutdownApproved = true
        var ownerObservedDuringCancel = null

        verify(ProfileDownloadRouting.retainAndCancelIfShutdownApproved(
                   host, request, function(download) {
                       ownerObservedDuringCancel =
                               host.downloadCompletionRequests["20"]
                       compare(download, request)
                       download.cancellations += 1
                   }))
        compare(ownerObservedDuringCancel, request)
        compare(host.downloadCompletionRequests["20"], request)
        compare(request.cancellations, 1)

        var secondary = source({})
        secondary.windowShutdownApproved = true
        var secondaryRequest = { id: 22, cancellations: 0 }
        verify(ProfileDownloadRouting.retainAndCancelIfShutdownApproved(
                   secondary, secondaryRequest, function(download) {
                       compare(secondary.downloadCompletionRequests["22"], download)
                       download.cancellations += 1
                   }))
        compare(secondaryRequest.cancellations, 1)
    }

    function test_live_ordinary_request_is_retained_without_being_cancelled() {
        var request = { id: 21, cancellations: 0 }
        var host = source({})
        var cancelled = false

        verify(!ProfileDownloadRouting.retainAndCancelIfShutdownApproved(
                   host, request, function() { cancelled = true }))
        compare(host.downloadCompletionRequests["21"], request)
        verify(!cancelled)
        verify(ProfileDownloadRouting.retainAndCancelIfShutdownApproved(
                   host, request, function() { cancelled = true }))
        compare(host.downloadCompletionRequests["21"], request)
        verify(!cancelled)
    }

    function test_shutdown_retention_refuses_collisions_and_malformed_ledgers() {
        var original = { id: 23 }
        var replacement = { id: 23, cancellations: 0 }
        var collidingHost = source({})
        collidingHost.downloadCompletionRequests["23"] = original
        verify(ProfileDownloadRouting.retainAndCancelIfShutdownApproved(
                   collidingHost, replacement, function(download) {
                       download.cancellations += 1
                   }))
        compare(collidingHost.downloadCompletionRequests["23"], original)
        compare(replacement.cancellations, 1)

        var malformedHost = source({})
        malformedHost.downloadCompletionRequests = []
        var malformedRequest = { id: 24, cancellations: 0 }
        verify(ProfileDownloadRouting.retainAndCancelIfShutdownApproved(
                   malformedHost, malformedRequest, function(download) {
                       download.cancellations += 1
                   }))
        compare(malformedRequest.cancellations, 1)

        var throwingHost = source({})
        Object.defineProperty(throwingHost, "downloadCompletionRequests", {
            get: function() { throw new Error("owner ledger unavailable") }
        })
        var throwingRequest = { id: 25, cancellations: 0 }
        verify(ProfileDownloadRouting.retainAndCancelIfShutdownApproved(
                   throwingHost, throwingRequest, function(download) {
                       download.cancellations += 1
                   }))
        compare(throwingRequest.cancellations, 1)

        var invalidIdRequest = { id: "18/../../escape", cancellations: 0 }
        verify(ProfileDownloadRouting.retainAndCancelIfShutdownApproved(
                   source({}), invalidIdRequest, function(download) {
                       download.cancellations += 1
                   }))
        compare(invalidIdRequest.cancellations, 1)
    }

    function test_shutdown_retention_catches_ledger_write_failure() {
        var host = source({})
        Object.defineProperty(host.downloadCompletionRequests, "26", {
            set: function() { throw new Error("owner ledger is read-only") }
        })
        var request = { id: 26, cancellations: 0 }
        verify(ProfileDownloadRouting.retainAndCancelIfShutdownApproved(
                   host, request, function(download) {
                       download.cancellations += 1
                   }))
        compare(request.cancellations, 1)

        var shutdownStateHost = source({})
        Object.defineProperty(shutdownStateHost, "shutdownApproved", {
            get: function() { throw new Error("shutdown state unavailable") }
        })
        var shutdownStateRequest = { id: 27, cancellations: 0 }
        verify(ProfileDownloadRouting.retainAndCancelIfShutdownApproved(
                   shutdownStateHost, shutdownStateRequest, function(download) {
                       download.cancellations += 1
                   }))
        compare(shutdownStateHost.downloadCompletionRequests["27"], shutdownStateRequest)
        compare(shutdownStateRequest.cancellations, 1)
    }

    function test_shutdown_cancellation_falls_back_when_owner_cancel_fails() {
        var successful = source({})
        successful.shutdownApproved = true
        var successfulCancelCalls = 0
        var successfulRequest = { id: 30, cancel: function() {
            ++successfulCancelCalls
        } }
        verify(ProfileDownloadRouting.retainAndCancelIfShutdownApproved(
                   successful, successfulRequest, function() { return true }))
        compare(successfulCancelCalls, 0)

        var rejected = source({})
        rejected.shutdownApproved = true
        var rejectedCancelCalls = 0
        var rejectedRequest = { id: 28, cancel: function() {
            ++rejectedCancelCalls
        } }
        verify(ProfileDownloadRouting.retainAndCancelIfShutdownApproved(
                   rejected, rejectedRequest, function() { return false }))
        compare(rejectedCancelCalls, 1)

        var throwing = source({})
        throwing.windowShutdownApproved = true
        var throwingCancelCalls = 0
        var throwingRequest = { id: 29, cancel: function() {
            ++throwingCancelCalls
        } }
        verify(ProfileDownloadRouting.retainAndCancelIfShutdownApproved(
                   throwing, throwingRequest, function() {
                       throw new Error("owner cancellation failed")
                   }))
        compare(throwingCancelCalls, 1)
    }

    function test_viewless_ordinary_download_uses_only_its_bound_profile_owner() {
        var profile = {}
        var host = source({})
        host.browserProfile = profile
        var download = { id: 18, isSavePageDownload: false }
        Object.defineProperty(download, "view", { get: function() {
            throw new Error("ordinary profile routing must not inspect Qt's view getter")
        } })
        var exactOwner = owner(download, null, host, null, [], [], profile)
        if (exactOwner === null)
            exactOwner = ProfileDownloadRouting.boundProfileOwner(
                        download, host, profile)
        compare(exactOwner, host)
        compare(ProfileDownloadRouting.boundProfileOwner(download, host, {}), null)
        compare(ProfileDownloadRouting.boundProfileOwner(
                    { isSavePageDownload: true }, host, profile), null)
        compare(ProfileDownloadRouting.boundProfileOwner(
                    { isSavePageDownload: true }, host, profile, true), host)

        var unboundHost = source({})
        compare(ProfileDownloadRouting.boundProfileOwner(
                    download, unboundHost, profile), null)
    }

    function test_authorized_page_save_routes_by_exact_private_path_without_view_getter() {
        var profile = {}
        var view = {}
        var host = source(view)
        host.browserProfile = profile
        var path = "/private/staging/one-use/capture.mhtml"
        host.nativePageCaptureSave = { profile: profile, path: path }
        var download = {
            id: 81,
            isSavePageDownload: true,
            downloadDirectory: "/private/staging/one-use",
            downloadFileName: "capture.mhtml"
        }
        Object.defineProperty(download, "view", { get: function() {
            throw new Error("authorized path routing must not inspect Qt's view getter")
        } })
        compare(owner(download, null, host, null, [], [], profile), host)

        var otherProfile = {}
        host.nativePageCaptureSave.profile = otherProfile
        compare(owner(download, null, host, null, [], [], profile), null)
    }

    function test_authorized_page_save_path_conflict_is_ambiguous() {
        var profile = {}
        var sourceHost = source({})
        sourceHost.browserProfile = profile
        var path = "/private/staging/one-use/capture.mhtml"
        sourceHost.nativePageCaptureSave = { profile: profile, path: path }
        var first = { browserProfile: profile, nativePageCaptureSave: {
            profile: profile, path: path
        } }
        var second = { browserProfile: profile, nativePageCaptureSave: {
            profile: profile, path: path
        } }
        var download = {
            id: 82,
            isSavePageDownload: true,
            downloadDirectory: "/private/staging/one-use",
            downloadFileName: "capture.mhtml"
        }
        compare(owner(download, null, sourceHost, null,
                      [{ host: first, view: {}, profile: profile },
                       { host: second, view: {}, profile: profile }], [], profile), undefined)
    }

    function test_same_profile_secondary_and_popup_route_exactly() {
        var profile = {}
        var otherProfile = {}
        var host = source({})
        var secondaryView = {}
        var secondary = {
            activeView: secondaryView,
            activeDownloads: ({}),
            pendingDownloadRequests: ({}),
            downloadCompletionRequests: ({})
        }
        var popupView = {}
        var popup = {
            popupProfile: profile,
            activeDownloads: ({}),
            pendingDownloadRequests: ({}),
            downloadCompletionRequests: ({})
        }
        var secondaryEntry = { host: secondary, view: secondaryView, profile: profile }
        var popupEntry = { host: popup, view: popupView }
        var download = { id: 9 }
        compare(owner(download, secondaryView, host, null,
                      [secondaryEntry], [popupEntry], profile), secondary)
        compare(owner(download, popupView, host, null,
                      [secondaryEntry], [popupEntry], profile), popup)
        secondaryEntry.profile = otherProfile
        compare(owner(download, secondaryView, host, null,
                      [secondaryEntry], [popupEntry], profile), null)
    }

    function test_popup_records_are_unique_and_snapshotted_once() {
        var profile = {}
        var sourceHost = source({})
        var popupView = {}
        var hostReads = 0
        var viewReads = 0
        var profileReads = 0
        var popupHost = {
            activeDownloads: ({}),
            pendingDownloadRequests: ({}),
            downloadCompletionRequests: ({})
        }
        Object.defineProperty(popupHost, "popupProfile", { get: function() {
            profileReads += 1
            return profile
        } })
        var popupEntry = {}
        Object.defineProperty(popupEntry, "host", { get: function() {
            hostReads += 1
            return popupHost
        } })
        Object.defineProperty(popupEntry, "view", { get: function() {
            viewReads += 1
            return popupView
        } })

        compare(owner({ id: 91 }, popupView, sourceHost, null,
                      [], [popupEntry], profile), popupHost)
        compare(hostReads, 1)
        compare(viewReads, 1)
        compare(profileReads, 1)

        verify(owner({ id: 92 }, null, sourceHost, null,
                     [], [{ host: popupHost }, { host: popupHost }], profile)
               === undefined)
    }

    function test_throwing_popup_ownership_getter_is_unknown() {
        var profile = {}
        var popupHost = { activeDownloads: ({}), pendingDownloadRequests: ({}),
                          downloadCompletionRequests: ({}) }
        Object.defineProperty(popupHost, "popupProfile", { get: function() {
            throw new Error("popup profile ownership is unavailable")
        } })
        verify(owner({ id: 93 }, null, source({}), null, [],
                     [{ host: popupHost, view: null }], profile) === undefined)

        var inaccessibleHost = { popupProfile: profile }
        Object.defineProperty(inaccessibleHost, "activeDownloads", { get: function() {
            throw new Error("popup request ownership is unavailable")
        } })
        verify(owner({ id: 94 }, null, source({}), null, [],
                     [{ host: inaccessibleHost, view: null }], profile) === undefined)
    }

    function test_malformed_or_unstable_owner_indexes_are_unknown() {
        var profile = {}
        var requestView = {}
        var host = source(requestView)
        var request = { id: 941 }

        host.tabViews = null
        verify(owner(request, requestView, host, null, [], [], profile) === undefined)
        host.tabViews = [requestView]

        host.pendingDownloadRequests = null
        verify(owner(request, requestView, host, null, [], [], profile) === undefined)

        host.pendingDownloadRequests = ({})
        Object.defineProperty(host, "activeDownloads", { configurable: true,
            get: function() { return null } })
        verify(owner(request, requestView, host, null, [], [], profile) === undefined)

        var exactMap = ({})
        var reads = 0
        Object.defineProperty(host, "activeDownloads", { configurable: true,
            get: function() {
                reads += 1
                return exactMap
            } })
        var exactPendingMap = host.pendingDownloadRequests
        var exactCompletionMap = host.downloadCompletionRequests
        var viewReads = 0
        var pendingReads = 0
        var completionReads = 0
        var captureReads = 0
        Object.defineProperty(host, "tabViews", { configurable: true,
            get: function() {
                viewReads += 1
                return [requestView]
            } })
        Object.defineProperty(host, "pendingDownloadRequests", { configurable: true,
            get: function() {
                pendingReads += 1
                return exactPendingMap
            } })
        Object.defineProperty(host, "downloadCompletionRequests", { configurable: true,
            get: function() {
                completionReads += 1
                return exactCompletionMap
            } })
        Object.defineProperty(host, "nativePageCaptureSave", { configurable: true,
            get: function() {
                captureReads += 1
                return null
            } })
        compare(owner(request, requestView, host, null, [], [], profile), host)
        compare(reads, 1)
        compare(viewReads, 1)
        compare(pendingReads, 1)
        compare(completionReads, 1)
        compare(captureReads, 1)

        var unstableRequest = { id: 942 }
        Object.defineProperty(exactMap, "942", { get: function() {
            throw new Error("request identity index is unavailable")
        } })
        verify(owner(unstableRequest, null, host, null, [], [], profile) === undefined)

        var browserView = {}
        var malformedBrowser = {
            activeDownloads: [],
            pendingDownloadRequests: ({}),
            downloadCompletionRequests: ({})
        }
        verify(owner({ id: 943 }, browserView, host, null,
                     [{ host: malformedBrowser, view: browserView, profile: profile }],
                     [], profile) === undefined)

        var inheritedRequest = { id: "944" }
        host.activeDownloads = Object.create({ "944": inheritedRequest })
        compare(owner(inheritedRequest, null, host, null, [], [], profile), null)
    }

    function test_browser_owner_records_must_be_stable_and_unique() {
        var profile = {}
        var sourceHost = source({})
        var browserHost = {
            activeDownloads: ({}),
            pendingDownloadRequests: ({}),
            downloadCompletionRequests: ({})
        }
        var browserView = {}
        var hostReads = 0
        var browserEntry = { profile: profile, view: browserView }
        Object.defineProperty(browserEntry, "host", { get: function() {
            hostReads += 1
            if (hostReads > 1)
                throw new Error("browser owner host must be snapshotted once")
            return browserHost
        } })
        compare(owner({ id: 95 }, browserView, sourceHost, null,
                      [browserEntry], [], profile), browserHost)
        compare(hostReads, 1)

        verify(owner({ id: 96 }, null, sourceHost, null,
                     [{ host: browserHost, profile: profile },
                      { host: browserHost, profile: profile }], [], profile)
               === undefined)
        verify(owner({ id: 97 }, null, sourceHost, null,
                     [null], [], profile) === undefined)
        browserHost.popupProfile = profile
        verify(owner({ id: 98 }, null, sourceHost, null,
                     [{ host: browserHost, profile: profile }],
                     [{ host: browserHost, view: null }], profile) === undefined)
    }

    function test_restore_owner_and_duplicate_claim_refuse() {
        var profile = {}
        var view = {}
        var host = source({})
        var restore = { browserProfile: profile }
        var download = { id: 11 }
        compare(owner(download, view, host, restore, [], [], profile), restore)
        var popup = { popupProfile: profile }
        verify(owner(download, view, host, restore, [],
                     [{ host: popup, view: view }], profile) === undefined)
        restore.browserProfile = {}
        verify(owner(download, view, host, restore, [], [], profile) === undefined)
    }

    function test_unknown_or_oversized_registry_is_not_assigned_to_source() {
        var profile = {}
        var host = source({})
        var download = { id: 13 }
        compare(owner(download, {}, host, null, [], [], profile), null)
        var entries = []
        for (var i = 0; i < 65; ++i)
            entries.push({ host: {}, profile: profile, view: {} })
        verify(owner(download, null, host, null, entries, [], profile)
               === undefined)
    }

    function test_live_request_lookup_rejects_malformed_and_duplicate_owners() {
        var request = { id: 71 }
        var sourceHost = source({})
        var profile = {}
        sourceHost.browserProfile = profile
        sourceHost.activeDownloads["71"] = request
        compare(ProfileDownloadRouting.liveRequestForId(
                    71, sourceHost, [], [], profile), request)
        compare(ProfileDownloadRouting.liveRequestForId(
                    71, sourceHost, [{ host: sourceHost, profile: profile }], [], profile), request)
        verify(ProfileDownloadRouting.liveRequestForId(
                   71, sourceHost,
                   [{ host: sourceHost, profile: profile },
                    { host: sourceHost, profile: profile }], [], profile) === undefined)
        compare(ProfileDownloadRouting.liveRequestForId(
                    72, sourceHost, [], [], profile), null)

        sourceHost.pendingDownloadRequests = null
        verify(ProfileDownloadRouting.liveRequestForId(
                   71, sourceHost, [], [], profile) === undefined)
        sourceHost.pendingDownloadRequests = ({})

        var secondRequest = { id: 71 }
        var secondHost = source({})
        secondHost.browserProfile = profile
        secondHost.activeDownloads["71"] = secondRequest
        verify(ProfileDownloadRouting.liveRequestForId(
                   71, sourceHost, [{ host: secondHost, profile: profile }], [], profile)
               === undefined)

        sourceHost.activeDownloads = ({})
        compare(ProfileDownloadRouting.liveRequestForId(
                    71, sourceHost, [{ host: secondHost, profile: profile }], [], profile),
                secondRequest)
        verify(ProfileDownloadRouting.liveRequestForId(
                   71, sourceHost,
                   [{ host: secondHost, profile: profile },
                    { host: secondHost, profile: profile }], [], profile)
               === undefined)

        var popupHost = source({})
        popupHost.popupProfile = profile
        popupHost.pendingDownloadRequests["71"] = { id: 71 }
        verify(ProfileDownloadRouting.liveRequestForId(
                   71, sourceHost, [{ host: secondHost, profile: profile }],
                   [{ host: popupHost }], profile)
               === undefined)
        popupHost.downloadCompletionRequests = []
        verify(ProfileDownloadRouting.liveRequestForId(
                   71, sourceHost, [], [{ host: popupHost }], profile) === undefined)

        popupHost.pendingDownloadRequests = ({})
        popupHost.downloadCompletionRequests = ({})
        popupHost.activeDownloads["71"] = "not-a-request-object"
        verify(ProfileDownloadRouting.liveRequestForId(
                   71, sourceHost, [], [{ host: popupHost }], profile) === undefined)

        var otherProfileHost = source({})
        otherProfileHost.browserProfile = {}
        otherProfileHost.activeDownloads["71"] = { id: 71 }
        compare(ProfileDownloadRouting.liveRequestForId(
                    71, sourceHost,
                    [{ host: otherProfileHost, profile: otherProfileHost.browserProfile }],
                    [], profile), null)
    }

    function test_exact_request_owner_lookup_separates_no_match_from_unknown() {
        var profile = {}
        var otherProfile = {}
        var secondary = source({})
        secondary.browserProfile = profile
        var request = { id: 81 }
        secondary.activeDownloads["81"] = request
        compare(ProfileDownloadRouting.requestOwnerForExactDownload(
                    request, secondary, [], profile), secondary)

        var sameProfilePopup = source({})
        sameProfilePopup.popupProfile = profile
        sameProfilePopup.pendingDownloadRequests["81"] = request
        verify(ProfileDownloadRouting.requestOwnerForExactDownload(
                   request, secondary, [sameProfilePopup], profile) === undefined)

        sameProfilePopup.pendingDownloadRequests = ({})
        sameProfilePopup.activeDownloads["81"] = { id: 81 }
        verify(ProfileDownloadRouting.requestOwnerForExactDownload(
                   request, secondary, [sameProfilePopup], profile) === undefined)

        secondary.activeDownloads = ({})
        sameProfilePopup.activeDownloads["81"] = request
        compare(ProfileDownloadRouting.requestOwnerForExactDownload(
                    request, secondary, [sameProfilePopup], profile), sameProfilePopup)

        var otherProfilePopup = source({})
        otherProfilePopup.popupProfile = otherProfile
        otherProfilePopup.activeDownloads["81"] = { id: 81 }
        compare(ProfileDownloadRouting.requestOwnerForExactDownload(
                    request, secondary, [otherProfilePopup], profile), null)

        secondary.pendingDownloadRequests = null
        verify(ProfileDownloadRouting.requestOwnerForExactDownload(
                   request, secondary, [], profile) === undefined)
        secondary.pendingDownloadRequests = ({})
        verify(ProfileDownloadRouting.requestOwnerForExactDownload(
                   request, secondary, [sameProfilePopup, sameProfilePopup], profile)
               === undefined)
        compare(ProfileDownloadRouting.requestOwnerForExactDownload(
                    request, secondary, [], profile), null)
    }
}
