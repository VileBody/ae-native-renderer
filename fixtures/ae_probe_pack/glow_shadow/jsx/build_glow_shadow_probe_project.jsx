/*
AE Native Renderer Glow + Drop Shadow Probe Pack Builder

Run from After Effects:
  File > Scripts > Run Script File... > build_glow_shadow_probe_project.jsx

The script creates procedural source comps, one label-free comp per probe case,
and render-queue entries for PNG sequence export. The outputs are probes, not
goldens, until rendered in AE and reviewed.
*/

(function buildGlowShadowProbeProject() {
    app.beginUndoGroup("Build Glow + Drop Shadow Probe Pack");

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
        glowRadius: 35,
        glowIntensity: 1.25,
        shadowOpacity: 180,
        shadowDirection: 135,
        shadowDistance: 28,
        shadowSoftness: 18
    };

    if (!app.project) {
        app.newProject();
    }
    app.project.bitsPerChannel = 8;

    var folders = {
        root: getOrCreateFolder("AE_PROBE_GLOW_SHADOW"),
        cases: getOrCreateFolder("AE_PROBE_GLOW_SHADOW/cases"),
        precomps: getOrCreateFolder("AE_PROBE_GLOW_SHADOW/precomps")
    };

    var sources = buildSources(CFG, folders);
    var cases = buildCases(CFG, sources, folders);
    var master = buildMasterReel(CFG, cases, folders);
    enqueueCases(cases, master, PNG_DIR, PREVIEW_DIR);

    alert(
        "Glow + Drop Shadow probe pack created.\n\n" +
        "Case comps: " + cases.length + "\n" +
        "Master comp: " + master.name + "\n\n" +
        "Render queued case comps as PNG sequences with RGB + Alpha."
    );

    app.endUndoGroup();

    function buildSources(cfg, folders) {
        return {
            lumaRamp: buildLumaRamp(cfg, folders),
            alphaSplit: buildAlphaSplit(cfg, folders),
            alphaSquare: buildAlphaSquare(cfg, folders),
            lumaMask: buildLumaMask(cfg, folders),
            alphaSplitLumaMask: buildAlphaSplitLumaMask(cfg, folders),
            alphaSplitAlphaMask: buildAlphaSplitAlphaMask(cfg, folders)
        };
    }

    function buildCases(cfg, sources, folders) {
        return [
            createCase("GLO_010", "Glow threshold source opaque luma ramp", function (comp) {
                placeComp(comp, sources.lumaRamp, [256, 256], [100, 100]);
            }),
            createCase("GLO_020", "Glow luma threshold mask candidate", function (comp) {
                placeComp(comp, sources.lumaMask, [256, 256], [100, 100]);
            }),
            createCase("GLO_030", "Glow blurred luma mask candidate", function (comp) {
                var layer = placeComp(comp, sources.lumaMask, [256, 256], [100, 100]);
                addBoxBlur(layer, cfg.glowRadius, 3);
            }),
            createCase("GLO_040", "Glow intensity scaled luma candidate", function (comp) {
                addBlurredMaskCandidate(comp, sources.lumaMask, cfg.glowRadius, cfg.glowIntensity);
            }),
            createCase("GLO_050", "Glow final AE output opaque luma ramp", function (comp) {
                var layer = placeComp(comp, sources.lumaRamp, [256, 256], [100, 100]);
                addGlow(layer, cfg.threshold, cfg.glowRadius, cfg.glowIntensity, null);
            }),
            createCase("GLO_060", "Glow threshold source alpha luma split", function (comp) {
                placeComp(comp, sources.alphaSplit, [256, 256], [100, 100]);
            }),
            createCase("GLO_070", "Glow alpha split luma rule mask candidate", function (comp) {
                placeComp(comp, sources.alphaSplitLumaMask, [256, 256], [100, 100]);
            }),
            createCase("GLO_080", "Glow alpha split alpha rule mask candidate", function (comp) {
                placeComp(comp, sources.alphaSplitAlphaMask, [256, 256], [100, 100]);
            }),
            createCase("GLO_090", "Glow final AE output alpha luma split", function (comp) {
                var layer = placeComp(comp, sources.alphaSplit, [256, 256], [100, 100]);
                addGlow(layer, cfg.threshold, cfg.glowRadius, cfg.glowIntensity, null);
            }),
            createCase("GLO_100", "Glow final AE output based on enum 1", function (comp) {
                var layer = placeComp(comp, sources.alphaSplit, [256, 256], [100, 100]);
                addGlow(layer, cfg.threshold, cfg.glowRadius, cfg.glowIntensity, 1);
            }),
            createCase("GLO_101", "Glow final AE output based on enum 2", function (comp) {
                var layer = placeComp(comp, sources.alphaSplit, [256, 256], [100, 100]);
                addGlow(layer, cfg.threshold, cfg.glowRadius, cfg.glowIntensity, 2);
            }),
            createCase("DSH_010", "Drop Shadow source alpha square", function (comp) {
                placeComp(comp, sources.alphaSquare, [256, 256], [100, 100]);
            }),
            createCase("DSH_020", "Drop Shadow no softness shadow only raw output", function (comp) {
                var layer = placeComp(comp, sources.alphaSquare, [256, 256], [100, 100]);
                addDropShadow(layer, cfg.shadowOpacity, cfg.shadowDirection, cfg.shadowDistance, 0, 1);
            }),
            createCase("DSH_030", "Drop Shadow no softness final composite", function (comp) {
                var layer = placeComp(comp, sources.alphaSquare, [256, 256], [100, 100]);
                addDropShadow(layer, cfg.shadowOpacity, cfg.shadowDirection, cfg.shadowDistance, 0, 0);
            }),
            createCase("DSH_040", "Drop Shadow softened shadow only output", function (comp) {
                var layer = placeComp(comp, sources.alphaSquare, [256, 256], [100, 100]);
                addDropShadow(layer, cfg.shadowOpacity, cfg.shadowDirection, cfg.shadowDistance, cfg.shadowSoftness, 1);
            }),
            createCase("DSH_050", "Drop Shadow softened final composite", function (comp) {
                var layer = placeComp(comp, sources.alphaSquare, [256, 256], [100, 100]);
                addDropShadow(layer, cfg.shadowOpacity, cfg.shadowDirection, cfg.shadowDistance, cfg.shadowSoftness, 0);
            }),
            createCase("DSH_060", "Drop Shadow manual raw offset dx neg20 dy pos20", function (comp) {
                addManualShadow(comp, [236, 276], cfg.shadowOpacity);
            }),
            createCase("DSH_061", "Drop Shadow manual raw offset dx pos20 dy pos20", function (comp) {
                addManualShadow(comp, [276, 276], cfg.shadowOpacity);
            })
        ];

        function createCase(id, title, builder) {
            var comp = app.project.items.addComp(id + "__" + title, cfg.width, cfg.height, 1, cfg.duration, cfg.fps);
            comp.parentFolder = folders.cases;
            comp.bgColor = cfg.bg;
            builder(comp);
            return { id: id, title: title, comp: comp };
        }
    }

    function buildLumaRamp(cfg, folders) {
        var comp = createPrecomp("SRC_luma_ramp_16_steps", cfg, folders);
        var barW = 20;
        var barH = 220;
        var left = 256 - (barW * 16) / 2;
        for (var i = 0; i < 16; i++) {
            var g = i / 15;
            var layer = comp.layers.addSolid([g, g, g], "luma_" + Math.round(g * 255), barW, barH, 1, cfg.duration);
            setLayerPosition(layer, [left + barW * i + barW / 2, 256]);
        }
        return comp;
    }

    function buildLumaMask(cfg, folders) {
        var comp = createPrecomp("MASK_luma_ge_120", cfg, folders);
        var barW = 20;
        var barH = 220;
        var left = 256 - (barW * 16) / 2;
        for (var i = 0; i < 16; i++) {
            var lum = Math.round((i / 15) * 255);
            if (lum >= cfg.threshold) {
                var layer = comp.layers.addSolid([1, 1, 1], "mask_luma_" + lum, barW, barH, 1, cfg.duration);
                setLayerPosition(layer, [left + barW * i + barW / 2, 256]);
            }
        }
        return comp;
    }

    function buildAlphaSplit(cfg, folders) {
        var comp = createPrecomp("SRC_alpha_split_probe", cfg, folders);
        addPatch(comp, "bright_low_alpha", [1, 1, 1], [166, 196], 88, 35);
        addPatch(comp, "dark_high_alpha", [0.16, 0.16, 0.16], [346, 196], 88, 100);
        addPatch(comp, "bright_high_alpha", [1, 1, 1], [166, 336], 88, 100);
        addPatch(comp, "mid_high_alpha", [0.50, 0.50, 0.50], [346, 336], 88, 100);
        return comp;
    }

    function buildAlphaSplitLumaMask(cfg, folders) {
        var comp = createPrecomp("MASK_alpha_split_luma_rule", cfg, folders);
        addPatch(comp, "bright_low_alpha_luma_hit", [1, 1, 1], [166, 196], 88, 100);
        addPatch(comp, "bright_high_alpha_luma_hit", [1, 1, 1], [166, 336], 88, 100);
        addPatch(comp, "mid_high_alpha_luma_hit", [1, 1, 1], [346, 336], 88, 100);
        return comp;
    }

    function buildAlphaSplitAlphaMask(cfg, folders) {
        var comp = createPrecomp("MASK_alpha_split_alpha_rule", cfg, folders);
        addPatch(comp, "dark_high_alpha_hit", [1, 1, 1], [346, 196], 88, 100);
        addPatch(comp, "bright_high_alpha_hit", [1, 1, 1], [166, 336], 88, 100);
        addPatch(comp, "mid_high_alpha_hit", [1, 1, 1], [346, 336], 88, 100);
        return comp;
    }

    function buildAlphaSquare(cfg, folders) {
        var comp = createPrecomp("SRC_alpha_square", cfg, folders);
        addPatch(comp, "source_alpha_square", [1, 1, 1], [256, 256], 112, 100);
        return comp;
    }

    function createPrecomp(name, cfg, folders) {
        var comp = app.project.items.addComp(name, cfg.width, cfg.height, 1, cfg.duration, cfg.fps);
        comp.parentFolder = folders.precomps;
        comp.bgColor = cfg.bg;
        return comp;
    }

    function addPatch(comp, name, color, position, size, opacity) {
        var layer = comp.layers.addSolid(color, name, size, size, 1, comp.duration);
        setLayerPosition(layer, position);
        setLayerOpacity(layer, opacity);
        return layer;
    }

    function addManualShadow(comp, position, opacity255) {
        var alphaPercent = (opacity255 / 255) * 100;
        addPatch(comp, "manual_shadow_candidate", [0, 0, 0], position, 112, alphaPercent);
    }

    function addBlurredMaskCandidate(comp, source, radius, intensity) {
        var base = placeComp(comp, source, [256, 256], [100, 100]);
        addBoxBlur(base, radius, 3);
        if (intensity > 1) {
            var extra = placeComp(comp, source, [256, 256], [100, 100]);
            addBoxBlur(extra, radius, 3);
            setLayerOpacity(extra, (intensity - 1) * 100);
            try {
                extra.blendingMode = BlendingMode.ADD;
            } catch (_err) {
            }
        }
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

    function addDropShadow(layer, opacity, direction, distance, softness, shadowOnly) {
        return addEffect(layer, "ADBE Drop Shadow", {
            "0001": [0, 0, 0, 1],
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

    function buildMasterReel(cfg, cases, folders) {
        var duration = cases.length * cfg.duration;
        var comp = app.project.items.addComp("master_glow_shadow_probe_reel", cfg.width, cfg.height, 1, duration, cfg.fps);
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
        masterOm.file = new File(previewDir.fsName + "/master_glow_shadow_probe_reel.mov");
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
