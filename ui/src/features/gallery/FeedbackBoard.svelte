<!-- Gallery: score rings, stat tiles, notices and dialogs. -->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import Count, { COUNT_TONES } from '$components/Count.svelte';
  import Dialog from '$components/Dialog.svelte';
  import Notice, { NOTICE_TONES } from '$components/Notice.svelte';
  import ScoreRing, { type RingState } from '$components/ScoreRing.svelte';
  import StatTile from '$components/StatTile.svelte';
  import Section from './Section.svelte';
  import { text } from './gallery';

  const t = text.feedback;
  const noop = (): void => undefined;

  const rings: { id: string; state: RingState }[] = [
    { id: 'high', state: { status: 'scored', score: 91, band: 'high' } },
    { id: 'mid', state: { status: 'scored', score: 64, band: 'mid' } },
    { id: 'low', state: { status: 'scored', score: 28, band: 'low' } },
    { id: 'excluded', state: { status: 'excluded' } },
    { id: 'unscorable', state: { status: 'unscorable' } },
    { id: 'pending', state: { status: 'pending' } },
    { id: 'none', state: { status: 'none' } },
  ];

  let count = $state(12);
  let confirmOpen = $state(false);
  let dangerOpen = $state(false);
  /** The danger dialog fails like an action in the dry run: the error shows inside. */
  let dangerError = $state<string | null>(null);
</script>

<Section heading={t.rings} id="rings">
  {#each ['lg', 'md', 'sm'] as const as size (size)}
    <div class="row">
      {#each rings as ring (ring.id)}
        <ScoreRing ring={ring.state} {size} testid="ring-{ring.id}-{size}" />
      {/each}
    </div>
  {/each}
</Section>

<Section heading={t.stats} id="stats">
  <div class="row">
    <StatTile label={t.statNew} value={12} icon="inbox" hint={t.statHint} onclick={noop} />
    <StatTile label={t.statHigh} value={3} icon="star" tone="success" />
    <StatTile
      label={t.statIssues}
      value={1248}
      icon="triangle-alert"
      tone="warning"
      onclick={noop}
    />
  </div>
  <!-- Counts roll when they change on screen (not when they first appear). -->
  <div class="row">
    {#each COUNT_TONES as tone (tone)}
      <Count value={count} {tone} testid="count-{tone}" />
    {/each}
    <Button label={t.countMore} size="sm" onclick={() => (count += 1)} testid="count-more" />
    <Button label={t.countLess} size="sm" onclick={() => (count -= 1)} />
  </div>
</Section>

<Section heading={t.notices} id="notices">
  {#each NOTICE_TONES as tone (tone)}
    <Notice
      {tone}
      heading={t.noticeHeading}
      text={t.noticeText}
      action={{ label: t.noticeAction, onclick: noop }}
    />
  {/each}
  <div class="row">
    {#each NOTICE_TONES as tone (tone)}
      <Notice {tone} variant="inline" text={t.noticeHeading} />
    {/each}
  </div>
</Section>

<Section heading={t.dialogs} id="dialogs">
  <div class="row">
    <Button label={t.confirmOpen} onclick={() => (confirmOpen = true)} testid="open-confirm" />
    <Button
      label={t.dangerOpen}
      icon="rotate-ccw"
      onclick={() => (dangerOpen = true)}
      testid="open-danger"
    />
  </div>
  <Dialog
    bind:open={confirmOpen}
    heading={t.confirmHeading}
    text={t.confirmText}
    confirmLabel={t.confirmLabel}
    onconfirm={() => (confirmOpen = false)}
    testid="dialog-confirm"
  />
  <Dialog
    bind:open={dangerOpen}
    variant="danger"
    heading={t.dangerHeading}
    text={t.dangerText}
    confirmLabel={t.dangerLabel}
    error={dangerError}
    onconfirm={() => (dangerError = t.dangerError)}
    oncancel={() => (dangerError = null)}
    testid="dialog-danger"
  />
</Section>

<style>
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-16);
  }
</style>
