# XRM Rust Server — Coding Guidelines

You are writing production Rust for a metadata-driven XRM platform targeting GCC government clients.
These rules are non-negotiable. Apply every rule that is in scope for the current change.
Do not deviate unless the user explicitly overrides a rule in the current conversation.

---

## CRITICAL: Interaction Protocol (applies to every response)

1. **Never assume.** If any part of a request is unclear — scope, constraints, edge cases, preferences — ask specific questions before producing a solution. List all questions at once; do not ask one at a time.
2. **Always present alternatives.** When more than one valid approach exists, present each option with its advantages and disadvantages so the user can make an informed choice. Do not proceed with one approach until the user confirms.
3. **Wait for confirmation.** After presenting alternatives or asking questions, stop and wait. Do not implement anything until the user has answered.

---

## CRITICAL: Multi-Server Architecture (hard constraint, never compromise)

This system MUST be designed and implemented to run as **multiple simultaneous identical server instances** behind a load balancer at all times. This is not a future concern — every line of code must satisfy it from day one.

**Hard rules:**
- Every app node is completely stateless. No in-process mutable state survives beyond a single request.
- All persistent state lives exclusively in **PostgreSQL** (source of truth) and **Redis** (cache, sessions, pub/sub, locks).
- No in-memory structures that would diverge between nodes (no `Arc<Mutex<T>>` for shared data, no static mutable globals).
- Use `arc-swap` for read-only data that must be fast on every request (metadata, config, feature flags) — rebuilds from Postgres on invalidation signal, atomic swap on all nodes simultaneously via Redis pub/sub.
- Any coordination between nodes (cache invalidation, advisory locks, idempotency) MUST go through Redis or Postgres — never rely on same-process communication.
- A node joining or leaving the cluster must not disrupt in-flight requests on other nodes.
- Health check endpoint `/health/ready` MUST return 200 only after arc-swap pre-warm and Redis connection are confirmed — Kubernetes / load balancer uses this to gate traffic.

**What this means in practice:**
- If you are about to store something in a `static`, `lazy_static!`, `once_cell`, or any struct that lives for the lifetime of the process and can be written to at runtime — stop and redesign using Redis or Postgres.
- Read-only process-lifetime data (compiled constants, initial config loaded once at startup) is fine.
- Background jobs go through `apalis` (persistent queue in Postgres) — not `tokio::spawn` fire-and-forget, which would be lost if the node restarts.

---

## 0. Governing Priorities (in order)

1. Performance & Low Latency — every line of code is evaluated against its latency and throughput cost first; PostgreSQL is the system ceiling, not Axum; spend the performance budget on indexes, query plans, zero-copy paths, and eliminating allocations on the hot path
2. Scalability — design every component to scale horizontally without coordination; stateless handlers, shared-nothing caches where possible, no in-process state that cannot be rebuilt from PostgreSQL + Redis
3. Correctness — no silent data corruption, no stale security decisions
4. Safety — no unsafe except explicitly justified and isolated
5. Simplicity — three similar lines beats a premature abstraction

---

## 1. Workspace & Crate Rules

- The repo is a Cargo workspace. Every engine is its own crate under `crates/`.
- Crate names: `engine-{name}`, `db`, `cache`, `auth`, `bot`, `automation`, `generator`, `ai`, `config`, `server`
- A crate may only depend on crates below it in this hierarchy (no cycles):
  `server` → `engine-*` → `db`, `cache`, `config` → (no internal deps)
- Never put business logic in `server`. Handlers are thin: parse → call engine → shape response.
- If a function is used by more than one engine crate, it belongs in `engine-foundation`.

---

## 2. Race Condition Prevention — Required for All Mutating Handlers

Every `POST`/`PUT`/`DELETE` handler that changes state MUST apply all three layers:

### Layer 1 — Idempotency Key (HTTP)

