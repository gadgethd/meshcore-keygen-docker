import { useState, useEffect, useRef } from 'react';
import { api, SettingsData } from '../../api';

export default function Settings() {
  const [s, setS] = useState<SettingsData | null>(null);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState('');
  const savedT = useRef<ReturnType<typeof setTimeout>>();
  const debounceT = useRef<ReturnType<typeof setTimeout>>();

  useEffect(() => { api.settings().then(setS).catch(() => setError('Failed to load')); }, []);

  const update = async (k: string, v: unknown) => {
    if (!s) return; const prev = { ...s }; setS({ ...s, [k]: v }); setSaved(false);
    if (debounceT.current) clearTimeout(debounceT.current);
    debounceT.current = setTimeout(async () => {
      try { await api.updateSettings({ [k]: v }); setError(''); setSaved(true); if (savedT.current) clearTimeout(savedT.current); savedT.current = setTimeout(() => setSaved(false), 2000); }
      catch (e: any) { setS(prev); setError(`Save failed: ${e.message}`); }
    }, 500);
  };

  if (!s) return <div className="glass-card"><div className="skeleton skeleton-value" /></div>;

  return (
    <div>
      <div className="content-header"><h1>Settings</h1></div>
      {saved && <div style={{ color: 'var(--success)', fontSize: 13, marginBottom: 12 }}>Saved</div>}
      {error && <div className="error-banner">{error}</div>}
      <div className="grid grid-2">
        <div className="glass-card">
          <div className="panel-header"><span className="panel-title">General</span></div>
          <Field label="Timezone" v={s.timezone} set={v => update('timezone', v)} />
          <Field label="Max Log Lines" v={s.max_log_lines} set={v => update('max_log_lines', Number(v))} type="number" />
          <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13 }}>
            <input type="checkbox" checked={s.hide_secrets} onChange={e => update('hide_secrets', e.target.checked)} /> Hide secrets by default
          </label>
        </div>
        <div className="glass-card">
          <div className="panel-header"><span className="panel-title">Backend & CPU</span></div>
          <Field label="Default Backend" v={s.default_backend} set={v => update('default_backend', v)} />
          <Field label="Reserved CPU Cores" v={s.reserved_cpu_cores} set={v => update('reserved_cpu_cores', Number(v))} type="number" />
          <Field label="Max Worker Threads" v={s.max_worker_threads ?? ''} set={v => update('max_worker_threads', v ? Number(v) : null)} type="number" placeholder="Auto" />
          <Field label="Checkpoint Interval (s)" v={s.checkpoint_interval_secs} set={v => update('checkpoint_interval_secs', Number(v))} type="number" />
        </div>
        <div className="glass-card">
          <div className="panel-header"><span className="panel-title">Global Schedule</span></div>
          <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13, marginBottom: 14 }}>
            <input type="checkbox" checked={s.schedule_enabled} onChange={e => update('schedule_enabled', e.target.checked)} /> Enable schedule
          </label>
          <div className="form-row">
            <div className="form-group">
              <label className="form-label">Start (HH:MM)</label>
              <input className="form-input" value={s.schedule_start} onChange={e => update('schedule_start', e.target.value)} placeholder="23:00" />
            </div>
            <div className="form-group">
              <label className="form-label">End (HH:MM)</label>
              <input className="form-input" value={s.schedule_end} onChange={e => update('schedule_end', e.target.value)} placeholder="07:00" />
            </div>
          </div>
          <div className="form-hint">Keygen runs only during this window. If start &gt; end, window crosses midnight (e.g. 23:00–07:00 = overnight).</div>
          {s.schedule_enabled && (
            <div className="warning-banner" style={{ marginTop: 12 }}>
              Queue auto-paused outside window · Jobs won't start until {s.schedule_start}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function Field({ label, v, set, type = 'text', placeholder }: { label: string; v: string | number; set: (s: string) => void; type?: string; placeholder?: string }) {
  return (
    <div className="form-group">
      <label className="form-label">{label}</label>
      <input className="form-input" type={type} value={v} onChange={e => set(e.target.value)} placeholder={placeholder} />
    </div>
  );
}
