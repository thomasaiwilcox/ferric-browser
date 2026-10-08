.pragma library

// A shutdown probe is asynchronous. Capture the native view identities and
// navigation epochs so a result cannot authorize a different page or tab set.
function snapshotActive(views, activeState, allowLoading) {
    var snapshots = []
    try {
        for (var i = 0; i < views.length; ++i) {
            var view = views[i]
            if (!view || view.lifecycleState !== activeState) {
                continue
            }
            if (view.loading && !allowLoading) {
                return null
            }
            snapshots.push({
                view: view,
                navigationEpoch: Number(view.versioningNavigationEpoch),
                url: String(view.url),
                loading: !!view.loading
            })
        }
    } catch (error) {
        return null
    }
    return snapshots
}

function stillCurrent(snapshots, views, activeState, allowLoading) {
    if (!snapshots) {
        return false
    }
    var current = snapshotActive(views, activeState, allowLoading)
    if (!current || current.length !== snapshots.length) {
        return false
    }
    for (var i = 0; i < current.length; ++i) {
        if (current[i].view !== snapshots[i].view
                || current[i].navigationEpoch !== snapshots[i].navigationEpoch
                || current[i].url !== snapshots[i].url
                || current[i].loading !== snapshots[i].loading) {
            return false
        }
    }
    return true
}
