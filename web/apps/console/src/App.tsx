import { RouterProvider } from 'react-router-dom';
import './i18n/index.ts';
import { createAppRouter } from './routes/index.tsx';

const router = createAppRouter();

export default function App() {
  return <RouterProvider router={router} />;
}
