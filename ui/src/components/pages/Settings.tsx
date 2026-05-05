import { useState, useEffect, useRef } from 'react';
import { api, SettingsData } from '../../api';

export default function Settings() {
  const [s, setS] = useState<SettingsData | null>(null);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState('');
  const savedTimer = useRef<ReturnType<typeof setTimeout>>();
  const debounceTimer = useRef<ReturnType<typeof setTimeout>>();

  useEffect(() => { api.settings().then(setS).catch(() => setError('Failed to load')); }, []);

  const update = async (key: string, value: unknown) => {
    if (!s) return;
    const prev = { ...s };
    setS({ ...s, [key]: value }); setSaved(false);
    if (debounceTimer.current) clearTimeout(debounceTimer.current);
    debounceTimer.current = setTimeout(async () => {
      try {
        await api.updateSettings({ [key]: value }); setError(''); setSaved(true);
        if (savedTimer.current) clearTimeout(savedTimer.current);
        savedTimer.current = setTimeout(() => setSaved(false), 2000);
      } catch (e: any) { setS(prev); setError(`Save failed: ${e.message}`); }
    }, 500);
  };

  if (!s) return <div className="skeleton skeleton-value" />;

  return (
    <div>
      <div className="content-header"><h1>Settings</h1></div>
      {saved && <div style={{ color: 'var(--green)', fontSize: 13, marginBottom: 12 }}>Saved</div>}
      {error && <div className="error-banner">{error}</div>}
      <div className="grid grid-2">
        <div className="panel">
          <div className="panel-header"><span className="panel-title">General</span></div>
          <Field label="Timezone" value={s.timezone} onChange={v => update('timezone', v)} />
          <Field label="Max Log Lines" value={s.max_log_lines} onChange={v => update('max_log_lines', Number(v))} type="number" />
          <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13 }}>
            <input type="checkbox" checked={s.hide_secrets} onChange={e => update('hide_secrets', e.target.checked)} />
            Hide secrets by default
          </label>
        </div>
        <div className="panel">
          <div className="panel-header"><span className="panel-title">Backend & CPU</span></div>
          <Field label="Default Backend" value={s.default_backend} onChange={v => update('default_backend', v)} />
          <Field label="Reserved CPU Cores" value={s.reserved_cpu_cores} onChange={v => update('reserved_cpu_cores', Number(v))} type="number" />
          <Field label="Max Worker Threads" value={s.max_worker_threads ?? ''} onChange={v => update('max_worker_threads', v ? Number(v) : null)} type="number" placeholder="Auto" />
          <Field label="Checkpoint Interval (s)" value={s.checkpoint_interval_secs} onChange={v => update('checkpoint_interval_secs', Number(v))} type="number" />
        </div>
      </div>
    </div>
  );
}

function Field({ label, value, onChange, type = 'text', placeholder }: { label: string; value: string | number; onChange: (v: string) => void; type?: string; placeholder?: string }) {
  return (
    <div className="form-group">
      <label className="form-label">{label}</label>
      <input className="form-input" type={type} value={value} onChange={e => onChange(e.target.value)} placeholder={placeholder} />
    </div>
  );
}
