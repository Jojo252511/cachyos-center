/**
 * Realistic fixture data for the browser mock and the tests. Values follow a
 * CachyOS installation with Hyprland (pacman 7.1, libalpm 16, kernel 7.2).
 */
import type { AutoUpdateStatus } from '../bindings/AutoUpdateStatus';
import type { HistoryEntry } from '../bindings/HistoryEntry';
import type { HyprlandInfo } from '../bindings/HyprlandInfo';
import type { NewsItem } from '../bindings/NewsItem';
import type { PackageRecord } from '../bindings/PackageRecord';
import type { PackageSummary } from '../bindings/PackageSummary';
import type { PlanEntry } from '../bindings/PlanEntry';
import type { RepositoryInfo } from '../bindings/RepositoryInfo';
import type { Settings } from '../bindings/Settings';
import type { SystemInfo } from '../bindings/SystemInfo';
import type { TransactionPlan } from '../bindings/TransactionPlan';
import type { UpdateCandidate } from '../bindings/UpdateCandidate';
import type { UpdateFlag } from '../bindings/UpdateFlag';

export const MIB = 1024 * 1024;
export const GIB = 1024 * MIB;
const HOUR = 3600;
const DAY = 24 * HOUR;

/** Same as `cachyos_center_core::APP_ID` (window class and desktop file name). */
export const APP_ID = 'cachyos-center';

export const REPOSITORIES: RepositoryInfo[] = [
  { name: 'cachyos-v3', packageCount: 222 },
  { name: 'cachyos-core-v3', packageCount: 194 },
  { name: 'cachyos-extra-v3', packageCount: 7370 },
  { name: 'cachyos', packageCount: 829 },
  { name: 'core', packageCount: 299 },
  { name: 'extra', packageCount: 15005 },
  { name: 'multilib', packageCount: 182 },
];

export interface PackageDef {
  name: string;
  description: string;
  /** `null` for foreign (local/AUR) packages. */
  repository: string | null;
  version: string;
  installed: boolean;
  reason?: 'explicit' | 'dependency';
  size: number;
  download?: number;
  deps?: string[];
  optional?: string[];
  url?: string;
  licenses?: string[];
  groups?: string[];
}

const pkg = (
  name: string,
  repository: string | null,
  version: string,
  installed: boolean,
  reason: 'explicit' | 'dependency' | undefined,
  sizeMib: number,
  description: string,
  extra: Partial<PackageDef> = {},
): PackageDef => ({
  name,
  repository,
  version,
  installed,
  reason,
  size: Math.round(sizeMib * MIB),
  download: Math.round(sizeMib * 0.32 * MIB),
  description,
  ...extra,
});

