CREATE TABLE monitors (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name TEXT NOT NULL,
    url TEXT NOT NULL,
    interval_seconds INTEGER NOT NULL,
    paused BOOLEAN NOT NULL DEFAULT FALSE,
    next_check_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_monitors_next_check_at ON monitors (next_check_at);

CREATE TABLE check_results (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    monitor_id BIGINT NOT NULL REFERENCES monitors (id) ON DELETE CASCADE,
    -- The Vercel Queues message that triggered the check. It is unique so a redelivered
    -- message can't record the same check twice.
    message_id TEXT NOT NULL UNIQUE,
    trigger TEXT NOT NULL CHECK (trigger IN ('scheduled', 'manual')),
    checked_at TIMESTAMPTZ NOT NULL,
    ok BOOLEAN NOT NULL,
    status_code INTEGER,
    latency_ms INTEGER NOT NULL,
    error TEXT
);

CREATE INDEX idx_check_results_monitor_checked_at ON check_results (monitor_id, checked_at);

CREATE TABLE worker_heartbeats (
    instance_id TEXT PRIMARY KEY,
    deployment_id TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL,
    last_seen_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_worker_heartbeats_last_seen_at ON worker_heartbeats (last_seen_at);
