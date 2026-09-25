<!--
  No profile yet (or one that no longer reads): one sentence what the profile is for and
  the three ways in, side by side as siblings: "Profil anlegen" (the primary), "Aus
  Lebenslauf erstellen" (with a prompt for an AI) and "Datei wählen" (an existing JSON
  file). A file that no longer reads also offers its folder, to fix it by hand. Sits at about
  38 % of the height.
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
    /** The stored file no longer reads: its folder is at hand. */
    unreadable?: boolean;
    note: string | null;
    oncreate: () => void;
    onfromcv: () => void;
    onpick: () => void;
    onopenfolder?: () => void;
  }

  let {
    heading,
    text,
    picking,
    unreadable = false,
    note,
    oncreate,
    onfromcv,
    onpick,
    onopenfolder,
  }: Props = $props();
</script>

<div class="start" data-testid="profile-empty">
  <EmptyState icon="file-text" {heading} {text} />
  <div class="ways">
    <Button
      variant="primary"
      icon="plus"
      label={t.profile.create}
      testid="profile-create"
      onclick={oncreate}
    />
    <Button
      variant="secondary"
      icon="clipboard-paste"
      label={t.profile.fromCv}
      testid="profile-from-cv"
      onclick={onfromcv}
    />
    <Button
      variant="secondary"
      icon="file-up"
      label={t.profile.pick}
      loading={picking}
      testid="profile-pick"
      onclick={onpick}
    />
  </div>
  {#if unreadable && onopenfolder}
    <Button
      variant="link"
      icon="folder-open"
      label={t.common.openFolder}
      testid="profile-folder"
      onclick={onopenfolder}
    />
  {/if}
  {#if note}
    <Notice tone="danger" variant="inline" text={note} testid="profile-note" />
  {/if}
</div>

<style>
  .start {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-16);
  }

  .ways {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: var(--space-12);
  }
</style>
