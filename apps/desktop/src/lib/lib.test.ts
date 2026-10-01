import { describe, expect, it } from 'vitest';

import { buildUpgradePlan, UPDATE_DEFS } from '../api/mockData';
import type { NewsStatus } from '../bindings/NewsStatus';
import type { UpdateCheckResult } from '../bindings/UpdateCheckResult';
import { dashboardHeadline } from './headline';
import { isOpenableUrl } from './links';
import { newsGate, requiresNewsAcknowledgement } from './news';
import { countByAction, diffPlans, includedUpgrades, isDiffEmpty, planRepositories } from './plan';

const result = (patch: Partial<UpdateCheckResult>): UpdateCheckResult => ({
  status: 'fresh',
  checkedAt: 1_000,
  attemptedAt: 1_000,
  updates: [],
  plan: null,
  heldBack: [],
  totalDownloadSize: 0,
  rebootRecommended: false,
  error: null,
  missingPrerequisites: [],
  ...patch,
});

describe('plan helpers', () => {
  const plan = buildUpgradePlan(UPDATE_DEFS, 1_000);

  it('counts actions and lists repositories in order', () => {
    expect(countByAction(plan).upgrade).toBe(12);
    expect(planRepositories(plan)).toEqual(['cachyos-core-v3', 'core', 'cachyos-extra-v3', 'multilib', 'cachyos']);
  });

  it('diffs a confirmed and an actual plan like the Rust core', () => {
    const actual = {
      ...plan,
      entries: [
        ...plan.entries.filter((e) => e.name !== 'cachyos-keyring').map((e) => (e.name === 'firefox' ? { ...e, newVersion: '157.0.2-1.1' } : e)),
        { action: 'upgrade' as const, name: 'libdrm', repository: 'extra', oldVersion: '1', newVersion: '2', downloadSize: 1, requested: false, flags: [] },
      ],
    };
    const diff = diffPlans(plan, actual);
    expect(diff.added.map((e) => e.name)).toEqual(['libdrm']);
    expect(diff.removed.map((e) => e.name)).toEqual(['cachyos-keyring']);
    expect(diff.changed.map((e) => e.name)).toEqual(['firefox']);
    expect(isDiffEmpty(diffPlans(plan, plan))).toBe(true);
  });

  it('reads the number of included upgrades of an install plan', () => {
    expect(includedUpgrades({ ...plan, warnings: [{ kind: 'includesSystemUpgrade', upgradeCount: 7 }] })).toBe(7);
  });
});

describe('dashboard headline', () => {
  const lock = { state: 'free' } as const;

  it('never reports a count for uncertain states', () => {
    for (const status of ['neverChecked', 'failed', 'unsupported', 'prerequisiteMissing', 'stale'] as const) {
      const headline = dashboardHeadline({ updates: result({ status }), lock, operationActive: false });
      expect(headline.count, status).toBeNull();
      expect(headline.kind).not.toBe('current');
    }
  });

  it('reports fresh results with their count', () => {
    const update = { packageId: { repository: 'core', name: 'a', architecture: 'x86_64' }, oldVersion: '1', newVersion: '2', downloadSize: 1, flags: [] };
    expect(dashboardHeadline({ updates: result({ updates: [update] }), lock, operationActive: false })).toMatchObject({ kind: 'updates', count: 1 });
    expect(dashboardHeadline({ updates: result({}), lock, operationActive: false })).toMatchObject({ kind: 'current', count: 0 });
    expect(dashboardHeadline({ updates: result({ heldBack: [update] }), lock, operationActive: false }).kind).toBe('currentHeldBack');
  });

  it('prefers running operations and a foreign lock', () => {
    expect(dashboardHeadline({ updates: result({}), lock, operationActive: true }).kind).toBe('operation');
    expect(dashboardHeadline({ updates: result({}), lock: { state: 'locked', since: null, holderRunning: true }, operationActive: false }).kind).toBe('locked');
  });
});

describe('news gate', () => {
  const news = (patch: Partial<NewsStatus>): NewsStatus => ({ disabled: false, items: [], fetchedAt: 1, errors: [], acknowledgedUntil: null, unreadCount: 0, ...patch });

  it('requires an acknowledgement for unread or unavailable news only', () => {
    expect(requiresNewsAcknowledgement(newsGate(news({}), null, false))).toBe(false);
    expect(newsGate(news({ unreadCount: 2 }), null, false)).toMatchObject({ state: 'unread', unreadCount: 2 });
    expect(newsGate(news({ errors: [{ source: 'archLinux', message: 'dns' }] }), null, false).state).toBe('unavailable');
    expect(newsGate(news({ fetchedAt: null }), null, false).state).toBe('unavailable');
    expect(newsGate(news({ disabled: true }), null, false).state).toBe('unavailable');
    expect(newsGate(undefined, { code: 'OFFLINE', message: 'x', detail: null }, false).state).toBe('unavailable');
    expect(newsGate(undefined, null, true).state).toBe('loading');
  });
});

describe('external links', () => {
  it('accepts only allow-listed https hosts', () => {
    expect(isOpenableUrl('https://wiki.cachyos.org/configuration/post_install_setup/')).toBe(true);
    expect(isOpenableUrl('https://github.com/Jojo252511/cachyos-center')).toBe(true);
    expect(isOpenableUrl('https://github.com/someone/else')).toBe(false);
    expect(isOpenableUrl('http://archlinux.org/')).toBe(false);
    expect(isOpenableUrl('https://www.mozilla.org/firefox/')).toBe(false);
  });
});
