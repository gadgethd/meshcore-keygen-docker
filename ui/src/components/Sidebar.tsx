import { Link, useLocation } from 'react-router-dom';

interface NavItem { to: string; icon: string; label: string; }

const items: NavItem[] = [
  { to: '/', icon: '⊞', label: 'Dashboard' },
  { to: '/new', icon: '+', label: 'New Job' },
  { to: '/active', icon: '▶', label: 'Active Job' },
  { to: '/queue', icon: '☰', label: 'Queue' },
  { to: '/results', icon: '✓', label: 'Results' },
  { to: '/benchmarks', icon: '⏱', label: 'Benchmarks' },
  { to: '/schedules', icon: '↺', label: 'Schedules' },
  { to: '/settings', icon: '⚙', label: 'Settings' },
  { to: '/logs', icon: '☷', label: 'Logs' },
];

export default function Sidebar() {
  const loc = useLocation();
  return (
    <aside className="sidebar">
      <div className="sidebar-brand">mc-keygen</div>
      <nav className="sidebar-nav">
        {items.map(item => (
          <Link
            key={item.to}
            to={item.to}
            className={`sidebar-link${loc.pathname === item.to ? ' active' : ''}`}
          >
            <span className="icon">{item.icon}</span>
            {item.label}
          </Link>
        ))}
      </nav>
      <div className="sidebar-footer">
        <span>MeshCore Vanity Keygen</span>
      </div>
    </aside>
  );
}
