/*
AE Minimax enum/channel probe pack builder.

Run from After Effects:
  File > Scripts > Run Script File... > build_minimax_probe_project.jsx

The script creates self-contained alpha-square-like source comps, Minimax probe
case comps, render-queue entries, and a property metadata dump.
*/

(function buildMinimaxProbeProject() {
    app.beginUndoGroup("Build AE Minimax Probe Pack");

    var SCRIPT_FILE = new File($.fileName);
    var PACK_DIR = SCRIPT_FILE.parent.parent;
    var GOLDEN_DIR = new Folder(PACK_DIR.fsName + "/ae_goldens/png");
    var METADATA_DIR = new Folder(PACK_DIR.fsName + "/ae_goldens/metadata");
    ensureFolder(GOLDEN_DIR);
    ensureFolder(METADATA_DIR);

    var CFG = {
        width: 512,
        height: 512,
        sourceSize: 256,
        squareSize: 128,
        fps: 30,
        duration: 2.0,
        bg: [0.02, 0.02, 0.025],
        rowSampleY: 256,
        rowSampleXStart: 211,
        rowSampleXEnd: 300
    };

    if (!app.project) {
        app.newProject();
    }
    app.project.bitsPerChannel = 8;

    var folders = {
        root: getOrCreateFolder("AE_MINIMAX_PROBE_PACK"),
        sources: getOrCreateFolder("AE_MINIMAX_PROBE_PACK/sources"),
        cases: getOrCreateFolder("AE_MINIMAX_PROBE_PACK/cases")
    };

    var source = buildAlphaSquareSource(CFG, folders.sources);
    var caseDefs = getCaseDefs();
    var cases = buildCases(CFG, source, caseDefs, folders.cases);
    enqueueCases(cases, GOLDEN_DIR);
    writePropertyDump(caseDefs, METADATA_DIR, source, CFG, folders.sources);

    alert(
        "AE Minimax probe pack created.\n\n" +
        "Case comps queued: " + cases.length + "\n" +
        "PNG output root:\n" + GOLDEN_DIR.fsName + "\n\n" +
        "Metadata dump:\n" + METADATA_DIR.fsName + "/minimax_property_dump.json"
    );

    app.endUndoGroup();

    function getCaseDefs() {
        return [
            {
                id: "MINIMAX_PRE",
                title: "alpha-square-like source before Minimax",
                params: null,
                role: "pre-effect reference"
            },
            {
                id: "MINIMAX_OP1_CH1_R0",
                title: "Minimax op 1 channel 1 radius 0 identity probe",
                params: { "0001": 1, "0002": 0, "0003": 1 },
                role: "radius 0 identity"
            },
            {
                id: "MINIMAX_OP1_CH1_R12",
                title: "Minimax op 1 channel 1 radius 12",
                params: { "0001": 1, "0002": 12, "0003": 1 },
                role: "operation/channel enum probe"
            },
            {
                id: "MINIMAX_OP1_CH2_R0",
                title: "Minimax op 1 channel 2 radius 0 identity probe",
                params: { "0001": 1, "0002": 0, "0003": 2 },
                role: "radius 0 identity"
            },
            {
                id: "MINIMAX_OP1_CH2_R12",
                title: "Minimax op 1 channel 2 radius 12",
                params: { "0001": 1, "0002": 12, "0003": 2 },
                role: "operation/channel enum probe"
            },
            {
                id: "MINIMAX_OP2_CH1_R0",
                title: "Minimax op 2 channel 1 radius 0 identity probe",
                params: { "0001": 2, "0002": 0, "0003": 1 },
                role: "radius 0 identity"
            },
            {
                id: "MINIMAX_OP2_CH1_R12_EFF050",
                title: "Minimax op 2 channel 1 radius 12 EFF_050 tuple",
                params: { "0001": 2, "0002": 12, "0003": 1 },
                role: "EFF_050 tuple"
            },
            {
                id: "MINIMAX_OP2_CH2_R0",
                title: "Minimax op 2 channel 2 radius 0 identity probe",
                params: { "0001": 2, "0002": 0, "0003": 2 },
                role: "radius 0 identity"
            },
            {
                id: "MINIMAX_OP2_CH2_R12",
                title: "Minimax op 2 channel 2 radius 12",
                params: { "0001": 2, "0002": 12, "0003": 2 },
                role: "operation/channel enum probe"
            }
        ];
    }

    function buildAlphaSquareSource(cfg, folder) {
        var comp = app.project.items.addComp("MINIMAX_SRC_alpha_square_like", cfg.sourceSize, cfg.sourceSize, 1, cfg.duration, cfg.fps);
        comp.parentFolder = folder;
        comp.bgColor = [0, 0, 0];

        var square = comp.layers.addSolid([1, 1, 1], "opaque_center_128x128", cfg.squareSize, cfg.squareSize, 1, cfg.duration);
        square.property("ADBE Transform Group").property("ADBE Position").setValue([cfg.sourceSize / 2, cfg.sourceSize / 2]);
        square.inPoint = 0;
        square.outPoint = cfg.duration;

        return comp;
    }

    function buildCases(cfg, source, caseDefs, folder) {
        var built = [];
        for (var i = 0; i < caseDefs.length; i++) {
            var def = caseDefs[i];
            var comp = app.project.items.addComp(def.id + "__" + def.title, cfg.width, cfg.height, 1, cfg.duration, cfg.fps);
            comp.parentFolder = folder;
            comp.bgColor = cfg.bg;

            var layer = comp.layers.add(source);
            layer.name = "alpha_square_like";
            layer.property("ADBE Transform Group").property("ADBE Position").setValue([256, 256]);
            layer.property("ADBE Transform Group").property("ADBE Scale").setValue([70, 70]);
            layer.inPoint = 0;
            layer.outPoint = cfg.duration;

            if (def.params) {
                var fx = layer.property("ADBE Effect Parade").addProperty("ADBE Minimax");
                if (!fx) {
                    throw new Error("AE could not add ADBE Minimax");
                }
                applyEffectParams(fx, def.params);
            }

            built.push({ id: def.id, title: def.title, comp: comp, params: def.params, role: def.role });
        }
        return built;
    }

    function applyEffectParams(fx, params) {
        var keys = ["0001", "0002", "0003"];
        for (var i = 0; i < keys.length; i++) {
            var key = keys[i];
            var prop = fx.property(parseInt(key, 10));
            if (prop) {
                prop.setValue(params[key]);
            }
        }
    }

    function enqueueCases(cases, goldenDir) {
        for (var i = 0; i < cases.length; i++) {
            var caseDir = new Folder(goldenDir.fsName + "/" + cases[i].id);
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

    function writePropertyDump(caseDefs, metadataDir, source, cfg, folder) {
        var dumpComp = app.project.items.addComp("MINIMAX_METADATA_DUMP_COMP", cfg.width, cfg.height, 1, cfg.duration, cfg.fps);
        dumpComp.parentFolder = folder;
        var baseLayer = dumpComp.layers.add(source);
        baseLayer.property("ADBE Transform Group").property("ADBE Position").setValue([256, 256]);
        baseLayer.property("ADBE Transform Group").property("ADBE Scale").setValue([70, 70]);

        var dump = {
            pack_id: "minimax_enum_channel_probe_v1",
            effect_match_name: "ADBE Minimax",
            notes: [
                "AE scripting exposes property index/display name/match name/value.",
                "If AE does not expose selected popup labels via scripting, use screenshots or UI inspection with these exact values."
            ],
            source: {
                generated_precomp: "MINIMAX_SRC_alpha_square_like",
                precomp_size: [cfg.sourceSize, cfg.sourceSize],
                opaque_square_size: [cfg.squareSize, cfg.squareSize],
                placement_position: [256, 256],
                placement_scale_percent: [70, 70]
            },
            row_sample_request: {
                y: cfg.rowSampleY,
                x_start: cfg.rowSampleXStart,
                x_end: cfg.rowSampleXEnd
            },
            snapshots: []
        };

        for (var i = 0; i < caseDefs.length; i++) {
            var def = caseDefs[i];
            if (!def.params) {
                continue;
            }
            var layer = dumpComp.layers.add(source);
            layer.name = "metadata_" + def.id;
            layer.property("ADBE Transform Group").property("ADBE Position").setValue([256, 256]);
            layer.property("ADBE Transform Group").property("ADBE Scale").setValue([70, 70]);

            var fx = layer.property("ADBE Effect Parade").addProperty("ADBE Minimax");
            applyEffectParams(fx, def.params);

            dump.snapshots.push({
                case_id: def.id,
                role: def.role,
                requested_params: cloneParams(def.params),
                effect_name: safeString(fx.name),
                effect_match_name: safeString(fx.matchName),
                properties: collectProperties(fx)
            });
        }

        var outFile = new File(metadataDir.fsName + "/minimax_property_dump.json");
        outFile.encoding = "UTF-8";
        outFile.open("w");
        outFile.write(toJson(dump, 0));
        outFile.close();
    }

    function collectProperties(group) {
        var props = [];
        for (var i = 1; i <= group.numProperties; i++) {
            var prop = group.property(i);
            props.push({
                index: i,
                name: safeString(prop.name),
                match_name: safeString(prop.matchName),
                property_value_type: propertyValueTypeName(prop),
                value: propertyValue(prop),
                can_set_expression: safeBoolean(prop.canSetExpression),
                is_time_varying: safeBoolean(prop.isTimeVarying)
            });
        }
        return props;
    }

    function cloneParams(params) {
        return {
            "0001": params["0001"],
            "0002": params["0002"],
            "0003": params["0003"]
        };
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

    function propertyValueTypeName(prop) {
        try {
            switch (prop.propertyValueType) {
                case PropertyValueType.NO_VALUE:
                    return "NO_VALUE";
                case PropertyValueType.ThreeD_SPATIAL:
                    return "ThreeD_SPATIAL";
                case PropertyValueType.ThreeD:
                    return "ThreeD";
                case PropertyValueType.TwoD_SPATIAL:
                    return "TwoD_SPATIAL";
                case PropertyValueType.TwoD:
                    return "TwoD";
                case PropertyValueType.OneD:
                    return "OneD";
                case PropertyValueType.COLOR:
                    return "COLOR";
                case PropertyValueType.CUSTOM_VALUE:
                    return "CUSTOM_VALUE";
                case PropertyValueType.MARKER:
                    return "MARKER";
                case PropertyValueType.LAYER_INDEX:
                    return "LAYER_INDEX";
                case PropertyValueType.MASK_INDEX:
                    return "MASK_INDEX";
                case PropertyValueType.SHAPE:
                    return "SHAPE";
                case PropertyValueType.TEXT_DOCUMENT:
                    return "TEXT_DOCUMENT";
                default:
                    return String(prop.propertyValueType);
            }
        } catch (_err) {
            return "UNKNOWN";
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

    function safeBoolean(value) {
        try {
            return value ? true : false;
        } catch (_err) {
            return false;
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
        var objParts = [];
        for (var key in value) {
            if (value.hasOwnProperty(key)) {
                objParts.push(childPad + quoteJson(key) + ": " + toJson(value[key], indent + 1));
            }
        }
        if (objParts.length === 0) {
            return "{}";
        }
        return "{\n" + objParts.join(",\n") + "\n" + pad + "}";
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
            folder.create();
        }
    }
})();
