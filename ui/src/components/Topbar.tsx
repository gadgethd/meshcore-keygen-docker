import { useEffect, useState, useRef } from 'react';
import { api, SystemStatus } from '../api';

export default function Topbar() {
  const [status, setStatus] = useState<SystemStatus | null>(null);
  const interval = useRef<ReturnType<typeof setInterval>>();

  useEffect(() => {
    const poll = () => api.status().then(setStatus).catch(() => {});
    poll();
    interval.current = setInterval(poll, 2000);
    return () => clearInterval(interval.current);
  }, []);

  const state = !status ? 'idle' :
    status.active_job ? status.active_job.status :
    'idle';

  return (
    <div className="topbar">
      <div className="topbar-status">
        <span className={`topbar-dot ${state}`} />
        <span style={{ fontWeight: 600 }}>
          {state.charAt(0).toUpperCase() + state.slice(1)}
        </span>
        {status?.active_job && (
          <span style={{ color: 'var(--text-secondary)', marginLeft: 4 }}>
            {status.active_job.prefixes.join(', ')}
          </span>
        )}
      </div>
      <div className="topbar-metrics">
        {status?.active_job && (
          <div className="topbar-metric">
            <span className="label">Keys/s</span>
            <span className="value">{formatKps(status.active_job.keys_per_second)}</span>
          </div>
        )}
        <div className="topbar-metric">
          <span className="label">Queue</span>
          <span className="value">{status?.queue_length ?? '-'}</span>
        </div>
        <div className="topbar-metric">
          <span className="label">Results</span>
          <span className="value">{status?.results_count ?? '-'}</span>
        </div>
        {status?.active_job && (
          <div className="topbar-metric">
            <span className="label">Backend</span>
            <span className="value">{status.active_job.backend}</span>
          </div>
        )}
      </div>
    </div>
  );
}

function formatKps(n: number): string {
  if (n >= 1e9) return (n / 1e9).toFixed(2) + 'G';
  if (n >= 1e6) return (n / 1e6).toFixed(1) + 'M';
  if (n >= 1e3) return (n / 1e3).toFixed(1) + 'K';
  return n.toFixed(0);
}
