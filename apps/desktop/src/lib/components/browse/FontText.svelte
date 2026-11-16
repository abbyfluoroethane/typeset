<script lang="ts">
  import { inView } from "$lib/actions";
  import { loadFont, loadedFont } from "$lib/fonts.svelte";

  type Props = {
    /** Font file URL; text falls back to the UI font until it loads. */
    url: string | undefined;
    text: string;
    /** Font size in px. */
    size?: number;
    /** Max visible lines (text wraps and is clamped). */
    lines?: number;
    weight?: number;
    italic?: boolean;
    class?: string;
  };
  let { url, text, size = 32, lines = 1, weight, italic = false, class: className = "" }: Props = $props();

  let visible = $state(false);
  const fontName = $derived(loadedFont(url));

  // Start loading once scrolled near view; `fontName` updates when it lands.
  $effect(() => {
    if (visible && url) void loadFont(url);
  });
</script>

<div
  use:inView={() => (visible = true)}
  class="font-text {fontName ? 'ready' : ''} {className}"
  style:font-family={fontName ? `"${fontName}", sans-serif` : undefined}
  style:font-size="{size}px"
  style:font-weight={fontName && weight ? undefined : weight}
  style:font-style={italic ? "italic" : undefined}
  style:-webkit-line-clamp={lines}
  title={text}
>
  {text}
</div>

<style>
  .font-text {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
    line-height: 1.15;
    opacity: 0.45;
  }
  .font-text.ready {
    opacity: 1;
  }
  @media (prefers-reduced-motion: no-preference) {
    .font-text {
      transition: opacity 250ms ease;
    }
  }
</style>
