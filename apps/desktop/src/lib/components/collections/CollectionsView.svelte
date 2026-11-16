<script lang="ts">
  import { Button, Dialog, Icon, TextField } from "m3-svelte";
  import iconAdd from "@ktibow/iconset-material-symbols/add";
  import iconClose from "@ktibow/iconset-material-symbols/close";
  import iconDelete from "@ktibow/iconset-material-symbols/delete";
  import iconEdit from "@ktibow/iconset-material-symbols/edit";
  import iconFolderOpen from "@ktibow/iconset-material-symbols/folder-open";
  import iconCreate from "@ktibow/iconset-material-symbols/create-new-folder";
  import { parseKey, type Collection } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import PageHeader from "$lib/components/common/PageHeader.svelte";
  import EmptyState from "$lib/components/common/EmptyState.svelte";
  import SourceChip from "$lib/components/common/SourceChip.svelte";
  import FontText from "$lib/components/browse/FontText.svelte";
  import OverflowMenu from "$lib/components/library/OverflowMenu.svelte";

  const sorted = $derived(app.collections.toSorted((a, b) => a.name.localeCompare(b.name)));
  const totalFamilies = $derived(new Set(app.collections.flatMap((c) => c.familyKeys)).size);
  const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
  const count = (c: Collection) => plural(c.familyKeys.length, "family", "families");

  function fontUrl(key: string): string | undefined {
    const p = parseKey(key);
    if (!p || p.source !== "google") return undefined;
    const f = app.catalogByName.get(p.family);
    return f?.menu ?? f?.files.regular;
  }
  const famName = (key: string) => parseKey(key)?.family ?? key;

  // ---- create / rename (one dialog) ----
  let nameOpen = $state(false);
  let nameTarget = $state<Collection | null>(null); // null = creating
  let nameValue = $state("");

  function startCreate() {
    nameTarget = null;
    nameValue = "";
    nameOpen = true;
  }
  function startRename(c: Collection) {
    nameTarget = c;
    nameValue = c.name;
    nameOpen = true;
  }
  async function submitName() {
    const name = nameValue.trim();
    if (!name) return;
    if (nameTarget) {
      await app.renameCollection(nameTarget, name);
      nameOpen = false;
    } else if (await app.createCollection(name)) {
      nameOpen = false;
    }
  }

  // ---- delete confirm ----
  let deleting = $state<Collection | null>(null);
  let deleteOpen = $state(false);
  $effect(() => {
    if (!deleteOpen) deleting = null;
  });
  function startDelete(c: Collection) {
    deleting = c;
    deleteOpen = true;
  }
  async function confirmDelete() {
    if (deleting) await app.deleteCollection(deleting.id);
    deleteOpen = false;
  }

  // ---- contents dialog ----
  let viewId = $state<string | null>(null);
  let viewOpen = $state(false);
  const viewing = $derived(app.collections.find((c) => c.id === viewId) ?? null);
  $effect(() => {
    if (!viewOpen) viewId = null;
  });
  function show(c: Collection) {
    viewId = c.id;
    viewOpen = true;
  }
</script>