- Client sends `X-Idempotency-Key` header on every mutating request.
- The `IdempotencyLayer` Tower middleware (in `server`) runs BEFORE the handler:
  1. Compute key = `SHA256(user_id + ":" + idempotency_key_header)`
  2. `Redis SET NX EX 30 <key> "pending"` — if fails (key exists): return cached response immediately
  3. On handler success: overwrite Redis key with serialized response body, EX 30s
  4. On handler error: delete Redis key so client can retry
- Apply to: all state-changing endpoints. Skip for: pure reads (`GET`, `POST /api/query/*`, `POST /api/report/execute`).
- Do NOT apply to idempotent provisioning endpoints that do their own deduplication (bot/provision, automation/provision).

### Layer 2 — PostgreSQL Advisory Lock (per user, per action)

- Required for: any operation that must not run concurrently for the same user (provisioning, workflow transitions, enrollment, record create-if-not-exists patterns).
- Acquire inside the transaction, before any data mutation:

```rust
let locked: bool = sqlx::query_scalar!(
    "SELECT pg_try_advisory_xact_lock(hashtext($1))",
    format!("{user_id}:{action_namespace}")  // e.g. "uuid:bot/provision"
)
.fetch_one(&mut tx)
.await?
.unwrap_or(false);

if !locked {
    return Err(AppError::Conflict("action already in progress for this user"));
}
```

- `pg_try_advisory_xact_lock` is NON-BLOCKING — never use `pg_advisory_lock` (blocking variant).
- The lock releases automatically on transaction commit or rollback.
- **NEVER hold the transaction open across an HTTP call, S3 upload, gRPC call, or any I/O.** Complete all external I/O after the transaction commits.
- Action namespace format: `"{crate}/{operation}"` e.g. `"bot/provision"`, `"automation/enroll"`, `"workflow/transition"`.

### Layer 3 — Optimistic Concurrency (row_version)

- Every mutable entity table MUST have a `row_version BIGINT NOT NULL DEFAULT 0` column.
- All UPDATE statements MUST include `AND row_version = $n` in the WHERE clause and increment by 1:

```sql
UPDATE core_xxx
SET    field = $1, row_version = row_version + 1, updated_at = NOW()
WHERE  id = $2 AND row_version = $3
RETURNING row_version
```

- Check `rows_affected == 0` after execute → return `AppError::VersionConflict` (HTTP 409).
- The handler returns the new `row_version` in the response so the client always has the current value.
- All read responses that return mutable records MUST include `row_version` in the JSON.
- Exception: append-only tables (audit log, nonce table) do not need `row_version`.

---

## 3. Tower Middleware Pipeline Order

In `server/src/app.rs`, layers wrap in this exact order (outermost first):

```
TraceLayer              // tracing span + request_id
RequestBodyLimitLayer   // reject oversized bodies before any parsing
TimeoutLayer            // per-request deadline
CorsLayer               // CORS headers; bypass list must be explicit
TenantResolveLayer      // identify tenant from Host/header/token; attach TenantCtx
AuthnLayer              // validate session_token JWT; attach Identity
AuthzLayer              // resolve roles + row/field scope; L2 Redis cache-backed
RateLimitLayer          // tower-governor, per-tenant + per-user limits
```

- CORS/auth bypass list is a compile-time constant in `server/src/middleware/bypass.rs`.
- Current bypass paths: `/api/v1/auth/register`, `/api/v1/auth/login`, `/api/v1/auth/google`, `/api/v1/auth/logout`, `/no-auth/*`, `/health`.
- Any new public endpoint MUST be explicitly added to the bypass list — never make the bypass list a runtime config.

---

## 4. Database Rules

### Driver selection
- Use `sqlx` query macros for **static** queries (system tables, auth, known-schema reads) — compile-time SQL verification.
- Use `tokio-postgres` directly for **dynamic** SQL generated by the metadata engine (entity queries, form queries) — `sqlx` macros cannot handle runtime-generated SQL.
- Never use an ORM. The metadata engine builds SQL directly; an ORM fights that.

