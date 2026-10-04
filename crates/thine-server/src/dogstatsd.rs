//! Native DogStatsD UDP listener (Datadog Agent default :8125).

use std::net::SocketAddr;
use std::sync::Arc;
use thine_ingest::IngestService;
use tokio::net::UdpSocket;
use tracing::{info, warn};

pub async fn run_dogstatsd(ingest: Arc<IngestService>, port: u16) {
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let sock = match UdpSocket::bind(addr).await {
        Ok(s) => s,
        Err(e) => {
            warn!(%addr, error = %e, "DogStatsD UDP bind failed — native UDP path disabled");
            return;
        }
    };
    info!(%addr, "DogStatsD UDP listening (native Agent path)");
    let mut buf = vec![0u8; 65535];
    loop {
        match sock.recv_from(&mut buf).await {
            Ok((n, _peer)) => {
                let text = String::from_utf8_lossy(&buf[..n]);
                let _ = ingest.ingest_statsd_lines(&text);
            }
            Err(e) => {
                warn!(error = %e, "DogStatsD recv error");
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }
    }
}
