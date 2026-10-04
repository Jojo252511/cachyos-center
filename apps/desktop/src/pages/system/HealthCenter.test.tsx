import { screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { api } from '../../api/client';
import { createFormatters } from '../../i18n/format';
import { mockBackend, renderI18n } from '../../test/utils';
import { HealthCenter } from './HealthCenter';

describe('HealthCenter', () => {
  it('says what paccache -r would free, or that it finds nothing', async () => {
    mockBackend();
    const report = await api.getHealth();
    const reclaimable = report.packageCacheReclaimableBytes ?? 0;
    const { unmount } = renderI18n(<HealthCenter report={report} />);
    const size = createFormatters('de-DE').bytes(reclaimable);
    expect(screen.getByText(`paccache -r würde ${size} alter Paketversionen freigeben.`)).toBeInTheDocument();
    expect(screen.getByText('Alte Paketversionen im Cache')).toBeInTheDocument();
    unmount();
    renderI18n(<HealthCenter report={{ ...report, packageCacheReclaimableBytes: 0, items: report.items.filter((i) => i.kind !== 'packageCacheLarge') }} />);
    expect(screen.getByText('paccache -r findet nichts zum Aufräumen: Der Cache enthält höchstens drei Versionen je Paket.')).toBeInTheDocument();
    expect(screen.queryByText('Alte Paketversionen im Cache')).not.toBeInTheDocument();
  });
});
