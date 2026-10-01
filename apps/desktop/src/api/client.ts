/**
 * Typed client for the Tauri commands and events of the IPC contract
 * (`docs/ipc-vertrag.md`). Argument names are camelCase; Tauri maps them to the
 * snake_case parameters of the Rust commands.
 *
 * Inside the Tauri webview the real backend is used. In a plain browser (Vite
 * dev server, visual QA) a realistic mock is loaded with a dynamic import, so it
 * never runs inside the desktop app.
 */
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import type { AppInfo } from '../bindings/AppInfo';
import type { AutoUpdateConfig } from '../bindings/AutoUpdateConfig';
import type { AutoUpdateStatus } from '../bindings/AutoUpdateStatus';
import type { CatalogQuery } from '../bindings/CatalogQuery';
import type { Dashboard } from '../bindings/Dashboard';
import type { HealthReport } from '../bindings/HealthReport';
import type { HistoryEntry } from '../bindings/HistoryEntry';
import type { HyprlandInfo } from '../bindings/HyprlandInfo';
import type { InstalledQuery } from '../bindings/InstalledQuery';
import type { McpSetup } from '../bindings/McpSetup';
import type { NewsStatus } from '../bindings/NewsStatus';
import type { Operation } from '../bindings/Operation';
import type { OperationLogChunk } from '../bindings/OperationLogChunk';
import type { PackagePage } from '../bindings/PackagePage';
import type { PackageRecord } from '../bindings/PackageRecord';
import type { PackageRef } from '../bindings/PackageRef';
import type { PackageSummary } from '../bindings/PackageSummary';
import type { RepositoryInfo } from '../bindings/RepositoryInfo';
import type { Settings } from '../bindings/Settings';
import type { SystemInfo } from '../bindings/SystemInfo';
import type { TransactionPlan } from '../bindings/TransactionPlan';
import type { UpdateCheckResult } from '../bindings/UpdateCheckResult';
import { normalizeError } from './errors';

export type Unlisten = () => void;
export type InvokeArgs = Record<string, unknown>;

/** Minimal transport used by the client: the Tauri IPC or the browser mock. */
export interface Transport {
  readonly kind: 'tauri' | 'mock';
  /** Active demo scenario (browser mock only). */
  readonly scenario?: string;
  readonly scenarios?: readonly string[];
  invoke<T>(command: string, args?: InvokeArgs): Promise<T>;
  listen<T>(event: string, handler: (payload: T) => void): Promise<Unlisten>;
}

export interface BackendInfo {
  kind: Transport['kind'];
  scenario: string | null;
  scenarios: readonly string[];
}

/** Event names of the contract. */
export const EVENTS = {
  hyprlandChanged: 'hyprland-changed',
  updatesChecked: 'updates-checked',
  operationFinished: 'operation-finished',
} as const;

export function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

const tauriTransport: Transport = {
  kind: 'tauri',
  invoke: <T>(command: string, args?: InvokeArgs) => invoke<T>(command, args),
  listen: <T>(event: string, handler: (payload: T) => void) =>
    listen<T>(event, (e) => handler(e.payload)),
};

let transportPromise: Promise<Transport> | null = null;

async function createTransport(): Promise<Transport> {
  if (isTauriRuntime()) return tauriTransport;
  if (import.meta.env.TAURI_ENV_PLATFORM) {
    // Built or served by the Tauri CLI but opened outside the webview: never
    // show demo data where real system data is expected.
    throw new Error('Tauri runtime not available');
  }
  const { createMockTransport } = await import('./mock');
  return createMockTransport(window.location.search);
}

export function getTransport(): Promise<Transport> {
  transportPromise ??= createTransport();
  return transportPromise;
}

/** Replaces the transport (tests and tooling); `null` restores the runtime detection. */
export function overrideTransport(transport: Transport | null): void {
  transportPromise = transport ? Promise.resolve(transport) : null;
}

async function call<T>(command: string, args?: InvokeArgs): Promise<T> {
  try {
    const transport = await getTransport();
    return await transport.invoke<T>(command, args);
  } catch (error) {
    throw normalizeError(error);
  }
}

async function subscribe<T>(event: string, handler: (payload: T) => void): Promise<Unlisten> {
  const transport = await getTransport();
  return transport.listen<T>(event, handler);
}

