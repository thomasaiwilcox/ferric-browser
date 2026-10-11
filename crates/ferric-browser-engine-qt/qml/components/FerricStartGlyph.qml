import QtQuick

Item {
    id: glyph
    property string kind: "globe"
    property color color: "#888888"
    implicitWidth: 24
    implicitHeight: 24
    Accessible.ignored: true
    onKindChanged: drawing.requestPaint()
    onColorChanged: drawing.requestPaint()
    Canvas {
        id: drawing
        anchors.fill: parent
        onPaint: {
            var ctx = getContext("2d")
            ctx.clearRect(0, 0, width, height)
            ctx.save()
            ctx.scale(width / 24, height / 24)
            ctx.strokeStyle = glyph.color
            ctx.fillStyle = glyph.color
            ctx.lineWidth = 1.5
            ctx.lineCap = "round"
            ctx.lineJoin = "round"
            ctx.beginPath()
            if (glyph.kind === "globe") {
                ctx.arc(12, 12, 8.5, 0, Math.PI * 2)
                ctx.moveTo(3.5, 12); ctx.lineTo(20.5, 12)
                ctx.moveTo(12, 3.5)
                ctx.bezierCurveTo(5.5, 8, 5.5, 16, 12, 20.5)
                ctx.moveTo(12, 3.5)
                ctx.bezierCurveTo(18.5, 8, 18.5, 16, 12, 20.5)
            } else if (glyph.kind === "plus") {
                ctx.moveTo(12, 5); ctx.lineTo(12, 19)
                ctx.moveTo(5, 12); ctx.lineTo(19, 12)
            } else if (glyph.kind === "search") {
                ctx.arc(10, 10, 6.5, 0, Math.PI * 2)
                ctx.moveTo(15, 15); ctx.lineTo(21, 21)
            } else if (glyph.kind === "back") {
                ctx.moveTo(14, 5); ctx.lineTo(7, 12); ctx.lineTo(14, 19)
            } else if (glyph.kind === "chevron") {
                ctx.moveTo(9, 6); ctx.lineTo(15, 12); ctx.lineTo(9, 18)
            } else if (glyph.kind === "close") {
                ctx.moveTo(6, 6); ctx.lineTo(18, 18)
                ctx.moveTo(18, 6); ctx.lineTo(6, 18)
            } else if (glyph.kind === "menu") {
                for (var x = 5; x <= 19; x += 7) {
                    ctx.moveTo(x + 1.3, 12); ctx.arc(x, 12, 1.3, 0, Math.PI * 2)
                }
                ctx.fill()
            }
            ctx.stroke()
            ctx.restore()
        }
    }
}
