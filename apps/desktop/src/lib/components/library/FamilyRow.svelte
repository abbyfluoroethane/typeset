<script lang="ts">
  import { Button, Icon } from "m3-svelte";
  import IconButton from "$lib/components/browse/IconButton.svelte";
  import iconFavorite from "@ktibow/iconset-material-symbols/favorite";
  import iconFavoriteOutline from "@ktibow/iconset-material-symbols/favorite-outline";
  import iconDownload from "@ktibow/iconset-material-symbols/download";
  import iconCheck from "@ktibow/iconset-material-symbols/check";
  import iconDelete from "@ktibow/iconset-material-symbols/delete";
  import iconCloudOff from "@ktibow/iconset-material-symbols/cloud-off";
  import iconFolder from "@ktibow/iconset-material-symbols/folder";
  import { parseKey } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import SourceChip from "$lib/components/common/SourceChip.svelte";
  import FontText from "$lib/components/browse/FontText.svelte";
  import OverflowMenu, { type MenuEntry } from "./OverflowMenu.svelte";

  let {
    familyKey,
    onuninstall,
    onremove,
  }: {
    familyKey: string;
    onuninstall: (key: string) => void;
    onremove: (key: string) => void;
  } = $props();

  const parsed = $derived(parseKey(familyKey));
  const family = $derived(parsed ? app.catalogByName.get(parsed.family) : undefined);
  const entry = $derived(app.libraryByKey.get(familyKey));
  const installed = $derived(app.installedByKey.has(familyKey));
  const busy = $derived(app.busyKeys.includes(familyKey));
  const name = $derived(parsed?.family ?? familyKey);
  const isUpload = $derived(parsed?.source === "upload");
  const url = $derived(
    parsed?.source === "google" ? (family?.menu ?? family?.files.regular) : undefined,
  );
  const uploadCount = $derived(
    isUpload ? app.uploads.filter((u) => u.family === name).length : 0,
  );
  const secondary = $derived.by(() => {
    if (isUpload) return `${uploadCount} file${uploadCount === 1 ? "" : "s"}`;
    const v = entry?.variants.length ?? 0;
    return v === 0 ? "All styles" : `${v} style${v === 1 ? "" : "s"}`;
  });

  const menu = $derived.by<MenuEntry[]>(() => {
    const out: MenuEntry[] = [];
    out.push({ heading: "Add to collection" });
    if (app.collections.length === 0) {
      out.push({ label: "No collections yet", icon: "space", disabled: true, onclick: () => {} });
    }
    for (const c of app.collections) {
      out.push({
        label: c.name,
        icon: c.familyKeys.includes(familyKey) ? iconCheck : iconFolder,
        onclick: () => app.toggleInCollection(c, familyKey),
      });
    }
    out.push({ divider: true });
    if (installed) {
      out.push({
        label: "Uninstall from this computer",
        icon: iconCloudOff,
        danger: true,
        onclick: () => onuninstall(familyKey),
      });
    }
    out.push({
      label: "Remove from library",
      icon: iconDelete,
      danger: true,
      onclick: () => onremove(familyKey),
    });
    return out;
  });
</script>

<div class="row group flex items-center gap-3 rounded-xl bg-surface-container-low pr-2 pl-1 transition-colors hover:bg-surface-container">
  <button
    type="button"
    class="flex h-full min-w-0 flex-1 items-center gap-4 rounded-lg px-3 text-left focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-primary"
    aria-label="Open details for {name}"
    onclick={() => app.openDetail(familyKey)}
  >
    <div class="min-w-0 flex-1">
      <div class="flex min-w-0 items-center gap-2">
        {#if isUpload}
          <div class="upload-name min-w-0 truncate text-on-surface">{name}</div>
        {:else}
          <FontText {url} text={name} size={26} class="min-w-0 text-on-surface" />
        {/if}
        <SourceChip source={isUpload ? "upload" : "google"} />
      </div>
      <div class="m3-font-body-medium mt-0.5 truncate text-on-surface-variant">{secondary}</div>
    </div>
  </button>

  {#if installed}
    <span class="m3-font-label-large inline-flex shrink-0 items-center gap-1 pr-2 text-on-surface-variant">
      <Icon icon={iconCheck} size={18} /> Installed
    </span>
  {:else}
    <Button variant="tonal" size="s" iconType="left" disabled={busy} onclick={() => app.install(familyKey)}>
      <Icon icon={iconDownload} /> Install
    </Button>
  {/if}

  {#if entry}
    <IconButton
      icon={entry.favorite ? iconFavorite : iconFavoriteOutline}
      label={entry.favorite ? `Remove ${name} from favorites` : `Add ${name} to favorites`}
      pressed={entry.favorite}
      accent="error"
      onclick={() => app.toggleFavorite(familyKey)}
    />
  {/if}
  <OverflowMenu items={menu} label="More actions for {name}" />
</div>

<style>
  .row {
    min-height: 4.5rem;
    padding-block: 0.25rem;
  }
  /* Uploaded fonts are not loaded for preview, so match the specimen size. */
  .upload-name {
    font-size: 26px;
    line-height: 1.2;
  }
</style>
