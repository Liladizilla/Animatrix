-- name: init_tables
-- Create all tables needed for the Animatrix workspace.

CREATE TABLE IF NOT EXISTS channels (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    brand_json TEXT NOT NULL,
    style_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    channel_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS assets (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    channel_id TEXT NOT NULL,
    asset_type TEXT NOT NULL,
    source TEXT NOT NULL,
    provider TEXT,
    model TEXT,
    path TEXT NOT NULL,
    hash TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    metadata TEXT NOT NULL,
    created_at TEXT NOT NULL,
    parent_asset TEXT
);

CREATE TABLE IF NOT EXISTS providers (
    id TEXT PRIMARY KEY,
    provider_kind TEXT NOT NULL,
    health TEXT NOT NULL,
    last_probe_at TEXT,
    p95_latency_ms INTEGER,
    failure_rate REAL NOT NULL DEFAULT 0.0,
    quota_remaining INTEGER,
    rate_limit_reset_at TEXT,
    updated_at TEXT NOT NULL
);

-- name: jobs_table
-- Job persistence and tracking.

CREATE TABLE IF NOT EXISTS jobs (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    graph_node_id TEXT,
    job_type TEXT NOT NULL,
    status TEXT NOT NULL,
    generation_status TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 5,
    attempts INTEGER NOT NULL DEFAULT 0,
    idempotency_key TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    inputs TEXT NOT NULL,
    output_requirements TEXT NOT NULL,
    cost_ceiling TEXT,
    approval_gate TEXT,
    progress INTEGER NOT NULL DEFAULT 0,
    error TEXT,
    error_code TEXT,
    retryable INTEGER NOT NULL DEFAULT 0,
    retry_after_seconds INTEGER,
    metrics TEXT,
    started_at TEXT,
    finished_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_jobs_project ON jobs(project_id);
CREATE INDEX IF NOT EXISTS idx_jobs_status ON jobs(status);

-- name: job_attempts_table
-- Individual attempt records for each job.

CREATE TABLE IF NOT EXISTS job_attempts (
    id TEXT PRIMARY KEY,
    job_id TEXT NOT NULL,
    worker_id TEXT,
    transport TEXT NOT NULL,
    request TEXT NOT NULL,
    response TEXT,
    logs TEXT NOT NULL DEFAULT '[]',
    metrics TEXT,
    status TEXT NOT NULL,
    started_at TEXT,
    finished_at TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_job_attempts_job ON job_attempts(job_id);

-- name: events_table
-- Domain events for project timeline and audit.

CREATE TABLE IF NOT EXISTS events (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    job_id TEXT,
    event_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_events_project ON events(project_id);

-- name: outbox_table
-- Outbox pattern for reliable event propagation.

CREATE TABLE IF NOT EXISTS outbox (
    id TEXT PRIMARY KEY,
    event_id TEXT NOT NULL,
    project_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    created_at TEXT NOT NULL,
    dispatched_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_outbox_pending ON outbox(dispatched_at) WHERE dispatched_at IS NULL;
