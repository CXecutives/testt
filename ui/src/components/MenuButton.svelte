<!--
  A quiet button that names the current choice, with a chevron, and opens the OS's own menu
  of the choices right below it (the current one checked): the sort control of the list
  ("Nach Passung" / "Nach Datum"). Left click only, like every control; disabled it stays
  hoverable so the tooltip can say why.
-->
<script lang="ts" module>
  export interface MenuOption<Id extends string = string> {
    id: Id;
    label: string;
  }
</script>

<script lang="ts" generics="Id extends string">
  import { popupChoiceMenu } from '$lib/ipc/api';
  import { tokenPx } from '$lib/tokens';
  import Button from './Button.svelte';
  import type { IconName } from './Icon.svelte';

  interface Props {
    options: readonly MenuOption<Id>[];
    value: Id;
    /** A glyph before the label (optional). */
    icon?: IconName | null;
    disabled?: boolean;
    disabledReason?: string | null;
    testid?: string | null;
    onchange: (id: Id) => void;
  }

  let {
    options,
    value,
    icon = null,
    disabled = false,
    disabledReason = null,
    testid = null,
    onchange,
  }: Props = $props();

  let anchor = $state<HTMLElement | null>(null);
  const current = $derived(options.find((option) => option.id === value) ?? options[0]);

  /** The menu opens right below the button, its left edge on the button's. */
  function open(): void {
    const box = anchor?.getBoundingClientRect();
    const at = box ? { x: box.left, y: box.bottom + tokenPx('--menu-gap') } : null;
    void popupChoiceMenu(
      options.map((option) => ({
        text: option.label,
        checked: option.id === value,
        onchoose: () => {
          if (option.id !== value) onchange(option.id);
        },
      })),
      at,
    );
  }
</script>

<span class="menu-button" bind:this={anchor}>
  <Button
    variant="ghost"
    size="sm"
    label={current?.label ?? ''}
    {icon}
    trailing="chevron-down"
    menu
    {disabled}
    {disabledReason}
    {testid}
    onclick={open}
  />
</span>

<style>
  .menu-button {
    display: inline-flex;
    flex: none;
  }
</style>