export const PACKAGES: PackageDef[] = [
  pkg('base', 'core', '3-2', true, 'explicit', 0.01, 'Minimal package set to define a basic Arch Linux installation', {
    deps: ['bash', 'coreutils', 'glibc', 'pacman', 'systemd'],
    url: 'https://www.archlinux.org',
    licenses: ['GPL-3.0-or-later'],
  }),
  pkg('linux-cachyos', 'cachyos-core-v3', '7.2.8-1', true, 'explicit', 148, 'Linux kernel with the BORE scheduler and CachyOS patches', {
    deps: ['linux-firmware-amdgpu'],
    optional: ['linux-cachyos-headers: build modules'],
    url: 'https://github.com/CachyOS/linux-cachyos',
    licenses: ['GPL-2.0-only'],
  }),
  pkg('linux-cachyos-headers', 'cachyos-core-v3', '7.2.8-1', true, 'explicit', 38, 'Headers and scripts for building modules for the linux-cachyos kernel', {
    deps: ['linux-cachyos'],
    licenses: ['GPL-2.0-only'],
  }),
  pkg('linux-firmware-amdgpu', 'core', '20260910-1', true, 'dependency', 64, 'Firmware files for Linux – AMD GPUs and APUs', {
    licenses: ['LicenseRef-amdgpu'],
  }),
  pkg('amd-ucode', 'core', '20260910-1', true, 'explicit', 0.2, 'Microcode update image for AMD CPUs', {
    licenses: ['LicenseRef-amd-ucode'],
  }),
  pkg('glibc', 'cachyos-core-v3', '2.44+r50+g1848099f063e-3', true, 'dependency', 48, 'GNU C Library', {
    url: 'https://www.gnu.org/software/libc',
    licenses: ['GPL-2.0-or-later', 'LGPL-2.1-or-later'],
  }),
  pkg('systemd', 'cachyos-core-v3', '262-1', true, 'dependency', 32, 'System and service manager', {
    deps: ['systemd-libs', 'dbus-broker', 'glibc'],
    url: 'https://systemd.io',
    licenses: ['LGPL-2.1-or-later'],
  }),
  pkg('systemd-libs', 'cachyos-core-v3', '262-1', true, 'dependency', 4, 'systemd client libraries', { deps: ['glibc'] }),
  pkg('dbus-broker', 'cachyos-core-v3', '37-2', true, 'dependency', 0.6, 'Linux D-Bus message broker', { deps: ['systemd-libs'] }),
  pkg('bash', 'cachyos-core-v3', '5.3.3-2', true, 'dependency', 9, 'The GNU Bourne Again shell', { deps: ['glibc'] }),
  pkg('coreutils', 'cachyos-core-v3', '9.8-1', true, 'dependency', 17, 'The basic file, shell and text manipulation utilities of the GNU operating system', { deps: ['glibc'] }),
  pkg('sudo', 'cachyos-core-v3', '1.9.17.p2-1', true, 'explicit', 8, 'Give certain users the ability to run some commands as root', { deps: ['glibc'] }),
  pkg('pacman', 'cachyos', '7.1.0.r9.g54d9411-2', true, 'explicit', 5, 'A library-based package manager with dependency support', {
    deps: ['bash', 'glibc', 'archlinux-keyring', 'cachyos-keyring'],
    optional: ['pacman-contrib: checkupdates, paccache and pacdiff'],
    url: 'https://www.archlinux.org/pacman/',
    licenses: ['GPL-2.0-or-later'],
  }),
  pkg('pacman-contrib', 'extra', '1.13.0-1', true, 'explicit', 0.3, 'Contributed scripts and tools for pacman systems', { deps: ['pacman', 'bash'] }),
  pkg('archlinux-keyring', 'core', '20260901-1', true, 'dependency', 1.6, 'Arch Linux PGP keyring'),
  pkg('cachyos-keyring', 'cachyos', '3-1', true, 'explicit', 0.1, 'CachyOS PGP keyring'),
  pkg('cachyos-mirrorlist', 'cachyos', '22-1', true, 'explicit', 0.03, 'CachyOS mirrorlist'),
  pkg('cachyos-settings', 'cachyos', '1:1.3.5-1', true, 'explicit', 0.2, 'CachyOS system settings (sysctl, udev rules, zram)', { deps: ['systemd'] }),
  pkg('cachyos-hello', 'cachyos', '0.18.1-1', true, 'explicit', 6, 'Welcome screen for CachyOS', { deps: ['gtk3'] }),
  pkg('limine', 'cachyos', '10.2.1-1', true, 'explicit', 1.2, 'An advanced, portable, multiprotocol bootloader'),
  pkg('btrfs-progs', 'cachyos-core-v3', '6.17-1', true, 'explicit', 6, 'Btrfs filesystem utilities', { deps: ['glibc'] }),
  pkg('snapper', 'extra', '0.13.0-1', true, 'explicit', 7, 'A tool for managing BTRFS and LVM snapshots', { deps: ['btrfs-progs', 'systemd'] }),
  pkg('mesa', 'cachyos-extra-v3', '3:26.2.3-1', true, 'dependency', 92, 'Open-source OpenGL drivers', { deps: ['glibc'] }),
  pkg('vulkan-radeon', 'cachyos-extra-v3', '3:26.2.3-1', true, 'explicit', 34, 'Open-source Vulkan driver for AMD GPUs', { deps: ['mesa'] }),
  pkg('lib32-mesa', 'multilib', '3:26.2.3-1', true, 'dependency', 81, 'Open-source OpenGL drivers – 32-bit', { deps: ['mesa'] }),
  pkg('hyprland', 'cachyos-extra-v3', '0.56.2-3.1', true, 'explicit', 11, 'A highly customizable dynamic tiling Wayland compositor', {
    deps: ['mesa', 'systemd-libs', 'xdg-desktop-portal-hyprland'],
    url: 'https://github.com/hyprwm/Hyprland',
    licenses: ['BSD-3-Clause'],
  }),
  pkg('xdg-desktop-portal-hyprland', 'cachyos-extra-v3', '1.3.11-2.1', true, 'dependency', 1.1, 'xdg-desktop-portal backend for Hyprland'),
  pkg('hyprpolkitagent', 'extra', '0.1.3-1', true, 'explicit', 0.5, 'Polkit authentication agent for Hyprland', { deps: ['polkit'] }),
  pkg('polkit', 'cachyos-extra-v3', '127-1.1', true, 'dependency', 2, 'Application development toolkit for controlling system-wide privileges', { deps: ['glibc'] }),
  pkg('waybar', 'cachyos-extra-v3', '0.14.0-2.1', true, 'explicit', 3.5, 'Highly customizable Wayland bar for Sway and wlroots based compositors', { deps: ['gtk3'] }),
  pkg('kitty', 'cachyos-extra-v3', '0.44.0-1.1', true, 'explicit', 28, 'A modern, hackable, featureful, OpenGL-based terminal emulator', { deps: ['python'] }),
  pkg('fish', 'cachyos-extra-v3', '4.1.2-1.1', true, 'explicit', 22, 'Smart and user friendly shell intended mostly for interactive use'),
  pkg('firefox', 'cachyos-extra-v3', '157.0-1.1', true, 'explicit', 262, 'Fast, private and safe web browser', {
    deps: ['gtk3', 'dbus-broker'],
    url: 'https://www.mozilla.org/firefox/',
    licenses: ['MPL-2.0'],
  }),
  pkg('gtk3', 'cachyos-extra-v3', '1:3.24.51-1.1', true, 'dependency', 58, 'GObject-based multi-platform GUI toolkit', { deps: ['glibc'] }),
  pkg('networkmanager', 'cachyos-extra-v3', '1.56.0-1.1', true, 'explicit', 16, 'Network connection manager and user applications', { deps: ['systemd'] }),
  pkg('pipewire', 'cachyos-extra-v3', '1:1.6.0-1.1', true, 'dependency', 5, 'Low-latency audio/video router and processor', { deps: ['systemd-libs'] }),
  pkg('wireplumber', 'cachyos-extra-v3', '0.5.12-1.1', true, 'dependency', 2, 'Session and policy manager implementation for PipeWire', { deps: ['pipewire'] }),
  pkg('python', 'cachyos-core-v3', '3.14.2-1', true, 'dependency', 118, 'The Python programming language', { deps: ['glibc'] }),
  pkg('git', 'cachyos-extra-v3', '2.52.0-1.1', true, 'explicit', 30, 'The fast distributed version control system'),
  pkg('neovim', 'cachyos-extra-v3', '0.12.1-1.1', true, 'explicit', 30, 'Fork of Vim aiming to improve user experience, plugins and GUIs'),
  pkg('noto-fonts', 'extra', '1:2026.09.01-1', true, 'explicit', 110, 'Google Noto TTF fonts'),
  pkg('steam', 'multilib', '1.0.0.85-1', true, 'explicit', 5, "Valve's digital software delivery system", { deps: ['lib32-mesa'] }),
  // Foreign packages (installed, but not in any configured sync repository)
  pkg('yay-bin', null, '12.5.3-1', true, 'explicit', 9, 'Pacman wrapper and AUR helper written in Go (pre-compiled)', { deps: ['pacman', 'git'] }),
  pkg('visual-studio-code-bin', null, '1.106.0-1', true, 'explicit', 390, 'Visual Studio Code: editor for building and debugging modern web and cloud applications', { deps: ['gtk3'] }),
  pkg('hyprshot', null, '1.3.0-3', true, 'explicit', 0.02, 'Utility to easily take screenshots in Hyprland using the mouse', { deps: ['hyprland'] }),
  pkg('caelestia-shell', null, '1.4.1-1', true, 'explicit', 12, 'Desktop shell for Hyprland based on Quickshell', { deps: ['hyprland'] }),
  // Available in the repositories, not installed
  pkg('gimp', 'cachyos-extra-v3', '3.2.0-1.1', false, undefined, 132, 'GNU Image Manipulation Program', { deps: ['gtk3', 'babl', 'gegl'] }),
  pkg('babl', 'cachyos-extra-v3', '0.1.116-1.1', false, undefined, 5, 'Dynamic, any to any, pixel format conversion library'),
  pkg('gegl', 'cachyos-extra-v3', '0.4.64-1.1', false, undefined, 18, 'Graph based image processing framework', { deps: ['babl'] }),
  pkg('inkscape', 'cachyos-extra-v3', '1.4.2-3.1', false, undefined, 180, 'Professional vector graphics editor', { deps: ['gtk3'] }),
  pkg('obs-studio', 'cachyos-extra-v3', '32.0.2-1.1', false, undefined, 38, 'Free, open source software for live streaming and recording'),
  pkg('blender', 'cachyos-extra-v3', '17:5.0.0-1.1', false, undefined, 480, 'A fully integrated 3D graphics creation suite'),
  pkg('kdenlive', 'extra', '25.08.2-1', false, undefined, 44, 'A non-linear video editor using the MLT video framework'),
  pkg('libreoffice-fresh', 'extra', '26.2.1-1', false, undefined, 410, 'LibreOffice branch which contains new features and program enhancements'),
  pkg('thunderbird', 'cachyos-extra-v3', '145.0-1.1', false, undefined, 230, 'Standalone mail and news reader from mozilla.org', { deps: ['gtk3'] }),
  pkg('vlc', 'extra', '3.0.21-15', false, undefined, 60, 'Multi-platform MPEG, VCD/DVD and DivX player'),
  pkg('btop', 'cachyos-extra-v3', '1.4.5-1.1', false, undefined, 2, 'A monitor of system resources'),
  pkg('htop', 'cachyos-extra-v3', '3.4.1-1.1', false, undefined, 0.6, 'Interactive process viewer'),
  pkg('fastfetch', 'cachyos-extra-v3', '2.55.0-1.1', false, undefined, 1.8, 'A feature-rich and performance oriented system information tool'),
  pkg('keepassxc', 'extra', '2.7.10-3', false, undefined, 20, 'Cross-platform community-driven port of KeePass'),
  pkg('lutris', 'extra', '0.5.19-2', false, undefined, 14, 'Open gaming platform for managing games in a single interface', { deps: ['python'] }),
  pkg('gamemode', 'extra', '1.8.2-1', false, undefined, 0.4, 'A daemon/lib combo that allows games to request a set of optimisations'),
  pkg('mangohud', 'cachyos-extra-v3', '0.8.2-1.1', false, undefined, 3, 'A Vulkan overlay layer for monitoring FPS, temperatures, CPU/GPU load'),
  pkg('cachyos-gaming-meta', 'cachyos', '1-5', false, undefined, 0.01, 'Meta package for gaming on CachyOS', { deps: ['steam', 'gamemode', 'mangohud', 'lutris'] }),
  pkg('docker', 'cachyos-extra-v3', '1:28.5.1-1.1', false, undefined, 95, 'Pack, ship and run any application as a lightweight container'),
  pkg('rustup', 'extra', '1.28.2-3', false, undefined, 9, 'The Rust toolchain installer'),
  pkg('code', 'extra', '1.106.0-1', false, undefined, 330, 'The open source build of Visual Studio Code (vscode) editor'),
];

