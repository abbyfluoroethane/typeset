<script lang="ts">
  import { Icon } from "m3-svelte";
  import iconDisk from "@ktibow/iconset-material-symbols/save-outline";
  import type { FontSource } from "$lib/api";

  // Marks a family the user uploaded. Google Fonts families are the normal
  // case and get no marker, so the chip means something when it appears.
  let { source }: { source: FontSource } = $props();
</script>

{#if source === "upload"}
  <span class="chip m3-font-label-medium" title="A font you uploaded">
    <Icon icon={iconDisk} size={14} />
    Uploaded
  </span>
{/if}

<style>
  .chip {
    display: inline-flex;
    flex-shrink: 0;
    align-items: center;
    gap: 0.25rem;
    height: 1.5rem;
    padding: 0 0.5rem 0 0.375rem;
    border-radius: 9999px;
    background-color: var(--m3c-surface-container-highest);
    color: var(--m3c-on-surface-variant);
    white-space: nowrap;
  }
</style>
