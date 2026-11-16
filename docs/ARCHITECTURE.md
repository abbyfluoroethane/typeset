# Architecture

## Goals

- One person (or a few, up to about 10) keeps the same fonts on several machines.
- Google font files always come from Google, fresh. The server holds metadata only.
- Fonts that users upload sync through the server, because they have no other source.
- Small to run: one Rust binary and one SQLite file.

## Components

```
┌──────────────────────┐          ┌──────────────────────┐        ┌──────────────┐
│ Desktop (Tauri)      │  HTTPS   │ typeset-server       │ HTTPS  │ Google Fonts │
│ Svelte UI ── invoke ─┼─────────▶│ Axum + SQLite        │───────▶│ Developer API│
│ Rust: api/sync/fonts │  bearer  │ catalog cache, sync, │        └──────────────┘
│ local SQLite mirror  │          │ uploads, OIDC        │        ┌──────────────┐
└──────────┬───────────┘          └──────────┬───────────┘        │ Your OIDC    │
           │ TTF downloads                   └───────────────────▶│ provider     │
           ▼                                                      └──────────────┘
     fonts.gstatic.com
```

## Sign-in

The server is the only OIDC client. The desktop app never talks to the identity provider, so the IdP needs one redirect URI.

1. The desktop app listens on `127.0.0.1:<random port>` and opens
   `<server>/auth/login?redirect_uri=http://127.0.0.1:<port>/callback&state=…&code_challenge=…` in the browser.
2. The server redirects to the IdP (authorization code flow, PKCE, nonce).
3. On `/auth/callback`, the server verifies the ID token and creates or updates the user. If `TYPESET_ALLOWED_EMAILS` is set, it checks the user's email against it. It then redirects to the loopback URL with a one-time code that is valid for 60 seconds.
4. The app sends the code and its PKCE verifier to `POST /auth/token`. It gets back a session token and stores it in the OS keychain.

Sessions last 90 days and renew while in use. The server stores only SHA-256 hashes of codes and tokens. It accepts only loopback redirect URIs.

## Setup and admin page

The server serves one web page at `/admin` (three static files compiled into the binary, no build step). It has two jobs.

**Setup.** The public URL, the OIDC client and the Google Fonts key are settings. Each comes from its `TYPESET_*` environment variable if that is set, otherwise from the `settings` table. The server starts without them: sign-in and the catalog answer 503, and the page shows a setup form.

- Setup is open while the server is not configured or has no admin. At startup in that state, the server makes a random setup token, logs it, and keeps only its hash in memory.
- The page accepts the token by link (`/setup?token=…`) or by paste, and keeps it in an HttpOnly cookie. Reading the server log is the proof that you run the server.
- Saving tests the issuer (OIDC discovery) and the key (a catalog fetch) when they change, and rejects values that fail. New settings apply at once.
- The user who signs in from the setup page becomes the first admin. After that the token is worth nothing.

**Admin.** Admins sign in through the same IdP round trip as the desktop app. The pending login is marked `web`, so `/auth/callback` ends with a session cookie (7 days, `SameSite=Lax`) and a redirect to `/admin`, not a one-time code. Admins can change settings, make other users admins, delete users with their data, sign devices out and refresh the catalog. An admin cannot demote or delete their own account, so one admin always remains.

Every state change is a JSON `POST`, `PUT` or `DELETE`. `SameSite=Lax` keeps the cookies off cross-site requests of those kinds. The page's content security policy allows only its own script and stylesheet.

## Sync

- Every write on the server takes the next value from one global counter (`rev`).
- `GET /api/sync?since=N` returns every library entry, collection and upload of the user with `rev > N`. This includes deletes, which are tombstones.
- The client stores the highest `rev` and sends it on the next sync.
- If two writes conflict, the last write wins. This is enough for a few users with a few devices each.
- The desktop app writes to the server directly. It has no offline write queue yet.

A library entry is keyed by `google:<Family>` or `upload:<Family>`. For Google families, it can limit which variants to install. An empty list means all variants.

## Installing fonts

`fonts::reconcile` makes the installed fonts on the machine match the synced library:

- It installs live entries that are not installed yet.
- It reinstalls a Google family when its catalog `version` changes, so each device gets the current files from Google. It reinstalls an upload family when its set of files changes.
- It removes families whose library entry is deleted.

The app installs fonts into the user's font folder, so it needs no admin rights. On Windows it also registers fonts under HKCU. The app keeps a record of every file it installs in `installed_files` and only deletes files from that list.

## Data

| Server table | Contents |
| --- | --- |
| `users`, `sessions`, `pending_logins`, `auth_codes` | Auth |
| `settings` | Settings saved from the setup page |
| `library_entries`, `collections`, `uploads` | Synced user data, with `rev` and tombstones |
| `rev_counter` | The global revision counter |
| `catalog_cache` | Last Google Fonts catalog (metadata JSON) |

The server stores uploaded font files at `<data_dir>/uploads/<sha256>`.

The desktop app keeps a local mirror of the synced tables, the catalog cache, settings and `installed_files` in the app data folder.
