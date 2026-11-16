-- Typeset server schema. All timestamps are Unix seconds.

CREATE TABLE users (
    id         TEXT PRIMARY KEY,
    issuer     TEXT NOT NULL,
    subject    TEXT NOT NULL,
    email      TEXT,
    name       TEXT,
    created_at INTEGER NOT NULL,
    UNIQUE (issuer, subject)
);

-- Bearer tokens handed to desktop clients. Only the SHA-256 is stored.
CREATE TABLE sessions (
    token_hash   TEXT PRIMARY KEY,
    user_id      TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    device_name  TEXT,
    created_at   INTEGER NOT NULL,
    last_used_at INTEGER NOT NULL,
    expires_at   INTEGER NOT NULL
);

-- A login in flight between /auth/login and /auth/callback. `id` is the
-- OIDC `state` sent to the identity provider.
CREATE TABLE pending_logins (
    id                    TEXT PRIMARY KEY,
    client_redirect_uri   TEXT NOT NULL,
    client_state          TEXT NOT NULL,
    client_code_challenge TEXT NOT NULL,
    oidc_pkce_verifier    TEXT NOT NULL,
    nonce                 TEXT NOT NULL,
    created_at            INTEGER NOT NULL
);

-- One-time codes the desktop client exchanges at /auth/token.
CREATE TABLE auth_codes (
    code_hash      TEXT PRIMARY KEY,
    user_id        TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    code_challenge TEXT NOT NULL,
    expires_at     INTEGER NOT NULL
);

-- Single server-wide revision counter used for incremental sync.
CREATE TABLE rev_counter (
    id    INTEGER PRIMARY KEY CHECK (id = 1),
    value INTEGER NOT NULL
);
INSERT INTO rev_counter (id, value) VALUES (1, 0);

CREATE TABLE library_entries (
    user_id    TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    key        TEXT NOT NULL,
    source     TEXT NOT NULL CHECK (source IN ('google', 'upload')),
    family     TEXT NOT NULL,
    variants   TEXT NOT NULL DEFAULT '[]', -- JSON array of strings
    favorite   INTEGER NOT NULL DEFAULT 0,
    deleted    INTEGER NOT NULL DEFAULT 0,
    rev        INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, key)
);
CREATE INDEX library_entries_user_rev ON library_entries (user_id, rev);

CREATE TABLE collections (
    id          TEXT PRIMARY KEY,
    user_id     TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    family_keys TEXT NOT NULL DEFAULT '[]', -- JSON array of family keys
    deleted     INTEGER NOT NULL DEFAULT 0,
    rev         INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
CREATE INDEX collections_user_rev ON collections (user_id, rev);

-- Metadata for user-uploaded font files. The bytes live on disk at
-- <data_dir>/uploads/<sha256>.
CREATE TABLE uploads (
    id         TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    family     TEXT NOT NULL,
    style      TEXT NOT NULL,
    weight     INTEGER NOT NULL,
    italic     INTEGER NOT NULL,
    filename   TEXT NOT NULL,
    sha256     TEXT NOT NULL,
    size       INTEGER NOT NULL,
    deleted    INTEGER NOT NULL DEFAULT 0,
    rev        INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX uploads_user_rev ON uploads (user_id, rev);
CREATE INDEX uploads_sha256 ON uploads (sha256);

-- Last Google Fonts catalog response (metadata only).
CREATE TABLE catalog_cache (
    id         INTEGER PRIMARY KEY CHECK (id = 1),
    fetched_at INTEGER NOT NULL,
    body       TEXT NOT NULL -- JSON-encoded CatalogResponse
);