export const FOREIGN = PACKAGES.filter((p) => p.repository === null);

/** Update definitions: name → new version, repository and flags. */
export interface UpdateDef {
  name: string;
  repository: string;
  oldVersion: string;
  newVersion: string;
  download: number | null;
  flags: UpdateFlag[];
}

export const UPDATE_DEFS: UpdateDef[] = [
  { name: 'linux-cachyos', repository: 'cachyos-core-v3', oldVersion: '7.2.8-1', newVersion: '7.2.9-1', download: 147 * MIB, flags: ['kernel', 'rebootRecommended'] },
  { name: 'linux-cachyos-headers', repository: 'cachyos-core-v3', oldVersion: '7.2.8-1', newVersion: '7.2.9-1', download: 38 * MIB, flags: [] },
  { name: 'linux-firmware-amdgpu', repository: 'core', oldVersion: '20260910-1', newVersion: '20260925-1', download: 22 * MIB, flags: ['firmware', 'rebootRecommended'] },
  { name: 'amd-ucode', repository: 'core', oldVersion: '20260910-1', newVersion: '20260925-1', download: 0.2 * MIB, flags: ['microcode', 'rebootRecommended'] },
  { name: 'mesa', repository: 'cachyos-extra-v3', oldVersion: '3:26.2.3-1', newVersion: '3:26.2.4-1', download: 32 * MIB, flags: ['driver', 'rebootRecommended'] },
  { name: 'vulkan-radeon', repository: 'cachyos-extra-v3', oldVersion: '3:26.2.3-1', newVersion: '3:26.2.4-1', download: 11 * MIB, flags: ['driver', 'rebootRecommended'] },
  { name: 'lib32-mesa', repository: 'multilib', oldVersion: '3:26.2.3-1', newVersion: '3:26.2.4-1', download: 27 * MIB, flags: ['driver', 'rebootRecommended'] },
  { name: 'systemd', repository: 'cachyos-core-v3', oldVersion: '262-1', newVersion: '262.1-1', download: 9 * MIB, flags: ['coreSystem', 'rebootRecommended'] },
  { name: 'systemd-libs', repository: 'cachyos-core-v3', oldVersion: '262-1', newVersion: '262.1-1', download: 1.4 * MIB, flags: ['coreSystem', 'rebootRecommended'] },
  { name: 'hyprland', repository: 'cachyos-extra-v3', oldVersion: '0.56.2-3.1', newVersion: '0.56.3-1.1', download: 3.2 * MIB, flags: ['desktopSession'] },
  { name: 'cachyos-keyring', repository: 'cachyos', oldVersion: '3-1', newVersion: '4-1', download: null, flags: ['packageManager'] },
  { name: 'firefox', repository: 'cachyos-extra-v3', oldVersion: '157.0-1.1', newVersion: '157.0.1-1.1', download: 78 * MIB, flags: [] },
];

