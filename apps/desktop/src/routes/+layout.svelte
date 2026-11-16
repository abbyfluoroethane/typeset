<script lang="ts">
  import "../app.css";
  import { Snackbar } from "m3-svelte";
  import { theme } from "$lib/theme.svelte";

  let { children } = $props();

  theme.apply();
</script>

<Snackbar />
{@render children()}
