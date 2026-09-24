// Enter in a row list of the Profil form (competences, languages), like a table in a native
// app: Enter goes to the next row, on the last row it adds a new one, and on an empty last
// row it ends the list (the row goes, the caret moves to the next field after the list).

import { tick } from 'svelte';

/** The rows of a list container, in order. */
const rowsOf = (list: HTMLElement): HTMLElement[] => [
  ...list.querySelectorAll<HTMLElement>('[data-row]'),
];

/** The caret into the first field of the row at `index`. */
export function focusRow(list: HTMLElement | null, index: number): void {
  if (list === null) return;
  rowsOf(list)[index]?.querySelector('input')?.focus();
}

/** The caret into the first field after the list. */
function moveOn(list: HTMLElement): void {
  const after = [...document.querySelectorAll<HTMLElement>('input, textarea')].find(
    (field) =>
      !list.contains(field) &&
      (list.compareDocumentPosition(field) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0,
  );
  after?.focus();
}

export interface RowEnter<Row> {
  list: HTMLElement | null;
  rows: Row[];
  row: Row;
  blank: (row: Row) => boolean;
  add: () => void;
  remove: (row: Row) => void;
}

/** What Enter does inside `row`. */
export async function enterRow<Row>({
  list,
  rows,
  row,
  blank,
  add,
  remove,
}: RowEnter<Row>): Promise<void> {
  if (list === null) return;
  const index = rows.indexOf(row);
  if (index < rows.length - 1) {
    focusRow(list, index + 1);
    return;
  }
  if (blank(row)) {
    remove(row);
    await tick();
    moveOn(list);
    return;
  }
  add();
  await tick();
  focusRow(list, index + 1);
}
