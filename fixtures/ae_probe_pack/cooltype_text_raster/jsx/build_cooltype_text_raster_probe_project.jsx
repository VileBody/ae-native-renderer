/*
Minimal render-time text raster probe.

This intentionally avoids the full conformance project/effect dump so Frida can
attach near the actual render queue work instead of spending its event budget on
Scripting/sourceRect setup.
*/

(function buildCoolTypeTextRasterProbe() {
    app.beginUndoGroup("Build CoolType Text Raster Probe");

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

    buildTextCase(
        "RAS_010",
        "RAS_010_montserrat_text",
        "WORD RASTER\nMONTSERRAT",
        512,
        512,
        72,
        [256, 256],
        "RAS_010_[#####].png"
    );

    buildTextCase(
        "RAS_020",
        "RAS_020_single_glyph_W",
        "W",
        256,
        256,
        96,
        [128, 150],
        "RAS_020_[#####].png"
    );

    app.endUndoGroup();

    function buildTextCase(caseId, layerName, text, width, height, fontSize, position, fileName) {
        var outDir = new Folder(PACK_DIR.fsName + "/ae_goldens/png/" + caseId);
        ensureFolder(outDir);

        var comp = app.project.items.addComp(caseId, width, height, 1, 0.2, 30);
        comp.bgColor = [0, 0, 0];

        var textLayer = comp.layers.addText(text);
        textLayer.name = layerName;
        textLayer.property("ADBE Transform Group").property("ADBE Position").setValue(position);

        var textProp = textLayer.property("ADBE Text Properties").property("ADBE Text Document");
        var doc = textProp.value;
        doc.font = "Montserrat-BoldItalic";
        doc.fontSize = fontSize;
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
        om.file = new File(outDir.fsName + "/" + fileName);
    }

    function ensureFolder(folder) {
        if (!folder.exists) {
            folder.create();
        }
    }
})();