/** Kernel packages held back by `offline.conf` (scenario `offlineConfHeld`). */
export const HELD_BACK_NAMES = ['linux-cachyos', 'linux-cachyos-headers'];

export function toCandidate(def: UpdateDef, heldBack = false): UpdateCandidate {
  return {
    packageId: { repository: def.repository, name: def.name, architecture: 'x86_64' },
    oldVersion: def.oldVersion,
    newVersion: def.newVersion,
    downloadSize: def.download === null ? null : Math.round(def.download),
    flags: heldBack ? [...def.flags, 'heldBack'] : def.flags,
  };
}

export function toPlanEntry(def: UpdateDef): PlanEntry {
  return {
    action: 'upgrade',
    name: def.name,
    repository: def.repository,
    oldVersion: def.oldVersion,
    newVersion: def.newVersion,
    downloadSize: def.download === null ? null : Math.round(def.download),
    requested: false,
    flags: def.flags,
  };
}

/** Deterministic pseudo digest (64 lower-case hex chars) for mock plans. */
export function mockDigest(seed: string): string {
  let h1 = 0x811c9dc5;
  let out = '';
  for (let round = 0; out.length < 64; round += 1) {
    for (let i = 0; i < seed.length; i += 1) {
      h1 ^= seed.charCodeAt(i) + round;
      h1 = Math.imul(h1, 0x01000193) >>> 0;
    }
    out += h1.toString(16).padStart(8, '0');
  }
  return out.slice(0, 64);
}

