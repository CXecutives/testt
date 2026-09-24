<!--
  Component gallery (`?gallery`, development and harness builds only). Token boards first,
  then every component × variant × size × state. core/tests/ui_contract.rs requires that
  every file in components/ is imported here.
-->
<script lang="ts">
  import Button, { BUTTON_SIZES, BUTTON_VARIANTS } from '$components/Button.svelte';
  import EmptyState from '$components/EmptyState.svelte';
  import Icon, { ICON_NAMES } from '$components/Icon.svelte';
  import IconTile, { PORTAL_MONOGRAM, TILE_TONES } from '$components/IconTile.svelte';
  import NavTabs from '$components/NavTabs.svelte';
  import Spinner from '$components/Spinner.svelte';
  import Tooltip from '$components/Tooltip.svelte';
  import WindowControls from '$components/WindowControls.svelte';
  import { tooltip } from '$lib/actions/tooltip';
  import ColourBoard from './ColourBoard.svelte';
  import MotionBoard from './MotionBoard.svelte';
  import Section from './Section.svelte';
  import FeedbackBoard from './FeedbackBoard.svelte';
  import InputBoard from './InputBoard.svelte';
  import MatchBoard from './MatchBoard.svelte';
  import SurfaceBoard from './SurfaceBoard.svelte';
  import TokenBoards from './TokenBoards.svelte';
  import { text } from './gallery';

  const tabs = text.navigation.tabs.map((label, index) => ({ id: String(index), label }));
  let activeTab = $state('0');
  const noop = (): void => undefined;
</script>

<div class="gallery" data-testid="gallery">
  <header class="intro">
    <h1 class="title">{text.title}</h1>
    <p class="lead">{text.intro}</p>
  </header>

  <Section heading={text.sections.colours} id="colours">
    <ColourBoard />
  </Section>

  <TokenBoards />

  <MotionBoard />

  <Section heading={text.sections.icons} id="icons">
    {#each ['sm', 'md', 'lg'] as const as size (size)}
      <div class="icons">
        {#each ICON_NAMES as name (name)}
          <span class="icon-cell" use:tooltip={name}><Icon {name} {size} /></span>
        {/each}
      </div>
    {/each}
  </Section>

  <Section heading={text.sections.buttons} id="buttons">
    {#each BUTTON_VARIANTS as variant (variant)}
      <div class="matrix">
        <span class="row-label">{variant}</span>
        {#each BUTTON_SIZES as size (size)}
          <div class="button-row" data-testid="buttons-{variant}-{size}">
            <Button {variant} {size} label={text.buttons.fetch} onclick={noop} />
            <Button {variant} {size} label={text.buttons.save} icon="download" onclick={noop} />
            <Button {variant} {size} label={text.buttons.pin} icon="star" iconOnly onclick={noop} />
            <Button {variant} {size} label={text.buttons.fetch} icon="refresh-cw" loading />
            <Button
              {variant}
              {size}
              label={text.buttons.remove}
              icon="trash-2"
              disabled
              disabledReason={text.buttons.busy}
            />
          </div>
        {/each}
      </div>
    {/each}
    <div class="button-row">
      <Button
        variant="ghost"
        label={text.buttons.pin}
        icon="star"
        iconOnly
        pressed
        onclick={noop}
      />
      <Button
        variant="ghost"
        label={text.buttons.pin}
        icon="star"
        iconOnly
        pressed={false}
        onclick={noop}
      />
    </div>
  </Section>

  <Section heading={text.sections.navigation} id="navigation">
    <div class="bar">
      <NavTabs
        {tabs}
        active={activeTab}
        label={text.sections.navigation}
        onselect={(id) => (activeTab = id)}
      />
      <WindowControls />
    </div>
  </Section>

  <Section heading={text.sections.tiles} id="tiles">
    {#each ['sm', 'md', 'lg'] as const as size (size)}
      <div class="tiles">
        {#each TILE_TONES as tone (tone)}
          <IconTile {tone} {size} icon="sparkles" />
        {/each}
        {#each Object.values(PORTAL_MONOGRAM) as monogram (monogram)}
          <IconTile tone="slate" {size} {monogram} />
        {/each}
      </div>
    {/each}
  </Section>

  <Section heading={text.sections.empty} id="empty">
    <div class="empty">
      <EmptyState
        icon="mail"
        heading={text.empty.heading}
        text={text.empty.text}
        action={{ label: text.empty.action, icon: 'refresh-cw', onclick: noop }}
        secondary={{ label: text.empty.secondary, onclick: noop }}
      />
    </div>
  </Section>

  <Section heading={text.sections.activity} id="activity">
    <div class="tiles">
      <Spinner size="sm" />
      <Spinner size="md" />
      <Spinner size="lg" />
    </div>
  </Section>

  <SurfaceBoard />

  <InputBoard />

  <FeedbackBoard />

  <MatchBoard />

  <Tooltip />
</div>

<style>
  .gallery {
    display: flex;
    flex-direction: column;
    gap: var(--space-24);
    height: 100%;
    padding: var(--space-32);
    overflow: auto;
    background: var(--grad-wash);
    background-color: var(--bg);
  }

  .intro {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }

  .title {
    background: var(--grad-brand);
    background-clip: text;
    -webkit-background-clip: text;
    color: transparent;
    font: var(--type-display);
    letter-spacing: var(--tracking-tight);
    justify-self: start;
    align-self: flex-start;
  }

  .lead {
    color: var(--text-muted);
    font: var(--type-body);
  }

  .icons,
  .tiles {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-12);
  }

  .icon-cell {
    display: inline-flex;
    padding: var(--space-8);
    border-radius: var(--radius-sm);
    color: var(--text-heading);
  }

  .icon-cell:hover {
    background-color: var(--surface-hover);
  }

  .matrix {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
  }

  .row-label {
    color: var(--text-muted);
    font: var(--type-sm);
    font-weight: var(--weight-semibold);
  }

  .button-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-12);
  }

  .bar {
    display: flex;
    justify-content: space-between;
    height: var(--titlebar-height);
    padding-left: var(--space-16);
    border: var(--border-width) solid var(--border);
    border-radius: var(--radius-md);
    background-color: var(--surface);
    overflow: hidden;
  }

  .empty {
    display: flex;
    justify-content: center;
    padding: var(--space-32);
    border-radius: var(--radius-card);
    background-color: var(--bg);
  }
</style>
