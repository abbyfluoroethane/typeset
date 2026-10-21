<script lang="ts">
  import { Icon } from "m3-svelte";
  import type { IconifyIcon } from "@iconify/types";
  import type { HTMLButtonAttributes } from "svelte/elements";

  type Props = {
    icon: IconifyIcon;
    label: string;
    /** Toggle state; renders aria-pressed and the accent color. */
    pressed?: boolean;
    /** Accent for the pressed state. */
    accent?: "primary" | "error";
  } & Omit<HTMLButtonAttributes, "children">;
  let { icon, label, pressed, accent = "primary", ...rest }: Props = $props();
</script>

<button
  type="button"
  class="ib {pressed ? `on ${accent}` : ''}"
  aria-label={label}
  title={label}
  aria-pressed={pressed}
  {...rest}
>
  <Icon {icon} size={22} />
</button>

<style>
  .ib {
    display: inline-grid;
    place-items: center;
    flex-shrink: 0;
    width: 2.5rem;
    height: 2.5rem;
    padding: 0;
    border: 0;
    border-radius: 9999px;
    background: transparent;
    color: var(--m3c-on-surface-variant);
    cursor: pointer;
    transition: background-color 150ms ease, color 150ms ease;
  }
  .ib:hover {
    background-color: color-mix(in srgb, var(--m3c-on-surface) 8%, transparent);
  }
  .ib:active {
    background-color: color-mix(in srgb, var(--m3c-on-surface) 12%, transparent);
  }
  .ib:focus-visible {
    outline: 2px solid var(--m3c-primary);
    outline-offset: -2px;
  }
  .ib.on.primary {
    color: var(--m3c-primary);
  }
  .ib.on.error {
    color: var(--m3c-error);
  }
</style>
