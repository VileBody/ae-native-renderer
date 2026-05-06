/*
Turbulent Displace coordinate-field AE probe pack.

Run from After Effects:
  File > Scripts > Run Script File... > build_turbulent_field_probe_project.jsx

The script creates one comp per probe variant and queues PNG sequences. It also
writes a small effect-property dump so enum/control indices can be confirmed
before native formula work.
*/

(function buildTurbulentFieldProbeProject() {
    app.beginUndoGroup("Build Turbulent Field Probe Pack");

    var SCRIPT_FILE = new File($.fileName);
    var PACK_DIR = SCRIPT_FILE.parent.parent;
    var ASSET_DIR = new Folder(PACK_DIR.fsName + "/assets/primitives");
    var OUT_DIR = new Folder(PACK_DIR.fsName + "/ae_goldens/png8");
    var LOG_DIR = new Folder(PACK_DIR.fsName + "/ae_goldens/logs");
    ensureFolder(OUT_DIR);
    ensureFolder(LOG_DIR);

    var LOG_FILE = new File(LOG_DIR.fsName + "/turbulent_field_builder_log.txt");
    LOG_FILE.open("w");
    logLine("Turbulent field probe builder");
    logLine("Pack: " + PACK_DIR.fsName);
    purgeAeCaches("before building probes");

    var CFG = {
        width: 512,
        height: 512,
        fps: 30,
        duration: 2.0,
        bg: [0, 0, 0]
    };

    if (!app.project) {
        app.newProject();
    }
    app.project.bitsPerChannel = 8;

    var folders = {
        root: getOrCreateFolder("AE_TURBULENT_FIELD_PROBE_PACK"),
        footage: getOrCreateFolder("AE_TURBULENT_FIELD_PROBE_PACK/footage"),
        cases: getOrCreateFolder("AE_TURBULENT_FIELD_PROBE_PACK/cases")
    };

    var assets = importAssets(folders.footage);
    var cases = [];
    var dumpedProperties = false;

    addSweep(cases, assets.coordinate, "TD_AMOUNT_SWEEP", "amount", [0, 1, 10, 45, 100], {
        amount: 45,
        size: 65,
        complexity: 2,
        evolution: 0
    });
    addSweep(cases, assets.coordinate, "TD_SIZE_SWEEP", "size", [1, 8, 16, 32, 65, 128, 256], {
        amount: 45,
        size: 65,
        complexity: 2,
        evolution: 0
    });
    addSweep(cases, assets.coordinate, "TD_COMPLEXITY_SWEEP", "complexity", [1, 2, 3, 4, 6], {
        amount: 45,
        size: 65,
        complexity: 2,
        evolution: 0
    });
    addSweep(cases, assets.coordinate, "TD_EVOLUTION_STATIC", "evolution", [0, 45, 90, 180, 360, 720], {
        amount: 45,
        size: 65,
        complexity: 2,
        evolution: 0
    });

    createProbe(cases, "TD_EVOLUTION_ANIM", "evolution 0 to 180 over 2s", assets.coordinate, {
        amount: 45,
        size: 65,
        complexity: 2,
        evolution: 0
    }, {
        animateEvolution: true
    });

    for (var displacement = 1; displacement <= 9; displacement++) {
        createProbe(cases, "TD_DISPLACEMENT_TYPE_" + two(displacement), "displacement type value " + displacement, assets.coordinate, {
            displacement: displacement,
            amount: 45,
            size: 65,
            complexity: 2,
            evolution: 0
        }, {});
    }

    var seeds = [0, 1, 2, 10, 999];
    for (var s = 0; s < seeds.length; s++) {
        createProbe(cases, "TD_SEED_SWEEP_" + seeds[s], "seed value " + seeds[s] + " if AE exposes one", assets.coordinate, {
            amount: 45,
            size: 65,
            complexity: 2,
            evolution: 0,
            seed: seeds[s]
        }, {
            setSeedByName: true
        });
    }

    createProbe(cases, "TD_PINNING_EDGE_OFF", "pinning off over hard edge alpha ramp", assets.edgeAlpha, {
        amount: 90,
        size: 65,
        complexity: 2,
        evolution: 0
    }, {
        pinning: 0
    });
    createProbe(cases, "TD_PINNING_EDGE_ON", "pinning on over hard edge alpha ramp", assets.edgeAlpha, {
        amount: 90,
        size: 65,
        complexity: 2,
        evolution: 0
    }, {
        pinning: 1
    });

    createProbe(cases, "TD_RESIZE_LAYER_OFF", "resize layer off near borders", assets.coordinate, {
        amount: 120,
        size: 48,
        complexity: 2,
        evolution: 0
    }, {
        resizeLayer: 0
    });
    createProbe(cases, "TD_RESIZE_LAYER_ON", "resize layer on near borders", assets.coordinate, {
        amount: 120,
        size: 48,
        complexity: 2,
        evolution: 0
    }, {
        resizeLayer: 1
    });

    addSamplerProbe(cases, assets.coordinate, "COORD", [1, 2, 4]);
    addSamplerProbe(cases, assets.checker, "CHECKER", [1, 2, 4]);
    createProbe(cases, "TD_IMPULSE_GRID_A045", "sparse impulse grid amount 45", assets.impulseGrid, {
        amount: 45,
        size: 65,
        complexity: 2,
        evolution: 0
    }, {});

    enqueueCases(cases, OUT_DIR);
    purgeAeCaches("after queueing probes");
    LOG_FILE.close();

    alert(
        "Turbulent field probe pack created.\n\n" +
        "Case comps: " + cases.length + "\n" +
        "Output: " + OUT_DIR.fsName + "\n" +
        "Log: " + LOG_FILE.fsName + "\n\n" +
        "Render queued case comps as PNG sequences."
    );

    app.endUndoGroup();

    function addSweep(cases, source, prefix, param, values, baseParams) {
        for (var i = 0; i < values.length; i++) {
            var params = cloneParams(baseParams);
            params[param] = values[i];
            createProbe(cases, prefix + "_" + valueToken(param, values[i]), param + " " + values[i], source, params, {});
        }
    }

    function addSamplerProbe(cases, source, sourceToken, amounts) {
        for (var i = 0; i < amounts.length; i++) {
            createProbe(cases, "TD_SAMPLER_CHECK_" + sourceToken + "_A" + pad3(amounts[i]), "sampler source " + sourceToken + " amount " + amounts[i], source, {
                amount: amounts[i],
                size: 256,
                complexity: 1,
                evolution: 0
            }, {});
        }
    }

    function createProbe(cases, id, title, source, params, options) {
        var comp = app.project.items.addComp(id + "__" + title, CFG.width, CFG.height, 1, CFG.duration, CFG.fps);
        comp.parentFolder = folders.cases;
        comp.bgColor = CFG.bg;
        var layer = comp.layers.add(source);
        layer.name = id + "_source";
        setLayerPosition(layer, [CFG.width / 2, CFG.height / 2]);
        setLayerScale(layer, [100, 100]);
        layer.inPoint = 0;
        layer.outPoint = CFG.duration;

        var fx = layer.property("ADBE Effect Parade").addProperty("ADBE Turbulent Displace");
        if (!fx) {
            throw new Error("AE could not add ADBE Turbulent Displace");
        }
        if (!dumpedProperties) {
            dumpEffectProperties(fx);
            dumpedProperties = true;
        }

        if (params.displacement !== undefined) {
            setIndexedScalar(fx, 1, params.displacement, id, "displacement");
        }
        setIndexedScalar(fx, 2, params.amount, id, "amount");
        setIndexedScalar(fx, 3, params.size, id, "size");
        setIndexedScalar(fx, 5, params.complexity, id, "complexity");

        if (options.animateEvolution) {
            setAnimatedIndexedScalar(fx, 6, 0, 0, CFG.duration, 180, id, "evolution");
        } else {
            setIndexedScalar(fx, 6, params.evolution, id, "evolution");
        }

        if (options.setSeedByName) {
            setFirstNamedScalar(fx, /seed|random/i, params.seed, id, "seed/random");
        }
        if (options.pinning !== undefined) {
            setFirstNamedScalar(fx, /pin/i, options.pinning, id, "pinning");
        }
        if (options.resizeLayer !== undefined) {
            setFirstNamedScalar(fx, /resize/i, options.resizeLayer, id, "resize layer");
        }

        cases.push({ id: id, title: title, comp: comp });
    }

    function importAssets(folder) {
        return {
            coordinate: importStill("coordinate_field", "coordinate_field.png", folder),
            impulseGrid: importStill("sparse_impulse_grid", "sparse_impulse_grid.png", folder),
            edgeAlpha: importStill("hard_edge_alpha_ramp", "hard_edge_alpha_ramp.png", folder),
            checker: importStill("unique_checkerboard", "unique_checkerboard.png", folder)
        };
    }

    function importStill(name, relative, folder) {
        var file = new File(ASSET_DIR.fsName + "/" + relative);
        if (!file.exists) {
            throw new Error("Missing primitive asset: " + file.fsName);
        }
        var options = new ImportOptions(file);
        options.importAs = ImportAsType.FOOTAGE;
        var item = app.project.importFile(options);
        item.name = name;
        item.parentFolder = folder;
        return item;
    }

    function setIndexedScalar(fx, index, value, id, label) {
        var prop = fx.property(index);
        if (!prop) {
            logLine(id + ": missing indexed control " + index + " for " + label);
            return false;
        }
        try {
            prop.setValue(value);
            return true;
        } catch (err) {
            logLine(id + ": failed setting " + label + " index " + index + " to " + value + ": " + err.toString());
            return false;
        }
    }

    function setAnimatedIndexedScalar(fx, index, t0, v0, t1, v1, id, label) {
        var prop = fx.property(index);
        if (!prop) {
            logLine(id + ": missing indexed control " + index + " for animated " + label);
            return false;
        }
        try {
            prop.setValueAtTime(t0, v0);
            prop.setValueAtTime(t1, v1);
            prop.setInterpolationTypeAtKey(1, KeyframeInterpolationType.LINEAR, KeyframeInterpolationType.LINEAR);
            prop.setInterpolationTypeAtKey(2, KeyframeInterpolationType.LINEAR, KeyframeInterpolationType.LINEAR);
            return true;
        } catch (err) {
            logLine(id + ": failed animating " + label + " index " + index + ": " + err.toString());
            return false;
        }
    }

    function setFirstNamedScalar(fx, regex, value, id, label) {
        for (var i = 1; i <= fx.numProperties; i++) {
            var prop = fx.property(i);
            if (!prop) {
                continue;
            }
            var name = String(prop.name || "");
            var matchName = String(prop.matchName || "");
            if (regex.test(name) || regex.test(matchName)) {
                try {
                    prop.setValue(value);
                    logLine(id + ": set " + label + " via property " + i + " '" + name + "' to " + value);
                    return true;
                } catch (err) {
                    logLine(id + ": failed setting named " + label + " property " + i + " '" + name + "': " + err.toString());
                    return false;
                }
            }
        }
        logLine(id + ": no property matched " + label + " regex; variant may equal default AE behavior");
        return false;
    }

    function dumpEffectProperties(fx) {
        logLine("");
        logLine("ADBE Turbulent Displace property dump:");
        for (var i = 1; i <= fx.numProperties; i++) {
            var prop = fx.property(i);
            if (prop) {
                logLine(i + "\tname='" + prop.name + "'\tmatchName='" + prop.matchName + "'\tvalue='" + safeValueString(prop) + "'");
            }
        }
        logLine("");
    }

    function safeValueString(prop) {
        try {
            return String(prop.value);
        } catch (_err) {
            return "<unreadable>";
        }
    }

    function cloneParams(input) {
        var out = {};
        for (var key in input) {
            if (input.hasOwnProperty(key)) {
                out[key] = input[key];
            }
        }
        return out;
    }

    function valueToken(param, value) {
        if (param === "amount") {
            return "A" + pad3(value);
        }
        if (param === "size") {
            return "S" + pad3(value);
        }
        if (param === "complexity") {
            return "C" + pad2(value);
        }
        if (param === "evolution") {
            return "E" + pad3(value);
        }
        return String(value);
    }

    function pad2(value) {
        var text = String(value);
        while (text.length < 2) {
            text = "0" + text;
        }
        return text;
    }

    function pad3(value) {
        var text = String(value);
        while (text.length < 3) {
            text = "0" + text;
        }
        return text;
    }

    function two(value) {
        return pad2(value);
    }

    function setLayerPosition(layer, value) {
        layer.property("ADBE Transform Group").property("ADBE Position").setValue(value);
    }

    function setLayerScale(layer, value) {
        layer.property("ADBE Transform Group").property("ADBE Scale").setValue(value);
    }

    function enqueueCases(cases, outDir) {
        for (var i = 0; i < cases.length; i++) {
            var caseDir = new Folder(outDir.fsName + "/" + cases[i].id);
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

    function purgeAeCaches(label) {
        try {
            app.purge(PurgeTarget.ALL_CACHES);
            logLine(label + ": purged AE memory/disk caches");
        } catch (err) {
            logLine(label + ": cache purge failed: " + err.toString());
        }
    }

    function logLine(line) {
        LOG_FILE.writeln(line);
    }
})();
