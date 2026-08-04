/**
 * SpaceOne Mission Control — Web Dashboard
 * Interface de contrôle mission graphique pour le control room.
 * Connexion WebSocket temps réel au serveur Rust (port 3420).
 *
 * Technologies: React + TypeScript + WebSocket
 */

import React, { useState, useEffect, useCallback } from 'react';
import { Dashboard } from './components/Dashboard';
import { AssetsPanel } from './components/AssetsPanel';
import { CommandCenter } from './components/CommandCenter';
import { AlertsPanel } from './components/AlertsPanel';
import { ScienceData } from './components/ScienceData';
import { Timeline } from './components/Timeline';
import { useWebSocket } from './hooks/useWebSocket';
import { useApi } from './hooks/useApi';
import './App.css';

// ─── Types ───

interface MissionState {
  missionName: string;
  solNumber: number;
  marsUtc: string;
  earthUtc: string;
  communicationDelayMinutes: number;
  healthScore: number;
  dataRateMbps: number;
  connected: boolean;
  assets: Asset[];
  alerts: Alert[];
  pendingCommands: number;
  deliveredBundles: number;
}

interface Asset {
  id: string;
  name: string;
  type: 'rover' | 'habitat' | 'orbiter';
  state: 'nominal' | 'degraded' | 'safe_mode' | 'recovery' | 'offline';
  healthScore: number;
  positionLat: number;
  positionLon: number;
  temperatureC: number;
  powerLevelPct: number;
  storageUsedMb: number;
  storageTotalMb: number;
  lastContact: string;
  dtnBufferPct: number;
}

interface Alert {
  id: string;
  severity: 'info' | 'warning' | 'critical' | 'emergency';
  assetId: string;
  message: string;
  timestamp: string;
  acknowledged: boolean;
}

// ─── Application ───

type ActiveView = 'dashboard' | 'assets' | 'commands' | 'alerts' | 'science' | 'timeline';

const App: React.FC = () => {
  const [activeView, setActiveView] = useState<ActiveView>('dashboard');
  const [missionState, setMissionState] = useState<MissionState | null>(null);
  const [lastUpdate, setLastUpdate] = useState<Date>(new Date());

  // WebSocket connection au serveur Rust
  const { isConnected, error } = useWebSocket('ws://localhost:3420/ws', {
    onMessage: useCallback((data: any) => {
      if (data.type === 'telemetry_update') {
        setMissionState(data.payload);
        setLastUpdate(new Date());
      }
    }, []),
    onConnect: () => console.log('[MC] WebSocket connected'),
    onDisconnect: () => console.log('[MC] WebSocket disconnected'),
  });

  // Fetch initial data
  const { telemetry, assets, alerts, timeline, commands } = useApi('http://localhost:3420/api');

  // Mettre à jour l'état local avec les données API
  useEffect(() => {
    if (telemetry) {
      setMissionState({
        ...telemetry,
        connected: isConnected,
        assets: assets || [],
        alerts: alerts || [],
      });
    }
  }, [telemetry, assets, alerts, isConnected]);

  // Auto-refresh toutes les 5 secondes
  useEffect(() => {
    const interval = setInterval(() => {
      setLastUpdate(new Date());
    }, 5000);
    return () => clearInterval(interval);
  }, []);

  const renderView = () => {
    if (!missionState) {
      return <LoadingScreen error={error} connecting={!isConnected} />;
    }

    switch (activeView) {
      case 'dashboard':
        return <Dashboard missionState={missionState} />;
      case 'assets':
        return <AssetsPanel assets={missionState.assets} />;
      case 'commands':
        return <CommandCenter commands={commands || []} />;
      case 'alerts':
        return <AlertsPanel alerts={missionState.alerts} />;
      case 'science':
        return <ScienceData />;
      case 'timeline':
        return <Timeline events={timeline || []} />;
      default:
        return <Dashboard missionState={missionState} />;
    }
  };

  return (
    <div className="mission-control">
      {/* Header */}
      <header className="mc-header">
        <div className="mc-logo">
          <span className="mc-logo-icon">&#9788;</span>
          <h1>SpaceOne Mission Control</h1>
        </div>
        <div className="mc-status-bar">
          <span className={`mc-status-indicator ${isConnected ? 'connected' : 'disconnected'}`}>
            {isConnected ? '● CONNECTED' : '○ DISCONNECTED'}
          </span>
          <span className="mc-sol">Sol {missionState?.solNumber || 0}</span>
          <span className="mc-delay">Delay: {missionState?.communicationDelayMinutes?.toFixed(1)}min</span>
          <span className="mc-uptime">Last update: {lastUpdate.toLocaleTimeString()}</span>
        </div>
      </header>

      {/* Navigation */}
      <nav className="mc-nav">
        {(['dashboard', 'assets', 'commands', 'alerts', 'science', 'timeline'] as ActiveView[]).map((view) => (
          <button
            key={view}
            className={`mc-nav-btn ${activeView === view ? 'active' : ''}`}
            onClick={() => setActiveView(view)}
          >
            {view.charAt(0).toUpperCase() + view.slice(1)}
          </button>
        ))}
      </nav>

      {/* Main Content */}
      <main className="mc-content">
        {renderView()}
      </main>

      {/* Footer */}
      <footer className="mc-footer">
        <span>SpaceOne v1.0.0 | AsterQuanta OS v0.1.0</span>
        <span>Health: {(missionState?.healthScore || 0) * 100}%</span>
        <span>Data Rate: {missionState?.dataRateMbps?.toFixed(1)} Mbps</span>
        <span>Pending Commands: {missionState?.pendingCommands || 0}</span>
      </footer>
    </div>
  );
};

// ─── Loading Screen ───

const LoadingScreen: React.FC<{ error: string | null; connecting: boolean }> = ({ error, connecting }) => (
  <div className="loading-screen">
    <div className="loading-spinner" />
    <h2>{connecting ? 'Connexion au Mission Control...' : 'Déconnexion'}</h2>
    {error && <p className="error">{error}</p>}
    <p>Tentative de reconnexion automatique...</p>
  </div>
);

export default App;
