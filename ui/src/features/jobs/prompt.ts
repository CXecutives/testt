// The best matches as one prompt for any AI chat (the pinned jobs first, then by score; the
// backend picks them). Copied to the clipboard and confirmed by a toast; nothing is sent.
// A clipboard that refuses says so in its own words (no log line would explain it).

import { t } from '$lib/i18n/t';
import { errorText } from '$lib/i18n/texts';
import { IpcError } from '$lib/ipc/api';
import { jobs } from '$lib/state/jobs.svelte';
import { toasts } from '$lib/state/toasts.svelte';

/** How many jobs the prompt compares. */
const TOP = 5;

/** Puts text on the clipboard; false when the clipboard refused it. */
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

/** Copy the prompt; resolves with the error text, or null. */
export async function copyTopPrompt(): Promise<string | null> {
  let prompt: string;
  try {
    prompt = await jobs.aiPromptTop(TOP);
  } catch (error) {
    // No scored job and no favourite yet: nothing to compare.
    return error instanceof IpcError && error.kind === 'notFound'
      ? t.overview.promptTopNone
      : errorText(error);
  }
  if (!(await copyText(prompt))) return t.reader.promptNotCopied;
  toasts.show(t.toast.prompt);
  return null;
}
