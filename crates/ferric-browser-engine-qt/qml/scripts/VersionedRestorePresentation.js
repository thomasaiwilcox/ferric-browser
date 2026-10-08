.pragma library

// Window construction is a presentation boundary only. It receives a
// validated commit identifier and the owning window's already-selected
// normal-profile identity; failure must not mutate the source window.
function openRestoreWindow(factory, properties, ui, owner) {
    var canCreate = false
    try {
        canCreate = !!owner && typeof owner.canCreate === "function"
                && owner.canCreate() === true
    } catch (error) {
        canCreate = false
    }
    if (!canCreate) {
        if (ui)
            ui.status_text = "Restore window limit reached or owner unavailable; current work is intact"
        return null
    }
    var child = null
    try {
        if (factory && typeof factory.createObject === "function")
            child = factory.createObject(null, properties) || null
    } catch (error) {
        child = null
    }
    if (!child) {
        if (ui) {
            ui.status_text = "Could not create versioned restore window; current work is intact"
        }
        return null
    }
    var retained = false
    try {
        retained = !!owner && typeof owner.retain === "function"
                && owner.retain(child) === true
    } catch (error) {
        retained = false
    }
    if (!retained) {
        try {
            if (child && typeof child.destroy === "function")
                child.destroy()
        } catch (error) {}
        if (ui)
            ui.status_text = "Could not retain versioned restore window; current work is intact"
        return null
    }
    if (ui) {
        ui.status_text = "Opening versioned restore in a new normal-profile window"
    }
    return child
}
