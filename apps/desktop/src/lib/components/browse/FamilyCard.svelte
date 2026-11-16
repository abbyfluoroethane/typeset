<script lang="ts">
  import { Button, Icon } from "m3-svelte";
  import iconHeart from "@ktibow/iconset-material-symbols/favorite";
  import iconHeartOutline from "@ktibow/iconset-material-symbols/favorite-outline";
  import iconAdd from "@ktibow/iconset-material-symbols/add";
  import iconCheck from "@ktibow/iconset-material-symbols/check";
  import { familyKey, type CatalogFamily } from "$lib/api";
  import { app } from "$lib/state.svelte";
  import FontText from "./FontText.svelte";
  import FamilyMenu from "./FamilyMenu.svelte";
  import IconButton from "./IconButton.svelte";
  import { prettyCategory, removeWithUndo } from "./helpers";

  let { family, size }: { family: CatalogFamily; size: number } = $props();

  const key = $derived(familyKey("google", family.family));
  const entry = $derived(app.libraryByKey.get(key));
  const isInstalled = $derived(app.installedByKey.has(key));
  const custom = $derived(app.previewText.trim() !== "");
  const url = $derived(
    custom ? (family.files.regular ?? Object.values(family.files)[0]) : (family.menu ?? family.files.regular),
  );
  const styles = $derived(`${family.variants.length} style${family.variants.length === 1 ? "" : "s"}`);
</script>

<article class="card">
  <button
    type="button"
    class="open"
    aria-label="Open details for {family.family}"
    onclick={() => app.openDetail(key)}
  >
    <span class="specimen" style:min-height="{Math.max(88, Math.round(size * 2.5))}px">
      <FontText {url} text={custom ? app.previewText : family.family} {size} lines={2} />
    </span>
    <span class="meta">
      {#if custom}<span class="m3-font-title-small name">{family.family}</span>{/if}
      <span class="m3-font-body-small sub">
        {prettyCategory(family.category)}, {styles}
      </span>
    </span>
  </button>

  <div class="actions">
    <span class="quiet" class:keep={entry?.favorite}>
      <IconButton
      icon={entry?.favorite ? iconHeart : iconHeartOutline}
      label={entry?.favorite ? `Remove ${family.family} from favorites` : `Add ${family.family} to favorites`}
      pressed={entry?.favorite ?? false}
      accent="error"
        onclick={() => app.toggleFavorite(key)}
      />
    </span>
    <span class="quiet"><FamilyMenu familyKey={key} name={family.family} full /></span>
    <div class="ml-auto">
      {#if entry}
        <Button
          variant="text"
          size="s"
          iconType="left"
          title="{isInstalled ? 'Installed on this computer' : 'In your library'}. Click to remove from your library."
          aria-label="{family.family} is in your library. Remove from library"
          onclick={() => removeWithUndo(key, family.family)}
        >
          <Icon icon={iconCheck} size={20} /> {isInstalled ? "Installed" : "In library"}
        </Button>
      {:else}
        <Button
          variant="text"
          size="s"
          iconType="left"
          aria-label="Add {family.family} to library"
          onclick={() => app.addToLibrary(key)}
        >
          <Icon icon={iconAdd} size={20} /> Add
        </Button>
      {/if}
    </div>
  </div>
</article>

<style>
  .card {
    display: flex;
    flex-direction: column;
    border-radius: var(--m3-shape-large);
    background-color: var(--m3c-surface-container-low);
    color: var(--m3c-on-surface);
    overflow: hidden;
    transition: background-color 150ms ease;
  }
  .card:hover,
  .card:focus-within {
    background-color: var(--m3c-surface-container);
  }
  /* Favourite and menu stay out of the way until the card is in use. A
     favourited heart stays visible. */
  .quiet {
    display: inline-flex;
    transition: opacity 150ms ease;
  }
  @media (hover: hover) {
    /* Also stay visible while this card's menu is open. */
    .card:not(:hover):not(:focus-within):not(:has([aria-expanded="true"])) .quiet:not(.keep) {
      opacity: 0;
    }
  }
  .open {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 0.75rem;
    padding: 1.25rem 1.25rem 0.5rem;
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .open:focus-visible {
    outline: 3px solid var(--m3c-primary);
    outline-offset: -3px;
    border-radius: var(--m3-shape-large);
  }
  .specimen {
    display: flex;
    align-items: center;
    min-width: 0;
  }
  .specimen > :global(.font-text) {
    width: 100%;
  }
  .meta {
    display: flex;
    flex-direction: column;
    gap: 0.125rem;
    min-width: 0;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.25rem 0.5rem;
    color: var(--m3c-on-surface-variant);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.25rem 0.75rem 0.75rem;
  }
</style>
