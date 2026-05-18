use serde::Serialize;
use tokio::sync::broadcast;

const CHANNEL_CAPACITY: usize = 256;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WatcherEvent {
    MissionStart {
        guid: String,
        debug_name: String,
        generator: String,
        start_ts: f64,
    },
    MissionComplete {
        guid: String,
        debug_name: Option<String>,
        generator: Option<String>,
        completion: String,
        reason: String,
        end_ts: f64,
    },
    MissionEnded {
        guid: String,
        debug_name: Option<String>,
        generator: Option<String>,
        completion: String,
        reason: String,
        end_ts: f64,
    },
    BlueprintReceived {
        product_name: String,
        mission_guid: Option<String>,
        mission_debug_name: Option<String>,
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
