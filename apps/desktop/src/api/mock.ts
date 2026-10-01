/**
 * Browser mock of the Tauri backend for development and visual QA.
 *
 * It is only loaded (dynamic import) when the app does not run inside the Tauri
 * webview. The scenario is selected with the URL query `?scenario=<id>`.
 * Package operations run through a simulated lifecycle
 * (awaitingAuthorization → preparing → downloading → installing → succeeded)
 * with progress values and log lines, roughly one second per phase.
 */
import type { AppError } from '../bindings/AppError';
import type { AppInfo } from '../bindings/AppInfo';
import type { AutoUpdateConfig } from '../bindings/AutoUpdateConfig';
import type { AutoUpdateStatus } from '../bindings/AutoUpdateStatus';
import type { CatalogQuery } from '../bindings/CatalogQuery';
import type { CheckStatus } from '../bindings/CheckStatus';
import type { ConfigFileHint } from '../bindings/ConfigFileHint';
import type { Dashboard } from '../bindings/Dashboard';
import type { ErrorCode } from '../bindings/ErrorCode';
import type { HealthItem } from '../bindings/HealthItem';
import type { HealthReport } from '../bindings/HealthReport';
import type { HistoryEntry } from '../bindings/HistoryEntry';
import type { HyprlandInfo } from '../bindings/HyprlandInfo';
import type { InstalledQuery } from '../bindings/InstalledQuery';
import type { LockStatus } from '../bindings/LockStatus';
import type { McpSetup } from '../bindings/McpSetup';
import type { NewsFeedError } from '../bindings/NewsFeedError';
import type { NewsItem } from '../bindings/NewsItem';
import type { NewsStatus } from '../bindings/NewsStatus';
import type { Operation } from '../bindings/Operation';
import type { OperationKind } from '../bindings/OperationKind';
import type { OperationLogChunk } from '../bindings/OperationLogChunk';
import type { OperationState } from '../bindings/OperationState';
import type { PackagePage } from '../bindings/PackagePage';
import type { PackageRecord } from '../bindings/PackageRecord';
import type { PackageRef } from '../bindings/PackageRef';
import type { PackageSummary } from '../bindings/PackageSummary';
import type { PlanEntry } from '../bindings/PlanEntry';
import type { RebootReason } from '../bindings/RebootReason';
import type { Settings } from '../bindings/Settings';
import type { SnapshotSupport } from '../bindings/SnapshotSupport';
import type { SystemInfo } from '../bindings/SystemInfo';
import type { TransactionPlan } from '../bindings/TransactionPlan';
import type { UpdateBlocker } from '../bindings/UpdateBlocker';
import type { UpdateCheckResult } from '../bindings/UpdateCheckResult';
import type { InvokeArgs, Transport, Unlisten } from './client';
import {
  APP_ID,
  GIB,
  HELD_BACK_NAMES,
  MCP_TOOLS,
  MIB,
  PACKAGES,
  REPOSITORIES,
  UPDATE_DEFS,
  buildActivity,
  buildHyprland,
  buildNewsItems,
  buildRecord,
  buildSystemInfo,
  buildUpgradePlan,
  defaultAutoUpdate,
  defaultSettings,
  isCritical,
  mcpHostConfig,
  requiredBy,
  sealPlan,
  toCandidate,
  toSummary,
  type PackageDef,
  type UpdateDef,
} from './mockData';

export const SCENARIOS = [
  'default',
  'neverChecked',
  'stale',
  'offline',
  'locked',
  'unsupported',
  'helperMissing',
  'planChanged',
  'failed',
  'needsAttention',
  'running',
  'noHyprland',
  'newsUnread',
  'offlineConfHeld',
  'offlinePrepared',
  'prerequisiteMissing',
  'notCachyos',
] as const;

export type ScenarioId = (typeof SCENARIOS)[number];

const HOUR = 3600;
const DAY = 24 * HOUR;
const STALE_AFTER = 6 * HOUR;
const INSTALL_MAX_AGE = HOUR;
const ACTIVE_STATES: readonly OperationState[] = ['checking', 'awaitingAuthorization', 'preparing', 'downloading', 'installing'];
const CANCELLABLE: readonly OperationState[] = ['awaitingAuthorization', 'preparing', 'downloading'];
const TERMINAL: readonly OperationState[] = ['succeeded', 'failed', 'needsAttention', 'cancelledBeforeCommit'];
const ALLOWED_HOSTS = ['archlinux.org', 'wiki.archlinux.org', 'man.archlinux.org', 'cachyos.org', 'wiki.cachyos.org', 'github.com'];

interface MockOperation {
  op: Operation;
  lines: string[];
  plan: TransactionPlan;
  createSnapshot: boolean;
}

interface MockState {
  scenario: ScenarioId;
  appInfo: AppInfo;
  settings: Settings;
  system: SystemInfo;
  hyprland: HyprlandInfo;
  packages: PackageDef[];
  updateDefs: UpdateDef[];
  heldBack: string[];
  check: { status: CheckStatus; checkedAt: number | null; attemptedAt: number | null; error: AppError | null };
  checkBehavior: 'ok' | 'offline' | 'prerequisite' | 'unsupported';
  lock: LockStatus;
  news: { items: NewsItem[]; fetchedAt: number | null; errors: NewsFeedError[]; acknowledgedUntil: number | null };
  newsOffline: boolean;
  activity: HistoryEntry[];
  autoUpdate: AutoUpdateStatus;
  experimentalUnlocked: boolean;
  lastFullUpgrade: number | null;
  rebootReasons: RebootReason[];
  configFiles: ConfigFileHint[];
  snapshot: SnapshotSupport;
  operations: Map<string, MockOperation>;
  currentId: string | null;
  planChangedPending: boolean;
  failMode: 'none' | 'beforeCommit' | 'afterCommit';
  listeners: Map<string, Set<(payload: unknown) => void>>;
}

const nowSec = () => Math.floor(Date.now() / 1000);
const clone = <T>(value: T): T => structuredClone(value);

/** English technical detail, as produced by the backend (`RebootReason::describe`). */
function describeRebootReason(reason: RebootReason): string {
  return reason.kind === 'kernelReplaced'
    ? 'running kernel was replaced by an update'
    : `updated since boot: ${reason.packages.join(', ')}`;
}
const wait = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

function appError(code: ErrorCode, message: string, detail: string | null = null): AppError {
  return { code, message, detail };
}

function uuid(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') return crypto.randomUUID();
  const hex = () => Math.floor(Math.random() * 16).toString(16);
  return 'xxxxxxxx-xxxx-4xxx-8xxx-xxxxxxxxxxxx'.replace(/x/g, hex);
}

export function parseScenario(search: string): ScenarioId {
  const value = new URLSearchParams(search).get('scenario');
  return (SCENARIOS as readonly string[]).includes(value ?? '') ? (value as ScenarioId) : 'default';
}

// ---------------------------------------------------------------------------
// Scenario setup
// ---------------------------------------------------------------------------

