/*
P2-C clipped text coverage probe.

SourceRect is used only to place the glyph by a known fractional offset. The
rendered alpha bbox is measured after AE writes the final frame.
*/

(function buildP2TextClipProbe() {
    app.beginUndoGroup("Build P2 Text Clip Probe");

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

    var W = 96;
    var H = 96;
    var FONT_SIZE = 72;
    var CENTER = [W / 2, H / 2];

    buildCase("CLP_LEFT_N049", "W", W, H, FONT_SIZE, "left", -0.49);
    buildCase("CLP_TOP_N049", "H", W, H, FONT_SIZE, "top", -0.49);
    buildCase("CLP_RIGHT_P049", "W", W, H, FONT_SIZE, "right", W + 0.49);
    buildCase("CLP_BOTTOM_P049", "H", W, H, FONT_SIZE, "bottom", H + 0.49);

    app.endUndoGroup();

    function buildCase(caseId, text, width, height, fontSize, edge, targetEdge) {
        var outDir = new Folder(PACK_DIR.fsName + "/ae_goldens/png/" + caseId);
        ensureFolder(outDir);

        var comp = app.project.items.addComp(caseId, width, height, 1, 1 / 30, 30);
        comp.bgColor = [0, 0, 0];

        var textLayer = comp.layers.addText(text);
        textLayer.name = caseId + "_glyph";
        textLayer.property("ADBE Transform Group").property("ADBE Position").setValue(CENTER);

        var textProp = textLayer.property("ADBE Text Properties").property("ADBE Text Document");
        var doc = textProp.value;
        doc.font = "Montserrat-BoldItalic";
        doc.fontSize = fontSize;
        doc.fillColor = [1, 1, 1];
        doc.applyFill = true;
        doc.applyStroke = false;
        doc.justification = ParagraphJustification.LEFT_JUSTIFY;
        textProp.setValue(doc);

        var rect = textLayer.sourceRectAtTime(0, false);
        var pos = CENTER.slice(0);
        if (edge === "left") {
            pos[0] = targetEdge - rect.left;
        } else if (edge === "right") {
            pos[0] = targetEdge - (rect.left + rect.width);
        } else if (edge === "top") {
            pos[1] = targetEdge - rect.top;
        } else if (edge === "bottom") {
            pos[1] = targetEdge - (rect.top + rect.height);
        }
        textLayer.property("ADBE Transform Group").property("ADBE Position").setValue(pos);

        var marker = new MarkerValue(caseId + " edge=" + edge + " target=" + targetEdge);
        marker.setParameters([
            "sourceRectLeft", String(rect.left),
            "sourceRectTop", String(rect.top),
            "sourceRectWidth", String(rect.width),
            "sourceRectHeight", String(rect.height),
            "positionX", String(pos[0]),
            "positionY", String(pos[1])
        ]);
        textLayer.property("ADBE Marker").setValueAtTime(0, marker);

        var rq = app.project.renderQueue.items.add(comp);
        var om = rq.outputModule(1);
        try {
            om.applyTemplate("PNG Sequence");
        } catch (_templateErr) {
        }
        om.file = new File(outDir.fsName + "/" + caseId + "_[#####].png");
    }

    function ensureFolder(folder) {
        if (!folder.exists) {
            folder.create();
        }
    }
})();

