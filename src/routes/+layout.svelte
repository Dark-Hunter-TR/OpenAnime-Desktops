<script lang="ts">
  // === OpenAnime Desktop — Kök Layout === //
  // Tüm tema deneyimi openani.me sayfası içinde inline çalıştığı için
  // ayrı bir Svelte route/titlebar gerekmez. Bu layout yalnızca alt
  // route'ları (örn. ana sayfa) render eder.

  // Svelte 5: children snippet (eski <slot/> yerine)
  let { children } = $props();
</script>

{@render children()}