export function createState(scenario: ScenarioId): MockState {
  const now = nowSec();
  const lastFullUpgrade = now - 2 * DAY - 3 * HOUR;
  const state: MockState = {
    scenario,
    appInfo: {
      version: '0.1.0',
      appId: APP_ID,
      systemLanguage: 'de',
      systemColorScheme: 'dark',
      helperAvailable: true,
      helperError: null,
      backend: { state: 'ready', libalpmVersion: '16.0.1', builtAgainst: '16.0.1' },
      developmentMode: true,
    },
    settings: defaultSettings(),
    system: buildSystemInfo(now, lastFullUpgrade),
    hyprland: buildHyprland(now),
    packages: clone(PACKAGES),
    updateDefs: clone(UPDATE_DEFS),
    heldBack: [],
    check: { status: 'fresh', checkedAt: now - 25 * 60, attemptedAt: now - 25 * 60, error: null },
    checkBehavior: 'ok',
    lock: { state: 'free' },
    news: { items: buildNewsItems(now, 0), fetchedAt: now - 20 * 60, errors: [], acknowledgedUntil: now - 2 * DAY },
    newsOffline: false,
    activity: buildActivity(now),
    autoUpdate: defaultAutoUpdate(),
    experimentalUnlocked: false,
    lastFullUpgrade,
    rebootReasons: [],
    configFiles: [
      { path: '/etc/pacman.conf.pacnew', kind: 'pacnew', modifiedAt: now - 9 * DAY },
      { path: '/etc/mkinitcpio.conf.pacnew', kind: 'pacnew', modifiedAt: now - 2 * DAY },
      { path: '/etc/default/grub.pacsave', kind: 'pacsave', modifiedAt: now - 40 * DAY },
    ],
    snapshot: { btrfsRoot: true, snapperInstalled: true, rootConfig: 'root', snapPacActive: false, canRequestSnapshot: true },
    operations: new Map(),
    currentId: null,
    planChangedPending: false,
    failMode: 'none',
    listeners: new Map(),
  };

  switch (scenario) {
    case 'neverChecked':
      state.check = { status: 'neverChecked', checkedAt: null, attemptedAt: null, error: null };
      break;
    case 'stale':
      state.check = { status: 'fresh', checkedAt: now - 7 * HOUR, attemptedAt: now - 7 * HOUR, error: null };
      break;
    case 'offline':
      state.checkBehavior = 'offline';
      state.newsOffline = true;
      state.check = {
        status: 'failed',
        checkedAt: now - 2 * DAY,
        attemptedAt: now - 10 * 60,
        error: appError('OFFLINE', 'failed to synchronize all databases (could not resolve host)', 'error: failed retrieving file core.db from mirror.cachyos.org : Could not resolve host'),
      };
      state.news = { items: buildNewsItems(now - 2 * DAY, 0), fetchedAt: now - 2 * DAY, errors: [], acknowledgedUntil: now - 2 * DAY };
      break;
    case 'locked':
      state.lock = { state: 'locked', since: now - 95, holderRunning: true };
      break;
    case 'unsupported':
      state.appInfo.backend = { state: 'unavailable', reason: 'libalpm 17.0.0 is not supported by this build (built against 16.0.1)' };
      state.system.pacman.backend = state.appInfo.backend;
      state.checkBehavior = 'unsupported';
      state.check = { status: 'unsupported', checkedAt: null, attemptedAt: null, error: null };
      break;
    case 'helperMissing':
      state.appInfo.helperAvailable = false;
      state.appInfo.helperError = 'org.cachyos_center.Packages1 was not provided by any .service files';
      state.autoUpdate.helperAvailable = false;
      break;
    case 'planChanged':
      state.planChangedPending = true;
      break;
    case 'failed':
      state.failMode = 'beforeCommit';
      break;
    case 'needsAttention':
      state.failMode = 'afterCommit';
      break;
    case 'noHyprland':
      state.hyprland = { available: false, reason: 'noSession', version: null, monitors: [], activeWorkspace: null, workspaceCount: null, appClass: 'cachyos-center', collectedAt: now };
      state.system.session = { kind: 'otherWayland', desktop: 'KDE', sessionType: 'wayland' };
      state.updateDefs = state.updateDefs.map((d) => (d.name === 'hyprland' ? { ...d, flags: [] } : d));
      break;
    case 'newsUnread':
      state.news = { items: buildNewsItems(now, 2), fetchedAt: now - 5 * 60, errors: [], acknowledgedUntil: now - 10 * DAY };
      break;
    case 'offlineConfHeld':
      state.heldBack = [...HELD_BACK_NAMES];
      state.autoUpdate.offline = {
        installed: true,
        prepared: false,
        prepareTimerActive: false,
        rebootTimerActive: false,
        offlineConfIncluded: true,
        offlineConfIgnored: [...HELD_BACK_NAMES],
        configurationVerifiable: true,
      };
      break;
    case 'offlinePrepared':
      state.autoUpdate.offline = {
        installed: true,
        prepared: true,
        prepareTimerActive: true,
        rebootTimerActive: false,
        offlineConfIncluded: false,
        offlineConfIgnored: [],
        configurationVerifiable: true,
      };
      state.autoUpdate.preparedForNextReboot = true;
      state.autoUpdate.externalUpdaters = [
        ...state.autoUpdate.externalUpdaters,
        { name: 'pacman-offline-prepare.timer', scope: 'system', active: true, description: 'pacman-offline: scheduled preparation of offline updates' },
      ];
      break;
    case 'prerequisiteMissing':
      state.checkBehavior = 'prerequisite';
      state.check = { status: 'prerequisiteMissing', checkedAt: null, attemptedAt: now - 60, error: appError('PREREQUISITE_MISSING', 'checkupdates is not installed') };
      state.packages = state.packages.filter((p) => p.name !== 'pacman-contrib');
      break;
    case 'notCachyos':
      state.system.os = { id: 'arch', name: 'Arch Linux', prettyName: 'Arch Linux', buildId: 'rolling', isCachyos: false, isArchBased: true };
      state.system.kernel = { release: '6.17.1-arch1-1', package: 'linux', uptimeSeconds: 5 * HOUR, modulesMissing: false };
      state.system.session = { kind: 'otherWayland', desktop: 'GNOME', sessionType: 'wayland' };
      state.system.pacman.repositories = ['core', 'extra', 'multilib'];
      state.hyprland = { available: false, reason: 'noSession', version: null, monitors: [], activeWorkspace: null, workspaceCount: null, appClass: 'cachyos-center', collectedAt: now };
      state.appInfo.systemColorScheme = 'light';
      break;
    case 'running':
    case 'default':
      break;
  }

  state.autoUpdate.prepareModeBlockers = prepareBlockers(state);
  if (scenario === 'running') resumeRunningUpgrade(state);
  return state;
}

function prepareBlockers(state: MockState): string[] {
  const out: string[] = [];
  const offline = state.autoUpdate.offline;
  if (!state.appInfo.helperAvailable) out.push('helperMissing');
  if (!state.experimentalUnlocked) out.push('experimentalLocked');
  if (!offline.installed) out.push('pacmanOfflineMissing');
  if (!offline.configurationVerifiable) out.push('offlineConfigUnverifiable');
  if (offline.prepareTimerActive) out.push('externalPrepareTimer');
  if (offline.offlineConfIncluded) out.push('offlineConfHoldsPackages');
  return out;
}

// ---------------------------------------------------------------------------
// Derived data
// ---------------------------------------------------------------------------

const installedDefs = (state: MockState) => state.packages.filter((p) => p.installed);
const onlineUpdates = (state: MockState) => state.updateDefs.filter((d) => !state.heldBack.includes(d.name));
const backendReady = (state: MockState) => state.appInfo.backend.state === 'ready';

function checkStatus(state: MockState): CheckStatus {
  const { status, checkedAt } = state.check;
  if (status === 'fresh' && checkedAt !== null && nowSec() - checkedAt > STALE_AFTER) return 'stale';
  return status;
}

