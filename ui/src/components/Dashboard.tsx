import { useEffect, useState, useRef } from 'react';
import { api, SystemStatus } from '../api';

function fmt(n: number): string {
  if (n > 1e9) return (n / 1e9).toFixed(1) + 'B';
  if (n > 1e6) return (n / 1e6).toFixed(1) + 'M';
  if (n > 1e3) return (n / 1e3).toFixed(1) + 'K';
  return n.toLocaleString();
}

function fmtDur(s: number): string {
  if (s < 60) return s.toFixed(1) + 's';
  if (s < 3600) return Math.floor(s / 60) + 'm ' + Math.floor(s % 60) + 's';
  return Math.floor(s / 3600) + 'h ' + Math.floor((s % 3600) / 60) + 'm';
}

export default function Dashboard() {
  const [status, setStatus] = useState<SystemStatus | null>(null);
  const [error, setError] = useState('');
  const mountedRef = useRef(true);
  const intervalRef = useRef<ReturnType<typeof setInterval>>();

  useEffect(() => {
    mountedRef.current = true;
    const poll = () => {
      api.status()
        .then(s => { if (mountedRef.current) { setStatus(s); setError(''); } })
        .catch(e => { if (mountedRef.current) setError(e.message); });
    };
    poll();
    intervalRef.current = setInterval(poll, 2000);
    return () => { mountedRef.current = false; clearInterval(intervalRef.current); };
  }, []);

  if (error && !status) return <div style={{ color: '#f85149' }}>Error: {error}</div>;
  if (!status) return <div style={{ color: '#8b949e' }}>Loading...</div>;

  const job = status.active_job;
  const pct = job && job.max_attempts ? Math.min(100, (job.attempts_done / job.max_attempts * 100)) : 0;

  return (
    <div>
      <h2 style={{ marginBottom: 16 }}>Dashboard</h2>
      {error && <div style={{ color: '#f85149', fontSize: 12, marginBottom: 12 }}>{error} (retrying...)</div>}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(200px, 1fr))', gap: 16, marginBottom: 24 }}>
        <Card title="Active Job" value={job ? job.prefixes.join(', ') : 'None'} />
        <Card title="Queue" value={String(status.queue_length)} />
        <Card title="Results" value={String(status.results_count)} />
        <Card title="CPU Cores" value={`${status.cpu_available_workers}/${status.cpu_total_cores} available`} />
        <Card title="GPU" value={status.gpu_available ? 'Available' : 'None'} />
        <Card title="Benchmark" value={status.last_benchmark_keys_per_second ? fmt(status.last_benchmark_keys_per_second) + ' k/s' : 'None'} />
      </div>
      {job && (
        <div className="card" style={{ marginTop: 16 }}>
          <h3 style={{ marginBottom: 12 }}>Active Job: {job.prefixes.join(', ')}</h3>
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(150px, 1fr))', gap: 12 }}>
            <Stat label="Status" value={job.status} />
            <Stat label="Keys/s" value={fmt(job.keys_per_second)} />
            <Stat label="Attempts" value={fmt(job.attempts_done)} />
            <Stat label="Elapsed" value={fmtDur(job.elapsed_seconds)} />
            <Stat label="Backend" value={`${job.backend}${job.device ? ' (' + job.device + ')' : ''}`} />
            <Stat label="CPU Workers" value={`${job.cpu_worker_threads} (${job.cpu_reserved_cores} reserved)`} />
          </div>
          {job.max_attempts && (
            <div style={{ marginTop: 12, background: '#21262d', borderRadius: 6, height: 8, overflow: 'hidden' }}>
              <div style={{ width: `${pct}%`, height: '100%', background: '#238636', transition: 'width 0.5s' }} />
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function Card({ title, value }: { title: string; value: string }) {
  return (
    <div className="card">
      <div style={{ fontSize: 12, color: '#8b949e', marginBottom: 4 }}>{title}</div>
      <div style={{ fontSize: 20, fontWeight: 600 }}>{value}</div>
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div style={{ fontSize: 11, color: '#8b949e' }}>{label}</div>
      <div style={{ fontWeight: 600 }}>{value}</div>
    </div>
  );
}