export function sealPlan(plan: Omit<TransactionPlan, 'digest'>): TransactionPlan {
  const canonical = plan.entries
    .map((e) => `${e.action}|${e.repository ?? ''}|${e.name}|${e.oldVersion ?? ''}|${e.newVersion ?? ''}`)
    .sort()
    .join('\n');
  return { ...plan, digest: mockDigest(`${plan.kind}|${plan.recursive ? 'recursive' : 'plain'}\n${canonical}`) };
}

export function buildUpgradePlan(defs: readonly UpdateDef[], computedAt: number, heldBack: readonly string[] = []): TransactionPlan {
  const entries = defs.map(toPlanEntry);
  const reboot = defs.filter((d) => d.flags.includes('rebootRecommended')).map((d) => d.name);
  const warnings: TransactionPlan['warnings'] = [];
  if (heldBack.length > 0) warnings.push({ kind: 'heldBackPackages', packages: [...heldBack] });
  if (reboot.length > 0) warnings.push({ kind: 'rebootRecommended', packages: reboot });
  return sealPlan({
    kind: 'systemUpgrade',
    targets: [],
    entries,
    downloadSize: entries.reduce((sum, e) => sum + (e.downloadSize ?? 0), 0),
    installSizeDelta: Math.round(14.6 * MIB),
    source: 'isolatedCheckDb',
    computedAt,
    warnings,
    recursive: false,
  });
}