function updatesResult(state: MockState): UpdateCheckResult {
  const status = checkStatus(state);
  const base: UpdateCheckResult = {
    status,
    checkedAt: state.check.checkedAt,
    attemptedAt: state.check.attemptedAt,
    updates: [],
    plan: null,
    heldBack: [],
    totalDownloadSize: null,
    rebootRecommended: false,
    error: state.check.error,
    missingPrerequisites: status === 'prerequisiteMissing' ? ['pacman-contrib'] : [],
  };
  if (state.check.checkedAt === null || status === 'unsupported' || status === 'prerequisiteMissing') return base;
  const online = onlineUpdates(state);
  const held = state.updateDefs.filter((d) => state.heldBack.includes(d.name));
  const plan = online.length > 0 ? buildUpgradePlan(online, state.check.checkedAt, held.map((d) => d.name)) : null;
  return {
    ...base,
    updates: online.map((d) => toCandidate(d)),
    heldBack: held.map((d) => toCandidate(d, true)),
    plan,
    totalDownloadSize: plan?.downloadSize ?? 0,
    rebootRecommended: online.some((d) => d.flags.includes('rebootRecommended')),
  };
}

function newsStatus(state: MockState): NewsStatus {
  if (!state.settings.newsEnabled) {
    return { disabled: true, items: [], fetchedAt: null, errors: [], acknowledgedUntil: state.news.acknowledgedUntil, unreadCount: 0 };
  }
  const ack = state.news.acknowledgedUntil ?? 0;
  const items = state.news.items.map((item) => ({ ...item, unread: item.unread && item.publishedAt > ack }));
  return {
    disabled: false,
    items,
    fetchedAt: state.news.fetchedAt,
    errors: state.news.errors,
    acknowledgedUntil: state.news.acknowledgedUntil,
    unreadCount: items.filter((i) => i.unread).length,
  };
}

function systemInfo(state: MockState): SystemInfo {
  const installed = installedDefs(state);
  const system = clone(state.system);
  system.pacman.lock = clone(state.lock);
  system.pacman.lastFullUpgrade = state.lastFullUpgrade;
  system.pacman.installedCount = backendReady(state) ? installed.length : 0;
  system.pacman.foreignCount = backendReady(state) ? installed.filter((p) => p.repository === null).length : 0;
  system.collectedAt = nowSec();
  return system;
}

function lastOperation(state: MockState): MockOperation | null {
  return state.currentId ? (state.operations.get(state.currentId) ?? null) : null;
}

function healthReport(state: MockState): HealthReport {
  const now = nowSec();
  const items: HealthItem[] = [];
  const updates = updatesResult(state);
  const news = newsStatus(state);
  const blockers: UpdateBlocker[] = [];
  const pacnew = state.configFiles.filter((f) => f.kind === 'pacnew').length;
  const pacsave = state.configFiles.filter((f) => f.kind === 'pacsave').length;
  const root = state.system.disks.find((d) => d.mountPoint === '/');
  if (pacnew > 0) items.push({ kind: 'pacnewFiles', severity: 'warning', detail: 'configuration files need a manual merge', count: pacnew });
  if (pacsave > 0) items.push({ kind: 'pacsaveFiles', severity: 'info', detail: 'saved configuration files of removed packages', count: pacsave });
  if (state.rebootReasons.length > 0) {
    items.push({ kind: 'rebootRecommended', severity: state.system.kernel.modulesMissing ? 'warning' : 'info', detail: state.rebootReasons.map(describeRebootReason).join('; '), count: null });
  }
  if (state.lock.state === 'locked') {
    items.push({ kind: 'packageManagerLocked', severity: 'warning', detail: 'another package manager is running', count: null });
    blockers.push('packageManagerBusy');
  }
  const last = lastOperation(state)?.op;
  if (last && (last.state === 'failed' || last.state === 'needsAttention')) {
    items.push({ kind: 'lastOperationFailed', severity: last.state === 'needsAttention' ? 'critical' : 'warning', detail: last.summary, count: null });
    if (last.state === 'needsAttention') blockers.push('lastOperationNeedsAttention');
  }
  items.push({ kind: 'packageCacheLarge', severity: 'info', detail: 'package cache is large (paccache can clean it)', count: null });
  if (state.appInfo.backend.state === 'unavailable') {
    items.push({ kind: 'packageBackendUnavailable', severity: 'critical', detail: state.appInfo.backend.reason, count: null });
    blockers.push('packageBackendUnavailable');
  }
  if (updates.status === 'prerequisiteMissing') {
    items.push({ kind: 'prerequisiteMissing', severity: 'warning', detail: 'missing: pacman-contrib', count: null });
    blockers.push('prerequisitesMissing');
  } else if (updates.status === 'failed') {
    items.push({ kind: 'updateCheckFailed', severity: 'warning', detail: updates.error?.message ?? '', count: null });
  } else if (updates.status === 'stale') {
    items.push({ kind: 'updateCheckStale', severity: 'info', detail: 'update information is outdated', count: null });
  }
  const offline = state.autoUpdate.offline;
  if (offline.prepared) items.push({ kind: 'offlineUpdatePrepared', severity: 'info', detail: 'updates will be installed on the next reboot', count: null });
  if (offline.offlineConfIncluded && offline.offlineConfIgnored.length > 0) {
    items.push({ kind: 'offlineConfHoldsPackages', severity: 'warning', detail: `held back for online updates: ${offline.offlineConfIgnored.join(', ')}`, count: offline.offlineConfIgnored.length });
  }
  const external = state.autoUpdate.externalUpdaters.filter((u) => u.active);
  if (external.length > 0) {
    items.push({ kind: 'externalUpdaterActive', severity: 'info', detail: external.map((u) => u.name).join(', '), count: external.length });
    if (offline.prepareTimerActive) blockers.push('externalPrepareTimer');
  }
  if (news.disabled) {
    blockers.push('newsDisabled');
  } else {
    if (news.unreadCount > 0) {
      items.push({ kind: 'newsUnread', severity: 'warning', detail: 'unread Arch Linux/CachyOS news', count: news.unreadCount });
      blockers.push('newsUnread');
    }
    if (news.errors.length > 0 || news.fetchedAt === null) {
      items.push({ kind: 'newsUnavailable', severity: 'info', detail: 'news check not possible', count: null });
      blockers.push('newsUnavailable');
    }
  }
  if (root && root.availableBytes < 5 * GIB) {
    items.push({ kind: 'lowDiskSpace', severity: root.availableBytes < GIB ? 'critical' : 'warning', detail: 'less than 5 GiB free on /', count: null });
  }
  const order = { critical: 0, warning: 1, info: 2 } as const;
  items.sort((a, b) => order[a.severity] - order[b.severity]);
  return {
    items,
    pacnewCount: pacnew,
    pacsaveCount: pacsave,
    configFiles: clone(state.configFiles),
    lock: clone(state.lock),
    rebootRecommended: state.rebootReasons.length > 0,
    rebootReasons: clone(state.rebootReasons),
    packageCacheBytes: Math.round(7.8 * GIB),
    snapshot: clone(state.snapshot),
    offlineUpdate: clone(offline),
    externalUpdaters: clone(state.autoUpdate.externalUpdaters),
    updateBlockers: blockers,
    collectedAt: now,
  };
}

