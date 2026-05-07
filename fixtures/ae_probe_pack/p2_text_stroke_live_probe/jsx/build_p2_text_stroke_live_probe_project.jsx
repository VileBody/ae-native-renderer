/*
P2-S1 TXT ARE stroke/fill live-lock probe.

Crash-safe default entrypoint: build only the fill-only control case. The
stroke variants have separate one-case entry scripts so AE never constructs the
whole risky queue before remote case filtering.
*/

$.evalFile(new File(new File($.fileName).parent.fsName + "/p2_text_stroke_live_shared.jsxinc"));

(function buildP2TextStrokeLiveProbeDefault() {
    buildStrokeLiveCase({
        caseId: "STR_LIVE_FILL_ONLY",
        layerName: "STR_LIVE_FILL_ONLY_W",
        applyFill: true,
        applyStroke: false,
        fillColor: [1, 1, 1],
        strokeColor: [1, 0, 0],
        strokeWidth: 0,
        strokeOverFill: false,
        fileName: "STR_LIVE_FILL_ONLY_[#####].png"
    });
})();
