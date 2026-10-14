.pragma library

// Presentation only. Labels still resolve through the current Rust Hint session.
function identity(row) {
    return JSON.stringify([String(row.label), String(row.frame_path || ""),
                           Number(row.element_id || 0), row.related_element_id || null])
}

function coordinateScale(rendered, source) {
    var ratio = rendered / Number(source)
    return Number.isFinite(ratio) && ratio >= 0.2 && ratio <= 5 ? ratio : 1
}

function intersects(a, b) {
    return a.marker_x < b.marker_x + b.marker_width
            && a.marker_x + a.marker_width > b.marker_x
            && a.marker_y < b.marker_y + b.marker_height
            && a.marker_y + a.marker_height > b.marker_y
}

function description(row) {
    if (!row) return ""
    var kind = String(row.kind || "target").replace(/-/g, " ")
    return String(row.label) + " · " + kind + " · " + String(row.text || "Target")
            + (row.action ? " · " + String(row.action) : "")
}

function layout(values, viewportWidth, viewportHeight, sourceWidth, sourceHeight,
                scale, widths, previous) {
    if (viewportWidth <= 0 || viewportHeight <= 0) return []
    var sx = coordinateScale(viewportWidth, sourceWidth)
    var sy = coordinateScale(viewportHeight, sourceHeight)
    var rows = [], cells = Object.create(null), targetCells = Object.create(null), cellSize = Math.max(32, 48 * scale)
    function buckets(marker, callback) {
        var minX = Math.floor(marker.marker_x / cellSize)
        var maxX = Math.floor((marker.marker_x + marker.marker_width) / cellSize)
        var minY = Math.floor(marker.marker_y / cellSize)
        var maxY = Math.floor((marker.marker_y + marker.marker_height) / cellSize)
        for (var y = minY; y <= maxY; ++y)
            for (var x = minX; x <= maxX; ++x) callback(x + ":" + y)
    }
    function collisions(marker) {
        var found = Object.create(null), result = []
        buckets(marker, function(key) {
            var bucket = cells[key] || []
            for (var i = 0; i < bucket.length; ++i) {
                var other = bucket[i]
                if (!found[other.layout_key] && intersects(marker, other)) {
                    found[other.layout_key] = true
                    result.push(other)
                }
            }
        })
        return result
    }
    function coversOtherControl(marker) {
        var found = false
        buckets(marker,function(key){
            var bucket = targetCells[key] || []
            for (var i=0;i<bucket.length;++i) {
                var other = bucket[i], own = marker.fragments[0]
                // A card/region containing this control must not block its badge.
                var containsOwn = other.marker_x <= own.x && other.marker_y <= own.y
                        && other.marker_x+other.marker_width >= own.x+own.width
                        && other.marker_y+other.marker_height >= own.y+own.height
                if (other.layout_key !== marker.layout_key && !containsOwn && intersects(marker,other)) {
                    found=true;break
                }
            }
        })
        return found
    }
    function occupy(marker) {
        buckets(marker, function(key) {
            if (!cells[key]) cells[key] = []
            cells[key].push(marker)
        })
    }
    function position(row, x, y) {
        row.marker_x = Math.max(0, Math.min(Math.max(0, viewportWidth - row.marker_width), x))
        row.marker_y = Math.max(0, Math.min(Math.max(0, viewportHeight - row.marker_height), y))
    }
    function nearOwnControl(marker) {
        for (var fragment of marker.fragments) {
            var dx=Math.max(0,fragment.x-marker.marker_x-marker.marker_width,marker.marker_x-fragment.x-fragment.width)
            var dy=Math.max(0,fragment.y-marker.marker_y-marker.marker_height,marker.marker_y-fragment.y-fragment.height)
            if (dx*dx+dy*dy <= 16*scale*scale) return true
        }
        return false
    }
    for (var i = 0; i < values.length; ++i) {
        var source = values[i], row = ({})
        for (var key in source) row[key] = source[key]
        row.layout_key = identity(source)
        row.x = source.x * sx; row.y = source.y * sy
        row.width = source.width * sx; row.height = source.height * sy
        row.fragments = []
        var presentation = source.presentation
        var fragments = presentation && presentation.fragments || []
        for (var f = 0; f < fragments.length; ++f) {
            var fragment = fragments[f]
            row.fragments.push({x:fragment.x*sx,y:fragment.y*sy,
                                width:fragment.width*sx,height:fragment.height*sy})
        }
        if (!row.fragments.length) row.fragments = [{x:row.x,y:row.y,width:row.width,height:row.height}]
        row.anchor_x = presentation ? presentation.anchor.x * sx : row.fragments[0].x
        row.anchor_y = presentation ? presentation.anchor.y * sy : row.fragments[0].y
        row.document_scroll = source.kind === "scrollable" && source.text === "Page scroll"
                && row.x < 1 && row.y < 1
                && row.width >= viewportWidth * 0.9 && row.height >= viewportHeight * 0.9
        row.marker_width = Math.min(viewportWidth, Math.max(24 * scale,
                                            (widths[String(source.label)] || 18 * scale) + 14 * scale))
        if (row.document_scroll) row.marker_width = Math.min(viewportWidth, row.marker_width + 100 * scale)
        row.marker_height = Math.min(viewportHeight, 22 * scale)
        row.collision_cluster = 0
        row.cluster = null
        var old = previous && previous[row.layout_key]
        row.retained = !!old && old.width === row.marker_width && old.height === row.marker_height
        if (row.document_scroll) {
            position(row, 8 * scale, viewportHeight - row.marker_height - 8 * scale)
        } else if (row.retained) {
            position(row, row.anchor_x + old.dx, row.anchor_y + old.dy)
        } else {
            position(row, row.anchor_x, row.anchor_y)
        }
        rows.push(row)
    }
    for (var t=0;t<rows.length;++t) {
        var control=rows[t]
        if (control.document_scroll) continue
        // Protect visible fragments of titles and wide links as well as icons.
        for (var fragment of control.fragments) {
            var box={layout_key:control.layout_key,marker_x:fragment.x,marker_y:fragment.y,
                     marker_width:fragment.width,marker_height:fragment.height}
            buckets(box,function(key){if(!targetCells[key])targetCells[key]=[];targetCells[key].push(box)})
        }
    }
    // Keep surviving placements unless they now obscure a neighboring control.
    for (var r = 0; r < rows.length; ++r)
        if (rows[r].retained && !rows[r].document_scroll) {
            if (!nearOwnControl(rows[r]) || coversOtherControl(rows[r])) rows[r].retained=false
            else occupy(rows[r])
        }
    for (var n = 0; n < rows.length; ++n) {
        var marker = rows[n]
        if (marker.retained || marker.document_scroll) continue
        var h = marker.marker_height, gap = 4 * scale, fragment = marker.fragments[0]
        var small=fragment.width<marker.marker_width || fragment.height<h
        var centerY=Math.min(0,(fragment.height-h)/2)
        var options = small ? [[0,-h-gap],[0,centerY],[0,0]] : [[0,0],[0,-h-gap]]
        // Move along the owning fragment before placing a badge on another row.
        // Every candidate remains on or within one gap of its own control.
        var step=marker.marker_width+gap
        var offsets=[step,step*2,step*3,fragment.width-marker.marker_width]
        for (var offset of offsets) {
            if (offset <= 0 || offset+marker.marker_width > fragment.width) continue
            options.push([offset,centerY])
            if (small) options.push([offset,-h-gap])
        }
        options.push([0,fragment.height+gap],[fragment.width+gap,0])
        var chosen = false
        for (var o = 0; o < options.length; ++o) {
            position(marker, marker.anchor_x + options[o][0], marker.anchor_y + options[o][1])
            if (!nearOwnControl(marker)) continue
            if (!collisions(marker).length && !coversOtherControl(marker)) { chosen = true; break }
        }
        if (!chosen) position(marker, marker.anchor_x, marker.anchor_y)
        occupy(marker)
    }
    // Union actual badge collisions. Large target boxes are not collision groups.
    var parents = Object.create(null)
    function root(key) {
        while (parents[key] !== key) { parents[key] = parents[parents[key]]; key = parents[key] }
        return key
    }
    for (var p = 0; p < rows.length; ++p) parents[rows[p].layout_key] = rows[p].layout_key
    for (var c = 0; c < rows.length; ++c) {
        var current = rows[c]
        if (current.document_scroll) continue
        var others = collisions(current)
        for (var j = 0; j < others.length; ++j) {
            if (!others[j].document_scroll) parents[root(others[j].layout_key)] = root(current.layout_key)
        }
    }
    var groups = Object.create(null)
    for (var g = 0; g < rows.length; ++g) {
        var item = rows[g], groupKey = root(item.layout_key)
        if (!groups[groupKey]) groups[groupKey] = []
        groups[groupKey].push(item.label)
    }
    for (var m = 0; m < rows.length; ++m) {
        var target = rows[m]
        target.cluster_key = root(target.layout_key)
        target.cluster = groups[target.cluster_key]
        target.collision_cluster = target.cluster.length > 1 ? target.cluster.length : 0
        target.connected = !target.document_scroll && !target.fragments.some(function(fragment) {
            return target.marker_x >= fragment.x && target.marker_y >= fragment.y
                    && target.marker_x+target.marker_width <= fragment.x+fragment.width
                    && target.marker_y+target.marker_height <= fragment.y+fragment.height
        })
        // Use the nearest interior point so an edge badge has a short leader,
        // instead of drawing across the card and unrelated controls.
        var centerX=target.marker_x+target.marker_width/2,centerY=target.marker_y+target.marker_height/2,bestDistance=Infinity
        for (var visibleFragment of target.fragments) {
            var insetX=Math.min(4*scale,visibleFragment.width/2),insetY=Math.min(4*scale,visibleFragment.height/2)
            var endpointX=Math.max(visibleFragment.x+insetX,Math.min(visibleFragment.x+visibleFragment.width-insetX,centerX))
            var endpointY=Math.max(visibleFragment.y+insetY,Math.min(visibleFragment.y+visibleFragment.height-insetY,centerY))
            var distance=(centerX-endpointX)*(centerX-endpointX)+(centerY-endpointY)*(centerY-endpointY)
            if (distance < bestDistance) {
                bestDistance=distance;target.endpoint_x=endpointX;target.endpoint_y=endpointY
            }
        }
    }
    return rows
}
