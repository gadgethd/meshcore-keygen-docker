import { useState, useEffect } from 'react';
import { Link } from 'react-router-dom';
import { api, Job } from '../../api';
import { StatusChip, PrefixBadge, TimeDisplay, formatNum } from '../shared';

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
      <div style={{ display: 'flex', gap: 12, marginBottom: 20 }}>
        {Object.entries(counts).map(([k, v]) => (
          <div key={k} className="metric-card small" style={{ flex: 1 }}>
            <div className="label">{k}</div><div className="value">{v}</div>
          </div>
        ))}
      </div>
      {loading && jobs.length === 0 ? (
        <div className="glass-card"><div className="skeleton skeleton-text" /><div className="skeleton skeleton-text short" /></div>
      ) : jobs.length === 0 ? (
        <div className="glass-card"><div className="empty-state"><div className="icon">☰</div><div className="title">No jobs</div><div className="desc">Create a new search to get started</div><Link to="/new"><button className="primary">Create Job</button></Link></div></div>
      ) : (
        <div className="glass-table">
          <table><thead><tr><th>Status</th><th>Name</th><th>Prefixes</th><th>Backend</th><th>Attempts</th><th>Runtime</th><th>Actions</th></tr></thead>
          <tbody>{jobs.map(job => (
            <tr key={job.id}>
              <td><StatusChip status={job.status} /></td>
              <td>{job.name || <span className="text-muted">—</span>}</td>
              <td><div style={{ display:'flex', gap:4, flexWrap:'wrap' }}>{job.prefixes.map(p => <PrefixBadge key={p} prefix={p} />)}</div></td>
              <td><span className="device-badge">{job.backend}</span></td>
              <td className="tabular mono">{formatNum(job.attempts_done)}</td>
              <td className="tabular"><TimeDisplay seconds={job.elapsed_seconds} /></td>
              <td>
                <div style={{ display:'flex', gap:4 }}>
                  {job.status === 'queued' && <button onClick={() => act(() => api.resumeJob(job.id))} style={{ fontSize:11, padding:'3px 10px' }}>Start</button>}
                  {job.status === 'running' && <button onClick={() => act(() => api.pauseJob(job.id))} style={{ fontSize:11, padding:'3px 10px' }}>Pause</button>}
                  {job.status === 'paused' && <button onClick={() => act(() => api.resumeJob(job.id))} style={{ fontSize:11, padding:'3px 10px' }}>Resume</button>}
                  <button onClick={() => act(() => api.duplicateJob(job.id))} style={{ fontSize:11, padding:'3px 10px' }}>Copy</button>
                  <button className="danger" onClick={() => { if (confirm('Delete?')) act(() => api.deleteJob(job.id)); }} style={{ fontSize:11, padding:'3px 10px' }}>Del</button>
                </div>
              </td>
            </tr>
          ))}</tbody></table>
        </div>
      )}
    </div>
  );
}
