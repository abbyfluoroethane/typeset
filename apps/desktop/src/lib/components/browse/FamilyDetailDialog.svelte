<script lang="ts">
  import { Button, Checkbox, Icon } from "m3-svelte";
  import iconHeart from "@ktibow/iconset-material-symbols/favorite";
  import iconHeartOutline from "@ktibow/iconset-material-symbols/favorite-outline";
  import iconDownload from "@ktibow/iconset-material-symbols/download";
  import iconDelete from "@ktibow/iconset-material-symbols/delete";
  import iconLaptop from "@ktibow/iconset-material-symbols/laptop-mac";
  import iconLibrary from "@ktibow/iconset-material-symbols/library-add-check";
  import iconAdd from "@ktibow/iconset-material-symbols/add";
  import iconClose from "@ktibow/iconset-material-symbols/close";
  import { parseKey } from "$lib/api";
  import { SAMPLE_TEXT, sortVariants, variantLabel, weightName } from "$lib/fonts.svelte";
  import { app } from "$lib/state.svelte";
  import SourceChip from "$lib/components/common/SourceChip.svelte";
  import FontText from "./FontText.svelte";
  import FamilyMenu from "./FamilyMenu.svelte";
  import IconButton from "./IconButton.svelte";
  import SizeControl from "./SizeControl.svelte";
  import { prettyCategory, removeWithUndo } from "./helpers";

  const SIZES = [
    { label: "S", value: 20, name: "Small sample" },
    { label: "M", value: 28, name: "Medium sample" },
    { label: "L", value: 40, name: "Large sample" },
    { label: "XL", value: 56, name: "Extra large sample" },
  ];

  const key = $derived(app.detailKey);
  const parsed = $derived(key ? parseKey(key) : null);
  const catalogFamily = $derived(parsed?.source === "google" ? app.catalogByName.get(parsed.family) : undefined);
  const entry = $derived(key ? app.libraryByKey.get(key) : undefined);
  const installed = $derived(key ? app.installedByKey.get(key) : undefined);
  const busy = $derived(key ? app.busyKeys.includes(key) : false);
  const uploadFiles = $derived(
    parsed?.source === "upload" ? app.uploads.filter((u) => u.family === parsed.family) : [],
  );
  const variants = $derived(catalogFamily ? sortVariants(catalogFamily.variants) : []);
  const heroUrl = $derived(catalogFamily ? (catalogFamily.files.regular ?? Object.values(catalogFamily.files)[0]) : undefined);
  const sample = $derived(app.previewText.trim() || SAMPLE_TEXT);

  let selected = $state<string[]>([]);
  let sampleSize = $state(28);
  let dialog = $state<HTMLDialogElement>();

  // Reset the checkbox selection when the dialog opens for another family or the entry changes.
  $effect(() => {
    selected = entry && entry.variants.length > 0 ? [...entry.variants] : [...variants];
  });

  // Drive the native modal: Escape and focus trapping come for free.
  $effect(() => {
    if (!dialog) return;
    if (key !== null && !dialog.open) dialog.showModal();
    else if (key === null && dialog.open) dialog.close();
  });

  const allSelected = $derived(variants.length > 0 && selected.length === variants.length);
  const dirty = $derived.by(() => {
    if (!entry) return true;
    const cur = entry.variants.length === 0 ? variants : entry.variants;
    return cur.length !== selected.length || cur.some((v) => !selected.includes(v));
  });

  function toggle(v: string, on: boolean) {
    selected = on ? [...selected, v] : selected.filter((x) => x !== v);
  }

  async function save() {
    if (!key) return;
    const chosen = allSelected ? [] : selected;
    await app.putLibrary(key, chosen, entry?.favorite ?? false);
  }

  const close = () => (app.detailKey = null);
</script>

<dialog
  bind:this={dialog}
  aria-labelledby="detail-title"
  onclose={close}
  onclick={(e) => e.target === e.currentTarget && close()}
