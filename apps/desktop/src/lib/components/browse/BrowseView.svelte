<script lang="ts">
  import { Button, Icon, Select } from "m3-svelte";
  import iconRefresh from "@ktibow/iconset-material-symbols/refresh";
  import iconText from "@ktibow/iconset-material-symbols/text-fields";
  import iconTune from "@ktibow/iconset-material-symbols/tune";
  import iconSearchOff from "@ktibow/iconset-material-symbols/search-off";
  import iconCloudOff from "@ktibow/iconset-material-symbols/cloud-off";
  import { onEnter } from "$lib/actions";
  import { app } from "$lib/state.svelte";
  import PageHeader from "$lib/components/common/PageHeader.svelte";
  import EmptyState from "$lib/components/common/EmptyState.svelte";
  import SearchField from "$lib/components/common/SearchField.svelte";
  import FilterChips from "$lib/components/common/FilterChips.svelte";
  import IconButton from "./IconButton.svelte";
  import FamilyCard from "./FamilyCard.svelte";
  import SizeControl from "./SizeControl.svelte";
  import { prettyCategory } from "./helpers";

  const PAGE = 36;
  const SIZES = [
    { label: "S", value: 24, name: "Small preview" },
    { label: "M", value: 36, name: "Medium preview" },
    { label: "L", value: 52, name: "Large preview" },
    { label: "XL", value: 72, name: "Extra large preview" },
  ];

  let search = $state("");
  let category = $state("all");
  let subset = $state("all");
  let sort = $state("popularity");
  let size = $state(36);
  let limit = $state(PAGE);
  let filtersOpen = $state(false);
  let scrolled = $state(false);
  let filtersEl = $state<HTMLElement>();

  const categories = $derived([...new Set(app.catalog.map((f) => f.category))].sort());
  const subsets = $derived([...new Set(app.catalog.flatMap((f) => f.subsets))].sort());

  const results = $derived.by(() => {
    const q = search.trim().toLowerCase();
    let list = app.catalog.filter(
      (f) =>
        (!q || f.family.toLowerCase().includes(q)) &&
        (category === "all" || f.category === category) &&
        (subset === "all" || f.subsets.includes(subset)),
    );
    if (sort === "name") list = [...list].sort((a, b) => a.family.localeCompare(b.family));
    else if (sort === "modified")
      list = [...list].sort((a, b) => b.lastModified.localeCompare(a.lastModified));
    return list;
  });

  // Reset the rendered window whenever the result set changes.
  $effect(() => {
    void results;
    limit = PAGE;
  });

  const shown = $derived(results.slice(0, limit));
  const filtered = $derived(search.trim() !== "" || category !== "all" || subset !== "all");
  const activeExtra = $derived((subset !== "all" ? 1 : 0) + (sort !== "popularity" ? 1 : 0));
  const categoryOptions = $derived([
    { id: "all", label: "All" },
    ...categories.map((c) => ({ id: c, label: prettyCategory(c) })),
  ]);

  function clearFilters() {
    search = "";
    category = "all";
    subset = "all";
  }
</script>

<svelte:window
  onpointerdown={(e) => {
    if (filtersOpen && filtersEl && !filtersEl.contains(e.target as Node)) filtersOpen = false;
  }}
  onkeydown={(e) => {
    if (filtersOpen && e.key === "Escape") filtersOpen = false;
  }}
/>

