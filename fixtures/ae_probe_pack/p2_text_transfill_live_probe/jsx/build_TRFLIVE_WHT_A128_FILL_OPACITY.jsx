/*
Single-case semitransparent white text fill probe.
*/

$.evalFile(new File(new File($.fileName).parent.fsName + "/p2_text_transfill_live_shared.jsxinc"));

(function buildWhiteA128FillOpacity() {
    buildTransfillLiveCase({
        id: "TRFLIVE_WHT_A128_FILL_OPACITY",
        text: "H",
        fill: [1, 1, 1],
        opacity: 128.0 / 255.0 * 100.0,
        bg: null,
        addAnimator: true
    });
})();
