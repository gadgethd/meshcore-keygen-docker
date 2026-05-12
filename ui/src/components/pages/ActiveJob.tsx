import { useState, useEffect, useRef } from 'react';
import { Link } from 'react-router-dom';
import { api, SystemStatus } from '../../api';
import { MetricCard, StatusChip, PrefixBadge, DeviceBadge, ProbabilityProgress, CopyButton, TimeDisplay, EmptyState, formatKps, formatNum, formatPct } from '../shared';

export default function ActiveJob() {
  const [s, setS] = useState<SystemStatus | null>(null);
  const [error, setError] = useState('');
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    const poll = () => api.status().then(v => { if (mounted.current) { setS(v); setError(''); } }).catch(() => {});
    poll();
    const i = setInterval(poll, 1000);
    return () => { mounted.current = false; clearInterval(i); };
  }, []);

  const job = s?.active_job;
  if (!s) return <div className="glass-card"><div className="skeleton skeleton-value" /></div>;
  if (!job) return (
    <EmptyState icon="▶" title="No Active Job" desc="Start a new search from the New Job page" action={<Link to="/new"><button className="primary">Create Job</button></Link>} />
  );

  const minLen = Math.min(...job.prefixes.map(p => p.length));
  const sameLenCount = job.prefixes.filter(p => p.length === minLen).length;
  const expected = (16 ** minLen) / Math.max(sameLenCount, 1);
  const prob = 1 - Math.exp(-job.attempts_done / expected);

  const act = async (fn: () => Promise<any>) => { try { await fn(); } catch(e: any) { setError(e.message); } };

  return (
    <div>
      <div className="content-header">
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <h1>{job.name || 'Active Search'}</h1>
          <StatusChip status={job.status} />
          {job.prefixes.map(p => <PrefixBadge key={p} prefix={p} />)}
        </div>
        <div style={{ display: 'flex', gap: 6 }}>
          {job.status === 'running' && <button onClick={() => act(() => api.pauseJob(job.id))}>Pause</button>}
          {job.status === 'paused' && <button className="primary" onClick={() => act(() => api.resumeJob(job.id))}>Resume</button>}
          {(job.status === 'running' || job.status === 'paused') && <button className="danger" onClick={() => act(() => api.stopJob(job.id))}>Stop</button>}
          <button onClick={() => act(() => api.restartJob(job.id))}>Restart</button>
          <button onClick={() => act(() => api.duplicateJob(job.id))}>Duplicate</button>
        </div>
      </div>
      {error && <div className="error-banner">{error}</div>}

      <div className="grid grid-4" style={{ marginBottom: 20 }}>
        <MetricCard label="Keys/s" value={formatKps(job.keys_per_second)} color="var(--accent)" size="hero" />
        <MetricCard label="Attempts" value={formatNum(job.attempts_done)} subtitle={`expected ${formatNum(expected)}`} />
        <MetricCard label="Probability" value={formatPct(prob)} />
        <MetricCard label="Elapsed" value={<TimeDisplay seconds={job.elapsed_seconds} />} />
      </div>

      <div className="glass-card" style={{ marginBottom: 20 }}>
        <div className="panel-header"><span className="panel-title">Probability Progress</span></div>
        <ProbabilityProgress attempts={job.attempts_done} expected={expected} />
      </div>

      <div className="grid grid-2">
        <div className="glass-card">
          <div className="panel-header"><span className="panel-title">Search Details</span></div>
          <div style={{ fontSize: 13, display: 'flex', flexDirection: 'column', gap: 8 }}>
            <div><span className="text-muted">Prefixes: </span>{job.prefixes.map(p => <PrefixBadge key={p} prefix={p} />)}</div>
            <div><span className="text-muted">Backend: </span><DeviceBadge backend={job.backend} /> · {job.cpu_worker_threads} workers ({job.cpu_reserved_cores} reserved)</div>
            <div><span className="text-muted">Max attempts: </span>{job.max_attempts ? formatNum(job.max_attempts) : 'Unlimited'}</div>
            <div><span className="text-muted">Max runtime: </span>{job.max_runtime ? <TimeDisplay seconds={job.max_runtime} /> : 'Unlimited'}</div>
          </div>
        </div>
        <div className="glass-card">
          <div className="panel-header"><span className="panel-title">Deterministic Resume</span></div>
          <div style={{ fontSize: 13, display: 'flex', flexDirection: 'column', gap: 8 }}>
            <div><span className="text-muted">Master seed: </span>
              {job.master_seed ? <><span className="mono">{job.master_seed.slice(0, 16)}...</span><CopyButton text={job.master_seed} /></> : 'Not set'}
            </div>
            <div><span className="text-muted">Next counter: </span><span className="tabular mono">{job.next_counter != null ? formatNum(job.next_counter) : '—'}</span></div>
            <div><span className="text-muted">Job ID: </span><span className="mono" style={{ fontSize: 11 }}>{job.id.slice(0, 16)}...</span><CopyButton text={job.id} /></div>
          </div>
        </div>
      </div>
    </div>
  );
}
