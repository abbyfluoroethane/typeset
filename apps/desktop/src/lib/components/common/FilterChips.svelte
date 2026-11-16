<script lang="ts" generics="T extends string">
  import { Icon } from "m3-svelte";
  import type { IconifyIcon } from "@iconify/types";

  // A row of quiet filter choices; exactly one is selected. Only the
  // selected one is filled, so the row reads as text until you look for it.
  let {
    value = $bindable(),
    options,
    label,
  }: {
    value: T;
    options: { id: T; label: string; icon?: IconifyIcon }[];
    label: string;
  } = $props();
</script>

<div class="chips" role="radiogroup" aria-label={label}>
  {#each options as o (o.id)}
    <button
      type="button"
      role="radio"
      aria-checked={value === o.id}
      class="m3-font-label-large"
      class:on={value === o.id}
      onclick={() => (value = o.id)}
    >
      {#if o.icon}<Icon icon={o.icon} size={18} />{/if}
      {o.label}
    </button>
  {/each}
</div>

<style>
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
  }
  button {
    display: inline-flex;
    align-items: center;
    gap: 0.375rem;
    height: 2rem;
    padding: 0 0.75rem;
    border: 0;
    border-radius: 9999px;
    background: transparent;
    color: var(--m3c-on-surface-variant);
    cursor: pointer;
    transition: background-color 150ms ease, color 150ms ease;
  }
  button:hover:not(.on) {
    background-color: color-mix(in srgb, var(--m3c-on-surface) 8%, transparent);
    color: var(--m3c-on-surface);
  }
  button.on {
    background-color: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
  }
  button:focus-visible {
    outline: 2px solid var(--m3c-primary);
    outline-offset: 1px;
  }
</style>
