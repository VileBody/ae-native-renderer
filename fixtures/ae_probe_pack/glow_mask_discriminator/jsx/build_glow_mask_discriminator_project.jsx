/*
AE Native Renderer Glow Mask Discriminator Probe Pack Builder

Run from After Effects:
  File > Scripts > Run Script File... > build_glow_mask_discriminator_project.jsx

The script creates procedural source comps, one label-free comp per probe case,
and render-queue entries for PNG sequence export. The outputs are probes, not
goldens, until rendered in AE and measured.
*/

(function buildGlowMaskDiscriminatorProbeProject() {
    app.beginUndoGroup("Build Glow Mask Discriminator Probe Pack");

    var SCRIPT_FILE = new File($.fileName);
    var PACK_DIR = SCRIPT_FILE.parent.parent;
    var PNG_DIR = new Folder(PACK_DIR.fsName + "/ae_probe_outputs/png");
    var PREVIEW_DIR = new Folder(PACK_DIR.fsName + "/ae_probe_outputs/preview");
    ensureFolder(PNG_DIR);
    ensureFolder(PREVIEW_DIR);

    var CFG = {
        width: 512,
        height: 512,
        fps: 30,
        duration: 1.0,
        bg: [0, 0, 0],
        threshold: 120,
        maskRadius: 0,
        maskIntensity: 1,
        intensityRadius: 5,
        compositeRadius: 10,
        radiusSweep: [
            { id: "R0", value: 0 },
            { id: "R0P5", value: 0.5 },
            { id: "R1", value: 1 },
            { id: "R2", value: 2 },
            { id: "R5", value: 5 },
            { id: "R10", value: 10 }
        ],
        intensitySweep: [
            { id: "I0", value: 0 },
            { id: "I0P5", value: 0.5 },
            { id: "I1", value: 1 },
            { id: "I1P25", value: 1.25 },
            { id: "I2", value: 2 }
        ]
    };

    if (!app.project) {
        app.newProject();
    }
    app.project.bitsPerChannel = 8;

    var folders = {
        root: getOrCreateFolder("AE_PROBE_GLOW_MASK_DISCRIMINATOR"),
        cases: getOrCreateFolder("AE_PROBE_GLOW_MASK_DISCRIMINATOR/cases"),
        precomps: getOrCreateFolder("AE_PROBE_GLOW_MASK_DISCRIMINATOR/precomps")
    };

    var sources = buildSources(CFG, folders);
    var cases = buildCases(CFG, sources, folders);
    var master = buildMasterReel(CFG, cases, folders);
    enqueueCases(cases, master, PNG_DIR, PREVIEW_DIR);

    alert(
        "Glow Mask Discriminator probe pack created.\n\n" +
        "Case comps: " + cases.length + "\n" +
        "Master comp: " + master.name + "\n\n" +
        "Render queued case comps as PNG sequences with RGB + Alpha."
    );

    app.endUndoGroup();

    function buildSources(cfg, folders) {
        return {
            maskSamples: buildMaskSamples(cfg, folders),
            impulse: buildImpulse(cfg, folders),
            compositeImpulse: buildCompositeImpulse(cfg, folders)
        };
    }

    function buildCases(cfg, sources, folders) {
        var cases = [
            createCase("GMD_SRC_MASK_SAMPLES", "Source RGBA split samples without Glow", function (comp) {
                placeComp(comp, sources.maskSamples, [256, 256], [100, 100]);
            }),
            createCase("GMD_MASK_DEFAULT", "Glow mask discriminator default based-on property", function (comp) {
                var layer = placeComp(comp, sources.maskSamples, [256, 256], [100, 100]);
                addGlow(layer, cfg.threshold, cfg.maskRadius, cfg.maskIntensity, null);
            }),
            createCase("GMD_MASK_BASEDON_1", "Glow mask discriminator based-on enum 1", function (comp) {
                var layer = placeComp(comp, sources.maskSamples, [256, 256], [100, 100]);
                addGlow(layer, cfg.threshold, cfg.maskRadius, cfg.maskIntensity, 1);
            }),
            createCase("GMD_MASK_BASEDON_2", "Glow mask discriminator based-on enum 2", function (comp) {
                var layer = placeComp(comp, sources.maskSamples, [256, 256], [100, 100]);
                addGlow(layer, cfg.threshold, cfg.maskRadius, cfg.maskIntensity, 2);
            })
        ];

        for (var r = 0; r < cfg.radiusSweep.length; r++) {
            (function (radiusCase) {
                cases.push(createCase("GMD_RADIUS_" + radiusCase.id, "Glow radius impulse radius " + radiusCase.value, function (comp) {
                    var layer = placeComp(comp, sources.impulse, [256, 256], [100, 100]);
                    addGlow(layer, cfg.threshold, radiusCase.value, 1, null);
                }));
            })(cfg.radiusSweep[r]);
        }

        for (var i = 0; i < cfg.intensitySweep.length; i++) {
            (function (intensityCase) {
                cases.push(createCase("GMD_INTENSITY_" + intensityCase.id, "Glow intensity impulse intensity " + intensityCase.value, function (comp) {
                    var layer = placeComp(comp, sources.impulse, [256, 256], [100, 100]);
                    addGlow(layer, cfg.threshold, cfg.intensityRadius, intensityCase.value, null);
                }));
            })(cfg.intensitySweep[i]);
        }

        cases.push(createCase("GMD_COMP_TRANSPARENT", "Glow composite on transparent background", function (comp) {
            var layer = placeComp(comp, sources.compositeImpulse, [256, 256], [100, 100]);
            addGlow(layer, cfg.threshold, cfg.compositeRadius, 1, null);
        }));
        cases.push(createCase("GMD_COMP_BLACK", "Glow composite on opaque black background", function (comp) {
            addSolidLayer(comp, "opaque_black_background", [0, 0, 0], [256, 256], cfg.width, cfg.height, 100);
            var layer = placeComp(comp, sources.compositeImpulse, [256, 256], [100, 100]);
            addGlow(layer, cfg.threshold, cfg.compositeRadius, 1, null);
        }));
        cases.push(createCase("GMD_COMP_BLACK_50A", "Glow composite on semitransparent black background", function (comp) {
            addSolidLayer(comp, "half_alpha_black_background", [0, 0, 0], [256, 256], cfg.width, cfg.height, 50);
            var layer = placeComp(comp, sources.compositeImpulse, [256, 256], [100, 100]);
            addGlow(layer, cfg.threshold, cfg.compositeRadius, 1, null);
        }));

        return cases;

        function createCase(id, title, builder) {
            var comp = app.project.items.addComp(id + "__" + title, cfg.width, cfg.height, 1, cfg.duration, cfg.fps);
            comp.parentFolder = folders.cases;
            comp.bgColor = cfg.bg;
            builder(comp);
            return { id: id, title: title, comp: comp };
        }
    }

    function buildMaskSamples(cfg, folders) {
        var comp = createPrecomp("SRC_mask_samples_rgba_split", cfg, folders);
        addPatch(comp, "bright_high_alpha__rgb190_a255", [190 / 255, 190 / 255, 190 / 255], [96, 160], 48, 100);
        addPatch(comp, "bright_low_alpha__rgb190_a89", [190 / 255, 190 / 255, 190 / 255], [200, 160], 48, 35);
        addPatch(comp, "dark_high_alpha__rgb41_a255", [41 / 255, 41 / 255, 41 / 255], [304, 160], 48, 100);
        addPatch(comp, "dark_low_alpha__rgb41_a89", [41 / 255, 41 / 255, 41 / 255], [408, 160], 48, 35);
        addPatch(comp, "luma_119_opaque", [119 / 255, 119 / 255, 119 / 255], [200, 320], 48, 100);
        addPatch(comp, "luma_120_opaque", [120 / 255, 120 / 255, 120 / 255], [304, 320], 48, 100);
        addPatch(comp, "mid_128_high_alpha", [128 / 255, 128 / 255, 128 / 255], [408, 320], 48, 100);
        return comp;
    }

    function buildImpulse(cfg, folders) {
        var comp = createPrecomp("SRC_impulse_1px", cfg, folders);
        addPatch(comp, "white_impulse_1px", [1, 1, 1], [256, 256], 1, 100);
        return comp;
    }

    function buildCompositeImpulse(cfg, folders) {
        var comp = createPrecomp("SRC_composite_impulse", cfg, folders);
        addPatch(comp, "white_composite_impulse_3px", [1, 1, 1], [256, 256], 3, 100);
        return comp;
    }

    function createPrecomp(name, cfg, folders) {
        var comp = app.project.items.addComp(name, cfg.width, cfg.height, 1, cfg.duration, cfg.fps);
        comp.parentFolder = folders.precomps;
        comp.bgColor = cfg.bg;
        return comp;
    }

    function addPatch(comp, name, color, position, size, opacity) {
        return addSolidLayer(comp, name, color, position, size, size, opacity);
    }

    function addSolidLayer(comp, name, color, position, width, height, opacity) {
        var layer = comp.layers.addSolid(color, name, width, height, 1, comp.duration);
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

    function addGlow(layer, threshold, radius, intensity, basedOnEnum) {
        var params = {
            "0002": threshold,
            "0003": radius,
            "0004": intensity
        };
        if (basedOnEnum !== null) {
            params["0001"] = basedOnEnum;
        }
        return addEffect(layer, "ADBE Glo2", params);
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

    function buildMasterReel(cfg, cases, folders) {
        var duration = cases.length * cfg.duration;
        var comp = app.project.items.addComp("master_glow_mask_discriminator_probe_reel", cfg.width, cfg.height, 1, duration, cfg.fps);
        comp.parentFolder = folders.root;
        comp.bgColor = cfg.bg;
        for (var i = 0; i < cases.length; i++) {
            var layer = comp.layers.add(cases[i].comp);
            layer.name = cases[i].id;
            layer.startTime = i * cfg.duration;
            layer.inPoint = i * cfg.duration;
            layer.outPoint = (i + 1) * cfg.duration;
            addMasterSlate(comp, cases[i].id, cases[i].title, i * cfg.duration, cfg.duration);
        }
        return comp;
    }

    function addMasterSlate(comp, id, title, startTime, duration) {
        var layer = comp.layers.addText(id + "  " + title);
        layer.name = "case_slate";
        layer.startTime = startTime;
        layer.inPoint = startTime;
        layer.outPoint = startTime + Math.min(0.45, duration);
        setLayerPosition(layer, [18, 32]);
        var docProp = layer.property("ADBE Text Properties").property("ADBE Text Document");
        var doc = docProp.value;
        doc.fontSize = 18;
        doc.fillColor = [1, 1, 1];
        doc.applyFill = true;
        docProp.setValue(doc);
        setLayerOpacity(layer, 80);
    }

    function enqueueCases(cases, master, pngDir, previewDir) {
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
        var masterRq = app.project.renderQueue.items.add(master);
        var masterOm = masterRq.outputModule(1);
        masterOm.file = new File(previewDir.fsName + "/master_glow_mask_discriminator_probe_reel.mov");
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
