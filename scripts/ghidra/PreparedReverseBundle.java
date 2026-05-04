// Builds a file-based reverse bundle for focused AE parity work.
// Raw output is intended for ignored target/reverse/predecoded directories.

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Instruction;
import ghidra.program.model.listing.InstructionIterator;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.Symbol;

import java.io.File;
import java.io.FileOutputStream;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Set;

public class PreparedReverseBundle extends GhidraScript {
    private static final int DECOMPILE_TIMEOUT_SECONDS = 120;
    private static final int MAX_DECOMPILE_LINES = 5000;
    private static final int MAX_DISASSEMBLY_LINES = 20000;
    private static final int MAX_REF_LINES = 3000;

    private static class TargetSpec {
        String label;
        Address address;

        TargetSpec(String label, Address address) {
            this.label = label;
            this.address = address;
        }
    }

    private Address parseHexAddress(String text) {
        String s = text.trim();
        if (s.startsWith("0x") || s.startsWith("0X")) {
            s = s.substring(2);
        }
        return toAddr(Long.parseUnsignedLong(s, 16));
    }

    private TargetSpec parseTargetSpec(String spec) {
        String label = spec;
        String address = spec;
        int at = spec.lastIndexOf('@');
        if (at >= 0) {
            label = spec.substring(0, at);
            address = spec.substring(at + 1);
        }
        if (label.trim().isEmpty()) {
            label = address;
        }
        return new TargetSpec(label.trim(), parseHexAddress(address.trim()));
    }

    private String safeFileName(String text) {
        String safe = text.replaceAll("[^A-Za-z0-9._-]+", "_");
        safe = safe.replaceAll("_+", "_");
        if (safe.length() > 96) {
            safe = safe.substring(0, 96);
        }
        if (safe.length() == 0) {
            return "target";
        }
        return safe;
    }

    private PrintWriter writer(File file) throws Exception {
        File parent = file.getParentFile();
        if (parent != null) {
            parent.mkdirs();
        }
        return new PrintWriter(new OutputStreamWriter(new FileOutputStream(file), StandardCharsets.UTF_8));
    }

    private String safeStringAt(Address address) {
        try {
            Data data = currentProgram.getListing().getDataContaining(address);
            if (data != null && data.getValue() != null) {
                String value = data.getValue().toString();
                if (value.length() > 0 && value.length() < 2000) {
                    return value.replace('\n', ' ').replace('\r', ' ');
                }
            }
        } catch (Exception ignored) {
        }
        return null;
    }

    private Function functionFor(Address address) {
        Function fn = getFunctionAt(address);
        if (fn == null) {
            fn = getFunctionContaining(address);
        }
        return fn;
    }

    private String functionNameAt(Address address) {
        Function fn = functionFor(address);
        return fn == null ? "<none>" : fn.getName(true);
    }

    private void writeMetadata(File dir, TargetSpec spec, Function fn) throws Exception {
        PrintWriter out = writer(new File(dir, "metadata.md"));
        out.println("# " + spec.label);
        out.println();
        out.println("| Field | Value |");
        out.println("| --- | --- |");
        out.println("| Program | `" + currentProgram.getName() + "` |");
        out.println("| Image base | `" + currentProgram.getImageBase() + "` |");
        out.println("| Requested address | `" + spec.address + "` |");
        MemoryBlock block = currentProgram.getMemory().getBlock(spec.address);
        out.println("| Memory block | `" + (block == null ? "<none>" : block.getName()) + "` |");
        if (fn == null) {
            out.println("| Function | `<missing>` |");
            Data data = currentProgram.getListing().getDataAt(spec.address);
            Data containing = currentProgram.getListing().getDataContaining(spec.address);
            out.println("| Data at requested | `" + describeData(data) + "` |");
            out.println("| Data containing requested | `" + describeData(containing) + "` |");
            out.println();
            out.println("## Symbols At Requested Address");
            writeSymbols(out, spec.address);
            out.close();
            return;
        }
        out.println("| Function entry | `" + fn.getEntryPoint() + "` |");
        out.println("| Function name | `" + fn.getName(true).replace("|", "\\|") + "` |");
        out.println("| Body | `" + fn.getBody() + "` |");
        out.println("| Body addresses | `" + fn.getBody().getNumAddresses() + "` |");
        out.println("| Calling convention | `" + fn.getCallingConventionName() + "` |");
        out.println("| Parameter count | `" + fn.getParameterCount() + "` |");
        out.println("| Return type | `" + fn.getReturnType() + "` |");
        out.println();
        out.println("## Symbols At Entry");
        writeSymbols(out, fn.getEntryPoint());
        out.close();
    }

