import { useEffect, useState } from 'react';
import { api, Result } from '../api';

export default function Results() {
  const [results, setResults] = useState<Result[]>([]);
  const [revealed, setRevealed] = useState<Set<string>>(new Set());
  useEffect(() => { api.results().then(setResults).catch(() => {}); }, []);

  const toggle = (id: string) => {
    setRevealed(prev => { const next = new Set(prev); if (next.has(id)) next.delete(id); else next.add(id); return next; });
  };

  return (
    <div>
      <h2 style={{ marginBottom: 16 }}>Results</h2>
      {results.length === 0 && <div style={{ color: '#8b949e' }}>No results yet.</div>}
      <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
        {results.map(r => (
          <div key={r.id} className="card">
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 8 }}>
              <span style={{ fontWeight: 600, color: '#58a6ff' }}>{r.prefix}</span>
              <span style={{ fontSize: 12, color: '#8b949e' }}>{r.backend} &middot; {r.attempts.toLocaleString()} att &middot; {r.elapsed_seconds.toFixed(1)}s</span>
            </div>
            <div style={{ fontSize: 13, fontFamily: 'monospace', color: '#8b949e' }}>
              Public: {r.public_key}
            </div>
            {revealed.has(r.id) ? (
              <div style={{ fontSize: 13, fontFamily: 'monospace', color: '#f85149', marginTop: 4 }}>
                Private: {r.private_key}
              </div>
            ) : (
              <button onClick={() => toggle(r.id)} style={{ marginTop: 4, fontSize: 12, background: '#30363d', color: '#8b949e' }}>
                Show Private Key
              </button>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
