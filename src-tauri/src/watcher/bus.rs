use serde::Serialize;
use tokio::sync::broadcast;

const CHANNEL_CAPACITY: usize = 256;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WatcherEvent {
    MissionStart {
        guid: String,
        #[serde(rename = "debugName")]
        debug_name: String,
        generator: String,
        #[serde(rename = "startTs")]
        start_ts: f64,
    },
    MissionComplete {
        guid: String,
        #[serde(rename = "debugName")]
        debug_name: Option<String>,
        generator: Option<String>,
        completion: String,
        reason: String,
        #[serde(rename = "endTs")]
        end_ts: f64,
    },
    MissionEnded {
        guid: String,
        #[serde(rename = "debugName")]
        debug_name: Option<String>,
        generator: Option<String>,
        completion: String,
        reason: String,
        #[serde(rename = "endTs")]
        end_ts: f64,
    },
    BlueprintReceived {
        #[serde(rename = "productName")]
        product_name: String,
        #[serde(rename = "missionGuid")]
        mission_guid: Option<String>,
        #[serde(rename = "missionDebugName")]
        mission_debug_name: Option<String>,
        #[serde(rename = "missionTrigger")]
        mission_trigger: Option<String>,
        ts: f64,
    },
    SessionReset,
    StateSnapshot {
        active: Vec<super::state::ActiveMission>,
    },
}

#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<WatcherEvent>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        Self { tx }
    }

    pub fn broadcast(&self, event: WatcherEvent) {
        // Ignore error (no subscribers)
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WatcherEvent> {
        self.tx.subscribe()
    }
}