### Query patterns
- All queries MUST use parameterized placeholders — never string-interpolate user data into SQL.
- All writes MUST go through a `sqlx::Transaction` — never use `&Pool` for mutations.
- Keep transactions short: begin → lock (advisory if needed) → read → write → commit. Zero external I/O inside a transaction.
- Always check `rows_affected` on UPDATE/DELETE before assuming success.

### Schema conventions
- Every entity table: `id UUID PRIMARY KEY DEFAULT gen_random_uuid()`, `created_at TIMESTAMPTZ DEFAULT NOW()`, `updated_at TIMESTAMPTZ DEFAULT NOW()`, `row_version BIGINT NOT NULL DEFAULT 0`, `is_deleted BOOL NOT NULL DEFAULT FALSE`.
- Soft delete only — never `DELETE` from entity tables. Filter `WHERE is_deleted = FALSE` in all reads.
- Custom tenant fields go in `JSONB` columns, never in EAV (`entity_attribute_value`) tables.
- Add GIN index on every JSONB column that is queried: `CREATE INDEX CONCURRENTLY ON table USING GIN (jsonb_col)`.
- `core_systemuser`: `password_hash TEXT`, `google_sub TEXT`, `auth_type TEXT`, `last_login TIMESTAMPTZ`, `user_name TEXT UNIQUE`.

### Migrations
- All schema changes via `sqlx migrate` files in `db/migrations/`.
- Migration files: `{timestamp}_{snake_case_description}.sql`. Never edit an applied migration.
- Each migration must be idempotent where possible (`CREATE TABLE IF NOT EXISTS`, `CREATE INDEX CONCURRENTLY IF NOT EXISTS`).

---

## 5. Caching Rules

| Data | Cache | Crate | Invalidation |
|------|-------|-------|-------------|
| Per-tenant metadata (entities, fields, forms, rules) | L1 arc-swap snapshot | `cache` | Version stamp bump → Redis pub/sub → atomic arc-swap rebuild on all nodes |
| Hot records, permission resolutions, computed results | L1 moka (TTL/LRU) | `cache` | Key eviction on write to that record |
| Sessions, cross-node permissions, hot records | L2 Redis (TTL) | `cache` | Redis pub/sub broadcast on write |
| Expensive read queries (entity + query hash) | L3 Redis (bounded TTL) | `cache` (Phase 2) | Pub/sub drop on any write to affected entity |
| HTTP responses (unchanged payloads) | L4 ETags | `server` (Phase 2) | Natural — ETag changes when data changes |

- Every metadata change MUST bump the per-tenant `metadata_version` stamp in Redis.
- Nodes subscribe to the `metadata_invalidation:{tenant_id}` Redis channel at startup.
- On invalidation message: rebuild arc-swap snapshot from PostgreSQL, then atomic-swap.
- Never read metadata directly from PostgreSQL in a hot path — always read from arc-swap.
- L1 moka key format: `"{tenant_id}:{entity}:{record_id}"`.

---

## 6. Error Handling

- Library crates (`engine-*`, `db`, `cache`, `auth`): use `thiserror` — typed, structured errors.
- `server` crate: `AppError` enum that implements `IntoResponse` for Axum; maps domain errors to HTTP status codes.
- Standard HTTP status mappings:

| AppError variant | HTTP |
|-----------------|------|
| `NotFound` | 404 |
| `Unauthorized` | 401 |
| `Forbidden` | 403 |
| `Conflict` / `VersionConflict` | 409 |
| `Validation(msg)` | 422 |
| `FeatureUnavailable` | 503 |
| `FeatureHidden` | 404 |
| `Internal(e)` | 500 |

- Never expose internal error details to the client. Log with `tracing::error!` server-side; return a safe message.
- Never use `unwrap()` or `expect()` except in tests and in `fn main()` startup (where panic is acceptable).

