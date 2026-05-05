import { useState, useEffect, useRef } from 'react';
import { Link } from 'react-router-dom';
import { api, SystemStatus } from '../../api';
import { MetricCard, StatusChip, PrefixBadge, DeviceBadge, ProgressBar, EmptyState, ErrorBanner, formatKps, formatNum, formatPct, TimeDisplay } from '../shared';

export default function Dashboard() {
  const [s, setS] = useState<SystemStatus | null>(null);
  const [error, setError] = useState('');
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    const poll = () => api.status().then(v => { if (mounted.current) { setS(v); setError(''); } }).catch(e => { if (mounted.current) setError(e.message); });
    poll();
    const i = setInterval(poll, 2000);
    return () => { mounted.current = false; clearInterval(i); };
  }, []);

  const job = s?.active_job;
  const minLen = job ? Math.min(...job.prefixes.map(p => p.length)) : 0;
  const expected = 16 ** minLen;
  const prob = job ? 1 - Math.exp(-job.attempts_done / Math.max(expected, 1)) : 0;

  return (
    <div>
      {error && <ErrorBanner message={`${error} (retrying...)`} />}

      {!s ? (
        <div className="glass-card"><div className="skeleton skeleton-value" /><div className="skeleton skeleton-text" /><div className="skeleton skeleton-text short" /></div>
      ) : (
        <>
          {/* Hero active search */}
          {job ? (
            <div className="glass-card hero" style={{ marginBottom: 20 }}>
              <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', marginBottom: 20 }}>
                <div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 8 }}>
                    <StatusChip status={job.status} />
                    {job.prefixes.map(p => <PrefixBadge key={p} prefix={p} />)}
                    <DeviceBadge backend={job.backend} device={job.device} />
                  </div>
                  <div style={{ fontSize: 13, color: 'var(--text-secondary)' }}>{job.name || 'Active search'}</div>
                </div>
                <Link to="/active"><button className="primary">View Details</button></Link>
              </div>
              <div className="grid grid-4" style={{ marginBottom: 18 }}>
                <MetricCard label="Keys/s" value={formatKps(job.keys_per_second)} color="var(--accent)" />
                <MetricCard label="Attempts" value={formatNum(job.attempts_done)} subtitle={`expected ${formatNum(expected)}`} />
                <MetricCard label="Probability" value={formatPct(prob)} />
                <MetricCard label="Elapsed" value={<TimeDisplay seconds={job.elapsed_seconds} />} />
              </div>
              <ProgressBar pct={prob * 100} markers={[
                { at: 50, label: '50%' }, { at: 90, label: '90%' }, { at: 95, label: '95%' }, { at: 99, label: '99%' }
              ]} />
            </div>
          ) : (
            <div className="glass-card hero" style={{ marginBottom: 20 }}>
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                <div>
                  <div style={{ fontSize: 18, fontWeight: 700, marginBottom: 4 }}>No active search</div>
                  <div style={{ color: 'var(--text-secondary)', fontSize: 13 }}>{s.queue_length > 0 ? `${s.queue_length} job(s) queued` : 'Queue is empty'}</div>
                </div>
                <div style={{ display: 'flex', gap: 8 }}>
                  <Link to="/new"><button className="primary">Create Job</button></Link>
                </div>
              </div>
            </div>
          )}

          <div className="grid grid-3">
            {/* System card */}
            <div className="glass-card">
              <div className="panel-header"><span className="panel-title">System</span></div>
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '8px 16px', fontSize: 13 }}>
                <div style={{ color: 'var(--text-muted)' }}>CPU Cores</div><div className="tabular">{s.cpu_available_workers}/{s.cpu_total_cores}</div>
                <div style={{ color: 'var(--text-muted)' }}>GPU</div><div>{s.gpu_available ? <span className="text-accent">{s.gpu_name || 'Available'}</span> : 'None'}</div>
                <div style={{ color: 'var(--text-muted)' }}>Queue</div><div className="tabular">{s.queue_length}</div>
                <div style={{ color: 'var(--text-muted)' }}>Results</div><div className="tabular">{s.results_count}</div>
                <div style={{ color: 'var(--text-muted)' }}>Benchmark</div><div className="tabular">{s.last_benchmark_keys_per_second ? formatKps(s.last_benchmark_keys_per_second) + ' k/s' : 'None'}</div>
              </div>
            </div>

            <div className="glass-card">
              <div className="panel-header"><span className="panel-title">Recent Results</span></div>
              <div style={{ fontSize: 13 }}>
                {s.results_count === 0 ? <span className="text-muted">No results yet</span> : `${s.results_count} total · `}
                <Link to="/results">View all</Link>
              </div>
            </div>

            <div className="glass-card">
              <div className="panel-header"><span className="panel-title">Queue</span></div>
              <div style={{ fontSize: 13 }}>
                {s.queue_length === 0 ? <span className="text-muted">Queue is empty</span> : `${s.queue_length} job(s) waiting`}
                {s.queue_length > 0 && <> · <Link to="/queue">Manage</Link></>}
              </div>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
