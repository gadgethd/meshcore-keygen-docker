import { useState, useEffect } from 'react';
import { api, Result } from '../../api';
import { PrefixBadge, SecretField, CopyButton, TimeDisplay, formatNum } from '../shared';

export default function Results() {
  const [results, setResults] = useState<Result[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const load = () => api.results().then(r => { setResults(r); setLoading(false); }).catch(() => {});
    load();
    const i = setInterval(load, 10000);
    return () => clearInterval(i);
  }, []);

  return (
    <div>
      <div className="content-header"><h1>Results</h1></div>
      <div style={{ display: 'flex', gap: 16, marginBottom: 20 }}>
        <div className="metric-card small"><div className="label">Total</div><div className="value">{results.length}</div></div>
        <div className="metric-card small"><div className="label">Fastest</div><div className="value mono">{results.length > 0 ? results.reduce((a,b) => a.elapsed_seconds < b.elapsed_seconds ? a : b).elapsed_seconds.toFixed(2) + 's' : '-'}</div></div>
        <div className="metric-card small"><div className="label">Longest</div><div className="value mono">{results.length > 0 ? results.reduce((a,b) => a.attempts > b.attempts ? a : b).attempts.toLocaleString() : '-'}</div></div>
      </div>
      {loading && results.length === 0 ? (
        <div className="panel"><div className="skeleton skeleton-text" /><div className="skeleton skeleton-text short" /></div>
      ) : results.length === 0 ? (
        <div className="panel"><div style={{ textAlign: 'center', padding: 40, color: 'var(--text-muted)' }}>No results yet</div></div>
      ) : (
        results.map(r => (
          <div key={r.id} className="panel" style={{ marginBottom: 12 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 8 }}>
              <PrefixBadge prefix={r.prefix} />
              <span className="device-badge">{r.backend}</span>
              <span className="text-muted" style={{ fontSize: 12 }}>{formatNum(r.attempts)} attempts · <TimeDisplay seconds={r.elapsed_seconds} /></span>
            </div>
            <div style={{ fontSize: 13, fontFamily: '"JetBrains Mono", monospace', color: 'var(--text-secondary)', display: 'flex', alignItems: 'center', gap: 8 }}>
              <span className="text-muted">Public:</span> {r.public_key.slice(0, 16)}...{r.public_key.slice(-4)}
              <CopyButton text={r.public_key} />
            </div>
            <SecretField value={r.private_key} label="Private Key" />
          </div>
        ))
      )}
    </div>
  );
}
