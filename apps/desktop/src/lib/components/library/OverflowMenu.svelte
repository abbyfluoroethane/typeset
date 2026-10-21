<script lang="ts" module>
  import type { IconifyIcon } from "@iconify/types";
  export type MenuEntry =
    | { heading: string }
    | { divider: true }
    | {
        label: string;
        icon?: IconifyIcon | "space";
        danger?: boolean;
        disabled?: boolean;
        onclick: () => void;
      };
</script>

<script lang="ts">
  import { tick } from "svelte";
  import { Button, Icon, Menu, MenuItem } from "m3-svelte";
  import iconMore from "@ktibow/iconset-material-symbols/more-vert";

  // A small anchored "more actions" menu. Danger items are drawn in the
  // error color through a display:contents wrapper (see .danger below).
  let { items, label }: { items: MenuEntry[]; label: string } = $props();

  let open = $state(false);
  let anchor = $state<HTMLElement>();
  let menuEl = $state<HTMLElement>();
  let pos = $state({ right: 0, top: 0 as number | undefined, bottom: 0 as number | undefined });

  async function toggle() {
    if (open) return (open = false);
    const r = anchor?.getBoundingClientRect();
    if (r) {
      const up = r.bottom + 320 > window.innerHeight && r.top > 320;
      pos = {
        right: Math.max(8, window.innerWidth - r.right),
        top: up ? undefined : r.bottom + 4,
        bottom: up ? window.innerHeight - r.top + 4 : undefined,
      };
    }
    open = true;
    await tick();
    menuEl?.querySelector<HTMLElement>("button:not(:disabled)")?.focus();
  }

  function close(refocus = false) {
    open = false;
    if (refocus) anchor?.querySelector<HTMLElement>("button")?.focus();
  }

  function onmenukey(e: KeyboardEvent) {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    const els = [...(menuEl?.querySelectorAll<HTMLElement>("button:not(:disabled)") ?? [])];
    const i = els.indexOf(document.activeElement as HTMLElement);
    const next = e.key === "ArrowDown" ? (i + 1) % els.length : (i - 1 + els.length) % els.length;
    els[next]?.focus();
    e.preventDefault();
  }
</script>

<svelte:window
  onpointerdown={(e) => {
    if (open && !(e.target as Element).closest("[data-overflow-menu]")) open = false;
  }}
  onkeydown={(e) => {
    if (open && e.key === "Escape") {
      e.stopPropagation();
      close(true);
    }
  }}
  onresize={() => (open = false)}
/>

<span bind:this={anchor} class="inline-flex" data-overflow-menu>
  <Button
    variant="text"
    iconType="full"
    aria-haspopup="menu"
    aria-expanded={open}
    aria-label={label}
    title={label}
    onclick={toggle}
  >
    <Icon icon={iconMore} size={24} />
  </Button>
</span>

{#if open}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    bind:this={menuEl}
    data-overflow-menu
    role="menu"
    tabindex="-1"
    class="fixed z-50 max-h-80 overflow-y-auto rounded-md"
    style:right="{pos.right}px"
    style:top={pos.top !== undefined ? `${pos.top}px` : undefined}
    style:bottom={pos.bottom !== undefined ? `${pos.bottom}px` : undefined}
    onkeydown={onmenukey}
  >
    <Menu>
      {#each items as it, i (i)}
        {#if "heading" in it}
          <p class="m3-font-label-medium px-4 py-2 text-on-surface-variant">{it.heading}</p>
        {:else if "divider" in it}
          <hr class="my-2 border-0 border-t border-outline-variant" />
        {:else}
          <span class:danger={it.danger} class="contents">
            <MenuItem
              icon={it.icon}
              disabled={it.disabled}
              onclick={() => {
                close();
                it.onclick();
              }}
            >
              {it.label}
            </MenuItem>
          </span>
        {/if}
      {/each}
    </Menu>
  </div>
{/if}

<style>
  .danger {
    display: contents;
    --m3c-on-surface: var(--m3c-error);
    --m3c-on-surface-variant: var(--m3c-error);
  }
</style>
