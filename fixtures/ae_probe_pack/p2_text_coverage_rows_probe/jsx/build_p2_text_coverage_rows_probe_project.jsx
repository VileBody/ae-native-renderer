/*
Tiny render-time text probe for P2-A CoolType/BIB coverage rows.
*/

(function buildP2TextCoverageRowsProbe() {
    app.beginUndoGroup("Build P2 Text Coverage Rows Probe");

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

    buildTextCase("COV_W", "W", 128, 150);
    buildTextCase("COV_I", "I", 128, 150);
    buildTextCase("COV_O", "O", 128, 150);

    app.endUndoGroup();

    function buildTextCase(caseId, text, x, y) {
        var outDir = new Folder(PACK_DIR.fsName + "/ae_goldens/png/" + caseId);
        ensureFolder(outDir);

        var comp = app.project.items.addComp(caseId, 256, 256, 1, 0.2, 30);
        comp.bgColor = [0, 0, 0];

        var textLayer = comp.layers.addText(text);
        textLayer.name = caseId + "_single_glyph";
        textLayer.property("ADBE Transform Group").property("ADBE Position").setValue([x, y]);

        var textProp = textLayer.property("ADBE Text Properties").property("ADBE Text Document");
        var doc = textProp.value;
        doc.font = "Montserrat-BoldItalic";
        doc.fontSize = 96;
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
        om.file = new File(outDir.fsName + "/" + caseId + "_[#####].png");
    }

    function ensureFolder(folder) {
        if (!folder.exists) {
            folder.create();
        }
    }
})();
