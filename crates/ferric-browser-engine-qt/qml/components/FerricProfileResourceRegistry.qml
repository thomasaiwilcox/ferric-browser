import QtQuick
import QtWebEngine
import io.github.ferricbrowser 1.0

// Native profile/view lifetime and signal dispatch. The owning BrowserUi keeps
// command policy, permissions, download decisions and persistent metadata.
QtObject {
    id: registry
    required property var rootWindow
    property var groups: []
    property var downloadOwners: []
    property FerricDownloadSource downloadSource: FerricDownloadSource {}
    property Timer finalizationPoll: Timer {
        interval: 50
        repeat: true
        running: registry.downloadOwners.some(function(entry) { return entry.finishing === true })
        onTriggered: {
            var copy = registry.downloadOwners.slice(0)
            var polled = []
            for (var i = 0; i < copy.length; ++i) {
                var owner = copy[i]
                if (!owner.finishing || !owner.ui) continue
                if (polled.indexOf(owner.ui) < 0) {
                    owner.ui.poll_download_finalizations()
                    polled.push(owner.ui)
                }
                if (!owner.ui.download_finalization_pending(owner.id)) {
                    if (owner.host && owner.host.runtimeReleased !== true)
                        owner.host.completeFinishedDownload(owner.id)
                    registry.downloadOwners = registry.downloadOwners.filter(function(entry) { return entry !== owner })
                }
            }
        }
    }
    property Component resourceFactory: Component {
        FerricProfileResources { rootWindow: registry.rootWindow }
    }

    function forProfile(profile) {
        return groups.find(function(group) { return group && group.profile===profile }) || null
    }

    function ensure(name, transient, base, profile, interceptor, prototype) {
        var existing=profile ? forProfile(profile) : groups.find(function(group) {
            return group && !transient && !group.transientProfile
                    && group.profileName===name && group.storageBase===base
        })
        if (existing) return existing
        var group=resourceFactory.createObject(registry.rootWindow, {
            profileName:name, transientProfile:transient, storageBase:base,
            existingProfile:profile || null, existingInterceptor:interceptor || null,
            existingPrototype:prototype || null
        })
        if (!group || !group.usable) {
            if (group) group.destroy()
            return null
        }
        groups=groups.concat([group])
        return group
    }

    function retainWindow(group, host, ui) {
        return !!group && group.lifetime.retainWindow(host,ui)
    }

    function activateWindow(host) {
        for (var i=0;i<groups.length;++i) groups[i].lifetime.activateWindow(host)
    }

    function trackView(view, host, ui) {
        if (!view || !host || !ui || host.runtimeReleased===true) return false
        if (host.profileBootstrapComplete===false) return false
        var group=forProfile(view.profile)
        return !!group && group.lifetime.retainWindow(host,ui)
                && group.lifetime.activateWindow(host)
                && group.lifetime.adoptView(view,host,ui)
    }

    function releaseWindow(host) {
        cancelDownloadsForHost(host)
        for (var i=0;i<groups.length;++i) groups[i].lifetime.releaseWindow(host)
    }

    function releaseView(view) {
        if (!view) return
        var group=forProfile(view.profile)
        if (group) group.lifetime.releaseView(view)
    }

    function release(group) {
        groups=groups.filter(function(entry) { return entry!==group })
        if (!rootWindow.runtimeDisposing) group.destroy()
    }

    function handleDownloadRequested(group, download) {
        // An admitted offer can outlive its source. Never look it up again.
        if (downloadOwners.some(function(entry) { return entry.download===download })) return
        if (!downloadSource.supported) {
            download.cancel()
            if (group.policyUi) group.policyUi.status_text="Downloads are unavailable in this build. Update or rebuild Ferric Browser, then try again."
            return
        }
        var view=downloadSource.viewFor(download)
        admitDownload(group, download, view)
    }
    function handleStreamDownloadRequested(group, view, download) {
        if (!view || !view.downloadInitiator || !view.downloadInitiator.permitsOffer(view)) return false
        return admitDownload(group, download, view)
    }
    function admitDownload(group, download, view) {
        var ui=group.lifetime.ownerUi(view)
        var host=group.lifetime.ownerHost(view)
        if (rootWindow.runtimeDisposing || !ui || !host || host.runtimeReleased===true
                || typeof host.handleDownloadRequested!=="function") {
            download.cancel()
            return
        }
        var initiator = view ? view.downloadInitiator : null
        if (initiator && !initiator.permitsOffer(view)) { download.cancel(); return }
        var id=ui.allocate_download_id()
        if (!id) { download.cancel(); if (initiator) initiator.fail(view); return }
        if (download.streamTransport === true) download.downloadId = String(id)
        downloadOwners=downloadOwners.concat([{download:download,ui:ui,host:host,id:id,profile:group.profile}])
        var sourceUrl = initiator ? initiator.sourceForOffer(view) : String(download.url)
        var safeName = ui.offer_download(id, sourceUrl, download.suggestedFileName)
        if (!safeName) {
            cancelDownloadRequest(ui, download)
            if (initiator) {
                ui.discard_download_request(view.requestToken)
                initiator.release(view)
            }
            return
        }
        if (initiator && !initiator.acceptOffer(view, download, id)) {
            cancelDownloadRequest(ui, download)
            ui.update_download(id, "cancelled", 0, true)
            return
        }
        host.handleDownloadRequested(download,id,safeName)
        return true
    }

    // A request is decided once. Its accepted transport can subsequently be
    // paused, resumed or cancelled, using the captured native-profile owner.
    function downloadInvocation(ui) {
        if (!ui) return null
        var invoking = rootWindow.liveWindowEntries().find(function(entry) { return entry.ui === ui })
        if (!invoking || !invoking.profile || invoking.host.runtimeDisposing === true
                || invoking.host.runtimeReleased === true || !ui.profile_available()) return null
        return invoking
    }
    function downloadOwnerFor(ui, id) {
        var invoking = downloadInvocation(ui)
        if (!invoking) return null
        return downloadOwners.find(function(entry) {
            return entry.id === String(id) && entry.profile === invoking.profile
                && entry.ui && entry.host && entry.host.runtimeReleased !== true
                && entry.host.runtimeDisposing !== true
        }) || null
    }
    function downloadFor(ui, id) {
        var owner = downloadOwnerFor(ui, id)
        // A finished transport may already have been destroyed. The session
        // retains its final bytes/size while the file worker prepares it.
        return owner && !owner.finishing ? owner.download : null
    }
    function controlDownload(ui, id, action) {
        if (!downloadInvocation(ui)) return false
        var owner = downloadOwnerFor(ui, id)
        if (!owner || !dispatchDownloadControl(owner, action)) {
            ui.status_text = "This download is no longer active. Refresh Downloads and try again."
            return false
        }
        return true
    }
    function cancelDownloadRequest(ui, download) {
        var owner = downloadOwners.find(function(entry) { return entry.ui === ui && entry.download === download })
        return !!owner && dispatchDownloadControl(owner, "cancel")
    }
    function dispatchDownloadControl(owner, action) {
        if (owner.finishing)
            return action === "cancel" && owner.ui.cancel_download_finalization(owner.id)
        var download = owner.download
        if (!download || download.isFinished) return false
        try {
            if (action === "cancel") {
                var accepted = (rootWindow.resolvedQtRequests || []).some(function(entry) {
                    return entry.request === download && entry.kind === "download" && entry.method === "accept"
                })
                if (download.state !== WebEngineDownloadRequest.DownloadRequested
                        && download.state !== WebEngineDownloadRequest.DownloadInProgress) return false
                if (!accepted && download.state === WebEngineDownloadRequest.DownloadRequested)
                    return rootWindow.resolveQtRequest(owner.ui, download, "download", "cancel", [])
                if (!accepted && download.state !== WebEngineDownloadRequest.DownloadInProgress) return false
                download.cancel()
            } else if (action === "pause" || action === "resume") {
                if (download.state !== WebEngineDownloadRequest.DownloadInProgress) return false
                if (action === "pause") download.pause()
                else download.resume()
            } else return false
            return true
        } catch (error) {
            if (owner.ui) owner.ui.status_text = "This download couldn't be changed. Refresh Downloads and try again."
            return false
        }
    }

    function markDownloadSelectionFailed(download) {
        var owner=downloadOwners.find(function(entry) { return entry.download===download })
        if (owner) owner.selectionFailed=true
    }

    function handleDownloadFinished(group, download) {
        var owner=downloadOwners.find(function(entry) { return entry.download===download })
        var invoking = owner ? downloadInvocation(owner.ui) : null
        if (owner && invoking && invoking.profile === owner.profile && owner.host && owner.host.runtimeReleased!==true
                && typeof owner.host.handleDownloadFinished==="function") {
            owner.ui.record_download_metrics(owner.id, Number(download.totalBytes),
                download.streamSaveFailure === true ? 0 : Number(download.interruptReason || 0), String(download.interruptReasonString || ""))
            owner.finishing = owner.host.handleDownloadFinished(download, owner.selectionFailed===true, owner.id) === true
        }
        if (!owner || !owner.finishing)
            downloadOwners=downloadOwners.filter(function(entry) { return entry.download!==download })
        else downloadOwners=downloadOwners.slice(0)
    }

    function cancelFinalizationsForHost(host) {
        downloadOwners.filter(function(entry) { return entry.host===host && entry.finishing }).forEach(function(entry) {
            entry.ui.cancel_download_finalization(entry.id)
        })
    }

    function finishingForHost(host) {
        var invoking = rootWindow.liveWindowEntries().find(function(entry) { return entry.host === host })
        if (!invoking || !invoking.ui) return {count:0,bytes:0}
        var owned = downloadOwners.filter(function(entry) { return entry.host === host && entry.finishing }).map(function(entry) { return entry.id })
        var rows = invoking.ui.download_rows().filter(function(row) { return row.active && row.finishing && owned.indexOf(String(row.id)) >= 0 })
        return {count:rows.length,bytes:rows.reduce(function(sum,row) { return sum+Math.max(0,Number(row.bytes)||0) },0)}
    }

    function cancelDownloadsForHost(host) {
        var owned=downloadOwners.filter(function(entry) { return entry.host===host })
        for (var i=0;i<owned.length;++i) {
            var entry=owned[i]
            if (!entry.ui) continue
            if (entry.finishing) {
                entry.ui.cancel_download_finalization(entry.id)
                continue
            }
            if (!entry.download) continue
            if (!entry.download.isFinished)
                dispatchDownloadControl(entry,"cancel")
            entry.ui.update_download(entry.id,"cancelled",entry.download.receivedBytes,true)
        }
        downloadOwners=downloadOwners.filter(function(entry) { return entry.host!==host })
    }

    function shutdown() {
        var copy=groups.slice(0)
        for (var i=0;i<copy.length;++i) copy[i].lifetime.shutdown()
    }
}
