<script lang="ts">
  import { Button, Chip, Dialog, Icon, LoadingIndicator } from "m3-svelte";
  import iconSync from "@ktibow/iconset-material-symbols/sync";
  import iconFavorite from "@ktibow/iconset-material-symbols/favorite";
  import iconFavoriteOutline from "@ktibow/iconset-material-symbols/favorite-outline";
  import iconLibrary from "@ktibow/iconset-material-symbols/library-books";
  import iconSearch from "@ktibow/iconset-material-symbols/search";
  import iconClose from "@ktibow/iconset-material-symbols/close";
  import iconCloudOff from "@ktibow/iconset-material-symbols/cloud-off";
  import iconDelete from "@ktibow/iconset-material-symbols/delete";
  import { parseKey } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import PageHeader from "$lib/components/common/PageHeader.svelte";
  import EmptyState from "$lib/components/common/EmptyState.svelte";
  import FamilyRow from "./FamilyRow.svelte";

  type Filter = "all" | "favorites" | "not-installed";
  let filter = $state<Filter>("all");
  let query = $state("");

  // Ticks so "Synced 2 min ago" stays fresh.
  let now = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(t);
  });

  function ago(unixSeconds: number): string {
    const s = Math.max(0, Math.round((now - unixSeconds * 1000) / 1000));
    if (s < 60) return "just now";
    const m = Math.floor(s / 60);
    if (m < 60) return `${m} min ago`;
    const h = Math.floor(m / 60);
    if (h < 24) return `${h} hr ago`;
    const d = Math.floor(h / 24);
    return `${d} day${d === 1 ? "" : "s"} ago`;
  }

  const name = (key: string) => parseKey(key)?.family ?? key;

  const entries = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return app.library
      .filter((e) => filter !== "favorites" || e.favorite)
      .filter((e) => filter !== "not-installed" || !app.installedByKey.has(e.key))
      .filter((e) => !q || name(e.key).toLowerCase().includes(q))
      .toSorted((a, b) => a.family.localeCompare(b.family));
  });
  const installedCount = $derived(app.library.filter((e) => app.installedByKey.has(e.key)).length);
  const total = $derived(app.library.length);
  const syncText = $derived(
    app.syncing
      ? "Syncing…"
      : app.local.lastSyncedAt
        ? `Synced ${ago(app.local.lastSyncedAt)}`
        : "Not synced yet",
  );

  const filters: { id: Filter; label: string }[] = [
    { id: "all", label: "All" },
    { id: "favorites", label: "Favorites" },
    { id: "not-installed", label: "Not installed" },
  ];

  // Confirmations for the two destructive row actions.
  let confirmKind = $state<"uninstall" | "remove" | null>(null);
  let confirmKey = $state<string | null>(null);
  let confirmOpen = $state(false);
  $effect(() => {
    if (!confirmOpen) confirmKind = confirmKey = null;
  });
  function ask(kind: "uninstall" | "remove", key: string) {
    confirmKind = kind;
    confirmKey = key;
    confirmOpen = true;
  }
  async function confirm() {
    const key = confirmKey;
    const kind = confirmKind;
    confirmOpen = false;
    if (!key) return;
    if (kind === "uninstall") await app.uninstall(key);
    else await app.removeFromLibrary(key);
  }
</script>

