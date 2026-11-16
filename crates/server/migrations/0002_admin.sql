-- Settings saved from the web setup page. Environment variables win over
-- these rows.
CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Admins can open the web admin page.
ALTER TABLE users ADD COLUMN is_admin INTEGER NOT NULL DEFAULT 0;

-- 'desktop' logins end at the app's loopback URL. 'web' logins end at the
-- admin page with a session cookie; 'web-claim' also makes the user the
-- first admin.
ALTER TABLE pending_logins ADD COLUMN kind TEXT NOT NULL DEFAULT 'desktop';
