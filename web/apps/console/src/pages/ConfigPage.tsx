import type { ReactNode } from 'react';
import { useState, useMemo } from 'react';
import { Table, Input, Button, Toast } from '@douyinfe/semi-ui';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  configValuesQueryOptions,
  configDefinitionsQueryOptions,
  useUpdateConfigValue,
} from '../api/configQuery.ts';
import { useSessionStore } from '../stores/session.ts';
import Can from '../components/Can.tsx';
import type { components } from '@clsc/api-client';

type ConfigValueDto = components['schemas']['ConfigValueDto'];
type ConfigDefinitionDto = components['schemas']['ConfigDefinitionDto'];

export default function ConfigPage(): ReactNode {
  const { t } = useTranslation('common');
  const tenant = useSessionStore((s) => s.tenant);
  const [search, setSearch] = useState('');
  const { data: values = [] } = useQuery(
    configValuesQueryOptions(tenant ?? undefined, { search }),
  );
  const { data: definitions = [] } = useQuery(
    configDefinitionsQueryOptions(tenant ?? undefined, { search }),
  );
  const update = useUpdateConfigValue(tenant ?? undefined);

  const defsByKey = useMemo(() => {
    const map = new Map<string, ConfigDefinitionDto>();
    for (const d of definitions) {
      map.set(d.configKey, d);
    }
    return map;
  }, [definitions]);

  const columns = [
    { title: t('configKey'), dataIndex: 'configKey' },
    { title: t('value'), dataIndex: 'value' },
    {
      title: t('secret'),
      render: (_text: string, record: ConfigValueDto) =>
        record.secretRef ? t('redacted') : t('none'),
    },
    {
      title: t('actions'),
      render: (_text: string, record: ConfigValueDto) => {
        const def = defsByKey.get(record.configKey);
        return (
          <div style={{ display: 'flex', gap: 8 }}>
            {def?.sensitive ? (
              <Can
                permission="tenant:config:write"
                capability="tenant:config:write"
              >
                <Button
                  size="small"
                  onClick={() => {
                    const next = window.prompt(t('newValue'));
                    if (next !== null) {
                      update.mutate(
                        {
                          id: record.id ?? '',
                          value: next,
                          clearSecret: false,
                          expectedRevision: record.revision,
                        },
                        { onError: (err) => Toast.error(String(err)) },
                      );
                    }
                  }}
                >
                  {t('replaceSecret')}
                </Button>
                <Button
                  size="small"
                  onClick={() => {
                    if (window.confirm(t('confirmClearSecret'))) {
                      update.mutate(
                        {
                          id: record.id ?? '',
                          clearSecret: true,
                          expectedRevision: record.revision,
                        },
                        { onError: (err) => Toast.error(String(err)) },
                      );
                    }
                  }}
                >
                  {t('clearSecret')}
                </Button>
              </Can>
            ) : (
              <Can
                permission="tenant:config:write"
                capability="tenant:config:write"
              >
                <Button
                  size="small"
                  onClick={() => {
                    const next = window.prompt(t('newValue'), record.value);
                    if (next !== null) {
                      update.mutate(
                        {
                          id: record.id ?? '',
                          value: next,
                          clearSecret: false,
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
            )}
          </div>
        );
      },
    },
  ];

  return (
    <section aria-labelledby="config-heading">
      <h1 id="config-heading">{t('config')}</h1>
      <div style={{ display: 'flex', gap: 8, marginBottom: 12 }}>
        <Input
          placeholder={t('search')}
          value={search}
          onChange={(v) => setSearch(v)}
          aria-label={t('search')}
        />
      </div>
      <Table
        dataSource={values}
        columns={columns}
        rowKey="id"
        empty={t('noData')}
      />
    </section>
  );
}
