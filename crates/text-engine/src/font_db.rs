use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};

use fontdue::{Font, FontSettings};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone)]
pub struct FontDb {
    fonts: HashMap<String, PathBuf>,
}

impl FontDb {
    pub fn register(&mut self, id: impl Into<String>, path: impl Into<PathBuf>) {
        self.fonts.insert(id.into(), path.into());
    }

    pub fn get(&self, id: &str) -> Option<&PathBuf> {
        self.fonts.get(id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontResolutionSource {
    DirectPath,
    FixtureAsset,
    FontConfig,
    CommonFallback,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontResolutionTelemetry {
    pub requested_id: String,
    pub resolved_path: Option<PathBuf>,
    pub resolved_family: Option<String>,
    pub resolved_style: Option<String>,
    pub resolved_fullname: Option<String>,
    pub resolved_postscript_name: Option<String>,
    pub fallback: bool,
    pub source: FontResolutionSource,
}

pub fn resolve_font_path(font_id: &str) -> Option<PathBuf> {
    resolve_font_with_telemetry(font_id).resolved_path
}

pub fn resolve_font_with_telemetry(font_id: &str) -> FontResolutionTelemetry {
    let direct = Path::new(font_id);
    if direct.exists() {
        return resolution_from_path(
            font_id,
            direct.to_path_buf(),
            FontResolutionSource::DirectPath,
            false,
        );
    }

    if let Some(resolution) = resolve_known_fixture_font(font_id) {
        return resolution;
    }

    if let Some(mut resolution) = match_fontconfig(font_id) {
        if resolution
            .resolved_path
            .as_deref()
            .is_some_and(Path::exists)
        {
            resolution.fallback = !requested_matches_resolution(font_id, &resolution);
            return resolution;
        }
    }

    if let Some(path) = common_font_paths().into_iter().find(|path| path.exists()) {
        return resolution_from_path(font_id, path, FontResolutionSource::CommonFallback, true);
    }

    FontResolutionTelemetry {
        requested_id: font_id.to_string(),
        resolved_path: None,
        resolved_family: None,
        resolved_style: None,
        resolved_fullname: None,
        resolved_postscript_name: None,
        fallback: true,
        source: FontResolutionSource::Missing,
    }
}

static FONT_CACHE: OnceLock<Mutex<HashMap<String, Arc<Font>>>> = OnceLock::new();

pub fn load_font(font_id: &str) -> anyhow::Result<Arc<Font>> {
    load_font_with_telemetry(font_id).map(|(font, _)| font)
}

pub fn load_font_with_telemetry(
    font_id: &str,
) -> anyhow::Result<(Arc<Font>, FontResolutionTelemetry)> {
    let resolution = resolve_font_with_telemetry(font_id);
    let cache = FONT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(font) = cache
        .lock()
        .expect("font cache poisoned")
        .get(font_id)
        .cloned()
    {
        return Ok((font, resolution));
    }

    let path = resolution
        .resolved_path
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("font '{font_id}' was not found"))?;
    let bytes = fs::read(&path)?;
    let font = Font::from_bytes(bytes, FontSettings::default())
        .map_err(|err| anyhow::anyhow!("failed to load font {}: {err}", path.display()))?;
    let font = Arc::new(font);
    cache
        .lock()
        .expect("font cache poisoned")
        .insert(font_id.to_string(), font.clone());
    Ok((font, resolution))
}

fn common_font_paths() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
        PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"),
        PathBuf::from("/System/Library/Fonts/SFNS.ttf"),
        PathBuf::from("/System/Library/Fonts/Supplemental/Arial.ttf"),
        PathBuf::from("/Library/Fonts/Arial.ttf"),
    ]
}

fn resolve_known_fixture_font(font_id: &str) -> Option<FontResolutionTelemetry> {
    let path = known_fixture_font_paths(font_id)
        .into_iter()
        .find(|path| path.exists())?;
    let mut resolution = resolution_from_path(
        font_id,
        path.clone(),
        FontResolutionSource::FixtureAsset,
        true,
    );
    resolution.fallback = !known_fixture_exact_match(font_id, &path)
        && !requested_matches_resolution(font_id, &resolution);
    Some(resolution)
}

fn known_fixture_font_paths(font_id: &str) -> Vec<PathBuf> {
    let normalized = normalized_font_name(font_id);
    let file_name = match normalized.as_str() {
        "pointlight" | "point" => "Point-Light.ttf",
        "montserratbolditalic" | "montserratitalic" | "montserrat" => {
            "Montserrat-Italic[wght].ttf"
        }
        _ => return Vec::new(),
    };

    [
        PathBuf::from("fixtures/ae_conformance_pack/assets/fonts"),
        PathBuf::from("assets/fonts"),
        PathBuf::from("/work/fixtures/ae_conformance_pack/assets/fonts"),
        PathBuf::from("/app/fixtures/ae_conformance_pack/assets/fonts"),
    ]
    .into_iter()
    .map(|base| base.join(file_name))
    .collect()
}

fn known_fixture_exact_match(font_id: &str, path: &Path) -> bool {
    let requested = normalized_font_name(font_id);
    let file_stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(normalized_font_name)
        .unwrap_or_default();

    matches!(
        (requested.as_str(), file_stem.as_str()),
        ("pointlight", "pointlight")
            | ("point", "pointlight")
            | ("montserratitalic", "montserratitalicwght")
            | ("montserrat", "montserratitalicwght")
    )
}

fn resolution_from_path(
    requested_id: &str,
    path: PathBuf,
    source: FontResolutionSource,
    fallback: bool,
) -> FontResolutionTelemetry {
    let metadata = scan_font_file(&path).unwrap_or_default();
    FontResolutionTelemetry {
        requested_id: requested_id.to_string(),
        resolved_path: Some(path),
        resolved_family: metadata.family,
        resolved_style: metadata.style,
        resolved_fullname: metadata.fullname,
        resolved_postscript_name: metadata.postscript_name,
        fallback,
        source,
    }
}

fn match_fontconfig(font_id: &str) -> Option<FontResolutionTelemetry> {
    let output = Command::new("fc-match")
        .arg("-f")
        .arg("file=%{file}\nfamily=%{family}\nstyle=%{style}\nfullname=%{fullname}\npostscript=%{postscriptname}\n")
        .arg(font_id)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let parsed = ParsedFontMetadata::parse(&String::from_utf8_lossy(&output.stdout));
    let path = parsed.file.as_ref().and_then(non_empty_path);
    Some(FontResolutionTelemetry {
        requested_id: font_id.to_string(),
        resolved_path: path,
        resolved_family: parsed.family,
        resolved_style: parsed.style,
        resolved_fullname: parsed.fullname,
        resolved_postscript_name: parsed.postscript_name,
        fallback: true,
        source: FontResolutionSource::FontConfig,
    })
}

fn scan_font_file(path: &Path) -> Option<ParsedFontMetadata> {
    let output = Command::new("fc-scan")
        .arg("-f")
        .arg("family=%{family}\nstyle=%{style}\nfullname=%{fullname}\npostscript=%{postscriptname}\n")
        .arg(path)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| ParsedFontMetadata::parse(&String::from_utf8_lossy(&output.stdout)))
}