---

## 7. Authentication & Security

### Auth endpoints (bypass list)
```
POST /api/v1/auth/register   → create core_systemuser, return session_token JWT cookie
POST /api/v1/auth/login      → authenticate by username/email + password, update last_login
POST /api/v1/auth/google     → Google credential token exchange
POST /api/v1/auth/logout     → clear session_token cookie
```

### Password hashing
- Use `argon2` crate. Store hash in `core_systemuser.password_hash`.
- Never log, trace, or return password hashes. Never put password_hash in a SELECT * — always enumerate columns.

### Session tokens
- JWT, HttpOnly + Secure + SameSite=Strict cookie named `session_token`.
- Claims: `sub` (user UUID), `tenant_id`, `iat`, `exp`.
- Validate in `AuthnLayer` — reject expired/malformed tokens with 401.

### Authorization
- Security Engine resolves roles + ownership + row/field scope — this runs in `AuthzLayer`, not in handlers.
- Handlers receive a resolved `SecurityContext` from the middleware — never re-check permissions inside a handler.
- Field-level security is enforced in `engine-field` when building query results — never return a field a user cannot read.

### Automation device auth
- Device-signed requests use headers: `X-Attome-Device-Id`, `X-Attome-Device-Ts`, `X-Attome-Device-Nonce`, `X-Attome-Device-Signature`.
- Signed message format (Ed25519): `METHOD\nPATH\nTS\nNONCE\nBODY_SHA256` where BODY_SHA256 is lowercase hex SHA256.
- Replay protection: insert `(device_id, nonce)` with unique constraint into `automation_device_nonce`. Reject duplicates as 409.
- Timestamp window: reject if `|now_ms - X-Attome-Device-Ts| > 30_000ms`.

---

## 8. Multi-Project Profiles

- Startup reads `APP_PROJECT` env var. Accepted values: `attome`, `suad-crafty` (alias `suadcrafty`), `viralgens`, `noobygame`, `helpurs`. Fallback: `attome`.
- Config file loaded: `{project}_config_{debug|prod}.json` from binary working dir. Environment vars override after load.
- Project-specific routes MUST be registered conditionally:

```rust
// In server route setup:
if config.project == Project::SuadCrafty {
    router = router.merge(suad_crafty_routes());
}
```

- Never hardcode project-specific values outside `config` crate constants.
- `generator_features` map from config controls visibility per service key: `available` (default) / `coming_soon` (503) / `hidden` (404).
- The global pre-routing gate in `server` checks this map for all `/api/generator/*` and `/api/comfyui*` paths before dispatching.

---

## 9. Generator & Config JSON Rules

- Handlers for generation endpoints (`/api/generator/*`) MUST build `config_json` backend-side from validated request params using `build_canonical_config_json(...)` in `engine-foundation`.
- Never trust or persist client-provided `config_json` for generation APIs. Frontend sends only runtime fields.
- Exception: MV project save (`/api/v1/mv/project/save`) — accepts user-authored `config_json` because it stores editable project state, not generation metadata.
- Seed values (e.g. negative / random seed sentinels) MUST be normalized to valid engine seeds before Comfy submission — never pass sentinels to the engine.
- `/api/comfyui/status` response objects MUST include `s3_key` for non-HTTP resource paths.

---

## 10. Async & Tokio Rules

- Never call blocking std I/O, `std::thread::sleep`, or any synchronous network/disk call on a Tokio async thread.
- Wrap CPU-intensive or blocking work: `tokio::task::spawn_blocking(|| { ... }).await`.
- Use `tokio::time::timeout` for any external call (HTTP, gRPC, Redis, S3) — never await without a deadline.
- Background jobs go through `apalis` (Phase 1) or NATS workers (Phase 2 scale) — never `tokio::spawn` fire-and-forget for work that must be reliable.
- Channels between tasks: prefer `tokio::sync::mpsc`; use `broadcast` only for fan-out invalidation signals.

