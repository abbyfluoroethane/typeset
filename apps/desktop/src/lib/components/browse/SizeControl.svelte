<script lang="ts">
  // Compact segmented control for preview size (radio semantics).
  type Opt = { label: string; value: number; name: string };
  let {
    value = $bindable(),
    options,
    label = "Preview size",
  }: { value: number; options: Opt[]; label?: string } = $props();
</script>

<div class="seg" role="radiogroup" aria-label={label}>
  {#each options as o (o.value)}
    <button
      type="button"
      role="radio"
      aria-checked={value === o.value}
      aria-label={o.name}
      title={o.name}
      class="m3-font-label-large"
      class:on={value === o.value}
      onclick={() => (value = o.value)}
    >
      {o.label}
    </button>
  {/each}
</div>

<style>
  .seg {
    display: inline-flex;
    flex-shrink: 0;
    height: 2.5rem;
    padding: 0.25rem;
    border-radius: 9999px;
    background-color: var(--m3c-surface-container-high);
  }
  button {
    min-width: 2.25rem;
    padding: 0 0.625rem;
    border: 0;
    border-radius: 9999px;
    background: transparent;
    color: var(--m3c-on-surface-variant);
    cursor: pointer;
    transition: background-color 150ms ease;
  }
  button:hover:not(.on) {
    background-color: color-mix(in srgb, var(--m3c-on-surface) 8%, transparent);
  }
  button.on {
    background-color: var(--m3c-secondary-container);
    color: var(--m3c-on-secondary-container);
  }
  button:focus-visible {
    outline: 2px solid var(--m3c-primary);
    outline-offset: -2px;
  }
</style>
