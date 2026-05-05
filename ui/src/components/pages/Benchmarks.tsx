import { useState, useEffect, useRef } from 'react';
import { api, Benchmark } from '../../api';
import { DeviceBadge, MetricCard, TimeDisplay, PrefixBadge, StatusChip, formatKps, formatNum } from '../shared';

interface ActiveBenchmark {
  id: string;
  target_prefix: string;
  prefix_length: number;
  attempts: number;
  keys_per_second: number;
  elapsed_seconds: number;
  backend: string;
  found: boolean;
}

export default function Benchmarks() {
  const [benchmarks, setBenchmarks] = useState<Benchmark[]>([]);
  const [loading, setLoading] = useState(true);
  const [active, setActive] = useState<ActiveBenchmark | null>(null);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState('');
  const [prefixLen, setPrefixLen] = useState(6);
  const [backend, setBackend] = useState('cpu');
  const pollRef = useRef<ReturnType<typeof setInterval>>();

  const loadHistory = () => {
    api.benchmarks().then(b => { setBenchmarks(b); setLoading(false); }).catch(() => {});
  };

  useEffect(() => {
    loadHistory();
    // Poll active benchmark
    const poll = async () => {
      try {
        const r = await fetch('/api/benchmarks/active');
        const a: ActiveBenchmark | null = await r.json();
        setActive(a);
        if (!a) setRunning(false);
      } catch {}
    };
    poll();
    pollRef.current = setInterval(poll, 500);
    return () => { if (pollRef.current) clearInterval(pollRef.current); };
  }, []);

  const startBenchmark = async () => {
    setError(''); setRunning(true);
    try {
      await fetch('/api/benchmarks/run', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ prefix_length: prefixLen, backend }),
      });
    } catch (e: any) {
      setError(e.message);
      setRunning(false);
    }
  };

  const stopBenchmark = async () => {
    try { await fetch('/api/benchmarks/stop', { method: 'POST' }); } catch {}
  };

  return (
    <div>
      <div className="content-header">
        <h1>Benchmarks</h1>
      </div>

      {error && <div className="error-banner">{error}</div>}

      {/* Run controls */}
      <div className="glass-card" style={{ marginBottom: 20 }}>
        <div className="panel-header"><span className="panel-title">Run Benchmark</span></div>
        <div style={{ display: 'flex', alignItems: 'flex-end', gap: 12, flexWrap: 'wrap' }}>
          <div className="form-group" style={{ marginBottom: 0 }}>
            <label className="form-label">Backend</label>
            <select value={backend} onChange={e => setBackend(e.target.value)} disabled={running}>
              <option value="cpu">CPU</option>
              <option value="cuda">CUDA GPU</option>
            </select>
          </div>
          <div className="form-group" style={{ marginBottom: 0 }}>
            <label className="form-label">Prefix Length</label>
            <select value={prefixLen} onChange={e => setPrefixLen(Number(e.target.value))} disabled={running}>
              {[4,5,6,7,8].map(n => <option key={n} value={n}>{n} chars (~{formatNum(16**n)} expected)</option>)}
            </select>
          </div>
          <button className="primary" onClick={startBenchmark} disabled={running} style={{ marginBottom: 0 }}>
            {running ? 'Running...' : 'Start Benchmark'}
          </button>
          {running && <button className="danger" onClick={stopBenchmark} style={{ marginBottom: 0 }}>Stop</button>}
        </div>
      </div>

      {/* Active benchmark progress */}
      {active && (
        <div className="glass-card hero" style={{ marginBottom: 20 }}>
          <div className="panel-header"><span className="panel-title">Live Benchmark</span></div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 16 }}>
            <StatusChip status="running" />
            <PrefixBadge prefix={active.target_prefix} />
            <DeviceBadge backend={active.backend} />
          </div>
          <div className="grid grid-4">
            <MetricCard label="Keys/s" value={formatKps(active.keys_per_second)} color="var(--accent)" />
            <MetricCard label="Attempts" value={formatNum(active.attempts)} subtitle={`expect ~${formatNum(16**active.prefix_length)}`} />
            <MetricCard label="Elapsed" value={<TimeDisplay seconds={active.elapsed_seconds} />} />
            <MetricCard label="Status" value={active.found ? 'Found' : 'Searching...'} color={active.found ? 'var(--success)' : 'var(--accent)'} />
          </div>
        </div>
      )}

      {/* History */}
      <div className="glass-card">
        <div className="panel-header"><span className="panel-title">History</span></div>
        {loading && benchmarks.length === 0 ? (
          <div><div className="skeleton skeleton-text" /><div className="skeleton skeleton-text short" /></div>
        ) : benchmarks.length === 0 ? (
          <div className="empty-state"><div className="icon">⏱</div><div className="title">No benchmarks</div><div className="desc">Run a benchmark to measure your hardware for accurate time estimates</div></div>
        ) : (
          <div className="glass-table" style={{ border: 'none', background: 'none', backdropFilter: 'none', WebkitBackdropFilter: 'none' }}>
            <table><thead><tr><th>Date</th><th>Backend</th><th>Target</th><th>Keys/s</th><th>Attempts</th><th>Runtime</th><th>Found</th><th>Default</th></tr></thead>
            <tbody>{benchmarks.map(b => (
              <tr key={b.id}>
                <td className="tabular">{new Date(Number(b.created_at) * 1000).toLocaleDateString()}</td>
                <td><DeviceBadge backend={b.backend} device={b.device} /></td>
                <td className="mono">{b.target_prefix} <span className="text-muted">({b.prefix_length}c)</span></td>
                <td className="tabular mono" style={{ color: 'var(--accent)' }}>{formatKps(b.keys_per_second)}</td>
                <td className="tabular mono">{formatNum(b.attempts)}</td>
                <td className="tabular"><TimeDisplay seconds={b.elapsed_seconds} /></td>
                <td>{b.found ? <span className="text-success">✓</span> : <span className="text-warning">Timeout</span>}</td>
                <td>{b.is_default ? <span className="status-chip completed" style={{ fontSize: 10 }}>Default</span> : ''}</td>
              </tr>
            ))}</tbody></table>
          </div>
        )}
      </div>
    </div>
  );
}