function dashboard(state: MockState): Dashboard {
  const system = systemInfo(state);
  const health = healthReport(state);
  const root = system.disks.find((d) => d.mountPoint === '/');
  const order = { critical: 3, warning: 2, info: 1 } as const;
  const worst = health.items.reduce<HealthItem['severity'] | null>(
    (acc, item) => (acc === null || order[item.severity] > order[acc] ? item.severity : acc),
    null,
  );
  return {
    system: {
      os: system.os.prettyName,
      isCachyos: system.os.isCachyos,
      kernel: system.kernel.release,
      uptimeSeconds: system.kernel.uptimeSeconds,
      cpu: system.cpu.model,
      gpus: system.gpus.map((g) => `${g.vendor} ${g.model}`),
      memoryTotalBytes: system.memory.totalBytes,
      memoryAvailableBytes: system.memory.availableBytes,
      rootTotalBytes: root?.totalBytes ?? 0,
      rootAvailableBytes: root?.availableBytes ?? 0,
      rootFilesystem: root?.filesystem ?? 'unknown',
      session: system.session.kind,
      desktop: system.session.desktop,
      pacmanVersion: system.pacman.pacmanVersion,
      packageBackendReady: backendReady(state),
    },
    updates: updatesResult(state),
    lock: clone(state.lock),
    installedCount: system.pacman.installedCount,
    foreignCount: system.pacman.foreignCount,
    healthItems: health.items,
    healthWorst: worst,
    lastActivity: state.activity[0] ?? null,
    lastFullUpgrade: state.lastFullUpgrade,
    nextCheckAt: state.settings.checkIntervalHours > 0 ? nowSec() + Math.min(state.settings.checkIntervalHours * HOUR, 2 * HOUR + 1260) : null,
    offlineUpdatePrepared: state.autoUpdate.offline.prepared,
    rebootRecommended: state.rebootReasons.length > 0,
    collectedAt: nowSec(),
  };
}

// ---------------------------------------------------------------------------
// Packages
// ---------------------------------------------------------------------------

function requireBackend(state: MockState): void {
  if (!backendReady(state)) throw appError('UNSUPPORTED', 'package functions are disabled (libalpm bridge unavailable)');
}

function summaries(state: MockState): PackageSummary[] {
  return state.packages.map((p) => toSummary(p, onlineUpdates(state), state.heldBack));
}

function listInstalled(state: MockState, query: InstalledQuery): PackagePage {
  requireBackend(state);
  const needle = (query.query ?? '').trim().toLowerCase();
  const limit = query.limit === 0 ? 100 : query.limit;
  if (limit < 1 || limit > 500) throw appError('INVALID_INPUT', 'limit must be between 1 and 500');
  const items = summaries(state)
    .filter((p) => p.installedVersion !== null)
    .filter((p) => !needle || p.name.includes(needle) || p.description.toLowerCase().includes(needle))
    .filter((p) => {
      switch (query.filter) {
        case 'explicit':
          return p.installReason === 'explicit';
        case 'dependency':
          return p.installReason === 'dependency';
        case 'repo':
          return p.origin === 'repo';
        case 'localOrAur':
          return p.origin === 'localOrAur';
        case 'updateAvailable':
          return p.updateAvailable;
        default:
          return true;
      }
    })
    .sort((a, b) => a.name.localeCompare(b.name));
  const page = items.slice(query.offset, query.offset + limit);
  const next = query.offset + limit;
  return { items: page, total: items.length, offset: query.offset, nextOffset: next < items.length ? next : null };
}

function searchPackages(state: MockState, query: CatalogQuery): PackageSummary[] {
  requireBackend(state);
  const needle = query.query.trim().toLowerCase();
  if (query.repository === null && needle.length < 2) throw appError('INVALID_INPUT', 'query must have at least 2 characters');
  if (query.limit < 1 || query.limit > 500) throw appError('INVALID_INPUT', 'limit must be between 1 and 500');
  return summaries(state)
    .filter((p) => p.repository !== null)
    .filter((p) => query.repository === null || p.repository === query.repository)
    .filter((p) => !needle || p.name.includes(needle) || p.description.toLowerCase().includes(needle))
    .filter((p) =>
      query.installFilter === 'installed' ? p.installedVersion !== null : query.installFilter === 'notInstalled' ? p.installedVersion === null : true,
    )
    .sort((a, b) => a.name.localeCompare(b.name))
    .slice(0, query.limit);
}

function packageDetails(state: MockState, reference: PackageRef): PackageRecord {
  requireBackend(state);
  const def = state.packages.find(
    (p) => p.name === reference.name && (reference.repository === null || p.repository === reference.repository),
  );
  if (!def) throw appError('NOT_FOUND', `package ${reference.name} not found`);
  const record = buildRecord(def, installedDefs(state), onlineUpdates(state), nowSec());
  record.ignored = state.heldBack.includes(def.name);
  return record;
}

function dataAgeCheck(state: MockState): void {
  const checkedAt = state.check.checkedAt;
  if (checkedAt === null || nowSec() - checkedAt > INSTALL_MAX_AGE) {
    throw appError('STALE', 'repository data is older than 1 hour', checkedAt === null ? 'no successful update check' : null);
  }
}

function planInstall(state: MockState, repository: string, name: string): TransactionPlan {
  requireBackend(state);
  const def = state.packages.find((p) => p.name === name && p.repository === repository);
  if (!def) throw appError('NOT_FOUND', `package ${repository}/${name} not found`);
  dataAgeCheck(state);
  const entries: PlanEntry[] = [];
  const seen = new Set<string>();
  const addInstall = (target: PackageDef, requested: boolean) => {
    if (seen.has(target.name) || target.installed) return;
    seen.add(target.name);
    entries.push({ action: 'install', name: target.name, repository: target.repository, oldVersion: null, newVersion: target.version, downloadSize: target.download ?? null, requested, flags: [] });
    for (const dep of target.deps ?? []) {
      const depDef = state.packages.find((p) => p.name === dep);
      if (depDef) addInstall(depDef, false);
    }
  };
  if (def.installed) {
    entries.push({ action: 'reinstall', name: def.name, repository: def.repository, oldVersion: def.version, newVersion: def.version, downloadSize: def.download ?? null, requested: true, flags: [] });
  } else {
    addInstall(def, true);
  }
  const upgrades = onlineUpdates(state).map((d) => ({ ...buildUpgradePlan([d], 0).entries[0]! }));
  const reboot = onlineUpdates(state).filter((d) => d.flags.includes('rebootRecommended')).map((d) => d.name);
  const warnings: TransactionPlan['warnings'] = [];
  if (upgrades.length > 0) warnings.push({ kind: 'includesSystemUpgrade', upgradeCount: upgrades.length });
  if (state.heldBack.length > 0) warnings.push({ kind: 'heldBackPackages', packages: [...state.heldBack] });
  if (reboot.length > 0) warnings.push({ kind: 'rebootRecommended', packages: reboot });
  const all = [...entries, ...upgrades];
  return sealPlan({
    kind: 'install',
    targets: [name],
    entries: all,
    downloadSize: all.reduce((sum, e) => sum + (e.downloadSize ?? 0), 0),
    installSizeDelta: entries.reduce((sum, e) => sum + (state.packages.find((p) => p.name === e.name)?.size ?? 0), 0) + (upgrades.length > 0 ? Math.round(14.6 * MIB) : 0),
    source: 'systemDb',
    computedAt: nowSec(),
    warnings,
    recursive: false,
  });
}

function planRemove(state: MockState, name: string, recursive: boolean): TransactionPlan {
  requireBackend(state);
  const installed = installedDefs(state);
  const def = installed.find((p) => p.name === name);
  if (!def) throw appError('NOT_FOUND', `package ${name} is not installed`);
  const dependents = requiredBy(name, installed);
  if (dependents.length > 0) {
    throw appError(
      'DEPENDENCY_PROBLEM',
      'pacman cannot plan the transaction: could not satisfy dependencies',
      dependents.map((d) => `${d}: requires ${name} (required by ${name})`).join('\n'),
    );
  }
  const removed = [def];
  if (recursive) {
    const remaining = installed.filter((p) => p.name !== name);
    for (const dep of def.deps ?? []) {
      const depDef = installed.find((p) => p.name === dep);
      if (!depDef || depDef.reason !== 'dependency') continue;
      const stillNeeded = remaining.some((p) => p.name !== dep && p.deps?.includes(dep));
      if (!stillNeeded) removed.push(depDef);
    }
  }
  const critical = removed.filter((p) => isCritical(p.name)).map((p) => p.name);
  return sealPlan({
    kind: 'remove',
    targets: [name],
    entries: removed.map((p) => ({ action: 'remove', name: p.name, repository: null, oldVersion: p.version, newVersion: null, downloadSize: null, requested: p.name === name, flags: [] })),
    downloadSize: 0,
    installSizeDelta: -removed.reduce((sum, p) => sum + p.size, 0),
    source: 'systemDb',
    computedAt: nowSec(),
    warnings: critical.length > 0 ? [{ kind: 'criticalPackages', packages: critical }] : [],
    recursive,
  });
}

