// Finds CoolType glyph/text metric function candidates from strings, symbols,
// and xrefs. Output is intended for ignored target/reverse directories.

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.DataIterator;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionIterator;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.SymbolIterator;

import java.io.File;
import java.io.FileOutputStream;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

public class CoolTypeGlyphTargetFinder extends GhidraScript {
    private static final int MAX_HITS_PER_FUNCTION = 80;

    private static class Needle {
        String text;
        int weight;

        Needle(String text, int weight) {
            this.text = text.toLowerCase();
            this.weight = weight;
        }
    }

    private static class Candidate {
        Address entry;
        String name;
        int score;
        List<String> hits = new ArrayList<>();

        Candidate(Function fn) {
            this.entry = fn.getEntryPoint();
            this.name = fn.getName(true);
        }
    }

    private static class StringHit {
        Address address;
        String value;
        int refs;
        List<String> callers = new ArrayList<>();
    }

    private final Needle[] needles = new Needle[] {
        new Needle("CTFontInstanceGetGlyphIDProc", 80),
        new Needle("CTFontInstanceGetGlyphIDsProc", 80),
        new Needle("CTFontInstanceGetWidthProc", 80),
        new Needle("CTFontInstanceGetWidthsProc", 80),
        new Needle("CTFontInstanceGetBBoxProc", 80),
        new Needle("CTFontInstanceGetBBoxesProc", 80),
        new Needle("CTFontInstanceGetBaselineDeltasProc", 80),
        new Needle("CTFontInstanceApplyFeaturesProc", 70),
        new Needle("CTFontInstanceProcessFeaturesProc", 70),
        new Needle("CTTextGetGlyphs", 70),
        new Needle("CTTextGetTextGlyphs", 70),
        new Needle("CTTextGetNumGlyphs", 70),
        new Needle("CTGlyphAccess", 70),
        new Needle("GetGlyphID", 55),
        new Needle("GetGlyphIDs", 55),
        new Needle("GetNumGlyphIDs", 50),
        new Needle("GetWidth", 45),
        new Needle("GetWidths", 55),
        new Needle("GetBBox", 55),
        new Needle("GetBBoxes", 55),
        new Needle("GetBaselineDeltas", 55),
        new Needle("ApplyFeatures", 45),
        new Needle("ProcessFeatures", 45),
        new Needle("GetXPSVerticalGlyphMetrics", 45),
        new Needle("failed to get glyph metrics", 90),
        new Needle("CharBBox error", 70),
        new Needle("requested glyph not in font", 65),
        new Needle("invalid glyph id", 65),
        new Needle("ct_numglyphs", 70),
        new Needle("ct_notdefglyphid", 60),
        new Needle("ct_fontbbox", 65),
        new Needle("ct_fontbboxHead", 65),
        new Needle("ct_baselines", 70),
        new Needle("ct_horizontalmetrics", 60),
        new Needle("ct_FTEHorizontalMetrics", 60),
        new Needle("ct_macHorizontalMetrics", 60),
        new Needle("ct_winHorizontalMetrics", 60),
        new Needle("ct_unitsPerEm", 55),
        new Needle("ct_GSUBTable", 60),
        new Needle("ct_GPOSTable", 60),
        new Needle("ct_GDEFTable", 60),
        new Needle("ct_BASETable", 65),
        new Needle("ct_ValidGlyphIDRanges", 65),
        new Needle("GlyphDirectory", 45),
        new Needle("FontBBox", 45),
        new Needle("WidthsOnly", 35),
        new Needle("defaultWidthX", 45),
        new Needle("nominalWidthX", 45),
        new Needle("sidebearing", 50),
        new Needle("VORG", 45),
        new Needle("var_lookuphmtx", 55),
        new Needle("var_lookupvmtx", 55)
    };

    private PrintWriter writer(File file) throws Exception {
        File parent = file.getParentFile();
        if (parent != null) {
            parent.mkdirs();
        }
        return new PrintWriter(new OutputStreamWriter(new FileOutputStream(file), StandardCharsets.UTF_8));
    }

    private String clean(String s) {
        return s.replace('\t', ' ').replace('\n', ' ').replace('\r', ' ').trim();
    }

    private int matchWeight(String text) {
        String lower = text.toLowerCase();
        int score = 0;
        for (Needle needle : needles) {
            if (lower.contains(needle.text)) {
                score += needle.weight;
            }
        }
        return score;
    }

    private void addCandidate(Map<String, Candidate> candidates, Function fn, int score, String hit) {
        if (fn == null || score <= 0) {
            return;
        }
        String key = fn.getEntryPoint().toString();
        Candidate candidate = candidates.get(key);
        if (candidate == null) {
            candidate = new Candidate(fn);
            candidates.put(key, candidate);
        }
        candidate.score += score;
        if (candidate.hits.size() < MAX_HITS_PER_FUNCTION) {
            candidate.hits.add(clean(hit));
        }
    }

    private Function functionFor(Address address) {
        Function fn = getFunctionAt(address);
        if (fn == null) {
            fn = getFunctionContaining(address);
        }
        return fn;
    }

    private void addXrefCandidates(
        Map<String, Candidate> candidates,
        Address address,
        int score,
        String hit
    ) {
        Reference[] refs = getReferencesTo(address);
        for (Reference ref : refs) {
            Function caller = functionFor(ref.getFromAddress());
            addCandidate(candidates, caller, score, hit + " xref@" + ref.getFromAddress());
        }
    }

