<script lang="ts">
  // The Typeset mark: a "Ts" monogram on a rounded tile. The app icon in
  // src-tauri/icons is drawn from the same shapes (see app-icon.html).
  let { size = 44 }: { size?: number } = $props();
</script>

<div class="logo" style:--size="{size}px" aria-hidden="true">Ts</div>

<style>
  .logo {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    width: var(--size);
    height: var(--size);
    border-radius: calc(var(--size) * 0.3);
    background: var(--m3c-primary);
    color: var(--m3c-on-primary);
    font-size: calc(var(--size) * 0.5);
    line-height: 1;
    font-weight: 800;
    letter-spacing: -0.04em;
    font-variation-settings: "opsz" 48;
    user-select: none;
  }
</style>
