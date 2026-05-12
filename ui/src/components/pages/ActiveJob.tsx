import { useState, useEffect, useRef } from 'react';
import { Link } from 'react-router-dom';
import { api, Job, SystemStatus } from '../../api';
import { MetricCard, StatusChip, PrefixBadge, DeviceBadge, ProbabilityProgress, CopyButton, TimeDisplay, EmptyState, formatKps, formatNum, formatPct } from '../shared';

export default function ActiveJob() {
  const [s, setS] = useState<SystemStatus | null>(null);
  const [jobs, setJobs] = useState<Job[]>([]);
  const [error, setError] = useState('');
  const mounted = useRef(true);

  const load = async () => {
    const [status, allJobs] = await Promise.all([api.status(), api.jobs()]);
    if (mounted.current) {
      setS(status);
      setJobs(allJobs);
      setError('');
    }
  };

  useEffect(() => {
    mounted.current = true;
    const poll = () => load().catch(() => {});
    poll();
    const i = setInterval(poll, 1000);
    return () => { mounted.current = false; clearInterval(i); };
  }, []);

  const job = s?.active_job;
  if (!s) return <div className="glass-card"><div className="skeleton skeleton-value" /></div>;
  const pausedJobs = jobs.filter(j => j.status === 'paused' && j.id !== job?.id);
  const act = async (fn: () => Promise<any>) => { try { await fn(); await load(); } catch(e: any) { setError(e.message); } };

  if (!job) return (
    <div>
      <EmptyState icon="▶" title="No Active Job" desc="Start a new search from the New Job page" action={<Link to="/new"><button className="primary">Create Job</button></Link>} />
      {error && <div className="error-banner">{error}</div>}
      <PausedJobsPanel jobs={pausedJobs} onResume={(pausedJob) => act(() => api.resumeJob(pausedJob.id))} />
    </div>
  );

  const expected = expectedAttempts(job);
  const prob = 1 - Math.exp(-job.attempts_done / expected);

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

      <PausedJobsPanel jobs={pausedJobs} onResume={(pausedJob) => act(() => api.resumeJob(pausedJob.id))} />
    </div>
  );
}

function PausedJobsPanel({ jobs, onResume }: { jobs: Job[]; onResume: (job: Job) => void }) {
  if (jobs.length === 0) return null;

  return (
    <div className="glass-card" style={{ marginTop: 20 }}>
      <div className="panel-header">
        <span className="panel-title">Paused Jobs</span>
        <span className="text-muted" style={{ fontSize: 12 }}>{jobs.length} ready to resume</span>
      </div>
      <div className="paused-job-list">
        {jobs.map(job => (
          <div key={job.id} className="paused-job-row">
            <div className="paused-job-main">
              <div className="paused-job-title">
                <StatusChip status={job.status} />
                <strong>{job.name || 'Paused Search'}</strong>
              </div>
              <div className="paused-job-prefixes">
                {job.prefixes.map(p => <PrefixBadge key={p} prefix={p} />)}
              </div>
            </div>
            <div className="paused-job-stats">
              <div>
                <span className="text-muted">Attempts</span>
                <strong className="tabular mono">{formatNum(job.attempts_done)}</strong>
              </div>
              <div>
                <span className="text-muted">Elapsed</span>
                <strong className="tabular"><TimeDisplay seconds={job.elapsed_seconds} /></strong>
              </div>
            </div>
            <button className="primary" onClick={() => onResume(job)}>Resume</button>
          </div>
        ))}
      </div>
    </div>
  );
}

function expectedAttempts(job: Job) {
  if (job.prefixes.length === 0) return 1;
  const minLen = Math.min(...job.prefixes.map(p => p.length));
  const sameLenCount = job.prefixes.filter(p => p.length === minLen).length;
  return (16 ** minLen) / Math.max(sameLenCount, 1);
}