---

## 11. Observability

- Every Axum handler MUST be covered by a tracing span — `TraceLayer` in middleware handles this automatically.
- Add structured fields to the current span for every meaningful operation:

```rust
tracing::Span::current()
    .record("user_id", &user_id.to_string())
    .record("tenant_id", &tenant_id.to_string())
    .record("entity", entity_name);
```

- Use `tracing::info!` for normal operations, `tracing::warn!` for recoverable anomalies, `tracing::error!` for failures requiring attention.
- Never log: passwords, password hashes, raw JWT tokens, API keys, device private keys, or PII beyond what's needed for audit.
- Every request gets a `request_id` (UUID v4) injected by `TraceLayer` and returned as `X-Request-Id` response header.

---

## 12. API & Response Conventions

- All request/response bodies are JSON (`Content-Type: application/json`).
- Successful creates return HTTP 201 with the created record including its `id`, `row_version`, `created_at`.
- Successful updates return HTTP 200 with the updated record including new `row_version`.
- Successful deletes (soft) return HTTP 200 `{ "id": "...", "deleted": true }`.
- List responses: `{ "items": [...], "total": n, "page": n, "page_size": n }`.
- Error responses: `{ "error": "snake_case_code", "message": "human readable", "request_id": "..." }`.
- Paginated endpoints: default page_size = 50, max = 200 (matches report component paging constants).
- All timestamps in responses: ISO 8601 UTC (`2025-06-24T10:00:00Z`).
- All UUIDs in responses: lowercase hyphenated string.

---

## 13. Bot & Automation Provisioning Rules

- Bot roster is compile-time constants in `bot/src/constants.rs` — 100 bots, `bot1@attome.com` .. `bot100@attome.com`.
- `POST /api/v1/bot/provision` MUST be idempotent — safe to call multiple times. Use advisory lock with namespace `"bot/provision"`.
- Bot registration normalizes username from email local-part for auth validation, then syncs display username from constants.
- `POST /api/automation/provision` MUST be idempotent. Requires `isSecurityAdmin` for USER area, or INTERNAL area.
- Device enrollment stores Ed25519 public key as base64 raw 32-byte. Proof message: `enroll\n{token_id}\n{proof_nonce}`. Verify before storing.

---

## 14. What NOT to Do

- Do NOT use `Arc<Mutex<T>>` for metadata — use `arc-swap`. Mutex causes contention on every request.
- Do NOT use EAV tables for tenant custom fields — use JSONB.
- Do NOT load an `.so` plugin into the process — use Wasmtime sandbox (Phase 2).
- Do NOT call `pg_advisory_lock` (blocking) — only `pg_try_advisory_xact_lock` (non-blocking).
- Do NOT hold a PostgreSQL transaction open while doing HTTP/gRPC/S3/Redis I/O.
- Do NOT build SQL by string concatenation — always parameterized queries.
- Do NOT skip the idempotency key layer on mutating endpoints.
- Do NOT add new public endpoints without adding them explicitly to the bypass list constant.
- Do NOT use `SELECT *` — always enumerate columns, especially on tables containing `password_hash`.
- Do NOT add `unsafe` blocks without a comment explaining exactly why it is sound.
- Do NOT spawn fire-and-forget `tokio::spawn` for reliable work — use `apalis`.
- Do NOT put project-specific logic outside the project-gated code path.

---

## 15. Performance & Latency Rules (Priority #1)

Performance is not an afterthought — it is the first constraint. Apply these rules on every change.

### 15.1 PostgreSQL — The Real Bottleneck

- Run `EXPLAIN (ANALYZE, BUFFERS)` on every new query touching tables with >10k rows. Fix the plan before merging.
- Every JSONB column that is filtered or sorted on MUST have a GIN index:
  `CREATE INDEX CONCURRENTLY ON tbl USING GIN (col jsonb_path_ops);`
