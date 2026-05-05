/*
AE Drop Shadow / Box Blur discriminator probe pack builder.

Run from After Effects:
  File > Scripts > Run Script File... > build_shadow_blur_discriminator_project.jsx

The script creates procedural source comps, imports the generated RGB-noise
alpha asset, builds one label-free comp per case, queues PNG sequence renders,
and writes a small property dump for effect index verification.
*/

(function buildShadowBlurDiscriminatorProject() {
    app.beginUndoGroup("Build Shadow / Blur Discriminator Probe Pack");

    var SCRIPT_FILE = new File($.fileName);
    var PACK_DIR = SCRIPT_FILE.parent.parent;
    var GOLDEN_DIR = new Folder(PACK_DIR.fsName + "/ae_goldens/png");
    var METADATA_DIR = new Folder(PACK_DIR.fsName + "/ae_goldens/metadata");
    ensureFolder(GOLDEN_DIR);
    ensureFolder(METADATA_DIR);

    var CFG = {
        width: 512,
        height: 512,
        fps: 30,
        duration: 1.0,
        bg: [0, 0, 0],
        rectW: 80,
        rectH: 64,
        distanceDir: 23,
        distanceSoft: 19,
        softDirection: 135,
        shadowOpacity: 180,
        colorShadowOpacity: 130
    };

    if (!app.project) {
        app.newProject();
    }
    app.project.bitsPerChannel = 8;

    var folders = {
        root: getOrCreateFolder("AE_SHADOW_BLUR_DISCRIMINATOR"),
        sources: getOrCreateFolder("AE_SHADOW_BLUR_DISCRIMINATOR/sources"),
        cases: getOrCreateFolder("AE_SHADOW_BLUR_DISCRIMINATOR/cases"),
        metadata: getOrCreateFolder("AE_SHADOW_BLUR_DISCRIMINATOR/metadata")
    };

    var sources = buildSources(CFG, folders.sources, PACK_DIR);
    var caseDefs = getCaseDefs();
    var cases = buildCases(CFG, sources, caseDefs, folders.cases);
    enqueueCases(cases, GOLDEN_DIR);
    writePropertyDump(caseDefs, METADATA_DIR, sources.hardAlpha, CFG, folders.metadata);

    alert(
        "Shadow / Blur discriminator probe pack created.\n\n" +
        "Case comps queued: " + cases.length + "\n" +
        "PNG output root:\n" + GOLDEN_DIR.fsName + "\n\n" +
        "Render queued case comps as PNG sequences with RGB + Alpha."
    );

    app.endUndoGroup();

    function buildSources(cfg, folder, packDir) {
        return {
            hardAlpha: buildHardAlphaSource(cfg, folder, "SHBL_SRC_PRECOMP_hard_alpha", [1, 1, 1], 100),
            hardBlackAlpha: buildHardAlphaSource(cfg, folder, "SHBL_SRC_PRECOMP_black_alpha", [0, 0, 0], 100),
            translucentColor: buildHardAlphaSource(cfg, folder, "SHBL_SRC_PRECOMP_translucent_color", [1, 0.31, 0.12], 55),
            noisyAlpha: importNoisyAlphaAsset(folder, packDir)
        };
    }

    function buildHardAlphaSource(cfg, folder, name, color, opacity) {
        var comp = app.project.items.addComp(name, cfg.width, cfg.height, 1, cfg.duration, cfg.fps);
        comp.parentFolder = folder;
        comp.bgColor = cfg.bg;
        var layer = comp.layers.addSolid(color, name + "_rect", cfg.rectW, cfg.rectH, 1, cfg.duration);
        setLayerPosition(layer, [256, 256]);
        setLayerOpacity(layer, opacity);
        layer.inPoint = 0;
        layer.outPoint = cfg.duration;
        return comp;
    }

    function importNoisyAlphaAsset(folder, packDir) {
        var file = new File(packDir.fsName + "/assets/noisy_rgb_under_alpha.png");
        if (!file.exists) {
            throw new Error(
                "Missing asset: " + file.fsName + "\n" +
                "Run: python3 scripts/measure_shadow_blur_discriminator.py --generate-assets"
            );
        }
        var opts = new ImportOptions(file);
        var item = app.project.importFile(opts);
        item.name = "SHBL_SRC_noisy_rgb_under_alpha_png";
        item.parentFolder = folder;
        return item;
    }

    function getCaseDefs() {
        var defs = [
            { id: "SHBL_SRC_HARD_ALPHA", family: "source", role: "hard alpha source" },
            { id: "SHBL_SRC_RGB_NOISE_ALPHA", family: "source", role: "RGB noise under alpha source" },
            { id: "SHBL_SRC_TRANSLUCENT_COLOR", family: "source", role: "colored translucent source" }
        ];
        var dirs = [0, 30, 45, 90, 120, 135, 180, 210, 225, 270, 300, 315];
        for (var i = 0; i < dirs.length; i++) {
            defs.push({
                id: "SHBL_DIR_" + pad3(dirs[i]),
                family: "direction_sweep",
                role: "softness 0 shadow-only direction probe",
                direction: dirs[i],
                distance: 23,
                softness: 0,
                shadowOnly: 1
            });
        }
        var softness = [0, 1, 2, 4, 8, 12, 18, 32];
        for (var j = 0; j < softness.length; j++) {
            defs.push({
                id: "SHBL_SOFT_" + pad3(softness[j]),
                family: "softness_sweep",
                role: "Drop Shadow softness sweep",
                direction: 135,
                distance: 19,
                softness: softness[j],
                shadowOnly: 1
            });
        }
        addBoxDefs(defs, 8, 11.2, 3);
        addBoxDefs(defs, 12, 16.8, 5);
        addBoxDefs(defs, 18, 25.2, 7);
        defs.push({
            id: "SHBL_NOISE_DSH_SHONLY",
            family: "alpha_only_rgb_noise",
            role: "Drop Shadow shadow-only on RGB-noise transparent source",
            direction: 135,
            distance: 19,
            softness: 12,
            shadowOnly: 1
        });
        defs.push({
            id: "SHBL_NOISE_BOX_RGBA",
            family: "alpha_only_rgb_noise",
            role: "Box Blur applied directly to RGB-noise transparent source",
            boxRadius: 5,
            boxIterations: 3
        });
        defs.push({
            id: "SHBL_COLOR_SHONLY_1",
            family: "colored_translucent_composite",
            role: "Colored translucent source, Drop Shadow shadow-only on",
            direction: 315,
            distance: 18,
            softness: 8,
            shadowOnly: 1
        });
        defs.push({
            id: "SHBL_COLOR_SHONLY_0",
            family: "colored_translucent_composite",
            role: "Colored translucent source, Drop Shadow shadow-only off",
            direction: 315,
            distance: 18,
            softness: 8,
            shadowOnly: 0
        });
        return defs;
    }

    function addBoxDefs(defs, softness, radiusA, radiusB) {
        defs.push({
            id: "SHBL_BB_S" + pad2(softness) + "_R" + String(radiusA).replace(".", "P") + "_I1",
            family: "box_blur_candidate",
            role: "Box Blur candidate: radius softness*1.4, 1 iteration",
            direction: 135,
            distance: 19,
            softness: softness,
            boxRadius: radiusA,
            boxIterations: 1
        });
        defs.push({
            id: "SHBL_BB_S" + pad2(softness) + "_R" + pad2(radiusB) + "_I3_DIV271",
            family: "box_blur_candidate",
            role: "Box Blur candidate: radius ceil(softness/2.71), 3 iterations",
            direction: 135,
            distance: 19,
            softness: softness,
            boxRadius: radiusB,
            boxIterations: 3
        });
    }

    function buildCases(cfg, sources, caseDefs, folder) {
        var built = [];
        for (var i = 0; i < caseDefs.length; i++) {
            var def = caseDefs[i];
            var comp = app.project.items.addComp(def.id + "__" + def.role, cfg.width, cfg.height, 1, cfg.duration, cfg.fps);
            comp.parentFolder = folder;
            comp.bgColor = cfg.bg;
            buildCaseComp(comp, def, cfg, sources);
            built.push({ id: def.id, title: def.role, comp: comp, def: def });
        }
        return built;
    }

    function buildCaseComp(comp, def, cfg, sources) {
        if (def.id === "SHBL_SRC_HARD_ALPHA") {
            placeComp(comp, sources.hardAlpha, [256, 256], [100, 100]);
            return;
        }
        if (def.id === "SHBL_SRC_RGB_NOISE_ALPHA") {
            placeComp(comp, sources.noisyAlpha, [256, 256], [100, 100]);
            return;
        }
        if (def.id === "SHBL_SRC_TRANSLUCENT_COLOR") {
            placeComp(comp, sources.translucentColor, [256, 256], [100, 100]);
            return;
        }
        if (def.family === "direction_sweep" || def.family === "softness_sweep") {
            var layer = placeComp(comp, sources.hardAlpha, [256, 256], [100, 100]);
            addDropShadow(layer, [1, 1, 1, 1], cfg.shadowOpacity, def.direction, def.distance, def.softness, def.shadowOnly);
            return;
        }
        if (def.family === "box_blur_candidate") {
            var off = aeOffset(def.direction, def.distance);
            var mask = placeComp(comp, sources.hardAlpha, [256 + off[0], 256 + off[1]], [100, 100]);
            addBoxBlur(mask, def.boxRadius, def.boxIterations);
            setLayerOpacity(mask, (cfg.shadowOpacity / 255) * 100);
            return;
        }
        if (def.id === "SHBL_NOISE_DSH_SHONLY") {
            var noiseShadow = placeComp(comp, sources.noisyAlpha, [256, 256], [100, 100]);
            addDropShadow(noiseShadow, [1, 1, 1, 1], cfg.shadowOpacity, def.direction, def.distance, def.softness, 1);
            return;
        }
        if (def.id === "SHBL_NOISE_BOX_RGBA") {
            var noiseBlur = placeComp(comp, sources.noisyAlpha, [256, 256], [100, 100]);
            addBoxBlur(noiseBlur, def.boxRadius, def.boxIterations);
            return;
        }
        if (def.family === "colored_translucent_composite") {
            var colorLayer = placeComp(comp, sources.translucentColor, [256, 256], [100, 100]);
            addDropShadow(colorLayer, [0.08, 0.55, 1, 1], cfg.colorShadowOpacity, def.direction, def.distance, def.softness, def.shadowOnly);
            return;
        }
        throw new Error("Unhandled case: " + def.id);
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

    function addBoxBlur(layer, radius, iterations) {
        return addEffect(layer, "ADBE Box Blur2", {
            "0001": radius,
            "0002": iterations
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
        var comp = app.project.items.addComp("SHBL_METADATA_DUMP_COMP", cfg.width, cfg.height, 1, cfg.duration, cfg.fps);
        comp.parentFolder = folder;

        var shadowLayer = comp.layers.add(source);
        setLayerPosition(shadowLayer, [256, 256]);
        var dsh = addDropShadow(shadowLayer, [1, 1, 1, 1], cfg.shadowOpacity, 135, 19, 12, 1);
        var shadowEffectDump = {
            name: safeString(dsh.name),
            match_name: safeString(dsh.matchName),
            requested_params: {
                "0001": [1, 1, 1, 1],
                "0002": cfg.shadowOpacity,
                "0003": 135,
                "0004": 19,
                "0005": 12,
                "0006": 1
            },
            properties: collectProperties(dsh)
        };

        var blurLayer = comp.layers.add(source);
        blurLayer.name = "metadata_box_blur_layer";
        setLayerPosition(blurLayer, [256, 256]);
        var blur = addBoxBlur(blurLayer, 5, 3);
        var blurEffectDump = {
            name: safeString(blur.name),
            match_name: safeString(blur.matchName),
            requested_params: {
                "0001": 5,
                "0002": 3
            },
            properties: collectProperties(blur)
        };

        var dump = {
            pack_id: "shadow_blur_discriminator_v1",
            effects: [shadowEffectDump, blurEffectDump],
            cases: compactCaseDefs(caseDefs)
        };
        var outFile = new File(metadataDir.fsName + "/shadow_blur_discriminator_property_dump.json");
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
                value: propertyValue(prop),
                property_value_type: propertyValueTypeName(prop)
            });
        }
        return props;
    }

    function compactCaseDefs(caseDefs) {
        var out = [];
        for (var i = 0; i < caseDefs.length; i++) {
            var def = caseDefs[i];
            out.push({
                id: def.id,
                family: def.family,
                role: def.role,
                direction: maybeNumber(def.direction),
                distance: maybeNumber(def.distance),
                softness: maybeNumber(def.softness),
                shadow_only: maybeNumber(def.shadowOnly),
                box_radius: maybeNumber(def.boxRadius),
                box_iterations: maybeNumber(def.boxIterations)
            });
        }
        return out;
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

    function aeOffset(direction, distance) {
        var radians = direction * Math.PI / 180;
        return [Math.cos(radians) * distance, Math.sin(radians) * distance];
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

    function maybeNumber(value) {
        return value === undefined ? null : value;
    }

    function pad2(value) {
        value = Math.round(value);
        return value < 10 ? "0" + value : String(value);
    }

    function pad3(value) {
        value = Math.round(value);
        if (value < 10) {
            return "00" + value;
        }
        if (value < 100) {
            return "0" + value;
        }
        return String(value);
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
})();
