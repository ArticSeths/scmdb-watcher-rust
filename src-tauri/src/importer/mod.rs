use serde::Serialize;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use crate::watcher::parser::parse_log_timestamp;
use crate::watcher::patterns::{RE_ACCEPTED, RE_BLUEPRINT, RE_END_MISSION, RE_MARKER};
use crate::watcher::state::WatcherStateInner;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedMission {
    pub guid: String,
    pub debug_name: String,
    pub generator: String,
    pub start_ts: f64,
    pub end_ts: f64,
    pub duration_sec: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedBlueprint {
    pub product_name: String,
    pub ts: f64,
    pub mission_guid: String,
    pub mission_debug_name: String,
    pub mission_trigger: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub missions: Vec<ExportedMission>,
    pub blueprints: Vec<ExportedBlueprint>,
    pub source_logs: Vec<String>,
    pub duplicates_merged: usize,
}

pub fn scan_file_for_export(path: &Path) -> (Vec<ExportedMission>, Vec<ExportedBlueprint>) {
    let mut state = WatcherStateInner::new();
    let mut missions = Vec::new();
    let mut blueprints = Vec::new();

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            // Try reading as bytes with lossy conversion
            match std::fs::read(path) {
                Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                Err(_) => {
                    warn!("Skipping {}: {}", path.display(), e);
                    return (missions, blueprints);
                }
            }
        }
    };

    for line in content.lines() {
        if line.is_empty() {
            continue;
        }

        let ts = parse_log_timestamp(line).unwrap_or(0.0);

        if let Some(caps) = RE_MARKER.captures(line) {
            let guid = caps.get(1).unwrap().as_str();
            let generator = caps.get(2).unwrap().as_str();
            let contract = caps.get(3).unwrap().as_str();
            state.record_marker(guid, generator, contract);
        } else if let Some(caps) = RE_ACCEPTED.captures(line) {
            let guid = caps.get(1).unwrap().as_str();
            state.record_accepted(guid, ts);
        } else if let Some(caps) = RE_END_MISSION.captures(line) {
            let guid = caps.get(1).unwrap().as_str();
            let completion = caps.get(2).unwrap().as_str();
            let reason = caps.get(3).unwrap().as_str();
            let active = state.record_end(guid, completion, ts);
            if completion == "Complete" {
                if let Some(active) = active {
                    missions.push(ExportedMission {
                        guid: guid.to_string(),
                        debug_name: active.debug_name,
                        generator: active.generator,
                        start_ts: active.start_ts,
                        end_ts: ts,
                        duration_sec: ((ts - active.start_ts) * 1000.0).round() / 1000.0,
                        reason: reason.to_string(),
                    });
                }
            }
        } else if let Some(caps) = RE_BLUEPRINT.captures(line) {
            let product_name = caps.get(1).unwrap().as_str().trim();
            if let Some(corr) = state.correlate_blueprint(ts) {
                blueprints.push(ExportedBlueprint {
                    product_name: product_name.to_string(),
                    ts,
                    mission_guid: corr.guid.clone(),
                    mission_debug_name: corr.debug_name.clone(),
                    mission_trigger: corr.trigger.clone(),
                });
            }
        }
    }

    (missions, blueprints)
}

pub fn run_import(
    logbackups_dir: &Path,
    include_current: bool,
    current_log_path: Option<&Path>,
) -> Result<ImportResult, String> {
    if !logbackups_dir.is_dir() {
        return Err(format!("Directory not found: {}", logbackups_dir.display()));
    }

    let pattern = logbackups_dir.join("Game Build(*).log");
    let pattern_str = pattern.to_string_lossy().to_string();

    let mut files: Vec<PathBuf> = glob::glob(&pattern_str)
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    files.sort();

    if include_current {
        if let Some(log_path) = current_log_path {
            if log_path.is_file() {
                files.push(log_path.to_path_buf());
            }
        }
    }

    if files.is_empty() {
        return Err(format!(
            "No log files found in {}",
            logbackups_dir.display()
        ));
    }

    info!(
        "Scanning {} log file(s) from: {}",
        files.len(),
        logbackups_dir.display()
    );

    let mut all_missions = Vec::new();
    let mut all_blueprints = Vec::new();
    let mut source_logs = Vec::new();

    for path in &files {
        let (m, b) = scan_file_for_export(path);
        if let Some(name) = path.file_name() {
            source_logs.push(name.to_string_lossy().to_string());
        }
        all_missions.extend(m);
        all_blueprints.extend(b);
    }

    // Dedup by GUID
    let mut seen = std::collections::HashSet::new();
    let original_count = all_missions.len();
    all_missions.retain(|m| seen.insert(m.guid.clone()));
    let duplicates_merged = original_count - all_missions.len();

    Ok(ImportResult {
        missions: all_missions,
        blueprints: all_blueprints,
        source_logs,
        duplicates_merged,
    })
}
