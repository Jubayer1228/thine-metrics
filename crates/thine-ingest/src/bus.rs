//! Kafka-like in-process intake bus: intake publishes → durable workers consume.
//! Hot query stores are written synchronously on the intake thread (RTDB-style).

use std::sync::Arc;
use thine_common::{IngestPath, IntakeEvent, IntakeLog, IntakeSpan, MetricPoint};
use thine_storage::{HuskyStore, RtdbEngine, TraceStore};
use tokio::sync::mpsc;
use tracing::{info, warn};

const CHAN: usize = 4096;

#[derive(Debug)]
pub enum BusMessage {
    Metrics {
        path: IngestPath,
        points: Vec<MetricPoint>,
    },
    Traces {
        path: IngestPath,
        spans: Vec<IntakeSpan>,
    },
    Logs {
        path: IngestPath,
        logs: Vec<IntakeLog>,
    },
    Events {
        path: IngestPath,
        events: Vec<IntakeEvent>,
    },
}

#[derive(Clone)]
pub struct IntakeBus {
    tx: mpsc::Sender<BusMessage>,
}

impl IntakeBus {
    pub fn publish(&self, msg: BusMessage) {
        if let Err(e) = self.tx.try_send(msg) {
            warn!(error = %e, "intake bus backpressured — dropping batch");
        }
    }
}

pub struct DurableBundle {
    pub rtdb: Arc<RtdbEngine>,
    pub husky: Arc<HuskyStore>,
    pub traces: Arc<TraceStore>,
}

/// Spawn durable consumers (Datadog Kafka → product stores).
pub fn spawn_intake_bus(durable: DurableBundle) -> IntakeBus {
    let (tx, mut rx) = mpsc::channel::<BusMessage>(CHAN);
    let rtdb = durable.rtdb;
    let husky = durable.husky;
    let traces = durable.traces;

    tokio::spawn(async move {
        info!("intake bus workers started (RTDB / TraceStore / Husky)");
        while let Some(msg) = rx.recv().await {
            match msg {
                BusMessage::Metrics { points, .. } => {
                    rtdb.write_points(&points);
                    rtdb.compact_hint();
                }
                BusMessage::Traces { spans, .. } => {
                    let _ = traces.ingest(spans);
                }
                BusMessage::Logs { logs, .. } => {
                    husky.ingest_logs(logs);
                }
                BusMessage::Events { events, .. } => {
                    husky.ingest_events(events);
                }
            }
        }
        husky.flush();
        info!("intake bus workers stopped");
    });

    IntakeBus { tx }
}