export function toSummary(def: PackageDef, updates: readonly UpdateDef[], ignored: readonly string[] = []): PackageSummary {
  const update = updates.find((u) => u.name === def.name);
  return {
    name: def.name,
    description: def.description,
    repository: def.repository,
    origin: def.repository === null ? 'localOrAur' : 'repo',
    architecture: def.name === 'noto-fonts' || def.name === 'base' ? 'any' : 'x86_64',
    installedVersion: def.installed ? def.version : null,
    availableVersion: def.repository === null ? null : update ? update.newVersion : def.version,
    installReason: def.installed ? (def.reason ?? 'explicit') : null,
    installedSize: def.size,
    updateAvailable: def.installed && update !== undefined,
    ignored: ignored.includes(def.name),
  };
}

/** Classification like `classify::is_critical` of the Rust core. */
const CRITICAL = new Set([
  'base', 'filesystem', 'glibc', 'systemd', 'systemd-libs', 'pacman', 'bash', 'coreutils', 'sudo',
  'linux-firmware', 'limine', 'archlinux-keyring', 'cachyos-keyring', 'cachyos-mirrorlist',
  'networkmanager', 'dbus', 'dbus-broker', 'linux-cachyos', 'amd-ucode', 'hyprland',
]);

export function isCritical(name: string): boolean {
  return CRITICAL.has(name);
}

export function requiredBy(name: string, installed: readonly PackageDef[]): string[] {
  return installed.filter((p) => p.deps?.includes(name)).map((p) => p.name).sort();
}

export function buildRecord(
  def: PackageDef,
  installed: readonly PackageDef[],
  updates: readonly UpdateDef[],
  now: number,
): PackageRecord {
  const summary = toSummary(def, updates);
  const index = PACKAGES.indexOf(def);
  const arch = def.repository?.startsWith('cachyos');
  return {
    id: { repository: def.repository ?? 'local', name: def.name, architecture: summary.architecture },
    description: def.description,
    origin: summary.origin,
    installedVersion: summary.installedVersion,
    availableVersion: summary.availableVersion,
    installReason: summary.installReason,
    url: def.url ?? (def.repository === null ? null : `https://archlinux.org/packages/?q=${def.name}`),
    licenses: def.licenses ?? ['GPL-2.0-or-later'],
    groups: def.groups ?? [],
    dependencies: def.deps ?? (def.repository === null ? [] : ['glibc']),
    optionalDependencies: def.optional ?? [],
    requiredBy: def.installed ? requiredBy(def.name, installed) : [],
    optionalFor: [],
    provides: [],
    conflicts: [],
    replaces: [],
    installedSize: def.size,
    downloadSize: def.installed && !summary.updateAvailable ? null : (def.download ?? null),
    packager: def.repository === null ? 'Unknown Packager' : arch ? 'CachyOS Build System' : 'Arch Linux Build System',
    buildDate: now - (4 + (index % 20)) * DAY,
    installDate: def.installed ? now - (2 + (index % 40)) * DAY : null,
    critical: isCritical(def.name),
    ignored: false,
  };
}

