/*
Single-case stroke-over-fill TXT ARE live probe.
*/

$.evalFile(new File(new File($.fileName).parent.fsName + "/p2_text_stroke_live_shared.jsxinc"));

(function buildStrokeOverFill() {
    buildStrokeLiveCase({
        caseId: "STR_LIVE_STROKE_OVER",
        layerName: "STR_LIVE_STROKE_OVER_W",
        applyFill: true,
        applyStroke: true,
        fillColor: [0, 1, 0],
        strokeColor: [1, 0, 0],
        strokeWidth: 14,
        strokeOverFill: true,
        fileName: "STR_LIVE_STROKE_OVER_[#####].png"
    });
})();
