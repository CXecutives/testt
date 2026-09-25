// The only importer of @tauri-apps/api (eslint + core/tests/ui_contract.rs). Everything else
// talks to the backend through these functions, which is also what lets the harness swap
// Tauri for a typed stub (vite `--mode harness`).
//
// Run events: every command that takes a `channel` (`app_state` attaches - also to a run
// that is already going after a reload -, `start_run` reports) gets a Channel of its own;
// `onRun` fans the events of all of them out. Callers never pass the channel.
// One channel per call is required: Tauri numbers the messages of each Rust-side channel
// from 0 and the JS channel delivers them in that order, and when the Rust side drops its
// channel it unregisters the JS one. A shared JS channel therefore swallows every event of
// the second run (the run finished in the backend, the UI never heard of it).
//
// Types: `Commands` is generated from the Rust command table (types/commands.ts).

import { Channel, invoke as tauriInvoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { LogicalPosition } from '@tauri-apps/api/dpi';
import { CheckMenuItem, Menu, MenuItem, PredefinedMenuItem } from '@tauri-apps/api/menu';
import type { Commands, ErrorInfo, ErrorKind, RunEvent } from './types';

export type CommandName = keyof Commands;
/** Arguments as the UI passes them: everything but the run channel. */
export type CommandArgs<K extends CommandName> = Omit<Commands[K]['args'], 'channel'>;
export type CommandResult<K extends CommandName> = Commands[K]['result'];
type Params = ErrorInfo['params'];

/** All commands, in the order of docs/PLAN.md. */
export const COMMAND_NAMES = [
  'app_state',
  'start_run',
  'cancel_run',
  'list_jobs',
  'job_detail',
  'mark_read',
  'mark_all_read',
  'mark_unread',
  'set_pinned',
  'move_jobs',
  'set_override',
  'purge_jobs',
  'empty_trash',
  'ai_prompt',
  'ai_prompt_top',
  'pick_profile',
  'parse_profile',
  'profile_prompt',
  'save_profile',
  'remove_profile',
  'save_mailbox',
  'remove_mailbox',
  'portal_login',
  'portal_logout',
  'pick_workspace',
  'rewrite_txt',
  'clear_txt',
  'open_target',
  'save_settings',
  'reset_all',
  'report_ui_error',
] as const satisfies readonly CommandName[];

/** Commands that receive the run channel as `channel` argument. */
const STREAMING: ReadonlySet<CommandName> = new Set<CommandName>(['app_state', 'start_run']);

/**
 * A failed command: `kind` and `params` come from the backend (ErrorInfo), texts from the
 * catalog. `unknown` marks something that did not arrive as ErrorInfo (a bug of ours).
 */
export class IpcError extends Error {
  readonly kind: ErrorKind | 'unknown';
  readonly params: Params;

  constructor(kind: ErrorKind | 'unknown', params: Params = {}) {
    super(kind);
    this.name = 'IpcError';
    this.kind = kind;
    this.params = params;
  }
}

function isErrorInfo(value: unknown): value is ErrorInfo {
  return (
    typeof value === 'object' &&
    value !== null &&
    typeof (value as { kind?: unknown }).kind === 'string'
  );
}

function toIpcError(error: unknown): IpcError {
  if (error instanceof IpcError) return error;
  if (isErrorInfo(error)) return new IpcError(error.kind, error.params ?? {});
  return new IpcError('unknown', { detail: String(error).slice(0, 200) });
}

type RunHandler = (event: RunEvent) => void;
const runHandlers = new Set<RunHandler>();

/** A fresh channel for one command call; its events go to every run handler. */
function channel(): Channel<RunEvent> {
  const next = new Channel<RunEvent>();
  next.onmessage = (event) => {
    for (const handler of runHandlers) handler(event);
  };
  return next;
}

/** Subscribe to run events (progress, status, job updates, ...). Returns an unsubscribe. */
export function onRun(handler: RunHandler): () => void {
  runHandlers.add(handler);
  return () => runHandlers.delete(handler);
}

type ArgsTuple<K extends CommandName> = [CommandArgs<K>] extends [Record<string, never>]
  ? [args?: CommandArgs<K>]
  : [args: CommandArgs<K>];

/** Call a backend command, typed by the command map. Rejects with an IpcError. */
export async function invoke<K extends CommandName>(
  command: K,
  ...[args]: ArgsTuple<K>
): Promise<CommandResult<K>> {
  const payload: Record<string, unknown> = { ...(args ?? {}) };
  if (STREAMING.has(command)) payload.channel = channel();
  try {
    return await tauriInvoke<CommandResult<K>>(command, payload);
  } catch (error) {
    throw toIpcError(error);
  }
}

/** Subscribes to a Tauri event and returns a synchronous unsubscribe function. */
function subscribe(start: () => Promise<() => void>): () => void {
  let stop: (() => void) | null = null;
  let cancelled = false;
  void start().then(
    (unlisten) => {
      if (cancelled) unlisten();
      else stop = unlisten;
    },
    () => undefined,
  );
  return () => {
    cancelled = true;
    stop?.();
  };
}

/**
 * The window has been asked to close while a fetch runs: it stays until the run has stopped
 * (at most ten seconds, src-tauri/src/main.rs). Returns an unsubscribe function.
 */
export function onClosing(handler: () => void): () => void {
  return subscribe(() => listen('closing', () => handler()));
}

/**
 * The OS window gained (true) or lost (false) the focus: Tauri's window events, the moment
 * the native title bar dims. Returns an unsubscribe function.
 */
export function onWindowFocus(handler: (focused: boolean) => void): () => void {
  const stopFocus = subscribe(() => listen('tauri://focus', () => handler(true)));
  const stopBlur = subscribe(() => listen('tauri://blur', () => handler(false)));
  return () => {
    stopFocus();
    stopBlur();
  };
}

/**
 * The native menu asks for a view (macOS: "Einstellungen …" with Cmd+, in the app menu,
 * src-tauri/src/platform.rs). Returns an unsubscribe function.
 */
export function onNavigate(handler: (view: string) => void): () => void {
  return subscribe(() => listen<string>('navigate', (event) => handler(event.payload)));
}

/**
 * The native menu folds the sidebar or unfolds it (macOS: "Seitenleiste ein-/ausblenden" in
 * the View menu, src-tauri/src/platform.rs). Its key, Cmd+B, never reaches the menu from the
 * page: lib/input/input.ts takes it first. Returns an unsubscribe function.
 */
export function onSidebarMenu(handler: () => void): () => void {
  return subscribe(() => listen('sidebar', () => handler()));
}

/** An edit command the OS has a menu item of its own for (it acts on the focused field). */
export type EditCommand = 'Undo' | 'Cut' | 'Copy' | 'Paste' | 'SelectAll';

/** One entry of an edit menu: an OS command, Delete (the OS has no item of its own for it,
 *  so the page runs it) or a separator between groups. */
export type EditEntry =
  | { command: EditCommand; text: string; enabled: boolean }
  | { command: 'Delete'; text: string; enabled: boolean; run: () => void }
  | { command: 'Separator' };

/** The menu shown last; its native resources go when the next one opens. */
let shownMenu: Menu | null = null;

/** Show a native menu (at a point of the window, else at the pointer). */
async function show(menu: Menu, at: { x: number; y: number } | null = null): Promise<void> {
  const previous = shownMenu;
  shownMenu = menu;
  void previous?.close();
  await menu.popup(at === null ? undefined : new LogicalPosition(at.x, at.y));
}

/** One choice of a native menu with check marks (the chosen one is checked). */
export interface ChoiceEntry {
  text: string;
  checked: boolean;
  enabled?: boolean;
  onchoose: () => void;
}

/**
 * A native menu of choices below a menu button (`at`: its bottom left in window px), the
 * OS's own: the current choice checked, a click chooses.
 */
export async function popupChoiceMenu(
  entries: readonly ChoiceEntry[],
  at: { x: number; y: number } | null = null,
): Promise<void> {
  try {
    const items = await Promise.all(
      entries.map((entry) =>
        CheckMenuItem.new({
          text: entry.text,
          checked: entry.checked,
          enabled: entry.enabled ?? true,
          action: () => entry.onchoose(),
        }),
      ),
    );
    await show(await Menu.new({ items }), at);
  } catch (error) {
    reportUiError(`choice menu: ${String(error)}`, null, null);
  }
}

/**
 * A native context menu at the pointer, the OS's own (Windows and macOS draw it). An
 * enabled entry is the OS's predefined edit command, so the OS performs it on the focused
 * field or selection exactly like its own menus do; a disabled one is only shown.
 */
export async function popupEditMenu(entries: readonly EditEntry[]): Promise<void> {
  try {
    const items = await Promise.all(
      entries.map((entry) => {
        if (entry.command === 'Separator') return PredefinedMenuItem.new({ item: 'Separator' });
        if (entry.command === 'Delete') {
          return MenuItem.new({ text: entry.text, enabled: entry.enabled, action: entry.run });
        }
        return entry.enabled
          ? PredefinedMenuItem.new({ item: entry.command, text: entry.text })
          : MenuItem.new({ text: entry.text, enabled: false });
      }),
    );
    await show(await Menu.new({ items }));
  } catch (error) {
    reportUiError(`context menu: ${String(error)}`, null, null);
  }
}

const REPORT_LIMIT = 10;
const REPORT_WINDOW = 60_000;
const REPORT_LENGTH = 2000;
let reportTimes: number[] = [];

/** Send an unexpected UI error to the app log (truncated, at most 10 per minute). */
export function reportUiError(message: string, source: string | null, line: number | null): void {
  const now = Date.now();
  reportTimes = reportTimes.filter((t) => now - t < REPORT_WINDOW);
  if (reportTimes.length >= REPORT_LIMIT) return;
  reportTimes.push(now);
  invoke('report_ui_error', {
    message: message.slice(0, REPORT_LENGTH),
    source: source?.slice(0, 300) ?? null,
    line,
    // The log is the last resort: if even that fails, there is nowhere left to report to.
  }).catch(() => undefined);
}

/** Forward uncaught errors and rejections to the app log. Call once in main.ts. */
export function installErrorReporting(): void {
  addEventListener('error', (event) => {
    reportUiError(String(event.message), event.filename || null, event.lineno || null);
  });
  addEventListener('unhandledrejection', (event) => {
    reportUiError(String(event.reason), null, null);
  });
}
