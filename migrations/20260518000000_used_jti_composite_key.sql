-- Replace used_jti table with composite primary key (jti, has_groups)
-- to support ALLOW_REPLAY mode: same JTI can be used once with groups
-- and once without groups.

CREATE TABLE used_jti_new (
    jti         TEXT NOT NULL,
    has_groups  INTEGER NOT NULL DEFAULT 0,
    exp         INTEGER NOT NULL,
    PRIMARY KEY (jti, has_groups)
);

-- Migrate existing rows as "no groups" entries
INSERT OR IGNORE INTO used_jti_new (jti, has_groups, exp)
SELECT jti, 0, exp FROM used_jti;

DROP TABLE used_jti;
ALTER TABLE used_jti_new RENAME TO used_jti;

CREATE INDEX IF NOT EXISTS idx_used_jti_exp ON used_jti(exp);
