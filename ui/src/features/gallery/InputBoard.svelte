<!-- Gallery: toggles, segmented controls, fields, disclosure and setting rows. -->
<script lang="ts">
  import Badge from '$components/Badge.svelte';
  import Disclosure from '$components/Disclosure.svelte';
  import Field from '$components/Field.svelte';
  import Segmented from '$components/Segmented.svelte';
  import SettingRow from '$components/SettingRow.svelte';
  import TextField from '$components/TextField.svelte';
  import Toggle from '$components/Toggle.svelte';
  import Section from './Section.svelte';
  import { text } from './gallery';

  const t = text.inputs;

  let autoFetch = $state(true);
  let other = $state(false);
  let facet = $state<'new' | 'all'>('new');
  let sort = $state<'match' | 'newest' | 'portal'>('match');
  let address = $state('alerts@example.com');
  let password = $state('abcd efgh');
  let search = $state('Controlling');
  let empty = $state('');
  let open = $state(true);
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
      <Field label={t.password} for="gallery-password" error={t.passwordError}>
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
  </div>

  <div class="panel">
    <SettingRow label={t.toggle} hint={t.toggleHint}>
      <Toggle checked={autoFetch} label={t.toggle} onchange={(v) => (autoFetch = v)} />
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

  .panel {
    max-width: var(--reader-width);
    padding: 0 var(--space-24);
    border: var(--border-width) solid var(--border);
    border-radius: var(--radius-card);
  }

  .copy {
    color: var(--text-muted);
    font: var(--type-body);
  }
</style>