- Soft-delete tables MUST have a partial index: `CREATE INDEX CONCURRENTLY ON tbl (id) WHERE is_deleted = FALSE;`
- Never issue a `SELECT COUNT(*)` for pagination — use `SELECT COUNT(*) OVER()` window function in the same query, or maintain a counter.
- Avoid N+1 queries unconditionally. For related records use a JOIN, `json_agg`, or `array_agg` in one query, not a loop of per-row queries.
- Use `RETURNING` after INSERT/UPDATE instead of a follow-up SELECT — eliminates a round-trip.
- Prefer `COPY` for bulk inserts (import engine) — orders of magnitude faster than batched INSERTs.
- Connection pool: size = `(num_cpu_cores * 2) + effective_spindle_count`. Over-sizing starves PostgreSQL. Under-sizing queues requests. Tune with load tests, not guesses.
- Add `statement_timeout` per query class: fast reads ≤ 500ms, complex reports ≤ 5s, bulk imports ≤ 60s. Set at the session level before executing, reset after.
- Use `pg_stat_statements` in production. Any query with `mean_exec_time > 50ms` or `calls > 10_000` per hour is a tuning target.

### 15.2 Zero-Copy & Allocation Rules

- Use `bytes::Bytes` for response bodies that are already serialized (e.g. cached JSON). Never clone a large buffer to build a response.
- Prefer `&str` / `&[u8]` over `String` / `Vec<u8>` in function arguments unless ownership is required.
- Never call `.clone()` inside a loop or on a hot path without a comment justifying it.
- Use `Cow<'_, str>` for fields that are sometimes borrowed, sometimes owned (e.g. tenant-resolved strings).
- Pre-allocate `Vec` with `Vec::with_capacity(n)` when the size is known — avoids repeated reallocation.
- String formatting in the hot path: prefer `write!(buf, ...)` into a pre-allocated `String` over chained `format!()`.
- Use `smallvec::SmallVec<[T; N]>` for collections that are almost always ≤ N elements (e.g. per-request field list).

### 15.3 Axum & Tower Hot Path

- Avoid deserializing the request body twice. Extract once with `axum::extract::Json<T>` or `bytes::Bytes`, not both.
- Enable HTTP/2 on the Axum listener — multiplexing eliminates head-of-line blocking on the Angular frontend.
- Enable `tower-http::compression::CompressionLayer` for JSON responses > 1 KB — gzip/brotli typically cuts payload 70–80%.
- Set `Content-Length` on all fixed-size responses — avoids chunked encoding overhead.
- Use `axum::response::Response` with a pre-built body for frequently returned static shapes (health check, feature visibility map) — skip serde on every call.
- Middleware order matters for latency: `TraceLayer` and `RequestBodyLimitLayer` are near-zero cost. `AuthzLayer` hits Redis — cache permission decisions aggressively in `moka` (L1) before going to Redis.

### 15.4 Caching for Latency

- L1 `arc-swap` reads are **zero-cost** — a single atomic pointer load. Use arc-swap for any data read on every request (metadata, config, feature flags). Never read this data from PostgreSQL in a hot path.
- L1 `moka` is the first stop for permission resolution. Cache `(user_id, entity, action) → bool` with a 60s TTL. A cache hit here means `AuthzLayer` never touches Redis or PostgreSQL.
- Redis pipeline multiple cache reads in a single round-trip (`fred` supports pipelining). Never issue N individual Redis GETs in a loop — batch them.
- Pre-warm the arc-swap cache at startup before accepting traffic. Fail startup if pre-warm takes > 10s (indicates a DB problem).
- For list endpoints: cache the full serialized JSON response in Redis (L3, keyed by `tenant+entity+query_hash+page`) with a 5s TTL. A 5s stale window is acceptable for list views and eliminates >90% of repeat queries.

### 15.5 Concurrency & Scalability

