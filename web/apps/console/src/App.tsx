import { useState } from 'react';
import { Button } from '@clsc/ui';
import { Player } from '@clsc/player';
import { ApiClient } from '@clsc/api-client';
import './App.css';

const client = new ApiClient('/api/v1');

function App() {
  const [count, setCount] = useState(0);

  return (
    <main>
      <h1>Cloud Leopard Secure Center</h1>
      <Button
        label={`Clicked ${count} times`}
        onClick={() => setCount((c) => c + 1)}
      />
      <Player streamUrl="wss://example.com/live" />
      <pre>{JSON.stringify(client.health())}</pre>
    </main>
  );
}

export default App;
