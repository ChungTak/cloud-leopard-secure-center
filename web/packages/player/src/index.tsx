import * as React from 'react';

export interface PlayerProps {
  streamUrl: string;
}

export function Player({ streamUrl }: PlayerProps): React.ReactElement {
  return <div data-testid="player">Stream: {streamUrl}</div>;
}
