<script lang="ts">
  import type { Snippet } from "svelte";

  // The one header every view uses: title, a one-line summary, actions on
  // the right, and an optional toolbar row (search, filters) underneath.
  let {
    title,
    summary,
    actions,
    toolbar,
  }: {
    title: string;
    summary?: Snippet;
    actions?: Snippet;
    toolbar?: Snippet;
  } = $props();
</script>

<header class="header">
  <div class="top">
    <div class="min-w-0">
      <h1 class="m3-font-headline-medium truncate">{title}</h1>
      {#if summary}
        <p class="m3-font-body-medium text-on-surface-variant">{@render summary()}</p>
      {/if}
    </div>
    {#if actions}
      <div class="actions">{@render actions()}</div>
    {/if}
  </div>
  {#if toolbar}
    <div class="toolbar">{@render toolbar()}</div>
  {/if}
</header>

<style>
  .header {
    position: sticky;
    top: 0;
    z-index: 2;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    padding: 1.5rem 1.5rem 1rem;
    background-color: var(--m3c-surface);
  }
  .top {
    display: flex;
    align-items: center;
    gap: 1rem;
    min-height: 3rem;
  }
  h1 {
    margin: 0;
  }
  p {
    margin: 0.125rem 0 0;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-left: auto;
    flex-shrink: 0;
  }
  .toolbar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
  }
</style>