>
  {#if key && parsed}
    <div class="sheet">
      <header class="head">
        <div class="min-w-0 flex-1">
          <h2 id="detail-title" class="sr-only">{parsed.family}</h2>
          <div class="flex min-w-0 items-center gap-3">
            <div class="hero min-w-0" aria-hidden="true">
              {#if catalogFamily}
                <FontText url={heroUrl} text={parsed.family} size={56} lines={1} />
              {:else}
                <div class="m3-font-display-small">{parsed.family}</div>
              {/if}
            </div>
            <SourceChip source={parsed.source} />
          </div>
          <p class="m3-font-body-medium meta">
            {#if catalogFamily}
              {prettyCategory(catalogFamily.category)} · {variants.length} styles · {catalogFamily.version}
              · {catalogFamily.subsets.length} subsets
            {:else}
              Uploaded family
            {/if}
            {#if entry}
              <span class="badge lib"><Icon icon={iconLibrary} size={16} /> In library</span>
            {/if}
            {#if installed}
              <span class="badge inst"><Icon icon={iconLaptop} size={16} /> Installed</span>
            {/if}
          </p>
        </div>
        <IconButton icon={iconClose} label="Close" onclick={close} />
      </header>

      <div class="actions">
        {#if catalogFamily}
          {#if entry}
            <Button variant="filled" disabled={!dirty || selected.length === 0} onclick={save}>Save styles</Button>
          {:else}
            <Button variant="filled" iconType="left" disabled={selected.length === 0} onclick={save}>
              <Icon icon={iconAdd} size={20} /> Add to library
            </Button>
          {/if}
        {/if}
        {#if installed}
          <Button variant="outlined" iconType="left" disabled={busy} onclick={() => app.uninstall(key)}>
            <Icon icon={iconDelete} size={20} /> Uninstall from this computer
          </Button>
        {:else}
          <Button variant="tonal" iconType="left" disabled={busy} onclick={() => app.install(key)}>
            <Icon icon={iconDownload} size={20} /> Install on this computer
          </Button>
        {/if}
        <IconButton
          icon={entry?.favorite ? iconHeart : iconHeartOutline}
          label={entry?.favorite ? "Remove from favorites" : "Add to favorites"}
          pressed={entry?.favorite ?? false}
          accent="error"
          onclick={() => app.toggleFavorite(key)}
        />
        <FamilyMenu familyKey={key} name={parsed.family} label />
      </div>

      <div class="body">
        {#if catalogFamily}
          <div class="toolrow">
            <h3 class="m3-font-title-medium">Styles</h3>
            <div class="m3-font-label-large flex items-center gap-1 text-on-surface-variant">
              <span>{selected.length} of {variants.length} selected</span>
              <Button variant="text" size="xs" onclick={() => (selected = allSelected ? [] : [...variants])}>
                {allSelected ? "Select none" : "Select all"}
              </Button>
            </div>
            <div class="ml-auto"><SizeControl bind:value={sampleSize} options={SIZES} label="Sample size" /></div>
          </div>
          <ul class="list" aria-label="Styles">
            {#each variants as v (v)}
              <li>
                <label class="variant" class:on={selected.includes(v)}>
                  <Checkbox>
                    <input
                      type="checkbox"
                      checked={selected.includes(v)}
                      onchange={(e) => toggle(v, e.currentTarget.checked)}
                      aria-label="Include {variantLabel(v)}"
                    />
                  </Checkbox>
                  <div class="min-w-0 flex-1">
                    <p class="m3-font-label-medium mb-1 text-on-surface-variant">
                      {variantLabel(v)}
                    </p>
                    <FontText url={catalogFamily.files[v]} text={sample} size={sampleSize} lines={2} class="text-on-surface" />
                  </div>
                </label>
              </li>
            {/each}
          </ul>
        {:else if uploadFiles.length > 0}
          <ul class="list">
            {#each uploadFiles as u (u.id)}
              <li class="variant m3-font-body-medium justify-between">
                <span class="text-on-surface">{u.style}</span>
                <span class="m3-font-label-medium text-on-surface-variant">
                  {weightName(u.weight)}{u.italic ? " · italic" : ""} · {u.filename}
                </span>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="m3-font-body-large py-6 text-on-surface-variant">
            Font details are unavailable until the catalog has loaded.
          </p>
        {/if}
      </div>

      <footer class="foot">
        {#if entry}
          <span class="tone-error">
            <Button variant="text" iconType="left" onclick={() => removeWithUndo(key, parsed.family)}>
              <Icon icon={iconDelete} size={20} /> Remove from library
            </Button>
          </span>
        {/if}
        <div class="ml-auto"><Button variant="text" onclick={close}>Done</Button></div>
      </footer>
    </div>
  {/if}
</dialog>

<style>
  dialog {
    width: min(60rem, 94vw);
    max-width: 94vw;
    height: min(50rem, 90vh);
    max-height: 90vh;
    margin: auto;
    padding: 0;
    border: 0;
    border-radius: var(--m3-shape-extra-large);
    background-color: var(--m3c-surface-container-high);
    color: var(--m3c-on-surface);
    overflow: hidden;
  }
  dialog[open] {
    display: flex;
    animation: in 220ms var(--m3-timing-function-emphasized-decel, ease);
  }
  dialog::backdrop {
    background-color: color-mix(in srgb, var(--m3c-scrim) 40%, transparent);
  }
  @keyframes in {
    from {
      opacity: 0;
      transform: translateY(1rem) scale(0.98);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    dialog[open] {
      animation: none;
    }
  }
  .sheet {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
  }
  .head {
    display: flex;
    align-items: flex-start;
    gap: 1rem;
    padding: 1.5rem 1.5rem 0.5rem 2rem;
  }
  .hero {
    min-height: 4rem;
    display: flex;
    align-items: center;
  }
  .hero > :global(.font-text) {
    width: 100%;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
    margin: 0.25rem 0 0;
    color: var(--m3c-on-surface-variant);
  }
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.125rem 0.625rem;
    border-radius: 9999px;
    font-size: 0.75rem;
    font-weight: 500;
  }
  .badge.lib {
    background-color: var(--m3c-primary-container);
    color: var(--m3c-on-primary-container);
  }
  .badge.inst {
    background-color: var(--m3c-tertiary-container);
    color: var(--m3c-on-tertiary-container);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
    padding: 0.75rem 2rem 1rem;
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0.5rem 2rem 1rem;
    border-top: 1px solid var(--m3c-outline-variant);
    background-color: var(--m3c-surface-container-low);
  }
  .toolrow {
    position: sticky;
    top: -0.5rem;
    z-index: 1;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem 1rem;
    margin: 0 -2rem;
    padding: 0.75rem 2rem;
    background-color: var(--m3c-surface-container-low);
  }
  .toolrow h3 {
    margin: 0;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .variant {
    display: flex;
    align-items: flex-start;
    gap: 1rem;
    padding: 0.75rem 1rem;
    border-radius: var(--m3-shape-medium);
    background-color: var(--m3c-surface-container);
    cursor: pointer;
    transition: background-color 150ms ease;
  }
  li.variant {
    cursor: default;
  }
  .variant:hover {
    background-color: var(--m3c-surface-container-highest);
  }
  .variant.on {
    background-color: var(--m3c-surface-container-high);
    box-shadow: inset 0 0 0 1px var(--m3c-outline-variant);
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.75rem 1.5rem;
    border-top: 1px solid var(--m3c-outline-variant);
  }
</style>