<div class="flex h-full flex-col">
  <PageHeader title="Collections">
    {#snippet summary()}
      {plural(sorted.length, "collection", "collections")}, {plural(totalFamilies, "family", "families")}
    {/snippet}
    {#snippet actions()}
      <Button variant="filled" size="s" iconType="left" onclick={startCreate}>
        <Icon icon={iconAdd} size={20} /> New collection
      </Button>
    {/snippet}
  </PageHeader>

  <div class="min-h-0 flex-1 overflow-y-auto px-6 pb-6">
    {#if sorted.length > 0}
      <ul class="grid grid-cols-[repeat(auto-fill,minmax(18rem,1fr))] gap-3">
        {#each sorted as c (c.id)}
          <li class="card relative rounded-xl bg-surface-container-low transition-colors hover:bg-surface-container">
            <button
              type="button"
              class="flex h-full w-full flex-col gap-3 rounded-xl p-4 text-left focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-primary"
              aria-label="Open collection {c.name}"
              onclick={() => show(c)}
            >
              <div class="flex items-center gap-3 pr-10">
                <div class="min-w-0">
                  <h2 class="m3-font-title-medium truncate text-on-surface">{c.name}</h2>
                  <p class="m3-font-body-medium text-on-surface-variant">{count(c)}</p>
                </div>
              </div>
              {#if c.familyKeys.length > 0}
                <div class="flex flex-col gap-1">
                  {#each c.familyKeys.slice(0, 3) as k (k)}
                    <div class="flex min-w-0 items-center gap-2">
                      <FontText url={fontUrl(k)} text={famName(k)} size={18} class="min-w-0 text-on-surface" />
                      <SourceChip source={parseKey(k)?.source ?? "google"} />
                    </div>
                  {/each}
                  {#if c.familyKeys.length > 3}
                    <span class="m3-font-label-medium text-on-surface-variant">
                      +{c.familyKeys.length - 3} more
                    </span>
                  {/if}
                </div>
              {:else}
                <p class="m3-font-body-medium text-on-surface-variant">
                  Empty. Add families from Browse.
                </p>
              {/if}
            </button>
            <div class="absolute top-2 right-2">
              <OverflowMenu
                label="More actions for {c.name}"
                items={[
                  { label: "Rename", icon: iconEdit, onclick: () => startRename(c) },
                  { divider: true },
                  { label: "Delete collection", icon: iconDelete, danger: true, onclick: () => startDelete(c) },
                ]}
              />
            </div>
          </li>
        {/each}
      </ul>
    {:else}
      <EmptyState
        icon={iconCreate}
        title="No collections yet"
        body="Group fonts for a project, a client, or a mood. Create one, then add families from Browse."
      >
        {#snippet action()}
          <Button variant="filled" iconType="left" onclick={startCreate}>
            <Icon icon={iconAdd} /> New collection
          </Button>
        {/snippet}
      </EmptyState>
    {/if}
  </div>
</div>

<Dialog headline={nameTarget ? "Rename collection" : "New collection"} bind:open={nameOpen}>
  <!-- svelte-ignore a11y_autofocus -->
  <TextField label="Collection name" bind:value={nameValue} enter={submitName} autofocus />
  {#snippet buttons()}
    <Button variant="text" onclick={() => (nameOpen = false)}>Cancel</Button>
    <Button variant="filled" disabled={!nameValue.trim()} onclick={submitName}>
      {nameTarget ? "Save" : "Create"}
    </Button>
  {/snippet}
</Dialog>

<Dialog headline={`Delete "${deleting?.name ?? ""}"?`} icon={iconDelete} bind:open={deleteOpen}>
  The fonts in it stay in your library.
  {#snippet buttons()}
    <Button variant="text" onclick={() => (deleteOpen = false)}>Cancel</Button>
    <span class="tone-error"><Button variant="filled" onclick={confirmDelete}>Delete</Button></span>
  {/snippet}
</Dialog>

<Dialog headline={viewing?.name ?? "Collection"} icon={iconFolderOpen} bind:open={viewOpen} style="width: 32rem">
  {#if viewing && viewing.familyKeys.length > 0}
    <p class="m3-font-body-medium mb-2">{count(viewing)}</p>
    <ul class="flex max-h-[50vh] flex-col gap-1 overflow-y-auto">
      {#each viewing.familyKeys as k (k)}
        <li class="flex items-center gap-2 rounded-lg bg-surface-container py-1 pr-1 pl-3">
          <button
            type="button"
            class="min-w-0 flex-1 rounded-md py-1 text-left focus-visible:outline-2 focus-visible:outline-primary"
            aria-label="Open details for {famName(k)}"
            onclick={() => {
              viewOpen = false;
              app.openDetail(k);
            }}
          >
            <span class="flex min-w-0 items-center gap-2">
              <FontText url={fontUrl(k)} text={famName(k)} size={22} class="min-w-0 text-on-surface" />
              <SourceChip source={parseKey(k)?.source ?? "google"} />
            </span>
          </button>
          <span class="tone-error">
            <Button
              variant="text"
              iconType="full"
              aria-label="Remove {famName(k)} from {viewing.name}"
              title="Remove from collection"
              onclick={() => app.toggleInCollection(viewing, k)}
            >
              <Icon icon={iconClose} size={22} />
            </Button>
          </span>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="py-4 text-center">Nothing here yet. Add families from Browse using the collection menu.</p>
  {/if}
  {#snippet buttons()}
    <Button
      variant="text"
      disabled={!viewing}
      onclick={() => {
        const c = viewing;
        viewOpen = false;
        if (c) startRename(c);
      }}>Rename</Button
    >
    <Button variant="tonal" onclick={() => (viewOpen = false)}>Done</Button>
  {/snippet}
</Dialog>
