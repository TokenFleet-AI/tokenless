# 0014 — Semantic-Aware Compression

## 1. Motivation

Current ResponseCompressor uses hardcoded rules: drop fields named `debug`/`trace`/`stack`, truncate strings >512 chars, limit arrays >16 items. All fields are treated equally regardless of the current task.

Semantic-aware compression asks: given what the LLM is trying to do right now, which fields matter and which don't?

```
API Response (weather API, 50 fields)

Task: "今天天气怎么样"           Task: "气象站状态检查"
─────────────────────           ─────────────────────
Keep: temp, humidity, wind      Keep: station_id, sensor_version
      forecast.today                   last_calibration, battery
Drop: station_id, calibration    Drop: forecast.day3, humidity
      maintenance_contact               maintenance_contact
```

Same response, different compression — based on context.

## 2. Three-Level Architecture

```
┌─────────────────────────────────────────────────────────┐
│                  SemanticCompressor                      │
│                                                         │
│  compress(value, context) → compressed_value             │
│                                                         │
│  ┌──────────┐    ┌──────────────┐    ┌──────────────┐  │
│  │ Level 1  │    │  Level 2     │    │  Level 3     │  │
│  │ Rules    │    │  ONNX Model  │    │  External API│  │
│  │ (zero    │    │  (local      │    │  (OpenAI     │  │
│  │  deps)   │    │   embedding) │    │   embedding) │  │
│  └────┬─────┘    └──────┬───────┘    └──────┬───────┘  │
│       │                 │                    │          │
│       └─────────────────┼────────────────────┘          │
│                         ▼                               │
│                  Auto-degradation                        │
│           (ONNX fails → Level 1 fallback)               │
└─────────────────────────────────────────────────────────┘
```

## 3. Level 1: Rule-Based Field Classification

Zero dependencies. A static mapping from context keywords to field keep/drop rules.

### Configuration (context_rules.toml bundled at compile time)

```toml
[weather]
keep = ["temp*", "humid*", "wind*", "forecast*", "condition*", "pressure*", "visibility*", "uv*"]
drop = ["station_id", "sensor_*", "calibration", "maintenance_*"]

[devops]
keep = ["status*", "pod*", "node*", "deploy*", "error*", "cpu*", "memory*", "replicas*"]
drop = ["internal_*", "debug_*", "trace_*"]

[database]
keep = ["query*", "table*", "row*", "index*", "latency*", "schema*", "connection*"]
drop = ["internal_*", "debug_*"]

[git]
keep = ["branch*", "status", "modified*", "staged*", "untracked*", "ahead*", "behind*"]
drop = ["author_*", "committer_*"]   # unless context explicitly mentions author

[default]
drop = ["debug", "trace", "traces", "stack", "stacktrace", "logs", "logging"]
```

### Matching Algorithm

```rust
fn classify_field(field_name: &str, context: &str) -> FieldAction {
    let rules = match context_category(context) {
        "weather"  => &WEATHER_RULES,
        "devops"   => &DEVOPS_RULES,
        "database" => &DATABASE_RULES,
        "git"      => &GIT_RULES,
        _          => &DEFAULT_RULES,
    };

    // Drop rules match first (security-sensitive fields)
    for pattern in &rules.drop {
        if glob_match(pattern, field_name) {
            return FieldAction::Drop;
        }
    }

    // Keep rules override default truncation
    for pattern in &rules.keep {
        if glob_match(pattern, field_name) {
            return FieldAction::Keep;
        }
    }

    // Default: apply standard truncation rules
    FieldAction::Truncate
}
```

### Context Category Detection

Simple keyword matching on the user-provided context string:

```rust
fn context_category(context: &str) -> &str {
    let ctx = context.to_lowercase();
    if ctx.contains("weather") || ctx.contains("天气") || ctx.contains("温度") { return "weather"; }
    if ctx.contains("deploy") || ctx.contains("pod") || ctx.contains("k8s") || ctx.contains("集群") { return "devops"; }
    if ctx.contains("query") || ctx.contains("sql") || ctx.contains("table") || ctx.contains("查询") { return "database"; }
    if ctx.contains("git") || ctx.contains("commit") || ctx.contains("branch") { return "git"; }
    "default"
}
```

## 4. Level 2: ONNX Embedding Model

### Model: all-MiniLM-L6-v2 (ONNX quantized)

| Property | Value |
|----------|-------|
| Model size | ~15 MB (INT8 quantized) |
| Embedding dimension | 384 |
| Inference time | <1 ms per text on CPU |
| Max input length | 256 tokens |
| License | Apache 2.0 |

