/*
AE Native Renderer Conformance Pack Builder

Run from After Effects:
  File > Scripts > Run Script File... > build_conformance_project.jsx

The script creates one comp per case, a master reel, and render-queue items.
PNG sequences from the case comps are the golden source of truth.
*/

(function buildConformanceProject() {
    app.beginUndoGroup("Build AE Native Renderer Conformance Pack");

    var SCRIPT_FILE = new File($.fileName);
    var PACK_DIR = SCRIPT_FILE.parent.parent;
    var ASSET_DIR = new Folder(PACK_DIR.fsName + "/assets/primitives");
    var GOLDEN_DIR = new Folder(PACK_DIR.fsName + "/ae_goldens/png");
    var PREVIEW_DIR = new Folder(PACK_DIR.fsName + "/ae_goldens/preview");
    var METADATA_DIR = new Folder(PACK_DIR.fsName + "/ae_goldens/metadata");
    ensureFolder(GOLDEN_DIR);
    ensureFolder(PREVIEW_DIR);
    ensureFolder(METADATA_DIR);

    var CFG = {
        width: 512,
        height: 512,
        fps: 30,
        duration: 2.0,
        bg: [0.02, 0.02, 0.025],
        fontMontserrat: "Montserrat-BoldItalic",
        fontPoint: "Point-Light"
    };

    if (!app.project) {
        app.newProject();
    }
    app.project.bitsPerChannel = 8;

    var folders = {
        root: getOrCreateFolder("AE_NATIVE_CONFORMANCE_PACK"),
        footage: getOrCreateFolder("AE_NATIVE_CONFORMANCE_PACK/footage"),
        cases: getOrCreateFolder("AE_NATIVE_CONFORMANCE_PACK/cases"),
        precomps: getOrCreateFolder("AE_NATIVE_CONFORMANCE_PACK/precomps")
    };

    var assets = importAssets(folders.footage);
    var cases = buildCases(CFG, assets, folders);
    writeEffectPropertyDump(CFG, assets, folders, METADATA_DIR);
    var master = buildMasterReel(CFG, cases, folders);
    enqueueCases(cases, master, GOLDEN_DIR, PREVIEW_DIR);

    alert(
        "AE conformance pack created.\n\n" +
        "Case comps: " + cases.length + "\n" +
        "Master comp: " + master.name + "\n\n" +
        "Metadata dump:\n" + METADATA_DIR.fsName + "/effect_property_dump.json\n\n" +
        "Required fonts:\n" +
        "- " + CFG.fontMontserrat + "\n" +
        "- " + CFG.fontPoint + "\n\n" +
        "Render PNG sequences from the queued case comps."
    );

    app.endUndoGroup();

    function buildCases(cfg, assets, folders) {
        return [
            createCase("PRI_010", "primitive impulse/alpha/color source sanity", function (comp) {
                placeAsset(comp, assets.impulse, 128, 128, 100);
                placeAsset(comp, assets.alphaSquare, 384, 128, 70);
                placeAsset(comp, assets.coordinate, 128, 384, 70);
                placeAsset(comp, assets.colorBars, 384, 384, 70);
            }),
            createCase("INT_010", "linear/hold position and opacity interpolation", function (comp) {
                var linear = placeAsset(comp, assets.alphaSquare, 96, 170, 45);
                animatePosition(linear, 0, [96, 170], cfg.duration, [416, 170], KeyframeInterpolationType.LINEAR);
                var hold = placeAsset(comp, assets.alphaSquare, 96, 340, 45);
                animatePosition(hold, 0, [96, 340], cfg.duration, [416, 340], KeyframeInterpolationType.HOLD);
                animateOpacity(hold, 0, 100, cfg.duration * 0.5, 35, cfg.duration, 100);
            }),
            createCase("INT_020", "bezier/ease position and opacity interpolation", function (comp) {
                var layer = placeAsset(comp, assets.alphaSquare, 96, 256, 45);
                animatePosition(layer, 0, [96, 256], cfg.duration, [416, 256], KeyframeInterpolationType.BEZIER);
                easeAllKeys(layer.property("ADBE Transform Group").property("ADBE Position"));
                animateOpacity(layer, 0, 0, cfg.duration * 0.5, 100, cfg.duration, 0);
                easeAllKeys(layer.property("ADBE Transform Group").property("ADBE Opacity"));
            }),
            createCase("TMP_010", "source/layer start time and frame mapping", function (comp) {
                var seq = placeAsset(comp, assets.numberedFrames, 256, 256, 100);
                seq.startTime = -0.5;
                seq.inPoint = 0.25;
                seq.outPoint = cfg.duration;
            }),
            createCase("TMP_020", "Posterize Time numbered-frame boundaries", function (comp) {
                var seq = placeAsset(comp, assets.numberedFrames, 256, 256, 100);
                addEffect(seq, "ADBE Posterize Time", { "0001": 6 });
            }),
            createCase("TMP_030", "motion blur velocity and shutter window", function (comp) {
                comp.motionBlur = true;
                comp.shutterAngle = 180;
                comp.shutterPhase = -90;
                var layer = placeAsset(comp, assets.alphaSquare, 80, 256, 38);
                layer.motionBlur = true;
                animatePosition(layer, 0, [80, 256], cfg.duration, [432, 256], KeyframeInterpolationType.LINEAR);
            }),
            createCase("EFF_010", "Drop Shadow on alpha square", function (comp) {
                var layer = placeAsset(comp, assets.alphaSquare, 256, 256, 65);
                addEffect(layer, "ADBE Drop Shadow", {
                    "0001": [0, 0, 0, 1],
                    "0002": 180,
                    "0003": 135,
                    "0004": 28,
                    "0005": 18,
                    "0006": 0
                });
            }),
            createCase("EFF_020", "Glow on luma ramp", function (comp) {
                var layer = placeAsset(comp, assets.lumaRamp, 256, 256, 100);
                addEffect(layer, "ADBE Glo2", {
                    "0002": 120,
                    "0003": 35,
                    "0004": 1.25
                });
            }),
            createCase("EFF_030", "Box Blur on impulse", function (comp) {
                var layer = placeAsset(comp, assets.impulse, 256, 256, 100);
                addEffect(layer, "ADBE Box Blur2", {
                    "0001": 18,
                    "0002": 3
                });
            }),
            createCase("EFF_040", "Geometry2 coordinate-field transform", function (comp) {
                var layer = placeAsset(comp, assets.coordinate, 256, 256, 100);
                addEffect(layer, "ADBE Geometry2", {
                    "0001": [128, 128],
                    "0002": [256, 256],
                    "0003": 82,
                    "0004": 120,
                    "0008": 72,
                    "rotation": 17
                });
            }),
            createCase("EFF_041", "Geometry2 bicubic coordinate-field transform", function (comp) {
                var layer = placeAsset(comp, assets.coordinate, 256, 256, 100);
                addEffect(layer, "ADBE Geometry2", {
                    "0001": [128, 128],
                    "0002": [256, 256],
                    "0003": 82,
                    "0004": 120,
                    "0008": 72,
                    "0012": 2,
                    "rotation": 17
                });
            }),
            createCase("EFF_050", "Minimax alpha-square morphology", function (comp) {
                var layer = placeAsset(comp, assets.alphaSquare, 256, 256, 70);
                addEffect(layer, "ADBE Minimax", {
                    "0001": 2,
                    "0002": 12,
                    "0003": 1
                });
            }),
            createCase("EFF_060", "Turbulent Displace coordinate-field warp", function (comp) {
                placeAsset(comp, assets.checker, 256, 256, 100);
                var field = placeAsset(comp, assets.coordinate, 256, 256, 100);
                setLayerOpacity(field, 80);
                var fx = addEffect(field, "ADBE Turbulent Displace", {
                    "0002": 45,
                    "0003": 65,
                    "0005": 2
                });
                setAnimatedEffectScalar(fx, "0006", 0, 0, cfg.duration, 180);
            }),
            createCase("EFF_070", "animated effect params over fixed primitive", function (comp) {
                var impulse = placeAsset(comp, assets.impulse, 170, 256, 100);
                var blur = addEffect(impulse, "ADBE Box Blur2", { "0001": 1, "0002": 2 });
                setAnimatedEffectScalar(blur, "0001", 0, 1, cfg.duration, 28);

                var glowLayer = placeAsset(comp, assets.lumaRamp, 342, 256, 55);
                var glow = addEffect(glowLayer, "ADBE Glo2", { "0002": 160, "0003": 10, "0004": 0.5 });
                setAnimatedEffectScalar(glow, "0003", 0, 10, cfg.duration, 55);
            }),
            createCase("TXT_010", "Montserrat word reveal", function (comp) {
                var text = addText(comp, "WORD REVEAL\nMONTSERRAT TEST", cfg.fontMontserrat, 58, [1, 1, 1], [256, 256]);
                addRangeAnimator(text, "Words", 3, 0, 100);
            }),
            createCase("TXT_020", "Montserrat character and line reveal", function (comp) {
                var chars = addText(comp, "CHARACTER REVEAL", cfg.fontMontserrat, 48, [1, 1, 1], [256, 190]);
                addRangeAnimator(chars, "Characters", 1, 0, 100);
                var lines = addText(comp, "LINE ONE\nLINE TWO\nLINE THREE", cfg.fontMontserrat, 42, [1, 1, 1], [256, 330]);
                addRangeAnimator(lines, "Lines", 4, 0, 100);
            }),
            createCase("TXT_030", "Point-Light glyph animator position/scale/rotation/blur", function (comp) {
                var text = addText(comp, "GLYPH MOTION", cfg.fontPoint, 74, [1, 1, 1], [256, 256]);
                addGlyphAnimator(text);
            }),
            createCase("TXT_040", "expression-selector bounce pattern", function (comp) {
                var text = addText(comp, "BOUNCE SELECTOR", cfg.fontPoint, 64, [1, 1, 1], [256, 256]);
                addBounceExpressionAnimator(text);
            }),
            createCase("EXP_010", "edge_wobble expression subset", function (comp) {
                var layer = placeAsset(comp, assets.alphaSquare, 256, 256, 55);
                var pos = layer.property("ADBE Transform Group").property("ADBE Position");
                pos.expression =
                    "intro = 0.25; outro = 0.25; amp = 34; freq = 2.0;\n" +
                    "edge = Math.min(time - inPoint, outPoint - time);\n" +
                    "env = Math.max(0, Math.min(1, edge / intro));\n" +
                    "value + [Math.sin(time * freq * 2 * Math.PI) * amp * env, 0];";
            }),
            createCase("STK_010", "Drop Shadow x2 stack", function (comp) {
                var layer = placeAsset(comp, assets.alphaSquare, 256, 256, 60);
                addEffect(layer, "ADBE Drop Shadow", { "0002": 210, "0003": 135, "0004": 8, "0005": 8 });
                addEffect(layer, "ADBE Drop Shadow", { "0002": 160, "0003": 45, "0004": 22, "0005": 18 });
            }),
            createCase("STK_020", "Blur then Minimax non-commutative stack", function (comp) {
                var left = placeAsset(comp, assets.hardEdge, 150, 256, 70);
                addEffect(left, "ADBE Box Blur2", { "0001": 10, "0002": 2 });
                addEffect(left, "ADBE Minimax", { "0001": 2, "0002": 6, "0003": 1 });
                var right = placeAsset(comp, assets.hardEdge, 362, 256, 70);
                addEffect(right, "ADBE Minimax", { "0001": 2, "0002": 6, "0003": 1 });
                addEffect(right, "ADBE Box Blur2", { "0001": 10, "0002": 2 });
            }),
            createCase("STK_030", "Geometry2 -> Posterize -> Minimax -> Turbulent adjustment stack", function (comp) {
                var seq = placeAsset(comp, assets.numberedFrames, 256, 256, 100);
                setLayerOpacity(seq, 70);
                placeAsset(comp, assets.coordinate, 256, 256, 100);
                var adj = comp.layers.addSolid([1, 1, 1], "ADJ_template_like_stack", cfg.width, cfg.height, 1, cfg.duration);
                adj.adjustmentLayer = true;
                addEffect(adj, "ADBE Geometry2", { "0003": 96, "0004": 110, "0008": 92 });
                addEffect(adj, "ADBE Posterize Time", { "0001": 6 });
                addEffect(adj, "ADBE Minimax", { "0001": 2, "0002": 4, "0003": 1 });
                var turb = addEffect(adj, "ADBE Turbulent Displace", { "0002": 24, "0003": 72, "0005": 2 });
                setAnimatedEffectScalar(turb, "0006", 0, 0, cfg.duration, 180);
            }),
            createCase("STK_031", "Geometry2 bicubic adjustment-layer transform", function (comp) {
                placeAsset(comp, assets.coordinate, 256, 256, 100);
                var adj = comp.layers.addSolid([1, 1, 1], "ADJ_geometry2_bicubic", cfg.width, cfg.height, 1, cfg.duration);
                adj.adjustmentLayer = true;
                addEffect(adj, "ADBE Geometry2", {
                    "0001": [128, 128],
                    "0002": [256, 256],
                    "0003": 82,
                    "0004": 120,
                    "0008": 72,
                    "0012": 2,
                    "rotation": 17
                });
            }),
            createCase("GPH_010", "nested precomp and collapse-transform text sharpness", function (comp) {
                var child = app.project.items.addComp("GPH_010_child_text", cfg.width, cfg.height, 1, cfg.duration, cfg.fps);
                child.parentFolder = folders.precomps;
                addText(child, "COLLAPSE", cfg.fontMontserrat, 48, [1, 1, 1], [256, 256]);
                var normal = comp.layers.add(child);
                normal.name = "rasterized_precomp";
                setLayerPosition(normal, [150, 256]);
                setLayerScale(normal, [180, 180]);
                var collapsed = comp.layers.add(child);
                collapsed.name = "collapsed_precomp";
                setLayerPosition(collapsed, [362, 256]);
                setLayerScale(collapsed, [180, 180]);
                collapsed.collapseTransformation = true;
            }),
            createCase("CMP_010", "alpha/composite/sampling audit", function (comp) {
                placeAsset(comp, assets.checker, 256, 256, 100);
                var probe = placeAsset(comp, assets.premultProbe, 256, 256, 80);
                probe.blendingMode = BlendingMode.NORMAL;
                var field = placeAsset(comp, assets.coordinate, 256, 256, 55);
                setLayerOpacity(field, 45);
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

    function importAssets(folder) {
        return {
            transparent: importStill("transparent", "transparent.png", folder),
            impulse: importStill("impulse_center", "impulse_center.png", folder),
            impulseGrid: importStill("impulse_grid", "impulse_grid.png", folder),
            alphaSquare: importStill("alpha_square", "alpha_square.png", folder),
            alphaRamp: importStill("alpha_ramp", "alpha_ramp.png", folder),
            lumaRamp: importStill("luma_ramp", "luma_ramp.png", folder),
            coordinate: importStill("coordinate_field", "coordinate_field.png", folder),
            checker: importStill("checkerboard_16", "checkerboard_16.png", folder),
            hardEdge: importStill("hard_edge", "hard_edge.png", folder),
            colorBars: importStill("color_bars", "color_bars.png", folder),
            premultProbe: importStill("premult_probe", "premult_probe.png", folder),
            numberedFrames: importSequence("numbered_frames", "numbered_frames/frame_0000.png", folder)
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

    function importSequence(name, relativeFirstFrame, folder) {
        var file = new File(ASSET_DIR.fsName + "/" + relativeFirstFrame);
        if (!file.exists) {
            throw new Error("Missing primitive sequence: " + file.fsName);
        }
        var options = new ImportOptions(file);
        options.importAs = ImportAsType.FOOTAGE;
        options.sequence = true;
        options.forceAlphabetical = true;
        var item = app.project.importFile(options);
        item.name = name;
        item.parentFolder = folder;
        if (item.mainSource && item.mainSource.conformFrameRate) {
            item.mainSource.conformFrameRate = CFG.fps;
        }
        return item;
    }

    function placeAsset(comp, item, x, y, scalePercent) {
        var layer = comp.layers.add(item);
        setLayerPosition(layer, [x, y]);
        setLayerScale(layer, [scalePercent, scalePercent]);
        layer.inPoint = 0;
        layer.outPoint = comp.duration;
        return layer;
    }

    function addMasterSlate(comp, id, title, startTime, duration) {
        var layer = comp.layers.addText(id + "  " + title);
        layer.name = "case_slate";
        layer.startTime = startTime;
        layer.inPoint = startTime;
        layer.outPoint = startTime + Math.min(0.45, duration);
        setLayerPosition(layer, [18, 34]);
        var doc = layer.property("ADBE Text Properties").property("ADBE Text Document").value;
        doc.font = CFG.fontMontserrat;
        doc.fontSize = 18;
        doc.fillColor = [1, 1, 1];
        doc.applyFill = true;
        layer.property("ADBE Text Properties").property("ADBE Text Document").setValue(doc);
        layer.property("ADBE Transform Group").property("ADBE Opacity").setValue(70);
    }

    function addText(comp, value, fontName, fontSize, fill, position) {
        var layer = comp.layers.addText(value);
        setLayerPosition(layer, position);
        layer.inPoint = 0;
        layer.outPoint = comp.duration;
        var docProp = layer.property("ADBE Text Properties").property("ADBE Text Document");
        var doc = docProp.value;
        doc.font = fontName;
        doc.fontSize = fontSize;
        doc.fillColor = fill;
        doc.applyFill = true;
        doc.justification = ParagraphJustification.CENTER_JUSTIFY;
        docProp.setValue(doc);
        return layer;
    }

    function addRangeAnimator(layer, modeName, basedOnCode, startValue, endValue) {
        var textProps = layer.property("ADBE Text Properties");
        var animators = textProps.property("ADBE Text Animators");
        var animator = animators.addProperty("ADBE Text Animator");
        animator.name = modeName + "_reveal";
        var props = animator.property("ADBE Text Animator Properties");
        props.addProperty("ADBE Text Opacity").setValue(0);
        var selectors = animator.property("ADBE Text Selectors");
        var selector = selectors.addProperty("ADBE Text Selector");
        selector.property("ADBE Text Percent Start").setValueAtTime(0, startValue);
        selector.property("ADBE Text Percent Start").setValueAtTime(layer.containingComp.duration, endValue);
        selector.property("ADBE Text Range Advanced").property("ADBE Text Range Type2").setValue(basedOnCode);
        return animator;
    }

    function addGlyphAnimator(layer) {
        var textProps = layer.property("ADBE Text Properties");
        var animators = textProps.property("ADBE Text Animators");
        var animator = animators.addProperty("ADBE Text Animator");
        animator.name = "glyph_position_scale_rotation_blur";
        var props = animator.property("ADBE Text Animator Properties");
        props.addProperty("ADBE Text Position 3D").setValue([0, -72, 0]);
        props.addProperty("ADBE Text Scale 3D").setValue([125, 125, 100]);
        props.addProperty("ADBE Text Rotation").setValue(18);
        try {
            props.addProperty("ADBE Text Blur").setValue([10, 10]);
        } catch (_err) {
        }
        var selector = animator.property("ADBE Text Selectors").addProperty("ADBE Text Selector");
        selector.property("ADBE Text Percent Start").setValueAtTime(0, 0);
        selector.property("ADBE Text Percent Start").setValueAtTime(layer.containingComp.duration, 100);
        selector.property("ADBE Text Range Advanced").property("ADBE Text Range Type2").setValue(1);
    }

    function addBounceExpressionAnimator(layer) {
        var textProps = layer.property("ADBE Text Properties");
        var animators = textProps.property("ADBE Text Animators");
        var animator = animators.addProperty("ADBE Text Animator");
        animator.name = "expression_selector_bounce";
        var props = animator.property("ADBE Text Animator Properties");
        props.addProperty("ADBE Text Scale 3D").setValue([0, 0, 100]);
        var selector = animator.property("ADBE Text Selectors").addProperty("ADBE Text Expressible Selector");
        selector.property("ADBE Text Expressible Amount").expression =
            "delay = 0.0500;\n" +
            "myDelay = delay*textIndex;\n" +
            "t = (time - inPoint) - myDelay;\n" +
            "if (t >= 0){\n" +
            "  freq = 2; amplitude = 100; decay = 8.0;\n" +
            "  s = amplitude*Math.cos(freq*t*2*Math.PI)/Math.exp(decay*t);\n" +
            "  s\n" +
            "} else { value }";
    }

    function animatePosition(layer, t0, v0, t1, v1, interpolation) {
        var prop = layer.property("ADBE Transform Group").property("ADBE Position");
        prop.setValueAtTime(t0, v0);
        prop.setValueAtTime(t1, v1);
        prop.setInterpolationTypeAtKey(1, interpolation, interpolation);
        prop.setInterpolationTypeAtKey(2, interpolation, interpolation);
    }

    function animateOpacity(layer, t0, v0, t1, v1, t2, v2) {
        var prop = layer.property("ADBE Transform Group").property("ADBE Opacity");
        prop.setValueAtTime(t0, v0);
        prop.setValueAtTime(t1, v1);
        prop.setValueAtTime(t2, v2);
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

    function easeAllKeys(prop) {
        for (var i = 1; i <= prop.numKeys; i++) {
            prop.setInterpolationTypeAtKey(i, KeyframeInterpolationType.BEZIER, KeyframeInterpolationType.BEZIER);
            var ease = new KeyframeEase(0, 33.333);
            prop.setTemporalEaseAtKey(i, [ease], [ease]);
        }
    }

    function addEffect(layer, matchName, params) {
        var fx = layer.property("ADBE Effect Parade").addProperty(matchName);
        if (!fx) {
            throw new Error("AE could not add effect: " + matchName);
        }
        applyEffectParams(fx, params || {});
        return fx;
    }

    function writeEffectPropertyDump(cfg, assets, folders, metadataDir) {
        var dump = {
            schema: "ae-native-renderer.effect-property-dump.v1",
            generated_by: "fixtures/ae_conformance_pack/jsx/build_conformance_project.jsx",
            bits_per_channel: app.project.bitsPerChannel,
            composition: {
                width: cfg.width,
                height: cfg.height,
                fps: cfg.fps,
                duration: cfg.duration
            },
            snapshots: []
        };
        var comp = app.project.items.addComp("PROPERTY_DUMP_effect_controls", cfg.width, cfg.height, 1, cfg.duration, cfg.fps);
        comp.parentFolder = folders.precomps;

        var defs = [
            {
                id: "transform_group",
                match_name: "ADBE Transform Group",
                params: {}
            },
            {
                id: "geometry2_default",
                match_name: "ADBE Geometry2",
                params: {}
            },
            {
                id: "geometry2_numbered_probe",
                match_name: "ADBE Geometry2",
                params: {
                    "0001": [128, 128],
                    "0002": [256, 256],
                    "0003": 82,
                    "0004": 120,
                    "0005": 90,
                    "0006": 14,
                    "0007": 35,
                    "0008": 72,
                    "0009": 88,
                    "0010": 1,
                    "0011": 120,
                    "0012": 1
                }
            },
            {
                id: "box_blur2",
                match_name: "ADBE Box Blur2",
                params: { "0001": 18, "0002": 3 }
            },
            {
                id: "drop_shadow",
                match_name: "ADBE Drop Shadow",
                params: { "0002": 180, "0003": 135, "0004": 28, "0005": 18, "0006": 0 }
            },
            {
                id: "glow2",
                match_name: "ADBE Glo2",
                params: { "0002": 120, "0003": 35, "0004": 1.25 }
            },
            {
                id: "minimax",
                match_name: "ADBE Minimax",
                params: { "0001": 2, "0002": 12, "0003": 1 }
            },
            {
                id: "posterize_time",
                match_name: "ADBE Posterize Time",
                params: { "0001": 6 }
            },
            {
                id: "turbulent_displace",
                match_name: "ADBE Turbulent Displace",
                params: { "0002": 45, "0003": 65, "0005": 2, "0006": 90 }
            }
        ];

        for (var i = 0; i < defs.length; i++) {
            var def = defs[i];
            var layer = placeAsset(comp, assets.coordinate, 256, 256, 100);
            layer.name = "metadata_" + def.id;
            var transformGroup = layer.property("ADBE Transform Group");
            if (def.match_name === "ADBE Transform Group") {
                dump.snapshots.push({
                    id: def.id,
                    match_name: def.match_name,
                    requested_params: cloneParams(def.params),
                    layer_transform_properties: collectProperties(transformGroup)
                });
                continue;
            }
            var fx = layer.property("ADBE Effect Parade").addProperty(def.match_name);
            if (!fx) {
                dump.snapshots.push({
                    id: def.id,
                    match_name: def.match_name,
                    error: "AE could not add effect"
                });
                continue;
            }
            applyEffectParams(fx, def.params);
            dump.snapshots.push({
                id: def.id,
                match_name: def.match_name,
                effect_name: safeString(fx.name),
                effect_match_name: safeString(fx.matchName),
                requested_params: cloneParams(def.params),
                properties: collectProperties(fx)
            });
        }

        var outFile = new File(metadataDir.fsName + "/effect_property_dump.json");
        outFile.encoding = "UTF-8";
        outFile.open("w");
        outFile.write(toJson(dump, 0));
        outFile.close();
    }

    function applyEffectParams(fx, params) {
        for (var key in params) {
            if (!params.hasOwnProperty(key)) {
                continue;
            }
            var prop = null;
            if (/^\d+$/.test(key)) {
                prop = fx.property(parseInt(key, 10));
            } else if (/^0+\d+$/.test(key)) {
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

    function setAnimatedEffectScalar(fx, key, t0, v0, t1, v1) {
        var prop = fx.property(parseInt(key, 10));
        if (!prop) {
            prop = fx.property(key);
        }
        if (!prop) {
            return;
        }
        prop.setValueAtTime(t0, v0);
        prop.setValueAtTime(t1, v1);
    }

    function collectProperties(group) {
        var props = [];
        if (!group) {
            return props;
        }
        for (var i = 1; i <= group.numProperties; i++) {
            var prop = group.property(i);
            props.push({
                index: i,
                name: safeString(prop.name),
                match_name: safeString(prop.matchName),
                property_value_type: propertyValueTypeName(prop),
                value: propertyValue(prop),
                can_set_expression: safeBoolean(prop.canSetExpression),
                is_time_varying: safeBoolean(prop.isTimeVarying),
                num_properties: safeNumber(prop.numProperties)
            });
        }
        return props;
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

    function safeNumber(value) {
        try {
            if (value === null || value === undefined) {
                return 0;
            }
            return Number(value);
        } catch (_err) {
            return 0;
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

    function buildMasterReel(cfg, cases, folders) {
        var duration = cases.length * cfg.duration;
        var comp = app.project.items.addComp("master_conformance_reel", cfg.width, cfg.height, 1, duration, cfg.fps);
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

    function enqueueCases(cases, master, goldenDir, previewDir) {
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
        var masterRq = app.project.renderQueue.items.add(master);
        var masterOm = masterRq.outputModule(1);
        masterOm.file = new File(previewDir.fsName + "/master_conformance_reel.mov");
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
