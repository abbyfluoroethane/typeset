<script lang="ts">
  import { Button, Icon, LoadingIndicator, TextField } from "m3-svelte";
  import iconLogin from "@ktibow/iconset-material-symbols/login-rounded";
  import iconError from "@ktibow/iconset-material-symbols/error-rounded";
  import iconLink from "@ktibow/iconset-material-symbols/link-rounded";
  import { app } from "$lib/state.svelte";

  let url = $state(app.settings.serverUrl ?? "");

  const urlError = $derived.by(() => {
    const v = url.trim();
    if (!v) return null;
    try {
      const u = new URL(v);
      return u.protocol === "http:" || u.protocol === "https:" ? null : "Start the address with https://";
    } catch {
      return "Enter a full address, like https://fonts.example.com";
    }
  });
  let touched = $state(false);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    touched = true;
    if (app.signingIn || !url.trim() || urlError) return;
    if (!(await app.saveServerUrl(url))) return;
    await app.signIn();
  }

  // Hero glyphs: weight and optical size are the font's variable axes.
  const glyphs = [
    { t: "A", w: 900, o: 144, cls: "g1" },
    { t: "g", w: 200, o: 144, cls: "g2" },
    { t: "&", w: 500, o: 144, cls: "g3" },
    { t: "Rr", w: 760, o: 100, cls: "g4" },
    { t: "a", w: 100, o: 144, cls: "g5" },
  ];
</script>

<div class="flex h-full gap-3 bg-surface p-3">
  <section
    class="hero relative hidden flex-1 overflow-hidden rounded-[48px] bg-surface-container text-on-surface min-[860px]:block"
    aria-hidden="true"
  >
    {#each glyphs as g (g.cls)}
      <span class="glyph {g.cls}" style:font-variation-settings="'wght' {g.w}, 'opsz' {g.o}">{g.t}</span>
    {/each}
  </section>

  <section class="flex flex-1 items-center justify-center overflow-y-auto">
    <form class="w-full max-w-md space-y-6 p-4" onsubmit={submit}>
      <div class="space-y-3">
        <p class="m3-font-label-large inline-block rounded-full bg-secondary-container px-3 py-1 text-on-secondary-container">
          Font library sync
        </p>
        <h1 class="title text-primary">Typeset</h1>
        <p class="m3-font-body-large text-on-surface-variant">
          Connect to your Typeset server and carry your whole font library with you.
        </p>
      </div>

      <div class="field"><TextField
        label="Server URL"
        type="url"
        required
        bind:value={url}
        disabled={app.signingIn}
        leadingIcon={iconLink}
        placeholder="https://fonts.example.com"
        autocomplete="url"
        spellcheck={false}
        error={touched && !!urlError}
        oninput={() => (touched = true)}
      />
        <p class="hint m3-font-body-small" class:bad={touched && !!urlError} role={touched && urlError ? "alert" : undefined}>
          {touched && urlError ? urlError : "The address of your Typeset server. Ask your admin if you're not sure."}
        </p>
      </div>

      {#if app.signingIn}
        <div class="flex items-center gap-4 rounded-xl bg-secondary-container p-4 text-on-secondary-container" role="status">
          <LoadingIndicator size={40} aria-label="Waiting for your browser" />
          <div class="min-w-0 flex-1">
            <p class="m3-font-title-medium">Waiting for your browser</p>
            <p class="m3-font-body-medium">Finish signing in in the window that just opened.</p>
          </div>
          <Button variant="text" size="s" onclick={() => app.cancelSignIn()}>Cancel</Button>
        </div>
      {/if}

      {#if app.signInError}
        <div class="flex items-start gap-3 rounded-xl bg-error-container p-4 text-on-error-container" role="alert">
          <Icon icon={iconError} size={24} />
          <div>
            <p class="m3-font-title-medium">Sign in failed</p>
            <p class="m3-font-body-medium break-words">{app.signInError}</p>
          </div>
        </div>
      {/if}

      <div class="flex items-center gap-2">
        <Button type="submit" variant="filled" size="m" iconType="left" disabled={app.signingIn || !url.trim()}>
          <Icon icon={iconLogin} />
          {app.signingIn ? "Waiting…" : "Sign in"}
        </Button>
      </div>
    </form>
  </section>
</div>

<style>
  .field :global(.m3-container) {
    display: flex;
    width: 100%;
  }
  .hint {
    margin: 4px 0 0 16px;
    color: var(--m3c-on-surface-variant);
  }
  .hint.bad {
    color: var(--m3c-error);
  }
  .hero::after {
    content: "";
    position: absolute;
    inset: 0;
    background: radial-gradient(circle at 85% 10%, color-mix(in oklab, var(--m3c-primary) 18%, transparent), transparent 55%);
  }
  .title {
    font-size: clamp(3rem, 6vw, 4.5rem);
    line-height: 1;
    letter-spacing: -0.03em;
    font-weight: 800;
    font-variation-settings: "opsz" 144;
    margin: 0;
  }
  .hero {
    container-type: size;
    background-image: radial-gradient(
      circle at 20% 90%,
      color-mix(in oklab, var(--m3c-tertiary) 22%, transparent),
      transparent 60%
    );
  }
  .glyph {
    position: absolute;
    line-height: 0.8;
    user-select: none;
    animation: float 9s ease-in-out infinite alternate;
  }
  .g1 { font-size: 62cqmin; left: 4cqw; top: 6cqh; color: var(--m3c-primary); }
  .g2 { font-size: 70cqmin; right: 2cqw; top: 20cqh; color: var(--m3c-on-surface); opacity: 0.85; animation-delay: -3s; }
  .g3 { font-size: 36cqmin; left: 8cqw; bottom: 4cqh; color: var(--m3c-tertiary); animation-delay: -5s; }
  .g4 { font-size: 22cqmin; right: 10cqw; bottom: 8cqh; color: var(--m3c-secondary); animation-delay: -1s; }
  .g5 { font-size: 30cqmin; left: 42cqw; top: 2cqh; color: var(--m3c-on-surface-variant); opacity: 0.5; animation-delay: -7s; }
  @keyframes float {
    to {
      translate: 0 -2.5cqh;
      rotate: 2deg;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .glyph {
      animation: none;
    }
  }
</style>
