// draft - replaced by ts-rs output
//
// Hand-written draft of the IPC v3 contract (docs/PLAN.md, "IPC v3"). The integrator
// replaces this file with the types generated from core/src/view.rs; until then the UI and
// the harness stub compile against it. camelCase, `null` instead of missing fields, and the
// backend never sends prose: notices, errors and status are `{code, params}`.

/** RFC 3339 timestamp (jiff::Timestamp). */
export type Timestamp = string;

export type Portal = 'linkedin' | 'freelance' | 'freelancermap';

export interface JobKey {
  portal: Portal;
  id: string;
}

export type ParamValue = string | number | boolean | null;
export type Params = Record<string, ParamValue>;

/** A message the UI renders from its catalog. */
export interface Coded {
  code: string;
  params: Params;
}

export type Band = 'high' | 'mid' | 'low';
export type MatchStatus = 'scored' | 'excluded' | 'unscorable';
export type DetailState = 'missing' | 'ok' | 'teaser' | 'failed' | 'unfetchable' | 'gone';
export type WorkMode = 'remote' | 'hybrid' | 'onSite';

export type ReasonKind = 'met' | 'partial' | 'open' | 'violation' | 'check';
export type ReasonWeight = 'must' | 'nice' | 'hard' | 'info';

export interface ReasonTop {
  kind: ReasonKind;
  label: string;
}

export interface JobMatch {
  score: number | null;
  band: Band | null;
  status: MatchStatus;
  note: Coded | null;
  mustMet: number;
  mustTotal: number;
  top: ReasonTop[];
}

export interface JobView {
  key: JobKey;
  portal: Portal;
  title: string;
  company: string;
  location: string;
  workMode: WorkMode | null;
  mailDate: Timestamp | null;
  firstSeenAt: Timestamp;
  unread: boolean;
  pinned: boolean;
  detail: DetailState;
  match: JobMatch | null;
  alsoOn: Portal[];
}

export interface Evidence {
  profile: string;
  path: string;
  via: string;
  quote: string | null;
}

export interface Reason {
  id: string;
  kind: ReasonKind;
  weight: ReasonWeight;
  code: string;
  label: string;
  evidence: Evidence | null;
  params: Params;
  /** UTF-16 ranges in the job text. */
  ranges: [number, number][];
}

export interface Highlight {
  id: string;
  /** UTF-16 offsets. */
  start: number;
  end: number;
  kind: ReasonKind;
  reason: string;
}

export type CriterionId = 'anue' | 'country' | 'dayRate' | 'availability' | 'permanent';
export type CriterionState = 'met' | 'violated' | 'check' | 'inactive' | 'unknown';

export interface Criterion {
  id: CriterionId;
  state: CriterionState;
  params: Params;
}

export interface JobDetailMatch {
  score: number | null;
  status: MatchStatus;
  band: Band | null;
  rev: string;
  at: Timestamp;
  summary: Coded;
  /** At most 40. */
  reasons: Reason[];
  /** At most 200. */
  highlights: Highlight[];
  criteria: Criterion[];
}

export interface JobDetail {
  job: JobView;
  text: string | null;
  url: string;
  fetchedAt: Timestamp | null;
  mail: { subject: string; gmailUrl: string | null };
  match: JobDetailMatch | null;
}

export type ProfileQuality = 'good' | 'thin' | 'empty';

export interface ProfileInfo {
  fileName: string;
  bytes: number;
  savedAt: Timestamp;
  quality: ProfileQuality;
  understood: {
    competenceCount: number;
    competences: string[];
    sources: string[];
    criteria: Coded[];
    warnings: Coded[];
  };
  scoredAt: Timestamp | null;
  pending: number;
}

export type PortalHealth =
  | { kind: 'ok' }
  | { kind: 'paused'; until: Timestamp; reason: Coded }
  | { kind: 'quotaReached'; until: Timestamp }
  | { kind: 'layoutSuspect'; emptyMails: number; pages: number }
  | { kind: 'loginRequired' };

export type PortalRisk = 'low' | 'grey' | 'account';
export type LoginMode = 'none' | 'required' | 'optional';

export interface PortalQuota {
  used: number;
  cap: number;
}

export interface PortalState {
  portal: Portal;
  enabled: boolean;
  fetchDetails: boolean;
  login: LoginMode;
  loginEnabled: boolean;
  signedIn: boolean | null;
  risk: PortalRisk;
  health: PortalHealth;
  quota: PortalQuota | null;
}

export interface Settings {
  workspace: string | null;
  autoFetchOnStart: boolean;
}

export interface SettingsPatch {
  workspace?: string;
  autoFetchOnStart?: boolean;
  portal?: {
    portal: Portal;
    enabled?: boolean;
    fetchDetails?: boolean;
    loginEnabled?: boolean;
  };
}

