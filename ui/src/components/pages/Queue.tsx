import { useState, useEffect, useRef } from 'react';
import { Link } from 'react-router-dom';
import { api, Job } from '../../api';
import { StatusBadge, PrefixBadge, TimeDisplay, formatNum } from '../shared';

export default function Queue() {
  const [jobs, setJobs] = useState<Job[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  const load = () => api.jobs().then(j => { setJobs(j); setLoading(false); setError(''); }).catch(e => setError(e.message));
  useEffect(() => { load(); const i = setInterval(load, 5000); return () => clearInterval(i); }, []);

  const act = async (fn: () => Promise<any>) => { setError(''); try { await fn(); load(); } catch (e: any) { setError(e.message); } };

  const counts = { queued: jobs.filter(j => j.status === 'queued').length, running: jobs.filter(j => j.status === 'running').length, paused: jobs.filter(j => j.status === 'paused').length, completed: jobs.filter(j => j.status === 'completed').length };

  return (
    <div>
      <div className="content-header">
        <h1>Queue</h1>
        <Link to="/new"><button className="primary">New Job</button></Link>
      </div>
      {error && <div className="error-banner">{error}</div>}
      <div style={{ display: 'flex', gap: 16, marginBottom: 20 }}>
        {Object.entries(counts).map(([k, v]) => (
          <div key={k} className="metric-card small" style={{ flex: 1 }}>
            <div className="label">{k}</div>
            <div className="value">{v}</div>
          </div>
        ))}
      </div>
      {loading && jobs.length === 0 ? (
        <div className="panel"><div className="skeleton skeleton-text" /><div className="skeleton skeleton-text short" /></div>
      ) : jobs.length === 0 ? (
        <div className="panel"><div style={{ textAlign: 'center', padding: 40, color: 'var(--text-muted)' }}>No jobs. <Link to="/new">Create one</Link></div></div>
      ) : (
        <div className="panel" style={{ overflow: 'auto' }}>
          <table>
            <thead><tr><th>Status</th><th>Name</th><th>Prefixes</th><th>Backend</th><th>Attempts</th><th>Est. Runtime</th><th>Actions</th></tr></thead>
            <tbody>
              {jobs.map(job => (
                <tr key={job.id}>
                  <td><StatusBadge status={job.status} /></td>
                  <td>{job.name || <span className="text-muted">-</span>}</td>
                  <td>{job.prefixes.map(p => <PrefixBadge key={p} prefix={p} />)}</td>
                  <td><span className="device-badge">{job.backend}</span></td>
                  <td className="tabular mono">{formatNum(job.attempts_done)}</td>
                  <td className="tabular mono"><TimeDisplay seconds={job.elapsed_seconds} /></td>
                  <td>
                    <div style={{ display: 'flex', gap: 4 }}>
                      {job.status === 'queued' && <button onClick={() => act(() => api.resumeJob(job.id))} style={{ fontSize: 11, padding: '2px 8px' }}>Start</button>}
                      {job.status === 'running' && <button onClick={() => act(() => api.pauseJob(job.id))} style={{ fontSize: 11, padding: '2px 8px' }}>Pause</button>}
                      {job.status === 'paused' && <button onClick={() => act(() => api.resumeJob(job.id))} style={{ fontSize: 11, padding: '2px 8px' }}>Resume</button>}
                      <button onClick={() => act(() => api.duplicateJob(job.id))} style={{ fontSize: 11, padding: '2px 8px' }}>Copy</button>
                      <button className="danger" onClick={() => { if (confirm('Delete?')) act(() => api.deleteJob(job.id)); }} style={{ fontSize: 11, padding: '2px 8px' }}>Del</button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