### Architecture

```rust
// crates/tokenless-semantic/src/embedder.rs

use ort::{Session, SessionBuilder, Value as OrtValue};

pub struct Embedder {
    session: Session,
    tokenizer: Tokenizer,  // HuggingFace tokenizers via tokenizers crate
}

impl Embedder {
    /// Load model from ~/.tokenfleet-ai/tokenless/models/model.onnx.
    /// Downloads automatically on first run if missing.
    pub fn load() -> Result<Self, EmbedderError> {
        let model_path = model_path()?;
        if !model_path.exists() {
            download_model(&model_path)?;
        }
        let session = SessionBuilder::new()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(2)?
            .commit_from_file(&model_path)?;
        let tokenizer = Tokenizer::from_file(tokenizer_path()?)?;
        Ok(Self { session, tokenizer })
    }

    /// Compute embedding for a text string.
    pub fn embed(&self, text: &str) -> Result<Vec<f32>, EmbedderError> {
        let tokens = self.tokenizer.encode(text, true)?;
        let input = OrtValue::from_array(self.session.allocator(), &tokens)?;
        let outputs = self.session.run(vec![input])?;
        let embedding: Vec<f32> = outputs[0].try_extract()?.view()?.iter().collect();
        // Mean pooling
        Ok(embedding)
    }

    /// Cosine similarity between two embeddings.
    pub fn similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 { return 0.0; }
        dot / (norm_a * norm_b)
    }
}
```

### Model Download

```rust
fn download_model(model_path: &Path) -> Result<(), EmbedderError> {
    let url = "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/onnx/model_quantized.onnx";
    let tokenizer_url = "https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/tokenizer.json";

    fs::create_dir_all(model_path.parent().unwrap())?;

    // Download with progress
    download_file(url, model_path)?;
    download_file(tokenizer_url, &model_path.with_file_name("tokenizer.json"))?;

    Ok(())
}
```

### Field Classification with Embeddings

```rust
fn classify_field_onnx(
    embedder: &Embedder,
    field_name: &str,
    field_value: &str,
    context_embedding: &[f32],
) -> FieldAction {
    // Build field description from name + value prefix
    let field_text = format!("{}: {}", field_name,
        &field_value[..field_value.len().min(100)]);

    let field_embedding = match embedder.embed(&field_text) {
        Ok(e) => e,
        Err(_) => return FieldAction::Truncate,  // degrade gracefully
    };

    let similarity = Embedder::similarity(&field_embedding, context_embedding);

    if similarity > 0.7 {
        FieldAction::Keep
    } else if similarity > 0.4 {
        FieldAction::Truncate    // string at 32 chars instead of 512
    } else {
        FieldAction::Drop
    }
}
```

## 5. Level 3: External Embedding API

For users who already have API access. Zero local model.

```rust
fn embed_remote(text: &str, api_key: &str) -> Result<Vec<f32>, EmbedderError> {
    let client = reqwest::blocking::Client::new();
    let resp = client
        .post("https://api.openai.com/v1/embeddings")
        .header("Authorization", format!("Bearer {api_key}"))
        .json(&json!({"input": text, "model": "text-embedding-3-small"}))
        .send()?;
    // Parse response, extract embedding
}
```

## 6. Unified API

```rust
// crates/tokenless-semantic/src/semantic_compressor.rs

pub struct SemanticCompressor {
    base: ResponseCompressor,       // standard structural compression
    rules: ContextRules,            // Level 1: rule-based
    embedder: Option<Embedder>,     // Level 2: ONNX (optional)
    api_key: Option<String>,        // Level 3: remote API (optional)
}

impl SemanticCompressor {
    pub fn new() -> Self {
        Self {
            base: ResponseCompressor::new(),
            rules: ContextRules::load(),
            embedder: None,
            api_key: None,
        }
    }

    pub fn with_onnx(mut self) -> Result<Self, EmbedderError> {
        self.embedder = Some(Embedder::load()?);
        Ok(self)
    }

    pub fn with_remote(mut self, api_key: &str) -> Self {
        self.api_key = Some(api_key.to_string());
        self
    }

    pub fn compress(&self, value: &Value, context: &str) -> Value {
        // Step 1: Apply standard structural compression
        let compressed = self.base.compress(value);

        // Step 2: Semantic field filtering
        let context_embedding = self.get_context_embedding(context);

        match &compressed {
            Value::Object(obj) => {
                let mut result = serde_json::Map::new();
                for (key, val) in obj {
                    let action = self.classify_field(key, val, &context_embedding);
                    match action {
                        FieldAction::Keep => { result.insert(key.clone(), val.clone()); }
                        FieldAction::Drop => { /* skip */ }
                        FieldAction::Truncate => {
                            result.insert(key.clone(), self.truncate_value(val));
                        }
                    }
                }
                Value::Object(result)
            }
            _ => compressed,
        }
    }
}
```

