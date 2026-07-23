import type { ReactNode } from 'react';
import { useState } from 'react';
import { Tree, Button, Input, Toast } from '@douyinfe/semi-ui';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  organizationUnitsQueryOptions,
  useCreateOrganizationUnit,
  useMoveOrganizationUnit,
  useDeleteOrganizationUnit,
} from '../api/organizationQuery.ts';
import { useSessionStore } from '../stores/session.ts';
import Can from '../components/Can.tsx';
import type { components } from '@clsc/api-client';

type OrganizationUnitDto = components['schemas']['OrganizationUnitDto'];

interface TreeNode {
  key: string;
  label: string;
  value: string;
  icon?: ReactNode;
  children?: TreeNode[];
  isLeaf?: boolean;
}

function buildTree(units: OrganizationUnitDto[]): TreeNode[] {
  const nodes = new Map<string, TreeNode>();
  const children = new Map<string, TreeNode[]>();

  for (const u of units) {
    nodes.set(u.id, {
      key: u.id,
      label: `${u.name} (${u.code})`,
      value: u.id,
      isLeaf: true,
    });
  }

  const roots: TreeNode[] = [];
  for (const u of units) {
    const node = nodes.get(u.id)!;
    if (u.parentId) {
      const list = children.get(u.parentId) ?? [];
      list.push(node);
      children.set(u.parentId, list);
    } else {
      roots.push(node);
    }
  }

  for (const [id, node] of nodes) {
    const list = children.get(id);
    if (list && list.length > 0) {
      node.children = list;
      node.isLeaf = false;
    }
  }

  return roots;
}

export default function OrganizationPage(): ReactNode {
  const { t } = useTranslation('common');
  const tenant = useSessionStore((s) => s.tenant);
  const [search, setSearch] = useState('');
  const [selected, setSelected] = useState<string | undefined>();
  const { data: units = [] } = useQuery(
    organizationUnitsQueryOptions(tenant ?? undefined, { search }),
  );
  const create = useCreateOrganizationUnit(tenant ?? undefined);
  const move = useMoveOrganizationUnit(tenant ?? undefined);
  const remove = useDeleteOrganizationUnit(tenant ?? undefined);

  const treeData = buildTree(units);

  return (
    <section aria-labelledby="organization-heading">
      <h1 id="organization-heading">{t('organizations')}</h1>
      <div style={{ display: 'flex', gap: 8, marginBottom: 12 }}>
        <Input
          placeholder={t('search')}
          value={search}
          onChange={(v) => setSearch(v)}
          aria-label={t('search')}
        />
        <Can
          permission="tenant:organization:write"
          capability="tenant:organization:write"
        >
          <Button
            onClick={() => {
              create.mutate(
                { code: 'new', name: t('newOrganizationUnit') },
                {
                  onError: (err) => Toast.error(String(err)),
                },
              );
            }}
          >
            {t('create')}
          </Button>
        </Can>
      </div>
      <Tree
        treeData={treeData}
        emptyContent={t('noData')}
        onSelect={(key) => setSelected(key)}
        renderLabel={(label) => <span data-testid="org-label">{label}</span>}
      />
      {selected && (
        <div style={{ marginTop: 12, display: 'flex', gap: 8 }}>
          <Can
            permission="tenant:organization:write"
            capability="tenant:organization:write"
          >
            <Button
              onClick={() => {
                if (!selected) return;
                move.mutate(
                  { id: selected, parentId: null, expectedRevision: 1 },
                  { onError: (err) => Toast.error(String(err)) },
                );
              }}
            >
              {t('moveToRoot')}
            </Button>
          </Can>
          <Can
            permission="tenant:organization:write"
            capability="tenant:organization:write"
          >
            <Button
              type="danger"
              onClick={() => {
                if (!selected) return;
                remove.mutate(selected, {
                  onError: (err) => Toast.error(String(err)),
                });
              }}
            >
              {t('delete')}
            </Button>
          </Can>
        </div>
      )}
    </section>
  );
}
