import { useEffect, useState, useRef } from 'react';
import { api, SystemStatus } from '../api';
import { formatKps } from './shared';

export default function Topbar() {
  const [status, setStatus] = useState<SystemStatus | null>(null);
  const interval = useRef<ReturnType<typeof setInterval>>();

  useEffect(() => {
    const poll = () => api.status().then(setStatus).catch(() => {});
    poll();
    interval.current = setInterval(poll, 2000);
    return () => clearInterval(interval.current);
  }, []);

  const state = !status ? 'idle' : status.active_job ? status.active_job.status : 'idle';

  return (
    <div className="topbar">
      <div className="topbar-status">
        <span className={`topbar-dot ${state}`} />
        <span style={{ fontWeight: 600, fontSize: 13 }}>
          {state.charAt(0).toUpperCase() + state.slice(1)}
        </span>
        {status?.active_job && (
          <span style={{ color: 'var(--text-secondary)', marginLeft: 4, fontSize: 13 }}>
            {status.active_job.prefixes.join(', ')}
          </span>
        )}
      </div>
      <div className="topbar-metrics">
        {status?.active_job && (
          <div className="topbar-metric">
            <span className="label">Keys/s</span>
            <span className="value" style={{ color: 'var(--accent)' }}>{formatKps(status.active_job.keys_per_second)}</span>
          </div>
        )}
        <div className="topbar-metric">
          <span className="label">Queue</span>
          <span className="value">{status?.queue_length ?? '—'}</span>
        </div>
        <div className="topbar-metric">
          <span className="label">Results</span>
          <span className="value">{status?.results_count ?? '—'}</span>
        </div>
        {status?.active_job && (
          <div className="topbar-metric">
            <span className="label">Backend</span>
            <span className="value">{status.active_job.backend.toUpperCase()}</span>
          </div>
        )}
        <div className="topbar-metric">
          <span className="label">GPU</span>
          <span className="value" style={{ color: status?.gpu_available ? 'var(--success)' : 'var(--text-muted)' }}>
            {status?.gpu_available ? status.gpu_name?.split(' ').pop() || 'GPU' : 'None'}
          </span>
        </div>
      </div>
    </div>
  );
}
