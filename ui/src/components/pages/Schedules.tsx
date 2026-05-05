import { useState, useEffect } from 'react';
import { api, Job } from '../../api';
import { StatusChip, PrefixBadge } from '../shared';

export default function Schedules() {
  const [jobs, setJobs] = useState<Job[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  const load = () => api.jobs().then(j => { setJobs(j); setLoading(false); setError(''); }).catch(e => setError(e.message));
  useEffect(() => { load(); }, []);

  const scheduledJobs = jobs.filter(j => j.schedule_enabled);
  const updatableJobs = jobs.filter(j => j.status === 'queued' || j.status === 'paused' || j.status === 'scheduled');

  const toggleSchedule = async (job: Job, enabled: boolean) => {
    setError('');
    try {
      await api.updateJob(job.id, { schedule_enabled: enabled } as any);
      load();
    } catch (e: any) { setError(e.message); }
  };

  return (
    <div>
      <div className="content-header"><h1>Schedules</h1></div>
      {error && <div className="error-banner">{error}</div>}

      {loading ? (
        <div className="glass-card"><div className="skeleton skeleton-text" /><div className="skeleton skeleton-text short" /></div>
      ) : (
        <>
          {/* Active schedules */}
          <div className="glass-card" style={{ marginBottom: 20 }}>
            <div className="panel-header">
              <span className="panel-title">Active Schedules ({scheduledJobs.length})</span>
            </div>
            {scheduledJobs.length === 0 ? (
              <div className="empty-state" style={{ padding: '24px 0' }}>
                <div className="icon">⌛</div>
                <div className="title">No schedules configured</div>
                <div className="desc">Enable schedule on a job below to see it here</div>
              </div>
            ) : (
              scheduledJobs.map(job => (
                <div key={job.id} className="glass-card compact" style={{ marginBottom: 10, border: '1px solid var(--glass-border)' }}>
                  <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 6 }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                      <StatusChip status={job.status} />
                      {job.prefixes.map(p => <PrefixBadge key={p} prefix={p} />)}
                      <span style={{ fontSize: 13 }}>{job.name || 'Unnamed'}</span>
                    </div>
                    <button className="danger" style={{ fontSize: 11, padding: '3px 10px' }} onClick={() => toggleSchedule(job, false)}>
                      Disable
                    </button>
                  </div>
                  <div style={{ fontSize: 12, color: 'var(--text-secondary)', display: 'flex', gap: 24 }}>
                    <span>Start: {job.schedule_start || 'Not set'}</span>
                    <span>End: {job.schedule_end || 'Not set'}</span>
                    <span>Backend: {job.backend}</span>
                  </div>
                </div>
              ))
            )}
          </div>

          {/* Available jobs to schedule */}
          <div className="glass-card">
            <div className="panel-header">
              <span className="panel-title">Available Jobs ({updatableJobs.length})</span>
              <span className="text-muted" style={{ fontSize: 12 }}>Toggle schedule on jobs below</span>
            </div>
            {updatableJobs.length === 0 ? (
              <div className="text-muted" style={{ fontSize: 13, padding: '12px 0' }}>No jobs available to schedule. Queued or paused jobs appear here.</div>
            ) : (
              <div className="glass-table" style={{ border: 'none', background: 'none', backdropFilter: 'none', WebkitBackdropFilter: 'none' }}>
                <table>
                  <thead><tr><th>Job</th><th>Prefixes</th><th>Status</th><th>Schedule</th><th>Action</th></tr></thead>
                  <tbody>
                    {updatableJobs.map(job => (
                      <tr key={job.id}>
                        <td>{job.name || <span className="text-muted">—</span>}</td>
                        <td><div style={{ display: 'flex', gap: 4, flexWrap: 'wrap' }}>{job.prefixes.map(p => <PrefixBadge key={p} prefix={p} />)}</div></td>
                        <td><StatusChip status={job.status} /></td>
                        <td>
                          {job.schedule_enabled ? (
                            <span className="text-accent" style={{ fontSize: 12 }}>{job.schedule_start || 'any'} → {job.schedule_end || 'any'}</span>
                          ) : (
                            <span className="text-muted" style={{ fontSize: 12 }}>None</span>
                          )}
                        </td>
                        <td>
                          {job.schedule_enabled ? (
                            <button style={{ fontSize: 11, padding: '3px 10px' }} onClick={() => toggleSchedule(job, false)}>Disable</button>
                          ) : (
                            <button className="primary" style={{ fontSize: 11, padding: '3px 10px' }} onClick={() => toggleSchedule(job, true)}>Enable</button>
                          )}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        </>
      )}
    </div>
  );
}
