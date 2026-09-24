<!--
  No profile yet (or one that no longer reads): one sentence what the profile is for and
  the three ways in. "Profil anlegen" is the primary, "Aus Lebenslauf erstellen" the way for
  people who use Claude, "Datei wählen" for an existing JSON file. Sits at about 38 % of
  the height.
-->
<script lang="ts">
  import Button from '$components/Button.svelte';
  import EmptyState from '$components/EmptyState.svelte';
  import Notice from '$components/Notice.svelte';
  import { t } from '$lib/i18n/t';

  interface Props {
    heading: string;
    text: string;
    picking: boolean;
    note: string | null;
    oncreate: () => void;
    onfromcv: () => void;
    onpick: () => void;
  }

  let { heading, text, picking, note, oncreate, onfromcv, onpick }: Props = $props();
</script>

<div class="start" data-testid="profile-empty">
  <EmptyState
    icon="file-text"
    {heading}
    {text}
    action={{ label: t.profile.create, onclick: oncreate }}
    secondary={{ label: t.profile.fromCv, onclick: onfromcv }}
  />
  <Button
    variant="ghost"
    icon="file-up"
    label={t.profile.pick}
    loading={picking}
    testid="profile-pick"
    onclick={onpick}
  />
  {#if note}
    <Notice tone="danger" variant="inline" text={note} testid="profile-note" />
  {/if}
</div>

<style>
  .start {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-8);
  }
</style>
