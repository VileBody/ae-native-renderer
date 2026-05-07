/*
P2-S2 semitransparent text fill live probe.

Crash-safe default entrypoint: build only the opaque control. The alpha cases
have separate one-case entry scripts because AE crashed when one JSX built the
whole animator matrix in a single project.
*/

$.evalFile(new File(new File($.fileName).parent.fsName + "/p2_text_transfill_live_shared.jsxinc"));

(function buildP2TextTransfillLiveDefault() {
    buildTransfillLiveCase({
        id: "TRFLIVE_WHT_A255_CONTROL",
        text: "H",
        fill: [1, 1, 1],
        opacity: 100.0,
        bg: null,
        addAnimator: false
    });
})();
