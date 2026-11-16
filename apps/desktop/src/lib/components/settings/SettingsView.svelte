<script lang="ts">
  import { Button, Dialog, Icon, Switch, TextField } from "m3-svelte";
  import iconLogout from "@ktibow/iconset-material-symbols/logout-rounded";
  import iconCheck from "@ktibow/iconset-material-symbols/check-rounded";
  import iconSystem from "@ktibow/iconset-material-symbols/brightness-auto-rounded";
  import iconLight from "@ktibow/iconset-material-symbols/light-mode-rounded";
  import iconDark from "@ktibow/iconset-material-symbols/dark-mode-rounded";
  import PageHeader from "$lib/components/common/PageHeader.svelte";
  import { app } from "$lib/state.svelte";
  import { SEED_PRESETS, theme, type Scheme } from "$lib/theme.svelte";

  let url = $state(app.settings.serverUrl ?? "");
  const normalized = $derived(url.trim().replace(/\/+$/, ""));
  const changed = $derived(normalized !== (app.settings.serverUrl ?? ""));
  const urlError = $derived.by(() => {
    if (!normalized) return null;
    try {
      const u = new URL(normalized);
      return u.protocol === "http:" || u.protocol === "https:"
        ? null
        : "The address must start with http:// or https://";
    } catch {
      return "That doesn't look like a web address, e.g. https://fonts.example.com";
    }
  });
  const isPreset = $derived(SEED_PRESETS.some((p) => p.color.toLowerCase() === theme.seed.toLowerCase()));
  let confirmSignOut = $state(false);
  let saving = $state(false);

  const schemes: { value: Scheme; label: string; icon: typeof iconSystem }[] = [
    { value: "system", label: "System", icon: iconSystem },
    { value: "light", label: "Light", icon: iconLight },
    { value: "dark", label: "Dark", icon: iconDark },
  ];

  async function save(e?: SubmitEvent) {
    e?.preventDefault();
    if (!changed || urlError || saving) return;
    saving = true;
    await app.saveServerUrl(url);
    url = app.settings.serverUrl ?? "";
    saving = false;
  }

  const initial = $derived((app.user?.name ?? app.user?.email ?? "?").trim().charAt(0).toUpperCase());
</script>

