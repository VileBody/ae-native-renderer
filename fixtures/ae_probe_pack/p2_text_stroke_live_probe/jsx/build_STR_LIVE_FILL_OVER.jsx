/*
Single-case fill-over-stroke TXT ARE live probe.
*/

$.evalFile(new File(new File($.fileName).parent.fsName + "/p2_text_stroke_live_shared.jsxinc"));

(function buildFillOverStroke() {
    buildStrokeLiveCase({
        caseId: "STR_LIVE_FILL_OVER",
        layerName: "STR_LIVE_FILL_OVER_W",
        applyFill: true,
        applyStroke: true,
        fillColor: [0, 1, 0],
        strokeColor: [1, 0, 0],
        strokeWidth: 14,
        strokeOverFill: false,
        fileName: "STR_LIVE_FILL_OVER_[#####].png"
    });
})();
