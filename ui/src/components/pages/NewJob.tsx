import { useState, useEffect, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { api, Estimate } from '../../api';
import { PrefixBadge, TimeDisplay, formatKps, formatNum, formatPct } from '../shared';

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
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ prefixes: validPrefixes, backend }), signal: controller.signal,
      }).then(r => r.json()).then(setEstimate).catch(() => {});
    }, 300);
    return () => { clearTimeout(tid); abortRef.current?.abort(); };
  }, [prefixes, backend]);

  const submit = async (startNow: boolean) => {
    if (validPrefixes.length === 0) return;
    setSubmitting(true); setError('');
    try {
      await api.createJob({ name: name || undefined, prefixes: validPrefixes, backend, max_attempts: maxAttempts ? Number(maxAttempts) : undefined, max_runtime: maxRuntime ? Number(maxRuntime) : undefined, notes: notes || undefined });
      nav('/queue');
    } catch (e: any) { setError(e.message); setSubmitting(false); }
  };

  return (
    <div>
      <div className="content-header"><h1>New Job</h1></div>
      {error && <div className="error-banner">{error}</div>}
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 20, alignItems: 'start' }}>
        {/* Left: form */}
        <div className="panel">
          <div className="form-group">
            <label className="form-label">Hex Prefix(es)</label>
            <input className="form-input mono" placeholder="e.g. C0DE BEEF or C0DEBA5ED" value={prefixes} onChange={e => setPrefixes(e.target.value)} />
            <div className="form-hint">Separate multiple prefixes with spaces or commas</div>
          </div>
          {hasError && <div className="error-banner">Invalid hex: {prefixList.filter(p => !/^[0-9A-F]+$/.test(p)).join(', ')}</div>}
          <div className="form-group">
            <label className="form-label">Name (optional)</label>
            <input className="form-input" value={name} onChange={e => setName(e.target.value)} />
          </div>
          <div className="form-group">
            <label className="form-label">Backend</label>
            <select className="form-input" value={backend} onChange={e => setBackend(e.target.value)}>
              <option value="cpu">CPU</option>
              <option value="cuda">CUDA GPU</option>
            </select>
          </div>
          <div className="form-row">
            <div className="form-group">
              <label className="form-label">Max Attempts</label>
              <input className="form-input mono" type="number" value={maxAttempts} onChange={e => setMaxAttempts(e.target.value)} placeholder="Unlimited" />
            </div>
            <div className="form-group">
              <label className="form-label">Max Runtime (s)</label>
              <input className="form-input mono" type="number" value={maxRuntime} onChange={e => setMaxRuntime(e.target.value)} placeholder="Unlimited" />
            </div>
          </div>
          <div className="form-group">
            <label className="form-label">Notes</label>
            <input className="form-input" value={notes} onChange={e => setNotes(e.target.value)} />
          </div>
          <div style={{ display: 'flex', gap: 8, marginTop: 16 }}>
            <button className="primary" onClick={() => submit(false)} disabled={validPrefixes.length === 0 || submitting}>
              {submitting ? 'Creating...' : 'Create & Queue'}
            </button>
          </div>
        </div>

        {/* Right: estimate */}
        <div className="panel" style={{ position: 'sticky', top: 12 }}>
          <div className="panel-header"><span className="panel-title">Live Estimate</span></div>
          {estimate ? (
            <>
              {validPrefixes.map(p => <div key={p} style={{ marginBottom: 6 }}><PrefixBadge prefix={p} /></div>)}
              <div style={{ fontSize: 11, color: 'var(--text-muted)' }}>{estimate.backend}{estimate.device ? ` · ${estimate.device}` : ''}</div>
              <div className="grid grid-2" style={{ marginTop: 12 }}>
                <div><span className="text-muted" style={{ fontSize: 11 }}>Expected</span><div className="tabular">{formatNum(estimate.expected_attempts)}</div></div>
                <div><span className="text-muted" style={{ fontSize: 11 }}>Keys/s</span><div className="tabular">{formatKps(estimate.keys_per_second)}</div></div>
                <div><span className="text-muted" style={{ fontSize: 11 }}>50%</span><div className="tabular"><TimeDisplay seconds={estimate.milestone_50pct_seconds} /></div></div>
                <div><span className="text-muted" style={{ fontSize: 11 }}>90%</span><div className="tabular"><TimeDisplay seconds={estimate.milestone_90pct_seconds} /></div></div>
                <div><span className="text-muted" style={{ fontSize: 11 }}>95%</span><div className="tabular"><TimeDisplay seconds={estimate.milestone_95pct_seconds} /></div></div>
                <div><span className="text-muted" style={{ fontSize: 11 }}>99%</span><div className="tabular"><TimeDisplay seconds={estimate.milestone_99pct_seconds} /></div></div>
              </div>
              {estimate.prefix_length >= 9 && (
                <div className="error-banner" style={{ background: 'var(--amber-dim)', borderColor: 'var(--amber)', color: 'var(--amber)', marginTop: 12 }}>
                  {estimate.prefix_length}-char prefix — {formatNum(estimate.expected_attempts)} attempts expected
                </div>
              )}
            </>
          ) : (
            <div style={{ color: 'var(--text-muted)', fontSize: 13 }}>
              Type a hex prefix to see live estimates
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
