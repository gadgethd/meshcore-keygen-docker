import { useEffect, useState } from 'react';
import { api, SettingsData } from '../api';

export default function Settings() {
  const [s, setS] = useState<SettingsData | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => { api.settings().then(setS).catch(() => {}); }, []);

  const update = async (key: string, value: any) => {
    if (!s) return;
    const next = { ...s, [key]: value };
    setS(next);
    await api.updateSettings({ [key]: value });
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  if (!s) return <div>Loading...</div>;

  return (
    <div>
      <h2 style={{ marginBottom: 16 }}>Settings</h2>
      {saved && <div style={{ color: '#238636', marginBottom: 12 }}>Saved</div>}
      <div className="card" style={{ maxWidth: 500 }}>
        <Field label="Reserved CPU Cores" value={s.reserved_cpu_cores} onChange={v => update('reserved_cpu_cores', Number(v))} type="number" />
        <Field label="Max Worker Threads" value={s.max_worker_threads ?? ''} onChange={v => update('max_worker_threads', v ? Number(v) : null)} type="number" />
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