// ---------------------------------------------------------------------------
// Operations
// ---------------------------------------------------------------------------

function emit(state: MockState, event: string, payload: unknown): void {
  state.listeners.get(event)?.forEach((handler) => handler(clone(payload)));
}

function log(mop: MockOperation, ...lines: string[]): void {
  mop.lines.push(...lines);
}

const isDone = (mop: MockOperation) => TERMINAL.includes(mop.op.state);

function assertStartable(state: MockState, digest: string): void {
  if (!state.appInfo.helperAvailable) throw appError('UNAVAILABLE', 'helper is not available', state.appInfo.helperError);
  if (!/^[0-9a-f]{64}$/.test(digest)) throw appError('INVALID_INPUT', 'plan digest must be a lower-case hex SHA-256');
  const current = lastOperation(state);
  if (current && ACTIVE_STATES.includes(current.op.state)) throw appError('BUSY', 'another operation is running');
  if (state.lock.state === 'locked') throw appError('BUSY', 'unable to lock database', '/var/lib/pacman/db.lck exists');
  if (state.autoUpdate.offline.prepared) throw appError('CONFLICT', 'an offline update is prepared for the next reboot');
}

function newOperation(state: MockState, kind: OperationKind, targets: string[], plan: TransactionPlan, confirmedDigest: string, createSnapshot: boolean): MockOperation {
  const now = nowSec();
  const mop: MockOperation = {
    op: {
      id: uuid(),
      kind,
      origin: 'user',
      requestedAt: now,
      state: 'awaitingAuthorization',
      packageTargets: targets,
      startedAt: now,
      endedAt: null,
      exitCode: null,
      summary: '',
      error: null,
      commitStarted: false,
      confirmedDigest,
      actualPlan: null,
      progress: { currentPackage: null, packagesDone: 0, packagesTotal: null, phaseDetail: 'waiting for polkit authorization' },
      snapshot: null,
      newPacnewFiles: 0,
      changes: { installed: 0, upgraded: 0, downgraded: 0, reinstalled: 0, removed: 0, packages: [] },
      outcomeUnknown: false,
    },
    lines: [],
    plan,
    createSnapshot,
  };
  state.operations.set(mop.op.id, mop);
  state.currentId = mop.op.id;
  return mop;
}

function finish(state: MockState, mop: MockOperation, next: OperationState, summary: string, error: AppError | null = null): void {
  mop.op.state = next;
  mop.op.endedAt = nowSec();
  mop.op.summary = summary;
  mop.op.error = error;
  mop.op.progress.currentPackage = null;
  mop.op.progress.phaseDetail = null;
  mop.op.exitCode = next === 'succeeded' ? 0 : next === 'cancelledBeforeCommit' ? null : 1;
  state.activity.unshift({
    id: mop.op.id,
    source: 'app',
    kind: mop.op.kind,
    origin: 'user',
    state: next,
    logOutcome: null,
    startedAt: mop.op.startedAt ?? mop.op.requestedAt,
    endedAt: mop.op.endedAt,
    summary,
    errorCode: error?.code ?? null,
    installed: mop.op.changes.installed,
    upgraded: mop.op.changes.upgraded,
    removed: mop.op.changes.removed,
    downgraded: mop.op.changes.downgraded,
    packages: mop.op.changes.packages,
    outcomeUnknown: mop.op.outcomeUnknown,
  });
  emit(state, 'operation-finished', mop.op);
}

function applySuccess(state: MockState, mop: MockOperation): void {
  const now = nowSec();
  const entries = mop.plan.entries;
  const changes = mop.op.changes;
  for (const entry of entries) {
    const def = state.packages.find((p) => p.name === entry.name);
    if (entry.action === 'remove') {
      changes.removed += 1;
      if (def) def.installed = false;
    } else if (entry.action === 'install') {
      changes.installed += 1;
      if (def) {
        def.installed = true;
        def.reason = entry.requested ? 'explicit' : 'dependency';
      }
    } else if (entry.action === 'reinstall') {
      changes.reinstalled += 1;
    } else if (entry.action === 'downgrade') {
      changes.downgraded += 1;
      if (def) def.version = entry.newVersion ?? def.version;
    } else {
      changes.upgraded += 1;
      if (def) def.version = entry.newVersion ?? def.version;
    }
  }
  changes.packages = entries.map((e) => e.name).slice(0, 20);
  const upgraded = entries.filter((e) => e.action === 'upgrade').map((e) => e.name);
  if (upgraded.length > 0) {
    state.updateDefs = state.updateDefs.filter((d) => !upgraded.includes(d.name));
    state.check = { status: 'fresh', checkedAt: now, attemptedAt: now, error: null };
    state.lastFullUpgrade = now;
    const reboot = upgraded.filter((n) => UPDATE_DEFS.find((d) => d.name === n)?.flags.includes('rebootRecommended'));
    if (reboot.length > 0) state.rebootReasons = [{ kind: 'updatedSinceBoot', packages: reboot }];
    if (upgraded.includes('linux-cachyos')) {
      state.system.kernel.modulesMissing = true;
      state.rebootReasons.unshift({ kind: 'kernelReplaced' });
    }
  }
  if (upgraded.includes('systemd')) {
    mop.op.newPacnewFiles = 1;
    state.configFiles.push({ path: '/etc/systemd/logind.conf.pacnew', kind: 'pacnew', modifiedAt: now });
  }
}

