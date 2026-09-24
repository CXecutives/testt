<!--
  An empty part of the window's toolbar row (macOS): it moves the window and a double click
  zooms it, like the toolbar of a Mac app. The title bar of macOS is transparent over the
  page and WKWebView has no app-region, so Tauri's drag script handles the attribute. As
  high as the toolbar row (--window-top); on a white sheet it sticks to the top of its scroll
  area and hides what scrolls below it.
-->
<script lang="ts">
  interface Props {
    /** Lies on the white sheet (otherwise on the cream of the sidebar). */
    sheet?: boolean;
  }

  let { sheet = false }: Props = $props();
</script>

<div
  class="band"
  class:sheet
  data-tauri-drag-region
  data-testid="drag-band"
  aria-hidden="true"
></div>

<style>
  .band {
    flex: none;
    height: var(--window-top);
  }

  .sheet {
    position: sticky;
    top: 0;
    z-index: var(--z-sticky);
    background-color: var(--surface);
  }
</style>
