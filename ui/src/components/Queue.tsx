import { useEffect, useState } from 'react';
import { api, Job } from '../api';

export default function Queue() {
  const [jobs, setJobs] = useState<Job[]>([]);
  const [error, setError] = useState('');

  const load = () => api.jobs().then(setJobs).catch(e => setError(e.message));
  useEffect(() => { load(); const i = setInterval(load, 3000); return () => clearInterval(i); }, []);

  const act = async (fn: () => Promise<any>) => { try { await fn(); load(); } catch (e: any) { setError(e.message); } };

  return (
    <div>
      <h2 style={{ marginBottom: 16 }}>Queue</h2>
      {error && <div style={{ color: '#f85149', marginBottom: 12 }}>{error}</div>}
      {jobs.length === 0 && <div style={{ color: '#8b949e' }}>No jobs yet. <a href="/new">Create one</a></div>}
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