    private List<Candidate> sortedCandidates(Map<String, Candidate> candidates) {
        List<Candidate> list = new ArrayList<>(candidates.values());
        Collections.sort(list, new Comparator<Candidate>() {
            @Override
            public int compare(Candidate a, Candidate b) {
                int byScore = Integer.compare(b.score, a.score);
                if (byScore != 0) {
                    return byScore;
                }
                return a.entry.compareTo(b.entry);
            }
        });
        return list;
    }

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 1) {
            println("usage: CoolTypeGlyphTargetFinder.java <output-dir>");
            return;
        }
        File outDir = new File(args[0]);
        outDir.mkdirs();

        Map<String, Candidate> candidates = new LinkedHashMap<>();
        List<StringHit> stringHits = new ArrayList<>();

        FunctionIterator functions = currentProgram.getFunctionManager().getFunctions(true);
        for (Function fn : functions) {
            int score = matchWeight(fn.getName(true));
            if (score > 0) {
                addCandidate(candidates, fn, score, "function-name:" + fn.getName(true));
            }
        }

        SymbolIterator symbols = currentProgram.getSymbolTable().getAllSymbols(true);
        while (symbols.hasNext()) {
            Symbol symbol = symbols.next();
            String name = symbol.getName(true);
            int score = matchWeight(name);
            if (score <= 0) {
                continue;
            }
            Function fn = functionFor(symbol.getAddress());
            addCandidate(candidates, fn, score, "symbol:" + name + "@" + symbol.getAddress());
            addXrefCandidates(candidates, symbol.getAddress(), score, "symbol:" + name);
        }

        DataIterator dataIt = currentProgram.getListing().getDefinedData(true);
        while (dataIt.hasNext()) {
            Data data = dataIt.next();
            Object valueObj = null;
            try {
                valueObj = data.getValue();
            } catch (Exception ignored) {
            }
            if (valueObj == null) {
                continue;
            }
            String value = valueObj.toString();
            int score = matchWeight(value);
            if (score <= 0) {
                continue;
            }
            Address address = data.getAddress();
            StringHit stringHit = new StringHit();
            stringHit.address = address;
            stringHit.value = clean(value);
            Reference[] refs = getReferencesTo(address);
            stringHit.refs = refs.length;
            for (Reference ref : refs) {
                Function caller = functionFor(ref.getFromAddress());
                if (caller != null) {
                    if (stringHit.callers.size() < 20) {
                        stringHit.callers.add(caller.getEntryPoint() + " " + caller.getName(true) + " @" + ref.getFromAddress());
                    }
                    addCandidate(candidates, caller, score, "string:" + value + "@" + address + " xref@" + ref.getFromAddress());
                }
            }
            stringHits.add(stringHit);
        }

        List<Candidate> sorted = sortedCandidates(candidates);

        try (PrintWriter out = writer(new File(outDir, "candidate_functions.tsv"))) {
            out.println("rank\tentry\tname\tscore\thit_count\thits");
            int rank = 1;
            for (Candidate candidate : sorted) {
                out.println(rank + "\t" + candidate.entry + "\t" + candidate.name + "\t" +
                    candidate.score + "\t" + candidate.hits.size() + "\t" + clean(String.join(" | ", candidate.hits)));
                rank++;
            }
        }

        try (PrintWriter out = writer(new File(outDir, "candidate_functions.md"))) {
            out.println("# CoolType Glyph Candidate Functions");
            out.println();
            out.println("| Rank | Entry | Function | Score | Sample hits |");
            out.println("| --- | --- | --- | ---: | --- |");
            int rank = 1;
            for (Candidate candidate : sorted) {
                String hits = clean(String.join("; ", candidate.hits));
                if (hits.length() > 500) {
                    hits = hits.substring(0, 500) + "...";
                }
                out.println("| " + rank + " | `" + candidate.entry + "` | `" +
                    candidate.name.replace("|", "\\|") + "` | " + candidate.score +
                    " | " + hits.replace("|", "\\|") + " |");
                rank++;
                if (rank > 120) {
                    break;
                }
            }
        }

        try (PrintWriter out = writer(new File(outDir, "matched_strings.tsv"))) {
            out.println("address\trefs\tvalue\tcallers");
            for (StringHit hit : stringHits) {
                out.println(hit.address + "\t" + hit.refs + "\t" + hit.value + "\t" + clean(String.join(" | ", hit.callers)));
            }
        }

        try (PrintWriter out = writer(new File(outDir, "index.md"))) {
            out.println("# CoolType Glyph Target Finder");
            out.println();
            out.println("| Field | Value |");
            out.println("| --- | --- |");
            out.println("| Program | `" + currentProgram.getName() + "` |");
            out.println("| Image base | `" + currentProgram.getImageBase() + "` |");
            MemoryBlock textBlock = currentProgram.getMemory().getBlock(".text");
            out.println("| .text | `" + (textBlock == null ? "<none>" : textBlock.getStart() + ".." + textBlock.getEnd()) + "` |");
            out.println("| Candidate functions | `" + sorted.size() + "` |");
            out.println("| Matched strings/data | `" + stringHits.size() + "` |");
            out.println();
            out.println("Top candidates are in `candidate_functions.md`; raw xref strings are in `matched_strings.tsv`.");
        }

        println("Wrote CoolType glyph target finder output to " + outDir.getAbsolutePath());
    }
}