async function runLifecycle(state: MockState, mop: MockOperation, resumeAtInstall = 0): Promise<void> {
  const op = mop.op;
  const entries = mop.plan.entries;
  const isRemove = op.kind === 'remove';
  if (resumeAtInstall === 0) {
    log(mop, `==> ${isRemove ? 'pacman -R' : 'pacman -Syu'} requested (plan ${op.confirmedDigest?.slice(0, 12) ?? '?'})`, 'waiting for polkit authorization');
    await wait(1100);
    if (isDone(mop)) return;
    op.state = 'preparing';
    op.progress.phaseDetail = 'checking plan and database lock';
    log(mop, 'authorization granted', ':: Synchronizing package databases...', ' cachyos-v3 downloading...', ' cachyos-core-v3 downloading...', ' cachyos-extra-v3 downloading...', ' core downloading...', ' extra downloading...', ' multilib downloading...');
    await wait(500);
    if (isDone(mop)) return;
    if (mop.createSnapshot) {
      log(mop, 'snapper: creating pre-transaction snapshot (config root)');
      op.snapshot = { created: true, snapperConfig: 'root', number: 412, error: null };
      log(mop, 'snapper: snapshot 412 created');
    }
    log(mop, 'resolving dependencies...', 'looking for conflicting packages...');
    await wait(600);
    if (isDone(mop)) return;

    const current = currentPlanFor(state, mop);
    if (state.planChangedPending && op.kind === 'systemUpgrade') {
      state.planChangedPending = false;
      state.updateDefs = changedUpdateDefs(state.updateDefs);
    }
    const actual = currentPlanFor(state, mop) ?? current;
    if (actual && actual.digest !== op.confirmedDigest) {
      op.actualPlan = actual;
      log(mop, `plan digest mismatch: confirmed ${op.confirmedDigest?.slice(0, 12)}, actual ${actual.digest.slice(0, 12)}`, 'stopping before commit, nothing was changed');
      finish(state, mop, 'cancelledBeforeCommit', 'the package plan changed before commit', appError('PLAN_CHANGED', 'the actual plan differs from the confirmed plan'));
      return;
    }
    log(mop, '', `Packages (${entries.length}) ${entries.map((e) => `${e.name}-${e.newVersion ?? e.oldVersion ?? ''}`).join('  ')}`, '');
    op.progress.packagesTotal = entries.length;

    if (!isRemove) {
      op.state = 'downloading';
      op.progress.phaseDetail = 'retrieving packages';
      log(mop, ':: Retrieving packages...');
      for (const entry of entries) {
        if (entry.downloadSize === null || entry.downloadSize === 0) continue;
        op.progress.currentPackage = entry.name;
        log(mop, ` ${entry.name}-${entry.newVersion ?? ''}-x86_64 downloading...`);
        await wait(90);
        if (isDone(mop)) return;
      }
      op.progress.currentPackage = null;
      op.progress.phaseDetail = 'checking keyring';
      log(mop, 'checking keyring...', 'checking package integrity...');
      await wait(300);
      if (isDone(mop)) return;
      if (state.failMode === 'beforeCommit') {
        state.failMode = 'none';
        log(mop, 'error: linux-firmware-amdgpu: signature from "CachyOS Build System" is unknown trust', ':: File /var/cache/pacman/pkg/linux-firmware-amdgpu-20260925-1-any.pkg.tar.zst is corrupted (invalid or corrupted package (PGP signature)).', 'error: failed to commit transaction (invalid or corrupted package (PGP signature))', 'Errors occurred, no packages were upgraded.');
        finish(state, mop, 'failed', 'invalid or corrupted package (PGP signature)', appError('TRANSACTION_FAILED', 'failed to commit transaction (invalid or corrupted package (PGP signature))', 'error: linux-firmware-amdgpu: signature from "CachyOS Build System" is unknown trust'));
        return;
      }
      log(mop, 'loading package files...', 'checking for file conflicts...', 'checking available disk space...');
    }
    op.state = 'installing';
    op.commitStarted = true;
    op.progress.phaseDetail = 'running pre-transaction hooks';
    log(mop, ':: Running pre-transaction hooks...', '(1/2) Performing snapper pre snapshots for the following configurations...', '(2/2) Removing old entries from the ESP...', `:: Processing package changes...`);
    await wait(250);
    if (isDone(mop)) return;
  }

  op.progress.phaseDetail = isRemove ? 'removing packages' : 'installing packages';
  for (let i = resumeAtInstall; i < entries.length; i += 1) {
    const entry = entries[i]!;
    op.progress.currentPackage = entry.name;
    const verb = entry.action === 'remove' ? 'removing' : entry.action === 'install' ? 'installing' : entry.action === 'reinstall' ? 'reinstalling' : 'upgrading';
    log(mop, `(${i + 1}/${entries.length}) ${verb} ${entry.name}`);
    if (entry.name === 'systemd' && entry.action === 'upgrade') log(mop, 'warning: /etc/systemd/logind.conf installed as /etc/systemd/logind.conf.pacnew');
    await wait(160);
    op.progress.packagesDone = i + 1;
    if (state.failMode === 'afterCommit' && i === Math.floor(entries.length / 2)) {
      state.failMode = 'none';
      log(mop, `error: could not extract /usr/lib/modules/7.2.9-1-cachyos/vmlinuz (No space left on device)`, `error: problem occurred while upgrading ${entry.name}`, 'error: failed to commit transaction (transaction aborted)', 'Errors occurred, no packages were upgraded.');
      mop.op.changes.upgraded = i;
      mop.op.changes.packages = entries.slice(0, i).map((e) => e.name);
      finish(state, mop, 'needsAttention', 'transaction aborted during commit', appError('TRANSACTION_FAILED', 'failed to commit transaction (No space left on device)', `error: could not extract /usr/lib/modules/7.2.9-1-cachyos/vmlinuz (No space left on device)`));
      return;
    }
  }
  op.progress.currentPackage = null;
  op.progress.phaseDetail = 'running post-transaction hooks';
  log(mop, ':: Running post-transaction hooks...', '(1/5) Arming ConditionNeedsUpdate...', '(2/5) Updating module dependencies...', '(3/5) Updating linux initcpios...', '(4/5) Reloading system manager configuration...', '(5/5) Performing snapper post snapshots for the following configurations...');
  await wait(300);
  applySuccess(state, mop);
  log(mop, '==> transaction completed');
  finish(state, mop, 'succeeded', `${op.kind} completed`);
}

function currentPlanFor(state: MockState, mop: MockOperation): TransactionPlan | null {
  try {
    switch (mop.op.kind) {
      case 'systemUpgrade':
        return updatesResult(state).plan;
      case 'install': {
        const requested = mop.plan.entries.find((e) => e.requested);
        return requested?.repository ? planInstall(state, requested.repository, requested.name) : null;
      }
      case 'remove':
        return planRemove(state, mop.op.packageTargets[0] ?? '', mop.plan.recursive);
      default:
        return null;
    }
  } catch {
    return null;
  }
}

/** Mirror refresh between check and commit: one package added, one changed, one dropped. */
function changedUpdateDefs(defs: readonly UpdateDef[]): UpdateDef[] {
  const changed = defs
    .filter((d) => d.name !== 'cachyos-keyring')
    .map((d) => (d.name === 'firefox' ? { ...d, newVersion: '157.0.2-1.1' } : d));
  changed.push({ name: 'libdrm', repository: 'cachyos-extra-v3', oldVersion: '2.4.127-1.1', newVersion: '2.4.128-1.1', download: Math.round(0.4 * MIB), flags: ['driver', 'rebootRecommended'] });
  return changed;
}

function resumeRunningUpgrade(state: MockState): void {
  const plan = updatesResult(state).plan;
  if (!plan) return;
  const mop = newOperation(state, 'systemUpgrade', [], plan, plan.digest, false);
  const now = nowSec();
  mop.op.requestedAt = now - 40;
  mop.op.startedAt = now - 38;
  mop.op.state = 'installing';
  mop.op.commitStarted = true;
  mop.op.progress = { currentPackage: plan.entries[4]?.name ?? null, packagesDone: 4, packagesTotal: plan.entries.length, phaseDetail: 'installing packages' };
  log(mop, '==> pacman -Syu requested', 'authorization granted', ':: Synchronizing package databases...', ':: Starting full system upgrade...', ':: Retrieving packages...', 'checking keyring...', 'checking package integrity...', ':: Processing package changes...');
  plan.entries.slice(0, 4).forEach((e, i) => log(mop, `(${i + 1}/${plan.entries.length}) upgrading ${e.name}`));
  void runLifecycle(state, mop, 4);
}

function cancelOperation(state: MockState, id: string): Operation {
  const mop = state.operations.get(id);
  if (!mop) throw appError('NOT_FOUND', 'operation not found');
  if (!CANCELLABLE.includes(mop.op.state)) throw appError('INVALID_INPUT', 'the operation can no longer be cancelled (commit phase started)');
  log(mop, 'cancelled by user before commit; nothing was changed');
  finish(state, mop, 'cancelledBeforeCommit', 'cancelled by user');
  return clone(mop.op);
}

function operationLog(state: MockState, id: string, offset: number): OperationLogChunk {
  const mop = state.operations.get(id);
  if (!mop) throw appError('NOT_FOUND', 'operation not found');
  const encoder = new TextEncoder();
  let position = 0;
  const lines: string[] = [];
  for (const line of mop.lines) {
    if (position >= offset) lines.push(line);
    position += encoder.encode(`${line}\n`).length;
  }
  return { lines, nextOffset: position, complete: isDone(mop) };
}

