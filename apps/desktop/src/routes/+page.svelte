<script lang="ts">
  import { onMount } from "svelte";
  import { LoadingIndicator } from "m3-svelte";
  import { app } from "$lib/state.svelte";
  import SignInCard from "$lib/components/auth/SignInCard.svelte";
  import AppNav from "$lib/components/layout/AppNav.svelte";
  import BrowseView from "$lib/components/browse/BrowseView.svelte";
  import FamilyDetailDialog from "$lib/components/browse/FamilyDetailDialog.svelte";
  import LibraryView from "$lib/components/library/LibraryView.svelte";
  import CollectionsView from "$lib/components/collections/CollectionsView.svelte";
  import UploadsView from "$lib/components/uploads/UploadsView.svelte";
  import SettingsView from "$lib/components/settings/SettingsView.svelte";

  onMount(() => {
    void app.boot();
    return () => app.stop();
  });
</script>

<main class="h-screen overflow-hidden bg-surface text-on-surface">
  {#if !app.booted}
    <div class="flex h-full items-center justify-center text-primary">
      <LoadingIndicator size={64} aria-label="Loading Typeset" />
    </div>
  {:else if !app.signedIn}
    <SignInCard />
  {:else}
    <div class="flex h-full bg-surface-container">
      <AppNav />
      <div class="my-3 mr-3 min-h-0 min-w-0 flex-1 overflow-hidden rounded-xl bg-surface">
        {#key app.view}
          <div class="view h-full">
            {#if app.view === "browse"}
              <BrowseView />
            {:else if app.view === "library"}
              <LibraryView />
            {:else if app.view === "collections"}
              <CollectionsView />
            {:else if app.view === "uploads"}
              <UploadsView />
            {:else}
              <SettingsView />
            {/if}
          </div>
        {/key}
      </div>
    </div>
    <FamilyDetailDialog />
  {/if}
</main>

<style>
  .view {
    animation: view-in var(--m3-easing-spatial);
  }
  @keyframes view-in {
    from {
      opacity: 0;
      translate: 0 12px;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .view {
      animation: none;
    }
  }
</style>
