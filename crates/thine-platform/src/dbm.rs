//! Granular Database Monitoring — Datadog DBM parity for Postgres/MySQL.
//!
//! Metric namespaces mirror Datadog integrations:
//! `postgresql.*`, `mysql.*`, plus query metrics `*.queries.*`.
//! Docs: https://docs.datadoghq.com/database_monitoring/data_collected/

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thine_common::{MetricPoint, MetricType, Sample, Tags};

use crate::models_ext::DbQuerySample;
use crate::obs18::{DbSchemaTable, ExplainPlan};
use crate::state::{DbInstance, PlatformState};

const MAX_DB_SAMPLES: usize = 2_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbMetricDef {
    pub name: String,
    pub unit: String,
    pub description: String,
    pub group: String,
    pub engines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbHostSample {
    pub timestamp_ms: i64,
    pub db: String,
    pub qps: f64,
    pub tps: f64,
    pub connections: f64,
    pub active_connections: f64,
    pub idle_connections: f64,
    pub waiting_connections: f64,
    pub avg_query_ms: f64,
    pub p95_query_ms: f64,
    pub rows_returned_per_sec: f64,
    pub rows_affected_per_sec: f64,
    pub buffer_hit_ratio: f64,
    pub disk_read_ops: f64,
    pub disk_write_ops: f64,
    pub replication_lag_ms: f64,
    pub deadlocks: f64,
    pub locks_waiting: f64,
    pub cpu_pct: f64,
    pub mem_used_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbQueryMetric {
    pub fingerprint: String,
    pub sql: String,
    pub db: String,
    pub engine: String,
    pub schema: Option<String>,
    pub calls: u64,
    pub requests_per_sec: f64,
    pub avg_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub max_latency_ms: f64,
    pub total_time_ms: f64,
    pub pct_time: f64,
    pub rows_per_sec: f64,
    pub rows_examined_per_sec: f64,
    pub shared_blks_hit: f64,
    pub shared_blks_read: f64,
    pub shared_blks_dirtied: f64,
    pub temp_blks_written: f64,
    pub block_read_time_ms: f64,
    pub block_write_time_ms: f64,
    pub apm_service: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbWaitEvent {
    pub db: String,
    pub event_type: String,
    pub event: String,
    pub waits: u64,
    pub wait_time_ms: f64,
    pub pct_wait_time: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbBlockingQuery {
    pub db: String,
    pub blocked_pid: u32,
    pub blocking_pid: u32,
    pub blocked_sql: String,
    pub blocking_sql: String,
    pub wait_event: String,
    pub duration_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbActivity {
    pub db: String,
    pub pid: u32,
    pub user: String,
    pub application: String,
    pub client_addr: String,
    pub state: String,
    pub wait_event_type: Option<String>,
    pub wait_event: Option<String>,
    pub query: String,
    pub duration_ms: f64,
}

pub fn dbm_metric_catalog() -> Vec<DbMetricDef> {
    let mut m = Vec::new();
    let mut add = |name: &str, unit: &str, desc: &str, group: &str, engines: &[&str]| {
        m.push(DbMetricDef {
            name: name.into(),
            unit: unit.into(),
            description: desc.into(),
            group: group.into(),
            engines: engines.iter().map(|s| (*s).into()).collect(),
        });
    };

    // —— Connections ——
    add("postgresql.connections", "connection", "Total connections", "connections", &["postgres"]);
    add("postgresql.active_connections", "connection", "Active backends", "connections", &["postgres"]);
    add("postgresql.idle_connections", "connection", "Idle backends", "connections", &["postgres"]);
    add("postgresql.waiting_connections", "connection", "Waiting on lock/IO", "connections", &["postgres"]);
    add("postgresql.max_connections", "connection", "max_connections setting", "connections", &["postgres"]);
    add("postgresql.percent_usage_connections", "%", "Connections / max", "connections", &["postgres"]);
    add("mysql.performance.threads_connected", "connection", "Threads connected", "connections", &["mysql"]);
    add("mysql.performance.threads_running", "connection", "Threads running", "connections", &["mysql"]);
    add("mysql.net.max_connections", "connection", "max_connections", "connections", &["mysql"]);
    add("mysql.net.connections", "connection/s", "Connection attempts/s", "connections", &["mysql"]);
    add("mysql.net.aborted_connects", "connection/s", "Aborted connects/s", "connections", &["mysql"]);

    // —— Throughput ——
    add("postgresql.queries.count", "query/s", "Queries executed/s", "throughput", &["postgres"]);
    add("postgresql.commits", "commit/s", "Transactions committed/s", "throughput", &["postgres"]);
    add("postgresql.rollbacks", "rollback/s", "Transactions rolled back/s", "throughput", &["postgres"]);
    add("postgresql.rows_returned", "row/s", "Rows returned/s", "throughput", &["postgres"]);
    add("postgresql.rows_fetched", "row/s", "Rows fetched/s", "throughput", &["postgres"]);
    add("postgresql.rows_inserted", "row/s", "Rows inserted/s", "throughput", &["postgres"]);
    add("postgresql.rows_updated", "row/s", "Rows updated/s", "throughput", &["postgres"]);
    add("postgresql.rows_deleted", "row/s", "Rows deleted/s", "throughput", &["postgres"]);
    add("mysql.performance.queries", "query/s", "Queries/s", "throughput", &["mysql"]);
    add("mysql.performance.questions", "query/s", "Questions/s", "throughput", &["mysql"]);
    add("mysql.performance.com_select", "query/s", "COM_SELECT/s", "throughput", &["mysql"]);
    add("mysql.performance.com_insert", "query/s", "COM_INSERT/s", "throughput", &["mysql"]);
    add("mysql.performance.com_update", "query/s", "COM_UPDATE/s", "throughput", &["mysql"]);
    add("mysql.performance.com_delete", "query/s", "COM_DELETE/s", "throughput", &["mysql"]);
    add("mysql.performance.slow_queries", "query/s", "Slow queries/s", "throughput", &["mysql"]);

    // —— Latency / query metrics ——
    add("postgresql.queries.time", "ms", "Avg query time", "latency", &["postgres"]);
    add("postgresql.queries.p95_time", "ms", "p95 query time", "latency", &["postgres"]);
    add("postgresql.queries.max_time", "ms", "Max query time", "latency", &["postgres"]);
    add("mysql.queries.time", "ms", "Avg query time", "latency", &["mysql"]);
    add("mysql.queries.p95_time", "ms", "p95 query time", "latency", &["mysql"]);

    // —— Buffer / cache ——
    add("postgresql.buffer_hit_ratio", "%", "Buffer cache hit ratio", "buffer", &["postgres"]);
    add("postgresql.blocks_read", "block/s", "Blocks read from disk", "buffer", &["postgres"]);
    add("postgresql.blocks_hit", "block/s", "Blocks hit in buffer", "buffer", &["postgres"]);
    add("postgresql.temp_bytes", "byte/s", "Temp file bytes/s", "buffer", &["postgres"]);
    add("mysql.innodb.buffer_pool_reads", "read/s", "InnoDB buffer pool reads", "buffer", &["mysql"]);
    add("mysql.innodb.buffer_pool_read_requests", "request/s", "Buffer pool read requests", "buffer", &["mysql"]);
    add("mysql.innodb.buffer_pool_utilization", "%", "Buffer pool utilization", "buffer", &["mysql"]);
    add("mysql.innodb.buffer_pool_bytes_data", "byte", "Buffer pool data bytes", "buffer", &["mysql"]);
    add("mysql.innodb.buffer_pool_bytes_dirty", "byte", "Buffer pool dirty bytes", "buffer", &["mysql"]);

    // —— Locks / deadlocks ——
    add("postgresql.locks.deadlocks", "deadlock/s", "Deadlocks/s", "locks", &["postgres"]);
    add("postgresql.locks.waiting", "lock", "Sessions waiting on locks", "locks", &["postgres"]);
    add("postgresql.locks.relations", "lock", "Relation locks held", "locks", &["postgres"]);
    add("mysql.innodb.deadlocks", "deadlock/s", "InnoDB deadlocks/s", "locks", &["mysql"]);
    add("mysql.innodb.row_lock_waits", "wait/s", "Row lock waits/s", "locks", &["mysql"]);
    add("mysql.innodb.row_lock_time", "ms", "Avg row lock wait time", "locks", &["mysql"]);

    // —— Replication ——
    add("postgresql.replication_delay", "ms", "Replica apply lag", "replication", &["postgres"]);
    add("postgresql.replication.slots.count", "slot", "Replication slots", "replication", &["postgres"]);
    add("mysql.replication.seconds_behind_source", "s", "Seconds behind source", "replication", &["mysql"]);
    add("mysql.replication.slaves_connected", "connection", "Replicas connected", "replication", &["mysql"]);

    // —— Vacuum / maintenance (PG) ——
    add("postgresql.vacuum.running", "vacuum", "Autovacuum workers running", "maintenance", &["postgres"]);
    add("postgresql.dead_rows", "row", "Dead tuples (approx)", "maintenance", &["postgres"]);
    add("postgresql.live_rows", "row", "Live tuples (approx)", "maintenance", &["postgres"]);
    add("postgresql.index_bloat", "%", "Estimated index bloat", "maintenance", &["postgres"]);
    add("postgresql.table_bloat", "%", "Estimated table bloat", "maintenance", &["postgres"]);

    // —— I/O & resources ——
    add("postgresql.disk_read_ops", "op/s", "Disk read ops/s", "io", &["postgres"]);
    add("postgresql.disk_write_ops", "op/s", "Disk write ops/s", "io", &["postgres"]);
    add("postgresql.cpu.user", "%", "DB CPU user", "resources", &["postgres"]);
    add("postgresql.cpu.system", "%", "DB CPU system", "resources", &["postgres"]);
    add("postgresql.mem.used_pct", "%", "DB memory used %", "resources", &["postgres"]);
    add("mysql.performance.created_tmp_tables", "table/s", "Tmp tables/s", "io", &["mysql"]);
    add("mysql.performance.created_tmp_disk_tables", "table/s", "Tmp disk tables/s", "io", &["mysql"]);
    add("mysql.performance.table_locks_waited", "wait/s", "Table lock waits/s", "locks", &["mysql"]);
    add("mysql.performance.open_files", "file", "Open files", "resources", &["mysql"]);
    add("mysql.innodb.data_reads", "read/s", "InnoDB data reads", "io", &["mysql"]);
    add("mysql.innodb.data_writes", "write/s", "InnoDB data writes", "io", &["mysql"]);
    add("mysql.innodb.os_log_fsyncs", "fsync/s", "Log fsyncs/s", "io", &["mysql"]);

    // —— Query-level (DBM) ——
    add("postgresql.queries.rows", "row/s", "Rows/s for normalized query", "query", &["postgres"]);
    add("postgresql.queries.shared_blks_hit", "block/s", "Shared buffer hits/s", "query", &["postgres"]);
    add("postgresql.queries.shared_blks_read", "block/s", "Shared buffer reads/s", "query", &["postgres"]);
    add("postgresql.queries.temp_blks_written", "block/s", "Temp blocks written/s", "query", &["postgres"]);
    add("mysql.queries.rows", "row/s", "Rows examined/s", "query", &["mysql"]);
    add("mysql.queries.errors", "error/s", "Query errors/s", "query", &["mysql"]);

    m
}

impl PlatformState {
    pub fn seed_dbm_fleet(&self) {
        // Rich Postgres primary
        self.db_instances.insert(
            "pg-primary".into(),
            rich_pg(
                "pg-primary",
                "i-db-1",
                "primary",
                240.0,
                3,
                0.0,
                72.0,
                68.0,
            ),
        );
        self.db_instances.insert(
            "pg-replica".into(),
            rich_pg(
                "pg-replica",
                "i-db-2",
                "replica",
                180.0,
                1,
                420.0,
                55.0,
                61.0,
            ),
        );
        // MySQL
        self.db_instances.insert(
            "mysql-orders".into(),
            rich_mysql("mysql-orders", "i-db-3", 310.0, 5, 180.0, 64.0, 70.0),
        );

        // Expanded explain plans
        self.upsert_explain_plan(ExplainPlan {
            query_fingerprint: "sel_orders_join_users".into(),
            db: "pg-primary".into(),
            plan: json!({
                "Node Type": "Hash Join",
                "Join Type": "Inner",
                "Total Cost": 1840.2,
                "Plan Rows": 12000,
                "Actual Rows": 11840,
                "Shared Hit Blocks": 4200,
                "Shared Read Blocks": 890,
                "Plans": [
                    {"Node Type": "Seq Scan", "Relation Name": "orders", "Total Cost": 920.0, "Filter": "(created_at > $1)"},
                    {"Node Type": "Hash", "Plans": [{"Node Type": "Seq Scan", "Relation Name": "users", "Total Cost": 410.0}]}
                ]
            }),
            total_cost: 1840.2,
            estimated_rows: 12_000,
        });
        self.upsert_explain_plan(ExplainPlan {
            query_fingerprint: "sel_orders_by_id".into(),
            db: "pg-primary".into(),
            plan: json!({
                "Node Type": "Index Scan",
                "Relation Name": "orders",
                "Index Name": "orders_pkey",
                "Startup Cost": 0.42,
                "Total Cost": 8.44,
                "Plan Rows": 1,
                "Actual Rows": 1,
                "Shared Hit Blocks": 4
            }),
            total_cost: 8.44,
            estimated_rows: 1,
        });
        self.upsert_explain_plan(ExplainPlan {
            query_fingerprint: "upd_inventory".into(),
            db: "pg-primary".into(),
            plan: json!({
                "Node Type": "ModifyTable",
                "Operation": "Update",
                "Relation Name": "inventory",
                "Total Cost": 25.1,
                "Plans": [{"Node Type": "Seq Scan", "Relation Name": "inventory", "Filter": "(qty < 10)"}]
            }),
            total_cost: 25.1,
            estimated_rows: 1,
        });
        self.upsert_explain_plan(ExplainPlan {
            query_fingerprint: "sel_payments_status".into(),
            db: "mysql-orders".into(),
            plan: json!({
                "query_block": {
                    "select_id": 1,
                    "table": {"table_name": "payments", "access_type": "ref", "key": "payments_status_idx", "rows": 2400, "filtered": 100.0}
                }
            }),
            total_cost: 120.5,
            estimated_rows: 2_400,
        });
        self.upsert_explain_plan(ExplainPlan {
            query_fingerprint: "sel_users_email".into(),
            db: "pg-primary".into(),
            plan: json!({
                "Node Type": "Index Only Scan",
                "Relation Name": "users",
                "Index Name": "users_email_uidx",
                "Total Cost": 4.2,
                "Plan Rows": 1
            }),
            total_cost: 4.2,
            estimated_rows: 1,
        });

        // Schemas with size / bloat
        for (table, rows, size_mb, dead, bloat, indexes, cols) in [
            (
                "orders",
                2_400_000u64,
                820.0,
                48_000u64,
                12.0,
                vec!["orders_pkey", "orders_user_id_idx", "orders_created_at_idx"],
                vec!["id", "user_id", "amount", "status", "created_at"],
            ),
            (
                "users",
                890_000,
                210.0,
                9_200,
                6.5,
                vec!["users_pkey", "users_email_uidx"],
                vec!["id", "email", "created_at", "plan"],
            ),
            (
                "payments",
                1_100_000,
                340.0,
                22_000,
                9.0,
                vec!["payments_pkey", "payments_order_id_idx", "payments_status_idx"],
                vec!["id", "order_id", "status", "amount", "provider"],
            ),
            (
                "inventory",
                50_000,
                18.0,
                1_200,
                4.0,
                vec!["inventory_pkey", "inventory_sku_uidx"],
                vec!["sku", "qty", "warehouse"],
            ),
            (
                "order_items",
                6_800_000,
                1_450.0,
                110_000,
                15.5,
                vec!["order_items_pkey", "order_items_order_id_idx"],
                vec!["id", "order_id", "sku", "qty", "price"],
            ),
            (
                "sessions",
                3_200_000,
                95.0,
                800_000,
                28.0,
                vec!["sessions_pkey", "sessions_user_id_idx"],
                vec!["id", "user_id", "expires_at", "payload"],
            ),
        ] {
            self.upsert_schema_table(DbSchemaTable {
                db: "pg-primary".into(),
                schema: "public".into(),
                table: table.into(),
                columns: cols.into_iter().map(String::from).collect(),
                indexes: indexes.into_iter().map(String::from).collect(),
                approx_rows: rows,
            });
            // stash size metadata into a sidecar sample later via query metrics tags
            let _ = (size_mb, dead, bloat);
        }
        self.upsert_schema_table(DbSchemaTable {
            db: "mysql-orders".into(),
            schema: "orders".into(),
            table: "payments".into(),
            columns: vec!["id".into(), "order_id".into(), "status".into(), "amount".into()],
            indexes: vec!["PRIMARY".into(), "payments_status_idx".into()],
            approx_rows: 980_000,
        });
        self.upsert_schema_table(DbSchemaTable {
            db: "mysql-orders".into(),
            schema: "orders".into(),
            table: "carts".into(),
            columns: vec!["id".into(), "user_id".into(), "updated_at".into()],
            indexes: vec!["PRIMARY".into(), "carts_user_id_idx".into()],
            approx_rows: 420_000,
        });

        // Query samples (rich)
        let samples = [
            ("SELECT o.*, u.email FROM orders o JOIN users u ON u.id = o.user_id WHERE o.created_at > $1", 240.0, 8_200u64, "pg-primary", Some("api"), Some("app"), 12000.0, 1840.0),
            ("SELECT * FROM orders WHERE id = $1", 8.0, 40_000, "pg-primary", Some("api"), Some("app"), 1.0, 8.4),
            ("SELECT sku, qty FROM inventory WHERE qty < $1", 55.0, 15_000, "pg-primary", Some("worker"), Some("inventory-job"), 420.0, 25.1),
            ("UPDATE inventory SET qty = qty - $1 WHERE sku = $2", 13.0, 22_000, "pg-primary", Some("worker"), Some("inventory-job"), 1.0, 12.0),
            ("SELECT id, email FROM users WHERE email = $1", 3.2, 55_000, "pg-primary", Some("api"), Some("app"), 1.0, 4.2),
            ("SELECT * FROM payments WHERE status = ? ORDER BY created_at DESC LIMIT ?", 62.0, 9_500, "mysql-orders", Some("checkout"), Some("checkout"), 2400.0, 120.5),
            ("INSERT INTO payments (order_id, status, amount) VALUES (?, ?, ?)", 11.0, 18_000, "mysql-orders", Some("checkout"), Some("checkout"), 1.0, 6.0),
            ("SELECT oi.*, p.sku FROM order_items oi JOIN inventory p ON p.sku = oi.sku WHERE oi.order_id = $1", 95.0, 12_000, "pg-primary", Some("api"), Some("app"), 40.0, 88.0),
            ("DELETE FROM sessions WHERE expires_at < $1", 180.0, 400, "pg-primary", Some("worker"), Some("janitor"), 50_000.0, 210.0),
            ("SELECT count(*) FROM orders WHERE status = $1 GROUP BY date_trunc('hour', created_at)", 320.0, 900, "pg-primary", Some("analytics"), Some("reporting"), 80_000.0, 2400.0),
        ];
        for (sql, dur, calls, db, svc, app, rows, cost) in samples {
            self.add_query_sample(DbQuerySample {
                sql: sql.into(),
                duration_ms: dur,
                calls,
                db: db.into(),
            });
            let _ = (svc, app, rows, cost);
        }

        // Query metrics rollup
        {
            let mut qm = self.db_query_metrics.write();
            *qm = seed_query_metrics();
        }
        {
            let mut waits = self.db_wait_events.write();
            *waits = seed_wait_events();
        }
        {
            let mut blk = self.db_blocking.write();
            *blk = seed_blocking();
        }
        {
            let mut act = self.db_activity.write();
            *act = seed_activity();
        }

        // Historical samples for charts
        let now = Utc::now().timestamp_millis();
        {
            let mut q = self.db_samples.write();
            q.clear();
            for i in 0..60 {
                let ts = now - (60 - i) as i64 * 15_000;
                for (db, base_qps, lag) in [
                    ("pg-primary", 230.0, 0.0),
                    ("pg-replica", 170.0, 350.0 + (i % 8) as f64 * 20.0),
                    ("mysql-orders", 300.0, 120.0 + (i % 5) as f64 * 15.0),
                ] {
                    let wave = ((i as f64) / 8.0).sin().abs();
                    q.push_back(DbHostSample {
                        timestamp_ms: ts,
                        db: db.into(),
                        qps: base_qps + wave * 40.0,
                        tps: base_qps * 0.35 + wave * 10.0,
                        connections: 80.0 + wave * 30.0 + (i % 7) as f64,
                        active_connections: 20.0 + wave * 15.0,
                        idle_connections: 50.0 + wave * 10.0,
                        waiting_connections: if i > 50 { 4.0 + wave * 3.0 } else { wave },
                        avg_query_ms: 12.0 + wave * 8.0 + if i > 54 { 40.0 } else { 0.0 },
                        p95_query_ms: 45.0 + wave * 30.0 + if i > 54 { 120.0 } else { 0.0 },
                        rows_returned_per_sec: 8_000.0 + wave * 2_000.0,
                        rows_affected_per_sec: 1_200.0 + wave * 400.0,
                        buffer_hit_ratio: 98.5 - wave * 1.5,
                        disk_read_ops: 120.0 + wave * 80.0,
                        disk_write_ops: 90.0 + wave * 40.0,
                        replication_lag_ms: lag,
                        deadlocks: if i % 17 == 0 { 1.0 } else { 0.0 },
                        locks_waiting: if i > 52 { 3.0 + wave } else { wave * 0.5 },
                        cpu_pct: 40.0 + wave * 25.0,
                        mem_used_pct: 60.0 + wave * 10.0,
                    });
                }
            }
            while q.len() > MAX_DB_SAMPLES {
                q.pop_front();
            }
        }

        let _ = self.emit_db_metrics(None);
    }

    pub fn list_db_samples(&self, db: Option<&str>, limit: usize) -> Vec<DbHostSample> {
        self.db_samples
            .read()
            .iter()
            .rev()
            .filter(|s| db.map(|d| s.db == d).unwrap_or(true))
            .take(limit)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    pub fn list_db_query_metrics(&self, limit: usize) -> Vec<DbQueryMetric> {
        let mut v = self.db_query_metrics.read().clone();
        v.sort_by(|a, b| {
            b.total_time_ms
                .partial_cmp(&a.total_time_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        v.truncate(limit);
        v
    }

    pub fn list_db_wait_events(&self) -> Vec<DbWaitEvent> {
        self.db_wait_events.read().clone()
    }

    pub fn list_db_blocking(&self) -> Vec<DbBlockingQuery> {
        self.db_blocking.read().clone()
    }

    pub fn list_db_activity(&self, limit: usize) -> Vec<DbActivity> {
        self.db_activity.read().iter().take(limit).cloned().collect()
    }

    pub fn dbm_metric_catalog_json(&self) -> Value {
        let cat = dbm_metric_catalog();
        json!({
            "count": cat.len(),
            "groups": {
                "connections": cat.iter().filter(|m| m.group == "connections").count(),
                "throughput": cat.iter().filter(|m| m.group == "throughput").count(),
                "latency": cat.iter().filter(|m| m.group == "latency").count(),
                "buffer": cat.iter().filter(|m| m.group == "buffer").count(),
                "locks": cat.iter().filter(|m| m.group == "locks").count(),
                "replication": cat.iter().filter(|m| m.group == "replication").count(),
                "maintenance": cat.iter().filter(|m| m.group == "maintenance").count(),
                "io": cat.iter().filter(|m| m.group == "io").count(),
                "resources": cat.iter().filter(|m| m.group == "resources").count(),
                "query": cat.iter().filter(|m| m.group == "query").count(),
            },
            "metrics": cat,
            "docs": "https://docs.datadoghq.com/database_monitoring/data_collected/"
        })
    }

    pub fn dbm_summary(&self) -> Value {
        let instances = self.list_db_instances();
        let queries = self.top_queries(200);
        let plans = self.list_explain_plans();
        let schemas = self.list_schemas(None);
        let qm = self.list_db_query_metrics(200);
        let total_qps: f64 = instances.iter().map(|d| d.qps).sum();
        let slow: u64 = instances.iter().map(|d| d.slow_queries).sum();
        let max_lag = instances
            .iter()
            .map(|d| d.replication_lag_ms)
            .fold(0.0_f64, f64::max);
        let avg_hit = if instances.is_empty() {
            0.0
        } else {
            instances.iter().map(|d| d.buffer_hit_ratio).sum::<f64>() / instances.len() as f64
        };
        json!({
            "instances": instances.len(),
            "engines": {
                "postgres": instances.iter().filter(|d| d.engine == "postgres").count(),
                "mysql": instances.iter().filter(|d| d.engine == "mysql").count(),
            },
            "total_qps": total_qps,
            "slow_queries": slow,
            "explain_plans": plans.len(),
            "tables_tracked": schemas.len(),
            "query_samples": queries.len(),
            "query_metrics": qm.len(),
            "max_replication_lag_ms": max_lag,
            "avg_buffer_hit_ratio": avg_hit,
            "metrics_catalog_count": dbm_metric_catalog().len(),
            "costliest_plan": plans.iter().map(|p| p.total_cost).fold(0.0_f64, f64::max),
            "docs": "https://docs.datadoghq.com/database_monitoring/"
        })
    }

    pub fn dbm_health(&self) -> Value {
        let mut findings = Vec::new();
        for d in self.list_db_instances() {
            if d.replication_lag_ms > 1_000.0 {
                findings.push(json!({
                    "db": d.name, "severity": "high",
                    "finding": format!("replication lag {:.0}ms", d.replication_lag_ms)
                }));
            }
            if d.buffer_hit_ratio < 95.0 {
                findings.push(json!({
                    "db": d.name, "severity": "medium",
                    "finding": format!("buffer hit ratio {:.1}%", d.buffer_hit_ratio)
                }));
            }
            if d.connections_pct > 80.0 {
                findings.push(json!({
                    "db": d.name, "severity": "high",
                    "finding": format!("connections at {:.0}% of max", d.connections_pct)
                }));
            }
            if d.deadlocks_per_sec > 0.0 {
                findings.push(json!({
                    "db": d.name, "severity": "medium",
                    "finding": format!("deadlocks {:.2}/s", d.deadlocks_per_sec)
                }));
            }
            if d.locks_waiting > 5.0 {
                findings.push(json!({
                    "db": d.name, "severity": "high",
                    "finding": format!("{:.0} sessions waiting on locks", d.locks_waiting)
                }));
            }
        }
        for q in self.list_db_query_metrics(5) {
            if q.avg_latency_ms > 100.0 {
                findings.push(json!({
                    "db": q.db, "severity": "medium",
                    "finding": format!("{} avg {:.0}ms", q.fingerprint, q.avg_latency_ms)
                }));
            }
        }
        json!({
            "instances_checked": self.list_db_instances().len(),
            "findings": findings,
            "blocking": self.list_db_blocking().len(),
            "wait_events": self.list_db_wait_events().len(),
        })
    }

    pub fn emit_db_metrics(&self, ts: Option<i64>) -> usize {
        let ts = ts.unwrap_or_else(|| Utc::now().timestamp_millis());
        let mut n = 0usize;
        for d in self.list_db_instances() {
            let mut tags = Tags::new();
            tags.insert("db".into(), d.name.clone());
            tags.insert("engine".into(), d.engine.clone());
            tags.insert("host".into(), d.host.clone());
            tags.insert("role".into(), d.role.clone());
            tags.insert("env".into(), "prod".into());

            let pairs: Vec<(&str, f64)> = if d.engine == "postgres" {
                vec![
                    ("postgresql.connections", d.connections),
                    ("postgresql.active_connections", d.active_connections),
                    ("postgresql.idle_connections", d.idle_connections),
                    ("postgresql.waiting_connections", d.waiting_connections),
                    ("postgresql.max_connections", d.max_connections),
                    ("postgresql.percent_usage_connections", d.connections_pct),
                    ("postgresql.queries.count", d.qps),
                    ("postgresql.commits", d.tps),
                    ("postgresql.rollbacks", d.rollbacks_per_sec),
                    ("postgresql.rows_returned", d.rows_returned_per_sec),
                    ("postgresql.rows_fetched", d.rows_fetched_per_sec),
                    ("postgresql.rows_inserted", d.rows_inserted_per_sec),
                    ("postgresql.rows_updated", d.rows_updated_per_sec),
                    ("postgresql.rows_deleted", d.rows_deleted_per_sec),
                    ("postgresql.queries.time", d.avg_query_ms),
                    ("postgresql.queries.p95_time", d.p95_query_ms),
                    ("postgresql.buffer_hit_ratio", d.buffer_hit_ratio),
                    ("postgresql.blocks_read", d.blocks_read_per_sec),
                    ("postgresql.blocks_hit", d.blocks_hit_per_sec),
                    ("postgresql.temp_bytes", d.temp_bytes_per_sec),
                    ("postgresql.locks.deadlocks", d.deadlocks_per_sec),
                    ("postgresql.locks.waiting", d.locks_waiting),
                    ("postgresql.replication_delay", d.replication_lag_ms),
                    ("postgresql.disk_read_ops", d.disk_read_ops),
                    ("postgresql.disk_write_ops", d.disk_write_ops),
                    ("postgresql.cpu.user", d.cpu_pct * 0.7),
                    ("postgresql.cpu.system", d.cpu_pct * 0.3),
                    ("postgresql.mem.used_pct", d.mem_used_pct),
                    ("postgresql.dead_rows", d.dead_rows as f64),
                    ("postgresql.live_rows", d.live_rows as f64),
                    ("postgresql.vacuum.running", d.autovacuum_workers as f64),
                    ("postgresql.index_bloat", d.index_bloat_pct),
                    ("postgresql.table_bloat", d.table_bloat_pct),
                ]
            } else {
                vec![
                    ("mysql.performance.threads_connected", d.connections),
                    ("mysql.performance.threads_running", d.active_connections),
                    ("mysql.net.max_connections", d.max_connections),
                    ("mysql.net.connections", d.connection_errors_per_sec + d.qps * 0.01),
                    ("mysql.net.aborted_connects", d.connection_errors_per_sec),
                    ("mysql.performance.queries", d.qps),
                    ("mysql.performance.questions", d.qps * 0.95),
                    ("mysql.performance.com_select", d.qps * 0.7),
                    ("mysql.performance.com_insert", d.rows_inserted_per_sec),
                    ("mysql.performance.com_update", d.rows_updated_per_sec),
                    ("mysql.performance.com_delete", d.rows_deleted_per_sec),
                    ("mysql.performance.slow_queries", d.slow_queries as f64 / 60.0),
                    ("mysql.queries.time", d.avg_query_ms),
                    ("mysql.queries.p95_time", d.p95_query_ms),
                    ("mysql.innodb.buffer_pool_utilization", d.buffer_pool_utilization),
                    ("mysql.innodb.buffer_pool_reads", d.blocks_read_per_sec),
                    ("mysql.innodb.buffer_pool_read_requests", d.blocks_hit_per_sec),
                    ("mysql.innodb.buffer_pool_bytes_data", d.buffer_pool_bytes),
                    ("mysql.innodb.buffer_pool_bytes_dirty", d.buffer_pool_dirty_bytes),
                    ("mysql.innodb.deadlocks", d.deadlocks_per_sec),
                    ("mysql.innodb.row_lock_waits", d.locks_waiting),
                    ("mysql.innodb.row_lock_time", d.avg_lock_wait_ms),
                    ("mysql.replication.seconds_behind_source", d.replication_lag_ms / 1000.0),
                    ("mysql.innodb.data_reads", d.disk_read_ops),
                    ("mysql.innodb.data_writes", d.disk_write_ops),
                    ("mysql.performance.created_tmp_tables", d.tmp_tables_per_sec),
                    ("mysql.performance.created_tmp_disk_tables", d.tmp_disk_tables_per_sec),
                    ("mysql.performance.open_files", d.open_files as f64),
                ]
            };

            for (name, value) in pairs {
                let unit = unit_for(name);
                if self
                    .metrics
                    .ingest_point(MetricPoint {
                        name: name.into(),
                        metric_type: MetricType::Gauge,
                        tags: tags.clone(),
                        sample: Sample {
                            timestamp_ms: ts,
                            value,
                        },
                        unit: Some(unit.into()),
                        description: None,
                    })
                    .is_ok()
                {
                    n += 1;
                }
            }
        }

        // Also emit per-query metrics for top fingerprints
        for q in self.list_db_query_metrics(25) {
            let mut tags = Tags::new();
            tags.insert("db".into(), q.db.clone());
            tags.insert("engine".into(), q.engine.clone());
            tags.insert("query_fingerprint".into(), q.fingerprint.clone());
            if let Some(svc) = &q.apm_service {
                tags.insert("service".into(), svc.clone());
            }
            let prefix = if q.engine == "mysql" {
                "mysql.queries"
            } else {
                "postgresql.queries"
            };
            for (suffix, value, unit) in [
                ("count", q.requests_per_sec, "query/s"),
                ("time", q.avg_latency_ms, "ms"),
                ("rows", q.rows_per_sec, "row/s"),
                ("shared_blks_hit", q.shared_blks_hit, "block/s"),
                ("shared_blks_read", q.shared_blks_read, "block/s"),
                ("temp_blks_written", q.temp_blks_written, "block/s"),
            ] {
                if self
                    .metrics
                    .ingest_point(MetricPoint {
                        name: format!("{prefix}.{suffix}"),
                        metric_type: MetricType::Gauge,
                        tags: tags.clone(),
                        sample: Sample {
                            timestamp_ms: ts,
                            value,
                        },
                        unit: Some(unit.into()),
                        description: None,
                    })
                    .is_ok()
                {
                    n += 1;
                }
            }
        }
        n
    }
}

fn unit_for(name: &str) -> &'static str {
    if name.contains("ratio") || name.contains("pct") || name.contains("utilization") || name.contains("cpu") || name.contains("bloat") || name.contains("percent") {
        "%"
    } else if name.contains("time") || name.contains("delay") || name.ends_with("_ms") {
        "ms"
    } else if name.contains("bytes") {
        "byte"
    } else if name.contains("connection") || name.contains("thread") {
        "connection"
    } else {
        "1"
    }
}

fn rich_pg(
    name: &str,
    host: &str,
    role: &str,
    qps: f64,
    slow: u64,
    lag: f64,
    cpu: f64,
    mem: f64,
) -> DbInstance {
    DbInstance {
        name: name.into(),
        engine: "postgres".into(),
        qps,
        slow_queries: slow,
        host: host.into(),
        role: role.into(),
        version: "15.4".into(),
        port: 5432,
        connections: 112.0,
        active_connections: 34.0,
        idle_connections: 70.0,
        waiting_connections: if lag > 0.0 { 2.0 } else { 4.0 },
        max_connections: 200.0,
        connections_pct: 56.0,
        tps: qps * 0.32,
        rollbacks_per_sec: 0.4,
        avg_query_ms: 14.5,
        p95_query_ms: 62.0,
        p99_query_ms: 140.0,
        rows_returned_per_sec: 9_200.0,
        rows_fetched_per_sec: 8_500.0,
        rows_inserted_per_sec: 420.0,
        rows_updated_per_sec: 310.0,
        rows_deleted_per_sec: 40.0,
        buffer_hit_ratio: if role == "primary" { 98.8 } else { 97.2 },
        blocks_hit_per_sec: 45_000.0,
        blocks_read_per_sec: 520.0,
        temp_bytes_per_sec: 1_200_000.0,
        deadlocks_per_sec: 0.02,
        locks_waiting: if role == "primary" { 3.0 } else { 0.0 },
        avg_lock_wait_ms: 8.0,
        replication_lag_ms: lag,
        disk_read_ops: 180.0,
        disk_write_ops: 140.0,
        cpu_pct: cpu,
        mem_used_pct: mem,
        dead_rows: 190_000,
        live_rows: 12_500_000,
        autovacuum_workers: 2,
        index_bloat_pct: 11.0,
        table_bloat_pct: 9.5,
        buffer_pool_utilization: 0.0,
        buffer_pool_bytes: 0.0,
        buffer_pool_dirty_bytes: 0.0,
        tmp_tables_per_sec: 0.0,
        tmp_disk_tables_per_sec: 0.0,
        open_files: 0,
        connection_errors_per_sec: 0.1,
        uptime_hours: 720.0,
        size_gb: 420.0,
    }
}

fn rich_mysql(name: &str, host: &str, qps: f64, slow: u64, lag: f64, cpu: f64, mem: f64) -> DbInstance {
    DbInstance {
        name: name.into(),
        engine: "mysql".into(),
        qps,
        slow_queries: slow,
        host: host.into(),
        role: "primary".into(),
        version: "8.0.35".into(),
        port: 3306,
        connections: 95.0,
        active_connections: 28.0,
        idle_connections: 60.0,
        waiting_connections: 2.0,
        max_connections: 300.0,
        connections_pct: 31.6,
        tps: qps * 0.28,
        rollbacks_per_sec: 0.2,
        avg_query_ms: 11.0,
        p95_query_ms: 48.0,
        p99_query_ms: 110.0,
        rows_returned_per_sec: 7_500.0,
        rows_fetched_per_sec: 7_200.0,
        rows_inserted_per_sec: 380.0,
        rows_updated_per_sec: 220.0,
        rows_deleted_per_sec: 30.0,
        buffer_hit_ratio: 99.1,
        blocks_hit_per_sec: 60_000.0,
        blocks_read_per_sec: 300.0,
        temp_bytes_per_sec: 800_000.0,
        deadlocks_per_sec: 0.01,
        locks_waiting: 1.0,
        avg_lock_wait_ms: 5.5,
        replication_lag_ms: lag,
        disk_read_ops: 150.0,
        disk_write_ops: 160.0,
        cpu_pct: cpu,
        mem_used_pct: mem,
        dead_rows: 0,
        live_rows: 0,
        autovacuum_workers: 0,
        index_bloat_pct: 0.0,
        table_bloat_pct: 0.0,
        buffer_pool_utilization: 82.0,
        buffer_pool_bytes: 12_000_000_000.0,
        buffer_pool_dirty_bytes: 400_000_000.0,
        tmp_tables_per_sec: 12.0,
        tmp_disk_tables_per_sec: 1.5,
        open_files: 860,
        connection_errors_per_sec: 0.05,
        uptime_hours: 510.0,
        size_gb: 180.0,
    }
}

fn seed_query_metrics() -> Vec<DbQueryMetric> {
    vec![
        qm("sel_orders_join_users", "SELECT o.*, u.email FROM orders o JOIN users u ON u.id = o.user_id WHERE o.created_at > ?", "pg-primary", "postgres", 8_200, 240.0, 520.0, 890.0, 0.32, 4_200.0, 12_000.0, 890.0, 4200.0, 12.0, Some("api")),
        qm("sel_orders_by_id", "SELECT * FROM orders WHERE id = ?", "pg-primary", "postgres", 40_000, 8.0, 18.0, 55.0, 0.11, 900.0, 1.0, 0.2, 38_000.0, 0.0, Some("api")),
        qm("sel_inventory_low", "SELECT sku, qty FROM inventory WHERE qty < ?", "pg-primary", "postgres", 15_000, 55.0, 120.0, 210.0, 0.14, 2_100.0, 420.0, 180.0, 2_800.0, 40.0, Some("worker")),
        qm("upd_inventory", "UPDATE inventory SET qty = qty - ? WHERE sku = ?", "pg-primary", "postgres", 22_000, 13.0, 40.0, 95.0, 0.09, 800.0, 1.0, 0.5, 20_000.0, 0.0, Some("worker")),
        qm("sel_users_email", "SELECT id, email FROM users WHERE email = ?", "pg-primary", "postgres", 55_000, 3.2, 8.0, 22.0, 0.06, 400.0, 1.0, 0.0, 55_000.0, 0.0, Some("api")),
        qm("sel_payments_status", "SELECT * FROM payments WHERE status = ? ORDER BY created_at DESC LIMIT ?", "mysql-orders", "mysql", 9_500, 62.0, 140.0, 300.0, 0.18, 3_500.0, 2_400.0, 600.0, 8_000.0, 20.0, Some("checkout")),
        qm("ins_payments", "INSERT INTO payments (order_id, status, amount) VALUES (?, ?, ?)", "mysql-orders", "mysql", 18_000, 11.0, 28.0, 70.0, 0.07, 600.0, 1.0, 0.0, 0.0, 0.0, Some("checkout")),
        qm("sel_order_items", "SELECT oi.*, p.sku FROM order_items oi JOIN inventory p ON p.sku = oi.sku WHERE oi.order_id = ?", "pg-primary", "postgres", 12_000, 95.0, 210.0, 400.0, 0.16, 2_800.0, 40.0, 120.0, 9_000.0, 80.0, Some("api")),
        qm("del_sessions", "DELETE FROM sessions WHERE expires_at < ?", "pg-primary", "postgres", 400, 180.0, 400.0, 900.0, 0.05, 900.0, 50_000.0, 2_000.0, 100.0, 500.0, Some("worker")),
        qm("agg_orders_hourly", "SELECT count(*) FROM orders WHERE status = ? GROUP BY date_trunc('hour', created_at)", "pg-primary", "postgres", 900, 320.0, 700.0, 1_200.0, 0.12, 4_000.0, 80_000.0, 40_000.0, 200.0, 8_000.0, Some("analytics")),
    ]
}

fn qm(
    fp: &str,
    sql: &str,
    db: &str,
    engine: &str,
    calls: u64,
    avg: f64,
    p95: f64,
    max: f64,
    pct: f64,
    rows: f64,
    rows_ex: f64,
    blks_read: f64,
    blks_hit: f64,
    temp: f64,
    svc: Option<&str>,
) -> DbQueryMetric {
    DbQueryMetric {
        fingerprint: fp.into(),
        sql: sql.into(),
        db: db.into(),
        engine: engine.into(),
        schema: Some("public".into()),
        calls,
        requests_per_sec: calls as f64 / 3600.0,
        avg_latency_ms: avg,
        p95_latency_ms: p95,
        max_latency_ms: max,
        total_time_ms: calls as f64 * avg,
        pct_time: pct,
        rows_per_sec: rows / 60.0,
        rows_examined_per_sec: rows_ex / 60.0,
        shared_blks_hit: blks_hit,
        shared_blks_read: blks_read,
        shared_blks_dirtied: blks_read * 0.1,
        temp_blks_written: temp,
        block_read_time_ms: blks_read * 0.05,
        block_write_time_ms: temp * 0.02,
        apm_service: svc.map(|s| s.into()),
    }
}

fn seed_wait_events() -> Vec<DbWaitEvent> {
    vec![
        DbWaitEvent { db: "pg-primary".into(), event_type: "Lock".into(), event: "transactionid".into(), waits: 420, wait_time_ms: 18_000.0, pct_wait_time: 28.0 },
        DbWaitEvent { db: "pg-primary".into(), event_type: "IO".into(), event: "DataFileRead".into(), waits: 1_200, wait_time_ms: 12_500.0, pct_wait_time: 19.0 },
        DbWaitEvent { db: "pg-primary".into(), event_type: "LWLock".into(), event: "BufferContent".into(), waits: 800, wait_time_ms: 6_200.0, pct_wait_time: 9.5 },
        DbWaitEvent { db: "pg-primary".into(), event_type: "Client".into(), event: "ClientRead".into(), waits: 3_400, wait_time_ms: 22_000.0, pct_wait_time: 34.0 },
        DbWaitEvent { db: "mysql-orders".into(), event_type: "wait/io/table".into(), event: "sql/handler".into(), waits: 900, wait_time_ms: 8_100.0, pct_wait_time: 22.0 },
        DbWaitEvent { db: "mysql-orders".into(), event_type: "wait/synch/mutex".into(), event: "innodb/buf_pool_mutex".into(), waits: 600, wait_time_ms: 4_400.0, pct_wait_time: 12.0 },
    ]
}

fn seed_blocking() -> Vec<DbBlockingQuery> {
    vec![
        DbBlockingQuery {
            db: "pg-primary".into(),
            blocked_pid: 1842,
            blocking_pid: 1720,
            blocked_sql: "UPDATE inventory SET qty = qty - $1 WHERE sku = $2".into(),
            blocking_sql: "SELECT sku, qty FROM inventory WHERE qty < $1 FOR UPDATE".into(),
            wait_event: "Lock: transactionid".into(),
            duration_ms: 1_840.0,
        },
    ]
}

fn seed_activity() -> Vec<DbActivity> {
    vec![
        DbActivity { db: "pg-primary".into(), pid: 1842, user: "app".into(), application: "api".into(), client_addr: "10.0.1.12".into(), state: "active".into(), wait_event_type: Some("Lock".into()), wait_event: Some("transactionid".into()), query: "UPDATE inventory SET qty = qty - $1 WHERE sku = $2".into(), duration_ms: 1840.0 },
        DbActivity { db: "pg-primary".into(), pid: 1901, user: "app".into(), application: "api".into(), client_addr: "10.0.1.14".into(), state: "active".into(), wait_event_type: None, wait_event: None, query: "SELECT * FROM orders WHERE id = $1".into(), duration_ms: 6.2 },
        DbActivity { db: "pg-primary".into(), pid: 1888, user: "analytics".into(), application: "reporting".into(), client_addr: "10.0.2.8".into(), state: "active".into(), wait_event_type: Some("IO".into()), wait_event: Some("DataFileRead".into()), query: "SELECT count(*) FROM orders WHERE status = $1 GROUP BY date_trunc('hour', created_at)".into(), duration_ms: 920.0 },
        DbActivity { db: "mysql-orders".into(), pid: 441, user: "checkout".into(), application: "checkout".into(), client_addr: "10.0.3.4".into(), state: "executing".into(), wait_event_type: Some("wait/io/table".into()), wait_event: Some("sql/handler".into()), query: "SELECT * FROM payments WHERE status = ? ORDER BY created_at DESC LIMIT ?".into(), duration_ms: 88.0 },
        DbActivity { db: "pg-replica".into(), pid: 502, user: "readonly".into(), application: "api".into(), client_addr: "10.0.1.20".into(), state: "idle in transaction".into(), wait_event_type: Some("Client".into()), wait_event: Some("ClientRead".into()), query: "SELECT id, email FROM users WHERE email = $1".into(), duration_ms: 12.0 },
    ]
}
