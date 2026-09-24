<!-- Gallery: reason items and the job list with its entry and FLIP reordering. -->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import Chip, { CHIP_STATES, type ChipState } from '$components/Chip.svelte';
  import type { IconName } from '$components/Icon.svelte';
  import JobRow from '$components/JobRow.svelte';
  import ListRow from '$components/ListRow.svelte';
  import ReasonItem, { REASON_KINDS, REASON_WEIGHTS } from '$components/ReasonItem.svelte';
  import type { JobView } from '$lib/ipc/types';
  import { flip, rowIn } from '$lib/motion/transitions';
  import Section from './Section.svelte';
  import { sampleJobs, text } from './gallery';

  const t = text.match;
  const CHIP_ICON: Record<ChipState, IconName> = {
    met: 'check',
    unknown: 'circle-help',
    violated: 'x',
    unset: 'minus',
    plain: 'file-text',
  };
  const now = new Date();
  let jobs = $state(sampleJobs(now));
  let selected = $state('1001');
  let active = $state<string | null>(null);
  let run = $state(0);

  function shuffle(): void {
    jobs = [...jobs.slice(1), jobs[0]!];
  }

  /** The row tools of the gallery: pin or archive a sample job. */
  function toggle(job: JobView, field: 'pinned' | 'archived'): void {
    jobs = jobs.map((j) => (j.key.id === job.key.id ? { ...j, [field]: !j[field] } : j));
  }
</script>

<Section heading={t.reasons} id="reasons">
  <div class="reasons">
    {#each REASON_KINDS as kind, index (kind)}
      <ReasonItem
        {kind}
        label={t.reasonLabels[kind]}
        weight={REASON_WEIGHTS[index % REASON_WEIGHTS.length] ?? null}
        hint={kind === 'met' ? t.evidence : null}
        active={active === kind}
        onhover={(on) => (active = on ? kind : null)}
        onselect={() => undefined}
      />
    {/each}
    {#each REASON_KINDS as kind (kind)}
      <ReasonItem {kind} label={t.reasonLabels[kind]} compact />
    {/each}
  </div>
  <div class="chips">
    {#each CHIP_STATES as state (state)}
      <Chip
        {state}
        label={t.chipLabels[state]}
        icon={CHIP_ICON[state]}
        active={active === state}
        onhover={(on) => (active = on ? state : null)}
        onselect={state === 'plain' ? null : () => undefined}
      />
    {/each}
  </div>
</Section>

<Section heading={t.rows} id="rows">
  <div class="actions">
    <Button label={t.shuffle} icon="refresh-cw" onclick={shuffle} testid="rows-shuffle" />
    <Button label={t.replay} variant="ghost" onclick={() => (run += 1)} />
  </div>
  {#key run}
    <div class="list" data-testid="job-list">
      {#each jobs as job, index (job.key.id)}
        <div animate:flip={{ count: jobs.length }} in:rowIn|global={{ index, fresh: true }}>
          <JobRow
            {job}
            {now}
            selected={selected === job.key.id}
            onselect={(j) => (selected = j.key.id)}
            onpin={(j) => toggle(j, 'pinned')}
            onarchive={(j) => toggle(j, 'archived')}
          />
        </div>
      {/each}
      <ListRow>
        <span class="plain">{t.rows}</span>
      </ListRow>
    </div>
  {/key}
</Section>

<style>
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-6);
    margin-top: var(--space-16);
  }

  .reasons {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(var(--list-min), 1fr));
    gap: var(--space-4) var(--space-24);
    max-width: var(--reader-width);
  }

  .actions {
    display: flex;
    gap: var(--space-12);
  }

  .list {
    max-width: var(--list-max);
    overflow: hidden;
    border: var(--border-width) solid var(--border);
    border-radius: var(--radius-card);
    background-color: var(--surface);
  }

  .plain {
    color: var(--text-muted);
    font: var(--type-sm);
  }
</style>
