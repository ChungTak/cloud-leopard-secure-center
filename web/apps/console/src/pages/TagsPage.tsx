import type { ReactNode } from 'react';
import { useState } from 'react';
import { Table, Button, Input, Toast } from '@douyinfe/semi-ui';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  tagsQueryOptions,
  useCreateTag,
  useUpdateTag,
  useDeleteTag,
} from '../api/tagQuery.ts';
import { useSessionStore } from '../stores/session.ts';
import Can from '../components/Can.tsx';
import type { components } from '@clsc/api-client';

type TagDto = components['schemas']['TagDto'];

export default function TagsPage(): ReactNode {
  const { t } = useTranslation('common');
  const tenant = useSessionStore((s) => s.tenant);
  const [search, setSearch] = useState('');
  const { data: tags = [] } = useQuery(
    tagsQueryOptions(tenant ?? undefined, { search }),
  );
  const create = useCreateTag(tenant ?? undefined);
  const update = useUpdateTag(tenant ?? undefined);
  const remove = useDeleteTag(tenant ?? undefined);

  const columns = [
    { title: t('key'), dataIndex: 'key' },
    { title: t('value'), dataIndex: 'value' },
    { title: t('resourceType'), dataIndex: 'resourceType' },
    {
      title: t('actions'),
      render: (_text: string, record: TagDto) => (
        <div style={{ display: 'flex', gap: 8 }}>
          <Can
            permission="tenant:device:write"
            capability="tenant:device:write"
          >
            <Button
              size="small"
              onClick={() => {
                const value = window.prompt(t('value'), record.value);
                if (value !== null) {
                  update.mutate(
                    { id: record.id, value, expectedRevision: record.revision },
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
    <section aria-labelledby="tags-heading">
      <h1 id="tags-heading">{t('tags')}</h1>
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
              const key = window.prompt(t('key'));
              const value = window.prompt(t('value'));
              if (resourceType && resourceId && key && value) {
                create.mutate(
                  { resourceType, resourceId, key, value },
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
        dataSource={tags}
        columns={columns}
        rowKey="id"
        empty={t('noData')}
      />
    </section>
  );
}