export interface MailboxInfo {
  address: string;
  connected: boolean;
  lastScanAt: Timestamp | null;
}

export interface JobCounts {
  new: number;
  all: number;
  excluded: number;
  high: number;
  noDetail: number;
}

export interface PortalSummary {
  portal: Portal;
  new: number;
  known: number;
  dup: number;
  fetched: number;
  failed: number;
}

export interface ScoreSummary {
  scored: number;
  excluded: number;
  unscorable: number;
  pending: number;
  best: number | null;
}

export interface RunSummary {
  perPortal: PortalSummary[];
  score: ScoreSummary;
  stops: Coded[];
  emptyAlerts: Coded[];
}

export interface LastRun {
  at: Timestamp;
  summary: RunSummary;
}

export interface AppState {
  platform: 'windows' | 'macos';
  dryRun: boolean;
  firstRun: boolean;
  running: boolean;
  settings: Settings;
  mailbox: MailboxInfo | null;
  profile: ProfileInfo | null;
  portals: PortalState[];
  autoFetchOnStart: boolean;
  lastRun: LastRun | null;
  counts: JobCounts;
  /** At most 5. */
  topMatches: JobView[];
  matchPending: number;
  dataDir: string;
  logDir: string;
  resetReport: Coded | null;
}

export type RunRequest =
  | { kind: 'fetch' }
  | { kind: 'details'; keys: JobKey[] }
  | { kind: 'rescore' }
  | { kind: 'fullMailbox' };

export type JobFacet = 'new' | 'all';
export type JobSort = 'match' | 'newest';

export interface JobQuery {
  facet: JobFacet;
  sort: JobSort;
  search: string | null;
  limit: number;
  offset: number;
}

export interface JobPage {
  jobs: JobView[];
  counts: JobCounts;
}

export type OpenTarget =
  | { kind: 'jobUrl'; key: JobKey }
  | { kind: 'gmail'; key: JobKey }
  | { kind: 'workspace' }
  | { kind: 'excel' }
  | { kind: 'overview' }
  | { kind: 'logDir' };

export interface ExportSummary {
  txtWritten: number;
  txtFailed: number;
}

export interface CommandError {
  kind: string;
  params: Params;
}

export type RunStep = 'scan' | 'fetch' | 'score' | 'export';

/** Events on the `run` channel. Each stays below 8 KB. */
export type RunEvent =
  | { type: 'progress'; step: RunStep; portal: Portal | null; done: number; total: number }
  | { type: 'status'; code: string; portal: Portal | null; until: Timestamp | null }
  | {
      type: 'alert';
      portal: Portal;
      subject: string;
      date: Timestamp | null;
      postings: number;
      gmailId: string | null;
    }
  | { type: 'jobUpdated'; job: JobView }
  | { type: 'portalHealth'; portal: Portal; health: PortalHealth }
  | { type: 'loginNeeded'; portal: Portal; waiting: boolean }
  | { type: 'finished'; summary: RunSummary };

type NoArgs = Record<string, never>;

/** Every command: its arguments and its result. The key is the Rust command name. */
export interface Commands {
  app_state: { args: NoArgs; result: AppState };
  start_run: { args: { request: RunRequest }; result: null };
  cancel_run: { args: NoArgs; result: null };
  list_jobs: { args: { query: JobQuery }; result: JobPage };
  job_detail: { args: { key: JobKey }; result: JobDetail };
  mark_read: { args: { key: JobKey }; result: boolean };
  set_pinned: { args: { key: JobKey; on: boolean }; result: boolean };
  pick_profile: { args: NoArgs; result: ProfileInfo | null };
  remove_profile: { args: NoArgs; result: boolean };
  save_profile_template: { args: NoArgs; result: string | null };
  save_mailbox: { args: { address: string; appPassword: string }; result: MailboxInfo };
  remove_mailbox: { args: NoArgs; result: boolean };
  portal_login: { args: { portal: Portal }; result: boolean };
  portal_logout: { args: { portal: Portal }; result: boolean };
  pick_workspace: { args: NoArgs; result: string | null };
  rewrite_txt: { args: NoArgs; result: ExportSummary };
  clear_txt: { args: NoArgs; result: number };
  open_target: { args: { target: OpenTarget }; result: null };
  save_settings: { args: { patch: SettingsPatch }; result: AppState };
  reset_all: { args: NoArgs; result: null };
  report_ui_error: {
    args: { message: string; source: string | null; line: number | null };
    result: null;
  };
}

export type CommandName = keyof Commands;
export type CommandArgs<K extends CommandName> = Commands[K]['args'];
export type CommandResult<K extends CommandName> = Commands[K]['result'];
