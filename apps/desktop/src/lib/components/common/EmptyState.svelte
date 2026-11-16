<script lang="ts">
  import type { Snippet } from "svelte";
  import type { IconifyIcon } from "@iconify/types";
  import { Icon } from "m3-svelte";

  let {
    icon,
    title,
    body,
    action,
  }: { icon: IconifyIcon; title: string; body?: string; action?: Snippet } = $props();
</script>

<div class="empty" role="status">
  <div class="badge"><Icon {icon} size={32} /></div>
  <h2 class="m3-font-title-large">{title}</h2>
  {#if body}<p class="m3-font-body-medium text-on-surface-variant">{body}</p>{/if}
  {#if action}<div class="mt-2">{@render action()}</div>{/if}
</div>

<style>
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 0.5rem;
    padding: 4rem 1.5rem;
    max-width: 28rem;
    margin: 0 auto;
  }
  .badge {
    display: grid;
    place-items: center;
    width: 4.5rem;
    height: 4.5rem;
    margin-bottom: 0.5rem;
    border-radius: var(--m3-shape-extra-large);
    background-color: var(--m3c-primary-container);
    color: var(--m3c-on-primary-container);
  }
  h2,
  p {
    margin: 0;
  }
</style>
