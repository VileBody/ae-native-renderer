/*
P2 semi-transparent text fill probe.

The target native branch is TXT_ARE_Render_8bpc_fill_3d200:
fill alpha != 255 -> render full-alpha color into a temp PF_World, then
PF_TransferRect the temp into the destination with normalized opacity.
*/

(function buildP2TextTransfillProbeProject() {
    resetProjectAndCaches();
    app.beginUndoGroup("Build P2 Text Transfill Probe");

    var SCRIPT_FILE = new File($.fileName);
    var PACK_DIR = SCRIPT_FILE.parent.parent;
    var OUT_DIR = new Folder(PACK_DIR.fsName + "/ae_probe_outputs/png");
    ensureFolder(OUT_DIR);

    app.project.bitsPerChannel = 8;
    purgeCaches();

    var CFG = {
        width: 96,
        height: 96,
        duration: 0.2,
        fps: 30,
        font: "Arial-BoldMT",
        fallbackFont: "Arial",
        fontSize: 54,
        position: [48, 60]
    };

    var folders = {
        root: getOrCreateFolder("P2_TEXT_TRANSFILL"),
        cases: getOrCreateFolder("P2_TEXT_TRANSFILL/cases")
    };

    var cases = [
        { id: "TRF_WHT_A25_TRANSPARENT", fill: [1, 1, 1], alpha: 25, bg: null },
        { id: "TRF_WHT_A50_TRANSPARENT", fill: [1, 1, 1], alpha: 50, bg: null },
        { id: "TRF_WHT_A128_TRANSPARENT", fill: [1, 1, 1], alpha: 128, bg: null },
        { id: "TRF_WHT_A255_TRANSPARENT", fill: [1, 1, 1], alpha: 255, bg: null },
        { id: "TRF_RED_A128_TRANSPARENT", fill: [1, 0, 0], alpha: 128, bg: null },
        { id: "TRF_WHT_A25_BLACK", fill: [1, 1, 1], alpha: 25, bg: [0, 0, 0] },
        { id: "TRF_WHT_A50_BLACK", fill: [1, 1, 1], alpha: 50, bg: [0, 0, 0] },
        { id: "TRF_WHT_A128_BLACK", fill: [1, 1, 1], alpha: 128, bg: [0, 0, 0] },
        { id: "TRF_WHT_A255_BLACK", fill: [1, 1, 1], alpha: 255, bg: [0, 0, 0] },
        { id: "TRF_RED_A128_BLUE", fill: [1, 0, 0], alpha: 128, bg: [0, 0, 1] }
    ];

    for (var i = 0; i < cases.length; i++) {
        buildCase(cases[i], CFG, folders, OUT_DIR);
    }

    purgeCaches();
    app.endUndoGroup();

    function resetProjectAndCaches() {
        purgeCaches();
        if (app.project) {
            try {
                app.project.close(CloseOptions.DO_NOT_SAVE_CHANGES);
            } catch (_closeErr) {
            }
        }
        app.newProject();
        purgeCaches();
    }

    function purgeCaches() {
        try {
            app.purge(PurgeTarget.ALL_CACHES);
        } catch (_purgeErr) {
        }
    }

    function buildCase(spec, cfg, folders, outDir) {
        var comp = app.project.items.addComp(spec.id, cfg.width, cfg.height, 1, cfg.duration, cfg.fps);
        comp.parentFolder = folders.cases;
        comp.bgColor = [0, 0, 0];

        if (spec.bg !== null) {
            var bg = comp.layers.addSolid(spec.bg, spec.id + "_background", cfg.width, cfg.height, 1, cfg.duration);
            bg.property("ADBE Transform Group").property("ADBE Position").setValue([cfg.width / 2, cfg.height / 2]);
            bg.inPoint = 0;
            bg.outPoint = cfg.duration;
        }

        var layer = comp.layers.addText("H");
        layer.name = spec.id + "_text";
        layer.property("ADBE Transform Group").property("ADBE Position").setValue(cfg.position);

        var textProp = layer.property("ADBE Text Properties").property("ADBE Text Document");
        var doc = textProp.value;
        try {
            doc.font = cfg.font;
        } catch (_fontErr) {
            doc.font = cfg.fallbackFont;
        }
        doc.fontSize = cfg.fontSize;
        doc.applyFill = true;
        doc.applyStroke = false;
        doc.justification = ParagraphJustification.CENTER_JUSTIFY;
        setFillWithAlpha(doc, spec.fill, spec.alpha);
        textProp.setValue(doc);

        enqueue(comp, outDir, spec.id);
    }

    function setFillWithAlpha(doc, rgb, alphaByte) {
        var a = alphaByte / 255.0;
        try {
            doc.fillColor = [rgb[0], rgb[1], rgb[2], a];
        } catch (_rgbaErr) {
            doc.fillColor = [rgb[0], rgb[1], rgb[2]];
        }
        try {
            doc.fillOpacity = a * 100.0;
        } catch (_fillOpacityErr) {
        }
    }

    function enqueue(comp, outDir, caseId) {
        var caseDir = new Folder(outDir.fsName + "/" + caseId);
        ensureFolder(caseDir);
        var rq = app.project.renderQueue.items.add(comp);
        var om = rq.outputModule(1);
        try {
            om.applyTemplate("PNG Sequence");
        } catch (_templateErr) {
        }
        om.file = new File(caseDir.fsName + "/" + caseId + "_[#####].png");
    }

    function getOrCreateFolder(path) {
        var parts = path.split("/");
        var parent = app.project.rootFolder;
        for (var i = 0; i < parts.length; i++) {
            var found = null;
            for (var j = 1; j <= app.project.numItems; j++) {
                var item = app.project.item(j);
                if (item instanceof FolderItem && item.name === parts[i] && item.parentFolder === parent) {
                    found = item;
                    break;
                }
            }
            if (!found) {
                found = app.project.items.addFolder(parts[i]);
                found.parentFolder = parent;
            }
            parent = found;
        }
        return parent;
    }

    function ensureFolder(folder) {
        if (!folder.exists) {
            folder.create();
        }
    }
})();
