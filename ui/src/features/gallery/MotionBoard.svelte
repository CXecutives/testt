<!--
  Motion demo: every duration × easing as a bar, a list whose items rise in together and a
  count-up. Everything follows reduced motion.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import { cssVars } from '$lib/actions/cssVars';
  import { formatPercent } from '$lib/i18n/format';
  import { isReducedMotion, onMotionChange } from '$lib/motion/motion';
  import { countUp, rise } from '$lib/motion/transitions';
  import Section from './Section.svelte';
  import { durations, easings, text } from './gallery';

  let run = $state(0);
  let played = $state(false);
  let reduced = $state(isReducedMotion());
  const counter = countUp(0);

  $effect(() => onMotionChange((value) => (reduced = value)));

  function playAll(): void {
    played = false;
    counter.set(0, { duration: 0 });
    requestAnimationFrame(() => {
      played = true;
      run += 1;
      counter.target = 87;
    });
  }

  const items = Array.from({ length: 12 }, (_, i) => i + 1);
</script>

<Section heading={text.sections.motion} id="motion">
  <div class="head">
    <Button label={text.motion.play} icon="refresh-cw" onclick={playAll} testid="motion-play" />
    <span class="state" data-testid="motion-state"
      >{reduced ? text.motion.reduced : text.motion.full}</span
    >
  </div>

  <div class="bars" class:played>
    {#each durations as name (name)}
      {#each easings as ease (ease)}
        <div class="bar-row">
          <span class="label">{name} · {ease}</span>
          <span class="track">
            <span
              class="fill"
              use:cssVars={{ duration: `var(--dur-${name})`, ease: `var(--ease-${ease})` }}
            ></span>
          </span>
        </div>
      {/each}
    {/each}
  </div>

  <div class="demo">
    <div class="count" data-testid="motion-count">
      <span class="count-label">{text.motion.count}</span>
      <span class="count-value">{formatPercent(Math.round(counter.current))}</span>
    </div>
    {#key run}
      <ol class="list">
        {#each items as item (item)}
          <li class="item" in:rise|global>{text.motion.item} {item}</li>
        {/each}
      </ol>
    {/key}
  </div>
</Section>

<style>
  .head {
    display: flex;
    align-items: center;
    gap: var(--space-16);
  }

  .state,
  .label,
  .count-label {
    color: var(--text-muted);
    font: var(--type-sm);
  }

  .bars {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(var(--list-min), 1fr));
    gap: var(--space-8) var(--space-24);
  }

  .bar-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 2fr);
    align-items: center;
    gap: var(--space-12);
  }

  .track {
    position: relative;
    height: var(--space-8);
    overflow: hidden;
    border-radius: var(--radius-full);
    background-color: var(--surface-muted);
  }

  .fill {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background-color: var(--accent);
    transform: scaleX(0);
    transform-origin: left center;
  }

  .played .fill {
    transform: scaleX(1);
    transition: transform var(--duration) var(--ease);
  }

  .demo {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    align-items: start;
    gap: var(--space-32);
  }

  .count {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .count-value {
    color: var(--score-high-text);
    font: var(--type-display);
    font-variant-numeric: var(--numeric);
    letter-spacing: var(--tracking-tight);
  }

  .list {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-8);
  }

  .item {
    padding: var(--space-6) var(--space-12);
    border: var(--border-width) solid var(--border);
    border-radius: var(--radius-full);
    background-color: var(--surface-selected);
    color: var(--accent-text);
    font: var(--type-sm);
    font-weight: var(--weight-medium);
  }
</style>
