import type { ReactNode } from 'react';
import { useState } from 'react';
import { Table, Input, Switch } from '@douyinfe/semi-ui';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { projectionsQueryOptions } from '../api/projectionQuery.ts';
import { useSessionStore } from '../stores/session.ts';

export default function ProjectionsPage(): ReactNode {
  const { t } = useTranslation('common');
  const tenant = useSessionStore((s) => s.tenant);
  const [deviceId, setDeviceId] = useState('');
  const [showStale, setShowStale] = useState(false);
  const { data: projections = [] } = useQuery(
    projectionsQueryOptions(tenant ?? undefined, {
      deviceId: deviceId || undefined,
      isStale: showStale,
    }),
  );

  const columns = [
    { title: t('channelId'), dataIndex: 'channelId' },
    { title: t('deviceId'), dataIndex: 'deviceId' },
    { title: t('onlineState'), dataIndex: 'isOnline' },
    { title: t('observedAt'), dataIndex: 'observedAt' },
    { title: t('isStale'), dataIndex: 'isStale' },
  ];

  return (
    <section aria-labelledby="projections-heading">
      <h1 id="projections-heading">{t('projections')}</h1>
      <div style={{ display: 'flex', gap: 8, marginBottom: 12 }}>
        <Input
          placeholder={t('deviceId')}
          value={deviceId}
          onChange={(v) => setDeviceId(v)}
          aria-label={t('deviceId')}
        />
        <Switch
          checked={showStale}
          onChange={(v) => setShowStale(Boolean(v))}
          aria-label={t('isStale')}
        >
          {t('isStale')}
        </Switch>
      </div>
      <Table
        dataSource={projections}
        columns={columns}
        rowKey="channelId"
        empty={t('noData')}
      />
    </section>
  );
}
