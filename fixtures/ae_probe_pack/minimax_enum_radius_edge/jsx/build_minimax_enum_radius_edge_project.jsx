/*
AE Minimax enum/radius/edge probe pack builder.

Run from After Effects:
  File > Scripts > Run Script File... >
  fixtures/ae_probe_pack/minimax_enum_radius_edge/jsx/build_minimax_enum_radius_edge_project.jsx

The script imports deterministic PNG primitives, builds one-frame case comps,
queues PNG-with-alpha renders, and dumps ADBE Minimax property metadata.
*/

(function buildMinimaxEnumRadiusEdgeProject() {
    app.beginUndoGroup("Build AE Minimax Enum Radius Edge Probe Pack");
    try {
        var SCRIPT_FILE = new File($.fileName);
        var PACK_DIR = SCRIPT_FILE.parent.parent;
        var PNG_DIR = new Folder(PACK_DIR.fsName + "/ae_goldens/png");
        var METADATA_DIR = new Folder(PACK_DIR.fsName + "/ae_goldens/metadata");
        ensureTree(PNG_DIR);
        ensureTree(METADATA_DIR);

        var CFG = {
            width: 128,
            height: 128,
            fps: 1,
            duration: 1.0,
            bitsPerChannel: 8
        };

        if (!app.project) {
            app.newProject();
        }
        app.project.bitsPerChannel = CFG.bitsPerChannel;

        var folders = {
            root: getOrCreateFolder("AE_MINIMAX_ENUM_RADIUS_EDGE"),
            sources: getOrCreateFolder("AE_MINIMAX_ENUM_RADIUS_EDGE/sources"),
            cases: getOrCreateFolder("AE_MINIMAX_ENUM_RADIUS_EDGE/cases")
        };

        var sourceDefs = getSourceDefs(PACK_DIR);
        var sources = importSources(sourceDefs, folders.sources);
        var caseDefs = getCaseDefs();
        var builtCases = buildCases(CFG, sources, caseDefs, folders.cases);
        enqueueCases(builtCases, PNG_DIR);
        writePropertyDump(caseDefs, sourceDefs, METADATA_DIR, CFG, sources);

        alert(
            "AE Minimax enum/radius/edge probe pack created.\n\n" +
            "Case comps queued: " + builtCases.length + "\n" +
            "Baseline bpc: " + CFG.bitsPerChannel + "\n" +
            "PNG output root:\n" + PNG_DIR.fsName + "\n\n" +
            "Metadata dump:\n" +
            METADATA_DIR.fsName + "/minimax_enum_radius_edge_property_dump.json"
        );
    } catch (err) {
        alert("Minimax enum/radius/edge probe build failed:\n" + err.toString());
        throw err;
    } finally {
        app.endUndoGroup();
    }

    function getSourceDefs(packDir) {
        return [
            {
                id: "impulse_dot",
                file: new File(packDir.fsName + "/assets/primitives/impulse_dot.png")
            },
            {
                id: "ramp_steps",
                file: new File(packDir.fsName + "/assets/primitives/ramp_steps.png")
            },
            {
                id: "rgba_lanes",
                file: new File(packDir.fsName + "/assets/primitives/rgba_lanes.png")
            },
            {
                id: "orientation_impulses",
                file: new File(packDir.fsName + "/assets/primitives/orientation_impulses.png")
            },
            {
                id: "boundary_corner",
                file: new File(packDir.fsName + "/assets/primitives/boundary_corner.png")
            }
        ];
    }

    function getCaseDefs() {
        return [
            c("MMER_PRE_IMPULSE", "pre_reference", "impulse_dot", null),
            c("MMER_PRE_RAMP", "pre_reference", "ramp_steps", null),
            c("MMER_PRE_LANES", "pre_reference", "rgba_lanes", null),
            c("MMER_PRE_ORIENT", "pre_reference", "orientation_impulses", null),
            c("MMER_PRE_BOUNDARY", "pre_reference", "boundary_corner", null),

            c("MMER_OP1_IMPULSE_R8", "operation_probe", "impulse_dot", p(1, 8, 1, 1, 0)),
            c("MMER_OP2_IMPULSE_R8", "operation_probe", "impulse_dot", p(2, 8, 1, 1, 0)),
            c("MMER_OP3_IMPULSE_R8", "operation_probe", "impulse_dot", p(3, 8, 1, 1, 0)),
            c("MMER_OP4_IMPULSE_R8", "operation_probe", "impulse_dot", p(4, 8, 1, 1, 0)),
            c("MMER_OP1_RAMP_R8", "operation_probe", "ramp_steps", p(1, 8, 1, 1, 0)),
            c("MMER_OP2_RAMP_R8", "operation_probe", "ramp_steps", p(2, 8, 1, 1, 0)),
            c("MMER_OP3_RAMP_R8", "operation_probe", "ramp_steps", p(3, 8, 1, 1, 0)),
            c("MMER_OP4_RAMP_R8", "operation_probe", "ramp_steps", p(4, 8, 1, 1, 0)),

            c("MMER_CH1_LANES_R6", "channel_probe", "rgba_lanes", p(2, 6, 1, 1, 0)),
            c("MMER_CH2_LANES_R6", "channel_probe", "rgba_lanes", p(2, 6, 2, 1, 0)),
            c("MMER_CH3_LANES_R6", "channel_probe", "rgba_lanes", p(2, 6, 3, 1, 0)),
            c("MMER_CH4_LANES_R6", "channel_probe", "rgba_lanes", p(2, 6, 4, 1, 0)),
            c("MMER_CH5_LANES_R6", "channel_probe", "rgba_lanes", p(2, 6, 5, 1, 0)),
            c("MMER_CH6_LANES_R6", "channel_probe", "rgba_lanes", p(2, 6, 6, 1, 0)),

            c("MMER_DIR1_ORIENT_R8", "direction_probe", "orientation_impulses", p(2, 8, 1, 1, 0)),
            c("MMER_DIR2_ORIENT_R8", "direction_probe", "orientation_impulses", p(2, 8, 1, 2, 0)),
            c("MMER_DIR3_ORIENT_R8", "direction_probe", "orientation_impulses", p(2, 8, 1, 3, 0)),

            c("MMER_RAD025_IMPULSE", "radius_probe", "impulse_dot", p(2, 0.25, 1, 1, 0)),
            c("MMER_RAD049_IMPULSE", "radius_probe", "impulse_dot", p(2, 0.49, 1, 1, 0)),
            c("MMER_RAD050_IMPULSE", "radius_probe", "impulse_dot", p(2, 0.5, 1, 1, 0)),
            c("MMER_RAD051_IMPULSE", "radius_probe", "impulse_dot", p(2, 0.51, 1, 1, 0)),
            c("MMER_RAD149_IMPULSE", "radius_probe", "impulse_dot", p(2, 1.49, 1, 1, 0)),
            c("MMER_RAD150_IMPULSE", "radius_probe", "impulse_dot", p(2, 1.5, 1, 1, 0)),
            c("MMER_RAD151_IMPULSE", "radius_probe", "impulse_dot", p(2, 1.51, 1, 1, 0)),

            c("MMER_EDGE0_BOUNDARY_R8", "boundary_probe", "boundary_corner", p(2, 8, 1, 1, 0)),
            c("MMER_EDGE1_BOUNDARY_R8", "boundary_probe", "boundary_corner", p(2, 8, 1, 1, 1))
        ];
    }

    function c(id, role, sourceId, params) {
        return {
            id: id,
            role: role,
            source_id: sourceId,
            effect_params: params
        };
    }

    function p(op, radius, channel, direction, boundary) {
        return {
            "0001": op,
            "0002": radius,
            "0003": channel,
            "0004": direction,
            "0005": boundary
        };
    }

    function importSources(sourceDefs, folder) {
        var out = {};
        for (var i = 0; i < sourceDefs.length; i++) {
            var def = sourceDefs[i];
            if (!def.file.exists) {
                throw new Error(
                    "Missing asset " + def.file.fsName +
                    ". Run scripts/measure_minimax_enum_radius_edge.py --generate-assets first."
                );
            }
            var options = new ImportOptions(def.file);
            var item = app.project.importFile(options);
            item.name = "MMER_SRC_" + def.id;
            item.parentFolder = folder;
            out[def.id] = item;
        }
        return out;
    }

    function buildCases(cfg, sources, caseDefs, folder) {
        var built = [];
        for (var i = 0; i < caseDefs.length; i++) {
            var def = caseDefs[i];
            var source = sources[def.source_id];
            if (!source) {
                throw new Error("Missing imported source for " + def.source_id);
            }
            var comp = app.project.items.addComp(def.id, cfg.width, cfg.height, 1, cfg.duration, cfg.fps);
            comp.parentFolder = folder;
            comp.bgColor = [0, 0, 0];
            comp.workAreaStart = 0;
            comp.workAreaDuration = cfg.duration;

            var layer = comp.layers.add(source);
            layer.name = def.source_id;
            layer.property("ADBE Transform Group").property("ADBE Position").setValue([cfg.width / 2, cfg.height / 2]);
            layer.inPoint = 0;
            layer.outPoint = cfg.duration;

            var applied = [];
            if (def.effect_params) {
                var fx = layer.property("ADBE Effect Parade").addProperty("ADBE Minimax");
                if (!fx) {
                    throw new Error("AE could not add ADBE Minimax");
                }
                applied = applyEffectParams(fx, def.effect_params);
            }
            built.push({
                id: def.id,
                role: def.role,
                source_id: def.source_id,
                effect_params: def.effect_params,
                comp: comp,
                applied_properties: applied
            });
        }
        return built;
    }

    function applyEffectParams(fx, params) {
        var keys = ["0001", "0002", "0003", "0004", "0005"];
        var applied = [];
        for (var i = 0; i < keys.length; i++) {
            var key = keys[i];
            var prop = fx.property(parseInt(key, 10));
            var record = {
                param: key,
                index: parseInt(key, 10),
                requested_value: params[key],
                applied: false,
                error: null
            };
            if (!prop) {
                record.error = "missing_property";
            } else {
                try {
                    prop.setValue(params[key]);
                    record.applied = true;
                    record.actual_value = propertyValue(prop);
                    record.name = safeString(prop.name);
                    record.match_name = safeString(prop.matchName);
                } catch (err) {
                    record.error = safeString(err);
                }
            }
            applied.push(record);
        }
        return applied;
    }

    function enqueueCases(cases, pngDir) {
        for (var i = 0; i < cases.length; i++) {
            var caseDir = new Folder(pngDir.fsName + "/" + cases[i].id);
            ensureTree(caseDir);
            var rq = app.project.renderQueue.items.add(cases[i].comp);
            var om = rq.outputModule(1);
            var templateName = "PNG Sequence with Alpha";
            try {
                om.applyTemplate(templateName);
            } catch (_err) {
                templateName = "PNG Sequence";
                try {
                    om.applyTemplate(templateName);
                } catch (_err2) {
                    templateName = "AE default output module";
                }
            }
            om.file = new File(caseDir.fsName + "/" + cases[i].id + "_[#####].png");
            cases[i].output_template = templateName;
            cases[i].output_pattern = om.file.fsName;
        }
    }

    function writePropertyDump(caseDefs, sourceDefs, metadataDir, cfg, sources) {
        var dump = {
            pack_id: "minimax_enum_radius_edge_v1",
            effect_match_name: "ADBE Minimax",
            baseline_bits_per_channel: app.project.bitsPerChannel,
            composition: {
                width: cfg.width,
                height: cfg.height,
                fps: cfg.fps,
                duration_seconds: cfg.duration
            },
            notes: [
                "Properties are addressed as 0001..0005 by AE effect property index.",
                "If popup labels are not exposed through scripting, use requested values plus this property dump."
            ],
            sources: [],
            cases: []
        };

        for (var i = 0; i < sourceDefs.length; i++) {
            dump.sources.push({
                id: sourceDefs[i].id,
                path: sourceDefs[i].file.fsName,
                imported_name: safeString(sources[sourceDefs[i].id].name)
            });
        }

        for (var j = 0; j < caseDefs.length; j++) {
            var def = caseDefs[j];
            var snapshot = {
                case_id: def.id,
                role: def.role,
                source_id: def.source_id,
                requested_params: cloneParams(def.effect_params),
                effect: null
            };
            if (def.effect_params) {
                var comp = app.project.items.addComp("MMER_METADATA_" + def.id, cfg.width, cfg.height, 1, cfg.duration, cfg.fps);
                var layer = comp.layers.add(sources[def.source_id]);
                var fx = layer.property("ADBE Effect Parade").addProperty("ADBE Minimax");
                var applied = applyEffectParams(fx, def.effect_params);
                snapshot.effect = {
                    name: safeString(fx.name),
                    match_name: safeString(fx.matchName),
                    applied_properties: applied,
                    properties: collectProperties(fx)
                };
            }
            dump.cases.push(snapshot);
        }

        var outFile = new File(metadataDir.fsName + "/minimax_enum_radius_edge_property_dump.json");
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
        if (!params) {
            return null;
        }
        return {
            "0001": params["0001"],
            "0002": params["0002"],
            "0003": params["0003"],
            "0004": params["0004"],
            "0005": params["0005"]
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

    function ensureTree(folder) {
        if (folder.exists) {
            return;
        }
        var parent = folder.parent;
        if (parent && !parent.exists) {
            ensureTree(parent);
        }
        folder.create();
    }
})();
