import qs.modules.common
import QtQuick

/**
 * Curve over the last seven days: a solid line above a gradient fill.
 *
 * The series arrives already normalised within [0, 1] and downsampled by the
 * binary; the canvas merely scales it, with no minimum or maximum to compute
 * on every repaint.
 */
Canvas {
    id: root

    /// Points within [0, 1], oldest to newest.
    property var series: []
    property color stroke: Appearance.colors.colOnLayer1
    property real thickness: 1.5
    /// Top and bottom margin, so the line is not clipped.
    property real inset: root.thickness

    onSeriesChanged: root.requestPaint()
    onStrokeChanged: root.requestPaint()
    onWidthChanged: root.requestPaint()
    onHeightChanged: root.requestPaint()

    onPaint: {
        const ctx = root.getContext("2d");
        ctx.reset();
        const points = root.series;
        if (!points || points.length < 2)
            return;

        const usableHeight = Math.max(0, root.height - root.inset * 2);
        const step = root.width / (points.length - 1);
        const yOf = value => root.inset + (1 - value) * usableHeight;

        ctx.beginPath();
        ctx.moveTo(0, yOf(points[0]));
        for (let i = 1; i < points.length; i++)
            ctx.lineTo(i * step, yOf(points[i]));

        // The fill reuses the path, closed onto the baseline.
        ctx.lineTo(root.width, root.height);
        ctx.lineTo(0, root.height);
        ctx.closePath();
        const gradient = ctx.createLinearGradient(0, 0, 0, root.height);
        gradient.addColorStop(0, Qt.alpha(root.stroke, 0.28));
        gradient.addColorStop(1, Qt.alpha(root.stroke, 0));
        ctx.fillStyle = gradient;
        ctx.fill();

        ctx.beginPath();
        ctx.moveTo(0, yOf(points[0]));
        for (let i = 1; i < points.length; i++)
            ctx.lineTo(i * step, yOf(points[i]));
        ctx.lineWidth = root.thickness;
        ctx.lineJoin = "round";
        ctx.lineCap = "round";
        ctx.strokeStyle = root.stroke;
        ctx.stroke();
    }
}