export function buildSystemInfo(now: number, lastFullUpgrade: number | null): SystemInfo {
  return {
    os: { id: 'cachyos', name: 'CachyOS Linux', prettyName: 'CachyOS', buildId: 'rolling', isCachyos: true, isArchBased: true },
    kernel: { release: '7.2.8-1-cachyos', package: 'linux-cachyos', uptimeSeconds: 2 * DAY + 5 * HOUR + 17 * 60, modulesMissing: false },
    cpu: { model: 'AMD Ryzen 7 7800X3D 8-Core Processor', vendor: 'AuthenticAMD', cores: 8, threads: 16, isaLevel: 'x86-64-v4' },
    gpus: [{ vendor: 'AMD', model: 'Navi 32 [Radeon RX 7800 XT]', driver: 'amdgpu', pciId: '1002:747e' }],
    memory: { totalBytes: 32 * GIB, availableBytes: Math.round(21.4 * GIB), swapTotalBytes: 32 * GIB, swapFreeBytes: 32 * GIB },
    disks: [
      { mountPoint: '/', filesystem: 'btrfs', totalBytes: 930 * GIB, availableBytes: Math.round(412.7 * GIB) },
      { mountPoint: '/boot', filesystem: 'vfat', totalBytes: 2 * GIB, availableBytes: Math.round(1.6 * GIB) },
      { mountPoint: '/var/cache/pacman/pkg', filesystem: 'btrfs', totalBytes: 930 * GIB, availableBytes: Math.round(412.7 * GIB) },
    ],
    session: { kind: 'hyprland', desktop: 'Hyprland', sessionType: 'wayland' },
    pacman: {
      pacmanVersion: '7.1.0',
      backend: { state: 'ready', libalpmVersion: '16.0.1', builtAgainst: '16.0.1' },
      lock: { state: 'free' },
      lastFullUpgrade,
      installedCount: 0,
      foreignCount: 0,
      repositories: REPOSITORIES.map((r) => r.name),
    },
    collectedAt: now,
  };
}

export function buildHyprland(now: number, workspace = 2): HyprlandInfo {
  return {
    available: true,
    reason: null,
    version: '0.56.2',
    monitors: [
      { name: 'DP-1', description: 'Dell Inc. DELL S2721DGF', width: 2560, height: 1440, refreshRate: 165, scale: 1, focused: true, activeWorkspace: String(workspace) },
      { name: 'HDMI-A-1', description: 'LG Electronics LG HDR 4K', width: 3840, height: 2160, refreshRate: 60, scale: 1.5, focused: false, activeWorkspace: '6' },
    ],
    activeWorkspace: String(workspace),
    workspaceCount: 6,
    // Verified on Hyprland: the window class of the desktop app is `cachyos-center`.
    appClass: 'cachyos-center',
    collectedAt: now,
  };
}

export function defaultSettings(): Settings {
  return {
    theme: 'system',
    language: 'system',
    density: 'comfortable',
    notifications: true,
    checkIntervalHours: 6,
    logRetentionDays: 90,
    newsEnabled: true,
    mcpEnabled: false,
  };
}

export function buildNewsItems(now: number, unread: number): NewsItem[] {
  const items: NewsItem[] = [
    {
      source: 'cachyOs',
      title: 'CachyOS September 2026 release',
      link: 'https://cachyos.org/blog/2609-september-release/',
      publishedAt: now - 2 * DAY,
      unread: false,
    },
    {
      source: 'archLinux',
      title: 'linux-firmware split packages require manual intervention',
      link: 'https://archlinux.org/news/linux-firmware-split-packages-require-manual-intervention/',
      publishedAt: now - 4 * DAY,
      unread: false,
    },
    {
      source: 'archLinux',
      title: 'Changes to the default mirrorlist handling',
      link: 'https://archlinux.org/news/changes-to-the-default-mirrorlist-handling/',
      publishedAt: now - 17 * DAY,
      unread: false,
    },
  ];
  return items.map((item, index) => ({ ...item, unread: index < unread }));
}

