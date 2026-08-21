import qs.modules.common
import QtQuick

/**
 * Courbe des sept derniers jours : trait plein sous un aplat dégradé.
 *
 * La série arrive déjà normalisée dans [0, 1] et sous-échantillonnée par le
 * binaire ; le canevas se contente de la mettre à l'échelle, sans calcul de
 * minimum ni de maximum à chaque repeinte.
 */
Canvas {
    id: root

    /// Points dans [0, 1], du plus ancien au plus récent.
    property var series: []
    property color stroke: Appearance.colors.colOnLayer1
    property real thickness: 1.5
    /// Marge haute et basse, pour que le trait ne soit pas rogné.
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

        // L'aplat réutilise le tracé, refermé sur la base.
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
