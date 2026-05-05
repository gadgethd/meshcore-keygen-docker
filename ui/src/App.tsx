import { Routes, Route, Navigate } from 'react-router-dom';
import Layout from './components/Layout';
import Dashboard from './components/pages/Dashboard';
import NewJob from './components/pages/NewJob';
import ActiveJob from './components/pages/ActiveJob';
import Queue from './components/pages/Queue';
import Results from './components/pages/Results';
import Benchmarks from './components/pages/Benchmarks';
import Schedules from './components/pages/Schedules';
import Settings from './components/pages/Settings';
import Logs from './components/pages/Logs';

export default function App() {
  return (
    <Layout>
      <Routes>
        <Route path="/" element={<Dashboard />} />
        <Route path="/new" element={<NewJob />} />
        <Route path="/active" element={<ActiveJob />} />
        <Route path="/queue" element={<Queue />} />
        <Route path="/results" element={<Results />} />
        <Route path="/benchmarks" element={<Benchmarks />} />
        <Route path="/schedules" element={<Schedules />} />
        <Route path="/settings" element={<Settings />} />
        <Route path="/logs" element={<Logs />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </Layout>
  );
}