- Handlers MUST be stateless. No `Arc<Mutex<T>>` shared across requests — use `arc-swap` for reads, PostgreSQL for writes.
- Use `tokio::join!` to parallelize independent async operations within a single request (e.g. fetch entity metadata + fetch user permissions simultaneously):
  ```rust
  let (metadata, permissions) = tokio::join!(
      cache.get_metadata(tenant_id, entity),
      security.resolve(user_id, entity, action),
  );
  ```
- For fan-out reads (e.g. load 50 related records), use `futures::future::join_all` or `tokio::task::JoinSet` — never `await` in a loop.
- Rate-limit with `tower-governor` per tenant AND per user. Shared tenant limits prevent one heavy user from starving others on the same tenant.
- Horizontal scaling: every app node is identical; state lives in PostgreSQL + Redis only. A new node joining the cluster MUST become fully operational (arc-swap pre-warm + Redis connection ready) before receiving traffic (Kubernetes readiness probe on `/health/ready`).
- Design `engine-*` crates to hold no mutable state — all state flows through `db` and `cache`. This makes the modulith trivially horizontally scalable.

### 15.6 Async Discipline

- Never `.await` inside a `rayon` thread pool or a `spawn_blocking` closure — Tokio futures cannot run there.
- Use `tokio::task::spawn_blocking` only for CPU-bound work (password hashing, WASM execution, report aggregation). Keep it scoped — move ownership in, return result out.
- Never hold a `tokio::sync::MutexGuard` across an `.await` — use `tokio::sync::Mutex` (async-aware) and keep the critical section as short as possible, or redesign to avoid shared mutable state.
- Set Tokio worker threads to `num_cpus::get()` — default is already correct; do not override unless profiling proves otherwise.
- Prefer `tokio::sync::RwLock` over `Mutex` for read-heavy shared state where `arc-swap` is not applicable. Writes should be rare.

### 15.7 Latency Budgets per Request Class

Enforce these as integration test assertions and production alerts:

| Request class | P50 target | P99 target |
|---------------|-----------|-----------|
| Auth (login/register) | < 50ms | < 200ms |
| Metadata read (arc-swap hit) | < 5ms | < 20ms |
| Simple CRUD read (moka hit) | < 10ms | < 40ms |
| Simple CRUD read (DB hit) | < 30ms | < 100ms |
| List query (L3 cache hit) | < 10ms | < 30ms |
| List query (DB hit, ≤1k rows) | < 80ms | < 300ms |
| Mutating write (with advisory lock) | < 50ms | < 200ms |
| Report execute (complex query) | < 500ms | < 2000ms |
| Generator enqueue (async, not sync) | < 30ms | < 100ms |

- Any new endpoint that cannot meet its P99 target in load testing blocks merge.
- Generator and render operations are ALWAYS async (enqueue + poll) — never block the request thread waiting for AI/Comfy output.

### 15.8 What Kills Latency — Never Do These

- Do NOT query PostgreSQL inside a loop (N+1). One query per handler, or use JOIN/array_agg.
- Do NOT hold an open PostgreSQL transaction while waiting on Redis, S3, gRPC, or HTTP calls.
- Do NOT use `Arc<RwLock<T>>` for metadata — contention under load will spike P99. Use `arc-swap`.
- Do NOT load full entity rows when you need only a subset of columns — enumerate only the columns the handler needs.
- Do NOT deserialize large JSON blobs from PostgreSQL that the handler will immediately re-serialize unchanged — return the raw `serde_json::RawValue` / `bytes::Bytes`.
- Do NOT add tower layers that do synchronous I/O (blocking DNS, file reads) to the middleware pipeline.
- Do NOT use `tokio::time::sleep` as a retry backoff on the main task — use exponential backoff in background jobs (`apalis` handles this).
- Do NOT enable `tracing` spans with `DEBUG` level in production — structured log volume is a latency tax under load. Use `INFO` in prod.
