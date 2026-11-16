<script lang="ts">
  import { Icon, Menu, MenuItem } from "m3-svelte";
  import iconMore from "@ktibow/iconset-material-symbols/more-vert";
  import iconFolder from "@ktibow/iconset-material-symbols/create-new-folder";
  import iconCheck from "@ktibow/iconset-material-symbols/check";
  import iconOpen from "@ktibow/iconset-material-symbols/open-in-full";
  import iconDelete from "@ktibow/iconset-material-symbols/delete";
  import { app } from "$lib/state.svelte";
  import IconButton from "./IconButton.svelte";
  import { removeWithUndo } from "./helpers";

  type Props = {
    familyKey: string;
    name: string;
    /** Include the "Details" and "Remove" items (card). Otherwise collections only (dialog). */
    full?: boolean;
    /** Show a labelled "Collections" button instead of the overflow icon. */
    label?: boolean;
  };
  let { familyKey, name, full = false, label = false }: Props = $props();

  // Re-parent the menu to <body> so card transforms/overflow can't clip or offset it.
  // Inside a modal dialog it must stay in the top layer, so mount into the dialog instead.
  function portal(node: HTMLElement) {
    const host = (anchor?.closest("dialog") as HTMLElement | null) ?? document.body;
    host.appendChild(node);
    return { destroy: () => node.remove() };
  }

  let open = $state(false);
  let anchor = $state<HTMLElement>();
  let menuEl = $state<HTMLElement>();
  let pos = $state({ x: 0, y: 0, up: false });
  const inLibrary = $derived(app.libraryByKey.has(familyKey));

  function toggle() {
    if (open) return (open = false);
    const r = anchor?.getBoundingClientRect();
    if (r) {
      const up = r.bottom + 300 > window.innerHeight && r.top > 300;
      pos = {
        x: Math.max(8, Math.min(r.left, window.innerWidth - 288)),
        y: up ? window.innerHeight - r.top + 4 : r.bottom + 4,
        up,
      };
    }
    open = true;
  }

  $effect(() => {
    if (open) menuEl?.querySelector<HTMLElement>("button")?.focus();
  });

  const run = (fn: () => void) => () => {
    open = false;
    fn();
  };
</script>

<svelte:window
  onpointerdown={(e) => {
    if (open && !(e.target as Element).closest("[data-family-menu]")) open = false;
  }}
  onkeydown={(e) => {
    if (open && e.key === "Escape") {
      e.stopPropagation();
      e.preventDefault();
      open = false;
      anchor?.querySelector("button")?.focus();
    }
  }}
  onresize={() => (open = false)}
/>

<span bind:this={anchor} class="inline-flex" data-family-menu>
  {#if label}
    <button
      type="button"
      class="lbl m3-font-label-large"
      aria-haspopup="menu"
      aria-expanded={open}
      onclick={toggle}
    >
      <Icon icon={iconFolder} size={20} /> Collections
    </button>
  {:else}
    <IconButton
      icon={iconMore}
      label="More actions for {name}"
      aria-haspopup="menu"
      aria-expanded={open}
      onclick={toggle}
    />
  {/if}
</span>

{#if open}
  <div
    bind:this={menuEl}
    use:portal
    data-family-menu
    role="menu"
    aria-label="Actions for {name}"
    class="fixed z-50 max-h-80 overflow-y-auto rounded-md"
    style:left="{pos.x}px"
    style:top={pos.up ? undefined : `${pos.y}px`}
    style:bottom={pos.up ? `${pos.y}px` : undefined}
  >
    <Menu>
      {#if full}
        <MenuItem icon={iconOpen} onclick={run(() => app.openDetail(familyKey))}>Details…</MenuItem>
        {#if inLibrary}
          <MenuItem icon={iconDelete} onclick={run(() => removeWithUndo(familyKey, name))}>
            Remove from library
          </MenuItem>
        {/if}
        <div class="my-1 h-px bg-outline-variant"></div>
      {/if}
      <p class="m3-font-label-medium px-4 py-2 text-on-surface-variant">Add to collection</p>
      {#each app.collections as c (c.id)}
        <MenuItem
          icon={c.familyKeys.includes(familyKey) ? iconCheck : "space"}
          onclick={() => app.toggleInCollection(c, familyKey)}
        >
          {c.name}
        </MenuItem>
      {:else}
        <MenuItem icon="space" disabled onclick={() => {}}>No collections yet</MenuItem>
      {/each}
    </Menu>
  </div>
{/if}

<style>
  .lbl {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    height: 2.5rem;
    padding: 0 1rem;
    border: 1px solid var(--m3c-outline);
    border-radius: 9999px;
    background: transparent;
    color: var(--m3c-primary);
    cursor: pointer;
  }
  .lbl:hover {
    background-color: color-mix(in srgb, var(--m3c-primary) 8%, transparent);
  }
  .lbl:focus-visible {
    outline: 2px solid var(--m3c-primary);
    outline-offset: 2px;
  }
</style>
