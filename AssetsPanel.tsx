/**
 * Assets Panel — Détail des assets martiens (rover, habitat, orbiteur)
 * Affiche la santé, la position, les ressources et l'état de chaque asset.
 */

import React, { useState } from 'react';

interface Asset {
  id: string;
  name: string;
  type: 'rover' | 'habitat' | 'orbiter';
  state: 'nominal' | 'degraded' | 'safe_mode' | 'recovery' | 'offline';
  health_score: number;
  position_lat: number;
  position_lon: number;
  temperature_c: number;
  power_level_pct: number;
  storage_used_mb: number;
  storage_total_mb: number;
  last_contact: string;
  dtn_buffer_pct: number;
  components?: Component[];
}

interface Component {
  name: string;
  status: 'ok' | 'warning' | 'fault';
  temperature?: number;
  voltage?: number;
}

const stateColors: Record<string, string> = {
  nominal: '#00ff41',
  degraded: '#ffaa00',
  safe_mode: '#ff8800',
  recovery: '#00b4d8',
  offline: '#ff0040',
};

const typeIcons: Record<string, string> = {
  rover: '🚗',
  habitat: '🏠',
  orbiter: '🛰',
};

export const AssetsPanel: React.FC<{ assets: Asset[] }> = ({ assets }) => {
  const [selectedAsset, setSelectedAsset] = useState<string | null>(null);
  const [viewMode, setViewMode] = useState<'grid' | 'list'>('grid');

  const selected = assets.find(a => a.id === selectedAsset);

  return (
    <div className="assets-panel">
      <div className="assets-header">
        <h3>Active Assets ({assets.length})</h3>
        <div className="view-toggle">
          <button onClick={() => setViewMode('grid')} className={viewMode === 'grid' ? 'active' : ''}>Grid</button>
          <button onClick={() => setViewMode('list')} className={viewMode === 'list' ? 'active' : ''}>List</button>
        </div>
      </div>

      <div className="assets-grid">
        {assets.map(asset => (
          <div
            key={asset.id}
            className={`asset-card ${selectedAsset === asset.id ? 'selected' : ''}`}
            onClick={() => setSelectedAsset(asset.id)}
          >
            <div className="asset-card-header">
              <span className="asset-type-icon">{typeIcons[asset.type]}</span>
              <div className="asset-card-title">
                <span className="asset-name">{asset.name}</span>
                <span className="asset-id">{asset.id}</span>
              </div>
              <span
                className="asset-state-badge"
                style={{ color: stateColors[asset.state] }}
              >
                {asset.state.toUpperCase()}
              </span>
            </div>

            <div className="asset-metrics-row">
              <div className="asset-metric">
                <span className="metric-label">Health</span>
                <div className="metric-bar">
                  <div
                    className="metric-bar-fill"
                    style={{
                      width: `${asset.health_score * 100}%`,
                      background: asset.health_score > 0.8 ? '#00ff41' : asset.health_score > 0.5 ? '#ffaa00' : '#ff0040',
                    }}
                  />
                </div>
                <span className="metric-value">{(asset.health_score * 100).toFixed(0)}%</span>
              </div>

              <div className="asset-metric">
                <span className="metric-label">Power</span>
                <div className="metric-bar">
                  <div className="metric-bar-fill" style={{ width: `${asset.power_level_pct}%` }} />
                </div>
                <span className="metric-value">{asset.power_level_pct.toFixed(0)}%</span>
              </div>

              <div className="asset-metric">
                <span className="metric-label">Storage</span>
                <span className="metric-value">
                  {asset.storage_used_mb} / {asset.storage_total_mb} MB
                </span>
              </div>
            </div>

            <div className="asset-details-row">
              <span>Temp: {asset.temperature_c.toFixed(1)}°C</span>
              <span>DTN Buffer: {asset.dtn_buffer_pct.toFixed(0)}%</span>
              <span>Last Contact: {new Date(asset.last_contact).toLocaleTimeString()}</span>
            </div>

            {asset.type === 'rover' && (
              <div className="asset-position">
                Position: {asset.position_lat.toFixed(4)}°, {asset.position_lon.toFixed(4)}°
              </div>
            )}
          </div>
        ))}
      </div>

      {/* Panel de détail */}
      {selected && (
        <div className="asset-detail-panel">
          <h4>{selected.name} — Détails</h4>
          <table className="detail-table">
            <tbody>
              <tr><td>ID</td><td>{selected.id}</td></tr>
              <tr><td>Type</td><td>{selected.type}</td></tr>
              <tr><td>État</td><td style={{ color: stateColors[selected.state] }}>{selected.state}</td></tr>
              <tr><td>Health Score</td><td>{(selected.health_score * 100).toFixed(1)}%</td></tr>
              <tr><td>Température</td><td>{selected.temperature_c.toFixed(1)}°C</td></tr>
              <tr><td>Énergie</td><td>{selected.power_level_pct.toFixed(1)}%</td></tr>
              <tr><td>Stockage</td><td>{selected.storage_used_mb} / {selected.storage_total_mb} MB</td></tr>
              <tr><td>DTN Buffer</td><td>{selected.dtn_buffer_pct.toFixed(1)}%</td></tr>
              <tr><td>Dernier contact</td><td>{selected.last_contact}</td></tr>
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
};
