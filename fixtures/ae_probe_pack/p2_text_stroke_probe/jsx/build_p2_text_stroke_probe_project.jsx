/*
P2-D TXT_DrawChar stroke/fill probe.

This is intentionally tiny: one glyph per comp, one queued frame per case.
*/

(function buildP2TextStrokeProbe() {
    app.beginUndoGroup("Build P2 Text Stroke Probe");

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

    buildTextCase({
        caseId: "STR_FILL_ONLY",
        layerName: "STR_FILL_ONLY_W",
        applyFill: true,
        applyStroke: false,
        fillColor: [1, 1, 1],
        strokeColor: [1, 0, 0],
        strokeWidth: 0,
        strokeOverFill: false,
        fileName: "STR_FILL_ONLY_[#####].png"
    });

    buildTextCase({
        caseId: "STR_STROKE_ONLY",
        layerName: "STR_STROKE_ONLY_W",
        applyFill: false,
        applyStroke: true,
        fillColor: [0, 0, 0],
        strokeColor: [1, 0, 0],
        strokeWidth: 14,
        strokeOverFill: true,
        fileName: "STR_STROKE_ONLY_[#####].png"
    });

    buildTextCase({
        caseId: "STR_FILL_STROKE_FILL_OVER",
        layerName: "STR_FILL_STROKE_FILL_OVER_W",
        applyFill: true,
        applyStroke: true,
        fillColor: [0, 1, 0],
        strokeColor: [1, 0, 0],
        strokeWidth: 14,
        strokeOverFill: false,
        fileName: "STR_FILL_STROKE_FILL_OVER_[#####].png"
    });

    buildTextCase({
        caseId: "STR_FILL_STROKE_STROKE_OVER",
        layerName: "STR_FILL_STROKE_STROKE_OVER_W",
        applyFill: true,
        applyStroke: true,
        fillColor: [0, 1, 0],
        strokeColor: [1, 0, 0],
        strokeWidth: 14,
        strokeOverFill: true,
        fileName: "STR_FILL_STROKE_STROKE_OVER_[#####].png"
    });

    app.endUndoGroup();

    function buildTextCase(opts) {
        var outDir = new Folder(PACK_DIR.fsName + "/ae_goldens/png/" + opts.caseId);
        ensureFolder(outDir);

        var comp = app.project.items.addComp(opts.caseId, 256, 256, 1, 0.1, 30);
        comp.bgColor = [0, 0, 0];

        var textLayer = comp.layers.addText("W");
        textLayer.name = opts.layerName;
        textLayer.property("ADBE Transform Group").property("ADBE Position").setValue([128, 151]);

        var textProp = textLayer.property("ADBE Text Properties").property("ADBE Text Document");
        var doc = textProp.value;
        doc.font = "Montserrat-BoldItalic";
        doc.fontSize = 96;
        doc.justification = ParagraphJustification.CENTER_JUSTIFY;
        doc.applyFill = opts.applyFill;
        doc.applyStroke = opts.applyStroke;
        doc.fillColor = opts.fillColor;
        doc.strokeColor = opts.strokeColor;
        doc.strokeWidth = opts.strokeWidth;
        doc.strokeOverFill = opts.strokeOverFill;
        textProp.setValue(doc);

        var rq = app.project.renderQueue.items.add(comp);
        var om = rq.outputModule(1);
        try {
            om.applyTemplate("PNG Sequence");
        } catch (_templateErr) {
        }
        om.file = new File(outDir.fsName + "/" + opts.fileName);
    }

    function ensureFolder(folder) {
        if (!folder.exists) {
            folder.create();
        }
    }
})();