    private String describeData(Data data) {
        if (data == null) {
            return "<none>";
        }
        String value = "";
        try {
            Object obj = data.getValue();
            if (obj != null) {
                value = " value=" + obj.toString().replace("|", "\\|").replace('\n', ' ').replace('\r', ' ');
            }
        } catch (Exception ignored) {
        }
        return data.getAddress() + " " + data.getDataType().getName() + value;
    }

    private void writeSymbols(PrintWriter out, Address address) {
        Symbol[] symbols = currentProgram.getSymbolTable().getSymbols(address);
        if (symbols.length == 0) {
            out.println();
            out.println("- `<none>`");
        } else {
            out.println();
            for (Symbol symbol : symbols) {
                out.println("- `" + symbol.getName(true) + "` primary=`" + symbol.isPrimary() + "`");
            }
        }
    }

    private void writeDataTarget(File dir, TargetSpec spec) throws Exception {
        PrintWriter out = writer(new File(dir, "data_target.md"));
        out.println("# Data Target");
        out.println();
        out.println("| Field | Value |");
        out.println("| --- | --- |");
        out.println("| Requested address | `" + spec.address + "` |");
        MemoryBlock block = currentProgram.getMemory().getBlock(spec.address);
        out.println("| Memory block | `" + (block == null ? "<none>" : block.getName()) + "` |");
        out.println("| Data at requested | `" + describeData(currentProgram.getListing().getDataAt(spec.address)) + "` |");
        out.println("| Data containing requested | `" + describeData(currentProgram.getListing().getDataContaining(spec.address)) + "` |");
        out.println();
        out.println("## Symbols");
        writeSymbols(out, spec.address);
        out.println();
        out.println("## Incoming References");
        Reference[] incoming = getReferencesTo(spec.address);
        if (incoming.length == 0) {
            out.println();
            out.println("- `<none>`");
        } else {
            out.println();
            for (Reference ref : incoming) {
                Function caller = getFunctionContaining(ref.getFromAddress());
                out.println("- `" + ref.getFromAddress() + "` `" + ref.getReferenceType() + "` caller=`" +
                    (caller == null ? "<none>" : caller.getName(true)) + "`");
            }
        }
        out.close();
    }

    private void writeDisassembly(File dir, Function fn) throws Exception {
        PrintWriter out = writer(new File(dir, "disassembly.txt"));
        if (fn == null) {
            out.println("missing function");
            out.close();
            return;
        }
        int count = 0;
        InstructionIterator it = currentProgram.getListing().getInstructions(fn.getBody(), true);
        while (it.hasNext()) {
            Instruction ins = it.next();
            out.println(ins.getAddress() + ": " + ins.toString());
            count++;
            if (count >= MAX_DISASSEMBLY_LINES) {
                out.println("... truncated disassembly after " + MAX_DISASSEMBLY_LINES + " instructions ...");
                break;
            }
        }
        out.close();
    }

    private void writeRefs(File dir, Function fn) throws Exception {
        PrintWriter callees = writer(new File(dir, "callees.tsv"));
        PrintWriter dataRefs = writer(new File(dir, "data_refs.tsv"));
        PrintWriter callers = writer(new File(dir, "callers.tsv"));
        PrintWriter allRefs = writer(new File(dir, "refs.tsv"));

        callees.println("from\tto\ttype\tcallee");
        dataRefs.println("from\tto\ttype\tvalue");
        callers.println("from\tto\ttype\tcaller");
        allRefs.println("direction\tfrom\tto\ttype\tfunction\tvalue");

        if (fn == null) {
            callees.close();
            dataRefs.close();
            callers.close();
            allRefs.close();
            return;
        }

        int refCount = 0;
        InstructionIterator it = currentProgram.getListing().getInstructions(fn.getBody(), true);
        while (it.hasNext() && refCount < MAX_REF_LINES) {
            Instruction ins = it.next();
            for (Reference ref : ins.getReferencesFrom()) {
                Address to = ref.getToAddress();
                if (to == null) {
                    continue;
                }
                String type = String.valueOf(ref.getReferenceType());
                String fnName = to.isExternalAddress() ? "<external>" : functionNameAt(to);
                String value = to.isExternalAddress() ? "" : safeStringAt(to);
                if (value == null) {
                    value = "";
                }
                allRefs.println("from\t" + ins.getAddress() + "\t" + to + "\t" + type + "\t" + fnName + "\t" + value);
                if (ref.getReferenceType() != null && ref.getReferenceType().isCall()) {
                    callees.println(ins.getAddress() + "\t" + to + "\t" + type + "\t" + fnName);
                } else if (value.length() > 0) {
                    dataRefs.println(ins.getAddress() + "\t" + to + "\t" + type + "\t" + value);
                }
                refCount++;
                if (refCount >= MAX_REF_LINES) {
                    break;
                }
            }
        }

        Reference[] incoming = getReferencesTo(fn.getEntryPoint());
        for (Reference ref : incoming) {
            Function caller = getFunctionContaining(ref.getFromAddress());
            callers.println(ref.getFromAddress() + "\t" + ref.getToAddress() + "\t" + ref.getReferenceType() + "\t" +
                (caller == null ? "<none>" : caller.getName(true)));
            allRefs.println("to\t" + ref.getFromAddress() + "\t" + ref.getToAddress() + "\t" + ref.getReferenceType() + "\t" +
                (caller == null ? "<none>" : caller.getName(true)) + "\t");
        }

        callees.close();
        dataRefs.close();
        callers.close();
        allRefs.close();
    }

