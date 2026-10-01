import { describe, expect, it } from 'vitest';

import { de } from '../i18n/de';
import { en } from '../i18n/en';
import { createTranslate } from '../i18n';
import { ERROR_CODES, ERROR_SPECS, describeError, normalizeError, parseDependentPackages } from './errors';

describe('error mapping', () => {
  it('covers every ErrorCode with a localized title, explanation and a concrete next action', () => {
    expect(ERROR_CODES).toHaveLength(15);
    for (const lang of ['de', 'en'] as const) {
      const t = createTranslate(lang);
      for (const code of ERROR_CODES) {
        const info = describeError({ code, message: 'technical', detail: null }, t);
        expect(info.title.length, code).toBeGreaterThan(0);
        expect(info.explanation.length, code).toBeGreaterThan(0);
        expect(info.actions.length, code).toBeGreaterThan(0);
        expect(['retry', 'viewLog', 'terminal', 'docs']).toEqual(expect.arrayContaining([...info.actions]));
        if (info.actions.includes('terminal')) expect(info.command, code).toMatch(/^sudo pacman -Syu/);
        if (info.actions.includes('docs')) expect(info.docUrl, code).toMatch(/^https:\/\//);
      }
    }
    expect(Object.keys(ERROR_SPECS).sort()).toEqual([...ERROR_CODES].sort());
    for (const code of ERROR_CODES) {
      expect(de[`error.${code}.title`]).toBeTruthy();
      expect(en[`error.${code}.text`]).toBeTruthy();
    }
  });

  it('never offers to remove the pacman lock', () => {
    expect(ERROR_SPECS.BUSY.actions).not.toContain('terminal');
    for (const spec of Object.values(ERROR_SPECS)) expect(spec.command ?? '').not.toMatch(/db\.lck|rm /);
    for (const dictionary of [de, en]) {
      for (const message of Object.values(dictionary)) {
        const text = typeof message === 'string' ? message : message.other;
        expect(text).not.toMatch(/rm .*db\.lck/);
      }
    }
  });

  it('normalizes rejections into AppError', () => {
    expect(normalizeError({ code: 'BUSY', message: 'locked', detail: null })).toEqual({ code: 'BUSY', message: 'locked', detail: null });
    expect(normalizeError({ code: 'OFFLINE', message: 'x' })).toEqual({ code: 'OFFLINE', message: 'x', detail: null });
    expect(normalizeError(JSON.stringify({ code: 'STALE', message: 'old', detail: 'd' }))).toEqual({ code: 'STALE', message: 'old', detail: 'd' });
    expect(normalizeError('plain failure')).toEqual({ code: 'INTERNAL', message: 'plain failure', detail: null });
    expect(normalizeError(new Error('boom')).code).toBe('INTERNAL');
    expect(normalizeError({ code: 'UNKNOWN_CODE', message: 'x' }).code).toBe('INTERNAL');
    expect(normalizeError(undefined).code).toBe('INTERNAL');
  });

  it('extracts dependent packages from DEPENDENCY_PROBLEM details', () => {
    const detail = [
      "removing gtk3 breaks dependency 'gtk3' required by firefox",
      "unable to satisfy dependency 'gtk3>=3.24' required by waybar",
      'a and b are in conflict (x)',
      "removing gtk3 breaks dependency 'gtk3' required by firefox",
    ].join('\n');
    expect(parseDependentPackages(detail)).toEqual(['firefox', 'waybar']);
    expect(parseDependentPackages(null)).toEqual([]);
    expect(parseDependentPackages("unable to satisfy dependency 'x' required by rm -rf /")).toEqual([]);
  });
});
