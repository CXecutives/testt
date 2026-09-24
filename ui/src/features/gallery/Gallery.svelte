<!--
  Component gallery (`?gallery`, development and harness builds only). Token boards first,
  then every component × variant × size × state. core/tests/ui_contract.rs requires that
  every file in components/ is imported here.
-->
<script lang="ts">
  import BrandMark from '$components/BrandMark.svelte';
  import Button, { BUTTON_SIZES, BUTTON_VARIANTS } from '$components/Button.svelte';
  import EmptyState from '$components/EmptyState.svelte';
  import Icon, { ICON_NAMES } from '$components/Icon.svelte';
  import IconTile, { PORTAL_MONOGRAM, TILE_TONES } from '$components/IconTile.svelte';
  import SideNav from '$components/SideNav.svelte';
  import StatusLine from '$components/StatusLine.svelte';
  import Spinner from '$components/Spinner.svelte';
  import Toast from '$components/Toast.svelte';
  import Tooltip from '$components/Tooltip.svelte';
  import { toasts } from '$lib/state/toasts.svelte';
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

  const NAV_ICONS = ['briefcase', 'user-round', 'sliders-horizontal'] as const;
  const tabs = text.navigation.tabs.map((label, index) => ({
    id: String(index),
    label,
    icon: NAV_ICONS[index] ?? 'briefcase',
    count: index === 0 ? 12 : null,
  }));
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
    <div class="navs">
      <div class="side">
        <SideNav
          items={tabs}
          active={activeTab}
          label={text.sections.navigation}
          onselect={(id) => (activeTab = id)}
        />
        <StatusLine text={text.navigation.status} label={text.navigation.status} onclick={noop} />
        <StatusLine
          text={text.navigation.running}
          label={text.navigation.running}
          busy
          progress={0.4}
          onclick={noop}
        />
      </div>
      <div class="side rail">
        <SideNav
          items={tabs}
          active={activeTab}
          label={text.sections.navigation}
          collapsed
          onselect={(id) => (activeTab = id)}
        />
        <StatusLine
          text={text.navigation.status}
          label={text.navigation.status}
          collapsed
          onclick={noop}
        />
      </div>
    </div>
    <div class="bar">
      <Button
        label={text.navigation.toast}
        onclick={() => toasts.show(text.navigation.toastText)}
      />
    </div>
  </Section>

  <Section heading={text.sections.tiles} id="tiles">
    {#each ['sm', 'md', 'lg'] as const as size (size)}
      <div class="tiles">
        {#each TILE_TONES as tone (tone)}
          <IconTile {tone} {size} icon="inbox" />
        {/each}
        {#each Object.values(PORTAL_MONOGRAM) as monogram (monogram)}
          <IconTile tone="neutral" {size} {monogram} />
        {/each}
        <BrandMark {size} label={text.title} />
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
    <div class="empty">
      <EmptyState text={text.empty.text} action={{ label: text.empty.action, onclick: noop }} />
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

  <Toast />
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
    background-color: var(--bg);
  }

  .intro {
    display: flex;
    flex-direction: column;
    gap: var(--space-8);
  }

  .title {
    color: var(--text-heading);
    font: var(--type-display);
    letter-spacing: var(--tracking-tight);
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
    font-weight: var(--weight-medium);
  }

  .button-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-12);
  }

  .navs {
    display: flex;
    gap: var(--space-16);
    margin-bottom: var(--space-16);
  }

  .side {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
    width: var(--sidebar-width);
    padding: var(--space-12);
    border-radius: var(--radius-md);
    border: var(--border-width) solid var(--border);
    background-color: var(--bg);
  }

  .side.rail {
    align-items: center;
    width: var(--rail-width);
  }

  .bar {
    display: flex;
    align-items: center;
  }

  .empty {
    display: flex;
    justify-content: center;
    padding: var(--space-32);
    border-radius: var(--radius-card);
    background-color: var(--bg);
  }
</style>
