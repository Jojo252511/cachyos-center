import type { AppError } from '../bindings/AppError';
import type { ErrorCode } from '../bindings/ErrorCode';
import type { Translate } from '../i18n';

/**
 * Every error code of the contract. `Record<ErrorCode, …>` makes the compiler
 * reject missing or unknown codes when the Rust enum changes.
 */
const ERROR_CODE_TABLE: Record<ErrorCode, true> = {
  UNAVAILABLE: true,
  BUSY: true,
  STALE: true,
  UNSUPPORTED: true,
  INTERNAL: true,
  INVALID_INPUT: true,
  NOT_FOUND: true,
  NOT_AUTHORIZED: true,
  OFFLINE: true,
  PREREQUISITE_MISSING: true,
  PLAN_CHANGED: true,
  CONFLICT: true,
  BLOCKED: true,
  TRANSACTION_FAILED: true,
  DEPENDENCY_PROBLEM: true,
};

export const ERROR_CODES = Object.keys(ERROR_CODE_TABLE) as ErrorCode[];

export function isErrorCode(value: unknown): value is ErrorCode {
  return typeof value === 'string' && Object.hasOwn(ERROR_CODE_TABLE, value);
}

export function isAppError(value: unknown): value is AppError {
  if (typeof value !== 'object' || value === null) return false;
  const candidate = value as Record<string, unknown>;
  return (
    isErrorCode(candidate.code) &&
    typeof candidate.message === 'string' &&
    (candidate.detail === null || candidate.detail === undefined || typeof candidate.detail === 'string')
  );
}

/**
 * Normalizes any promise rejection into an `AppError`.
 *
 * Tauri rejects with the serialized `AppError` object; other failures (IPC
 * transport problems, programming errors) become `INTERNAL`.
 */
export function normalizeError(error: unknown): AppError {
  if (isAppError(error)) {
    return { code: error.code, message: error.message, detail: error.detail ?? null };
  }
  if (typeof error === 'string') {
    try {
      const parsed: unknown = JSON.parse(error);
      if (isAppError(parsed)) return normalizeError(parsed);
    } catch {
      // Plain text error message.
    }
    return { code: 'INTERNAL', message: error, detail: null };
  }
  if (error instanceof Error) {
    return { code: 'INTERNAL', message: error.message, detail: null };
  }
  return { code: 'INTERNAL', message: 'unknown error', detail: null };
}

/** Concrete next steps offered for an error (concept §7). */
export type ErrorActionKind = 'retry' | 'viewLog' | 'terminal' | 'docs';

export interface ErrorSpec {
  /** Offered actions in order of relevance. */
  readonly actions: readonly ErrorActionKind[];
  /** Exact, safe command for „Im Terminal reparieren“. Never touches db.lck. */
  readonly command?: string;
  /** Official documentation for „Dokumentation öffnen“ (allow-listed hosts only). */
  readonly docUrl?: string;
}

export const DOC_URLS = {
  project: 'https://github.com/Jojo252511/cachyos-center',
  cachyosPostInstall: 'https://wiki.cachyos.org/configuration/post_install_setup/',
  archSystemMaintenance: 'https://wiki.archlinux.org/title/System_maintenance',
  archPacman: 'https://wiki.archlinux.org/title/Pacman',
  archNews: 'https://archlinux.org/news/',
} as const;

export const FULL_UPGRADE_COMMAND = 'sudo pacman -Syu';

export function prerequisiteCommand(packages: readonly string[]): string {
  const names = packages.length > 0 ? packages.join(' ') : 'pacman-contrib';
  return `sudo pacman -Syu --needed ${names}`;
}

export const ERROR_SPECS: Record<ErrorCode, ErrorSpec> = {
  UNAVAILABLE: { actions: ['retry', 'docs'], docUrl: DOC_URLS.project },
  // Never offer to delete the lock: wait for the other package manager and check again.
  BUSY: { actions: ['retry'] },
  STALE: { actions: ['retry'] },
  UNSUPPORTED: { actions: ['docs'], docUrl: DOC_URLS.project },
  INTERNAL: { actions: ['viewLog', 'retry'] },
  INVALID_INPUT: { actions: ['retry'] },
  NOT_FOUND: { actions: ['retry'] },
  NOT_AUTHORIZED: { actions: ['retry'] },
  OFFLINE: { actions: ['retry'] },
  PREREQUISITE_MISSING: { actions: ['terminal', 'retry'], command: prerequisiteCommand([]) },
  PLAN_CHANGED: { actions: ['retry'] },
  CONFLICT: { actions: ['docs', 'retry'], docUrl: DOC_URLS.cachyosPostInstall },
  BLOCKED: { actions: ['viewLog', 'retry'] },
  TRANSACTION_FAILED: {
    actions: ['viewLog', 'terminal', 'retry'],
    command: FULL_UPGRADE_COMMAND,
    docUrl: DOC_URLS.archSystemMaintenance,
  },
  DEPENDENCY_PROBLEM: { actions: ['viewLog', 'docs'], docUrl: DOC_URLS.archPacman },
};

export interface ErrorInfo {
  code: ErrorCode;
  /** Localized title for `code`. */
  title: string;
  /** Localized explanation for `code`. */
  explanation: string;
  actions: readonly ErrorActionKind[];
  command?: string;
  docUrl?: string;
  /** Technical English message, shown only as detail. */
  message: string;
  detail: string | null;
}

/** Localized description of an error with its concrete next steps. */
export function describeError(error: AppError, t: Translate, overrides?: { command?: string }): ErrorInfo {
  const spec = ERROR_SPECS[error.code];
  return {
    code: error.code,
    title: t(`error.${error.code}.title`),
    explanation: t(`error.${error.code}.text`),
    actions: spec.actions,
    command: overrides?.command ?? spec.command,
    docUrl: spec.docUrl,
    message: error.message,
    detail: error.detail,
  };
}

const DEPENDENT_LINE =
  /^(?:removing \S+ breaks dependency|unable to satisfy dependency) '[^']*' required by ([a-zA-Z0-9@_+][a-zA-Z0-9@._+-]{0,127})$/;

/**
 * Extracts the dependent packages from the `detail` of a `DEPENDENCY_PROBLEM`.
 *
 * The libalpm bridge writes one line per unsatisfied dependency with pacman's
 * wording: `removing <pkg> breaks dependency '<dep>' required by <package>` or
 * `unable to satisfy dependency '<dep>' required by <package>`. Only names
 * matching that format are returned; other lines (e.g. conflicts) stay in the
 * raw detail.
 */
export function parseDependentPackages(detail: string | null): string[] {
  if (!detail) return [];
  const names = detail
    .split('\n')
    .map((line) => DEPENDENT_LINE.exec(line.trim())?.[1])
    .filter((name): name is string => name !== undefined);
  return [...new Set(names)];
}
