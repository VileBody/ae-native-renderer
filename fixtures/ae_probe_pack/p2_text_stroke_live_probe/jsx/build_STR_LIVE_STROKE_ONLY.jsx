/*
Single-case stroke-only TXT ARE live probe.
*/

$.evalFile(new File(new File($.fileName).parent.fsName + "/p2_text_stroke_live_shared.jsxinc"));

(function buildStrokeOnly() {
    buildStrokeLiveCase({
        caseId: "STR_LIVE_STROKE_ONLY",
        layerName: "STR_LIVE_STROKE_ONLY_W",
        applyFill: false,
        applyStroke: true,
        fillColor: [0, 0, 0],
        strokeColor: [1, 0, 0],
        strokeWidth: 14,
        strokeOverFill: true,
        fileName: "STR_LIVE_STROKE_ONLY_[#####].png"
    });
})();
