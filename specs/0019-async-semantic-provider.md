# 0019 — Async Semantic Provider

## 1. Motivation

`tokenless-semantic` currently has two semantic levels:

- Level 1: local rule-based matching
- Level 2: local ONNX embedding inference

Both are local and synchronous. That works well for low-latency, zero-network deployments, but it leaves a capability gap:

- some users do not want to ship or download local models
- some environments already standardize on managed embedding APIs
- some remote providers offer stronger multilingual embedding quality than a small local model
- network I/O naturally requires async execution if we want timeouts, cancellation, and concurrency to behave correctly

Adding a Level 3 remote embedding provider lets `tokenless` support hosted semantic inference without forcing every caller onto a blocking API.

The design goal is not to replace Level 1 or Level 2. It is to add a third level that preserves the existing local-first model while making remote providers a first-class, optional capability.

## 2. Three-Level Architecture

The semantic stack becomes a layered capability model:

```text
┌──────────────────────────────────────────────────────────────┐
│                    Semantic Engine                           │
│                                                              │
│  classify(text, context) → semantic decision                 │
│                                                              │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────────┐  │
│  │   Level 1    │   │   Level 2    │   │     Level 3      │  │
│  │ local rules  │   │ local ONNX   │   │ remote embedding │  │
│  │ synchronous  │   │ synchronous  │   │ asynchronous      │  │
│  └──────┬───────┘   └──────┬───────┘   └────────┬─────────┘  │
│         │                  │                    │            │
│         └──────────────────┴────────────────────┘            │
│                          fallback chain                      │
└──────────────────────────────────────────────────────────────┘
```

### Level 1: Local Rules

Level 1 remains the always-available baseline:

- no network dependency
- no model download
- deterministic behavior
- synchronous API
- lowest semantic accuracy, highest availability

This level should continue to handle coarse context detection and field-selection heuristics.

### Level 2: Local ONNX

Level 2 remains local inference using an embedded or downloaded ONNX model:

- no remote data transfer
- no per-request provider cost
- synchronous API today
- higher semantic quality than pure rules
- depends on local model availability and runtime support

This level is the preferred local semantic path when the model is available and the deployment allows local inference.

### Level 3: Remote Embedding API

Level 3 is a new optional provider abstraction over remote embedding services:

- async by design
- network-bound latency profile
- configurable timeout and retry behavior
- optional API authentication
- potentially stronger embedding quality and easier operations than shipping a local model

Unlike Level 1 and Level 2, Level 3 must assume partial failure:

- DNS and TCP failures
- provider 429 or 5xx responses
- request timeout
- invalid or rotated credentials
- schema drift or malformed responses

That failure model is the main reason the provider interface must become asynchronous at the abstraction boundary.

## 3. Provider Trait Design

The new provider abstraction is:

```rust
#[async_trait]
pub trait SemanticProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    fn name(&self) -> &str;
}
```

This trait should be the Level 3 contract and the long-term extension point for any future semantic backend.

### Trait Semantics

- `embed` returns a normalized embedding vector for the given input text.
- `name` returns a stable provider identifier for logging, metrics, and diagnostics.
- `Result<Vec<f32>>` uses a provider-specific or crate-level error type with enough detail to distinguish retryable and non-retryable failures.

### Why `async_trait`

Native async traits are improving, but `async-trait` remains the simplest way to support trait objects and heterogeneous providers without introducing complex generic plumbing throughout the crate. This is especially useful if configuration chooses the provider at runtime.

### Expected Implementations

Planned implementations:

- `RemoteEmbeddingProvider` for HTTP API providers
- adapter wrappers for concrete vendors if provider-specific request/response schemas differ
- optional future wrappers for Level 2 if the crate later wants a fully unified async-facing API

Example sketch:

```rust
pub struct RemoteEmbeddingProvider {
    client: reqwest::Client,
    endpoint: Url,
    api_key: SecretString,
    timeout: Duration,
    model: String,
}

#[async_trait]
impl SemanticProvider for RemoteEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // validate input bounds
        // build HTTP request
        // parse response
        // validate embedding dimension > 0
        // return embedding
    }

    fn name(&self) -> &str {
        "remote"
    }
}
```

