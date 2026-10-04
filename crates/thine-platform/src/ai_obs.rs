//! AI-native observability — LangSmith / OTel GenAI / deep-agent eval parity.
//!
//! Rust-first ingest path: DashMap projects, ring-buffered runs, lock-scoped
//! samples. Designed for higher ingest throughput than SaaS LLMObs clients.

use crate::state::PlatformState;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thine_common::{MetricPoint, MetricType, Sample, Tags};
use uuid::Uuid;

const MAX_RUNS: usize = 20_000;
const MAX_SAMPLES: usize = 2_000;
const MAX_FEEDBACK: usize = 5_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiMetricDef {
    pub name: String,
    pub unit: String,
    pub description: String,
    pub group: String,
    pub modalities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProject {
    pub id: String,
    pub name: String,
    pub description: String,
    pub region: String,
    pub modality: String,
    pub provider: String,
    pub model: String,
    pub runs_24h: u64,
    pub error_rate: f64,
    pub avg_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub tokens_per_sec: f64,
    pub cost_usd_24h: f64,
    pub pass_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiRun {
    pub id: String,
    pub trace_id: String,
    pub parent_run_id: Option<String>,
    pub project: String,
    pub name: String,
    pub run_type: String,
    pub status: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub latency_ms: f64,
    pub ttft_ms: Option<f64>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub cost_usd: f64,
    pub error: Option<String>,
    pub tool_name: Option<String>,
    pub modality: String,
    pub tags: Vec<String>,
    pub input_preview: String,
    pub output_preview: String,
    pub trajectory: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiFeedback {
    pub id: String,
    pub run_id: String,
    pub key: String,
    pub score: f64,
    pub comment: Option<String>,
    pub source: String,
    pub timestamp_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiDataset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub example_count: u64,
    pub modality: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiExample {
    pub id: String,
    pub dataset_id: String,
    pub inputs: serde_json::Value,
    pub outputs: serde_json::Value,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiExperiment {
    pub id: String,
    pub name: String,
    pub dataset_id: String,
    pub project: String,
    pub status: String,
    pub trials_per_task: u32,
    pub pass_at_k: f64,
    pub pass_hat_k: f64,
    pub avg_score: f64,
    pub started_ms: i64,
    pub finished_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiEvalResult {
    pub id: String,
    pub experiment_id: String,
    pub example_id: String,
    pub trial: u32,
    pub pattern: String,
    pub grader: String,
    pub score: f64,
    pub passed: bool,
    pub latency_ms: f64,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiGrader {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiHostSample {
    pub timestamp_ms: i64,
    pub project: String,
    pub modality: String,
    pub requests: f64,
    pub errors: f64,
    pub latency_ms: f64,
    pub ttft_ms: f64,
    pub tokens_in: f64,
    pub tokens_out: f64,
    pub cost_usd: f64,
    pub pass_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiCreateRun {
    pub name: String,
    pub run_type: String,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub parent_run_id: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub modality: Option<String>,
    #[serde(default)]
    pub input_tokens: Option<u64>,
    #[serde(default)]
    pub output_tokens: Option<u64>,
    #[serde(default)]
    pub latency_ms: Option<f64>,
    #[serde(default)]
    pub ttft_ms: Option<f64>,
    #[serde(default)]
    pub cost_usd: Option<f64>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub input_preview: Option<String>,
    #[serde(default)]
    pub output_preview: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub trajectory: Option<Vec<String>>,
}

pub fn ai_metric_catalog() -> Vec<AiMetricDef> {
    let mut m = Vec::new();
    let mut add = |name: &str, unit: &str, desc: &str, group: &str, mods: &[&str]| {
        m.push(AiMetricDef {
            name: name.into(),
            unit: unit.into(),
            description: desc.into(),
            group: group.into(),
            modalities: mods.iter().map(|s| (*s).into()).collect(),
        });
    };

    // —— LLM (OTel gen_ai + LangSmith / Datadog ml_obs parity) ——
    add("ai.llm.requests", "req/s", "LLM requests/s", "llm", &["llm"]);
    add("ai.llm.errors", "error/s", "LLM errors/s", "llm", &["llm"]);
    add("ai.llm.duration", "ms", "LLM span duration", "llm", &["llm"]);
    add("ai.llm.ttft", "ms", "Time to first token", "llm", &["llm"]);
    add("ai.llm.time_per_output_token", "ms", "Decode time per token", "llm", &["llm"]);
    add("ai.llm.input.tokens", "token", "Input tokens", "llm", &["llm"]);
    add("ai.llm.output.tokens", "token", "Output tokens", "llm", &["llm"]);
    add("ai.llm.reasoning.tokens", "token", "Reasoning tokens", "llm", &["llm"]);
    add("ai.llm.total.tokens", "token", "Total tokens", "llm", &["llm"]);
    add("ai.llm.cache_read.tokens", "token", "Prompt cache read tokens", "llm", &["llm"]);
    add("ai.llm.cache_write.tokens", "token", "Prompt cache write tokens", "llm", &["llm"]);
    add("ai.llm.input.cost", "usd", "Estimated input cost", "cost", &["llm"]);
    add("ai.llm.output.cost", "usd", "Estimated output cost", "cost", &["llm"]);
    add("ai.llm.total.cost", "usd", "Estimated total cost", "cost", &["llm"]);
    add("ai.llm.tokens_per_sec", "token/s", "Generation throughput", "llm", &["llm"]);
    add("ai.llm.context_utilization", "%", "Context window used", "llm", &["llm"]);

    // —— Embeddings / RAG ——
    add("ai.embedding.requests", "req/s", "Embedding requests/s", "embedding", &["embedding"]);
    add("ai.embedding.input.tokens", "token", "Embedding input tokens", "embedding", &["embedding"]);
    add("ai.embedding.duration", "ms", "Embedding latency", "embedding", &["embedding"]);
    add("ai.embedding.cost", "usd", "Embedding cost", "cost", &["embedding"]);
    add("ai.retrieval.duration", "ms", "Vector retrieval latency", "rag", &["embedding"]);
    add("ai.retrieval.docs", "doc", "Docs returned per query", "rag", &["embedding"]);
    add("ai.retrieval.score", "1", "Top-1 retrieval score", "rag", &["embedding"]);
    add("ai.retrieval.hit_rate", "%", "Cache / index hit rate", "rag", &["embedding"]);

    // —— Agents / tools (LangSmith deep agents) ——
    add("ai.agent.invocations", "req/s", "Agent invocations/s", "agent", &["agent"]);
    add("ai.agent.duration", "ms", "Full agent turn duration", "agent", &["agent"]);
    add("ai.agent.tool_calls", "call", "Tool calls per turn", "agent", &["agent"]);
    add("ai.agent.trajectory_depth", "step", "Trajectory depth", "agent", &["agent"]);
    add("ai.agent.tool_errors", "error/s", "Tool call errors/s", "agent", &["agent"]);
    add("ai.agent.pass_at_k", "%", "pass@k evaluation score", "eval", &["agent"]);
    add("ai.agent.pass_hat_k", "%", "pass^k consistency score", "eval", &["agent"]);
    add("ai.tool.duration", "ms", "Tool span duration", "agent", &["agent"]);

    // —— STT / TTS ——
    add("ai.stt.requests", "req/s", "STT requests/s", "speech", &["stt"]);
    add("ai.stt.audio_seconds", "s", "Audio seconds transcribed", "speech", &["stt"]);
    add("ai.stt.rtf", "1", "Real-time factor (wall/audio)", "speech", &["stt"]);
    add("ai.stt.partial_latency", "ms", "Streaming partial lag", "speech", &["stt"]);
    add("ai.stt.wer", "%", "Word error rate (eval)", "speech", &["stt"]);
    add("ai.stt.cost", "usd", "STT cost (per audio-min)", "cost", &["stt"]);
    add("ai.tts.requests", "req/s", "TTS requests/s", "speech", &["tts"]);
    add("ai.tts.characters", "char", "Characters synthesized", "speech", &["tts"]);
    add("ai.tts.audio_seconds", "s", "Audio seconds generated", "speech", &["tts"]);
    add("ai.tts.rtf", "1", "TTS real-time factor", "speech", &["tts"]);
    add("ai.tts.cost", "usd", "TTS cost", "cost", &["tts"]);

    // —— Image / Video ——
    add("ai.image.requests", "req/s", "Image gen requests/s", "image", &["image"]);
    add("ai.image.duration", "ms", "Image generation latency", "image", &["image"]);
    add("ai.image.steps", "step", "Diffusion steps", "image", &["image"]);
    add("ai.image.gpu_seconds", "s", "GPU-seconds per image", "image", &["image"]);
    add("ai.image.cost", "usd", "Cost per image", "cost", &["image"]);
    add("ai.video.requests", "req/s", "Video gen requests/s", "video", &["video"]);
    add("ai.video.duration", "ms", "Video job wall time", "video", &["video"]);
    add("ai.video.frames", "frame", "Frames rendered", "video", &["video"]);
    add("ai.video.output_seconds", "s", "Output video duration", "video", &["video"]);
    add("ai.video.gpu_hours", "h", "GPU-hours consumed", "video", &["video"]);
    add("ai.video.cost", "usd", "Video gen cost", "cost", &["video"]);

    // —— Physical AI / Simulation ——
    add("ai.physical.policy_latency", "ms", "Policy inference latency", "physical", &["physical"]);
    add("ai.physical.episode_success", "%", "Episode success rate", "physical", &["physical"]);
    add("ai.physical.sensor_hz", "Hz", "Sensor publish rate", "physical", &["physical"]);
    add("ai.sim.fps", "fps", "Simulation frame rate", "simulation", &["simulation"]);
    add("ai.sim.physics_hz", "Hz", "Physics steps/s", "simulation", &["simulation"]);
    add("ai.sim.rtf", "1", "Sim real-time factor", "simulation", &["simulation"]);
    add("ai.sim.sdg_images_per_sec", "img/s", "Synthetic data images/s", "simulation", &["simulation"]);

    // —— Serving ——
    add("ai.serving.queue_depth", "req", "Inference queue depth", "serving", &["llm", "image"]);
    add("ai.serving.batch_size", "req", "Continuous batch size", "serving", &["llm"]);
    add("ai.serving.kv_cache_hit", "%", "KV / prefix cache hit", "serving", &["llm"]);
    add("ai.serving.oom", "event", "OOM events", "serving", &["llm", "image", "video"]);

    // —— Evals / safety ——
    add("ai.eval.score", "1", "Aggregate eval score", "eval", &["llm", "agent"]);
    add("ai.eval.correctness", "1", "Correctness grader score", "eval", &["llm", "agent"]);
    add("ai.eval.trajectory", "1", "Trajectory grader score", "eval", &["agent"]);
    add("ai.safety.guardrail_blocks", "event/s", "Guardrail block rate", "safety", &["llm"]);
    add("ai.safety.pii_hits", "event/s", "PII detections in prompts", "safety", &["llm"]);

    m
}

impl PlatformState {
    pub fn seed_ai_obs(&self) {
        // Projects across modalities
        for p in [
            AiProject {
                id: "proj-sql-agent".into(),
                name: "text-to-sql-deep-agent".into(),
                description: "Deep agent on Bedrock Nova — Chinook SQL (LangSmith AWS pattern)".into(),
                region: "US".into(),
                modality: "agent".into(),
                provider: "aws.bedrock".into(),
                model: "amazon.nova-2-lite".into(),
                runs_24h: 18420,
                error_rate: 0.028,
                avg_latency_ms: 4200.0,
                p95_latency_ms: 9800.0,
                tokens_per_sec: 86.0,
                cost_usd_24h: 42.8,
                pass_rate: 0.91,
            },
            AiProject {
                id: "proj-chat".into(),
                name: "customer-support-llm".into(),
                description: "Chat + tool-calling production LLM".into(),
                region: "US".into(),
                modality: "llm".into(),
                provider: "openai".into(),
                model: "gpt-5.4".into(),
                runs_24h: 92000,
                error_rate: 0.012,
                avg_latency_ms: 680.0,
                p95_latency_ms: 1400.0,
                tokens_per_sec: 62.0,
                cost_usd_24h: 310.0,
                pass_rate: 0.96,
            },
            AiProject {
                id: "proj-rag".into(),
                name: "docs-rag".into(),
                description: "Embeddings + retrieval + LLM".into(),
                region: "EU".into(),
                modality: "embedding".into(),
                provider: "openai".into(),
                model: "text-embedding-3-large".into(),
                runs_24h: 210000,
                error_rate: 0.004,
                avg_latency_ms: 45.0,
                p95_latency_ms: 110.0,
                tokens_per_sec: 0.0,
                cost_usd_24h: 18.2,
                pass_rate: 0.88,
            },
            AiProject {
                id: "proj-stt".into(),
                name: "call-center-stt".into(),
                description: "Streaming speech-to-text".into(),
                region: "US".into(),
                modality: "stt".into(),
                provider: "openai".into(),
                model: "whisper-1".into(),
                runs_24h: 14000,
                error_rate: 0.019,
                avg_latency_ms: 920.0,
                p95_latency_ms: 2100.0,
                tokens_per_sec: 0.0,
                cost_usd_24h: 56.0,
                pass_rate: 0.94,
            },
            AiProject {
                id: "proj-tts".into(),
                name: "voice-reply-tts".into(),
                description: "Text-to-speech responses".into(),
                region: "US".into(),
                modality: "tts".into(),
                provider: "elevenlabs".into(),
                model: "eleven_multilingual_v2".into(),
                runs_24h: 22000,
                error_rate: 0.008,
                avg_latency_ms: 310.0,
                p95_latency_ms: 720.0,
                tokens_per_sec: 0.0,
                cost_usd_24h: 88.0,
                pass_rate: 0.97,
            },
            AiProject {
                id: "proj-image".into(),
                name: "product-image-gen".into(),
                description: "Diffusion image generation".into(),
                region: "APAC".into(),
                modality: "image".into(),
                provider: "fal".into(),
                model: "flux-schnell".into(),
                runs_24h: 6400,
                error_rate: 0.035,
                avg_latency_ms: 1800.0,
                p95_latency_ms: 4200.0,
                tokens_per_sec: 0.0,
                cost_usd_24h: 74.0,
                pass_rate: 0.89,
            },
            AiProject {
                id: "proj-video".into(),
                name: "promo-video-gen".into(),
                description: "Short-form video generation".into(),
                region: "US".into(),
                modality: "video".into(),
                provider: "runway".into(),
                model: "gen-3-alpha".into(),
                runs_24h: 420,
                error_rate: 0.06,
                avg_latency_ms: 48000.0,
                p95_latency_ms: 92000.0,
                tokens_per_sec: 0.0,
                cost_usd_24h: 210.0,
                pass_rate: 0.82,
            },
            AiProject {
                id: "proj-robot".into(),
                name: "warehouse-policy".into(),
                description: "Physical AI policy + Isaac Sim".into(),
                region: "US".into(),
                modality: "physical".into(),
                provider: "nvidia".into(),
                model: "gr00t-n1".into(),
                runs_24h: 9800,
                error_rate: 0.041,
                avg_latency_ms: 12.0,
                p95_latency_ms: 28.0,
                tokens_per_sec: 0.0,
                cost_usd_24h: 0.0,
                pass_rate: 0.86,
            },
        ] {
            self.ai_projects.insert(p.id.clone(), p);
        }

        // Graders (AWS blog patterns)
        for g in [
            ("grader-code-tool", "code_tool_called", "code", "Assert required tools appear in trajectory"),
            ("grader-code-answer", "code_string_match", "code", "Deterministic substring / regex match"),
            ("grader-llm-judge", "llm_as_judge", "model", "Rubric: correctness, completeness, clarity"),
            ("grader-trajectory", "trajectory_contains", "code", "Soft trajectory: tools present, order free"),
            ("grader-human", "human_spot_check", "human", "SME calibration for LLM judges"),
            ("grader-single-step", "single_step_first_tool", "code", "First action is schema explore, not guess"),
        ] {
            self.ai_graders.insert(
                g.0.into(),
                AiGrader {
                    id: g.0.into(),
                    name: g.1.into(),
                    kind: g.2.into(),
                    description: g.3.into(),
                },
            );
        }

        // Dataset + examples (text-to-SQL)
        self.ai_datasets.insert(
            "ds-chinook".into(),
            AiDataset {
                id: "ds-chinook".into(),
                name: "chinook-sql-evals".into(),
                description: "Capability + regression suite for text-to-SQL deep agent".into(),
                example_count: 4,
                modality: "agent".into(),
            },
        );
        let examples = [
            (
                "ex-canada",
                json!({"question": "How many customers are from Canada?"}),
                json!({"answer": "8"}),
                json!({"pattern": "full_turn", "grader": "code"}),
            ),
            (
                "ex-revenue",
                json!({"question": "Which employee generated the most revenue and from which countries?"}),
                json!({"answer_contains": ["Jane Peacock"]}),
                json!({"pattern": "full_turn", "grader": "llm_as_judge"}),
            ),
            (
                "ex-schema-first",
                json!({"question": "How many customers are from Canada?"}),
                json!({"first_tools": ["sql_db_list_tables", "sql_db_schema"]}),
                json!({"pattern": "single_step", "grader": "code"}),
            ),
            (
                "ex-multiturn",
                json!({"turns": ["What are the top 5 best-selling artists?", "For the top artist, how many albums do they have?"]}),
                json!({"min_answer_len": 20}),
                json!({"pattern": "multi_turn", "grader": "code"}),
            ),
        ];
        for (id, inputs, outputs, meta) in examples {
            self.ai_examples.insert(
                id.into(),
                AiExample {
                    id: id.into(),
                    dataset_id: "ds-chinook".into(),
                    inputs,
                    outputs,
                    metadata: meta,
                },
            );
        }

        // Experiment with pass@k / pass^k
        self.ai_experiments.insert(
            "exp-sql-v3".into(),
            AiExperiment {
                id: "exp-sql-v3".into(),
                name: "sql-agent-offline-v3".into(),
                dataset_id: "ds-chinook".into(),
                project: "text-to-sql-deep-agent".into(),
                status: "completed".into(),
                trials_per_task: 5,
                pass_at_k: 0.95,
                pass_hat_k: 0.78,
                avg_score: 0.89,
                started_ms: Utc::now().timestamp_millis() - 3_600_000,
                finished_ms: Some(Utc::now().timestamp_millis() - 2_400_000),
            },
        );

        let eval_rows = [
            ("eval-1", "ex-canada", 1u32, "full_turn", "code_string_match", 1.0, true, 3800.0, "answer contains 8"),
            ("eval-2", "ex-canada", 2, "full_turn", "code_string_match", 1.0, true, 4100.0, "answer contains 8"),
            ("eval-3", "ex-revenue", 1, "full_turn", "llm_as_judge", 0.92, true, 6200.0, "correctness=0.95 completeness=0.9 clarity=0.9"),
            ("eval-4", "ex-schema-first", 1, "single_step", "single_step_first_tool", 1.0, true, 220.0, "called sql_db_schema first"),
            ("eval-5", "ex-multiturn", 1, "multi_turn", "code_string_match", 0.8, true, 9100.0, "turn2 produced meaningful answer"),
            ("eval-6", "ex-revenue", 2, "full_turn", "llm_as_judge", 0.45, false, 7000.0, "missed country breakdown"),
        ];
        {
            let mut q = self.ai_eval_results.write();
            for (id, ex, trial, pattern, grader, score, passed, lat, detail) in eval_rows {
                q.push_back(AiEvalResult {
                    id: id.into(),
                    experiment_id: "exp-sql-v3".into(),
                    example_id: ex.into(),
                    trial,
                    pattern: pattern.into(),
                    grader: grader.into(),
                    score,
                    passed,
                    latency_ms: lat,
                    detail: detail.into(),
                });
            }
        }

        // Seed runs (agent trajectory + LLM + multimodal samples)
        let now = Utc::now().timestamp_millis();
        let agent_traj = vec![
            "write_todos".into(),
            "sql_db_list_tables".into(),
            "sql_db_schema".into(),
            "sql_db_query_checker".into(),
            "sql_db_query".into(),
        ];
        self.push_ai_run(AiRun {
            id: "run-agent-1".into(),
            trace_id: "tr-sql-1".into(),
            parent_run_id: None,
            project: "text-to-sql-deep-agent".into(),
            name: "sql_agent_turn".into(),
            run_type: "agent".into(),
            status: "success".into(),
            start_ms: now - 12_000,
            end_ms: now - 7_800,
            latency_ms: 4200.0,
            ttft_ms: None,
            model: Some("amazon.nova-2-lite".into()),
            provider: Some("aws.bedrock".into()),
            input_tokens: 2400,
            output_tokens: 380,
            reasoning_tokens: 120,
            cache_read_tokens: 800,
            cache_write_tokens: 0,
            cost_usd: 0.0042,
            error: None,
            tool_name: None,
            modality: "agent".into(),
            tags: vec!["deep-agent".into(), "bedrock".into(), "chinook".into()],
            input_preview: "How many customers are from Canada?".into(),
            output_preview: "There are 8 customers from Canada.".into(),
            trajectory: agent_traj.clone(),
        });
        for (i, tool) in agent_traj.iter().enumerate() {
            self.push_ai_run(AiRun {
                id: format!("run-tool-{i}"),
                trace_id: "tr-sql-1".into(),
                parent_run_id: Some("run-agent-1".into()),
                project: "text-to-sql-deep-agent".into(),
                name: tool.clone(),
                run_type: "tool".into(),
                status: "success".into(),
                start_ms: now - 12_000 + (i as i64) * 700,
                end_ms: now - 12_000 + (i as i64) * 700 + 400,
                latency_ms: 400.0,
                ttft_ms: None,
                model: None,
                provider: Some("aws.bedrock".into()),
                input_tokens: 0,
                output_tokens: 0,
                reasoning_tokens: 0,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                cost_usd: 0.0,
                error: None,
                tool_name: Some(tool.clone()),
                modality: "agent".into(),
                tags: vec!["tool".into()],
                input_preview: format!("tool={tool}"),
                output_preview: "ok".into(),
                trajectory: vec![],
            });
        }

        self.push_ai_run(AiRun {
            id: "run-llm-1".into(),
            trace_id: "tr-chat-1".into(),
            parent_run_id: None,
            project: "customer-support-llm".into(),
            name: "chat_completion".into(),
            run_type: "llm".into(),
            status: "success".into(),
            start_ms: now - 5_000,
            end_ms: now - 4_320,
            latency_ms: 680.0,
            ttft_ms: Some(180.0),
            model: Some("gpt-5.4".into()),
            provider: Some("openai".into()),
            input_tokens: 1200,
            output_tokens: 340,
            reasoning_tokens: 0,
            cache_read_tokens: 400,
            cache_write_tokens: 50,
            cost_usd: 0.012,
            error: None,
            tool_name: None,
            modality: "llm".into(),
            tags: vec!["prod".into()],
            input_preview: "How do I reset my password?".into(),
            output_preview: "Open Settings → Security → Reset password…".into(),
            trajectory: vec![],
        });

        self.push_ai_run(AiRun {
            id: "run-stt-1".into(),
            trace_id: "tr-stt-1".into(),
            parent_run_id: None,
            project: "call-center-stt".into(),
            name: "transcribe".into(),
            run_type: "stt".into(),
            status: "success".into(),
            start_ms: now - 8_000,
            end_ms: now - 7_080,
            latency_ms: 920.0,
            ttft_ms: Some(210.0),
            model: Some("whisper-1".into()),
            provider: Some("openai".into()),
            input_tokens: 0,
            output_tokens: 0,
            reasoning_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            cost_usd: 0.006,
            error: None,
            tool_name: None,
            modality: "stt".into(),
            tags: vec!["streaming".into()],
            input_preview: "audio=48.2s mono".into(),
            output_preview: "I'd like to update my shipping address…".into(),
            trajectory: vec![],
        });

        self.push_ai_run(AiRun {
            id: "run-img-1".into(),
            trace_id: "tr-img-1".into(),
            parent_run_id: None,
            project: "product-image-gen".into(),
            name: "generate_content".into(),
            run_type: "image".into(),
            status: "success".into(),
            start_ms: now - 20_000,
            end_ms: now - 18_200,
            latency_ms: 1800.0,
            ttft_ms: None,
            model: Some("flux-schnell".into()),
            provider: Some("fal".into()),
            input_tokens: 0,
            output_tokens: 0,
            reasoning_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            cost_usd: 0.02,
            error: None,
            tool_name: None,
            modality: "image".into(),
            tags: vec!["1024x1024".into(), "steps=4".into()],
            input_preview: "studio photo of hiking boot on marble".into(),
            output_preview: "image/png 1024x1024".into(),
            trajectory: vec![],
        });

        // Feedback
        self.push_ai_feedback(AiFeedback {
            id: Uuid::new_v4().to_string(),
            run_id: "run-agent-1".into(),
            key: "correctness".into(),
            score: 1.0,
            comment: Some("exact count".into()),
            source: "code".into(),
            timestamp_ms: now,
        });
        self.push_ai_feedback(AiFeedback {
            id: Uuid::new_v4().to_string(),
            run_id: "run-llm-1".into(),
            key: "user_thumb".into(),
            score: 1.0,
            comment: None,
            source: "human".into(),
            timestamp_ms: now,
        });

        // Time-series samples
        {
            let mut samples = self.ai_samples.write();
            for i in 0..60i64 {
                let ts = now - (60 - i) * 60_000;
                for (proj, modality, req, err, lat, ttft, tin, tout, cost, pass) in [
                    ("text-to-sql-deep-agent", "agent", 4.2, 0.1, 4100.0, 0.0, 2200.0, 360.0, 0.018, 0.91),
                    ("customer-support-llm", "llm", 38.0, 0.4, 650.0 + (i % 7) as f64 * 20.0, 170.0, 1100.0, 320.0, 0.13, 0.96),
                    ("docs-rag", "embedding", 85.0, 0.2, 42.0, 0.0, 800.0, 0.0, 0.008, 0.88),
                    ("call-center-stt", "stt", 5.5, 0.1, 900.0, 200.0, 0.0, 0.0, 0.02, 0.94),
                    ("product-image-gen", "image", 2.1, 0.07, 1750.0, 0.0, 0.0, 0.0, 0.03, 0.89),
                    ("warehouse-policy", "physical", 12.0, 0.4, 11.0, 0.0, 0.0, 0.0, 0.0, 0.86),
                ] {
                    samples.push_back(AiHostSample {
                        timestamp_ms: ts,
                        project: proj.into(),
                        modality: modality.into(),
                        requests: req * (1.0 + (i % 5) as f64 * 0.02),
                        errors: err,
                        latency_ms: lat,
                        ttft_ms: ttft,
                        tokens_in: tin,
                        tokens_out: tout,
                        cost_usd: cost,
                        pass_rate: pass,
                    });
                }
            }
            while samples.len() > MAX_SAMPLES {
                samples.pop_front();
            }
        }

        let _ = self.emit_ai_metrics(None);
    }

    fn push_ai_run(&self, run: AiRun) {
        let mut q = self.ai_runs.write();
        q.push_back(run);
        while q.len() > MAX_RUNS {
            q.pop_front();
        }
    }

    fn push_ai_feedback(&self, fb: AiFeedback) {
        let mut q = self.ai_feedback.write();
        q.push_back(fb);
        while q.len() > MAX_FEEDBACK {
            q.pop_front();
        }
    }

    pub fn ai_summary(&self) -> serde_json::Value {
        let projects: Vec<_> = self.ai_projects.iter().map(|e| e.value().clone()).collect();
        let runs = self.ai_runs.read().len();
        let feedback = self.ai_feedback.read().len();
        let experiments = self.ai_experiments.len();
        let datasets = self.ai_datasets.len();
        let total_cost: f64 = projects.iter().map(|p| p.cost_usd_24h).sum();
        let total_runs: u64 = projects.iter().map(|p| p.runs_24h).sum();
        let avg_pass = if projects.is_empty() {
            0.0
        } else {
            projects.iter().map(|p| p.pass_rate).sum::<f64>() / projects.len() as f64
        };
        let modalities: Vec<String> = {
            let mut m: Vec<_> = projects.iter().map(|p| p.modality.clone()).collect();
            m.sort();
            m.dedup();
            m
        };
        json!({
            "projects": projects.len(),
            "runs_buffered": runs,
            "runs_24h": total_runs,
            "feedback": feedback,
            "experiments": experiments,
            "datasets": datasets,
            "graders": self.ai_graders.len(),
            "cost_usd_24h": (total_cost * 100.0).round() / 100.0,
            "avg_pass_rate": (avg_pass * 1000.0).round() / 1000.0,
            "metrics_catalog_count": ai_metric_catalog().len(),
            "modalities": modalities,
            "docs": {
                "langsmith": "https://github.com/langchain-ai/langsmith-sdk",
                "aws_deep_agents": "https://aws.amazon.com/blogs/machine-learning/evaluating-deep-agents-using-langsmith-on-aws/",
                "otel_genai": "https://opentelemetry.io/docs/specs/semconv/gen-ai/",
            },
            "performance": {
                "engine": "rust",
                "run_ring_capacity": MAX_RUNS,
                "ingest": "lock-scoped VecDeque + DashMap — zero GC pause",
            }
        })
    }

    pub fn list_ai_projects(&self) -> Vec<AiProject> {
        let mut v: Vec<_> = self.ai_projects.iter().map(|e| e.value().clone()).collect();
        v.sort_by(|a, b| b.runs_24h.cmp(&a.runs_24h));
        v
    }

    pub fn list_ai_runs(&self, project: Option<&str>, run_type: Option<&str>, limit: usize) -> Vec<AiRun> {
        self.ai_runs
            .read()
            .iter()
            .rev()
            .filter(|r| project.map(|p| r.project == p).unwrap_or(true))
            .filter(|r| run_type.map(|t| r.run_type == t).unwrap_or(true))
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn list_ai_feedback(&self, limit: usize) -> Vec<AiFeedback> {
        self.ai_feedback.read().iter().rev().take(limit).cloned().collect()
    }

    pub fn list_ai_datasets(&self) -> Vec<AiDataset> {
        self.ai_datasets.iter().map(|e| e.value().clone()).collect()
    }

    pub fn list_ai_examples(&self, dataset_id: Option<&str>) -> Vec<AiExample> {
        self.ai_examples
            .iter()
            .map(|e| e.value().clone())
            .filter(|e| dataset_id.map(|d| e.dataset_id == d).unwrap_or(true))
            .collect()
    }

    pub fn list_ai_experiments(&self) -> Vec<AiExperiment> {
        self.ai_experiments.iter().map(|e| e.value().clone()).collect()
    }

    pub fn list_ai_eval_results(&self, experiment_id: Option<&str>) -> Vec<AiEvalResult> {
        self.ai_eval_results
            .read()
            .iter()
            .filter(|e| experiment_id.map(|id| e.experiment_id == id).unwrap_or(true))
            .cloned()
            .collect()
    }

    pub fn list_ai_graders(&self) -> Vec<AiGrader> {
        self.ai_graders.iter().map(|e| e.value().clone()).collect()
    }

    pub fn list_ai_samples(&self, project: Option<&str>, limit: usize) -> Vec<AiHostSample> {
        self.ai_samples
            .read()
            .iter()
            .rev()
            .filter(|s| project.map(|p| s.project == p).unwrap_or(true))
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn ai_metric_catalog_json(&self) -> serde_json::Value {
        let metrics = ai_metric_catalog();
        let mut groups = serde_json::Map::new();
        for m in &metrics {
            *groups.entry(m.group.clone()).or_insert(json!(0)) =
                json!(groups.get(&m.group).and_then(|v| v.as_u64()).unwrap_or(0) + 1);
        }
        json!({
            "count": metrics.len(),
            "metrics": metrics,
            "groups": groups,
            "docs": "https://opentelemetry.io/docs/specs/semconv/gen-ai/",
        })
    }

    pub fn ingest_ai_run(&self, req: AiCreateRun) -> AiRun {
        let now = Utc::now().timestamp_millis();
        let latency = req.latency_ms.unwrap_or(0.0);
        let run = AiRun {
            id: Uuid::new_v4().to_string(),
            trace_id: Uuid::new_v4().to_string(),
            parent_run_id: req.parent_run_id,
            project: req.project.unwrap_or_else(|| "default".into()),
            name: req.name,
            run_type: req.run_type,
            status: if req.error.is_some() {
                "error".into()
            } else {
                "success".into()
            },
            start_ms: now - latency as i64,
            end_ms: now,
            latency_ms: latency,
            ttft_ms: req.ttft_ms,
            model: req.model,
            provider: req.provider,
            input_tokens: req.input_tokens.unwrap_or(0),
            output_tokens: req.output_tokens.unwrap_or(0),
            reasoning_tokens: 0,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
            cost_usd: req.cost_usd.unwrap_or(0.0),
            error: req.error,
            tool_name: None,
            modality: req.modality.unwrap_or_else(|| "llm".into()),
            tags: req.tags.unwrap_or_default(),
            input_preview: req.input_preview.unwrap_or_default(),
            output_preview: req.output_preview.unwrap_or_default(),
            trajectory: req.trajectory.unwrap_or_default(),
        };
        self.push_ai_run(run.clone());
        run
    }

    pub fn ingest_ai_runs_batch(&self, runs: Vec<AiCreateRun>) -> serde_json::Value {
        let start = std::time::Instant::now();
        let n = runs.len();
        for r in runs {
            let _ = self.ingest_ai_run(r);
        }
        json!({
            "ingested": n,
            "elapsed_us": start.elapsed().as_micros() as u64,
            "runs_per_sec": if start.elapsed().as_secs_f64() > 0.0 {
                n as f64 / start.elapsed().as_secs_f64()
            } else {
                f64::INFINITY
            },
        })
    }

    /// Offline eval harness — patterns from AWS LangSmith deep-agent guide.
    pub fn run_ai_experiment(&self, dataset_id: &str, trials: u32) -> serde_json::Value {
        let start = std::time::Instant::now();
        let examples: Vec<_> = self.list_ai_examples(Some(dataset_id));
        if examples.is_empty() {
            return json!({"error": "dataset empty or missing"});
        }
        let exp_id = Uuid::new_v4().to_string();
        let k = trials.max(1);
        let mut passes_at_least_one = 0u32;
        let mut passes_all = 0u32;
        let mut scores = Vec::new();
        let mut results = Vec::new();

        for ex in &examples {
            let pattern = ex
                .metadata
                .get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("full_turn");
            let mut trial_pass = 0u32;
            for trial in 1..=k {
                // Deterministic demo scoring biased by pattern
                let score = match pattern {
                    "single_step" => 0.95,
                    "multi_turn" => 0.82,
                    "full_turn" => {
                        if ex.id.contains("revenue") && trial == k {
                            0.48
                        } else {
                            0.93
                        }
                    }
                    _ => 0.9,
                };
                let passed = score >= 0.5;
                if passed {
                    trial_pass += 1;
                }
                scores.push(score);
                let row = AiEvalResult {
                    id: Uuid::new_v4().to_string(),
                    experiment_id: exp_id.clone(),
                    example_id: ex.id.clone(),
                    trial,
                    pattern: pattern.into(),
                    grader: ex
                        .metadata
                        .get("grader")
                        .and_then(|v| v.as_str())
                        .unwrap_or("code")
                        .into(),
                    score,
                    passed,
                    latency_ms: 200.0 + trial as f64 * 50.0,
                    detail: format!("pattern={pattern} trial={trial}"),
                };
                results.push(row.clone());
                self.ai_eval_results.write().push_back(row);
            }
            if trial_pass >= 1 {
                passes_at_least_one += 1;
            }
            if trial_pass == k {
                passes_all += 1;
            }
        }

        let n_ex = examples.len() as f64;
        let pass_at_k = passes_at_least_one as f64 / n_ex;
        let pass_hat_k = passes_all as f64 / n_ex;
        let avg_score = scores.iter().sum::<f64>() / scores.len().max(1) as f64;

        let exp = AiExperiment {
            id: exp_id.clone(),
            name: format!("exp-{dataset_id}-{}", &exp_id[..8]),
            dataset_id: dataset_id.into(),
            project: "text-to-sql-deep-agent".into(),
            status: "completed".into(),
            trials_per_task: k,
            pass_at_k,
            pass_hat_k,
            avg_score,
            started_ms: Utc::now().timestamp_millis(),
            finished_ms: Some(Utc::now().timestamp_millis()),
        };
        self.ai_experiments.insert(exp_id.clone(), exp);

        json!({
            "experiment_id": exp_id,
            "dataset_id": dataset_id,
            "examples": examples.len(),
            "trials_per_task": k,
            "results": results.len(),
            "pass_at_k": pass_at_k,
            "pass_hat_k": pass_hat_k,
            "avg_score": avg_score,
            "patterns_covered": ["custom_per_datapoint", "single_step", "full_turn", "multi_turn"],
            "graders": ["code", "llm_as_judge", "human"],
            "elapsed_us": start.elapsed().as_micros() as u64,
        })
    }

    pub fn ai_health(&self) -> serde_json::Value {
        let mut findings = Vec::new();
        for p in self.list_ai_projects() {
            if p.error_rate > 0.05 {
                findings.push(json!({
                    "project": p.name,
                    "severity": "critical",
                    "finding": format!("error_rate {:.1}% > 5%", p.error_rate * 100.0),
                }));
            } else if p.pass_rate < 0.85 {
                findings.push(json!({
                    "project": p.name,
                    "severity": "warn",
                    "finding": format!("pass_rate {:.0}% below regression bar", p.pass_rate * 100.0),
                }));
            } else if p.p95_latency_ms > 10_000.0 && p.modality == "agent" {
                findings.push(json!({
                    "project": p.name,
                    "severity": "warn",
                    "finding": format!("p95 agent latency {:.0}ms", p.p95_latency_ms),
                }));
            }
        }
        json!({
            "projects_checked": self.ai_projects.len(),
            "findings": findings,
            "experiments": self.ai_experiments.len(),
        })
    }

    pub fn emit_ai_metrics(&self, project_filter: Option<&str>) -> usize {
        let ts = Utc::now().timestamp_millis();
        let mut n = 0usize;
        for p in self.list_ai_projects() {
            if let Some(f) = project_filter {
                if p.name != f && p.id != f {
                    continue;
                }
            }
            let mut tags = Tags::new();
            tags.insert("project".into(), p.name.clone());
            tags.insert("modality".into(), p.modality.clone());
            tags.insert("provider".into(), p.provider.clone());
            tags.insert("model".into(), p.model.clone());
            tags.insert("region".into(), p.region.clone());

            let pairs: Vec<(&str, f64)> = match p.modality.as_str() {
                "llm" | "agent" => vec![
                    ("ai.llm.requests", p.runs_24h as f64 / 86400.0),
                    ("ai.llm.errors", p.error_rate * p.runs_24h as f64 / 86400.0),
                    ("ai.llm.duration", p.avg_latency_ms),
                    ("ai.llm.ttft", if p.modality == "llm" { 180.0 } else { 0.0 }),
                    ("ai.llm.tokens_per_sec", p.tokens_per_sec),
                    ("ai.llm.total.cost", p.cost_usd_24h / 86400.0),
                    ("ai.agent.invocations", p.runs_24h as f64 / 86400.0),
                    ("ai.agent.pass_at_k", p.pass_rate * 100.0),
                    ("ai.eval.score", p.pass_rate),
                ],
                "embedding" => vec![
                    ("ai.embedding.requests", p.runs_24h as f64 / 86400.0),
                    ("ai.embedding.duration", p.avg_latency_ms),
                    ("ai.embedding.cost", p.cost_usd_24h / 86400.0),
                    ("ai.retrieval.duration", 18.0),
                    ("ai.retrieval.hit_rate", 92.0),
                ],
                "stt" => vec![
                    ("ai.stt.requests", p.runs_24h as f64 / 86400.0),
                    ("ai.stt.rtf", 0.42),
                    ("ai.stt.partial_latency", 210.0),
                    ("ai.stt.wer", 6.2),
                    ("ai.stt.cost", p.cost_usd_24h / 86400.0),
                ],
                "tts" => vec![
                    ("ai.tts.requests", p.runs_24h as f64 / 86400.0),
                    ("ai.tts.rtf", 0.28),
                    ("ai.tts.cost", p.cost_usd_24h / 86400.0),
                ],
                "image" => vec![
                    ("ai.image.requests", p.runs_24h as f64 / 86400.0),
                    ("ai.image.duration", p.avg_latency_ms),
                    ("ai.image.steps", 4.0),
                    ("ai.image.gpu_seconds", 1.6),
                    ("ai.image.cost", p.cost_usd_24h / 86400.0),
                ],
                "video" => vec![
                    ("ai.video.requests", p.runs_24h as f64 / 86400.0),
                    ("ai.video.duration", p.avg_latency_ms),
                    ("ai.video.frames", 120.0),
                    ("ai.video.gpu_hours", 0.02),
                    ("ai.video.cost", p.cost_usd_24h / 86400.0),
                ],
                "physical" | "simulation" => vec![
                    ("ai.physical.policy_latency", p.avg_latency_ms),
                    ("ai.physical.episode_success", p.pass_rate * 100.0),
                    ("ai.physical.sensor_hz", 30.0),
                    ("ai.sim.fps", 180.0),
                    ("ai.sim.physics_hz", 36.0),
                    ("ai.sim.rtf", 1.05),
                    ("ai.sim.sdg_images_per_sec", 28.0),
                ],
                _ => vec![("ai.llm.requests", p.runs_24h as f64 / 86400.0)],
            };

            for (name, value) in pairs {
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
                        unit: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use thine_storage::{MetricStore, StorageConfig};

    #[test]
    fn catalog_covers_modalities() {
        let cat = ai_metric_catalog();
        assert!(cat.len() >= 60);
        assert!(cat.iter().any(|m| m.group == "speech"));
        assert!(cat.iter().any(|m| m.group == "simulation"));
        assert!(cat.iter().any(|m| m.name.contains("pass_at_k")));
    }

    #[test]
    fn seed_and_eval_harness() {
        let metrics = MetricStore::new(StorageConfig::default());
        let p = PlatformState::new(metrics);
        p.seed_ai_obs();
        assert!(p.list_ai_projects().len() >= 6);
        assert!(!p.list_ai_runs(None, None, 10).is_empty());
        let exp = p.run_ai_experiment("ds-chinook", 3);
        assert!(exp["pass_at_k"].as_f64().unwrap() > 0.5);
        assert!(exp["elapsed_us"].as_u64().unwrap() < 50_000);
        let batch = p.ingest_ai_runs_batch(vec![AiCreateRun {
            name: "bench".into(),
            run_type: "llm".into(),
            project: Some("customer-support-llm".into()),
            parent_run_id: None,
            model: Some("gpt-5.4".into()),
            provider: Some("openai".into()),
            modality: Some("llm".into()),
            input_tokens: Some(10),
            output_tokens: Some(5),
            latency_ms: Some(12.0),
            ttft_ms: Some(4.0),
            cost_usd: Some(0.0001),
            error: None,
            input_preview: None,
            output_preview: None,
            tags: None,
            trajectory: None,
        }]);
        assert_eq!(batch["ingested"], 1);
    }
}
