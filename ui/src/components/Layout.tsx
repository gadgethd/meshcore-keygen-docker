import { Link, useLocation } from 'react-router-dom';

const nav = [
  { to: '/', label: 'Dashboard' },
  { to: '/new', label: 'New Job' },
  { to: '/queue', label: 'Queue' },
  { to: '/results', label: 'Results' },
  { to: '/settings', label: 'Settings' },
];

export default function Layout({ children }: { children: React.ReactNode }) {
  const loc = useLocation();
  return (
    <div style={{ minHeight: '100vh', display: 'flex', flexDirection: 'column' }}>
      <header style={{ background: '#161b22', borderBottom: '1px solid #30363d', padding: '12px 24px', display: 'flex', alignItems: 'center', gap: 24 }}>
        <span style={{ fontWeight: 700, fontSize: 18, color: '#58a6ff' }}>mc-keygen</span>
        <nav style={{ display: 'flex', gap: 16 }}>
          {nav.map(n => (
            <Link key={n.to} to={n.to} style={{
              color: loc.pathname === n.to ? '#f0f6fc' : '#8b949e',
              fontWeight: loc.pathname === n.to ? 600 : 400,
              fontSize: 14,
            }}>{n.label}</Link>
          ))}
        </nav>
      </header>
      <main style={{ flex: 1, padding: 24, maxWidth: 1200, margin: '0 auto', width: '100%' }}>
        {children}
      </main>
    </div>
  );
}
