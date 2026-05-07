/*
P2 text hinting/grid-fit/AA/subpixel probe.

Single glyphs, 8 bpc, transparent background, one frame per comp. The cases
only vary glyph size and fractional text layer position, so rendered alpha
edges can answer whether coverage is gray AA, position-quantized, or sensitive
to subpixel translation.
*/

(function buildP2TextHintingProbe() {
    app.beginUndoGroup("Build P2 Text Hinting Probe");

    var SCRIPT_FILE = new File($.fileName);
    var PACK_DIR = SCRIPT_FILE.parent.parent;

    if (!app.project) {
        app.newProject();
    }
    app.project.bitsPerChannel = 8;
    try {
        if (typeof PurgeTarget !== "undefined") {
            app.purge(PurgeTarget.ALL_CACHES);
        }
    } catch (_purgeErr) {
    }

    var cases = [
        { id: "HNT_010", glyph: "I", size: 12, pos: [64.00, 76.00] },
        { id: "HNT_011", glyph: "I", size: 12, pos: [64.25, 76.25] },
        { id: "HNT_012", glyph: "I", size: 12, pos: [64.50, 76.50] },
        { id: "HNT_013", glyph: "I", size: 12, pos: [64.75, 76.75] },
        { id: "HNT_020", glyph: "H", size: 24, pos: [64.00, 80.00] },
        { id: "HNT_021", glyph: "H", size: 24, pos: [64.25, 80.25] },
        { id: "HNT_022", glyph: "H", size: 24, pos: [64.50, 80.50] },
        { id: "HNT_023", glyph: "H", size: 24, pos: [64.75, 80.75] },
        { id: "HNT_030", glyph: "W", size: 96, pos: [96.00, 136.00] },
        { id: "HNT_031", glyph: "W", size: 96, pos: [96.25, 136.25] },
        { id: "HNT_032", glyph: "W", size: 96, pos: [96.50, 136.50] },
        { id: "HNT_033", glyph: "W", size: 96, pos: [96.75, 136.75] }
    ];

    for (var i = 0; i < cases.length; i++) {
        buildCase(cases[i]);
    }

    app.endUndoGroup();

    function buildCase(spec) {
        var outDir = new Folder(PACK_DIR.fsName + "/ae_goldens/png/" + spec.id);
        ensureFolder(outDir);

        var comp = app.project.items.addComp(spec.id, 192, 192, 1, 1 / 30, 30);
        comp.bgColor = [0, 0, 0];

        var textLayer = comp.layers.addText(spec.glyph);
        textLayer.name = spec.id + "_glyph_" + spec.glyph + "_" + spec.size;
        textLayer.property("ADBE Transform Group").property("ADBE Position").setValue(spec.pos);

        var textProp = textLayer.property("ADBE Text Properties").property("ADBE Text Document");
        var doc = textProp.value;
        doc.font = "Montserrat-Bold";
        doc.fontSize = spec.size;
        doc.fillColor = [1, 1, 1];
        doc.applyFill = true;
        doc.applyStroke = false;
        doc.justification = ParagraphJustification.CENTER_JUSTIFY;
        textProp.setValue(doc);

        var rq = app.project.renderQueue.items.add(comp);
        var om = rq.outputModule(1);
        try {
            om.applyTemplate("PNG Sequence");
        } catch (_templateErr) {
        }
        om.file = new File(outDir.fsName + "/" + spec.id + "_[#####].png");
    }

    function ensureFolder(folder) {
        if (!folder.exists) {
            folder.create();
        }
    }
})();