## 7. Degradation Strategy

```
ONNX model available?
    ├─ Yes → Level 2 (embedding-based classification)
    └─ No  → OPENAI_API_KEY set?
              ├─ Yes → Level 3 (remote API)
              └─ No  → Level 1 (rule-based, always available)
```

Level 1 is always the fallback. ONNX download failure → Level 1. ONNX inference error → Level 1 per-field (one bad field doesn't kill the whole compression).

## 8. CLI Integration

```bash
# Level 1: rule-based (zero config)
tokenless compress-response -f weather.json --semantic --context "今天天气"

# Level 2: ONNX local model
tokenless compress-response -f weather.json --semantic --context "今天天气" --model onnx

# Level 3: OpenAI embeddings
export OPENAI_API_KEY=sk-xxx
tokenless compress-response -f weather.json --semantic --context "今天天气" --model remote

# Auto: try ONNX → fallback remote → fallback rules
tokenless compress-response -f weather.json --semantic --context "今天天气" --model auto
```

## 9. Crate Structure

```
crates/tokenless-semantic/
├── Cargo.toml
│   ├── serde_json (workspace)
│   ├── tokenless-schema (workspace, for ResponseCompressor)
│   ├── ort (optional, for ONNX)
│   ├── tokenizers (optional, HuggingFace tokenizer)
│   └── reqwest (optional, for remote API)
├── src/
│   ├── lib.rs                       # Public API, re-exports
│   ├── rules.rs                     # Level 1: context → field rules
│   ├── context.rs                   # Context category detection
│   ├── embedder.rs                  # Level 2: ONNX loading + inference
│   ├── download.rs                  # Model download logic
│   ├── remote.rs                    # Level 3: OpenAI embedding API
│   ├── semantic_compressor.rs       # Unified compressor
│   └── context_rules.toml           # Bundled rule definitions
├── tests/
│   ├── rules_tests.rs               # Level 1 rule matching
│   ├── semantic_compressor_tests.rs # Integration tests
│   └── fixtures/
│       ├── weather_response.json
│       ├── weather_compressed.json   # Golden file: expected output
│       └── devops_response.json
└── README.md
```

## 10. Build Configuration

```toml
# crates/tokenless-semantic/Cargo.toml

[features]
default = ["rules"]              # Level 1 only, zero new deps
onnx = ["dep:ort", "dep:tokenizers"]   # Level 2
remote = ["dep:reqwest"]          # Level 3
full = ["onnx", "remote"]         # All levels
```

`tokenless-cli` depends on `tokenless-semantic` with `features = ["full"]` by default.

## 11. Test Strategy

| Test | Level | Description |
|------|-------|-------------|
| `test_context_detect_weather` | L1 | "今天天气怎么样" → category "weather" |
| `test_field_keep_temp` | L1 | Field "temperature" + context "weather" → Keep |
| `test_field_drop_station_id` | L1 | Field "station_id" + context "weather" → Drop |
| `test_context_unknown` | L1 | "random query" → category "default" |
| `test_weather_response_compressed` | L1 | Full weather JSON → golden file comparison |
| `test_embedder_load` | L2 | ONNX model loads without error |
| `test_embed_similarity` | L2 | "temperature" ≈ "weather", "station" ≉ "weather" |
| `test_degradation_no_model` | L2 | Missing model → falls back to Level 1 |
| `test_onnx_field_classification` | L2 | Real embedding → Keep/Drop/Truncate scores |

## 12. Implementation Order

1. **Phase 1: Level 1 rules** — `crates/tokenless-semantic/src/rules.rs` + `context.rs`, no new deps (~150 lines)
2. **Phase 2: CLI integration** — `main.rs` add `--semantic --context` flags (~30 lines)
3. **Phase 3: Level 2 ONNX** — `embedder.rs` + `download.rs`, `ort` + `tokenizers` deps (~300 lines)
4. **Phase 4: Level 3 remote** — `remote.rs`, `reqwest` dep (~80 lines)

Phase 1+2 can be done immediately. Phase 3+4 are feature-gated.
