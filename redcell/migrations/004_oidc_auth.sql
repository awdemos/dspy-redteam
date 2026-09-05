-- Switch from mandatory local passwords to OIDC-based authentication.
-- Existing users keep their password_hash for API key auth fallback, but new
-- OIDC users will have a NULL password_hash.
-- SQLite does not support ALTER COLUMN, so recreate the users table.
PRAGMA foreign_keys = OFF;

CREATE TABLE users_new (
    id TEXT PRIMARY KEY,
    email TEXT UNIQUE NOT NULL,
    password_hash TEXT,
    accepted_tos_version TEXT,
    accepted_tos_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO users_new (id, email, password_hash, accepted_tos_version, accepted_tos_at, created_at)
SELECT id, email, password_hash, accepted_tos_version, accepted_tos_at, created_at
FROM users;

DROP TABLE users;
ALTER TABLE users_new RENAME TO users;

PRAGMA foreign_keys = ON;

-- Short-lived OIDC state store for the PKCE flow. Rows are one-time use and
-- expire after 10 minutes.
CREATE TABLE IF NOT EXISTS oidc_state (
    state TEXT PRIMARY KEY,
    verifier TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
