-- Local state for one desktop install. All timestamps are Unix seconds.

CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL -- JSON
);

-- Last catalog fetched from the server.
CREATE TABLE catalog_cache (
    id         INTEGER PRIMARY KEY CHECK (id = 1),
    fetched_at INTEGER NOT NULL,
    body       TEXT NOT NULL -- JSON-encoded CatalogResponse
);

-- Mirrors of server rows, stored as their JSON wire form. Tombstones are
-- kept so fonts.rs can uninstall removed families.
CREATE TABLE library (
    key  TEXT PRIMARY KEY,
    body TEXT NOT NULL -- JSON LibraryEntry
);

CREATE TABLE collections (
    id   TEXT PRIMARY KEY,
    body TEXT NOT NULL -- JSON Collection
);

CREATE TABLE uploads (
    id   TEXT PRIMARY KEY,
    body TEXT NOT NULL -- JSON Upload
);

-- Font files Typeset installed on this machine. Typeset only ever removes
-- files listed here.
CREATE TABLE installed_files (
    family_key   TEXT NOT NULL,
    -- Google variant (e.g. `700italic`) or upload id.
    variant      TEXT NOT NULL,
    path         TEXT NOT NULL,
    -- Catalog `version` for Google fonts, sha256 for uploads.
    version      TEXT NOT NULL,
    installed_at INTEGER NOT NULL,
    PRIMARY KEY (family_key, variant)
);
