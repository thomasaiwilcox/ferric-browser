.pragma library
.import QtQuick 6.0 as Quick

function scrollTo(view, position) {
    var beginning = Math.min(view.originY, view.headerItem ? view.headerItem.y : view.originY)
    var end = beginning + view.contentHeight - view.height
    // Once the final row exists, its actual bottom is more precise than the
    // ListView's average-height estimate (including fractional origin shifts).
    var last = view.count > 0 ? view.itemAtIndex(view.count - 1) : null
    if (last) end = last.mapToItem(view.contentItem, 0, last.height).y - view.height
    // The total length is an estimate while buffered rows finish layout.
    // It must not clamp a readable position in the selected row backwards.
    var row = view.currentItem
    if (row) {
        var top = row.mapToItem(view.contentItem, 0, 0).y
        beginning = Math.min(beginning, top)
        end = Math.max(end, top + row.height - view.height)
    }
    view.contentY = Math.max(beginning,
        Math.min(Math.max(beginning, end), position))
}

function polishRow(item) {
    item.ensurePolished()
    var children = item.children
    for (var i = 0; i < children.length; ++i)
        if (children[i].visible) polishRow(children[i])
    item.ensurePolished()
}

function revealSelection(view, direction) {
    var row = view.currentItem
    if (!row || view.height <= 0) return
    if (row.height <= view.height) {
        view.positionViewAtIndex(view.currentIndex, Quick.ListView.Contain)
        return
    }
    var top = row.mapToItem(view.contentItem, 0, 0).y
    var bottom = top + row.height
    // Keep the reader's position inside a row taller than the viewport.
    // Explicit page entry starts at the end from which the reader approached.
    if (direction > 0 || top > view.contentY) scrollTo(view, top)
    else if (direction < 0 || bottom < view.contentY + view.height)
        scrollTo(view, bottom - view.height)
}

function rowAt(view, index) {
    var row = view.itemAtIndex(index)
    if (!row) {
        view.positionViewAtIndex(index, Quick.ListView.Contain)
        view.forceLayout()
        row = view.itemAtIndex(index)
    }
    if (row) {
        // Newly materialized Controls and Flow/Column layouts can still have
        // provisional heights even after the ListView itself forced layout.
        polishRow(row)
        view.forceLayout()
    }
    return row
}

function polishAncestors(item) {
    if (item.parent) polishAncestors(item.parent)
    item.ensurePolished()
}

function prepareViewport(view, layoutRoot) {
    var beginning = Math.min(view.originY, view.headerItem ? view.headerItem.y : view.originY)
    var atBeginning = view.contentY <= beginning + 1
    // Status and font changes can leave the viewport at its previous size
    // until the enclosing layouts polish. Use the size the next frame shows.
    polishAncestors(view)
    if (layoutRoot) polishRow(layoutRoot)
    if (view.headerItem) polishRow(view.headerItem)
    // Parent layouts can change text widths on the first pass. Reflow their
    // children at those widths before measuring the viewport for a key press.
    if (layoutRoot) polishRow(layoutRoot)
    polishAncestors(view)
    view.cancelFlick()
    view.forceLayout()
    if (atBeginning) view.positionViewAtBeginning()
}

function page(view, direction, layoutRoot) {
    if (!view.visible || !view.enabled) return
    prepareViewport(view, layoutRoot)
    if (view.height <= 0) return
    direction = direction < 0 ? -1 : 1
    if (view.count === 0) {
        scrollTo(view, view.contentY + direction * view.height * 0.9)
        return
    }
    if (view.currentIndex < 0) view.currentIndex = 0
    var start = view.currentIndex
    if (start === 0 && view.headerItem) {
        var headerEnd = view.headerItem.y + view.headerItem.height
        if (view.contentY < headerEnd - 1
                || (direction < 0 && view.contentY <= headerEnd + 1)) {
            scrollTo(view, direction < 0 ? view.contentY - view.height * 0.9
                : Math.min(headerEnd, view.contentY + view.height * 0.9))
            return
        }
    }
    var row = rowAt(view, start)
    if (!row) return
    revealSelection(view, 0)
    var top = row.mapToItem(view.contentItem, 0, 0).y
    var end = top + row.height - view.height
    if (row.height > view.height) {
        // Page through expanded details before moving to another result.
        if (direction > 0 && view.contentY < end - 1) {
            scrollTo(view, Math.min(end, view.contentY + view.height * 0.9))
            return
        }
        if (direction < 0 && view.contentY > top + 1) {
            scrollTo(view, Math.max(top, view.contentY - view.height * 0.9))
            return
        }
    }
    var next = start
    var distance = 0
    while (next + direction >= 0 && next + direction < view.count) {
        var candidate = next + direction
        var candidateRow = rowAt(view, candidate)
        if (!candidateRow) break
        var step = (direction > 0 ? row.height : candidateRow.height) + view.spacing
        if (distance + step > view.height && next !== start) break
        distance += step
        next = candidate
        row = candidateRow
        if (distance >= view.height) break
    }
    view.currentIndex = next
    view.forceLayout()
    if (next === start) {
        scrollTo(view, view.contentY + direction * view.height * 0.9)
        return
    }
    row = rowAt(view, next)
    if (row && row.height <= view.height) {
        // Probing a candidate can move the viewport before we reject it.
        // Anchor the completed page explicitly, preserving the rows between
        // this selection and the previous page even if estimates shifted.
        view.positionViewAtIndex(next, direction > 0 ? Quick.ListView.End : Quick.ListView.Beginning)
    } else revealSelection(view, direction)
}
