import { useState, useEffect } from 'react';
import { api, Job, SystemStatus } from '../../api';
import { StatusChip, PrefixBadge } from '../shared';

export default function Schedules() {
  const [jobs, setJobs] = useState<Job[]>([]);
  const [status, setStatus] = useState<SystemStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');

  useEffect(() => {
    api.jobs().then(j => setJobs(j)).catch(() => {});
    api.status().then(s => { setStatus(s); setLoading(false); }).catch(e => setError(e.message));
  }, []);

  const scheduledJobs = jobs.filter(j => j.schedule_enabled);

  return (
    <div>
      <div className="content-header"><h1>Schedules</h1></div>
      {error && <div className="error-banner">{error}</div>}

      {/* Global Schedule Status */}
      {status && (
        <div className="glass-card hero" style={{ marginBottom: 20 }}>
          <div className="panel-header"><span className="panel-title">Global Schedule</span></div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 16, flexWrap: 'wrap' }}>
            <StatusChip status={status.schedule_enabled ? 'running' : 'stopped'} />
            <span style={{ fontSize: 16, fontWeight: 600 }}>
              {status.schedule_enabled
                ? `Window: ${status.schedule_start} — ${status.schedule_end}`
                : 'Schedule disabled — queue runs freely'}
            </span>
          </div>
          {status.schedule_enabled && (
            <div style={{ marginTop: 12, color: 'var(--text-secondary)', fontSize: 13 }}>
              New jobs will only start during the configured window. Configure in Settings.
            </div>
          )}
        </div>
      )}

      {loading ? (
        <div className="glass-card"><div className="skeleton skeleton-text" /><div className="skeleton skeleton-text short" /></div>
      ) : (
        <>
          {/* Per-job schedules */}
          <div className="glass-card">
            <div className="panel-header">
              <span className="panel-title">Job Schedules ({scheduledJobs.length})</span>
            </div>
            {scheduledJobs.length === 0 ? (
              <div className="empty-state" style={{ padding: '24px 0' }}>
                <div className="icon">⌛</div>
                <div className="title">No jobs with schedules</div>
                <div className="desc">Enable per-job schedules on the Queue page</div>
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
        </>
      )}
    </div>
  );
}
