/*
Single-frame Turbulent Displace trace probe.

This is intentionally tiny so Frida can stalk render worker threads without
spending minutes on a full sequence.
*/

(function buildTurbulentSingleFrameTraceProject() {
    app.beginUndoGroup("Build Turbulent Single Frame Trace Probe");

    var SCRIPT_FILE = new File($.fileName);
    var PACK_DIR = SCRIPT_FILE.parent.parent;
    var ASSET_DIR = new Folder(PACK_DIR.fsName + "/assets/primitives");
    var OUT_DIR = new Folder(PACK_DIR.fsName + "/ae_goldens/png8");
    var LOG_DIR = new Folder(PACK_DIR.fsName + "/ae_goldens/logs");
    ensureFolder(OUT_DIR);
    ensureFolder(LOG_DIR);

    var LOG_FILE = new File(LOG_DIR.fsName + "/turbulent_single_frame_trace_log.txt");
    LOG_FILE.open("w");
    logLine("Turbulent single-frame trace probe");
    logLine("Pack: " + PACK_DIR.fsName);
    purgeAeCaches("before build");

    if (!app.project) {
        app.newProject();
    }
    app.project.bitsPerChannel = 8;

    var caseId = "TD_TRACE_SINGLE_A045";
    var source = importStill("coordinate_field", "coordinate_field.png");
    var comp = app.project.items.addComp(caseId + "__single frame amount 45", 512, 512, 1, 1.0 / 30.0, 30);
    comp.bgColor = [0, 0, 0];
    var layer = comp.layers.add(source);
    layer.name = caseId + "_source";
    layer.property("ADBE Transform Group").property("ADBE Position").setValue([256, 256]);
    layer.property("ADBE Transform Group").property("ADBE Scale").setValue([100, 100]);
    layer.inPoint = 0;
    layer.outPoint = comp.duration;

    var fx = layer.property("ADBE Effect Parade").addProperty("ADBE Turbulent Displace");
    if (!fx) {
        throw new Error("AE could not add ADBE Turbulent Displace");
    }
    setIndexedScalar(fx, 1, 1, "displacement");
    setIndexedScalar(fx, 2, 45, "amount");
    setIndexedScalar(fx, 3, 65, "size");
    setIndexedScalar(fx, 5, 2, "complexity");
    setIndexedScalar(fx, 6, 0, "evolution");
    setIndexedScalar(fx, 10, 0, "random seed");
    setIndexedScalar(fx, 12, 3, "pinning");
    setIndexedScalar(fx, 13, 0, "resize layer");
    setIndexedScalar(fx, 14, 1, "antialiasing");

    var caseDir = new Folder(OUT_DIR.fsName + "/" + caseId);
    ensureFolder(caseDir);
    var rq = app.project.renderQueue.items.add(comp);
    var om = rq.outputModule(1);
    try {
        om.applyTemplate("PNG Sequence");
    } catch (_err) {
    }
    om.file = new File(caseDir.fsName + "/" + caseId + "_[#####].png");

    purgeAeCaches("after queue");
    LOG_FILE.close();
    app.endUndoGroup();

    function importStill(name, relative) {
        var file = new File(ASSET_DIR.fsName + "/" + relative);
        if (!file.exists) {
            throw new Error("Missing primitive asset: " + file.fsName);
        }
        var options = new ImportOptions(file);
        options.importAs = ImportAsType.FOOTAGE;
        var item = app.project.importFile(options);
        item.name = name;
        return item;
    }

    function setIndexedScalar(fx, index, value, label) {
        var prop = fx.property(index);
        if (!prop) {
            logLine("missing indexed control " + index + " for " + label);
            return false;
        }
        try {
            prop.setValue(value);
            logLine("set " + label + " index " + index + " to " + value);
            return true;
        } catch (err) {
            logLine("failed setting " + label + " index " + index + " to " + value + ": " + err.toString());
            return false;
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
