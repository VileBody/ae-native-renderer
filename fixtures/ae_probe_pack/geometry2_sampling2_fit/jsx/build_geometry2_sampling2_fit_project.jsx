/*
AE Geometry2 Sampling=2 fit probe pack builder.

Generate primitive PNGs first:
  python3 scripts/measure_geometry2_sampling2_fit.py --pack . --generate-assets
*/

(function buildGeometry2Sampling2FitProject() {
    resetProjectAndCaches();
    app.beginUndoGroup("Build AE Geometry2 Sampling2 Fit Probe Pack");

    var SCRIPT_FILE = new File($.fileName);
    var PACK_DIR = SCRIPT_FILE.parent.parent;
    var ASSET_DIR = new Folder(PACK_DIR.fsName + "/assets/primitives");
    var OUT_DIR = new Folder(PACK_DIR.fsName + "/ae_goldens/png8");
    var METADATA_DIR = new Folder(PACK_DIR.fsName + "/ae_goldens/metadata");
    var LOG_DIR = new Folder(PACK_DIR.fsName + "/ae_goldens/logs");
    ensureFolder(OUT_DIR);
    ensureFolder(METADATA_DIR);
    ensureFolder(LOG_DIR);

    var LOG_FILE = new File(LOG_DIR.fsName + "/geometry2_sampling2_fit_builder_log.txt");
    LOG_FILE.encoding = "UTF-8";
    LOG_FILE.open("w");
    logLine("Geometry2 Sampling2 fit probe builder");
    logLine("Pack: " + PACK_DIR.fsName);

    var CFG = {
        width: 64,
        height: 64,
        fps: 30,
        duration: 1.0 / 30.0,
        bg: [0, 0, 0]
    };

    app.project.bitsPerChannel = 8;
    purgeCaches();

    var folders = {
        root: getOrCreateFolder("AE_GEOMETRY2_SAMPLING2_FIT_PROBE_PACK"),
        footage: getOrCreateFolder("AE_GEOMETRY2_SAMPLING2_FIT_PROBE_PACK/footage"),
        cases: getOrCreateFolder("AE_GEOMETRY2_SAMPLING2_FIT_PROBE_PACK/cases")
    };

    var sources = importAssets(folders.footage);
    var transforms = getTransforms();
    var sampling = getSampling();
    var cases = [];
    var propertyDumped = false;
    var propertySnapshot = [];

    for (var sourceKey in sources) {
        if (!sources.hasOwnProperty(sourceKey)) {
            continue;
        }
        for (var t = 0; t < transforms.length; t++) {
            for (var q = 0; q < sampling.length; q++) {
                var source = sources[sourceKey];
                var transform = transforms[t];
                var quality = sampling[q];
                var caseId = "G2S_" + source.token + "_" + transform.id + "_" + quality.id;
                var comp = app.project.items.addComp(
                    caseId + "__" + source.id + "__" + transform.title + "__" + quality.label,
                    CFG.width,
                    CFG.height,
                    1,
                    CFG.duration,
                    CFG.fps
                );
                comp.parentFolder = folders.cases;
                comp.bgColor = CFG.bg;

                var layer = comp.layers.add(source.item);
                layer.name = caseId + "_source";
                layer.inPoint = 0;
                layer.outPoint = CFG.duration;
                setLayerPosition(layer, [CFG.width / 2, CFG.height / 2]);
                setLayerScale(layer, [100, 100]);

                var fx = layer.property("ADBE Effect Parade").addProperty("ADBE Geometry2");
                if (!fx) {
                    throw new Error("AE could not add ADBE Geometry2");
                }
                applyGeometry2Params(fx, transform, quality, caseId);
                if (!propertyDumped) {
                    dumpEffectProperties(fx);
                    propertySnapshot = collectProperties(fx);
                    propertyDumped = true;
                }

                cases.push({
                    id: caseId,
                    source_id: source.id,
                    transform_id: transform.id,
                    sampling_id: quality.id,
                    sampling_value: quality.value,
                    comp: comp
                });
            }
        }
    }

    enqueueCases(cases, OUT_DIR);
    writeCaseDump(cases, transforms, sampling, propertySnapshot, METADATA_DIR);
    LOG_FILE.close();
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

    function getTransforms() {
        return [
            t("IDENTITY", "identity", 0.0, 0.0, 100, 100, 0),
            t("SHIFT_X_0_125", "source x minus 0.125", 0.125, 0.0, 100, 100, 0),
            t("SHIFT_X_0_250", "source x minus 0.250", 0.250, 0.0, 100, 100, 0),
            t("SHIFT_X_0_375", "source x minus 0.375", 0.375, 0.0, 100, 100, 0),
            t("SHIFT_X_0_500", "source x minus 0.500", 0.500, 0.0, 100, 100, 0),
            t("SHIFT_X_0_625", "source x minus 0.625", 0.625, 0.0, 100, 100, 0),
            t("SHIFT_X_0_750", "source x minus 0.750", 0.750, 0.0, 100, 100, 0),
            t("SHIFT_X_0_875", "source x minus 0.875", 0.875, 0.0, 100, 100, 0),
            t("SHIFT_Y_0_250", "source y minus 0.250", 0.0, 0.250, 100, 100, 0),
            t("SHIFT_XY_0_250_0_625", "source xy mixed", 0.250, 0.625, 100, 100, 0),
            t("EDGE_LEFT_1_250", "left edge source x minus 1.250", 1.250, 0.0, 100, 100, 0),
            t("EDGE_RIGHT_1_250", "right edge source x plus 1.250", -1.250, 0.0, 100, 100, 0),
            t("SCALE_92", "scale 92 percent", 0.0, 0.0, 92, 92, 0),
            t("ROTATE_3", "rotate 3 degrees", 0.0, 0.0, 100, 100, 3)
        ];
    }

    function t(id, title, dx, dy, sx, sy, rotation) {
        return {
            id: id,
            title: title,
            anchor: [32, 32],
            position: [32 + dx, 32 + dy],
            scale: [sx, sy],
            rotation: rotation
        };
    }

    function getSampling() {
        return [
            { id: "Q1_BILINEAR", value: 1, label: "Sampling 0012 value 1 Bilinear" },
            { id: "Q2_BICUBIC", value: 2, label: "Sampling 0012 value 2 Bicubic" }
        ];
    }

    function importAssets(folder) {
        return {
            texture: importStill("fit_texture", "FIT_TEXTURE", "fit_texture_64.png", folder),
            impulse: importStill("impulse_grid", "IMPULSE_GRID", "impulse_grid_64.png", folder),
            alpha: importStill("alpha_steps", "ALPHA_STEPS", "alpha_steps_64.png", folder)
        };
    }

    function importStill(id, token, relative, folder) {
        var file = new File(ASSET_DIR.fsName + "/" + relative);
        if (!file.exists) {
            throw new Error("Missing Geometry2 primitive asset: " + file.fsName);
        }
        var options = new ImportOptions(file);
        options.importAs = ImportAsType.FOOTAGE;
        var item = app.project.importFile(options);
        item.name = id;
        item.parentFolder = folder;
        return { id: id, token: token, item: item };
    }

    function applyGeometry2Params(fx, transform, quality, caseId) {
        setIndexedValue(fx, 1, transform.anchor, caseId, "Anchor Point");
        setIndexedValue(fx, 2, transform.position, caseId, "Position");
        setIndexedValue(fx, 3, 0, caseId, "Uniform Scale off");
        setIndexedValue(fx, 4, transform.scale[1], caseId, "Scale Height");
        setIndexedValue(fx, 5, transform.scale[0], caseId, "Scale Width");
        setIndexedValue(fx, 8, transform.rotation, caseId, "Rotation");
        setIndexedValue(fx, 12, quality.value, caseId, "Sampling");
    }

    function setIndexedValue(fx, index, value, caseId, label) {
        var prop = fx.property(index);
        if (!prop) {
            logLine(caseId + ": missing Geometry2 property " + index + " for " + label);
            return false;
        }
        try {
            prop.setValue(value);
            return true;
        } catch (err) {
            logLine(caseId + ": failed setting " + label + " index " + index + " to " + value + ": " + err.toString());
            return false;
        }
    }

    function dumpEffectProperties(fx) {
        logLine("");
        logLine("ADBE Geometry2 property dump:");
        for (var i = 1; i <= fx.numProperties; i++) {
            var prop = fx.property(i);
            if (prop) {
                logLine(i + "\tname='" + prop.name + "'\tmatchName='" + prop.matchName + "'\tvalue='" + safeValueString(prop) + "'");
            }
        }
        logLine("");
    }

    function collectProperties(fx) {
        var props = [];
        for (var i = 1; i <= fx.numProperties; i++) {
            var prop = fx.property(i);
            if (!prop) {
                continue;
            }
            props.push({
                index: i,
                name: safeString(prop.name),
                match_name: safeString(prop.matchName),
                value: propertyValue(prop)
            });
        }
        return props;
    }

    function writeCaseDump(cases, transforms, sampling, propertySnapshot, metadataDir) {
        var dump = {
            pack_id: "geometry2_sampling2_fit_probe_v1",
            effect_match_name: "ADBE Geometry2",
            first_case_property_snapshot: propertySnapshot,
            composition: {
                width: CFG.width,
                height: CFG.height,
                fps: CFG.fps,
                duration_seconds: CFG.duration,
                bits_per_channel: app.project.bitsPerChannel
            },
            transforms: transforms,
            sampling: sampling,
            cases: []
        };
        for (var i = 0; i < cases.length; i++) {
            dump.cases.push({
                id: cases[i].id,
                source_id: cases[i].source_id,
                transform_id: cases[i].transform_id,
                sampling_id: cases[i].sampling_id,
                sampling_value: cases[i].sampling_value
            });
        }
        var outFile = new File(metadataDir.fsName + "/geometry2_sampling2_fit_property_dump.json");
        outFile.encoding = "UTF-8";
        outFile.open("w");
        outFile.write(toJson(dump, 0));
        outFile.close();
    }

    function enqueueCases(cases, outDir) {
        for (var i = 0; i < cases.length; i++) {
            var caseDir = new Folder(outDir.fsName + "/" + cases[i].id);
            ensureFolder(caseDir);
            var rq = app.project.renderQueue.items.add(cases[i].comp);
            var om = rq.outputModule(1);
            try {
                om.applyTemplate("PNG Sequence with Alpha");
            } catch (_errAlpha) {
                try {
                    om.applyTemplate("PNG Sequence");
                } catch (_errPng) {
                }
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

    function safeValueString(prop) {
        try {
            return String(prop.value);
        } catch (_err) {
            return "<unreadable>";
        }
    }

    function propertyValue(prop) {
        try {
            var value = prop.value;
            if (value instanceof Array) {
                var out = [];
                for (var i = 0; i < value.length; i++) {
                    out.push(value[i]);
                }
                return out;
            }
            return value;
        } catch (_err) {
            return null;
        }
    }

    function safeString(value) {
        try {
            if (value === null || value === undefined) {
                return "";
            }
            return String(value);
        } catch (_err) {
            return "";
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
            var arr = [];
            for (var i = 0; i < value.length; i++) {
                arr.push(childPad + toJson(value[i], indent + 1));
            }
            return "[\n" + arr.join(",\n") + "\n" + pad + "]";
        }
        var obj = [];
        for (var key in value) {
            if (value.hasOwnProperty(key)) {
                obj.push(childPad + quoteJson(key) + ": " + toJson(value[key], indent + 1));
            }
        }
        if (obj.length === 0) {
            return "{}";
        }
        return "{\n" + obj.join(",\n") + "\n" + pad + "}";
    }

    function quoteJson(value) {
        return "\"" + String(value)
            .replace(/\\/g, "\\\\")
            .replace(/"/g, "\\\"")
            .replace(/\r/g, "\\r")
            .replace(/\n/g, "\\n")
            .replace(/\t/g, "\\t") + "\"";
    }

    function repeat(value, count) {
        var out = "";
        for (var i = 0; i < count; i++) {
            out += value;
        }
        return out;
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
            if (folder.parent && !folder.parent.exists) {
                ensureFolder(folder.parent);
            }
            folder.create();
        }
    }

    function logLine(line) {
        LOG_FILE.writeln(line);
    }
})();
