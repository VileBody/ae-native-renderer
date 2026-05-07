/*
CoolType Text Layout Probe Pack

This builder creates the text layers used by the conformance cases and calls
sourceRectAtTime on them at selected times. It intentionally queues no renders:
the point is to trigger AE/CoolType layout code under Frida without spending
minutes in aerender.
*/

(function buildCoolTypeTextLayoutProbeProject() {
    app.beginUndoGroup("Build CoolType Text Layout Probe Pack");
    try {
        var SCRIPT_FILE = new File($.fileName);
        var PACK_DIR = SCRIPT_FILE.parent.parent;
        var METADATA_DIR = new Folder(PACK_DIR.fsName + "/ae_goldens/metadata");
        ensureFolder(new Folder(PACK_DIR.fsName + "/ae_goldens"));
        ensureFolder(METADATA_DIR);

        var CFG = {
            width: 512,
            height: 512,
            fps: 30,
            duration: 2.0,
            bg: [0.02, 0.02, 0.025],
            fontMontserrat: "Montserrat-BoldItalic",
            fontPoint: "Point-Light",
            times: [0, 8 / 30, 16 / 30, 24 / 30, 32 / 30, 45 / 30, 59 / 30]
        };

        if (!app.project) {
            app.newProject();
        }
        app.project.bitsPerChannel = 8;
        try {
            if (typeof PurgeTarget !== "undefined") {
                app.purge(PurgeTarget.ALL_CACHES);
            }
        } catch (_purgeErr) {
        }

        var folders = {
            cases: getOrCreateFolder("COOLTYPE_TEXT_LAYOUT_PROBE_cases"),
            precomps: getOrCreateFolder("COOLTYPE_TEXT_LAYOUT_PROBE_precomps")
        };

        var records = [];
        buildTxt010(CFG, folders, records);
        buildTxt020(CFG, folders, records);
        buildTxt040(CFG, folders, records);
        buildGph010(CFG, folders, records);

        writeTextFile(
            new File(METADATA_DIR.fsName + "/cooltype_text_layout_probe.json"),
            toJson({
                schema: "ae-native-renderer.cooltype-text-layout-probe.v1",
                generated_by: "fixtures/ae_probe_pack/cooltype_text_layout/jsx/build_cooltype_text_layout_probe_project.jsx",
                render_queue_items: app.project.renderQueue.numItems,
                records: records
            }, 0)
        );
    } catch (err) {
        alert("CoolType text layout probe failed:\n" + String(err && err.stack ? err.stack : err));
        throw err;
    } finally {
        app.endUndoGroup();
    }

    function buildTxt010(cfg, folders, records) {
        var comp = makeComp("TXT_010__Montserrat word reveal sourceRect probe", cfg, folders.cases);
        var text = addText(comp, "WORD REVEAL\nMONTSERRAT TEST", cfg.fontMontserrat, 58, [1, 1, 1], [256, 256]);
        text.name = "TXT_010_word_reveal";
        addRangeAnimator(text, "Words", 3, 0, 100);
        sampleLayer(records, "TXT_010", text, cfg.times);
    }

    function buildTxt020(cfg, folders, records) {
        var comp = makeComp("TXT_020__Montserrat character and line reveal sourceRect probe", cfg, folders.cases);
        var chars = addText(comp, "CHARACTER REVEAL", cfg.fontMontserrat, 48, [1, 1, 1], [256, 190]);
        chars.name = "TXT_020_character_reveal";
        addRangeAnimator(chars, "Characters", 1, 0, 100);
        var lines = addText(comp, "LINE ONE\nLINE TWO\nLINE THREE", cfg.fontMontserrat, 42, [1, 1, 1], [256, 330]);
        lines.name = "TXT_020_line_reveal";
        addRangeAnimator(lines, "Lines", 4, 0, 100);
        sampleLayer(records, "TXT_020", chars, cfg.times);
        sampleLayer(records, "TXT_020", lines, cfg.times);
    }

    function buildTxt040(cfg, folders, records) {
        var comp = makeComp("TXT_040__expression selector bounce sourceRect probe", cfg, folders.cases);
        var text = addText(comp, "BOUNCE SELECTOR", cfg.fontPoint, 64, [1, 1, 1], [256, 256]);
        text.name = "TXT_040_bounce_selector";
        addBounceExpressionAnimator(text);
        sampleLayer(records, "TXT_040", text, [0, 5 / 30, 10 / 30, 15 / 30, 20 / 30, 1, 1.5, 59 / 30]);
    }

    function buildGph010(cfg, folders, records) {
        var child = makeComp("GPH_010_child_text", cfg, folders.precomps);
        var childText = addText(child, "COLLAPSE", cfg.fontMontserrat, 48, [1, 1, 1], [256, 256]);
        childText.name = "GPH_010_child_text_layer";

        var comp = makeComp("GPH_010__nested precomp and collapse text sourceRect probe", cfg, folders.cases);
        var normal = comp.layers.add(child);
        normal.name = "rasterized_precomp";
        setLayerPosition(normal, [150, 256]);
        setLayerScale(normal, [180, 180]);
        var collapsed = comp.layers.add(child);
        collapsed.name = "collapsed_precomp";
        setLayerPosition(collapsed, [362, 256]);
        setLayerScale(collapsed, [180, 180]);
        collapsed.collapseTransformation = true;

        sampleLayer(records, "GPH_010", childText, [0, 0.5, 1, 1.5]);
        sampleLayer(records, "GPH_010", normal, [0, 0.5, 1, 1.5]);
        sampleLayer(records, "GPH_010", collapsed, [0, 0.5, 1, 1.5]);
    }

    function sampleLayer(records, caseId, layer, times) {
        for (var i = 0; i < times.length; i++) {
            sampleLayerAt(records, caseId, layer, times[i], false);
            sampleLayerAt(records, caseId, layer, times[i], true);
        }
    }

    function sampleLayerAt(records, caseId, layer, time, includeExtents) {
        var rect = layer.sourceRectAtTime(time, includeExtents);
        records.push({
            case_id: caseId,
            comp: layer.containingComp.name,
            layer: layer.name,
            time: safeNumber(time),
            include_extents: includeExtents,
            rect: rectRecord(rect),
            transform: {
                position: valueOf(layer.property("ADBE Transform Group").property("ADBE Position")),
                scale: valueOf(layer.property("ADBE Transform Group").property("ADBE Scale")),
                opacity: valueOf(layer.property("ADBE Transform Group").property("ADBE Opacity"))
            },
            collapse_transformation: !!layer.collapseTransformation
        });
    }

    function makeComp(name, cfg, folder) {
        var comp = app.project.items.addComp(name, cfg.width, cfg.height, 1, cfg.duration, cfg.fps);
        comp.parentFolder = folder;
        comp.bgColor = cfg.bg;
        comp.workAreaStart = 0;
        comp.workAreaDuration = cfg.duration;
        return comp;
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
        var animator = textProps.property("ADBE Text Animators").addProperty("ADBE Text Animator");
        animator.name = modeName + "_reveal";
        animator.property("ADBE Text Animator Properties").addProperty("ADBE Text Opacity").setValue(0);
        var selector = animator.property("ADBE Text Selectors").addProperty("ADBE Text Selector");
        selector.property("ADBE Text Percent Start").setValueAtTime(0, startValue);
        selector.property("ADBE Text Percent Start").setValueAtTime(layer.containingComp.duration, endValue);
        selector.property("ADBE Text Range Advanced").property("ADBE Text Range Type2").setValue(basedOnCode);
    }

    function addBounceExpressionAnimator(layer) {
        var textProps = layer.property("ADBE Text Properties");
        var animator = textProps.property("ADBE Text Animators").addProperty("ADBE Text Animator");
        animator.name = "expression_selector_bounce";
        animator.property("ADBE Text Animator Properties").addProperty("ADBE Text Scale 3D").setValue([0, 0, 100]);
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

    function setLayerPosition(layer, value) {
        layer.property("ADBE Transform Group").property("ADBE Position").setValue(value);
    }

    function setLayerScale(layer, value) {
        layer.property("ADBE Transform Group").property("ADBE Scale").setValue(value);
    }

    function valueOf(prop) {
        try {
            var value = prop.value;
            if (value instanceof Array) {
                var out = [];
                for (var i = 0; i < value.length; i++) {
                    out.push(safeNumber(value[i]));
                }
                return out;
            }
            return safeNumber(value);
        } catch (_err) {
            return null;
        }
    }

    function rectRecord(rect) {
        return {
            left: safeNumber(rect.left),
            top: safeNumber(rect.top),
            width: safeNumber(rect.width),
            height: safeNumber(rect.height)
        };
    }

    function getOrCreateFolder(name) {
        for (var i = 1; i <= app.project.numItems; i++) {
            var item = app.project.item(i);
            if (item instanceof FolderItem && item.name === name) {
                return item;
            }
        }
        return app.project.items.addFolder(name);
    }

    function ensureFolder(folder) {
        if (!folder.exists) {
            folder.create();
        }
    }

    function safeNumber(value) {
        var n = Number(value);
        if (!isFinite(n)) {
            return 0;
        }
        return n;
    }

    function writeTextFile(file, text) {
        file.encoding = "UTF-8";
        file.open("w");
        file.write(text);
        file.close();
    }

    function toJson(value, indent) {
        indent = indent || 0;
        var pad = repeat("  ", indent);
        var childPad = repeat("  ", indent + 1);
        if (value === null || typeof value === "undefined") {
            return "null";
        }
        if (typeof value === "number") {
            if (!isFinite(value)) {
                return "0";
            }
            return String(value);
        }
        if (typeof value === "boolean") {
            return value ? "true" : "false";
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
