import QtQuick

// One visual/input ordering for browser-owned modal surfaces in one window.
QtObject {
    id: controller
    required property var hostWindow
    property bool acceptingSurfaces: true
    Component.onDestruction: acceptingSurfaces = false
    property var surfaces: []
    readonly property var activeSurface: {
        if (!acceptingSurfaces)
            return null;
        var selected = null;
        var selectedPath = [];
        for (var index = 0; index < surfaces.length; ++index) {
            var candidate = surfaces[index];
            if (!candidate || !candidate.wantsInput || candidate.hostWindow !== hostWindow)
                continue;
            var path = paintPath(candidate);
            if (path.length === 0)
                continue;
            if (!selected || paintedAfter(path, selectedPath)) {
                selected = candidate;
                selectedPath = path;
            }
        }
        return selected;
    }
    function registerSurface(surface) {
        if (!acceptingSurfaces)
            return;
        if (surfaces.indexOf(surface) < 0)
            surfaces = surfaces.concat([surface]);
    }
    function unregisterSurface(surface) {
        surfaces = surfaces.filter(function (entry) {
            return !!entry && entry !== surface;
        });
    }
    function paintPath(surface) {
        var path = [];
        var item = surface;
        for (var depth = 0; item && item !== hostWindow.contentItem && depth < 64; ++depth) {
            var parent = item.parent;
            if (!parent)
                return [];
            var siblings = parent.children;
            var order = 0;
            for (var index = 0; index < siblings.length; ++index) {
                if (siblings[index] === item) {
                    order = index;
                    break;
                }
            }
            path.unshift({
                item: item,
                z: Number(item.z) || 0,
                order: order
            });
            item = parent;
        }
        return item === hostWindow.contentItem ? path : [];
    }
    function paintedAfter(left, right) {
        var count = Math.min(left.length, right.length);
        for (var index = 0; index < count; ++index) {
            if (left[index].item === right[index].item)
                continue;
            if (left[index].z !== right[index].z)
                return left[index].z > right[index].z;
            return left[index].order > right[index].order;
        }
        return left.length >= right.length;
    }
    function allowsFocus(item) {
        var active = activeSurface;
        if (!active)
            return true;
        while (item && item !== active)
            item = item.parent;
        return item === active;
    }
}
