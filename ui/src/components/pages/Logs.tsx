import { useState, useEffect, useRef } from 'react';

interface LogEntry {
  id: number;
  timestamp: string;
  level: string;
  job_id?: string;
  message: string;
}

export default function Logs() {
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [level, setLevel] = useState('all');
  const [jobId, setJobId] = useState('');
  const [autoScroll, setAutoScroll] = useState(true);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const load = () => {
      const params = new URLSearchParams();
      params.set('limit', '300');
      if (jobId) params.set('job_id', jobId);
      fetch(`/api/logs?${params}`)
        .then(r => r.json())
        .then(data => { setLogs(Array.isArray(data) ? data : []); setLoading(false); })
        .catch(() => {});
    };
    load();
    const i = setInterval(load, 3000);
    return () => clearInterval(i);
  }, [jobId]);

  useEffect(() => {
    if (autoScroll && bottomRef.current) {
      bottomRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [logs, autoScroll]);

  const filtered = level === 'all' ? logs : logs.filter(l => l.level === level);

  return (
    <div>
      <div className="content-header">
        <h1>Logs</h1>
        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
          <label style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 12, color: 'var(--text-secondary)', cursor: 'pointer' }}>
            <input type="checkbox" checked={autoScroll} onChange={e => setAutoScroll(e.target.checked)} />
            Auto-scroll
          </label>
          <select value={level} onChange={e => setLevel(e.target.value)} style={{ fontSize: 12, padding: '4px 10px' }}>
            <option value="all">All levels</option>
            <option value="info">Info</option>
            <option value="warn">Warn</option>
            <option value="error">Error</option>
          </select>
          <input
            value={jobId}
            onChange={e => setJobId(e.target.value)}
            placeholder="Filter by job ID..."
            style={{ fontSize: 12, padding: '4px 10px', width: 200, fontFamily: 'var(--font-mono)' }}
          />
        </div>
      </div>

      {loading && logs.length === 0 ? (
        <div className="glass-card"><div className="skeleton skeleton-text" /><div className="skeleton skeleton-text short" /></div>
      ) : logs.length === 0 ? (
        <div className="glass-card"><div className="empty-state"><div className="icon">☷</div><div className="title">No logs yet</div><div className="desc">Job lifecycle events, errors, and checkpoint activity appear here</div></div></div>
      ) : (
        <div className="glass-card" style={{ padding: 0, overflow: 'hidden', fontFamily: 'var(--font-mono)' }}>
          <div style={{
            maxHeight: 'calc(100vh - 220px)', overflowY: 'auto',
            padding: '12px 16px', fontSize: 12, lineHeight: 1.9,
          }}>
            {filtered.map(l => (
              <div key={l.id} style={{
                display: 'flex', gap: 14, padding: '1px 0',
                color: l.level === 'error' ? 'var(--danger)' : l.level === 'warn' ? 'var(--warning)' : 'var(--text-secondary)',
              }}>
                <span className="tabular" style={{ color: 'var(--text-muted)', minWidth: 130, flexShrink: 0 }}>
                  {new Date(Number(l.timestamp) * 1000).toLocaleString()}
                </span>
                <span style={{
                  minWidth: 44, flexShrink: 0, fontWeight: 600,
                  textTransform: 'uppercase', fontSize: 10, letterSpacing: '0.5px',
                }}>{l.level}</span>
                {l.job_id && (
                  <span style={{ color: 'var(--accent)', minWidth: 80, flexShrink: 0, overflow: 'hidden', textOverflow: 'ellipsis' }}>
                    {l.job_id.slice(0, 8)}...
                  </span>
                )}
                <span style={{ flex: 1, wordBreak: 'break-all' }}>{l.message}</span>
              </div>
            ))}
            <div ref={bottomRef} />
          </div>
        </div>
      )}
    </div>
  );
}
