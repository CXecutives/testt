// The app state from the backend (`app_state`): settings, mailbox, profile, portals, last
// run. Loaded once at start and again after anything that changes it (a finished run,
// settings, profile, mailbox). `slow` turns on skeletons only when loading takes longer
// than --dur-fast, so a quick start never flashes placeholders. The settings change only
// through this store (`patchPortal`, `patchSettings`): the switch moves at once, the
// backend's answer then replaces the state, and an error loads the stored state again.

import { invoke } from '../ipc/api';
import type { AppState, Portal, PortalHealth, PortalPatch, SettingsPatch } from '../ipc/types';
import { tokenMs } from '../tokens';

/** The switches of one portal that a change names (the others stay). */
export type PortalChange = Partial<Omit<PortalPatch, 'portal'>>;

class AppStore {
  state = $state<AppState | null>(null);
  error = $state<unknown>(null);
  loading = $state(false);
  slow = $state(false);

  /** Loads the state; resolves with it (or null after an error). */
  async load(): Promise<AppState | null> {
    this.loading = true;
    this.error = null;
    const timer = setTimeout(() => (this.slow = true), tokenMs('--dur-fast'));
    try {
      const next = await invoke('app_state');
      this.state = next;
      return next;
    } catch (error) {
      this.error = error;
      return null;
    } finally {
      clearTimeout(timer);
      this.loading = false;
      this.slow = false;
    }
  }

  /**
   * Switches of a portal (active, details, sign-in). Rejects with the backend's error; the
   * stored state is loaded again then.
   */
  async patchPortal(portal: Portal, change: PortalChange): Promise<void> {
    const item = this.state?.portals.find((p) => p.portal === portal);
    if (item) Object.assign(item, withoutUnset(change));
    await this.save({
      portals: [
        {
          portal,
          enabled: change.enabled ?? null,
          fetchDetails: change.fetchDetails ?? null,
          loginEnabled: change.loginEnabled ?? null,
        },
      ],
      autoFetchOnStart: null,
    });
  }

  /** Settings beyond the portals (the auto fetch). Rejects like `patchPortal`. */
  async patchSettings(change: { autoFetchOnStart: boolean }): Promise<void> {
    if (this.state) this.state.autoFetchOnStart = change.autoFetchOnStart;
    await this.save({ portals: [], autoFetchOnStart: change.autoFetchOnStart });
  }

  private async save(patch: SettingsPatch): Promise<void> {
    try {
      this.state = await invoke('save_settings', { patch });
    } catch (error) {
      void this.load();
      throw error;
    }
  }

  /** Portal health from a run event, without a reload. */
  setHealth(portal: Portal, health: PortalHealth): void {
    const state = this.state;
    if (state === null) return;
    const item = state.portals.find((p) => p.portal === portal);
    if (item) item.health = health;
  }

  get hasMailbox(): boolean {
    return Boolean(this.state?.mailbox.user);
  }

  get hasProfile(): boolean {
    const profile = this.state?.profile;
    return Boolean(profile && profile.parseError === null && profile.quality !== 'empty');
  }
}

/** The fields of a change that it sets (`undefined` = unchanged). */
function withoutUnset(change: PortalChange): PortalChange {
  return Object.fromEntries(
    Object.entries(change).filter(([, value]) => value !== undefined && value !== null),
  );
}

export const app = new AppStore();
