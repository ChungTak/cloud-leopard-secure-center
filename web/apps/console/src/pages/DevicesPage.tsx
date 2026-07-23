import type { ReactNode } from 'react';
import { useState } from 'react';
import { Table, Button, Input, Toast } from '@douyinfe/semi-ui';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  devicesQueryOptions,
  useCreateDevice,
  useUpdateDevice,
  useChangeDeviceLifecycle,
  useDeleteDevice,
} from '../api/deviceQuery.ts';
import { useSessionStore } from '../stores/session.ts';
import Can from '../components/Can.tsx';
import type { components } from '@clsc/api-client';

type DeviceDto = components['schemas']['DeviceDto'];

export default function DevicesPage(): ReactNode {
  const { t } = useTranslation('common');
  const tenant = useSessionStore((s) => s.tenant);
  const [search, setSearch] = useState('');
  const { data: devices = [] } = useQuery(
    devicesQueryOptions(tenant ?? undefined, { search }),
  );
  const create = useCreateDevice(tenant ?? undefined);
  const update = useUpdateDevice(tenant ?? undefined);
  const changeLifecycle = useChangeDeviceLifecycle(tenant ?? undefined);
  const remove = useDeleteDevice(tenant ?? undefined);

  const columns = [
    { title: t('code'), dataIndex: 'code' },
    { title: t('name'), dataIndex: 'name' },
    { title: t('lifecycle'), dataIndex: 'lifecycle' },
    { title: t('onlineState'), dataIndex: 'onlineState' },
    {
      title: t('actions'),
      render: (_text: string, record: DeviceDto) => (
        <div style={{ display: 'flex', gap: 8 }}>
          <Can
            permission="tenant:device:write"
            capability="tenant:device:write"
          >
            <Button
              size="small"
              onClick={() => {
                const name = window.prompt(t('newName'), record.name);
                if (name !== null) {
                  update.mutate(
                    { id: record.id, name, expectedRevision: record.revision },
                    { onError: (err) => Toast.error(String(err)) },
                  );
                }
              }}
            >
              {t('edit')}
            </Button>
          </Can>
          <Can
            permission="tenant:device:write"
            capability="tenant:device:write"
          >
            <Button
              size="small"
              onClick={() => {
                const next =
                  record.lifecycle === 'active' ? 'retired' : 'active';
                changeLifecycle.mutate(
                  {
                    id: record.id,
                    lifecycle: next,
                    expectedRevision: record.revision,
                  },
                  { onError: (err) => Toast.error(String(err)) },
                );
              }}
            >
              {record.lifecycle === 'active' ? t('retire') : t('activate')}
            </Button>
          </Can>
          <Can
            permission="tenant:device:write"
            capability="tenant:device:write"
          >
            <Button
              type="danger"
              size="small"
              onClick={() => {
                if (window.confirm(t('confirmDelete'))) {
                  remove.mutate(record.id, {
                    onError: (err) => Toast.error(String(err)),
                  });
                }
              }}
            >
              {t('delete')}
            </Button>
          </Can>
        </div>
      ),
    },
  ];

  return (
    <section aria-labelledby="devices-heading">
      <h1 id="devices-heading">{t('devices')}</h1>
      <div style={{ display: 'flex', gap: 8, marginBottom: 12 }}>
        <Input
          placeholder={t('search')}
          value={search}
          onChange={(v) => setSearch(v)}
          aria-label={t('search')}
        />
        <Can permission="tenant:device:write" capability="tenant:device:write">
          <Button
            onClick={() => {
              const code = window.prompt(t('code'));
              const name = window.prompt(t('name'));
              if (code && name) {
                create.mutate(
                  { code, name },
                  { onError: (err) => Toast.error(String(err)) },
                );
              }
            }}
          >
            {t('create')}
          </Button>
        </Can>
      </div>
      <Table
        dataSource={devices}
        columns={columns}
        rowKey="id"
        empty={t('noData')}
      />
    </section>
  );
}
