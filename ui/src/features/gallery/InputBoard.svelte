<!-- Gallery: toggles, segmented controls, fields, chip fields, disclosure and setting rows. -->
<script lang="ts">
  import Badge from '$components/Badge.svelte';
  import ChipInput from '$components/ChipInput.svelte';
  import Disclosure from '$components/Disclosure.svelte';
  import Field from '$components/Field.svelte';
  import MenuButton from '$components/MenuButton.svelte';
  import Segmented from '$components/Segmented.svelte';
  import SettingRow from '$components/SettingRow.svelte';
  import TextArea from '$components/TextArea.svelte';
  import TextField from '$components/TextField.svelte';
  import Toggle from '$components/Toggle.svelte';
  import Section from './Section.svelte';
  import { text } from './gallery';

  const t = text.inputs;

  let autoFetch = $state(true);
  let other = $state(false);
  let facet = $state<'new' | 'all'>('new');
  let view = $state('new');
  let order = $state<'match' | 'date'>('match');
  const orders = [
    { id: 'match', label: t.orders[0] },
    { id: 'date', label: t.orders[1] },
  ] as const;
  const views = [
    { id: 'new', label: t.views[0], count: 0 },
    { id: 'all', label: t.views[1], count: 14 },
    { id: 'pinned', label: t.views[2], count: 1 },
    { id: 'applied', label: t.views[3], count: 2 },
  ];
  let sort = $state<'match' | 'newest' | 'portal'>('match');
  let address = $state('alerts@example.com');
  let password = $state('abcd efgh');
  let search = $state('Controlling');
  let empty = $state('');
  let open = $state(true);
  let tools = $state([...t.chipValues]);
  let industries = $state<string[]>([]);
  let focus = $state([...t.chipsShownValues]);
  let answer = $state('');

  /** A save that fails after a round trip (the dry run refuses it). */
  const failingSave = (): Promise<void> =>
    new Promise((_, reject) => setTimeout(() => reject(new Error('dry run')), 400));
</script>

<Section heading={t.heading} id="inputs">
  <div class="grid">
    <div class="stack">
      <div class="row">
        <Toggle checked={autoFetch} label={t.toggle} onchange={(v) => (autoFetch = v)} />
        <Toggle checked={other} label={t.toggle} onchange={(v) => (other = v)} />
        <Toggle
          checked={true}
          label={t.locked}
          disabled
          disabledReason={t.lockedReason}
          onchange={() => undefined}
        />
        <Toggle checked={autoFetch} label={t.toggle} showLabel onchange={(v) => (autoFetch = v)} />
        <!-- A save that fails after a while: the switch flips at once and slides back. -->
        <Toggle checked={false} label={t.toggle} onchange={failingSave} testid="toggle-fails" />
      </div>
      <div class="row">
        <Segmented
          label={t.facet}
          options={[
            { id: 'new', label: t.facets[0], count: 12 },
            { id: 'all', label: t.facets[1], count: 348 },
          ]}
          value={facet}
          onchange={(id) => (facet = id)}
          testid="segmented-facet"
        />
      </div>
      <!-- A native menu of choices below a quiet button (the sort of the list), and the same
           disabled with its reason. -->
      <div class="row">
        <MenuButton
          options={orders}
          value={order}
          onchange={(id) => (order = id)}
          testid="menu-order"
        />
        <MenuButton
          options={orders}
          value="date"
          disabled
          disabledReason={t.orderOff}
          onchange={() => undefined}
          testid="menu-order-off"
        />
      </div>
      <!-- Four options with counts: each pill covers exactly its option; in a narrow box the
           labels shorten instead of overlapping. -->
      <div class="row">
        <Segmented
          label={t.facet}
          size="sm"
          options={views}
          value={view}
          onchange={(id) => (view = id)}
          testid="segmented-views"
        />
      </div>
      <div class="row narrow">
        <Segmented
          label={t.facet}
          size="sm"
          options={views}
          value={view}
          onchange={(id) => (view = id)}
          testid="segmented-narrow"
        />
        <Segmented
          label={t.sort}
          size="sm"
          options={[
            { id: 'match', label: t.sorts[0] },
            { id: 'newest', label: t.sorts[1] },
            { id: 'portal', label: t.sorts[2] },
          ]}
          value={sort}
          onchange={(id) => (sort = id)}
        />
      </div>
    </div>
    <div class="stack">
      <Field label={t.address} for="gallery-address" hint={t.addressHint}>
        <TextField
          id="gallery-address"
          bind:value={address}
          describedby="gallery-address-message"
        />
      </Field>
      <Field
        label={t.password}
        for="gallery-password"
        error={t.passwordError}
        action={{ label: t.createPassword, icon: 'external-link', onclick: () => undefined }}
      >
        <TextField
          id="gallery-password"
          kind="password"
          bind:value={password}
          invalid
          describedby="gallery-password-message"
        />
      </Field>
      <TextField kind="search" label={t.search} placeholder={t.search} bind:value={search} />
      <TextField kind="search" label={t.search} placeholder={t.search} bind:value={empty} />
      <TextField label={t.address} bind:value={address} disabled />
    </div>
    <div class="stack">
      <Field label={t.chips} for="gallery-chips" hint={t.chipsHint}>
        <ChipInput
          id="gallery-chips"
          bind:values={tools}
          describedby="gallery-chips-message"
          testid="gallery-chips"
        />
      </Field>
      <Field label={t.chipsEmpty} for="gallery-chips-empty">
        <ChipInput
          id="gallery-chips-empty"
          bind:values={industries}
          placeholder={t.chipsPlaceholder}
        />
      </Field>
      <ChipInput label={t.chipsShown} bind:values={focus} entry={false} />
      <Field label={t.area} for="gallery-area">
        <TextArea id="gallery-area" bind:value={answer} rows={4} />
      </Field>
    </div>
  </div>

  <div class="panel">
    <!-- A switch row like the system settings: a click on its text toggles the switch. -->
    <SettingRow label={t.toggle} hint={t.toggleHint} for="gallery-auto-fetch">
      <Toggle
        id="gallery-auto-fetch"
        checked={autoFetch}
        label={t.toggle}
        testid="gallery-row-toggle"
        onchange={(v) => (autoFetch = v)}
      />
    </SettingRow>
    <SettingRow label={t.locked} hint={t.lockedReason}>
      {#snippet badges()}<Badge tone="warning" label={t.risk} icon="shield" />{/snippet}
      <Toggle
        checked={false}
        label={t.locked}
        disabled
        disabledReason={t.lockedReason}
        onchange={() => undefined}
      />
    </SettingRow>
    <Disclosure label={t.disclosure} bind:open testid="disclosure">
      <p class="copy">{t.disclosureText}</p>
    </Disclosure>
  </div>
</Section>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(var(--list-min), 1fr));
    gap: var(--space-32);
    align-items: start;
  }

  .stack {
    display: flex;
    flex-direction: column;
    gap: var(--space-20);
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-16);
  }

  .narrow {
    width: var(--stat-min);
  }

  .panel {
    max-width: var(--reader-width);
    overflow: hidden;
    padding: 0 var(--space-24);
    --row-inset: var(--space-24);
    border: var(--border-width) solid var(--border);
    border-radius: var(--radius-card);
  }

  .copy {
    color: var(--text-muted);
    font: var(--type-body);
  }
</style>
