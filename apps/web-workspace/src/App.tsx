import React, { useState } from 'react';

export function App() {
  const [activeTab, setActiveTab] = useState<'editor' | 'agents' | 'mobile' | 'brain' | 'terminal'>('editor');
  const [activeAgentRole, setActiveAgentRole] = useState('Principal Software Architect');
  const [taskPrompt, setTaskPrompt] = useState('');
  const [tasks, setTasks] = useState([
    { id: 'task-101', role: 'Architect', objective: 'Verify IPC and AST symbol indexing schemas', status: 'Completed' },
    { id: 'task-102', role: 'Mobile Developer', objective: 'Scaffold React Native + Expo e-commerce demo', status: 'In Progress' },
  ]);

  const handleDispatch = () => {
    if (!taskPrompt) return;
    setTasks([
      { id: `task-${Date.now().toString().slice(-3)}`, role: activeAgentRole, objective: taskPrompt, status: 'Queued' },
      ...tasks,
    ]);
    setTaskPrompt('');
  };

  return (
    <div style={{ display: 'flex', height: '100vh', flexDirection: 'column' }}>
      {/* Top Navbar */}
      <header style={{ height: 44, background: '#070a12', borderBottom: '1px solid #1e293b', display: 'flex', alignItems: 'center', padding: '0 16px', justifyContent: 'space-between' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <span style={{ fontWeight: 800, letterSpacing: 1.2, color: '#38bdf8', fontSize: 16 }}>AETHER // IDE</span>
          <span style={{ fontSize: 12, padding: '2px 8px', background: '#1e293b', borderRadius: 4, color: '#94a3b8' }}>v0.1.0-beta.2 Web</span>
        </div>
        <div style={{ display: 'flex', gap: 16, fontSize: 13 }}>
          <button onClick={() => setActiveTab('editor')} style={{ background: 'none', border: 'none', color: activeTab === 'editor' ? '#38bdf8' : '#94a3b8', cursor: 'pointer', fontWeight: 600 }}>Editor</button>
          <button onClick={() => setActiveTab('agents')} style={{ background: 'none', border: 'none', color: activeTab === 'agents' ? '#38bdf8' : '#94a3b8', cursor: 'pointer', fontWeight: 600 }}>AI Agents</button>
          <button onClick={() => setActiveTab('mobile')} style={{ background: 'none', border: 'none', color: activeTab === 'mobile' ? '#38bdf8' : '#94a3b8', cursor: 'pointer', fontWeight: 600 }}>Mobile Studio</button>
          <button onClick={() => setActiveTab('brain')} style={{ background: 'none', border: 'none', color: activeTab === 'brain' ? '#38bdf8' : '#94a3b8', cursor: 'pointer', fontWeight: 600 }}>Project Brain</button>
          <button onClick={() => setActiveTab('terminal')} style={{ background: 'none', border: 'none', color: activeTab === 'terminal' ? '#38bdf8' : '#94a3b8', cursor: 'pointer', fontWeight: 600 }}>Terminal</button>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 12, color: '#4ade80' }}>
          <span>● Rust Daemon Synced (44888)</span>
        </div>
      </header>

      {/* Main Workspace Body */}
      <div style={{ display: 'flex', flex: 1, overflow: 'hidden' }}>
        {/* Left Sidebar */}
        <aside style={{ width: 260, background: '#0b0f19', borderRight: '1px solid #1e293b', padding: 12, display: 'flex', flexDirection: 'column' }}>
          <div style={{ fontSize: 11, fontWeight: 'bold', textTransform: 'uppercase', color: '#64748b', marginBottom: 10 }}>Workspace Explorer</div>
          <div style={{ fontSize: 13, lineHeight: '24px', color: '#cbd5e1' }}>
            <div>📁 rust/</div>
            <div style={{ paddingLeft: 12 }}>📁 crates/</div>
            <div style={{ paddingLeft: 24 }}>📄 application-core/</div>
            <div style={{ paddingLeft: 24 }}>📄 ipc-protocol/</div>
            <div style={{ paddingLeft: 24 }}>📄 project-indexer/</div>
            <div style={{ paddingLeft: 24 }}>📄 model-gateway/</div>
            <div style={{ paddingLeft: 24 }}>📄 agent-orchestrator/</div>
            <div>📁 apps/</div>
            <div style={{ paddingLeft: 12 }}>📁 desktop/</div>
            <div style={{ paddingLeft: 12 }}>📁 ios-companion/</div>
            <div style={{ paddingLeft: 12 }}>📁 web-workspace/</div>
            <div>📁 docs/</div>
          </div>
        </aside>

        {/* Center Panel */}
        <main style={{ flex: 1, background: '#020617', display: 'flex', flexDirection: 'column', padding: 16, overflowY: 'auto' }}>
          {activeTab === 'editor' && (
            <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
              <div style={{ display: 'flex', background: '#0f172a', padding: '6px 12px', borderTopLeftRadius: 6, borderTopRightRadius: 6, gap: 12, fontSize: 13 }}>
                <span style={{ color: '#38bdf8', borderBottom: '2px solid #38bdf8', paddingBottom: 2 }}>rust/services/aether-daemon/src/main.rs</span>
              </div>
              <div style={{ flex: 1, background: '#090d16', padding: 16, fontFamily: 'monospace', fontSize: 13, lineHeight: '20px', color: '#e2e8f0', whiteSpace: 'pre-wrap', border: '1px solid #1e293b', borderTop: 'none' }}>
{`// AetherIDE Native Daemon v0.1.0-beta.2
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().init();
    info!("Starting AetherIDE Native Daemon");
    let config = AetherConfig::default();
    let db = DatabaseEngine::open(&config.data_dir.join("aether.db"))?;
    let model_gateway = ModelGateway::new();
    let agent_orchestrator = AgentOrchestrator::new(model_gateway, db);
    // IPC Listener over Named Pipe and loopback TCP
    let listener = TcpListener::bind("127.0.0.1:44888").await?;
    ...
}`}
              </div>
            </div>
          )}

          {activeTab === 'agents' && (
            <div>
              <h2 style={{ fontSize: 18, color: '#f8fafc', marginBottom: 12 }}>Autonomous AI Engineering Team</h2>
              <div style={{ background: '#0f172a', padding: 16, borderRadius: 8, border: '1px solid #1e293b', marginBottom: 20 }}>
                <label style={{ display: 'block', fontSize: 13, marginBottom: 6, color: '#94a3b8' }}>Select Role Persona:</label>
                <select value={activeAgentRole} onChange={(e) => setActiveAgentRole(e.target.value)} style={{ width: '100%', padding: 8, background: '#1e293b', color: '#fff', border: '1px solid #334155', borderRadius: 4, marginBottom: 12 }}>
                  <option>Principal Software Architect</option>
                  <option>Frontend Developer</option>
                  <option>Backend Developer</option>
                  <option>Database Engineer</option>
                  <option>Mobile Developer (React Native/Expo)</option>
                  <option>Rust/C++ Systems Engineer</option>
                  <option>Debugging Specialist</option>
                  <option>Testing Specialist</option>
                  <option>Security Reviewer</option>
                  <option>DevOps Engineer</option>
                </select>

                <label style={{ display: 'block', fontSize: 13, marginBottom: 6, color: '#94a3b8' }}>Objective / Requirement:</label>
                <textarea rows={3} value={taskPrompt} onChange={(e) => setTaskPrompt(e.target.value)} placeholder="Enter task specifications..." style={{ width: '100%', padding: 8, background: '#1e293b', color: '#fff', border: '1px solid #334155', borderRadius: 4, marginBottom: 12, boxSizing: 'border-box' }} />

                <button onClick={handleDispatch} style={{ background: '#0284c7', color: '#fff', padding: '10px 18px', border: 'none', borderRadius: 6, fontWeight: 600, cursor: 'pointer' }}>
                  Dispatch Autonomous Agent
                </button>
              </div>

              <h3 style={{ fontSize: 15, color: '#e2e8f0', marginBottom: 10 }}>Proof of Work Ledger & Task Status</h3>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
                {tasks.map(t => (
                  <div key={t.id} style={{ background: '#0f172a', padding: 12, borderRadius: 6, border: '1px solid #1e293b' }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 6 }}>
                      <span style={{ fontWeight: 'bold', color: '#38bdf8', fontSize: 13 }}>{t.role}</span>
                      <span style={{ fontSize: 11, padding: '2px 8px', borderRadius: 10, background: t.status === 'Completed' ? '#14532d' : '#1e3a8a', color: t.status === 'Completed' ? '#4ade80' : '#60a5fa' }}>{t.status}</span>
                    </div>
                    <div style={{ fontSize: 13, color: '#cbd5e1' }}>{t.objective}</div>
                  </div>
                ))}
              </div>
            </div>
          )}

          {activeTab === 'mobile' && (
            <div>
              <h2 style={{ fontSize: 18, color: '#f8fafc', marginBottom: 12 }}>Mobile App Studio</h2>
              <div style={{ background: '#0f172a', padding: 16, borderRadius: 8, border: '1px solid #1e293b' }}>
                <p style={{ color: '#94a3b8', fontSize: 14 }}>Default Architecture: <b>React Native + Expo Router + TypeScript</b></p>
                <div style={{ display: 'flex', gap: 12, marginTop: 14 }}>
                  <button style={{ background: '#16a34a', color: '#fff', padding: '10px 16px', border: 'none', borderRadius: 6, fontWeight: 600 }}>Queue EAS Android Build (APK)</button>
                  <button style={{ background: '#9333ea', color: '#fff', padding: '10px 16px', border: 'none', borderRadius: 6, fontWeight: 600 }}>Queue EAS iOS Build (IPA)</button>
                </div>
              </div>
            </div>
          )}

          {activeTab === 'brain' && (
            <div>
              <h2 style={{ fontSize: 18, color: '#f8fafc', marginBottom: 12 }}>Living Project Brain & Change Impact Radar</h2>
              <div style={{ background: '#0f172a', padding: 20, borderRadius: 8, border: '1px solid #1e293b' }}>
                <div style={{ fontWeight: 'bold', color: '#38bdf8' }}>AST Dependency Graph</div>
                <p style={{ fontSize: 13, color: '#94a3b8' }}>21 crates and subsystems mapped. Change Impact Radar ready to scan blast radius on any file edit.</p>
              </div>
            </div>
          )}

          {activeTab === 'terminal' && (
            <div style={{ background: '#000', padding: 16, borderRadius: 8, fontFamily: 'monospace', color: '#22c55e', height: '100%' }}>
              <div>Aether Integrated Web Terminal</div>
              <div>Connected to 127.0.0.1:44888 via Named Pipe / WebSocket</div>
              <div style={{ color: '#38bdf8', marginTop: 12 }}>$ aether-cli status</div>
              <div style={{ color: '#cbd5e1' }}>Daemon: Active | Memory: 24.2 MB | 14 Personas Loaded</div>
            </div>
          )}
        </main>
      </div>
    </div>
  );
}
