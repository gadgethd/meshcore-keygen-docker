import { Routes, Route } from 'react-router-dom';
import Layout from './components/Layout';
import Dashboard from './components/Dashboard';
import NewJob from './components/NewJob';
import Queue from './components/Queue';
import Results from './components/Results';
import Settings from './components/Settings';

export default function App() {
  return (
    <Layout>
      <Routes>
        <Route path="/" element={<Dashboard />} />
        <Route path="/new" element={<NewJob />} />
        <Route path="/queue" element={<Queue />} />
        <Route path="/results" element={<Results />} />
        <Route path="/settings" element={<Settings />} />
      </Routes>
    </Layout>
  );
}