export interface Api {
  // Reading commands
  getAppInfo(): Promise<AppInfo>;
  getDashboard(): Promise<Dashboard>;
  getSystemInfo(): Promise<SystemInfo>;
  getHyprlandInfo(): Promise<HyprlandInfo>;
  getUpdates(): Promise<UpdateCheckResult>;
  checkUpdates(): Promise<UpdateCheckResult>;
  listInstalled(query: InstalledQuery): Promise<PackagePage>;
  searchPackages(query: CatalogQuery): Promise<PackageSummary[]>;
  getPackageDetails(reference: PackageRef): Promise<PackageRecord>;
  listRepositories(): Promise<RepositoryInfo[]>;
  getHealth(): Promise<HealthReport>;
  getNews(refresh: boolean, force: boolean): Promise<NewsStatus>;
  acknowledgeNews(until: number): Promise<NewsStatus>;
  getActivity(limit: number): Promise<HistoryEntry[]>;
  getSettings(): Promise<Settings>;
  saveSettings(settings: Settings): Promise<Settings>;
  getAutoUpdateStatus(): Promise<AutoUpdateStatus>;
  getDiagnosticReport(): Promise<string>;
  getMcpSetup(): Promise<McpSetup>;
  // Planning commands (no changes, no root)
  planInstall(repository: string, name: string): Promise<TransactionPlan>;
  planRemove(name: string, recursive: boolean): Promise<TransactionPlan>;
  // System changing commands (helper, polkit)
  startUpgrade(planDigest: string, createSnapshot: boolean): Promise<string>;
  startInstall(repository: string, name: string, planDigest: string): Promise<string>;
  startRemove(name: string, recursive: boolean, planDigest: string): Promise<string>;
  setAutoUpdatePolicy(config: AutoUpdateConfig): Promise<AutoUpdateStatus>;
  cancelOperation(id: string): Promise<Operation>;
  // Operation status
  getOperation(id: string): Promise<Operation>;
  getOperationLog(id: string, offset: number): Promise<OperationLogChunk>;
  getCurrentOperation(): Promise<Operation | null>;
  // Further actions (only after an explicit user action)
  openExternal(url: string): Promise<void>;
  revealConfigFile(path: string): Promise<void>;
  writeClipboard(text: string): Promise<void>;
  // Runtime (not a backend command): real backend or browser demo mock
  backendInfo(): Promise<BackendInfo>;
  // Events
  onHyprlandChanged(handler: () => void): Promise<Unlisten>;
  onUpdatesChecked(handler: (result: UpdateCheckResult) => void): Promise<Unlisten>;
  onOperationFinished(handler: (operation: Operation) => void): Promise<Unlisten>;
}

export const api: Api = {
  getAppInfo: () => call('get_app_info'),
  getDashboard: () => call('get_dashboard'),
  getSystemInfo: () => call('get_system_info'),
  getHyprlandInfo: () => call('get_hyprland_info'),
  getUpdates: () => call('get_updates'),
  checkUpdates: () => call('check_updates'),
  listInstalled: (query) => call('list_installed', { query }),
  searchPackages: (query) => call('search_packages', { query }),
  getPackageDetails: (reference) => call('get_package_details', { reference }),
  listRepositories: () => call('list_repositories'),
  getHealth: () => call('get_health'),
  getNews: (refresh, force) => call('get_news', { refresh, force }),
  acknowledgeNews: (until) => call('acknowledge_news', { until }),
  getActivity: (limit) => call('get_activity', { limit }),
  getSettings: () => call('get_settings'),
  saveSettings: (settings) => call('save_settings', { settings }),
  getAutoUpdateStatus: () => call('get_auto_update_status'),
  getDiagnosticReport: () => call('get_diagnostic_report'),
  getMcpSetup: () => call('get_mcp_setup'),
  planInstall: (repository, name) => call('plan_install', { repository, name }),
  planRemove: (name, recursive) => call('plan_remove', { name, recursive }),
  startUpgrade: (planDigest, createSnapshot) =>
    call('start_upgrade', { planDigest, createSnapshot }),
  startInstall: (repository, name, planDigest) =>
    call('start_install', { repository, name, planDigest }),
  startRemove: (name, recursive, planDigest) =>
    call('start_remove', { name, recursive, planDigest }),
  setAutoUpdatePolicy: (config) => call('set_auto_update_policy', { config }),
  cancelOperation: (id) => call('cancel_operation', { id }),
  getOperation: (id) => call('get_operation', { id }),
  getOperationLog: (id, offset) => call('get_operation_log', { id, offset }),
  getCurrentOperation: () => call('get_current_operation'),
  openExternal: (url) => call('open_external', { url }),
  revealConfigFile: (path) => call('reveal_config_file', { path }),
  writeClipboard: (text) => call('write_clipboard', { text }),
  backendInfo: async () => {
    const transport = await getTransport();
    return { kind: transport.kind, scenario: transport.scenario ?? null, scenarios: transport.scenarios ?? [] };
  },
  onHyprlandChanged: (handler) => subscribe<unknown>(EVENTS.hyprlandChanged, () => handler()),
  onUpdatesChecked: (handler) => subscribe<UpdateCheckResult>(EVENTS.updatesChecked, handler),
  onOperationFinished: (handler) => subscribe<Operation>(EVENTS.operationFinished, handler),
};
