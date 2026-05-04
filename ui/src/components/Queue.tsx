import { useEffect, useState, useRef } from 'react';
import { Link } from 'react-router-dom';
import { api, Job } from '../api';

export default function Queue() {
  const [jobs, setJobs] = useState<Job[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const mountedRef = useRef(true);

  const load = () => {
    api.jobs()
      .then(j => { if (mountedRef.current) { setJobs(j); setError(''); setLoading(false); } })
      .catch(e => { if (mountedRef.current) setError(e.message); });
  };

  useEffect(() => {
    mountedRef.current = true;
    load();
    const i = setInterval(load, 3000);
    return () => { mountedRef.current = false; clearInterval(i); };
  }, []);

  const act = async (fn: () => Promise<any>) => {
    setError('');
    try { await fn(); load(); } catch (e: any) { setError(e.message); }
  };

  return (
    <div>
      <h2 style={{ marginBottom: 16 }}>Queue</h2>
      {error && <div style={{ color: '#f85149', marginBottom: 12 }}>{error}</div>}
      {loading && jobs.length === 0 && <div style={{ color: '#8b949e' }}>Loading...</div>}
      {!loading && jobs.length === 0 && <div style={{ color: '#8b949e' }}>No jobs yet. <Link to="/new">Create one</Link></div>}
      <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
        {jobs.map(job => (
          <div key={job.id} className="card" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <div>
              <div style={{ fontWeight: 600 }}>{job.name || job.prefixes.join(', ')}</div>
              <div style={{ fontSize: 12, color: '#8b949e' }}>
                {job.backend} &middot; {job.status} &middot; {job.attempts_done.toLocaleString()} attempts
                {job.keys_per_second > 0 && ` &middot; ${Math.round(job.keys_per_second).toLocaleString()} k/s`}
              </div>
            </div>
            <div style={{ display: 'flex', gap: 6 }}>
              {job.status === 'running' && <button onClick={() => act(() => api.pauseJob(job.id))}>Pause</button>}
              {job.status === 'paused' && <button onClick={() => act(() => api.resumeJob(job.id))}>Resume</button>}
              {(job.status === 'running' || job.status === 'paused') && <button className="danger" onClick={() => act(() => api.stopJob(job.id))}>Stop</button>}
              {job.status === 'queued' && <button onClick={() => act(() => api.resumeJob(job.id))}>Start</button>}
              <button onClick={() => act(() => api.duplicateJob(job.id))}>Copy</button>
              <button className="danger" onClick={() => { if (confirm('Delete?')) act(() => api.deleteJob(job.id)); }}>Delete</button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
