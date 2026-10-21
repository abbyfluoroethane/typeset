<script lang="ts">
  import { Icon } from "m3-svelte";
  import iconCloud from "@ktibow/iconset-material-symbols/cloud-outline";
  import iconDisk from "@ktibow/iconset-material-symbols/save-outline";
  import type { FontSource } from "$lib/api";

  // Where a family comes from: Google Fonts (cloud) or the user's own
  // upload (floppy disk). Sits to the right of a family name.
  let { source }: { source: FontSource } = $props();

  const label = $derived(source === "google" ? "Google Fonts" : "Uploaded");
</script>

<span class="chip m3-font-label-medium" class:upload={source === "upload"} title={label}>
  <Icon icon={source === "google" ? iconCloud : iconDisk} size={14} />
  {label}
</span>

<style>
  .chip {
    display: inline-flex;
    flex-shrink: 0;
    align-items: center;
    gap: 0.25rem;
    height: 1.5rem;
    padding: 0 0.5rem 0 0.375rem;
    border-radius: var(--m3-shape-small);
    border: 1px solid var(--m3c-outline-variant);
    color: var(--m3c-on-surface-variant);
    white-space: nowrap;
  }
  .chip.upload {
    border-color: transparent;
    background-color: var(--m3c-tertiary-container);
    color: var(--m3c-on-tertiary-container);
  }
</style>