// ---------------------------------------------------------------------------
// Settings, auto update, misc
// ---------------------------------------------------------------------------

function saveSettings(state: MockState, settings: Settings): Settings {
  if (![0, 1, 3, 6, 12, 24].includes(settings.checkIntervalHours)) throw appError('INVALID_INPUT', 'unsupported check interval');
  if (![7, 30, 90, 180, 365].includes(settings.logRetentionDays)) throw appError('INVALID_INPUT', 'unsupported log retention');
  state.settings = clone(settings);
  return clone(state.settings);
}

function nextWindowRun(config: AutoUpdateConfig): number | null {
  const [hours, minutes] = config.window.time.split(':').map(Number);
  const names = ['sun', 'mon', 'tue', 'wed', 'thu', 'fri', 'sat'];
  const start = new Date();
  for (let day = 0; day < 8; day += 1) {
    const candidate = new Date(start);
    candidate.setDate(start.getDate() + day);
    candidate.setHours(hours ?? 12, minutes ?? 0, 0, 0);
    const name = names[candidate.getDay()] as AutoUpdateConfig['window']['weekdays'][number];
    if (candidate > start && config.window.weekdays.includes(name)) return Math.floor(candidate.getTime() / 1000);
  }
  return null;
}

function setAutoUpdatePolicy(state: MockState, config: AutoUpdateConfig): AutoUpdateStatus {
  if (!state.appInfo.helperAvailable) throw appError('UNAVAILABLE', 'helper is not available', state.appInfo.helperError);
  if (config.window.weekdays.length === 0) throw appError('INVALID_INPUT', 'at least one weekday is required');
  if (!/^([01]\d|2[0-3]):[0-5]\d$/.test(config.window.time)) throw appError('INVALID_INPUT', 'time must use the HH:MM format');
  if (config.policy === 'prepareForNextReboot' && state.autoUpdate.prepareModeBlockers.length > 0) {
    throw appError('BLOCKED', 'prepareForNextReboot is blocked', state.autoUpdate.prepareModeBlockers.join(', '));
  }
  state.autoUpdate.config = clone(config);
  state.autoUpdate.timerEnabled = config.policy !== 'off';
  state.autoUpdate.nextRun = config.policy === 'off' ? null : nextWindowRun(config);
  return clone(state.autoUpdate);
}

function acknowledgeNews(state: MockState, until: number): NewsStatus {
  state.news.acknowledgedUntil = until;
  return newsStatus(state);
}

function getNews(state: MockState, refresh: boolean): NewsStatus {
  if (!state.settings.newsEnabled) return newsStatus(state);
  if (state.newsOffline) {
    state.news.errors = [
      { source: 'archLinux', message: 'error sending request for url (https://archlinux.org/feeds/news/): dns error' },
      { source: 'cachyOs', message: 'error sending request for url (https://cachyos.org/rss.xml): dns error' },
    ];
  } else if (refresh && (state.news.fetchedAt === null || nowSec() - state.news.fetchedAt > HOUR)) {
    state.news.fetchedAt = nowSec();
  }
  return newsStatus(state);
}

function checkUpdates(state: MockState): UpdateCheckResult {
  const now = nowSec();
  switch (state.checkBehavior) {
    case 'unsupported':
      throw appError('UNSUPPORTED', 'package functions are disabled (libalpm bridge unavailable)');
    case 'prerequisite':
      state.check = { status: 'prerequisiteMissing', checkedAt: null, attemptedAt: now, error: appError('PREREQUISITE_MISSING', 'checkupdates is not installed') };
      break;
    case 'offline':
      state.check = {
        ...state.check,
        status: 'failed',
        attemptedAt: now,
        error: appError('OFFLINE', 'failed to synchronize all databases (could not resolve host)', 'error: failed retrieving file core.db from mirror.cachyos.org : Could not resolve host'),
      };
      break;
    case 'ok':
      state.check = { status: 'fresh', checkedAt: now, attemptedAt: now, error: null };
      break;
  }
  return updatesResult(state);
}

function diagnosticReport(state: MockState): string {
  const s = systemInfo(state);
  const u = updatesResult(state);
  const h = healthReport(state);
  const gib = (bytes: number) => `${(bytes / GIB).toFixed(1)} GiB`;
  const ts = (value: number | null) => (value === null ? 'unknown' : new Date(value * 1000).toISOString().replace('.000Z', 'Z'));
  const lines = [
    'cachyos-center diagnostic report (sanitized)',
    `Created: ${ts(nowSec())}`,
    `cachyos-center: ${state.appInfo.version}`,
    '',
    '[System]',
    `OS: ${s.os.prettyName} (id=${s.os.id}${s.os.buildId ? `, build=${s.os.buildId}` : ''})`,
    `Kernel: ${s.kernel.release} (package: ${s.kernel.package ?? 'unknown'}), uptime: ${Math.floor(s.kernel.uptimeSeconds / 3600)} h`,
    `CPU: ${s.cpu.model} (${s.cpu.cores} cores/${s.cpu.threads} threads, ${s.cpu.isaLevel ?? 'unknown ISA level'})`,
    ...s.gpus.map((g) => `GPU: ${g.vendor} ${g.model} [${g.pciId}] driver=${g.driver ?? 'none'}`),
    `Memory: ${gib(s.memory.totalBytes)} total, ${gib(s.memory.availableBytes)} available; swap ${gib(s.memory.swapTotalBytes)}`,
    ...s.disks.map((d) => `Disk ${d.mountPoint}: ${d.filesystem}, ${gib(d.totalBytes)} total, ${gib(d.availableBytes)} free`),
    `Session: ${s.session.kind} (${s.session.desktop ?? 'unknown desktop'}, ${s.session.sessionType ?? 'unknown type'})`,
    '',
    '[Package management]',
    `pacman: ${s.pacman.pacmanVersion ?? 'unknown'}`,
    s.pacman.backend.state === 'ready'
      ? `libalpm: ${s.pacman.backend.libalpmVersion} (bridge built against ${s.pacman.backend.builtAgainst})`
      : `libalpm bridge: unavailable (${s.pacman.backend.reason})`,
    `Repositories: ${s.pacman.repositories.join(', ')}`,
    `Installed packages: ${s.pacman.installedCount} (local/AUR: ${s.pacman.foreignCount})`,
    `Lock: ${s.pacman.lock.state === 'free' ? 'free' : `locked (package manager running: ${String(s.pacman.lock.holderRunning)})`}`,
    `Last full upgrade: ${ts(s.pacman.lastFullUpgrade)}`,
    '',
    '[Updates]',
    `Status: ${u.status}, checked: ${ts(u.checkedAt)}, updates: ${u.updates.length}, held back: ${u.heldBack.length}, reboot recommended: ${u.rebootRecommended}`,
    ...(u.error ? [`Last check error: ${u.error.code} ${u.error.message}`] : []),
    '',
    '[Health]',
    ...(h.items.length === 0 ? ['No findings.'] : h.items.map((i) => `- ${i.severity} ${i.kind}: ${i.detail}`)),
    `Snapshots: btrfs=${h.snapshot.btrfsRoot}, snapper=${h.snapshot.snapperInstalled}, root config=${h.snapshot.rootConfig !== null}, snap-pac=${h.snapshot.snapPacActive}`,
    '',
    '[Automatic updates]',
    `Policy: ${state.autoUpdate.config.policy}, timer enabled: ${state.autoUpdate.timerEnabled}, next run: ${ts(state.autoUpdate.nextRun)}, pacman-offline installed: ${state.autoUpdate.offline.installed}, update prepared: ${state.autoUpdate.preparedForNextReboot}`,
    ...state.autoUpdate.externalUpdaters.map((x) => `Other updater: ${x.name} (${x.scope}, active: ${x.active})`),
    '',
    '[Recent activity]',
    ...state.activity.slice(0, 5).map((e) => `- ${ts(e.startedAt)} ${e.kind ?? 'pacman'} ${e.state ?? e.logOutcome ?? 'unknown'} (+${e.installed} ~${e.upgraded} -${e.removed})${e.errorCode ? ` ${e.errorCode}` : ''}`),
  ];
  return `${lines.join('\n')}\n`;
}

