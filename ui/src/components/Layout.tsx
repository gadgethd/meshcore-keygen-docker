import Sidebar from './Sidebar';
import Topbar from './Topbar';

export default function Layout({ children }: { children: React.ReactNode }) {
  return (
    <div className="app-shell">
      <Sidebar />
      <div className="main-area">
        <Topbar />
        <div className="content">
          {children}
        </div>
      </div>
    </div>
  );
}
