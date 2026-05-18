use once_cell::sync::Lazy;
use regex::Regex;

use super::bus::{EventBus, WatcherEvent};
use super::state::WatcherState;

static RE_TIMESTAMP: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^<([0-9T:\-.Z]+)>").unwrap());

static RE_MARKER: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"CreateMarker.*missionId \[([^\]]+)\].*generator name \[([^\]]+)\].*contract \[([^\]]+)\]").unwrap()
});

static RE_ACCEPTED: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"Added notification "Contract Accepted:.*?MissionId: \[([^\]]+)\]"#).unwrap()
});

static RE_END_MISSION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"<EndMission>.*MissionId\[([^\]]+)\].*CompletionType\[(\w+)\].*Reason\[([^\]]+)\]")
        .unwrap()
});

static RE_BLUEPRINT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"Added notification "Received Blueprint: ([^:]+):"#).unwrap()
});

pub fn parse_log_timestamp(line: &str) -> Option<f64> {
    let caps = RE_TIMESTAMP.captures(line)?;
    let raw = caps.get(1)?.as_str().replace('Z', "+00:00");
    let dt = chrono::DateTime::parse_from_rfc3339(&raw).ok()?;
    Some(dt.timestamp() as f64 + dt.timestamp_subsec_nanos() as f64 / 1_000_000_000.0)
}

pub async fn process_line(line: &str, state: &WatcherState, bus: &EventBus) {
    let ts = parse_log_timestamp(line)
        .unwrap_or_else(|| chrono::Utc::now().timestamp() as f64);

    if let Some(caps) = RE_MARKER.captures(line) {
        let guid = caps.get(1).unwrap().as_str();
        let generator = caps.get(2).unwrap().as_str();
        let contract = caps.get(3).unwrap().as_str();
        let mut s = state.inner.lock().await;
        s.record_marker(guid, generator, contract);
        return;
    }

    if let Some(caps) = RE_ACCEPTED.captures(line) {
        let guid = caps.get(1).unwrap().as_str();
        let mut s = state.inner.lock().await;
        if let Some(active) = s.record_accepted(guid, ts) {
            bus.broadcast(WatcherEvent::MissionStart {
                guid: active.guid,
                debug_name: active.debug_name,
                generator: active.generator,
                start_ts: active.start_ts,
            });
        }
        return;
    }

    if let Some(caps) = RE_END_MISSION.captures(line) {
        let guid = caps.get(1).unwrap().as_str();
        let completion = caps.get(2).unwrap().as_str();
        let reason = caps.get(3).unwrap().as_str();
        let mut s = state.inner.lock().await;
        let active = s.record_end(guid, completion, ts);
        let entry = s.guid_map.get(guid);
        let debug_name = active
            .as_ref()
            .map(|a| a.debug_name.clone())
            .or_else(|| entry.map(|e| e.debug_name.clone()));
        let generator = active
            .as_ref()
            .map(|a| a.generator.clone())
            .or_else(|| entry.map(|e| e.generator.clone()));

        let event = if completion == "Complete" {
            WatcherEvent::MissionComplete {
                guid: guid.to_string(),
                debug_name,
                generator,
                completion: completion.to_string(),
                reason: reason.to_string(),
                end_ts: ts,
            }
        } else {
            WatcherEvent::MissionEnded {
                guid: guid.to_string(),
                debug_name,
                generator,
                completion: completion.to_string(),
                reason: reason.to_string(),
                end_ts: ts,
            }
        };
        bus.broadcast(event);
        return;
    }

    if let Some(caps) = RE_BLUEPRINT.captures(line) {
        let product_name = caps.get(1).unwrap().as_str().trim().to_string();
        let s = state.inner.lock().await;
        let corr = s.correlate_blueprint(ts);
        let event = WatcherEvent::BlueprintReceived {
            product_name,
            mission_guid: corr.map(|c| c.guid.clone()),
            mission_debug_name: corr.map(|c| c.debug_name.clone()),
            mission_trigger: corr.map(|c| c.trigger.clone()),
            ts,
        };
        bus.broadcast(event);
    }
}
