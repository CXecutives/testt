<!-- Type scale, spacing, radii, shadows and gradients, each drawn from its token. -->
<script lang="ts">
  import { cssVars } from '$lib/actions/cssVars';
  import Section from './Section.svelte';
  import { gradients, radii, shadows, spacing, text, typeScale } from './gallery';
</script>

<Section heading={text.sections.type} id="type">
  <div class="type-list">
    {#each typeScale as step (step.name)}
      <div class="type-row">
        <span class="label">{step.name} · {step.spec}</span>
        <span
          class="type-sample"
          class:tight={['xl', '2xl', 'display'].includes(step.name)}
          use:cssVars={{ sample: `var(--type-${step.name})` }}>{text.sample}</span
        >
      </div>
    {/each}
  </div>
</Section>

<Section heading={text.sections.spacing} id="spacing">
  <div class="stack">
    {#each spacing as size (size)}
      <div class="space-row">
        <span class="label">{size}</span>
        <span class="bar" use:cssVars={{ size: `var(--space-${size})` }}></span>
      </div>
    {/each}
  </div>
</Section>

<Section heading={text.sections.radii} id="radii">
  <div class="tiles">
    {#each radii as radius (radius)}
      <div class="tile-box">
        <span class="radius" use:cssVars={{ radius: `var(--radius-${radius})` }}></span>
        <span class="label">{radius}</span>
      </div>
    {/each}
  </div>
</Section>

<Section heading={text.sections.shadows} id="shadows">
  <div class="tiles">
    {#each shadows as shadow (shadow)}
      <div class="tile-box">
        <span class="shadow" use:cssVars={{ shadow: `var(--${shadow})` }}></span>
        <span class="label">{shadow}</span>
      </div>
    {/each}
  </div>
</Section>

<Section heading={text.sections.gradients} id="gradients">
  <div class="tiles">
    {#each gradients as gradient (gradient)}
      <div class="tile-box">
        <span class="gradient" use:cssVars={{ gradient: `var(--${gradient})` }}></span>
        <span class="label">{gradient}</span>
      </div>
    {/each}
  </div>
</Section>

<style>
  .label {
    color: var(--text-subtle);
    font: var(--type-xs);
  }

  .type-list,
  .stack {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
  }

  .type-row {
    display: grid;
    grid-template-columns: var(--space-64) minmax(0, 1fr);
    align-items: baseline;
    gap: var(--space-4) var(--space-24);
  }

  .type-row .label {
    grid-column: 1 / -1;
  }

  .type-sample {
    grid-column: 1 / -1;
    overflow: hidden;
    color: var(--text);
    font: var(--sample);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tight {
    letter-spacing: var(--tracking-tight);
  }

  .space-row {
    display: grid;
    grid-template-columns: var(--space-40) minmax(0, 1fr);
    align-items: center;
    gap: var(--space-12);
  }

  .bar {
    width: var(--size);
    height: var(--space-12);
    border-radius: var(--radius-full);
    background-color: var(--accent);
  }

  .tiles {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-24);
  }

  .tile-box {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-8);
  }

  .radius,
  .shadow,
  .gradient {
    display: block;
    width: var(--row-height);
    height: var(--row-height);
    border: var(--border-width) solid var(--border);
    background-color: var(--surface);
  }

  .radius {
    border-radius: var(--radius);
    background-color: var(--surface-tinted);
  }

  .shadow {
    border-radius: var(--radius-card);
    box-shadow: var(--shadow);
  }

  .gradient {
    border-radius: var(--radius-card);
    background: var(--gradient);
  }
</style>
