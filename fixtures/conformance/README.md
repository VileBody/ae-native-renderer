# Conformance Fixtures

These micro-scenes are data-only probes for future AE parity checks. They are
small enough to render quickly and deterministic enough to use as golden tests
once matching AE reference PNGs are exported.

The manifest at `fixtures/conformance/manifest.json` records:

- the scene path;
- the feature area under test;
- frame indexes that should be compared;
- pending AE reference PNG paths;
- provisional diff thresholds.

Reference PNGs are intentionally absent. Put exported contiguous PNG sequences
under `fixtures/conformance/ae-reference/<case>/` when a case is ready to
graduate from scaffold to enforced golden comparison; the manifest frame list
marks the key frames to inspect.
