import { useState } from 'react';

export function MetricCard({ label, value, size = 'normal', color, subtitle }: {
  label: string; value: React.ReactNode; size?: 'small' | 'normal' | 'hero'; color?: string; subtitle?: string;
}) {
  return (
    <div className={`metric-card${size !== 'normal' ? ' ' + size : ''}`}>
      <div className="label">{label}</div>
      <div className="value" style={color ? { color } : undefined}>{value}</div>
      {subtitle && <div style={{ fontSize: 11, color: 'var(--text-muted)', marginTop: 2 }}>{subtitle}</div>}
    </div>
  );
}

export function StatusBadge({ status }: { status: string }) {
  const s = status.toLowerCase();
  return (
    <span className={`status-badge ${s}`}>
      <span className="dot" /> {status}
    </span>
  );
}

export function PrefixBadge({ prefix }: { prefix: string }) {
  return <span className="prefix-badge">{prefix.toUpperCase()}</span>;
}

export function DeviceBadge({ device, backend }: { device?: string; backend?: string }) {
  return <span className="device-badge">{backend || 'cpu'}{device ? ` (${device})` : ''}</span>;
}

export function CopyButton({ text, label = 'Copy' }: { text: string; label?: string }) {
  const copy = () => {
    navigator.clipboard.writeText(text).catch(() => {});
  };
  return <button className="ghost" onClick={copy} style={{ fontSize: 11, padding: '2px 8px' }}>{label}</button>;
}

export function ProgressBar({ pct, markers }: { pct: number; markers?: { at: number; label: string }[] }) {
  return (
    <div style={{ position: 'relative' }}>
      <div className="progress-bar">
        <div className="fill" style={{ width: `${Math.min(pct, 100)}%` }} />
        {markers?.map(m => (
          <div key={m.at} className="marker" style={{ left: `${m.at}%` }}>
            <span className="marker-label">{m.label}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

export function EmptyState({ icon, title, desc, action }: { icon: string; title: string; desc: string; action?: React.ReactNode }) {
  return (
    <div className="empty-state">
      <div className="icon">{icon}</div>
      <div className="title">{title}</div>
      <div className="desc">{desc}</div>
      {action}
    </div>
  );
}

export function ErrorBanner({ message }: { message: string }) {
  return <div className="error-banner">{message}</div>;
}

export function TimeDisplay({ seconds, long = false }: { seconds: number; long?: boolean }) {
  if (!isFinite(seconds) || seconds < 0) return <span className="tabular">-</span>;
  if (seconds === Infinity) return <span className="tabular">&infin;</span>;
  if (seconds < 60) return <span className="tabular">{seconds.toFixed(1)}s</span>;
  if (seconds < 3600) return <span className="tabular">{Math.floor(seconds / 60)}m {Math.floor(seconds % 60)}s</span>;
  if (seconds < 86400) return <span className="tabular">{Math.floor(seconds / 3600)}h {Math.floor((seconds % 3600) / 60)}m</span>;
  return <span className="tabular">{Math.floor(seconds / 86400)}d {Math.floor((seconds % 86400) / 3600)}h</span>;
}

export function formatKps(n: number): string {
  if (n >= 1e9) return (n / 1e9).toFixed(2) + 'G';
  if (n >= 1e6) return (n / 1e6).toFixed(1) + 'M';
  if (n >= 1e3) return (n / 1e3).toFixed(1) + 'K';
  return n.toFixed(0);
}

export function formatNum(n: number): string {
  if (n >= 1e12) return (n / 1e12).toFixed(2) + 'T';
  if (n >= 1e9) return (n / 1e9).toFixed(2) + 'B';
  if (n >= 1e6) return (n / 1e6).toFixed(2) + 'M';
  if (n >= 1e3) return n.toLocaleString();
  return n.toFixed(0);
}

export function formatPct(p: number): string {
  if (p < 0.0001) return '<0.01%';
  if (p < 0.01) return (p * 100).toFixed(4) + '%';
  if (p < 1) return (p * 100).toFixed(2) + '%';
  if (p < 99.995) return (p * 100).toFixed(1) + '%';
  return (p * 100).toFixed(3) + '%';
}

export function formatProb(attempts: number, expected: number): string {
  if (expected === 0) return '0%';
  return formatPct(1 - Math.exp(-attempts / expected));
}

export function SecretField({ value, label = 'Private Key' }: { value: string; label?: string }) {
  const [show, setShow] = useState(false);
  return (
    <div style={{ marginTop: 8 }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 4 }}>
        <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>{label}</span>
        <button
          className="ghost"
          style={{ fontSize: 11, padding: '2px 8px', color: show ? 'var(--red)' : 'var(--text-secondary)' }}
          onClick={() => setShow(!show)}
        >
          {show ? 'Hide' : 'Reveal'}
        </button>
        {show && <CopyButton text={value} />}
      </div>
      {show ? (
        <div className="secret-field">{value}</div>
      ) : (
        <div className="secret-field" style={{ color: 'var(--text-muted)', borderColor: 'var(--border-default)' }}>
          ●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●●
        </div>
      )}
    </div>
  );
}