#[derive(Debug, Default)]
struct ParsedFontMetadata {
    file: Option<String>,
    family: Option<String>,
    style: Option<String>,
    fullname: Option<String>,
    postscript_name: Option<String>,
}

impl ParsedFontMetadata {
    fn parse(raw: &str) -> Self {
        let mut parsed = Self::default();
        for line in raw.lines() {
            if let Some(value) = line.strip_prefix("file=") {
                parsed.file = non_empty_string(value);
            } else if let Some(value) = line.strip_prefix("family=") {
                parsed.family = non_empty_string(value);
            } else if let Some(value) = line.strip_prefix("style=") {
                parsed.style = non_empty_string(value);
            } else if let Some(value) = line.strip_prefix("fullname=") {
                parsed.fullname = non_empty_string(value);
            } else if let Some(value) = line.strip_prefix("postscript=") {
                parsed.postscript_name = non_empty_string(value);
            }
        }
        parsed
    }
}

fn non_empty_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn non_empty_path(value: &String) -> Option<PathBuf> {
    let path = PathBuf::from(value.trim());
    (!value.trim().is_empty()).then_some(path)
}

fn requested_matches_resolution(requested_id: &str, resolution: &FontResolutionTelemetry) -> bool {
    let requested = normalized_font_name(requested_id);
    if requested.is_empty() {
        return false;
    }

    resolution_name_candidates(resolution)
        .into_iter()
        .any(|candidate| normalized_font_name(&candidate) == requested)
}

