<script lang="ts">
  import { Icon } from "m3-svelte";
  import type { IconifyIcon } from "@iconify/types";
  import iconSearch from "@ktibow/iconset-material-symbols/search";
  import iconClose from "@ktibow/iconset-material-symbols/close";

  // The one compact text input the toolbars use: a filled pill with a
  // leading icon and a clear button.
  let {
    value = $bindable(""),
    label,
    icon = iconSearch,
    type = "search",
  }: { value: string; label: string; icon?: IconifyIcon; type?: "search" | "text" } = $props();
</script>

<div class="field">
  <Icon {icon} size={20} />
  <input {type} class="m3-font-body-large" placeholder={label} aria-label={label} bind:value />
  {#if value}
    <button type="button" class="clear" aria-label="Clear {label.toLowerCase()}" onclick={() => (value = "")}>
      <Icon icon={iconClose} size={18} />
    </button>
  {/if}
</div>

<style>
  .field {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    height: 2.5rem;
    min-width: 0;
    padding: 0 0.75rem;
    border-radius: 9999px;
    background-color: var(--m3c-surface-container-high);
    color: var(--m3c-on-surface-variant);
  }
  .field:focus-within {
    outline: 2px solid var(--m3c-primary);
  }
  input {
    flex: 1;
    min-width: 0;
    border: none;
    outline: none;
    background: transparent;
    color: var(--m3c-on-surface);
  }
  input::placeholder {
    color: var(--m3c-on-surface-variant);
  }
  input::-webkit-search-cancel-button {
    display: none;
  }
  .clear {
    display: grid;
    place-items: center;
    width: 1.5rem;
    height: 1.5rem;
    border: none;
    border-radius: 9999px;
    background: transparent;
    color: inherit;
    cursor: pointer;
  }
  .clear:focus-visible {
    outline: 2px solid var(--m3c-primary);
  }
</style>
