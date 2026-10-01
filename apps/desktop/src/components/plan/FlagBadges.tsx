import { Cpu, HardDrive, Monitor, Package, Pause, RotateCw, ShieldCheck, Wrench, Zap } from 'lucide-react';
import type { ReactNode } from 'react';

import type { UpdateFlag } from '../../bindings/UpdateFlag';
import { useI18n } from '../../i18n';
import { Badge, type BadgeTone } from '../Badge';

const FLAG_STYLE: Record<UpdateFlag, { tone: BadgeTone; icon: ReactNode }> = {
  kernel: { tone: 'accent', icon: <Cpu /> },
  driver: { tone: 'neutral', icon: <Monitor /> },
  firmware: { tone: 'neutral', icon: <HardDrive /> },
  microcode: { tone: 'neutral', icon: <Zap /> },
  coreSystem: { tone: 'neutral', icon: <Wrench /> },
  packageManager: { tone: 'neutral', icon: <Package /> },
  desktopSession: { tone: 'neutral', icon: <ShieldCheck /> },
  heldBack: { tone: 'warning', icon: <Pause /> },
  rebootRecommended: { tone: 'info', icon: <RotateCw /> },
};

/** Badges for the classification of an update (never color only: icon + text). */
export function FlagBadges({ flags }: { flags: readonly UpdateFlag[] }) {
  const { t } = useI18n();
  if (flags.length === 0) return null;
  return (
    <span className="badge-list">
      {flags.map((flag) => (
        <Badge key={flag} tone={FLAG_STYLE[flag].tone} icon={FLAG_STYLE[flag].icon}>
          {t(`flag.${flag}`)}
        </Badge>
      ))}
    </span>
  );
}