<div class="root bg-surface" class:scrolled onscroll={(e) => (scrolled = e.currentTarget.scrollTop > 4)}>
  <PageHeader title="Browse">
    {#snippet summary()}
      <span aria-live="polite">
        {#if app.catalog.length}
          {#if filtered}{results.length.toLocaleString()} of{" "}{/if}{app.catalog.length.toLocaleString()} families
        {:else if app.catalogLoading}
          Loading the Google Fonts catalog…
        {:else}
          Google Fonts catalog
        {/if}
      </span>
    {/snippet}
    {#snippet actions()}
      <IconButton
        icon={iconRefresh}
        label="Refresh the catalog from Google Fonts"
        disabled={app.catalogLoading}
        onclick={() => app.loadCatalog(true)}
      />
    {/snippet}
    {#snippet toolbar()}
      <div class="row">
        <div class="grow"><SearchField label="Search families" bind:value={search} /></div>
        <div class="grow">
          <SearchField label="Type your own preview text" icon={iconText} type="text" bind:value={app.previewText} />
        </div>
        <SizeControl bind:value={size} options={SIZES} />
        <div class="filters" bind:this={filtersEl}>
          <Button
            variant={activeExtra ? "tonal" : "text"}
            size="s"
            iconType="left"
            aria-haspopup="dialog"
            aria-expanded={filtersOpen}
            onclick={() => (filtersOpen = !filtersOpen)}
          >
            <Icon icon={iconTune} size={20} /> {activeExtra ? `Filters (${activeExtra})` : "Filters"}
          </Button>
          {#if filtersOpen}
            <div class="popover" role="dialog" aria-label="Subset and sort">
              <Select
                label="Subset"
                bind:value={subset}
                options={[{ value: "all", text: "All subsets" }, ...subsets.map((s) => ({ value: s, text: s }))]}
              />
              <Select
                label="Sort by"
                bind:value={sort}
                options={[
                  { value: "popularity", text: "Popularity" },
                  { value: "name", text: "Name" },
                  { value: "modified", text: "Last modified" },
                ]}
              />
              <div class="flex justify-end">
                <Button
                  variant="text"
                  size="s"
                  disabled={!activeExtra}
                  onclick={() => {
                    subset = "all";
                    sort = "popularity";
                  }}
                >
                  Reset
                </Button>
              </div>
            </div>
          {/if}
        </div>
      </div>
      <FilterChips label="Category" options={categoryOptions} bind:value={category} />
    {/snippet}
  </PageHeader>

  <div class="content">
    {#if app.catalogError && app.catalog.length === 0}
      <EmptyState icon={iconCloudOff} title="Could not load the font catalog" body={app.catalogError}>
        {#snippet action()}
          <Button variant="filled" onclick={() => app.loadCatalog(true)}>Try again</Button>
        {/snippet}
      </EmptyState>
    {:else if app.catalogLoading && app.catalog.length === 0}
      <div class="grid" role="status" aria-label="Loading font catalog">
        {#each Array(12) as _, i (i)}
          <div class="skeleton" aria-hidden="true">
            <div class="bar" style:width="{55 + ((i * 13) % 35)}%"></div>
            <div class="bar thin" style:width="40%"></div>
          </div>
        {/each}
      </div>
    {:else if results.length === 0}
      <EmptyState
        icon={iconSearchOff}
        title="No families match"
        body="Try a different name, or clear the filters to see the whole catalog."
      >
        {#snippet action()}
          <Button variant="tonal" onclick={clearFilters}>Clear filters</Button>
        {/snippet}
      </EmptyState>
    {:else}
      <div class="grid">
        {#each shown as f (f.family)}
          <FamilyCard family={f} {size} />
        {/each}
      </div>
      {#if shown.length < results.length}
        {#key limit}
          <div use:onEnter={() => (limit += PAGE)} class="h-16" aria-hidden="true"></div>
        {/key}
      {/if}
    {/if}
  </div>
</div>

<style>
  .root {
    height: 100%;
    overflow-y: auto;
  }
  /* Separate the sticky header from content scrolling beneath it. */
  .root > :global(header) {
    transition: box-shadow 150ms ease;
    padding-bottom: 0.75rem;
  }
  .root.scrolled > :global(header) {
    box-shadow:
      0 1px 0 var(--m3c-outline-variant),
      0 4px 12px -6px color-mix(in srgb, var(--m3c-scrim) 30%, transparent);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.75rem;
    width: 100%;
  }
  .grow {
    flex: 1 1 13rem;
    max-width: 24rem;
  }
  .filters {
    position: relative;
  }
  .popover {
    position: absolute;
    right: 0;
    top: calc(100% + 0.5rem);
    z-index: 5;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    width: 16rem;
    padding: 1rem;
    border-radius: var(--m3-shape-large);
    background-color: var(--m3c-surface-container-high);
    box-shadow: var(--m3-elevation-3);
  }
  .popover :global(.m3-container) {
    width: 100%;
  }
  .content {
    padding: 1rem 1.5rem 2rem;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(17.5rem, 1fr));
    gap: 1rem;
  }
  .skeleton {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 0.75rem;
    height: 11rem;
    padding: 1.25rem;
    border-radius: var(--m3-shape-large);
    background-color: var(--m3c-surface-container-low);
  }
  .bar {
    height: 2rem;
    border-radius: 0.5rem;
    background-color: var(--m3c-surface-container-highest);
  }
  .bar.thin {
    height: 0.75rem;
  }
  @media (prefers-reduced-motion: no-preference) {
    .bar {
      animation: pulse 1.4s ease-in-out infinite;
    }
    @keyframes pulse {
      50% {
        opacity: 0.45;
      }
    }
  }
</style>
