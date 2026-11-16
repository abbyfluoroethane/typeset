# Typeset

A font manager that keeps the same fonts installed on all your machines.

- **Desktop app** (`apps/desktop`): Tauri 2, SvelteKit + Svelte 5, Tailwind 4, m3-svelte (Material 3). Browse Google Fonts, build a library and collections, install fonts into your user font folder.
- **Server** (`crates/server`): Rust, Axum, SQLite (sqlx). Syncs library metadata between devices, proxies the Google Fonts catalog, stores fonts you upload. Sign-in uses your own OIDC provider.
- **Shared types** (`crates/core`): wire types used by both.

The server never stores Google font files. Each device downloads them from Google when it installs them, so you always get the latest version. Fonts you upload are the only files the server keeps.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the design.

## Running the server

```sh
cp .env.example .env   # then fill in the OIDC and Google Fonts values
set -a; source .env; set +a
cargo run -p typeset-server
```

In your identity provider, create an OIDC client with redirect URI `<TYPESET_PUBLIC_URL>/auth/callback` and scopes `openid email profile`.

## Deploying the server

`deploy/deploy.sh [ssh-host]` cross-compiles a static `aarch64-unknown-linux-musl` binary and installs it on a Linux host as the systemd service in `deploy/typeset.service`. The service runs as a dynamic user, keeps its data in `/var/lib/typeset`, and reads its config from `/etc/typeset/typeset.env`. Put TLS in front of it with a reverse proxy such as Caddy.

```sh
brew install zig && cargo install cargo-zigbuild
rustup target add aarch64-unknown-linux-musl
deploy/deploy.sh typeset@bento.foid.space
```

If uploads over SSH stall (for example behind hairpin NAT), set `PULL_FROM` to this machine's LAN IP. The host then downloads the build from a temporary HTTP server and checks it against SHA-256 sums.

```sh
PULL_FROM=10.0.0.235 deploy/deploy.sh
```

## Running the desktop app

```sh
cd apps/desktop
pnpm install
pnpm tauri dev
```

On first launch, enter your server URL and sign in. The app opens your browser for the identity provider.

`pnpm dev` in a plain browser (outside Tauri) uses an in-memory mock backend, so you can work on the UI without a server. The mock reads a catalog snapshot from `src/lib/mock-catalog.json`. This file is gitignored, so create it from the Google Fonts API in the `CatalogResponse` shape.

`pnpm tauri build` builds the app bundle into `target/release/bundle/`.

## Development

```sh
cargo test --workspace                    # Rust tests
cd apps/desktop && pnpm check             # Svelte/TypeScript checks
```
