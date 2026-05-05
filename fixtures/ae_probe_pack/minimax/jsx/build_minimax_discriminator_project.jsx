/*
AE Native Renderer Minimax discriminator probe.

Small M13 pack for resolving radius quantization, direction enum, channel lane
selection, compound operation order, and Don't Shrink Edges behavior.
*/

(function buildMinimaxDiscriminatorProject() {
    resetProjectAndCaches();

    app.beginUndoGroup("Build Minimax Discriminator Probe Pack");

    var SCRIPT_FILE = new File($.fileName);
    var PACK_DIR = SCRIPT_FILE.parent.parent;
    var PNG_DIR = new Folder(PACK_DIR.fsName + "/ae_probe_outputs/png");
    var METADATA_DIR = new Folder(PACK_DIR.fsName + "/ae_probe_outputs/metadata");
    ensureFolder(PNG_DIR);
    ensureFolder(METADATA_DIR);

    var CFG = {
        width: 128,
        height: 128,
        fps: 30,
        duration: 1.0,
        center: [64, 64],
        patchSize: 8,
        dotSize: 5,
        bg: [0, 0, 0]
    };

    app.project.bitsPerChannel = 8;
    purgeCaches();

    var folders = {
        root: getOrCreateFolder("AE_PROBE_MINIMAX_DISCRIMINATOR"),
        cases: getOrCreateFolder("AE_PROBE_MINIMAX_DISCRIMINATOR/cases"),
        precomps: getOrCreateFolder("AE_PROBE_MINIMAX_DISCRIMINATOR/precomps")
    };

    var sources = {
        patch: buildTransparentPatchSource("SRC_transparent_white_patch", CFG, folders),
        fullWhite: buildFullSolidSource("SRC_full_white", [1, 1, 1], CFG, folders),
        whiteDotOnBlack: buildDotOnSolidSource("SRC_white_dot_on_black", [0, 0, 0], [1, 1, 1], CFG, folders),
        blackDotOnWhite: buildDotOnSolidSource("SRC_black_dot_on_white", [1, 1, 1], [0, 0, 0], CFG, folders)
    };

    var cases = [];
    addSourceCases(CFG, sources, folders, cases);
    addRadiusCases(CFG, sources, folders, cases);
    addDirectionCases(CFG, sources, folders, cases);
    addChannelCases(CFG, sources, folders, cases);
    addOperationCases(CFG, sources, folders, cases);
    addEdgeCases(CFG, sources, folders, cases);
    enqueueCases(cases, PNG_DIR);
    writeMetadata(cases, METADATA_DIR);
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
        cases.push(createCase(cfg, folders, "MMD_SRC_PATCH", "transparent white patch source", function (comp) {
            placeComp(comp, sources.patch);
        }, null));
        cases.push(createCase(cfg, folders, "MMD_SRC_DOT_BLACK", "opaque white dot on black source", function (comp) {
            placeComp(comp, sources.whiteDotOnBlack);
        }, null));
        cases.push(createCase(cfg, folders, "MMD_SRC_FULL_WHITE", "full white source", function (comp) {
            placeComp(comp, sources.fullWhite);
        }, null));
    }

    function addRadiusCases(cfg, sources, folders, cases) {
        var radii = [0, 0.24, 0.49, 0.5, 0.51, 0.99, 1, 1.49, 1.5, 1.51, 2, 2.49, 2.5, 2.51];
        for (var i = 0; i < radii.length; i++) {
            (function (radiusValue) {
                cases.push(createCase(cfg, folders, caseId("MMD_RAD", radiusValue), "maximum radius " + radiusValue, function (comp) {
                    var layer = placeComp(comp, sources.patch);
                    addMinimax(layer, {
                        "0001": 2,
                        "0002": radiusValue,
                        "0003": 2,
                        "0004": 1,
                        "0005": 0
                    });
                }, {
                    family: "radius_quantization",
                    source: "SRC_transparent_white_patch",
                    params: { "0001": 2, "0002": radiusValue, "0003": 2, "0004": 1, "0005": 0 }
                }));
            })(radii[i]);
        }
    }

    function addDirectionCases(cfg, sources, folders, cases) {
        var defs = [
            { id: "HV", value: 1 },
            { id: "H", value: 2 },
            { id: "V", value: 3 }
        ];
        for (var i = 0; i < defs.length; i++) {
            (function (def) {
                cases.push(createCase(cfg, folders, "MMD_DIR_" + def.id, "maximum direction " + def.id, function (comp) {
                    var layer = placeComp(comp, sources.patch);
                    addMinimax(layer, {
                        "0001": 2,
                        "0002": 4,
                        "0003": 2,
                        "0004": def.value,
                        "0005": 0
                    });
                }, {
                    family: "direction",
                    source: "SRC_transparent_white_patch",
                    params: { "0001": 2, "0002": 4, "0003": 2, "0004": def.value, "0005": 0 }
                }));
            })(defs[i]);
        }
    }

    function addChannelCases(cfg, sources, folders, cases) {
        var channelDefs = [
            { id: "COLOR", value: 1 },
            { id: "ALPHA_COLOR", value: 2 },
            { id: "RED", value: 3 },
            { id: "GREEN", value: 4 },
            { id: "BLUE", value: 5 },
            { id: "ALPHA", value: 6 }
        ];
        for (var i = 0; i < channelDefs.length; i++) {
            (function (def) {
                cases.push(createCase(cfg, folders, "MMD_CH_OPAQUE_" + def.id, "channel " + def.id + " on opaque RGB dot", function (comp) {
                    var layer = placeComp(comp, sources.whiteDotOnBlack);
                    addMinimax(layer, {
                        "0001": 2,
                        "0002": 4,
                        "0003": def.value,
                        "0004": 1,
                        "0005": 0
                    });
                }, {
                    family: "channel_rgb",
                    source: "SRC_white_dot_on_black",
                    params: { "0001": 2, "0002": 4, "0003": def.value, "0004": 1, "0005": 0 }
                }));
            })(channelDefs[i]);
        }

        var alphaDefs = [
            { id: "COLOR", value: 1 },
            { id: "ALPHA_COLOR", value: 2 },
            { id: "ALPHA", value: 6 }
        ];
        for (var j = 0; j < alphaDefs.length; j++) {
            (function (def) {
                cases.push(createCase(cfg, folders, "MMD_CH_TRANSPARENT_" + def.id, "channel " + def.id + " on transparent patch", function (comp) {
                    var layer = placeComp(comp, sources.patch);
                    addMinimax(layer, {
                        "0001": 2,
                        "0002": 4,
                        "0003": def.value,
                        "0004": 1,
                        "0005": 0
                    });
                }, {
                    family: "channel_alpha",
                    source: "SRC_transparent_white_patch",
                    params: { "0001": 2, "0002": 4, "0003": def.value, "0004": 1, "0005": 0 }
                }));
            })(alphaDefs[j]);
        }
    }

    function addOperationCases(cfg, sources, folders, cases) {
        var defs = [
            { id: "MIN", value: 1, source: sources.whiteDotOnBlack, sourceName: "SRC_white_dot_on_black" },
            { id: "MAX", value: 2, source: sources.whiteDotOnBlack, sourceName: "SRC_white_dot_on_black" },
            { id: "MIN_THEN_MAX", value: 3, source: sources.whiteDotOnBlack, sourceName: "SRC_white_dot_on_black" },
            { id: "MAX_THEN_MIN", value: 4, source: sources.blackDotOnWhite, sourceName: "SRC_black_dot_on_white" }
        ];
        for (var i = 0; i < defs.length; i++) {
            (function (def) {
                cases.push(createCase(cfg, folders, "MMD_OP_" + def.id, "operation " + def.id, function (comp) {
                    var layer = placeComp(comp, def.source);
                    addMinimax(layer, {
                        "0001": def.value,
                        "0002": 4,
                        "0003": 1,
                        "0004": 1,
                        "0005": 0
                    });
                }, {
                    family: "operation",
                    source: def.sourceName,
                    params: { "0001": def.value, "0002": 4, "0003": 1, "0004": 1, "0005": 0 }
                }));
            })(defs[i]);
        }
    }

    function addEdgeCases(cfg, sources, folders, cases) {
        var defs = [
            { id: "0", value: 0 },
            { id: "1", value: 1 }
        ];
        for (var i = 0; i < defs.length; i++) {
            (function (def) {
                cases.push(createCase(cfg, folders, "MMD_EDGE_MIN_DSE" + def.id, "minimum full white don't shrink " + def.id, function (comp) {
                    var layer = placeComp(comp, sources.fullWhite);
                    addMinimax(layer, {
                        "0001": 1,
                        "0002": 4,
                        "0003": 2,
                        "0004": 1,
                        "0005": def.value
                    });
                }, {
                    family: "dont_shrink_edges",
                    source: "SRC_full_white",
                    params: { "0001": 1, "0002": 4, "0003": 2, "0004": 1, "0005": def.value }
                }));
            })(defs[i]);
        }
    }

    function buildTransparentPatchSource(name, cfg, folders) {
        var comp = createPrecomp(name, cfg, folders);
        addPatch(comp, name + "_patch", [1, 1, 1], cfg.center, cfg.patchSize, 100);
        return comp;
    }

    function buildFullSolidSource(name, color, cfg, folders) {
        var comp = createPrecomp(name, cfg, folders);
        addPatch(comp, name + "_solid", color, cfg.center, cfg.width, 100);
        return comp;
    }

    function buildDotOnSolidSource(name, bgColor, dotColor, cfg, folders) {
        var comp = createPrecomp(name, cfg, folders);
        addPatch(comp, name + "_bg", bgColor, cfg.center, cfg.width, 100);
        addPatch(comp, name + "_dot", dotColor, cfg.center, cfg.dotSize, 100);
        return comp;
    }

    function createPrecomp(name, cfg, folders) {
        var comp = app.project.items.addComp(name, cfg.width, cfg.height, 1, cfg.duration, cfg.fps);
        comp.parentFolder = folders.precomps;
        comp.bgColor = cfg.bg;
        return comp;
    }

    function createCase(cfg, folders, id, title, builder, metadata) {
        var comp = app.project.items.addComp(id + "__" + title, cfg.width, cfg.height, 1, cfg.duration, cfg.fps);
        comp.parentFolder = folders.cases;
        comp.bgColor = cfg.bg;
        builder(comp);
        return { id: id, title: title, comp: comp, metadata: metadata };
    }

    function addPatch(comp, name, color, position, size, opacity) {
        var layer = comp.layers.addSolid(color, name, size, size, 1, comp.duration);
        setLayerPosition(layer, position);
        setLayerOpacity(layer, opacity);
        layer.inPoint = 0;
        layer.outPoint = comp.duration;
        return layer;
    }

    function placeComp(comp, item) {
        var layer = comp.layers.add(item);
        setLayerPosition(layer, [comp.width / 2, comp.height / 2]);
        setLayerScale(layer, [100, 100]);
        layer.inPoint = 0;
        layer.outPoint = comp.duration;
        return layer;
    }

    function addMinimax(layer, params) {
        return addEffect(layer, "ADBE Minimax", params);
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
                prop.setValue(params[key]);
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

    function writeMetadata(cases, metadataDir) {
        var data = {
            schema: "ae-native-renderer.minimax-discriminator.v1",
            effect_match_name: "ADBE Minimax",
            comp_size: [CFG.width, CFG.height],
            center: CFG.center,
            patch_size: CFG.patchSize,
            dot_size: CFG.dotSize,
            cases: []
        };
        for (var i = 0; i < cases.length; i++) {
            data.cases.push({
                id: cases[i].id,
                title: cases[i].title,
                metadata: cases[i].metadata
            });
        }
        var outFile = new File(metadataDir.fsName + "/minimax_discriminator_manifest.json");
        outFile.encoding = "UTF-8";
        outFile.open("w");
        outFile.write(toJson(data, 0));
        outFile.close();
    }

    function caseId(prefix, value) {
        return prefix + "_" + String(value).replace(".", "P").replace("-", "NEG");
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

    function toJson(value, indent) {
        var pad = repeat("  ", indent);
        var childPad = repeat("  ", indent + 1);
        if (value === null || value === undefined) {
            return "null";
        }
        if (typeof value === "number" || typeof value === "boolean") {
            return String(value);
        }
        if (typeof value === "string") {
            return quoteJson(value);
        }
        if (value instanceof Array) {
            if (value.length === 0) {
                return "[]";
            }
            var arrParts = [];
            for (var i = 0; i < value.length; i++) {
                arrParts.push(childPad + toJson(value[i], indent + 1));
            }
            return "[\n" + arrParts.join(",\n") + "\n" + pad + "]";
        }
        var parts = [];
        for (var key in value) {
            if (value.hasOwnProperty(key)) {
                parts.push(childPad + quoteJson(key) + ": " + toJson(value[key], indent + 1));
            }
        }
        if (parts.length === 0) {
            return "{}";
        }
        return "{\n" + parts.join(",\n") + "\n" + pad + "}";
    }

    function repeat(text, count) {
        var out = "";
        for (var i = 0; i < count; i++) {
            out += text;
        }
        return out;
    }

    function quoteJson(text) {
        return "\"" + String(text)
            .replace(/\\/g, "\\\\")
            .replace(/"/g, "\\\"")
            .replace(/\r/g, "\\r")
            .replace(/\n/g, "\\n")
            .replace(/\t/g, "\\t") + "\"";
    }
})();
