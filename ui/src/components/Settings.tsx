import { useEffect, useState, useRef, useCallback } from 'react';
import { api, SettingsData } from '../api';

export default function Settings() {
  const [s, setS] = useState<SettingsData | null>(null);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState('');
  const savedTimer = useRef<ReturnType<typeof setTimeout>>();
  const debounceTimer = useRef<ReturnType<typeof setTimeout>>();

  useEffect(() => { api.settings().then(setS).catch(() => setError('Failed to load settings')); }, []);

  const update = useCallback(async (key: string, value: unknown) => {
    if (!s) return;
    const prev = { ...s };
    const next = { ...s, [key]: value };
    setS(next);
    setSaved(false);

    if (debounceTimer.current) clearTimeout(debounceTimer.current);
    debounceTimer.current = setTimeout(async () => {
      try {
        await api.updateSettings({ [key]: value });
        setError('');
        setSaved(true);
        if (savedTimer.current) clearTimeout(savedTimer.current);
        savedTimer.current = setTimeout(() => setSaved(false), 2000);
      } catch (e: any) {
        setS(prev); // rollback
        setError(`Save failed: ${e.message}`);
      }
    }, 500);
  }, [s]);

  if (!s) return <div style={{ color: error ? '#f85149' : '#8b949e' }}>{error || 'Loading...'}</div>;

  return (
    <div>
      <h2 style={{ marginBottom: 16 }}>Settings</h2>
      {saved && <div style={{ color: '#238636', marginBottom: 12 }}>Saved</div>}
      {error && <div style={{ color: '#f85149', marginBottom: 12 }}>{error}</div>}
      <div className="card" style={{ maxWidth: 500 }}>
        <Field label="Reserved CPU Cores" value={s.reserved_cpu_cores} onChange={v => update('reserved_cpu_cores', Number(v))} type="number" />
        <Field label="Max Worker Thread" value={s.max_worker_threads ?? ''} onChange={v => update('max_worker_threads', v ? Number(v) : null)} type="number" />
        <Field label="Checkpoint Interval (s)" value={s.checkpoint_interval_secs} onChange={v => update('checkpoint_interval_secs', Number(v))} type="number" />
        <Field label="Default Backend" value={s.default_backend} onChange={v => update('default_backend', v)} />
        <Field label="Timezone" value={s.timezone} onChange={v => update('timezone', v)} />
        <label style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 12 }}>
          <input type="checkbox" checked={s.hide_secrets} onChange={e => update('hide_secrets', e.target.checked)} />
          <span style={{ fontSize: 14 }}>Hide secrets by default</span>
        </label>
        <Field label="Max Log Lines" value={s.max_log_lines} onChange={v => update('max_log_lines', Number(v))} type="number" />
      </div>
    </div>
  );
}

function Field({ label, value, onChange, type = 'text' }: { label: string; value: string | number; onChange: (v: string) => void; type?: string }) {
  return (
    <div style={{ marginBottom: 12 }}>
      <label style={{ display: 'block', marginBottom: 4, color: '#8b949e', fontSize: 12 }}>{label}</label>
      <input style={{ width: '100%' }} type={type} value={value} onChange={e => onChange(e.target.value)} />
    </div>
  );
}
