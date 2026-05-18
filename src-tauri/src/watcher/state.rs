use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

const BLUEPRINT_CORRELATION_WINDOW_SEC: f64 = 5.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionEntry {
    pub debug_name: String,
    pub generator: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveMission {
    pub guid: String,
    pub debug_name: String,
    pub generator: String,
    pub start_ts: f64,
}

#[derive(Debug, Clone)]
pub struct MissionLifecycleEvent {
    pub trigger: String, // "accept" or "complete"
    pub guid: String,
    pub debug_name: String,
    pub ts: f64,
}

pub struct WatcherStateInner {
    pub guid_map: HashMap<String, MissionEntry>,
    pub active: HashMap<String, ActiveMission>,
    pub recent_lifecycle: VecDeque<MissionLifecycleEvent>,
}

impl WatcherStateInner {
    pub fn new() -> Self {
        Self {
            guid_map: HashMap::new(),
            active: HashMap::new(),
            recent_lifecycle: VecDeque::with_capacity(32),
        }
    }

    pub fn reset(&mut self) {
        self.guid_map.clear();
        self.active.clear();
        self.recent_lifecycle.clear();
    }

    pub fn record_marker(&mut self, guid: &str, generator: &str, contract: &str) {
        self.guid_map.entry(guid.to_string()).or_insert_with(|| MissionEntry {
            debug_name: contract.to_string(),
            generator: generator.to_string(),
        });
    }

    pub fn record_accepted(&mut self, guid: &str, ts: f64) -> Option<ActiveMission> {
        let entry = self.guid_map.get(guid)?;
        let active = ActiveMission {
            guid: guid.to_string(),
            debug_name: entry.debug_name.clone(),
            generator: entry.generator.clone(),
            start_ts: ts,
        };
        self.active.insert(guid.to_string(), active.clone());
        self.push_lifecycle(MissionLifecycleEvent {
            trigger: "accept".to_string(),
            guid: guid.to_string(),
            debug_name: entry.debug_name.clone(),
            ts,
        });
        Some(active)
    }

    pub fn record_end(&mut self, guid: &str, completion: &str, ts: f64) -> Option<ActiveMission> {
        let active = self.active.remove(guid);
        if completion == "Complete" {
            let debug_name = active
                .as_ref()
                .map(|a| a.debug_name.clone())
                .or_else(|| self.guid_map.get(guid).map(|e| e.debug_name.clone()))
                .unwrap_or_else(|| "?".to_string());
            self.push_lifecycle(MissionLifecycleEvent {
                trigger: "complete".to_string(),
                guid: guid.to_string(),
                debug_name,
                ts,
            });
        }
        active
    }

    pub fn correlate_blueprint(&self, ts: f64) -> Option<&MissionLifecycleEvent> {
        let mut best: Option<&MissionLifecycleEvent> = None;
        let mut best_delta = BLUEPRINT_CORRELATION_WINDOW_SEC + 1.0;
        for e in &self.recent_lifecycle {
            let delta = ts - e.ts;
            if (0.0..=BLUEPRINT_CORRELATION_WINDOW_SEC).contains(&delta) && delta < best_delta {
                best = Some(e);
                best_delta = delta;
            }
        }
        best
    }

    pub fn snapshot_active(&self) -> Vec<ActiveMission> {
        self.active.values().cloned().collect()
    }

    fn push_lifecycle(&mut self, event: MissionLifecycleEvent) {
        if self.recent_lifecycle.len() >= 32 {
            self.recent_lifecycle.pop_front();
        }
        self.recent_lifecycle.push_back(event);
    }
}

#[derive(Clone)]
pub struct WatcherState {
    pub inner: Arc<Mutex<WatcherStateInner>>,
}

impl WatcherState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(WatcherStateInner::new())),
        }
    }
}
