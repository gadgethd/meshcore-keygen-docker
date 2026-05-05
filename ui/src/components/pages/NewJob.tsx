import { useState, useEffect, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { api, Estimate } from '../../api';
import { PrefixBadge, TimeDisplay, WarningBanner, formatKps, formatNum } from '../shared';

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
  const nonHex = prefixList.filter(p => !/^[0-9A-F]+$/.test(p));
  const reserved = prefixList.filter(p => /^[0-9A-F]+$/.test(p) && (p.startsWith('00') || p.startsWith('FF')));
  const tooLong = prefixList.filter(p => /^[0-9A-F]+$/.test(p) && p.length > 64);
  const validPrefixes = prefixList.filter(p => /^[0-9A-F]+$/.test(p) && p.length <= 64 && !p.startsWith('00') && !p.startsWith('FF'));
  const errors: string[] = [];
  if (nonHex.length > 0) errors.push(`Invalid hex: ${nonHex.join(', ')}`);
  if (reserved.length > 0) errors.push(`Reserved prefix (00/FF): ${reserved.join(', ')}`);
  if (tooLong.length > 0) errors.push(`Too long (>64): ${tooLong.join(', ')}`);
  const hasError = errors.length > 0;

  useEffect(() => {
    if (validPrefixes.length === 0) { setEstimate(null); return; }
    abortRef.current?.abort();
    const c = new AbortController(); abortRef.current = c;
    const tid = setTimeout(() => {
      fetch('/api/estimate', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ prefixes: validPrefixes, backend }), signal: c.signal })
        .then(r => r.json()).then(setEstimate).catch(() => {});
    }, 300);
    return () => { clearTimeout(tid); abortRef.current?.abort(); };
  }, [prefixes, backend]);

  const submit = async () => {
    if (validPrefixes.length === 0) return;
    setSubmitting(true); setError('');
    try { await api.createJob({ name: name || undefined, prefixes: validPrefixes, backend, max_attempts: maxAttempts ? Number(maxAttempts) : undefined, max_runtime: maxRuntime ? Number(maxRuntime) : undefined, notes: notes || undefined }); nav('/queue'); }
    catch (e: any) { setError(e.message); setSubmitting(false); }
  };

  return (
    <div>
      <div className="content-header"><h1>New Search</h1></div>
      {error && <div className="error-banner">{error}</div>}
      <div className="grid grid-2">
        <div className="glass-card">
          <div className="form-group">
            <label className="form-label">Hex Prefix(es)</label>
            <input className="form-input mono" placeholder="e.g. C0DE BEEF or C0DEBA5ED" value={prefixes} onChange={e => setPrefixes(e.target.value)} style={{ fontSize: 14, fontFamily: 'var(--font-mono)', letterSpacing: 1 }} />
            <div className="form-hint">Separate with spaces or commas</div>
          </div>
          {hasError && errors.map((e, i) => <div key={i} className="error-banner" style={{ marginTop: i > 0 ? 4 : 8, marginBottom: 8 }}>{e}</div>)}
          <div className="form-group"><label className="form-label">Name</label><input className="form-input" value={name} onChange={e => setName(e.target.value)} placeholder="Optional" /></div>
          <div className="form-group"><label className="form-label">Backend</label><select className="form-input" value={backend} onChange={e => setBackend(e.target.value)}><option value="cpu">CPU</option><option value="cuda">CUDA GPU</option></select></div>
          <div className="form-row">
            <div className="form-group"><label className="form-label">Max Attempts</label><input className="form-input mono" type="number" value={maxAttempts} onChange={e => setMaxAttempts(e.target.value)} placeholder="Unlimited" /></div>
            <div className="form-group"><label className="form-label">Max Runtime (s)</label><input className="form-input mono" type="number" value={maxRuntime} onChange={e => setMaxRuntime(e.target.value)} placeholder="Unlimited" /></div>
          </div>
          <div className="form-group"><label className="form-label">Notes</label><input className="form-input" value={notes} onChange={e => setNotes(e.target.value)} /></div>
          <button className="primary" onClick={submit} disabled={validPrefixes.length === 0 || submitting} style={{ marginTop: 8 }}>
            {submitting ? 'Creating...' : 'Create & Queue'}
          </button>
        </div>

        <div className="glass-card" style={{ position: 'sticky', top: 16 }}>
          <div className="panel-header"><span className="panel-title">Live Estimate</span></div>
          {estimate ? (
            <>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, marginBottom: 12 }}>
                {validPrefixes.map(p => <PrefixBadge key={p} prefix={p} />)}
              </div>
              <div style={{ fontSize: 11, color: 'var(--text-muted)', marginBottom: 14 }}>{estimate.backend}{estimate.device ? ` · ${estimate.device}` : ''}</div>
              <div className="grid" style={{ gridTemplateColumns: '1fr 1fr' }}>
                <div><span className="text-muted" style={{ fontSize: 10, textTransform: 'uppercase' }}>Expected</span><div className="tabular" style={{ fontWeight: 600 }}>{formatNum(estimate.expected_attempts)}</div></div>
                <div><span className="text-muted" style={{ fontSize: 10, textTransform: 'uppercase' }}>Keys/s</span><div className="tabular" style={{ fontWeight: 600 }}>{formatKps(estimate.keys_per_second)}</div></div>
                <div style={{ marginTop: 8 }}><span className="text-muted" style={{ fontSize: 10, textTransform: 'uppercase' }}>50%</span><div className="tabular"><TimeDisplay seconds={estimate.milestone_50pct_seconds} /></div></div>
                <div style={{ marginTop: 8 }}><span className="text-muted" style={{ fontSize: 10, textTransform: 'uppercase' }}>90%</span><div className="tabular"><TimeDisplay seconds={estimate.milestone_90pct_seconds} /></div></div>
                <div style={{ marginTop: 8 }}><span className="text-muted" style={{ fontSize: 10, textTransform: 'uppercase' }}>95%</span><div className="tabular"><TimeDisplay seconds={estimate.milestone_95pct_seconds} /></div></div>
                <div style={{ marginTop: 8 }}><span className="text-muted" style={{ fontSize: 10, textTransform: 'uppercase' }}>99%</span><div className="tabular"><TimeDisplay seconds={estimate.milestone_99pct_seconds} /></div></div>
              </div>
              {estimate.prefix_length >= 9 && <WarningBanner message={`${estimate.prefix_length}-char prefix — ${formatNum(estimate.expected_attempts)} expected attempts`} />}
            </>
          ) : (
            <div className="empty-state"><div className="icon">⧩</div><div className="title">Enter a prefix</div><div className="desc">Type a hex prefix above to see live time estimates based on benchmarks</div></div>
          )}
        </div>
      </div>
    </div>
  );
}