<div class="h-full overflow-y-auto">
  <PageHeader title="Settings" />

  <div class="mx-auto max-w-2xl space-y-8 px-6 pt-2 pb-8">
    <section aria-labelledby="set-appearance">
      <h2 id="set-appearance" class="m3-font-title-small mb-2 px-1 text-primary">Appearance</h2>
      <div class="group">
        <div class="row stack">
          <div class="text">
            <p class="m3-font-body-large" id="seed-label">Accent color</p>
            <p class="m3-font-body-medium text-on-surface-variant">Colors buttons and highlights, with a faint tint on everything else.</p>
          </div>
          <div class="flex flex-wrap items-center gap-3 p-1" role="radiogroup" aria-labelledby="seed-label">
            {#each SEED_PRESETS as p (p.color)}
              {@const active = theme.seed.toLowerCase() === p.color.toLowerCase()}
              <button
                type="button"
                role="radio"
                aria-checked={active}
                aria-label={p.name}
                title={p.name}
                class="swatch"
                class:active
                style:background-color={p.color}
                onclick={() => theme.setSeed(p.color)}
              >
                {#if active}<Icon icon={iconCheck} size={24} />{/if}
              </button>
            {/each}
            <label class="swatch custom" class:active={!isPreset} title="Custom color">
              <span class="sr-only">Custom accent color</span>
              <input
                type="color"
                value={theme.seed}
                oninput={(e) => theme.setSeed(e.currentTarget.value)}
              />
              {#if !isPreset}<Icon icon={iconCheck} size={24} />{/if}
            </label>
          </div>
        </div>

        <div class="row stack">
          <div class="text">
            <p class="m3-font-body-large" id="scheme-label">Theme</p>
            <p class="m3-font-body-medium text-on-surface-variant">System follows your computer's appearance.</p>
          </div>
          <div class="schemes" role="radiogroup" aria-labelledby="scheme-label">
            {#each schemes as s (s.value)}
              <label class="scheme" class:active={theme.scheme === s.value}>
                <input
                  type="radio"
                  name="scheme"
                  value={s.value}
                  checked={theme.scheme === s.value}
                  onchange={() => theme.setScheme(s.value)}
                />
                <Icon icon={s.icon} size={20} />
                <span class="m3-font-label-large">{s.label}</span>
              </label>
            {/each}
          </div>
        </div>
      </div>
    </section>

    <section aria-labelledby="set-sync">
      <h2 id="set-sync" class="m3-font-title-small mb-2 px-1 text-primary">Sync &amp; server</h2>
      <div class="group">
        <form class="row stack" onsubmit={save}>
          <div class="text">
            <p class="m3-font-body-large">Server URL</p>
            <p class="m3-font-body-medium text-on-surface-variant">Where your Typeset library is hosted.</p>
          </div>
          <div class="field">
            <TextField
              label="Server URL"
              type="url"
              bind:value={url}
              error={!!urlError && changed}
              placeholder="https://fonts.example.com"
              autocomplete="url"
              spellcheck={false}
            />
            {#if urlError && changed}
              <p class="error-text m3-font-body-small" role="alert">{urlError}</p>
            {/if}
          </div>
          {#if changed}
            <div class="actions">
              <Button type="button" variant="text" onclick={() => (url = app.settings.serverUrl ?? "")}>Revert</Button>
              <Button type="submit" variant="filled" disabled={!!urlError || saving}>Save</Button>
            </div>
          {/if}
        </form>

        <label class="row clickable">
          <div class="text">
            <p class="m3-font-body-large">Install automatically</p>
            <p class="m3-font-body-medium text-on-surface-variant">
              Keep fonts on this computer in step with your synced library.
            </p>
          </div>
          <Switch
            checked={app.settings.autoInstall}
            onchange={(e) => app.setAutoInstall(e.currentTarget.checked)}
          />
        </label>
      </div>
    </section>

    <section aria-labelledby="set-account">
      <h2 id="set-account" class="m3-font-title-small mb-2 px-1 text-primary">Account</h2>
      <div class="group">
        <div class="row">
          <div class="avatar m3-font-title-large" aria-hidden="true">{initial}</div>
          <div class="text">
            <p class="m3-font-body-large truncate">{app.user?.name ?? app.user?.email ?? "Signed in"}</p>
            {#if app.user?.name && app.user.email}
              <p class="m3-font-body-medium truncate text-on-surface-variant">{app.user.email}</p>
            {/if}
          </div>
          <span class="tone-error">
            <Button variant="outlined" iconType="left" onclick={() => (confirmSignOut = true)}>
              <Icon icon={iconLogout} />
              Sign out
            </Button>
          </span>
        </div>
      </div>
    </section>
  </div>
</div>

<Dialog headline="Sign out of Typeset?" icon={iconLogout} bind:open={confirmSignOut}>
  <p>Fonts already installed stay on this computer. Your library will sync again next time you sign in.</p>
  {#snippet buttons()}
    <Button variant="text" onclick={() => (confirmSignOut = false)}>Cancel</Button>
    <span class="tone-error">
      <Button
        variant="filled"
        onclick={() => {
          confirmSignOut = false;
          void app.signOut();
        }}>Sign out</Button
      >
    </span>
  {/snippet}
</Dialog>

<style>
  .field {
    flex: 1;
    min-width: 0;
  }
  .field :global(.m3-container) {
    display: flex;
    width: 100%;
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
  .swatch {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 48px;
    height: 48px;
    padding: 0;
    border: none;
    border-radius: var(--m3-shape-full);
    color: white;
    cursor: pointer;
    transition:
      border-radius var(--m3-easing-spatial),
      scale var(--m3-easing-spatial);
  }
  .swatch:hover {
    border-radius: var(--m3-shape-large);
  }
  .swatch.active {
    border-radius: var(--m3-shape-large);
    scale: 1.12;
    outline: 3px solid var(--m3c-on-surface);
    outline-offset: 3px;
  }
  .swatch:focus-visible,
  .swatch:has(input:focus-visible) {
    outline: 3px solid var(--m3c-primary);
    outline-offset: 3px;
  }
  .swatch.custom {
    background: conic-gradient(#e5484d, #ffb224, #30a46c, #0090ff, #7c4dff, #d6409f, #e5484d);
    overflow: hidden;
  }
  .swatch.custom input {
    position: absolute;
    inset: 0;
    opacity: 0;
    width: 100%;
    height: 100%;
    cursor: pointer;
  }
  .schemes {
    display: flex;
    gap: 2px;
  }
  .scheme {
    position: relative;
    display: flex;
    flex: 1;
    align-items: center;
    justify-content: center;
    gap: 8px;
    height: 48px;
    cursor: pointer;
    background: var(--m3c-surface-container-highest);
    color: var(--m3c-on-surface-variant);
    border-radius: var(--m3-shape-extra-small);
    transition:
      background-color var(--m3-easing),
      border-radius var(--m3-easing-spatial);
  }
  .scheme:first-child {
    border-start-start-radius: var(--m3-shape-full);
    border-end-start-radius: var(--m3-shape-full);
  }
  .scheme:last-child {
    border-start-end-radius: var(--m3-shape-full);
    border-end-end-radius: var(--m3-shape-full);
  }
  .scheme.active {
    background: var(--m3c-primary);
    color: var(--m3c-on-primary);
    border-radius: var(--m3-shape-full);
  }
  .scheme input {
    position: absolute;
    inset: 0;
    opacity: 0;
    margin: 0;
    cursor: pointer;
  }
  .scheme:has(input:focus-visible) {
    outline: 3px solid var(--m3c-tertiary);
    outline-offset: 2px;
  }
  @media (prefers-reduced-motion: reduce) {
    .swatch,
    .scheme {
      transition: none;
    }
  }
  .group {
    border-radius: var(--m3-shape-large);
    background: var(--m3c-surface-container);
    overflow: hidden;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 16px;
    min-height: 72px;
    padding: 16px 20px;
  }
  .row + .row {
    border-top: 1px solid var(--m3c-outline-variant);
  }
  .row.clickable {
    cursor: pointer;
  }
  .row.stack {
    flex-direction: column;
    align-items: stretch;
    gap: 12px;
  }
  .text {
    flex: 1;
    min-width: 0;
  }
  .text p {
    margin: 0;
  }
  .avatar {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 48px;
    height: 48px;
    flex-shrink: 0;
    border-radius: var(--m3-shape-full);
    background: var(--m3c-primary);
    color: var(--m3c-on-primary);
  }
  .error-text {
    margin: 4px 0 0 16px;
    color: var(--m3c-error);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>