### Input and Validation Requirements

Remote embedding requests cross a trust boundary and must follow the repository security rules:

- reject empty inputs if the provider cannot handle them consistently
- cap input text length by bytes before network dispatch
- validate URL scheme and disallow unsafe schemes
- treat provider response payloads as hostile until parsed and validated
- reject zero-length embeddings and embeddings containing non-finite floats

## 4. Unified Interface Shape

The crate currently has synchronous Level 1 and Level 2 behavior. Introducing async Level 3 creates an API design choice:

1. keep existing synchronous entry points and add async remote-only paths
2. introduce an async top-level semantic interface and adapt sync implementations into it

The recommended direction is a hybrid transition:

### Near-Term

- keep existing synchronous Level 1 and Level 2 internals
- add a new async orchestration layer for provider selection and fallback
- expose async APIs where Level 3 can participate

### Long-Term

- standardize the public semantic selection/compression entry point on async
- internally call synchronous Level 1 and Level 2 logic from that async boundary without spawning unless necessary

Conceptually:

```rust
pub enum SemanticLevel {
    Level1Rules,
    Level2Onnx,
    Level3Remote,
}

pub struct SemanticEngine {
    rules: RulesEngine,
    onnx: Option<OnnxEmbedder>,
    remote: Option<Box<dyn SemanticProvider + Send + Sync>>,
}

impl SemanticEngine {
    pub async fn embed_with_fallback(&self, text: &str) -> Result<SemanticEmbedding> {
        // Level 3 → Level 2 → Level 1
    }
}
```

This avoids forcing the entire crate to become async in one step while still giving callers one canonical async path for the full three-level stack.

## 5. Fallback Strategy

Fallback must be explicit and deterministic:

```text
try Level 3 remote
    ↓ on timeout / network / retry exhaustion / 5xx / unsupported config
try Level 2 local ONNX
    ↓ on model unavailable / runtime init failure / inference failure
use Level 1 local rules
```

### Fallback Rules

#### Level 3 → Level 2

Fallback from remote to ONNX when:

- remote provider is not configured
- endpoint is invalid
- credentials are missing or rejected
- request times out
- retry budget is exhausted
- provider returns a transient server error
- response payload is invalid

The system should log or trace that the provider degraded, but should not fail the entire semantic operation if Level 2 is available.

#### Level 2 → Level 1

Fallback from ONNX to rules when:

- ONNX feature is disabled
- model artifact is missing
- model initialization fails
- inference fails for the given input

#### When to Return an Error

Only return a hard error when:

- the caller explicitly requested a single strict level and disabled fallback, or
- all configured levels fail and Level 1 cannot produce a meaningful classification for the requested operation

Default behavior should prefer degraded usefulness over request failure.

## 6. Configuration

Level 3 requires explicit runtime configuration.

### Required Settings

- remote API endpoint
- API key or token

### Recommended Settings

- model identifier
- request timeout
- connect timeout
- max retries
- retry backoff policy
- maximum input bytes
- optional custom headers

Example config shape:

```yaml
semantic:
  level: auto
  remote:
    enabled: true
    endpoint: https://example.com/v1/embeddings
    model: text-embedding-3-small
    api_key_env: TOKENLESS_EMBEDDING_API_KEY
    timeout_ms: 3000
    connect_timeout_ms: 1000
    max_retries: 2
    max_input_bytes: 8192
```

### Configuration Principles

- secrets are loaded from environment variables, not committed config files
- endpoint must parse as a valid URL before startup completes
- timeout and retry settings should have safe defaults
- retries must be bounded to avoid tail-latency explosions
- remote mode must be opt-in unless a future product requirement changes that default

## 7. Error Model

A dedicated error type should separate configuration, transport, and response failures. For example:

```rust
#[derive(Debug, thiserror::Error)]
pub enum SemanticProviderError {
    #[error("provider configuration is invalid: {0}")]
    InvalidConfig(String),
    #[error("embedding request timed out")]
    Timeout,
    #[error("embedding request failed: {0}")]
    Transport(String),
    #[error("provider returned unexpected status: {0}")]
    HttpStatus(u16),
    #[error("provider response was invalid: {0}")]
    InvalidResponse(String),
    #[error("embedding vector was invalid")]
    InvalidEmbedding,
}
```

