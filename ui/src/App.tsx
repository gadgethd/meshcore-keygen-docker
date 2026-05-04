import { Routes, Route, Navigate } from 'react-router-dom';
import Layout from './components/Layout';
import Dashboard from './components/Dashboard';
import NewJob from './components/NewJob';
import Queue from './components/Queue';
import Results from './components/Results';
import Settings from './components/Settings';

function ErrorFallback({ error }: { error?: Error }) {
  return (
    <div style={{ padding: 24, color: '#f85149' }}>
      <h3>Something went wrong</h3>
      <p style={{ fontSize: 14, color: '#8b949e' }}>{error?.message || 'Unknown error'}</p>
      <button onClick={() => window.location.reload()} style={{ marginTop: 12 }}>Reload</button>
    </div>
  );
}

export default function App() {
  return (
    <Layout>
      <Routes>
        <Route path="/" element={<Dashboard />} />
        <Route path="/new" element={<NewJob />} />
        <Route path="/queue" element={<Queue />} />
        <Route path="/results" element={<Results />} />
        <Route path="/settings" element={<Settings />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </Layout>
  );
}
