/*
AE Native Renderer Text Telemetry Export

Run from After Effects through the remote pack runner:
  jsx/export_text_telemetry.jsx

The script writes compact AE-side JSONL references for text layout and selector
unit mapping. These are intentionally reference subsets for
render-cli conformance-pack text_passport_comparison.json.
*/

(function exportTextTelemetry() {
    var SCRIPT_FILE = new File($.fileName);
    var PACK_DIR = SCRIPT_FILE.parent.parent;
    var OUT_DIR = new Folder(PACK_DIR.fsName + "/ae_goldens/text_telemetry");
    var METADATA_DIR = new Folder(PACK_DIR.fsName + "/ae_goldens/metadata");
    ensureFolder(OUT_DIR);
    ensureFolder(METADATA_DIR);

    var CFG = {
        width: 512,
        height: 512,
        fps: 30,
        duration: 2.0,
        fontMontserrat: "Montserrat-BoldItalic",
        fontPoint: "Point-Light"
    };

    if (!app.project) {
        app.newProject();
    }
    app.project.bitsPerChannel = 8;

    var folder = getOrCreateFolder("AE_NATIVE_TEXT_TELEMETRY");
    var measureComp = app.project.items.addComp(
        "TEXT_TELEMETRY_MEASURE",
        CFG.width,
        CFG.height,
        1,
        CFG.duration,
        CFG.fps
    );
    measureComp.parentFolder = folder;

    var cases = textCases(CFG);
    var summary = {
        schema: "ae-native-renderer.text-telemetry-export.v1",
        generated_by: "fixtures/ae_conformance_pack/jsx/export_text_telemetry.jsx",
        reference_semantics: "AE sourceRectAtTime-based reference subset",
        frames_written: 0,
        cases: []
    };

    for (var i = 0; i < cases.length; i++) {
        var caseDef = cases[i];
        var caseDir = new Folder(OUT_DIR.fsName + "/" + caseDef.id);
        ensureFolder(caseDir);
        var layerSnapshots = [];
        for (var li = 0; li < caseDef.layers.length; li++) {
            layerSnapshots.push(buildLayerSnapshot(measureComp, caseDef, caseDef.layers[li]));
        }

        for (var fi = 0; fi < caseDef.frames.length; fi++) {
            var frame = caseDef.frames[fi];
            var time = frame / CFG.fps;
            var records = [];
            for (var si = 0; si < layerSnapshots.length; si++) {
                appendLayerRecords(records, caseDef, layerSnapshots[si], frame, time);
            }
            var outFile = new File(caseDir.fsName + "/" + caseDef.id + "_" + padInt(frame, 5) + ".jsonl");
            writeJsonl(outFile, records);
            summary.frames_written += 1;
        }

        var summaryLayers = [];
        for (var sli = 0; sli < layerSnapshots.length; sli++) {
            summaryLayers.push({
                layer_id: layerSnapshots[sli].layer_id,
                glyph_rows: layerSnapshots[sli].glyphs.length,
                selector_units: layerSnapshots[sli].selector_units.length
            });
        }
        summary.cases.push({
            id: caseDef.id,
            frames: caseDef.frames,
            layers: summaryLayers
        });
    }

    writeTextFile(
        new File(METADATA_DIR.fsName + "/text_telemetry_summary.json"),
        toJson(summary, 0)
    );
    app.project.save(new File(PACK_DIR.fsName + "/ae_goldens/text_telemetry_project.aep"));

    function textCases(cfg) {
        return [
            {
                id: "TXT_010",
                frames: [0, 8, 16, 24, 32, 45, 59],
                layers: [{
                    layer_id: "TXT_010_word_reveal",
                    text: "WORD REVEAL\nMONTSERRAT TEST",
                    font: cfg.fontMontserrat,
                    font_size: 58,
                    position: [256, 256],
                    selector: {
                        animator: "words_reveal",
                        based_on: "words"
                    }
                }]
            },
            {
                id: "TXT_020",
                frames: [0, 8, 16, 24, 32, 45, 59],
                layers: [
                    {
                        layer_id: "TXT_020_character_reveal",
                        text: "CHARACTER REVEAL",
                        font: cfg.fontMontserrat,
                        font_size: 48,
                        position: [256, 190],
                        selector: {
                            animator: "characters_reveal",
                            based_on: "characters"
                        }
                    },
                    {
                        layer_id: "TXT_020_line_reveal",
                        text: "LINE ONE\nLINE TWO\nLINE THREE",
                        font: cfg.fontMontserrat,
                        font_size: 42,
                        position: [256, 330],
                        selector: {
                            animator: "lines_reveal",
                            based_on: "lines"
                        }
                    }
                ]
            },
            {
                id: "TXT_030",
                frames: [0, 8, 16, 24, 32, 45, 59],
                layers: [{
                    layer_id: "TXT_030_glyph_motion",
                    text: "GLYPH MOTION",
                    font: cfg.fontPoint,
                    font_size: 74,
                    position: [256, 256],
                    selector: {
                        animator: "glyph_position_scale_rotation_blur",
                        based_on: "characters"
                    }
                }]
            },
            {
                id: "TXT_040",
                frames: [0, 5, 10, 15, 20, 30, 45, 59],
                layers: [{
                    layer_id: "TXT_040_bounce_selector",
                    text: "BOUNCE SELECTOR",
                    font: cfg.fontPoint,
                    font_size: 64,
                    position: [256, 256],
                    selector: {
                        animator: "expression_selector_bounce",
                        based_on: "characters"
                    }
                }]
            }
        ];
    }

    function buildLayerSnapshot(comp, caseDef, layerDef) {
        var glyphs = buildGlyphRows(comp, layerDef);
        return {
            composition: caseDef.id,
            layer_id: layerDef.layer_id,
            text: layerDef.text,
            font: layerDef.font,
            font_size: layerDef.font_size,
            selector: layerDef.selector,
            glyphs: glyphs,
            selector_units: selectorUnits(glyphs, layerDef.selector.based_on)
        };
    }

    function appendLayerRecords(records, caseDef, snapshot, frame, time) {
        records.push({
            event: "text.layout",
            frame: frame,
            time: time,
            record: {
                composition: caseDef.id,
                layer_id: snapshot.layer_id,
                layout: {
                    glyphs: snapshot.glyphs
                }
            }
        });
        records.push({
            event: "text.selector_weights",
            frame: frame,
            time: time,
            record: {
                composition: caseDef.id,
                layer_id: snapshot.layer_id,
                animator: snapshot.selector.animator,
                selector: {
                    based_on: snapshot.selector.based_on
                },
                units: snapshot.selector_units
            }
        });
    }

    function buildGlyphRows(comp, layerDef) {
        var rows = [];
        var lines = String(layerDef.text).split("\n");
        var charOffset = 0;
        var wordIndex = 0;
        var seenWord = false;
        var inWord = false;
        var glyphRunIndex = 0;
        var lineHeight = layerDef.font_size * 1.2;
        var blockTop = layerDef.position[1] - ((lines.length - 1) * lineHeight) * 0.5;

        for (var lineIndex = 0; lineIndex < lines.length; lineIndex++) {
            var line = lines[lineIndex];
            var lineMetrics = measureText(comp, line.length ? line : " ", layerDef.font, layerDef.font_size);
            var lineWidth = Math.max(0, lineMetrics.width);
            var lineStartX = layerDef.position[0] - lineWidth * 0.5;
            var baseline = blockTop + lineIndex * lineHeight;
            var y = baseline + lineMetrics.top;
            var height = Math.max(0, lineMetrics.height);
            inWord = false;

            for (var localIndex = 0; localIndex < line.length; localIndex++) {
                var ch = line.charAt(localIndex);
                var isWord = !isWhitespace(ch);
                if (isWord && !inWord) {
                    if (seenWord) {
                        wordIndex += 1;
                    }
                    inWord = true;
                    seenWord = true;
                } else if (!isWord) {
                    inWord = false;
                }

                var before = line.substring(0, localIndex);
                var through = line.substring(0, localIndex + 1);
                var beforeWidth = measureText(comp, before.length ? before : " ", layerDef.font, layerDef.font_size).width;
                if (before.length === 0) {
                    beforeWidth = 0;
                }
                var throughWidth = measureText(comp, through.length ? through : " ", layerDef.font, layerDef.font_size).width;
                var advance = Math.max(0, throughWidth - beforeWidth);
                var x = lineStartX + beforeWidth;
                var bbox = [x, y, advance, height];

                rows.push({
                    character: ch,
                    glyph_run_index: glyphRunIndex,
                    char_index: charOffset + localIndex,
                    word_index: wordIndex,
                    line_index: lineIndex,
                    advance: advance,
                    advance_x: advance,
                    advance_y: 0,
                    bbox: bbox,
                    cooltype_bbox_minmax: [bbox[0], bbox[1], bbox[0] + bbox[2], bbox[1] + bbox[3]],
                    bbox_center: [bbox[0] + bbox[2] * 0.5, bbox[1] + bbox[3] * 0.5],
                    font_postscript_name: layerDef.font
                });
                glyphRunIndex += 1;
            }
            charOffset += line.length + 1;
        }
        return rows;
    }

    function selectorUnits(glyphs, basedOn) {
        if (basedOn === "characters") {
            var charUnits = [];
            for (var i = 0; i < glyphs.length; i++) {
                if (!isWhitespace(glyphs[i].character)) {
                    charUnits.push(selectorUnit(charUnits.length, [glyphs[i]]));
                }
            }
            return charUnits;
        }
        if (basedOn === "words") {
            return groupedUnits(glyphs, "word_index");
        }
        if (basedOn === "lines") {
            return groupedUnits(glyphs, "line_index");
        }
        return [];
    }

    function groupedUnits(glyphs, keyName) {
        var keys = [];
        var groups = {};
        for (var i = 0; i < glyphs.length; i++) {
            if (isWhitespace(glyphs[i].character)) {
                continue;
            }
            var key = String(glyphs[i][keyName]);
            if (!groups.hasOwnProperty(key)) {
                groups[key] = [];
                keys.push(key);
            }
            groups[key].push(glyphs[i]);
        }
        keys.sort(function (a, b) {
            return parseInt(a, 10) - parseInt(b, 10);
        });
        var out = [];
        for (var k = 0; k < keys.length; k++) {
            out.push(selectorUnit(out.length, groups[keys[k]]));
        }
        return out;
    }

    function selectorUnit(index, glyphs) {
        return {
            index: index,
            glyph_passport: {
                available: true,
                glyph_count: glyphs.length,
                glyph_run_indices: mapGlyphs(glyphs, "glyph_run_index"),
                char_indices: mapGlyphs(glyphs, "char_index"),
                word_indices: mapGlyphs(glyphs, "word_index"),
                line_indices: mapGlyphs(glyphs, "line_index"),
                characters: mapGlyphs(glyphs, "character")
            }
        };
    }

    function mapGlyphs(glyphs, key) {
        var out = [];
        for (var i = 0; i < glyphs.length; i++) {
            out.push(glyphs[i][key]);
        }
        return out;
    }

    function measureText(comp, value, fontName, fontSize) {
        var layer = comp.layers.addText(value);
        var docProp = layer.property("ADBE Text Properties").property("ADBE Text Document");
        var doc = docProp.value;
        doc.font = fontName;
        doc.fontSize = fontSize;
        doc.fillColor = [1, 1, 1];
        doc.applyFill = true;
        doc.justification = ParagraphJustification.CENTER_JUSTIFY;
        docProp.setValue(doc);
        layer.property("ADBE Transform Group").property("ADBE Position").setValue([256, 256]);
        var rect = layer.sourceRectAtTime(0, false);
        layer.remove();
        return {
            left: safeNumber(rect.left),
            top: safeNumber(rect.top),
            width: safeNumber(rect.width),
            height: safeNumber(rect.height)
        };
    }

    function writeJsonl(file, records) {
        file.encoding = "UTF-8";
        file.open("w");
        for (var i = 0; i < records.length; i++) {
            file.writeln(toJsonLine(records[i]));
        }
        file.close();
    }

    function writeTextFile(file, text) {
        file.encoding = "UTF-8";
        file.open("w");
        file.write(text);
        file.close();
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

    function isWhitespace(value) {
        return /^[\s\r\n\t]+$/.test(String(value));
    }

    function padInt(value, width) {
        var text = String(value);
        while (text.length < width) {
            text = "0" + text;
        }
        return text;
    }

    function safeNumber(value) {
        var n = Number(value);
        if (!isFinite(n)) {
            return 0;
        }
        return n;
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

    function toJsonLine(value) {
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
            var arrParts = [];
            for (var i = 0; i < value.length; i++) {
                arrParts.push(toJsonLine(value[i]));
            }
            return "[" + arrParts.join(",") + "]";
        }
        var objParts = [];
        for (var key in value) {
            if (value.hasOwnProperty(key)) {
                objParts.push(quoteJson(key) + ":" + toJsonLine(value[key]));
            }
        }
        return "{" + objParts.join(",") + "}";
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
