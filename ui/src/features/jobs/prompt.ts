// The best matches as one prompt for any AI chat (the pinned jobs first, then by score; the
// backend picks them). Copied to the clipboard and confirmed by a toast; nothing is sent.

import { de } from '$lib/i18n/de';
import { errorText } from '$lib/i18n/texts';
import { jobs } from '$lib/state/jobs.svelte';
import { toasts } from '$lib/state/toasts.svelte';

/** How many jobs the prompt compares. */
const TOP = 5;

/** Copy the prompt; resolves with the error text, or null. */
export async function copyTopPrompt(): Promise<string | null> {
  try {
    await navigator.clipboard.writeText(await jobs.aiPromptTop(TOP));
    toasts.show(de.toast.prompt);
    return null;
  } catch (error) {
    return errorText(error);
  }
}
