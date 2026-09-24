<!-- Every colour token with its computed value and WCAG contrast, measured in the browser. -->
<script lang="ts">
  import { cssVars, setVars } from '$lib/actions/cssVars';
  import {
    colourGroups,
    contrast,
    over,
    parseColour,
    text,
    type ColourToken,
    type Rgba,
  } from './gallery';

  const WHITE: Rgba = [255, 255, 255, 1];

  let probe = $state<HTMLElement | null>(null);

  function resolve(name: string): Rgba | null {
    if (probe === null) return null;
    setVars(probe, { probe: `var(--${name})` });
    return parseColour(getComputedStyle(probe).backgroundColor);
  }

  interface Row {
    token: ColourToken;
    value: string;
    checks: { label: string; ratio: number; min: number }[];
  }

  function fmt(ratio: number): string {
    return ratio.toFixed(2).replace('.', ',');
  }

  const rows = $derived.by((): Row[] => {
    if (probe === null) return [];
    const surface = resolve('surface') ?? WHITE;
    const bg = resolve('bg') ?? WHITE;
    const ink = resolve('text') ?? WHITE;
    const onAccent = resolve('text-on-accent') ?? WHITE;
    return colourGroups.flatMap((group) =>
      group.tokens.map((token): Row => {
        const raw = resolve(token.name) ?? WHITE;
        const onSurface = over(raw, surface);
        const onBg = over(raw, bg);
        const value = `rgb(${onSurface.slice(0, 3).map(Math.round).join(' ')})`;
        const checks: Row['checks'] = [];
        if (token.role === 'text') {
          checks.push({ label: 'surface', ratio: contrast(onSurface, surface), min: 4.5 });
          checks.push({ label: 'bg', ratio: contrast(onBg, bg), min: 4.5 });
        } else if (token.role === 'surface') {
          checks.push({ label: 'text', ratio: contrast(ink, onSurface), min: 4.5 });
        } else if (token.role === 'fill') {
          checks.push({ label: 'text-on-accent', ratio: contrast(onAccent, onSurface), min: 4.5 });
        }
        return { token, value, checks };
      }),
    );
  });

  function verdict(ratio: number, min: number, exception: boolean): string {
    if (ratio >= min) return text.contrast.pass;
    if (exception) return text.contrast.exception;
    return ratio >= 3 ? text.contrast.large : text.contrast.fail;
  }
</script>

<span class="probe" bind:this={probe} aria-hidden="true"></span>

{#each colourGroups as group (group.title)}
  <div class="group">
    <h3 class="group-title">{group.title}</h3>
    <div class="grid">
      {#each rows.filter((row) => group.tokens.includes(row.token)) as row (row.token.name)}
        <div class="swatch-row" data-testid="swatch-{row.token.name}">
          <span class="swatch" use:cssVars={{ swatch: `var(--${row.token.name})` }}></span>
          <div class="meta">
            <span class="name">--{row.token.name}</span>
            <span class="value">{row.value}</span>
            {#each row.checks as check (check.label)}
              <span
                class="check"
                class:ok={check.ratio >= check.min}
                class:exception={check.ratio < check.min && row.token.exception}
              >
                {fmt(check.ratio)} · {check.label} · {verdict(
                  check.ratio,
                  check.min,
                  row.token.exception ?? false,
                )}
              </span>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  </div>
{/each}

<style>
  .probe {
    position: absolute;
    width: 0;
    height: 0;
    background-color: var(--probe);
  }

  .group {
    display: flex;
    flex-direction: column;
    gap: var(--space-12);
  }

  .group-title {
    color: var(--text-muted);
    font: var(--type-sm);
    font-weight: var(--weight-medium);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(var(--list-min), 1fr));
    gap: var(--space-12);
  }

  .swatch-row {
    display: flex;
    align-items: center;
    gap: var(--space-12);
  }

  .swatch {
    flex: none;
    width: var(--tile-lg);
    height: var(--tile-lg);
    border: var(--border-width) solid var(--border);
    border-radius: var(--radius-md);
    background: var(--swatch);
  }

  .meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
    font: var(--type-xs);
  }

  .name {
    color: var(--text);
    font-weight: var(--weight-medium);
  }

  .value {
    color: var(--text-subtle);
  }

  .check {
    color: var(--danger-strong);
  }

  .check.ok {
    color: var(--success-strong);
  }

  .check.exception {
    color: var(--warning-strong);
  }
</style>
