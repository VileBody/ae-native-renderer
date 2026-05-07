/*
Single-case semitransparent red text over blue background probe.
*/

$.evalFile(new File(new File($.fileName).parent.fsName + "/p2_text_transfill_live_shared.jsxinc"));

(function buildRedA128Blue() {
    buildTransfillLiveCase({
        id: "TRFLIVE_RED_A128_BLUE",
        text: "H",
        fill: [1, 0, 0],
        opacity: 128.0 / 255.0 * 100.0,
        bg: [0, 0, 1],
        addAnimator: true
    });
})();
