<!--
  Motion demo: every duration × easing as a bar, a list whose items rise in together and a
  count-up; then the micro-interactions of the controls: press scales, icon nudges (hover),
  the pin pop, the sort turn, a rolling count, a drawn check, a passage flash and the shake
  of a field. Everything follows reduced motion.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import Count from '$components/Count.svelte';
  import Icon from '$components/Icon.svelte';
  import TextField from '$components/TextField.svelte';
  import { cssVars } from '$lib/actions/cssVars';
  import { formatPercent } from '$lib/i18n/format';
  import { isReducedMotion, onMotionChange } from '$lib/motion/motion';
  import { countUp, rise } from '$lib/motion/transitions';
  import Section from './Section.svelte';
  import { durations, easings, text } from './gallery';

  const m = text.motion;
  let run = $state(0);
  let played = $state(false);
  let reduced = $state(isReducedMotion());
  const counter = countUp(0);

  let pinned = $state(false);
  let newest = $state(false);
  let rolled = $state(4);
  let once = $state(0);
  let word = $state('');
  let field = $state<{ shake: () => void } | null>(null);

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

  <h3 class="sub">{m.micro}</h3>
  <div class="micro" data-testid="motion-micro">
    <Button variant="primary" icon="refresh-cw" label={m.fetch} />
    <Button icon="external-link" label={m.open} />
    <Button variant="ghost" icon="download" label={m.save} />
    <Button variant="ghost" icon="chevron-left" label={m.back} />
    <Button variant="ghost" icon="trash-2" label={m.remove} />
    <Button
      variant="ghost"
      iconOnly
      icon="star"
      label={m.pin}
      pressed={pinned}
      testid="motion-pin"
      onclick={() => (pinned = !pinned)}
    />
    <Button
      variant="ghost"
      iconOnly
      icon="arrow-up-down"
      label={newest ? m.newest : m.best}
      turned={newest}
      testid="motion-sort"
      onclick={() => (newest = !newest)}
    />
    <Button variant="link" icon="external-link" external label={m.link} />
  </div>
  <div class="micro">
    <Count value={rolled} />
    <Button size="sm" label={m.more} onclick={() => (rolled += 1)} />
    <Button size="sm" label={m.less} onclick={() => (rolled = Math.max(0, rolled - 1))} />
    <Button size="sm" icon="refresh-cw" label={m.again} onclick={() => (once += 1)} />
    {#key once}
      <span class="drawn" aria-hidden="true"><Icon name="check" size="md" /></span>
      <span class="passage">{m.passage}</span>
    {/key}
  </div>
  <div class="micro">
    <span class="shake-field">
      <TextField bind:this={field} bind:value={word} label={m.password} kind="password" />
    </span>
    <Button size="sm" label={m.shake} onclick={() => field?.shake()} testid="motion-shake" />
  </div>
</Section>

<style>
  .sub {
    color: var(--text-label);
    font: var(--type-sm);
    font-weight: var(--weight-medium);
  }

  .micro {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-12);
  }

  /* A check that draws itself (the run steps, the toast, the first-run stepper). */
  .drawn {
    display: inline-flex;
    color: var(--success-strong);
  }

  .drawn :global(path) {
    stroke-dasharray: var(--draw-length);
    animation: draw var(--dur-slow) var(--ease-out) both;
  }

  /* A passage of the ad that flashes once after a jump to it. */
  .passage {
    position: relative;
    padding: 0 var(--space-4);
    border-radius: var(--radius-xs);
    font: var(--type-body);
  }

  .passage::after {
    position: absolute;
    z-index: var(--z-below);
    inset: 0;
    border-radius: inherit;
    background-color: var(--score-high-surface);
    content: '';
    animation: pulse var(--dur-reveal) var(--ease-standard) both;
  }

  .shake-field {
    width: var(--stat-min);
  }

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
    background-color: var(--meter-fill);
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
    background-color: var(--active-surface);
    color: var(--active-text);
    font: var(--type-sm);
    font-weight: var(--weight-medium);
  }
</style>
