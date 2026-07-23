import type { ReactNode } from 'react';
import { useState } from 'react';
import { Table, Button, Input, Toast } from '@douyinfe/semi-ui';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  roleBindingsQueryOptions,
  useCreateRoleBinding,
  useUpdateRoleBinding,
  useDeleteRoleBinding,
  useExplainAuth,
} from '../api/roleBindingQuery.ts';
import { useSessionStore } from '../stores/session.ts';
import Can from '../components/Can.tsx';
import type { components } from '@clsc/api-client';

type RoleBindingDto = components['schemas']['RoleBindingDto'];

type Scope = components['schemas']['RoleBindingScopeDto'];

export default function RoleBindingsPage(): ReactNode {
  const { t } = useTranslation('common');
  const tenant = useSessionStore((s) => s.tenant);
  const [search, setSearch] = useState('');
  const { data: bindings = [] } = useQuery(
    roleBindingsQueryOptions(tenant ?? undefined, { search }),
  );
  const create = useCreateRoleBinding(tenant ?? undefined);
  const update = useUpdateRoleBinding(tenant ?? undefined);
  const remove = useDeleteRoleBinding(tenant ?? undefined);
  const explain = useExplainAuth();

  const columns = [
    { title: t('principalId'), dataIndex: 'principalId' },
    { title: t('roleId'), dataIndex: 'roleId' },
    {
      title: t('scope'),
      render: (_text: string, record: RoleBindingDto) =>
        JSON.stringify(record.scope),
    },
    { title: t('validFrom'), dataIndex: 'validFrom' },
    { title: t('validUntil'), dataIndex: 'validUntil' },
    {
      title: t('actions'),
      render: (_text: string, record: RoleBindingDto) => (
        <div style={{ display: 'flex', gap: 8 }}>
          <Can permission="tenant:role:write" capability="tenant:role:write">
            <Button
              size="small"
              onClick={() => {
                const roleId = window.prompt(t('roleId'), record.roleId);
                const scopeInput = window.prompt(
                  t('scope') + ' (tenant|organization|area|resource)',
                  JSON.stringify(record.scope),
                );
                if (roleId && scopeInput) {
                  let scope: Scope;
                  try {
                    scope = JSON.parse(scopeInput) as Scope;
                  } catch {
                    Toast.error(t('invalidScope'));
                    return;
                  }
                  update.mutate(
                    {
                      id: record.id,
                      roleId,
                      scope,
                      validFrom: record.validFrom,
                      validUntil: record.validUntil,
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
          <Button
            size="small"
            onClick={() => {
              explain.mutate(
                {
                  principalId: record.principalId,
                  action: 'tenant:resource:read',
                  resourceType: 'user',
                  resourceId: record.principalId,
                },
                {
                  onSuccess: (result) =>
                    Toast.info(`${result.decision}: ${result.reason}`),
                  onError: (err) => Toast.error(String(err)),
                },
              );
            }}
          >
            {t('preview')}
          </Button>
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
    <section aria-labelledby="role-bindings-heading">
      <h1 id="role-bindings-heading">{t('roleBindings')}</h1>
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
              const principalId = window.prompt(t('principalId'));
              const roleId = window.prompt(t('roleId'));
              const scopeInput = window.prompt(
                t('scope') + ' (tenant|organization|area|resource)',
              );
              if (principalId && roleId && scopeInput) {
                let scope: Scope;
                try {
                  scope = JSON.parse(scopeInput) as Scope;
                } catch {
                  Toast.error(t('invalidScope'));
                  return;
                }
                create.mutate(
                  {
                    principalId,
                    roleId,
                    scope,
                    validFrom: new Date().toISOString(),
                    validUntil: null,
                  },
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
