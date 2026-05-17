CREATE TABLE IF NOT EXISTS request_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp   TEXT NOT NULL DEFAULT (datetime('now')),
    source_ip   TEXT NOT NULL,
    inbound_sub    TEXT,
    inbound_iss    TEXT,
    inbound_aud    TEXT,
    validation  TEXT NOT NULL,
    error_detail TEXT,
    outbound_sub    TEXT,
    token_jti   TEXT,
    response_code INTEGER NOT NULL,
    elapsed_ms  INTEGER NOT NULL,
    exported_to_splunk INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_request_log_timestamp ON request_log(timestamp);
CREATE INDEX IF NOT EXISTS idx_request_log_inbound_sub ON request_log(inbound_sub);

CREATE TABLE IF NOT EXISTS used_jti (
    jti         TEXT PRIMARY KEY,
    exp         INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_used_jti_exp ON used_jti(exp);

CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT OR IGNORE INTO schema_version (version) VALUES (1);
