use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader};
use tokio::sync::watch;
use tracing::{info, warn};

use super::bus::{EventBus, WatcherEvent};
use super::parser::process_line;
use super::state::WatcherState;

const TAIL_POLL_INTERVAL_MS: u64 = 200;

pub struct LogTailer {
    path: PathBuf,
    state: WatcherState,
    bus: EventBus,
    stop_rx: watch::Receiver<bool>,
}

impl LogTailer {
    pub fn new(
        path: PathBuf,
        state: WatcherState,
        bus: EventBus,
        stop_rx: watch::Receiver<bool>,
    ) -> Self {
        Self {
            path,
            state,
            bus,
            stop_rx,
        }
    }

    pub async fn run(self) {
        let mut last_size: u64 = 0;
        let mut file_pos: u64 = 0;
        let mut first_open = true;

        loop {
            if *self.stop_rx.borrow() {
                info!("Tailer stop signal received");
                break;
            }

            let metadata = match tokio::fs::metadata(&self.path).await {
                Ok(m) => m,
                Err(_) => {
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    continue;
                }
            };

            let current_size = metadata.len();

            if current_size < last_size {
                info!("Log rotation detected — resetting state");
                self.state.inner.lock().await.reset();
                self.bus.broadcast(WatcherEvent::SessionReset);
                file_pos = 0;
            }

            if current_size > file_pos {
                match File::open(&self.path).await {
                    Ok(f) => {
                        if first_open {
                            info!(
                                "Opened {} (size={} bytes) — replaying for active missions",
                                self.path.display(),
                                current_size
                            );
                            first_open = false;
                        }

                        let mut reader = BufReader::new(f);

                        // Seek to last known position
                        if file_pos > 0 {
                            if let Err(e) = reader.seek(std::io::SeekFrom::Start(file_pos)).await {
                                warn!("Seek error: {}", e);
                                continue;
                            }
                        }

                        // Read new lines
                        let mut line = String::new();
                        loop {
                            line.clear();
                            match reader.read_line(&mut line).await {
                                Ok(0) => break,
                                Ok(n) => {
                                    file_pos += n as u64;
                                    let trimmed = line.trim_end();
                                    if !trimmed.is_empty() {
                                        process_line(trimmed, &self.state, &self.bus).await;
                                    }
                                }
                                Err(e) => {
                                    warn!("Read error: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to open log file: {}", e);
                    }
                }
            }

            last_size = current_size;
            tokio::time::sleep(tokio::time::Duration::from_millis(TAIL_POLL_INTERVAL_MS)).await;
        }
    }
}