<div class="flex h-full flex-col">
  <PageHeader title="Library">
    {#snippet summary()}
      <span aria-live="polite"
        >{total} {total === 1 ? "family" : "families"} · {installedCount} installed on this Mac · {syncText}</span
      >
    {/snippet}
    {#snippet actions()}
      <Button variant="tonal" size="m" iconType="left" disabled={app.syncing} onclick={() => app.sync()}>
        {#if app.syncing}
          <LoadingIndicator size={24} center={false} aria-label="Syncing" />
        {:else}
          <Icon icon={iconSync} />
        {/if}
        {app.syncing ? "Syncing…" : "Sync now"}
      </Button>
    {/snippet}
    {#snippet toolbar()}
      {#each filters as f (f.id)}
        <Chip
          variant="general"
          selected={filter === f.id}
          aria-pressed={filter === f.id}
          icon={f.id === "favorites" ? (filter === f.id ? iconFavorite : iconFavoriteOutline) : undefined}
          onclick={() => (filter = f.id)}>{f.label}</Chip
        >
      {/each}
      <div class="search ml-auto">
        <Icon icon={iconSearch} size={20} />
        <input
          type="search"
          class="m3-font-body-large"
          placeholder="Search library"
          aria-label="Search library"
          bind:value={query}
        />
        {#if query}
          <button type="button" class="clear" aria-label="Clear search" onclick={() => (query = "")}>
            <Icon icon={iconClose} size={18} />
          </button>
        {/if}
      </div>
    {/snippet}
  </PageHeader>

  <div class="min-h-0 flex-1 overflow-y-auto px-6 pb-6">
    {#if entries.length > 0}
      <ul class="flex flex-col gap-2">
        {#each entries as e (e.key)}
          <li>
            <FamilyRow
              familyKey={e.key}
              onuninstall={(k) => ask("uninstall", k)}
              onremove={(k) => ask("remove", k)}
            />
          </li>
        {/each}
      </ul>
    {:else if total === 0}
      <EmptyState
        icon={iconLibrary}
        title="Your library is empty"
        body="Add families from Browse or upload your own fonts, and they will show up here."
      >
        {#snippet action()}
          <Button variant="filled" onclick={() => (app.view = "browse")}>Browse fonts</Button>
        {/snippet}
      </EmptyState>
    {:else}
      <EmptyState
        icon={query ? iconSearch : filter === "favorites" ? iconFavorite : iconLibrary}
        title={query ? "No matches" : filter === "favorites" ? "No favorites yet" : "Everything is installed"}
        body={query
          ? `Nothing in your library matches "${query.trim()}".`
          : filter === "favorites"
            ? "Tap the heart on any family to pin it here."
            : "All of your library families are installed on this Mac."}
      >
        {#snippet action()}
          <Button
            variant="tonal"
            onclick={() => {
              filter = "all";
              query = "";
            }}>Show all families</Button
          >
        {/snippet}
      </EmptyState>
    {/if}
  </div>
</div>

<Dialog
  headline={confirmKind === "uninstall" ? `Uninstall "${name(confirmKey ?? "")}"?` : `Remove "${name(confirmKey ?? "")}"?`}
  icon={confirmKind === "uninstall" ? iconCloudOff : iconDelete}
  bind:open={confirmOpen}
>
  {#if confirmKind === "uninstall"}
    The font files are removed from this computer. It stays in your library, so you can install it again.
  {:else}
    It is removed from your library and from your other devices. You can add it again from Browse.
  {/if}
  {#snippet buttons()}
    <Button variant="text" onclick={() => (confirmOpen = false)}>Cancel</Button>
    <span class="tone-error">
      <Button variant="filled" onclick={confirm}>
        {confirmKind === "uninstall" ? "Uninstall" : "Remove"}
      </Button>
    </span>
  {/snippet}
</Dialog>

<style>
  .search {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    height: 2.5rem;
    min-width: 14rem;
    padding: 0 0.75rem;
    border-radius: 9999px;
    background-color: var(--m3c-surface-container-high);
    color: var(--m3c-on-surface-variant);
  }
  .search:focus-within {
    outline: 2px solid var(--m3c-primary);
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: none;
    outline: none;
    background: transparent;
    color: var(--m3c-on-surface);
  }
  .search input::-webkit-search-cancel-button {
    display: none;
  }
  .clear {
    display: grid;
    place-items: center;
    border: none;
    background: transparent;
    color: inherit;
    cursor: pointer;
    border-radius: 9999px;
  }
  .clear:focus-visible {
    outline: 2px solid var(--m3c-primary);
  }
</style>
