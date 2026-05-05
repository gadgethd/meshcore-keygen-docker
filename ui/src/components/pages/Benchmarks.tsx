import { useState, useEffect } from 'react';
import { api, Benchmark } from '../../api';
import { formatKps, formatNum, TimeDisplay, DeviceBadge } from '../shared';

export default function Benchmarks() {
  const [benchmarks, setBenchmarks] = useState<Benchmark[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    api.benchmarks().then(b => { setBenchmarks(b); setLoading(false); }).catch(() => {});
  }, []);

  return (
    <div>
      <div className="content-header">
        <h1>Benchmarks</h1>
        <span className="text-muted" style={{ fontSize: 13 }}>Benchmarks power time estimates on the New Job page</span>
      </div>
      {loading ? <div className="skeleton skeleton-text" /> : (
        benchmarks.length === 0 ? (
          <div className="panel"><div style={{ textAlign: 'center', padding: 40, color: 'var(--text-muted)' }}>No benchmarks yet. Run one from the CLI: mc-keygen --benchmark</div></div>
        ) : (
          <div className="panel" style={{ overflow: 'auto' }}>
            <table>
              <thead><tr><th>Date</th><th>Backend</th><th>Prefix</th><th>Keys/s</th><th>Attempts</th><th>Runtime</th><th>Found</th><th>Default</th></tr></thead>
              <tbody>
                {benchmarks.map(b => (
                  <tr key={b.id}>
                    <td className="tabular">{new Date(Number(b.created_at) * 1000).toLocaleDateString()}</td>
                    <td><DeviceBadge backend={b.backend} device={b.device} /></td>
                    <td className="mono">{b.target_prefix} ({b.prefix_length}c)</td>
                    <td className="tabular mono">{formatKps(b.keys_per_second)}</td>
                    <td className="tabular mono">{formatNum(b.attempts)}</td>
                    <td className="tabular mono"><TimeDisplay seconds={b.elapsed_seconds} /></td>
                    <td>{b.found ? <span className="text-green">Yes</span> : <span className="text-amber">Timeout</span>}</td>
                    <td>{b.is_default ? <span className="status-badge completed">Default</span> : ''}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )
      )}
    </div>
  );
}
