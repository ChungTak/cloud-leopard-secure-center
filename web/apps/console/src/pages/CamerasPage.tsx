import type { ReactNode } from 'react';
import { useState } from 'react';
import { Table, Button, Input, Toast } from '@douyinfe/semi-ui';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  camerasQueryOptions,
  useCreateCamera,
  useUpdateCamera,
  useDeleteCamera,
} from '../api/cameraQuery.ts';
import { useSessionStore } from '../stores/session.ts';
import Can from '../components/Can.tsx';
import type { components } from '@clsc/api-client';

type CameraDto = components['schemas']['CameraDto'];

const SENSITIVE_LEVELS = ['low', 'normal', 'high', 'critical'];

export default function CamerasPage(): ReactNode {
  const { t } = useTranslation('common');
  const tenant = useSessionStore((s) => s.tenant);
  const [search, setSearch] = useState('');
  const { data: cameras = [] } = useQuery(
    camerasQueryOptions(tenant ?? undefined, { search }),
  );
  const create = useCreateCamera(tenant ?? undefined);
  const update = useUpdateCamera(tenant ?? undefined);
  const remove = useDeleteCamera(tenant ?? undefined);

  const columns = [
    { title: t('code'), dataIndex: 'code' },
    { title: t('name'), dataIndex: 'name' },
    { title: t('sensitivity'), dataIndex: 'sensitivity' },
    { title: t('enabled'), dataIndex: 'isEnabled' },
    {
      title: t('actions'),
      render: (_text: string, record: CameraDto) => (
        <div style={{ display: 'flex', gap: 8 }}>
          <Can
            permission="tenant:device:write"
            capability="tenant:device:write"
          >
            <Button
              size="small"
              onClick={() => {
                const name = window.prompt(t('newName'), record.name);
                const sensitivity = window.prompt(
                  t('sensitivity') + ` (${SENSITIVE_LEVELS.join(',')})`,
                  record.sensitivity,
                );
                if (name !== null && sensitivity !== null) {
                  update.mutate(
                    {
                      id: record.id,
                      name,
                      sensitivity,
                      isEnabled: record.isEnabled,
                      expectedRevision: record.revision,
                    },
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
    <section aria-labelledby="cameras-heading">
      <h1 id="cameras-heading">{t('cameras')}</h1>
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
              const deviceId = window.prompt(t('deviceId'));
              const code = window.prompt(t('code'));
              const name = window.prompt(t('name'));
              const sensitivity = window.prompt(
                t('sensitivity') + ` (${SENSITIVE_LEVELS.join(',')})`,
              );
              if (deviceId && code && name && sensitivity) {
                create.mutate(
                  { deviceId, code, name, sensitivity },
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
        dataSource={cameras}
        columns={columns}
        rowKey="id"
        empty={t('noData')}
      />
    </section>
  );
}
