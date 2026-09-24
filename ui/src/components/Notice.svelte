<!--
  Feedback where the action happened: info | success | warning | danger, inline (icon and
  sentence in the tone), banner (tinted box) or row (a calm line inside a card: the icon in
  the tone, the text in ink), with at most one action.
-->
<script lang="ts" module>
  export type NoticeTone = 'info' | 'success' | 'warning' | 'danger';
  export const NOTICE_TONES: readonly NoticeTone[] = ['info', 'success', 'warning', 'danger'];
</script>

<script lang="ts">
  import Button from './Button.svelte';
  import Icon, { type IconName } from './Icon.svelte';

  interface Props {
    tone?: NoticeTone;
    variant?: 'inline' | 'banner' | 'row';
    heading?: string | null;
    text: string;
    action?: { label: string; onclick: () => void } | null;
    testid?: string | null;
  }

  let {
    tone = 'info',
    variant = 'banner',
    heading = null,
    text,
    action = null,
    testid = null,
  }: Props = $props();

  const ICONS: Record<NoticeTone, IconName> = {
    info: 'info',
    success: 'check',
    warning: 'triangle-alert',
    danger: 'triangle-alert',
  };
</script>

<div
  class="notice {tone} {variant}"
  role={tone === 'danger' || tone === 'warning' ? 'alert' : 'status'}
  data-testid={testid ?? undefined}
>
  <span class="icon"><Icon name={ICONS[tone]} size="sm" /></span>
  <div class="copy">
    {#if heading}<p class="heading">{heading}</p>{/if}
    <p class="text">{text}</p>
  </div>
  {#if action}
    <span class="action">
      <Button variant="secondary" size="sm" label={action.label} onclick={action.onclick} />
    </span>
  {/if}
</div>

<style>
  .notice {
    display: flex;
    align-items: flex-start;
    gap: var(--space-8);
    color: var(--notice-fg);
    font: var(--type-sm);
  }

  .banner {
    align-items: center;
    gap: var(--space-12);
    padding: var(--space-12) var(--space-16);
    border: var(--border-width) solid var(--notice-line);
    border-radius: var(--radius-card);
    background-color: var(--notice-bg);
    color: var(--text);
    font: var(--type-md);
  }

  .row {
    align-items: center;
    gap: var(--space-12);
    color: var(--text);
    font: var(--type-md);
  }

  .icon {
    display: inline-flex;
    flex: none;
    padding-top: var(--space-2);
    color: var(--notice-fg);
  }

  .banner .icon,
  .row .icon {
    padding-top: 0;
  }

  .copy {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: var(--space-2);
    min-width: 0;
  }

  .heading {
    font-weight: var(--weight-semibold);
  }

  .action {
    flex: none;
  }

  .info {
    --notice-fg: var(--info-strong);
    --notice-bg: var(--info-soft);
    --notice-line: var(--info-soft);
  }

  .success {
    --notice-fg: var(--success-strong);
    --notice-bg: var(--success-soft);
    --notice-line: var(--success-soft);
  }

  .warning {
    --notice-fg: var(--warning-strong);
    --notice-bg: var(--warning-soft);
    --notice-line: var(--warning);
  }

  .danger {
    --notice-fg: var(--danger-strong);
    --notice-bg: var(--danger-soft);
    --notice-line: var(--danger-soft);
  }
</style>