    private void writeDecompiler(File dir, Function fn, DecompInterface ifc) throws Exception {
        PrintWriter out = writer(new File(dir, "decompile.c"));
        if (fn == null) {
            out.println("/* missing function */");
            out.close();
            return;
        }
        DecompileResults res = ifc.decompileFunction(fn, DECOMPILE_TIMEOUT_SECONDS, monitor);
        if (res == null || !res.decompileCompleted() || res.getDecompiledFunction() == null) {
            out.println("/* decompile_failed */");
            if (res != null) {
                out.println("/* " + res.getErrorMessage() + " */");
            }
            out.close();
            return;
        }
        String[] lines = res.getDecompiledFunction().getC().split("\\r?\\n");
        int max = Math.min(lines.length, MAX_DECOMPILE_LINES);
        for (int i = 0; i < max; i++) {
            out.println(lines[i]);
        }
        if (lines.length > max) {
            out.println("/* ... truncated " + (lines.length - max) + " lines ... */");
        }
        out.close();
    }

    private void dumpTarget(File outDir, PrintWriter index, int ordinal, TargetSpec spec, DecompInterface ifc) throws Exception {
        Function fn = functionFor(spec.address);
        String dirName = String.format("%02d_%s_%s", ordinal, safeFileName(spec.label), spec.address.toString());
        File targetDir = new File(outDir, dirName);
        targetDir.mkdirs();

        writeMetadata(targetDir, spec, fn);
        writeDataTarget(targetDir, spec);
        writeDisassembly(targetDir, fn);
        writeRefs(targetDir, fn);
        writeDecompiler(targetDir, fn, ifc);

        index.println("| `" + spec.label + "` | `" + spec.address + "` | `" +
            (fn == null ? "<data/no function>" : fn.getEntryPoint()) + "` | `" +
            (fn == null ? dataOrSymbolLabel(spec.address) : fn.getName(true).replace("|", "\\|")) + "` | `" + dirName + "` |");
    }

    private String dataOrSymbolLabel(Address address) {
        Symbol[] symbols = currentProgram.getSymbolTable().getSymbols(address);
        if (symbols.length > 0) {
            return symbols[0].getName(true).replace("|", "\\|");
        }
        Data data = currentProgram.getListing().getDataAt(address);
        if (data != null) {
            return data.getDataType().getName().replace("|", "\\|");
        }
        return "<missing>";
    }

    @Override
    protected void run() throws Exception {
        String[] args = getScriptArgs();
        if (args == null || args.length < 3) {
            println("usage: PreparedReverseBundle.java <out_dir> <task_id> <label@0xADDR>...");
            return;
        }

        File outDir = new File(args[0]);
        outDir.mkdirs();
        String taskId = args[1];

        List<TargetSpec> targets = new ArrayList<TargetSpec>();
        for (int i = 2; i < args.length; i++) {
            targets.add(parseTargetSpec(args[i]));
        }

        DecompInterface ifc = new DecompInterface();
        ifc.openProgram(currentProgram);

        PrintWriter index = writer(new File(outDir, "index.md"));
        index.println("# Prepared Reverse Bundle");
        index.println();
        index.println("| Field | Value |");
        index.println("| --- | --- |");
        index.println("| Task | `" + taskId + "` |");
        index.println("| Program | `" + currentProgram.getName() + "` |");
        index.println("| Image base | `" + currentProgram.getImageBase() + "` |");
        index.println("| Target count | `" + targets.size() + "` |");
        index.println();
        index.println("| Label | Requested | Function Entry | Function Name | Directory |");
        index.println("| --- | --- | --- | --- | --- |");

        int ordinal = 1;
        for (TargetSpec target : targets) {
            dumpTarget(outDir, index, ordinal, target, ifc);
            ordinal++;
        }
        index.close();

        println("prepared_bundle=" + outDir.getAbsolutePath());
        println("targets=" + targets.size());
    }
}
