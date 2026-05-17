CREATE TABLE IF NOT EXISTS used_jti (
    jti     TEXT PRIMARY KEY,
    exp     INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_used_jti_exp ON used_jti(exp);