function validateUrl(url: string): void {
  const match = /^https:\/\/([^/:@\s]+)(\/[^\s]*)?$/.exec(url);
  const host = match?.[1]?.toLowerCase();
  if (!host || !ALLOWED_HOSTS.includes(host)) throw appError('INVALID_INPUT', `host ${host ?? '?'} is not allowed`);
  if (host === 'github.com' && !(match?.[2] ?? '').toLowerCase().startsWith('/jojo252511/cachyos-center')) {
    throw appError('INVALID_INPUT', 'only the project repository can be opened on GitHub');
  }
}

// ---------------------------------------------------------------------------
// Transport
// ---------------------------------------------------------------------------

const LATENCY: Record<string, number> = {
  check_updates: 2500,
  get_diagnostic_report: 500,
  plan_install: 700,
  plan_remove: 600,
  set_auto_update_policy: 1200,
  get_news: 450,
  start_upgrade: 400,
  start_install: 400,
  start_remove: 400,
};

type Handler = (state: MockState, args: InvokeArgs) => unknown;

const arg = <T>(args: InvokeArgs, name: string): T => {
  if (!(name in args)) throw appError('INVALID_INPUT', `missing argument ${name}`);
  return args[name] as T;
};

const HANDLERS: Record<string, Handler> = {
  get_app_info: (s) => clone(s.appInfo),
  get_dashboard: (s) => dashboard(s),
  get_system_info: (s) => systemInfo(s),
  get_hyprland_info: (s) => ({ ...clone(s.hyprland), collectedAt: nowSec() }),
  get_updates: (s) => updatesResult(s),
  check_updates: (s) => checkUpdates(s),
  list_installed: (s, a) => listInstalled(s, arg<InstalledQuery>(a, 'query')),
  search_packages: (s, a) => searchPackages(s, arg<CatalogQuery>(a, 'query')),
  get_package_details: (s, a) => packageDetails(s, arg<PackageRef>(a, 'reference')),
  list_repositories: (s) => {
    requireBackend(s);
    return REPOSITORIES.filter((r) => s.system.pacman.repositories.includes(r.name));
  },
  get_health: (s) => healthReport(s),
  get_news: (s, a) => getNews(s, arg<boolean>(a, 'refresh')),
  acknowledge_news: (s, a) => acknowledgeNews(s, arg<number>(a, 'until')),
  get_activity: (s, a) => {
    const limit = arg<number>(a, 'limit');
    if (limit < 1 || limit > 200) throw appError('INVALID_INPUT', 'limit must be between 1 and 200');
    return clone(s.activity.slice(0, limit));
  },
  get_settings: (s) => clone(s.settings),
  save_settings: (s, a) => saveSettings(s, arg<Settings>(a, 'settings')),
  get_auto_update_status: (s) => clone(s.autoUpdate),
  get_diagnostic_report: (s) => diagnosticReport(s),
  get_mcp_setup: (s): McpSetup => ({
    enabled: s.settings.mcpEnabled,
    binaryPath: '/usr/bin/cachyos-center-mcp',
    hostConfig: mcpHostConfig('/usr/bin/cachyos-center-mcp'),
    tools: [...MCP_TOOLS],
  }),
  plan_install: (s, a) => planInstall(s, arg<string>(a, 'repository'), arg<string>(a, 'name')),
  plan_remove: (s, a) => planRemove(s, arg<string>(a, 'name'), arg<boolean>(a, 'recursive')),
  start_upgrade: (s, a) => {
    const digest = arg<string>(a, 'planDigest');
    assertStartable(s, digest);
    const plan = updatesResult(s).plan;
    if (!plan) throw appError('INVALID_INPUT', 'there is nothing to upgrade');
    const mop = newOperation(s, 'systemUpgrade', [], plan, digest, arg<boolean>(a, 'createSnapshot'));
    void runLifecycle(s, mop);
    return mop.op.id;
  },
  start_install: (s, a) => {
    const digest = arg<string>(a, 'planDigest');
    const repository = arg<string>(a, 'repository');
    const name = arg<string>(a, 'name');
    assertStartable(s, digest);
    const plan = planInstall(s, repository, name);
    const mop = newOperation(s, 'install', [name], plan, digest, false);
    void runLifecycle(s, mop);
    return mop.op.id;
  },
  start_remove: (s, a) => {
    const digest = arg<string>(a, 'planDigest');
    const name = arg<string>(a, 'name');
    assertStartable(s, digest);
    const plan = planRemove(s, name, arg<boolean>(a, 'recursive'));
    const mop = newOperation(s, 'remove', [name], plan, digest, false);
    void runLifecycle(s, mop);
    return mop.op.id;
  },
  set_auto_update_policy: (s, a) => setAutoUpdatePolicy(s, arg<AutoUpdateConfig>(a, 'config')),
  cancel_operation: (s, a) => cancelOperation(s, arg<string>(a, 'id')),
  get_operation: (s, a) => {
    const mop = s.operations.get(arg<string>(a, 'id'));
    if (!mop) throw appError('NOT_FOUND', 'operation not found');
    return clone(mop.op);
  },
  get_operation_log: (s, a) => operationLog(s, arg<string>(a, 'id'), arg<number>(a, 'offset')),
  get_current_operation: (s) => {
    const current = lastOperation(s);
    return current ? clone(current.op) : null;
  },
  open_external: (_s, a) => {
    validateUrl(arg<string>(a, 'url'));
    return null;
  },
  reveal_config_file: (s, a) => {
    const path = arg<string>(a, 'path');
    if (!s.configFiles.some((f) => f.path === path)) throw appError('NOT_FOUND', 'file is not part of the current health report');
    return null;
  },
  write_clipboard: (_s, a) => {
    const text = arg<string>(a, 'text');
    void navigator.clipboard?.writeText(text).catch(() => undefined);
    return null;
  },
};

export interface MockTransport extends Transport {
  readonly scenario: ScenarioId;
  readonly scenarios: readonly ScenarioId[];
}

export function createMockTransport(search: string, options: { latency?: boolean } = {}): MockTransport {
  const scenario = parseScenario(search);
  const state = createState(scenario);
  const latency = options.latency ?? true;

  if (state.hyprland.available && latency) {
    let workspace = 2;
    setInterval(() => {
      workspace = (workspace % 6) + 1;
      state.hyprland = buildHyprland(nowSec(), workspace);
      emit(state, 'hyprland-changed', null);
    }, 45_000);
  }

  return {
    kind: 'mock',
    scenario,
    scenarios: SCENARIOS,
    async invoke<T>(command: string, args: InvokeArgs = {}): Promise<T> {
      const handler = HANDLERS[command];
      if (latency) await wait(LATENCY[command] ?? 120 + Math.round(Math.random() * 180));
      if (!handler) throw appError('INTERNAL', `unknown command ${command}`);
      return handler(state, args) as T;
    },
    async listen<T>(event: string, handler: (payload: T) => void): Promise<Unlisten> {
      const set = state.listeners.get(event) ?? new Set();
      const wrapped = handler as (payload: unknown) => void;
      set.add(wrapped);
      state.listeners.set(event, set);
      return () => set.delete(wrapped);
    },
  };
}
