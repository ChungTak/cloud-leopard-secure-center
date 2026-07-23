import type { ReactNode } from 'react';
import { useState } from 'react';
import { Table, Button, Input, Toast } from '@douyinfe/semi-ui';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  externalBindingsQueryOptions,
  useCreateExternalBinding,
  useResolveExternalBindingConflict,
  useDeleteExternalBinding,
} from '../api/externalBindingQuery.ts';
import { useSessionStore } from '../stores/session.ts';
import Can from '../components/Can.tsx';
import type { components } from '@clsc/api-client';

type ExternalBindingDto = components['schemas']['ExternalBindingDto'];

export default function ExternalBindingsPage(): ReactNode {
  const { t } = useTranslation('common');
  const tenant = useSessionStore((s) => s.tenant);
  const [search, setSearch] = useState('');
  const { data: bindings = [] } = useQuery(
    externalBindingsQueryOptions(tenant ?? undefined, { search }),
  );
  const create = useCreateExternalBinding(tenant ?? undefined);
  const resolve = useResolveExternalBindingConflict(tenant ?? undefined);
  const remove = useDeleteExternalBinding(tenant ?? undefined);

  const columns = [
    { title: t('externalRef'), dataIndex: 'externalRef' },
    { title: t('externalKind'), dataIndex: 'externalKind' },
    { title: t('state'), dataIndex: 'state' },
    {
      title: t('actions'),
      render: (_text: string, record: ExternalBindingDto) => (
        <div style={{ display: 'flex', gap: 8 }}>
          {record.state === 'conflict' && (
            <Can
              permission="tenant:device:write"
              capability="tenant:device:write"
            >
              <Button
                size="small"
                onClick={() => {
                  if (window.confirm(t('confirmResolve'))) {
                    resolve.mutate(
                      {
                        id: record.id,
                        action: 'active',
                        expectedRevision: record.revision,
                      },
                      { onError: (err) => Toast.error(String(err)) },
                    );
                  }
                }}
              >
                {t('resolve')}
              </Button>
            </Can>
          )}
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
    <section aria-labelledby="external-bindings-heading">
      <h1 id="external-bindings-heading">{t('externalBindings')}</h1>
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
              const resourceType = window.prompt(t('resourceType'));
              const resourceId = window.prompt(t('resourceId'));
              const externalRef = window.prompt(t('externalRef'));
              const externalKind = window.prompt(t('externalKind'));
              if (resourceType && resourceId && externalRef && externalKind) {
                create.mutate(
                  { resourceType, resourceId, externalRef, externalKind },
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
        dataSource={bindings}
        columns={columns}
        rowKey="id"
        empty={t('noData')}
      />
    </section>
  );
}
