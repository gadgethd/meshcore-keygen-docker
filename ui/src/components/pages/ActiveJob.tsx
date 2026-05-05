import { useState, useEffect, useRef } from 'react';
import { useParams, Link } from 'react-router-dom';
import { api, Job, SystemStatus } from '../../api';
import { MetricCard, StatusBadge, PrefixBadge, ProgressBar, SecretField, TimeDisplay, formatKps, formatNum, formatPct, CopyButton } from '../shared';

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
  if (!s) return <div className="skeleton skeleton-value" style={{ width: 200 }} />;
  if (!job) return (
    <div className="panel">
      <div style={{ textAlign: 'center', padding: 40, color: 'var(--text-muted)' }}>
        <div style={{ fontSize: 36, marginBottom: 8 }}>▶</div>
        <div style={{ fontSize: 16, color: 'var(--text-secondary)', marginBottom: 8 }}>No Active Job</div>
        <Link to="/new"><button className="primary">Create a Job</button></Link>
      </div>
    </div>
  );

  const minLen = Math.min(...job.prefixes.map(p => p.length));
  const expected = 16 ** minLen;
  const prob = 1 - Math.exp(-job.attempts_done / expected);

  const act = async (fn: () => Promise<any>) => { try { await fn(); } catch(e: any) { setError(e.message); } };

  return (
    <div>
      <div className="content-header">
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <h1>{job.name || 'Active Job'}</h1>
          <StatusBadge status={job.status} />
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

      {/* Hero metrics */}
      <div className="grid grid-4" style={{ marginBottom: 20 }}>
        <MetricCard label="Keys/s" value={formatKps(job.keys_per_second)} color="var(--accent)" size="hero" />
        <MetricCard label="Attempts" value={formatNum(job.attempts_done)} subtitle={`expected ${formatNum(expected)}`} />
        <MetricCard label="Probability" value={formatPct(prob)} />
        <MetricCard label="Elapsed" value={<TimeDisplay seconds={job.elapsed_seconds} />} />
      </div>

      {/* Progress */}
      <div className="panel" style={{ marginBottom: 20 }}>
        <div className="panel-header"><span className="panel-title">Progress</span></div>
        <ProgressBar pct={prob * 100} markers={[
          { at: 50, label: '50%' }, { at: 90, label: '90%' }, { at: 95, label: '95%' }, { at: 99, label: '99%' }
        ]} />
        <div className="grid grid-4" style={{ marginTop: 12 }}>
          <MetricCard size="small" label="50% at" value={formatNum(expected * 0.693)} />
          <MetricCard size="small" label="90% at" value={formatNum(expected * 2.302)} />
          <MetricCard size="small" label="95% at" value={formatNum(expected * 2.996)} />
          <MetricCard size="small" label="99% at" value={formatNum(expected * 4.605)} />
        </div>
      </div>

      {/* Details grid */}
      <div className="grid grid-2">
        <div className="panel">
          <div className="panel-header"><span className="panel-title">Search Details</span></div>
          <div style={{ fontSize: 13, display: 'flex', flexDirection: 'column', gap: 6 }}>
            <div><span className="text-muted">Prefixes: </span>{job.prefixes.map(p => <PrefixBadge key={p} prefix={p} />)}</div>
            <div><span className="text-muted">Backend: </span>{job.backend} · {job.cpu_worker_threads} workers ({job.cpu_reserved_cores} reserved)</div>
            <div><span className="text-muted">Max attempts: </span>{job.max_attempts ? formatNum(job.max_attempts) : 'Unlimited'}</div>
            <div><span className="text-muted">Max runtime: </span>{job.max_runtime ? <TimeDisplay seconds={job.max_runtime} /> : 'Unlimited'}</div>
          </div>
        </div>
        <div className="panel">
          <div className="panel-header"><span className="panel-title">Deterministic Resume</span></div>
          <div style={{ fontSize: 13, display: 'flex', flexDirection: 'column', gap: 6 }}>
            <div><span className="text-muted">Master seed: </span>
              {job.master_seed ? <><span className="mono">{job.master_seed.slice(0, 16)}...</span><CopyButton text={job.master_seed} /></> : <span className="text-muted">Not set</span>}
            </div>
            <div><span className="text-muted">Next counter: </span><span className="tabular mono">{job.next_counter !== null && job.next_counter !== undefined ? formatNum(job.next_counter) : '-'}</span></div>
            <div><span className="text-muted">Job ID: </span><span className="mono" style={{ fontSize: 11 }}>{job.id.slice(0, 16)}...</span><CopyButton text={job.id} /></div>
          </div>
        </div>
      </div>
    </div>
  );
}
