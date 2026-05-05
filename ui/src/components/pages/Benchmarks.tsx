import { useState, useEffect } from 'react';
import { api, Benchmark } from '../../api';
import { DeviceBadge, formatKps, formatNum, TimeDisplay } from '../shared';

export default function Benchmarks() {
  const [benchmarks, setBenchmarks] = useState<Benchmark[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => { api.benchmarks().then(b => { setBenchmarks(b); setLoading(false); }).catch(() => {}); }, []);

  return (
    <div>
      <div className="content-header">
        <h1>Benchmarks</h1>
        <span className="text-muted" style={{ fontSize: 13 }}>Run benchmarks from CLI: mc-keygen --benchmark --benchmark-prefix-length 6</span>
      </div>
      {loading ? (
        <div className="glass-card"><div className="skeleton skeleton-text" /><div className="skeleton skeleton-text short" /></div>
      ) : benchmarks.length === 0 ? (
        <div className="glass-card"><div className="empty-state"><div className="icon">⏱</div><div className="title">No benchmarks</div><div className="desc">Run a benchmark to measure your hardware for accurate time estimates</div></div></div>
      ) : (
        <div className="glass-table">
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
  );
}
