import type { ReactNode } from 'react';
import { useState } from 'react';
import { Table, Button, Input, Toast } from '@douyinfe/semi-ui';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  rolesQueryOptions,
  useCreateRole,
  useUpdateRole,
  useDeleteRole,
} from '../api/roleQuery.ts';
import { useSessionStore } from '../stores/session.ts';
import Can from '../components/Can.tsx';
import type { components } from '@clsc/api-client';

type RoleDto = components['schemas']['RoleDto'];

export default function RolesPage(): ReactNode {
  const { t } = useTranslation('common');
  const tenant = useSessionStore((s) => s.tenant);
  const [search, setSearch] = useState('');
  const { data: roles = [] } = useQuery(
    rolesQueryOptions(tenant ?? undefined, { search }),
  );
  const create = useCreateRole(tenant ?? undefined);
  const update = useUpdateRole(tenant ?? undefined);
  const remove = useDeleteRole(tenant ?? undefined);

  const columns = [
    { title: t('name'), dataIndex: 'name' },
    {
      title: t('permissions'),
      render: (_text: string, record: RoleDto) => record.permissions.join(', '),
    },
    {
      title: t('actions'),
      render: (_text: string, record: RoleDto) => (
        <div style={{ display: 'flex', gap: 8 }}>
          <Can permission="tenant:role:write" capability="tenant:role:write">
            <Button
              size="small"
              onClick={() => {
                const name = window.prompt(t('newName'), record.name);
                const perms = window.prompt(
                  t('permissions'),
                  record.permissions.join(','),
                );
                if (name && perms !== null) {
                  update.mutate(
                    {
                      id: record.id,
                      name,
                      permissions: perms.split(',').map((p) => p.trim()),
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
          <Can permission="tenant:role:write" capability="tenant:role:write">
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
    <section aria-labelledby="roles-heading">
      <h1 id="roles-heading">{t('roles')}</h1>
      <div style={{ display: 'flex', gap: 8, marginBottom: 12 }}>
        <Input
          placeholder={t('search')}
          value={search}
          onChange={(v) => setSearch(v)}
          aria-label={t('search')}
        />
        <Can permission="tenant:role:write" capability="tenant:role:write">
          <Button
            onClick={() => {
              const name = window.prompt(t('name'));
              const perms = window.prompt(t('permissions'));
              if (name && perms !== null) {
                create.mutate(
                  { name, permissions: perms.split(',').map((p) => p.trim()) },
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
        dataSource={roles}
        columns={columns}
        rowKey="id"
        empty={t('noData')}
      />
    </section>
  );
}
