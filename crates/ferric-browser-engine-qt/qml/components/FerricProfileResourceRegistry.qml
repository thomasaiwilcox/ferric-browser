import QtQuick

// Native profile/view lifetime and signal dispatch. The owning BrowserUi keeps
// command policy, permissions, download decisions and persistent metadata.
QtObject {
    id: registry
    required property var rootWindow
    property var groups: []
    property var downloadOwners: []
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
        var ui=group.lifetime.ownerUi(download.view)
        var host=group.lifetime.ownerHost(download.view)
        if (rootWindow.runtimeDisposing || !ui || !host || host.runtimeReleased===true
                || typeof host.handleDownloadRequested!=="function") {
            download.cancel()
            return
        }
        if (downloadOwners.some(function(entry) { return entry.download===download })) return
        downloadOwners=downloadOwners.concat([{download:download,ui:ui,host:host}])
        host.handleDownloadRequested(download)
    }

    function handleDownloadFinished(group, download) {
        var owner=downloadOwners.find(function(entry) { return entry.download===download })
        downloadOwners=downloadOwners.filter(function(entry) { return entry.download!==download })
        if (owner && owner.ui && owner.host && owner.host.runtimeReleased!==true
                && typeof owner.host.handleDownloadFinished==="function")
            owner.host.handleDownloadFinished(download)
    }

    function cancelDownloadsForHost(host) {
        var owned=downloadOwners.filter(function(entry) { return entry.host===host })
        downloadOwners=downloadOwners.filter(function(entry) { return entry.host!==host })
        for (var i=0;i<owned.length;++i) {
            var entry=owned[i]
            if (!entry.download || !entry.ui) continue
            if (!entry.download.isFinished)
                rootWindow.resolveQtRequest(entry.ui,entry.download,"download","cancel",[])
            entry.ui.update_download(String(entry.download.id),"cancelled",entry.download.receivedBytes,true)
        }
    }

    function shutdown() {
        var copy=groups.slice(0)
        for (var i=0;i<copy.length;++i) copy[i].lifetime.shutdown()
    }
}
