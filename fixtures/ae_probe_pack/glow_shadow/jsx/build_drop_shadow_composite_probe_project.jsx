/*
AE Native Renderer Drop Shadow Composite Probe Builder

Small M10 discriminator for color, opacity, Shadow Only, source alpha, and
final source-over behavior. It intentionally keeps softness at 0 so blur math is
not mixed into the composite questions.
*/

(function buildDropShadowCompositeProbeProject() {
    resetProjectAndCaches();

    app.beginUndoGroup("Build Drop Shadow Composite Probe Pack");

    var SCRIPT_FILE = new File($.fileName);
    var PACK_DIR = SCRIPT_FILE.parent.parent;
    var PNG_DIR = new Folder(PACK_DIR.fsName + "/ae_probe_outputs/png");
    ensureFolder(PNG_DIR);

    var CFG = {
        width: 512,
        height: 512,
        fps: 30,
        duration: 1.0,
        bg: [0, 0, 0],
        center: [256, 256],
        offsetRight: [288, 256],
        patchSize: 16
    };

    app.project.bitsPerChannel = 8;
    purgeCaches();

    var folders = {
        root: getOrCreateFolder("AE_PROBE_DROP_SHADOW_COMPOSITE"),
        cases: getOrCreateFolder("AE_PROBE_DROP_SHADOW_COMPOSITE/cases"),
        precomps: getOrCreateFolder("AE_PROBE_DROP_SHADOW_COMPOSITE/precomps")
    };

    var sources = {
        whiteOpaque: buildPatchSource("SRC_white_opaque_16px", [1, 1, 1], 100, CFG, folders),
        whiteHalf: buildPatchSource("SRC_white_half_alpha_16px", [1, 1, 1], 50, CFG, folders),
        blueOpaque: buildPatchSource("SRC_blue_opaque_16px", [0, 0, 1], 100, CFG, folders)
    };

    var cases = [];
    addSourceCases(CFG, sources, folders, cases);
    addOpacityCases(CFG, sources, folders, cases);
    addColorCases(CFG, sources, folders, cases);
    addCompositeCases(CFG, sources, folders, cases);
    enqueueCases(cases, PNG_DIR);
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

    function addSourceCases(cfg, sources, folders, cases) {
        cases.push(createCase(cfg, folders, "DSC_SRC_OPAQUE", "source opaque white patch", function (comp) {
            placeComp(comp, sources.whiteOpaque, cfg.center, [100, 100]);
        }));
        cases.push(createCase(cfg, folders, "DSC_SRC_HALF", "source half alpha white patch", function (comp) {
            placeComp(comp, sources.whiteHalf, cfg.center, [100, 100]);
        }));
    }

    function addOpacityCases(cfg, sources, folders, cases) {
        var values = [0, 25, 50, 100, 180, 255];
        for (var i = 0; i < values.length; i++) {
            (function (opacityValue) {
                cases.push(createCase(cfg, folders, "DSC_OPACITY_" + opacityValue, "shadow only opacity " + opacityValue, function (comp) {
                    var layer = placeComp(comp, sources.whiteOpaque, cfg.center, [100, 100]);
                    addDropShadow(layer, [1, 0, 0], opacityValue, 0, 0, 0, 1);
                }));
            })(values[i]);
        }
        cases.push(createCase(cfg, folders, "DSC_SOURCE_ALPHA_50", "shadow only source alpha 50 opacity 100", function (comp) {
            var layer = placeComp(comp, sources.whiteHalf, cfg.center, [100, 100]);
            addDropShadow(layer, [1, 0, 0], 100, 0, 0, 0, 1);
        }));
    }

    function addColorCases(cfg, sources, folders, cases) {
        var colors = [
            { id: "RED", value: [1, 0, 0] },
            { id: "GREEN", value: [0, 1, 0] },
            { id: "BLUE", value: [0, 0, 1] }
        ];
        for (var i = 0; i < colors.length; i++) {
            (function (colorCase) {
                cases.push(createCase(cfg, folders, "DSC_COLOR_" + colorCase.id, "shadow only color " + colorCase.id, function (comp) {
                    var layer = placeComp(comp, sources.whiteOpaque, cfg.center, [100, 100]);
                    addDropShadow(layer, colorCase.value, 50, 0, 0, 0, 1);
                }));
            })(colors[i]);
        }
    }

    function addCompositeCases(cfg, sources, folders, cases) {
        cases.push(createCase(cfg, folders, "DSC_FINAL_OFFSET", "final source over offset red shadow", function (comp) {
            var layer = placeComp(comp, sources.blueOpaque, cfg.center, [100, 100]);
            addDropShadow(layer, [1, 0, 0], 50, 90, 32, 0, 0);
        }));
        cases.push(createCase(cfg, folders, "DSC_SHADOW_ONLY_OFFSET", "shadow only offset red shadow", function (comp) {
            var layer = placeComp(comp, sources.blueOpaque, cfg.center, [100, 100]);
            addDropShadow(layer, [1, 0, 0], 50, 90, 32, 0, 1);
        }));
        cases.push(createCase(cfg, folders, "DSC_FINAL_OVERLAP_HALF", "final half source over same-pixel red shadow", function (comp) {
            var layer = placeComp(comp, sources.whiteHalf, cfg.center, [100, 100]);
            addDropShadow(layer, [1, 0, 0], 100, 0, 0, 0, 0);
        }));
        cases.push(createCase(cfg, folders, "DSC_SHADOW_ONLY_OVERLAP_HALF", "shadow only half source same-pixel red shadow", function (comp) {
            var layer = placeComp(comp, sources.whiteHalf, cfg.center, [100, 100]);
            addDropShadow(layer, [1, 0, 0], 100, 0, 0, 0, 1);
        }));
    }

    function buildPatchSource(name, color, opacity, cfg, folders) {
        var comp = createPrecomp(name, cfg, folders);
        addPatch(comp, name + "_patch", color, cfg.center, cfg.patchSize, opacity);
        return comp;
    }

    function createPrecomp(name, cfg, folders) {
        var comp = app.project.items.addComp(name, cfg.width, cfg.height, 1, cfg.duration, cfg.fps);
        comp.parentFolder = folders.precomps;
        comp.bgColor = cfg.bg;
        return comp;
    }

    function createCase(cfg, folders, id, title, builder) {
        var comp = app.project.items.addComp(id + "__" + title, cfg.width, cfg.height, 1, cfg.duration, cfg.fps);
        comp.parentFolder = folders.cases;
        comp.bgColor = cfg.bg;
        builder(comp);
        return { id: id, title: title, comp: comp };
    }

    function addPatch(comp, name, color, position, size, opacity) {
        var layer = comp.layers.addSolid(color, name, size, size, 1, comp.duration);
        setLayerPosition(layer, position);
        setLayerOpacity(layer, opacity);
        layer.inPoint = 0;
        layer.outPoint = comp.duration;
        return layer;
    }

    function placeComp(comp, item, position, scale) {
        var layer = comp.layers.add(item);
        setLayerPosition(layer, position);
        setLayerScale(layer, scale);
        layer.inPoint = 0;
        layer.outPoint = comp.duration;
        return layer;
    }

    function addDropShadow(layer, color, opacity, direction, distance, softness, shadowOnly) {
        return addEffect(layer, "ADBE Drop Shadow", {
            "0001": color,
            "0002": opacity,
            "0003": direction,
            "0004": distance,
            "0005": softness,
            "0006": shadowOnly
        });
    }

    function addEffect(layer, matchName, params) {
        var fx = layer.property("ADBE Effect Parade").addProperty(matchName);
        if (!fx) {
            throw new Error("AE could not add effect: " + matchName);
        }
        applyEffectParams(fx, params || {});
        return fx;
    }

    function applyEffectParams(fx, params) {
        for (var key in params) {
            if (!params.hasOwnProperty(key)) {
                continue;
            }
            var prop = null;
            if (/^0*\d+$/.test(key)) {
                prop = fx.property(parseInt(key, 10));
            } else {
                prop = fx.property(key);
            }
            if (prop) {
                try {
                    prop.setValue(params[key]);
                } catch (_err) {
                }
            }
        }
    }

    function enqueueCases(cases, pngDir) {
        for (var i = 0; i < cases.length; i++) {
            var caseDir = new Folder(pngDir.fsName + "/" + cases[i].id);
            ensureFolder(caseDir);
            var rq = app.project.renderQueue.items.add(cases[i].comp);
            var om = rq.outputModule(1);
            try {
                om.applyTemplate("PNG Sequence");
            } catch (_err) {
            }
            om.file = new File(caseDir.fsName + "/" + cases[i].id + "_[#####].png");
        }
    }

    function setLayerPosition(layer, value) {
        layer.property("ADBE Transform Group").property("ADBE Position").setValue(value);
    }

    function setLayerScale(layer, value) {
        layer.property("ADBE Transform Group").property("ADBE Scale").setValue(value);
    }

    function setLayerOpacity(layer, value) {
        layer.property("ADBE Transform Group").property("ADBE Opacity").setValue(value);
    }

    function getOrCreateFolder(path) {
        var parts = path.split("/");
        var parent = null;
        var current = null;
        for (var i = 0; i < parts.length; i++) {
            current = findFolder(parts[i], parent);
            if (!current) {
                current = app.project.items.addFolder(parts[i]);
                if (parent) {
                    current.parentFolder = parent;
                }
            }
            parent = current;
        }
        return current;
    }

    function findFolder(name, parent) {
        for (var i = 1; i <= app.project.numItems; i++) {
            var item = app.project.item(i);
            if (item instanceof FolderItem && item.name === name) {
                if (!parent || item.parentFolder === parent) {
                    return item;
                }
            }
        }
        return null;
    }

    function ensureFolder(folder) {
        if (!folder.exists) {
            folder.create();
        }
    }
})();
