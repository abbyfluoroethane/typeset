<script lang="ts">
  import { Button, Icon, LoadingIndicator, TextField } from "m3-svelte";
  import iconLogin from "@ktibow/iconset-material-symbols/login-rounded";
  import iconError from "@ktibow/iconset-material-symbols/error-rounded";
  import iconLink from "@ktibow/iconset-material-symbols/link-rounded";
  import { app } from "$lib/state.svelte";
  import Logo from "$lib/components/common/Logo.svelte";

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
</script>

<div class="flex h-full gap-3 bg-surface p-3">
  <!-- A quiet type specimen: one face, one colour, one slow change of weight. -->
  <section
    class="specimen hidden flex-1 flex-col justify-between overflow-hidden rounded-[32px] bg-surface-container p-10 min-[860px]:flex"
    aria-hidden="true"
  >
    <p class="m3-font-label-large text-on-surface-variant">Google Sans Flex</p>
    <p class="sample text-on-surface">Aa</p>
    <div class="space-y-3 text-on-surface-variant">
      <p class="pangram">Sphinx of black quartz, judge my vow.</p>
      <p class="charset">ABCDEFGHIJKLMNOPQRSTUVWXYZ<br />abcdefghijklmnopqrstuvwxyz<br />0123456789 &amp; ! ? @ #</p>
    </div>
  </section>

  <section class="flex flex-1 items-center justify-center overflow-y-auto">
    <form class="w-full max-w-[26rem] p-4" onsubmit={submit}>
      <Logo size={56} />
      <h1 class="title text-on-surface">Typeset</h1>
      <p class="m3-font-body-large mb-10 text-on-surface-variant">
        Sign in to your server to keep the same fonts on every machine.
      </p>

      <div class="field">
        <TextField
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
          {touched && urlError ? urlError : "The address of your Typeset server."}
        </p>
      </div>

      {#if app.signingIn}
        <div class="mt-6 flex items-center gap-4 rounded-xl bg-secondary-container p-4 text-on-secondary-container" role="status">
          <LoadingIndicator size={40} aria-label="Waiting for your browser" />
          <div class="min-w-0 flex-1">
            <p class="m3-font-title-medium">Waiting for your browser</p>
            <p class="m3-font-body-medium">Finish signing in in the window that just opened.</p>
          </div>
          <Button variant="text" size="s" onclick={() => app.cancelSignIn()}>Cancel</Button>
        </div>
      {/if}

      {#if app.signInError}
        <div class="mt-6 flex items-start gap-3 rounded-xl bg-error-container p-4 text-on-error-container" role="alert">
          <Icon icon={iconError} size={24} />
          <div>
            <p class="m3-font-title-medium">Sign in failed</p>
            <p class="m3-font-body-medium break-words">{app.signInError}</p>
          </div>
        </div>
      {/if}

      <div class="mt-8 flex items-center gap-2">
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
    margin: 6px 0 0 16px;
    color: var(--m3c-on-surface-variant);
  }
  .hint.bad {
    color: var(--m3c-error);
  }
  .title {
    margin: 28px 0 12px;
    font-size: 2.75rem;
    line-height: 1.15;
    letter-spacing: -0.02em;
    font-weight: 700;
    font-variation-settings: "opsz" 96;
  }

  .specimen {
    container-type: size;
  }
  .sample {
    margin: 0;
    font-size: min(58cqh, 62cqw);
    line-height: 0.9;
    letter-spacing: -0.04em;
    font-weight: 400;
    font-variation-settings: "opsz" 144;
    animation: breathe 14s ease-in-out infinite alternate;
  }
  .pangram {
    margin: 0;
    font-size: clamp(1.25rem, 4.2cqw, 2rem);
    line-height: 1.2;
    letter-spacing: -0.01em;
    color: var(--m3c-on-surface);
  }
  .charset {
    margin: 0;
    font-size: 0.875rem;
    line-height: 1.6;
    letter-spacing: 0.08em;
    overflow-wrap: anywhere;
  }
  /* The weight axis drifts between light and bold. */
  @keyframes breathe {
    from {
      font-weight: 250;
    }
    to {
      font-weight: 750;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .sample {
      animation: none;
    }
  }
</style>
