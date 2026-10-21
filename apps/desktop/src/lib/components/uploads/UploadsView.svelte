<script lang="ts">
  import { Button, Dialog, Icon } from "m3-svelte";
  import iconUpload from "@ktibow/iconset-material-symbols/upload";
  import iconDelete from "@ktibow/iconset-material-symbols/delete";
  import iconFontDownload from "@ktibow/iconset-material-symbols/font-download";
  import { open } from "@tauri-apps/plugin-dialog";
  import { weightName } from "$lib/fonts.svelte";
  import type { Upload } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import PageHeader from "$lib/components/common/PageHeader.svelte";
  import EmptyState from "$lib/components/common/EmptyState.svelte";

  let busy = $state(false);

  async function pick() {
    busy = true;
    try {
      const picked = await open({
        multiple: true,
        filters: [{ name: "Fonts", extensions: ["ttf", "otf"] }],
      });
      if (picked) await app.uploadFonts(Array.isArray(picked) ? picked : [picked]);
    } catch (e) {
      app.fail(e, "Could not open file picker");
    } finally {
      busy = false;
    }
  }

  const groups = $derived.by(() => {
    const m = new Map<string, typeof app.uploads>();
    for (const u of app.uploads) m.set(u.family, [...(m.get(u.family) ?? []), u]);
    return [...m.entries()]
      .toSorted(([a], [b]) => a.localeCompare(b))
      .map(([family, files]) => ({
        family,
        files: files.toSorted((a, b) => a.weight - b.weight || Number(a.italic) - Number(b.italic)),
      }));
  });

  const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
  const size = (n: number) =>
    n > 1048576 ? `${(n / 1048576).toFixed(1)} MB` : `${Math.ceil(n / 1024)} KB`;

  const traits = (u: Upload) => `${weightName(u.weight)}${u.italic ? " italic" : ""}`;

  const detail = (u: Upload) =>
    [traits(u).toLowerCase() === u.style.toLowerCase() ? null : traits(u), u.filename, size(u.size)]
      .filter(Boolean)
      .join(" · ");

  let deleting = $state<Upload | null>(null);
  let deleteOpen = $state(false);
  $effect(() => {
    if (!deleteOpen) deleting = null;
  });
  async function confirmDelete() {
    const u = deleting;
    deleteOpen = false;
    if (u) await app.deleteUpload(u.id);
  }
</script>

<div class="flex h-full flex-col">
  <PageHeader title="Uploads">
    {#snippet summary()}
      {#if groups.length > 0}
        {plural(groups.length, "family", "families")} · {plural(app.uploads.length, "file", "files")} synced to
        your account
      {:else}
        Your own TTF and OTF files, synced to your other devices
      {/if}
    {/snippet}
    {#snippet actions()}
      <Button variant="filled" size="m" iconType="left" disabled={busy} onclick={pick}>
        <Icon icon={iconUpload} /> Upload fonts
      </Button>
    {/snippet}
  </PageHeader>

  <div class="min-h-0 flex-1 overflow-y-auto px-6 pb-6">
    {#if groups.length > 0}
      <div class="flex flex-col gap-3">
        {#each groups as g (g.family)}
          <section class="rounded-xl bg-surface-container-low p-2" aria-label={g.family}>
            <div class="flex items-center gap-3 px-3 py-2">
              <div
                class="m3-font-title-large grid size-10 shrink-0 place-items-center rounded-lg bg-secondary-container text-on-secondary-container"
                aria-hidden="true"
              >
                {g.family.charAt(0).toUpperCase()}
              </div>
              <div class="min-w-0 flex-1">
                <h2 class="m3-font-title-medium truncate text-on-surface">{g.family}</h2>
                <p class="m3-font-body-medium text-on-surface-variant">
                  {plural(g.files.length, "file", "files")}
                </p>
              </div>
              <Button variant="text" size="s" onclick={() => app.openDetail(`upload:${g.family}`)}>
                Details
              </Button>
            </div>
            <ul class="flex flex-col">
              {#each g.files as u (u.id)}
                <li
                  class="flex min-h-14 items-center gap-3 rounded-lg py-1 pr-1 pl-3 transition-colors hover:bg-surface-container"
                >
                  <div class="min-w-0 flex-1">
                    <div class="m3-font-label-large truncate text-on-surface">{u.style}</div>
                    <div class="m3-font-body-small truncate text-on-surface-variant">
                      {detail(u)}
                    </div>
                  </div>
                  <span class="tone-error">
                    <Button
                      variant="text"
                      iconType="full"
                      aria-label="Delete {u.filename}"
                      title="Delete {u.filename}"
                      onclick={() => {
                        deleting = u;
                        deleteOpen = true;
                      }}
                    >
                      <Icon icon={iconDelete} size={22} />
                    </Button>
                  </span>
                </li>
              {/each}
            </ul>
          </section>
        {/each}
      </div>
    {:else}
      <EmptyState
        icon={iconFontDownload}
        title="Bring your own type"
        body="Got a typeface that is not on Google Fonts? Add TTF or OTF files and Typeset will install them here and on your other devices."
      >
        {#snippet action()}
          <Button variant="filled" size="m" iconType="left" disabled={busy} onclick={pick}>
            <Icon icon={iconUpload} /> Upload fonts
          </Button>
        {/snippet}
      </EmptyState>
    {/if}
  </div>
</div>

<Dialog headline={`Delete "${deleting?.filename ?? ""}"?`} icon={iconDelete} bind:open={deleteOpen}>
  The file is removed from your account and your other devices.
  {#snippet buttons()}
    <Button variant="text" onclick={() => (deleteOpen = false)}>Cancel</Button>
    <span class="tone-error"><Button variant="filled" onclick={confirmDelete}>Delete</Button></span>
  {/snippet}
</Dialog>
