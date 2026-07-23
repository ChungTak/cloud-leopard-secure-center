import type { ReactNode } from 'react';
import { useState } from 'react';
import { Table, Input, Modal } from '@douyinfe/semi-ui';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  auditRecordsQueryOptions,
  auditRecordQueryOptions,
} from '../api/auditQuery.ts';
import { useSessionStore } from '../stores/session.ts';
import type { components } from '@clsc/api-client';

type AuditRecordDto = components['schemas']['AuditRecordDto'];

export default function AuditPage(): ReactNode {
  const { t } = useTranslation('common');
  const tenant = useSessionStore((s) => s.tenant);
  const [search, setSearch] = useState('');
  const [selectedId, setSelectedId] = useState<string | undefined>();
  const { data: records = [] } = useQuery(
    auditRecordsQueryOptions(tenant ?? undefined, { search }),
  );
  const { data: detail } = useQuery(
    auditRecordQueryOptions(tenant ?? undefined, selectedId),
  );

  const columns = [
    { title: t('actorType'), dataIndex: 'actorType' },
    { title: t('action'), dataIndex: 'action' },
    { title: t('targetType'), dataIndex: 'targetType' },
    { title: t('targetId'), dataIndex: 'targetId' },
    { title: t('occurredAt'), dataIndex: 'occurredAt' },
    {
      title: t('actions'),
      render: (_text: string, record: AuditRecordDto) => (
        <button
          type="button"
          onClick={() => setSelectedId(record.id ?? undefined)}
        >
          {t('view')}
        </button>
      ),
    },
  ];

  return (
    <section aria-labelledby="audit-heading">
      <h1 id="audit-heading">{t('audit')}</h1>
      <div style={{ display: 'flex', gap: 8, marginBottom: 12 }}>
        <Input
          placeholder={t('search')}
          value={search}
          onChange={(v) => setSearch(v)}
          aria-label={t('search')}
        />
      </div>
      <Table
        dataSource={records}
        columns={columns}
        rowKey="id"
        empty={t('noData')}
      />
      <Modal
        title={t('auditDetails')}
        visible={Boolean(selectedId)}
        onCancel={() => setSelectedId(undefined)}
        footer={null}
      >
        <pre style={{ whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}>
          {detail ? JSON.stringify(detail, null, 2) : ''}
        </pre>
      </Modal>
    </section>
  );
}
