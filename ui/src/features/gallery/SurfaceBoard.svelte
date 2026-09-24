<!-- Gallery: cards, badges, skeletons and meters. -->
<script lang="ts">
  import Badge, { BADGE_TONES } from '$components/Badge.svelte';
  import Card from '$components/Card.svelte';
  import Meter from '$components/Meter.svelte';
  import Skeleton from '$components/Skeleton.svelte';
  import Section from './Section.svelte';
  import { text } from './gallery';

  const t = text.surfaces;
  const noop = (): void => undefined;
</script>

<Section heading={t.cards} id="cards">
  <div class="grid">
    <Card><p class="copy">{t.plain}</p></Card>
    <Card variant="interactive" onclick={noop} testid="card-interactive">
      <p class="copy">{t.interactive}</p>
    </Card>
    <Card variant="tinted"><p class="copy">{t.tinted}</p></Card>
  </div>
</Section>

<Section heading={t.badges} id="badges">
  <div class="row">
    {#each BADGE_TONES as tone (tone)}
      <Badge {tone} label={t.badge} />
    {/each}
    {#each BADGE_TONES as tone (tone)}
      <Badge {tone} label={t.badge} icon="info" />
    {/each}
  </div>
</Section>

<Section heading={t.loading} id="loading">
  <div class="grid">
    <div class="stack">
      <Skeleton shape="line" width={90} />
      <Skeleton shape="line" width={60} />
      <Skeleton shape="line" width={75} />
    </div>
    <Skeleton shape="block" />
    <div class="row">
      <Skeleton shape="circle" size="sm" />
      <Skeleton shape="circle" size="md" />
      <Skeleton shape="circle" size="lg" />
    </div>
  </div>
  <div class="stack">
    {#each ['sm', 'md', 'lg'] as const as size (size)}
      <Meter value={0.62} {size} label={t.meter} />
      <Meter value={0.35} {size} tone="neutral" label={t.meter} />
      <Meter value={0.8} {size} tone="warning" label={t.meter} />
      <Meter value={null} {size} label={t.meter} />
    {/each}
  </div>
</Section>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(var(--list-min), 1fr));
    gap: var(--space-24);
    align-items: start;
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-12);
  }

  .stack {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
  }

  .copy {
    color: var(--text);
    font: var(--type-body);
  }
</style>
