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
    var OUT_DIR = new Folder(PACK_DIR.fsName + "/ae_goldens/png/RAS_010");
    ensureFolder(OUT_DIR);

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

    var comp = app.project.items.addComp("RAS_010", 512, 512, 1, 0.2, 30);
    comp.bgColor = [0.02, 0.02, 0.025];

    var textLayer = comp.layers.addText("WORD RASTER\nMONTSERRAT");
    textLayer.name = "RAS_010_montserrat_text";
    textLayer.property("ADBE Transform Group").property("ADBE Position").setValue([256, 256]);

    var textProp = textLayer.property("ADBE Text Properties").property("ADBE Text Document");
    var doc = textProp.value;
    doc.font = "Montserrat-BoldItalic";
    doc.fontSize = 72;
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
    om.file = new File(OUT_DIR.fsName + "/RAS_010_[#####].png");

    app.endUndoGroup();

    function ensureFolder(folder) {
        if (!folder.exists) {
            folder.create();
        }
    }
})();