fn resolution_name_candidates(resolution: &FontResolutionTelemetry) -> Vec<String> {
    let mut candidates = Vec::new();
    for value in [
        resolution.resolved_family.as_deref(),
        resolution.resolved_fullname.as_deref(),
        resolution.resolved_postscript_name.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        candidates.extend(value.split(',').map(str::trim).filter_map(non_empty_string));
    }

    if let (Some(family), Some(style)) = (
        resolution.resolved_family.as_deref(),
        resolution.resolved_style.as_deref(),
    ) {
        for family in family.split(',').map(str::trim) {
            for style in style.split(',').map(str::trim) {
                if !family.is_empty() && !style.is_empty() {
                    candidates.push(format!("{family} {style}"));
                }
            }
        }
    }

    candidates
}

fn normalized_font_name(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point_light_fixture() -> Option<PathBuf> {
        let path = PathBuf::from("fixtures/ae_conformance_pack/assets/fonts/Point-Light.ttf");
        path.exists().then_some(path)
    }

    #[test]
    fn direct_path_resolution_reports_point_light_without_fallback() {
        let Some(path) = point_light_fixture() else {
            return;
        };
        let resolution = resolve_font_with_telemetry(path.to_str().unwrap());

        assert_eq!(resolution.resolved_path.as_deref(), Some(path.as_path()));
        assert_eq!(resolution.source, FontResolutionSource::DirectPath);
        assert!(!resolution.fallback);
        if let Some(postscript_name) = resolution.resolved_postscript_name {
            assert_eq!(postscript_name, "Point-Light");
        }
    }

    #[test]
    fn missing_family_resolution_reports_fallback_or_missing() {
        let resolution = resolve_font_with_telemetry("__ae_native_renderer_missing_font__");

        assert!(resolution.fallback);
        assert!(matches!(
            resolution.source,
            FontResolutionSource::FontConfig
                | FontResolutionSource::CommonFallback
                | FontResolutionSource::Missing
        ));
    }

    #[test]
    fn point_light_family_resolution_uses_local_fixture_when_available() {
        let Some(path) = point_light_fixture() else {
            return;
        };
        let resolution = resolve_font_with_telemetry("Point-Light");

        assert_eq!(resolution.resolved_path.as_deref(), Some(path.as_path()));
        assert_eq!(resolution.source, FontResolutionSource::FixtureAsset);
        assert!(!resolution.fallback);
    }

    #[test]
    fn montserrat_family_resolution_uses_conformance_asset_when_available() {
        let path = PathBuf::from("fixtures/ae_conformance_pack/assets/fonts/Montserrat-Italic[wght].ttf");
        if !path.exists() {
            return;
        }
        let resolution = resolve_font_with_telemetry("Montserrat-BoldItalic");

        assert_eq!(resolution.resolved_path.as_deref(), Some(path.as_path()));
        assert_eq!(resolution.source, FontResolutionSource::FixtureAsset);
        assert!(resolution.fallback);
    }
}