This enables better retry policy and clearer observability.

Suggested retryability policy:

- retry: timeout, connection reset, 429, 502, 503, 504
- do not retry: malformed request, authentication failure, invalid response schema, invalid endpoint config

## 8. Observability

Remote providers need better diagnostics than local rules.

At minimum, emit structured tracing fields for:

- provider name
- selected level
- fallback count
- request latency
- retry count
- response status class
- embedding dimension

Sensitive values must not be logged:

- API keys
- full request authorization headers
- raw user text where privacy policy forbids it

Useful counters and histograms:

- `semantic_provider_requests_total`
- `semantic_provider_failures_total`
- `semantic_provider_fallbacks_total`
- `semantic_provider_latency_ms`

## 9. Privacy, Cost, and Operational Risk

### Network Latency

Remote embedding adds round-trip latency and tail-risk from provider congestion.

Mitigations:

- strict timeouts
- bounded retries
- fallback to Level 2 or Level 1
- optional batching in later phases if the workflow supports it

### API Cost

Remote providers may charge per token or per request.

Mitigations:

- make Level 3 opt-in
- bound input size
- cache repeated embeddings where feasible
- expose metrics so operators can measure usage

### Privacy and Data Transfer

Remote embedding sends content off-host.

Mitigations:

- default to local levels when not explicitly configured
- document that Level 3 transmits input text to a third party
- support allowlisted endpoints only if enterprise deployments require it
- keep secrets wrapped with `secrecy` and redact debug output

### Provider Availability

Remote services can fail independently of the local application.

Mitigations:

- graceful degradation chain
- startup validation of configuration
- classify retryable vs permanent failures

## 10. Implementation Roadmap

### Phase 1: Trait and Error Types

Add the async provider trait and crate-level error model.

Likely files:

- `crates/tokenless-semantic/src/provider.rs`
- `crates/tokenless-semantic/src/error.rs`
- `crates/tokenless-semantic/src/lib.rs`

Deliverables:

- `SemanticProvider` trait
- `SemanticProviderError`
- feature gates for remote support

### Phase 2: Remote HTTP Provider

Implement the first remote provider using `reqwest` and Tokio-compatible async I/O.

Likely files:

- `crates/tokenless-semantic/src/remote.rs`
- `crates/tokenless-semantic/Cargo.toml`

Deliverables:

- config struct
- HTTP request/response parsing
- timeout and retry behavior
- unit tests with mocked HTTP responses

### Phase 3: Async Fallback Orchestrator

Add a semantic engine that tries Level 3, then Level 2, then Level 1.

Likely files:

- `crates/tokenless-semantic/src/engine.rs`
- existing semantic classification/compression entry points

Deliverables:

- `embed_with_fallback`
- structured tracing around level selection
- deterministic degradation behavior

### Phase 4: CLI and App Integration

Expose remote semantic provider configuration through the CLI and any app-level config loading.

Likely files:

- `crates/tokenless-cli/src/...`
- docs and user guide pages

Deliverables:

- config parsing
- environment variable wiring
- operator-facing docs

### Phase 5: Optimization and Hardening

After correctness is stable:

- embedding cache
- optional batching
- stricter endpoint allowlisting
- richer metrics
- benchmark remote-vs-local behavior

## 11. Open Questions

1. Should Level 2 also implement a trait adapter so all levels can be driven behind one async interface?
2. Should remote embedding support provider-specific response adapters or a single OpenAI-compatible schema first?
3. Should caching live inside `tokenless-semantic` or at the caller layer?
4. Should strict mode exist for users who want remote-only behavior with no silent fallback?

## 12. Recommendation

Adopt an async Level 3 provider trait now, without rewriting all existing semantic logic at once.

That gives the project:

- a stable extension point for hosted embeddings
- correct async handling for network I/O
- a clear degradation path that preserves local-first reliability
- a migration path toward a unified async semantic API over time

The key architectural principle is simple: remote semantics should improve capability, never become a single point of failure.