export function buildActivity(now: number): HistoryEntry[] {
  const entry = (partial: Partial<HistoryEntry> & Pick<HistoryEntry, 'id' | 'source' | 'startedAt'>): HistoryEntry => ({
    kind: null,
    origin: null,
    state: null,
    logOutcome: null,
    endedAt: partial.startedAt + 240,
    summary: '',
    errorCode: null,
    installed: 0,
    upgraded: 0,
    removed: 0,
    downgraded: 0,
    updatesFound: null,
    packages: [],
    outcomeUnknown: false,
    ...partial,
  });
  return [
    entry({
      id: 'b6c1f0e2-3a4d-4f5e-8a9b-0c1d2e3f4a5b',
      source: 'app',
      kind: 'systemUpgrade',
      origin: 'user',
      state: 'succeeded',
      startedAt: now - 2 * DAY - 3 * HOUR,
      summary: 'system upgrade completed',
      installed: 2,
      upgraded: 34,
      packages: ['cachyos-settings', 'firefox', 'glibc', 'kitty', 'mesa', 'noto-fonts', 'pipewire', 'python', 'qt6-base', 'wireplumber'],
    }),
    entry({
      id: 'pacman-log-1727600000',
      source: 'externalPacman',
      logOutcome: 'completed',
      startedAt: now - 4 * DAY,
      endedAt: now - 4 * DAY + 35,
      summary: 'pacman transaction',
      installed: 1,
      packages: ['btop'],
    }),
    entry({
      id: 'a2d4e6f8-1b3c-4d5e-9f0a-1b2c3d4e5f60',
      source: 'timer',
      kind: 'updateCheck',
      origin: 'timer',
      state: 'succeeded',
      startedAt: now - 6 * DAY,
      endedAt: now - 6 * DAY + 41,
      summary: 'update check: succeeded',
      updatesFound: 12,
    }),
    entry({
      id: 'c3e5a7b9-2c4d-4e6f-8a0b-2c3d4e5f6a71',
      source: 'app',
      kind: 'install',
      origin: 'user',
      state: 'succeeded',
      startedAt: now - 9 * DAY,
      summary: 'installed steam',
      installed: 3,
      upgraded: 21,
      packages: ['steam', 'lib32-mesa', 'lib32-systemd'],
    }),
    entry({
      id: 'd4f6b8c0-3d5e-4f70-9b1c-3d4e5f6a7b82',
      source: 'app',
      kind: 'systemUpgrade',
      origin: 'user',
      state: 'failed',
      startedAt: now - 15 * DAY,
      endedAt: now - 15 * DAY + 20,
      summary: 'failed to synchronize all databases (unable to resolve host)',
      errorCode: 'OFFLINE',
    }),
    entry({
      id: 'pacman-log-1726100000',
      source: 'externalPacman',
      logOutcome: 'unknown',
      startedAt: now - 20 * DAY,
      endedAt: null,
      summary: 'transaction started, no end marker found',
      upgraded: 3,
      packages: ['gtk3', 'gtk4', 'libadwaita'],
      outcomeUnknown: true,
    }),
  ];
}

export function defaultAutoUpdate(): AutoUpdateStatus {
  return {
    config: {
      policy: 'off',
      window: { weekdays: ['mon', 'tue', 'wed', 'thu', 'fri', 'sat', 'sun'], time: '12:00' },
      requireSnapshot: false,
      newsAcknowledgedUntil: null,
    },
    configError: null,
    timerEnabled: false,
    nextRun: null,
    lastRun: null,
    lastResult: null,
    preparedForNextReboot: false,
    offline: {
      installed: false,
      prepared: false,
      prepareTimerActive: false,
      rebootTimerActive: false,
      offlineConfIncluded: false,
      offlineConfIgnored: [],
      configurationVerifiable: true,
    },
    externalUpdaters: [
      {
        name: 'arch-update.timer',
        scope: 'user',
        active: false,
        description: 'Cachy-Update/Arch-Update: update checks and notifications',
      },
    ],
    prepareModeAvailable: false,
    prepareModeBlockers: [],
    helperAvailable: true,
  };
}

export const MCP_TOOLS = [
  'system_get_summary',
  'updates_list',
  'packages_search',
  'packages_installed',
  'operations_recent',
  'health_get',
];

export function mcpHostConfig(binaryPath: string): string {
  return JSON.stringify(
    { mcpServers: { 'cachyos-center': { type: 'stdio', command: binaryPath, args: [], env: {} } } },
    null,
    2,
  );
}
