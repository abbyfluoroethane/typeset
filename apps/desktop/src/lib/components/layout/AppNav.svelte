<script lang="ts">
  import { onMount } from "svelte";
  import { Icon } from "m3-svelte";
  import iconBrowse from "@ktibow/iconset-material-symbols/explore-rounded";
  import iconLibrary from "@ktibow/iconset-material-symbols/library-books-rounded";
  import iconCollections from "@ktibow/iconset-material-symbols/folder-copy-rounded";
  import iconUploads from "@ktibow/iconset-material-symbols/upload-rounded";
  import iconSettings from "@ktibow/iconset-material-symbols/settings-rounded";
  import iconSync from "@ktibow/iconset-material-symbols/sync-rounded";
  import { app, type View } from "$lib/state.svelte";
  import Logo from "$lib/components/common/Logo.svelte";

  const items: { view: View; label: string; icon: typeof iconBrowse }[] = [
    { view: "browse", label: "Browse", icon: iconBrowse },
    { view: "library", label: "Library", icon: iconLibrary },
    { view: "collections", label: "Collections", icon: iconCollections },
    { view: "uploads", label: "Uploads", icon: iconUploads },
  ];

  // Ticks so "2 min ago" stays fresh without a sync happening.
  let now = $state(Date.now());
  onMount(() => {
    const t = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(t);
  });

  function ago(unixSeconds: number): string {
    const s = Math.max(0, Math.round((now - unixSeconds * 1000) / 1000));
    if (s < 60) return "just now";
    const m = Math.round(s / 60);
    if (m < 60) return `${m} min ago`;
    const h = Math.round(m / 60);
    if (h < 24) return `${h} h ago`;
    return new Date(unixSeconds * 1000).toLocaleDateString();
  }

  const status = $derived(
    app.syncing
      ? "Syncing…"
      : app.lastSyncError
        ? `Last sync failed: ${app.lastSyncError}. Click to retry`
        : app.local.lastSyncedAt
          ? `Synced ${ago(app.local.lastSyncedAt)}. Click to sync now`
          : "Not synced yet. Click to sync now",
  );
</script>

{#snippet dest(view: View, label: string, icon: typeof iconBrowse)}
  <button
    type="button"
    class="dest"
    class:active={app.view === view}
    aria-current={app.view === view ? "page" : undefined}
    onclick={() => (app.view = view)}
  >
    <span class="pill"><Icon {icon} size={24} /></span>
    <span class="m3-font-label-medium">{label}</span>
  </button>
{/snippet}

<nav class="rail" aria-label="Main">
  <Logo />

  <div class="dests">
    {#each items as item (item.view)}
      {@render dest(item.view, item.label, item.icon)}
    {/each}
  </div>

  <div class="bottom">
    <button
      type="button"
      class="sync"
      class:spinning={app.syncing}
      class:failed={!!app.lastSyncError && !app.syncing}
      disabled={app.syncing}
      onclick={() => app.sync()}
      title={status}
      aria-label={status}
    >
      <Icon icon={iconSync} size={24} />
      {#if app.lastSyncError && !app.syncing}<span class="badge" aria-hidden="true"></span>{/if}
    </button>
    {@render dest("settings", "Settings", iconSettings)}
  </div>
</nav>

<style>
  .rail {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 80px;
    flex-shrink: 0;
    height: 100%;
    padding: 16px 0 12px;
    gap: 24px;
  }
  .dests,
  .bottom {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }
  .dests {
    flex: 1;
  }
  .bottom {
    gap: 8px;
  }
  .dest {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    width: 72px;
    padding: 4px 0;
    border: none;
    background: none;
    border-radius: var(--m3-shape-large);
    font-family: inherit;
    color: var(--m3c-on-surface-variant);
    cursor: pointer;
  }
  .pill {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 56px;
    height: 32px;
    border-radius: var(--m3-shape-full);
    transition:
      background-color var(--m3-easing),
      color var(--m3-easing);
  }
  .dest:hover:not(.active) .pill {
    background: color-mix(in oklab, var(--m3c-on-surface) 8%, transparent);
  }
  .dest.active {
    color: var(--m3c-on-surface);
    font-weight: 600;
  }
  .dest.active .pill {
    background: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
  }
  .dest:focus-visible {
    outline: 3px solid var(--m3c-primary);
    outline-offset: 0;
  }
  .sync {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 40px;
    height: 40px;
    border: none;
    border-radius: var(--m3-shape-full);
    background: none;
    color: var(--m3c-on-surface-variant);
    cursor: pointer;
  }
  .sync:hover:not(:disabled) {
    background: color-mix(in oklab, var(--m3c-on-surface) 8%, transparent);
  }
  .sync:disabled {
    cursor: progress;
  }
  .sync:focus-visible {
    outline: 3px solid var(--m3c-primary);
    outline-offset: 0;
  }
  .sync.failed {
    color: var(--m3c-error);
  }
  .badge {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 8px;
    height: 8px;
    border-radius: var(--m3-shape-full);
    background: var(--m3c-error);
    outline: 2px solid var(--m3c-surface-container);
  }
  .sync.spinning :global(svg) {
    animation: spin 1.2s linear infinite;
  }
  @keyframes spin {
    to {
      rotate: -360deg;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .pill {
      transition: none;
    }
    .sync.spinning :global(svg) {
      animation: none;
    }
    .sync.spinning {
      opacity: 0.6;
    }
  }
</style>
