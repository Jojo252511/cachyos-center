import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const { invoke, listen } = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen }));

/** Fresh client module so that the mocked Tauri modules are used. */
async function loadClient() {
  vi.resetModules();
  return import('./client');
}

describe('API client (Tauri transport)', () => {
  beforeEach(() => {
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    invoke.mockReset().mockResolvedValue('ok');
    listen.mockReset().mockResolvedValue(() => undefined);
  });

  afterEach(() => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  });

  it('calls every command of the contract with camelCase arguments', async () => {
    const { api } = await loadClient();
    const digest = 'a'.repeat(64);
    const calls: Array<[Promise<unknown>, string, Record<string, unknown> | undefined]> = [
      [api.getAppInfo(), 'get_app_info', undefined],
      [api.getDashboard(), 'get_dashboard', undefined],
      [api.getSystemInfo(), 'get_system_info', undefined],
      [api.getHyprlandInfo(), 'get_hyprland_info', undefined],
      [api.getUpdates(), 'get_updates', undefined],
      [api.checkUpdates(), 'check_updates', undefined],
      [api.listInstalled({ query: null, filter: 'all', offset: 0, limit: 50 }), 'list_installed', { query: { query: null, filter: 'all', offset: 0, limit: 50 } }],
      [api.searchPackages({ query: 'gimp', repository: null, installFilter: 'any', limit: 100 }), 'search_packages', { query: { query: 'gimp', repository: null, installFilter: 'any', limit: 100 } }],
      [api.getPackageDetails({ name: 'gimp', repository: 'extra' }), 'get_package_details', { reference: { name: 'gimp', repository: 'extra' } }],
      [api.listRepositories(), 'list_repositories', undefined],
      [api.getHealth(), 'get_health', undefined],
      [api.getNews(true, false), 'get_news', { refresh: true, force: false }],
      [api.acknowledgeNews(42), 'acknowledge_news', { until: 42 }],
      [api.getActivity(100), 'get_activity', { limit: 100 }],
      [api.getSettings(), 'get_settings', undefined],
      [api.getAutoUpdateStatus(), 'get_auto_update_status', undefined],
      [api.getDiagnosticReport(), 'get_diagnostic_report', undefined],
      [api.getMcpSetup(), 'get_mcp_setup', undefined],
      [api.planInstall('extra', 'gimp'), 'plan_install', { repository: 'extra', name: 'gimp' }],
      [api.planRemove('gimp', true), 'plan_remove', { name: 'gimp', recursive: true }],
      [api.startUpgrade(digest, true), 'start_upgrade', { planDigest: digest, createSnapshot: true }],
      [api.startInstall('extra', 'gimp', digest), 'start_install', { repository: 'extra', name: 'gimp', planDigest: digest }],
      [api.startRemove('gimp', false, digest), 'start_remove', { name: 'gimp', recursive: false, planDigest: digest }],
      [api.cancelOperation('id-1'), 'cancel_operation', { id: 'id-1' }],
      [api.getOperation('id-1'), 'get_operation', { id: 'id-1' }],
      [api.getOperationLog('id-1', 128), 'get_operation_log', { id: 'id-1', offset: 128 }],
      [api.getCurrentOperation(), 'get_current_operation', undefined],
      [api.openExternal('https://archlinux.org/news/'), 'open_external', { url: 'https://archlinux.org/news/' }],
      [api.revealConfigFile('/etc/pacman.conf.pacnew'), 'reveal_config_file', { path: '/etc/pacman.conf.pacnew' }],
      [api.writeClipboard('text'), 'write_clipboard', { text: 'text' }],
    ];
    const settings = { theme: 'dark', language: 'de', density: 'compact', notifications: true, checkIntervalHours: 6, logRetentionDays: 90, newsEnabled: true, mcpEnabled: false } as const;
    calls.push([api.saveSettings(settings), 'save_settings', { settings }]);
    const config = { policy: 'notifyOnly', window: { weekdays: ['mon'], time: '10:00' }, requireSnapshot: false, newsAcknowledgedUntil: null } as const;
    calls.push([api.setAutoUpdatePolicy({ ...config, window: { ...config.window, weekdays: [...config.window.weekdays] } }), 'set_auto_update_policy', { config }]);

    await Promise.all(calls.map(([promise]) => promise));
    expect(invoke).toHaveBeenCalledTimes(calls.length);
    calls.forEach(([, command, args], index) => {
      expect(invoke.mock.calls[index]?.[0]).toBe(command);
      expect(invoke.mock.calls[index]?.[1]).toEqual(args);
    });
  });

  it('subscribes to the contract events and unwraps the payload', async () => {
    const { api } = await loadClient();
    const handler = vi.fn();
    await api.onOperationFinished(handler);
    await api.onUpdatesChecked(handler);
    await api.onHyprlandChanged(handler);
    expect(listen.mock.calls.map((call) => call[0])).toEqual(['operation-finished', 'updates-checked', 'hyprland-changed']);
    const callback = listen.mock.calls[0]?.[1] as (event: { payload: unknown }) => void;
    callback({ payload: { id: 'x' } });
    expect(handler).toHaveBeenCalledWith({ id: 'x' });
  });

  it('normalizes rejected commands into AppError', async () => {
    const { api } = await loadClient();
    invoke.mockRejectedValueOnce({ code: 'NOT_AUTHORIZED', message: 'polkit denied', detail: null });
    await expect(api.startUpgrade('a'.repeat(64), false)).rejects.toEqual({ code: 'NOT_AUTHORIZED', message: 'polkit denied', detail: null });
    invoke.mockRejectedValueOnce('IPC broken');
    await expect(api.getDashboard()).rejects.toMatchObject({ code: 'INTERNAL', message: 'IPC broken' });
  });
});
