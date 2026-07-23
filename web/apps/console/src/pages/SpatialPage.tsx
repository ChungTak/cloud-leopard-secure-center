import type { ReactNode } from 'react';
import { useState } from 'react';
import { Tree, Button, Input, Toast } from '@douyinfe/semi-ui';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  spatialNodesQueryOptions,
  useCreateSpatialNode,
  useMoveSpatialNode,
  useDeleteSpatialNode,
  type SpatialNodeType,
} from '../api/spatialQuery.ts';
import { useSessionStore } from '../stores/session.ts';
import Can from '../components/Can.tsx';
import type { components } from '@clsc/api-client';

type SpatialNodeDto = components['schemas']['SpatialNodeDto'];

interface TreeNode {
  key: string;
  label: string;
  value: string;
  icon?: ReactNode;
  children?: TreeNode[];
  isLeaf?: boolean;
}

const TYPE_ICON: Record<SpatialNodeType, string> = {
  site: '🏢',
  building: '🏠',
  floor: '📶',
  area: '📍',
};

function buildTree(nodes: SpatialNodeDto[]): TreeNode[] {
  const byId = new Map<string, TreeNode>();
  const children = new Map<string, TreeNode[]>();

  for (const n of nodes) {
    byId.set(n.id, {
      key: n.id,
      label: `${TYPE_ICON[n.nodeType]} ${n.name} (${n.code})`,
      value: n.id,
      isLeaf: true,
    });
  }

  const roots: TreeNode[] = [];
  for (const n of nodes) {
    const node = byId.get(n.id)!;
    if (n.parentId) {
      const list = children.get(n.parentId) ?? [];
      list.push(node);
      children.set(n.parentId, list);
    } else {
      roots.push(node);
    }
  }

  for (const [id, node] of byId) {
    const list = children.get(id);
    if (list && list.length > 0) {
      node.children = list;
      node.isLeaf = false;
    }
  }

  return roots;
}

export default function SpatialPage(): ReactNode {
  const { t } = useTranslation('common');
  const tenant = useSessionStore((s) => s.tenant);
  const [search, setSearch] = useState('');
  const [selected, setSelected] = useState<string | undefined>();
  const { data: nodes = [] } = useQuery(
    spatialNodesQueryOptions(tenant ?? undefined, { search }),
  );
  const create = useCreateSpatialNode(tenant ?? undefined);
  const move = useMoveSpatialNode(tenant ?? undefined);
  const remove = useDeleteSpatialNode(tenant ?? undefined);

  const treeData = buildTree(nodes);
  const selectedNode = selected
    ? nodes.find((n) => n.id === selected)
    : undefined;

  return (
    <section aria-labelledby="spatial-heading">
      <h1 id="spatial-heading">{t('spatial')}</h1>
      <div style={{ display: 'flex', gap: 8, marginBottom: 12 }}>
        <Input
          placeholder={t('search')}
          value={search}
          onChange={(v) => setSearch(v)}
          aria-label={t('search')}
        />
        <Can permission="tenant:site:write" capability="tenant:site:write">
          <Button
            onClick={() => {
              create.mutate(
                { nodeType: 'area', code: 'new', name: t('newSpatialNode') },
                { onError: (err) => Toast.error(String(err)) },
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
        renderLabel={(label) => (
          <span data-testid="spatial-label">{label}</span>
        )}
      />
      {selectedNode && (
        <div style={{ marginTop: 12, display: 'flex', gap: 8 }}>
          <Can permission="tenant:site:write" capability="tenant:site:write">
            <Button
              onClick={() => {
                move.mutate(
                  {
                    id: selectedNode.id,
                    parentId: null,
                    expectedRevision: selectedNode.revision,
                  },
                  { onError: (err) => Toast.error(String(err)) },
                );
              }}
            >
              {t('moveToRoot')}
            </Button>
          </Can>
          <Can permission="tenant:site:write" capability="tenant:site:write">
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
