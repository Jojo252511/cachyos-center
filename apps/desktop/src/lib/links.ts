/** Hosts `open_external` accepts (mirror of the backend allowlist). */
const ALLOWED_HOSTS = ['archlinux.org', 'wiki.archlinux.org', 'man.archlinux.org', 'cachyos.org', 'wiki.cachyos.org', 'github.com'];

/** `true` when the backend will open the URL; other links are only shown for copying. */
export function isOpenableUrl(url: string): boolean {
  const match = /^https:\/\/([^/:@\s]+)(\/\S*)?$/.exec(url);
  const host = match?.[1]?.toLowerCase();
  if (!host || !ALLOWED_HOSTS.includes(host)) return false;
  if (host === 'github.com') return (match?.[2] ?? '').toLowerCase().startsWith('/jojo252511/cachyos-center');
  return true;
}
