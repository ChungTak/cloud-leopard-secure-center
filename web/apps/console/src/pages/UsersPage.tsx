import type { ReactNode } from 'react';
import { useState } from 'react';
import { Table, Button, Input, Toast, Modal } from '@douyinfe/semi-ui';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  usersQueryOptions,
  useCreateUser,
  useUpdateUser,
  useChangeUserStatus,
  useSetUserPassword,
  useManageUserMfa,
  useCreateServiceAccount,
} from '../api/userQuery.ts';
import { useSessionStore } from '../stores/session.ts';
import Can from '../components/Can.tsx';
import type { components } from '@clsc/api-client';

type UserDto = components['schemas']['UserDto'];

export default function UsersPage(): ReactNode {
  const { t } = useTranslation('common');
  const tenant = useSessionStore((s) => s.tenant);
  const [search, setSearch] = useState('');
  const [createdKey, setCreatedKey] = useState<string | null>(null);
  const { data: users = [] } = useQuery(
    usersQueryOptions(tenant ?? undefined, { search }),
  );
  const create = useCreateUser(tenant ?? undefined);
  const update = useUpdateUser(tenant ?? undefined);
  const changeStatus = useChangeUserStatus(tenant ?? undefined);
  const setPassword = useSetUserPassword(tenant ?? undefined);
  const manageMfa = useManageUserMfa(tenant ?? undefined);
  const createServiceAccount = useCreateServiceAccount(tenant ?? undefined);

  const columns = [
    { title: t('username'), dataIndex: 'username' },
    { title: t('displayName'), dataIndex: 'displayName' },
    { title: t('status'), dataIndex: 'status' },
    {
      title: t('actions'),
      render: (_text: string, record: UserDto) => (
        <div style={{ display: 'flex', gap: 8 }}>
          <Can permission="tenant:user:write" capability="tenant:user:write">
            <Button
              size="small"
              onClick={() => {
                const displayName = window.prompt(
                  t('newDisplayName'),
                  record.displayName,
                );
                if (displayName) {
                  update.mutate(
                    {
                      id: record.id,
                      displayName,
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
          <Can permission="tenant:user:write" capability="tenant:user:write">
            <Button
              size="small"
              onClick={() => {
                const next = record.status === 'active' ? 'disabled' : 'active';
                changeStatus.mutate(
                  {
                    id: record.id,
                    status: next,
                    expectedRevision: record.revision,
                  },
                  { onError: (err) => Toast.error(String(err)) },
                );
              }}
            >
              {record.status === 'active' ? t('disable') : t('activate')}
            </Button>
          </Can>
          <Can permission="tenant:user:write" capability="tenant:user:write">
            <Button
              size="small"
              onClick={() => {
                const password = window.prompt(t('newPassword'));
                if (password) {
                  setPassword.mutate(
                    {
                      id: record.id,
                      password,
                      expectedRevision: record.revision,
                    },
                    { onError: (err) => Toast.error(String(err)) },
                  );
                }
              }}
            >
              {t('setPassword')}
            </Button>
          </Can>
          <Can permission="tenant:user:write" capability="tenant:user:write">
            <Button
              size="small"
              onClick={() => {
                manageMfa.mutate(
                  {
                    id: record.id,
                    enabled: record.status !== 'active',
                    expectedRevision: record.revision,
                  },
                  { onError: (err) => Toast.error(String(err)) },
                );
              }}
            >
              {t('toggleMfa')}
            </Button>
          </Can>
          <Can permission="tenant:user:write" capability="tenant:user:write">
            <Button
              size="small"
              onClick={() => {
                createServiceAccount.mutate(record.username, {
                  onSuccess: (data) => {
                    setCreatedKey(data.key);
                  },
                  onError: (err) => Toast.error(String(err)),
                });
              }}
            >
              {t('serviceAccount')}
            </Button>
          </Can>
        </div>
      ),
    },
  ];

  return (
    <section aria-labelledby="users-heading">
      <h1 id="users-heading">{t('users')}</h1>
      <div style={{ display: 'flex', gap: 8, marginBottom: 12 }}>
        <Input
          placeholder={t('search')}
          value={search}
          onChange={(v) => setSearch(v)}
          aria-label={t('search')}
        />
        <Can permission="tenant:user:write" capability="tenant:user:write">
          <Button
            onClick={() => {
              const username = window.prompt(t('username'));
              const displayName = window.prompt(t('displayName'));
              if (username && displayName) {
                create.mutate(
                  { username, displayName },
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
        dataSource={users}
        columns={columns}
        rowKey="id"
        empty={t('noData')}
      />
      {createdKey && (
        <Modal
          title={t('apiKeyCreated')}
          visible={Boolean(createdKey)}
          onOk={() => setCreatedKey(null)}
          onCancel={() => setCreatedKey(null)}
        >
          <p>{t('apiKeyShownOnce')}</p>
          <code data-testid="api-key-once">{createdKey}</code>
        </Modal>
      )}
    </section>
  );
}
