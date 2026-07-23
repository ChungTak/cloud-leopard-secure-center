import * as React from 'react';

export type LayoutSize = 1 | 4 | 9 | 16;

export interface MultiPlayerLayoutProps {
  layout: LayoutSize;
  /** Render a player for each visible slot. */
  renderSlot?: (index: number) => React.ReactElement | null;
}

/**
 * Multi-view player layout with 1/4/9/16 slots.
 *
 * Phase 1: the grid is wired but each slot renders the caller's placeholder
 * because the upstream media stack is unavailable.
 */
export function MultiPlayerLayout({
  layout,
  renderSlot,
}: MultiPlayerLayoutProps): React.ReactElement {
  const columns = Math.sqrt(layout);
  const cells = Array.from({ length: layout }, (_, i) => i);

  return (
    <div
      data-testid="multi-player-layout"
      style={{
        display: 'grid',
        gridTemplateColumns: `repeat(${columns}, 1fr)`,
        gap: '8px',
      }}
    >
      {cells.map((index) => (
        <div
          key={index}
          data-testid="player-slot"
          data-slot-index={index}
          style={{ border: '1px solid #ccc', minHeight: '80px' }}
        >
          {renderSlot ? (
            renderSlot(index)
          ) : (
            <span data-testid="unsupported-slot">unsupported</span>
          )}
        </div>
      ))}
    </div>
  );
}
