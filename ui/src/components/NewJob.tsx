import { useState, useEffect, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { api, Estimate } from '../api';

function fmtDur(s: number): string {
  if (s < 1) return '<1s';
  if (s < 60) return Math.round(s) + 's';
  if (s < 3600) return Math.floor(s / 60) + 'm ' + Math.round(s % 60) + 's';
  if (s < 86400) return Math.floor(s / 3600) + 'h ' + Math.round((s % 3600) / 60) + 'm';
  return Math.floor(s / 86400) + 'd ' + Math.round((s % 86400) / 3600) + 'h';
}

function fmtSci(n: number): string {
  if (n < 1000) return n.toFixed(0);
  return n.toExponential(1);
}

export default function NewJob() {
  const nav = useNavigate();
  const [prefixes, setPrefixes] = useState('');
  const [name, setName] = useState('');
  const [backend, setBackend] = useState('cpu');
  const [maxAttempts, setMaxAttempts] = useState('');
  const [maxRuntime, setMaxRuntime] = useState('');
  const [notes, setNotes] = useState('');
  const [estimate, setEstimate] = useState<Estimate | null>(null);
  const [error, setError] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const abortRef = useRef<AbortController | null>(null);

  const prefixList = prefixes.split(/[\s,]+/).filter(Boolean).map(p => p.toUpperCase());
  const validPrefixes = prefixList.filter(p => /^[0-9A-F]+$/.test(p) && p.length <= 64 && !p.startsWith('00') && !p.startsWith('FF'));
  const hasError = prefixList.length > 0 && validPrefixes.length !== prefixList.length;

  useEffect(() => {
    if (validPrefixes.length === 0) { setEstimate(null); return; }
    abortRef.current?.abort();
    const controller = new AbortController();
    abortRef.current = controller;
    const tid = setTimeout(() => {
      fetch('/api/estimate', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ prefixes: validPrefixes, backend }),
        signal: controller.signal,
      }).then(r => r.json()).then(setEstimate).catch(() => {});
    }, 300);
    return () => { clearTimeout(tid); abortRef.current?.abort(); };
  }, [prefixes, backend]);

  const submit = async () => {
    if (validPrefixes.length === 0) return;
    setSubmitting(true);
    setError('');
    try {
      await api.createJob({
        name: name || undefined,
        prefixes: validPrefixes,
        backend,
        max_attempts: maxAttempts ? Number(maxAttempts) : undefined,
        max_runtime: maxRuntime ? Number(maxRuntime) : undefined,
        notes: notes || undefined,
      });
      nav('/queue');
    } catch (e: any) {
      setError(e.message);
      setSubmitting(false);
    }
  };

  return (
    <div>
      <h2 style={{ marginBottom: 16 }}>New Job</h2>
      <div className="card" style={{ maxWidth: 600 }}>
        <label style={{ display: 'block', marginBottom: 4, color: '#8b949e', fontSize: 12 }}>Hex Prefix(es)</label>
        <input
          style={{ width: '100%', marginBottom: 12 }}
          placeholder="e.g. C0DE BEEF or C0DEBA5ED"
          value={prefixes}
          onChange={e => setPrefixes(e.target.value)}
        />
        {hasError && <div style={{ color: '#f85149', fontSize: 12, marginBottom: 12 }}>Invalid hex in: {prefixList.filter(p => !/^[0-9A-F]+$/.test(p)).join(', ')}</div>}
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, marginBottom: 12 }}>
          <div>
            <label style={{ display: 'block', marginBottom: 4, color: '#8b949e', fontSize: 12 }}>Backend</label>
            <select value={backend} onChange={e => setBackend(e.target.value)} style={{ width: '100%' }}>
              <option value="cpu">CPU</option>
              <option value="cuda">CUDA GPU</option>
            </select>
          </div>
          <div>
            <label style={{ display: 'block', marginBottom: 4, color: '#8b949e', fontSize: 12 }}>Name (optional)</label>
            <input style={{ width: '100%' }} value={name} onChange={e => setName(e.target.value)} />
          </div>
          <div>
            <label style={{ display: 'block', marginBottom: 4, color: '#8b949e', fontSize: 12 }}>Max Attempts</label>
            <input style={{ width: '100%' }} type="number" value={maxAttempts} onChange={e => setMaxAttempts(e.target.value)} />
          </div>
          <div>
            <label style={{ display: 'block', marginBottom: 4, color: '#8b949e', fontSize: 12 }}>Max Runtime (s)</label>
            <input style={{ width: '100%' }} type="number" value={maxRuntime} onChange={e => setMaxRuntime(e.target.value)} />
          </div>
        </div>
        <label style={{ display: 'block', marginBottom: 4, color: '#8b949e', fontSize: 12 }}>Notes</label>
        <input style={{ width: '100%', marginBottom: 16 }} value={notes} onChange={e => setNotes(e.target.value)} />
        <button onClick={submit} disabled={validPrefixes.length === 0 || submitting}>
          {submitting ? 'Creating...' : 'Create Job'}
        </button>
        {error && <div style={{ color: '#f85149', marginTop: 8 }}>{error}</div>}
      </div>
      {estimate && (
        <div className="card" style={{ marginTop: 16, maxWidth: 600 }}>
          <h3 style={{ marginBottom: 12 }}>Estimate ({estimate.backend}{estimate.device ? ` - ${estimate.device}` : ''})</h3>
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(140px, 1fr))', gap: 12 }}>
            <Stat label="Prefix Length" value={`${estimate.prefix_length} chars`} />
            <Stat label="Expected" value={fmtSci(estimate.expected_attempts) + ' attempts'} />
            <Stat label="Keys/s" value={fmtSci(estimate.keys_per_second)} />
            <Stat label="50%" value={fmtDur(estimate.milestone_50pct_seconds)} />
            <Stat label="90%" value={fmtDur(estimate.milestone_90pct_seconds)} />
            <Stat label="95%" value={fmtDur(estimate.milestone_95pct_seconds)} />
            <Stat label="99%" value={fmtDur(estimate.milestone_99pct_seconds)} />
          </div>
          {estimate.prefix_length >= 9 && (
            <div style={{ marginTop: 12, color: '#d29922', fontSize: 13 }}>
              Warning: {estimate.prefix_length}-char prefix is computationally expensive ({fmtSci(estimate.expected_attempts)} attempts)
            </div>
          )}
        </div>
      )}
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
