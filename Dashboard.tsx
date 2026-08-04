/**
 * Dashboard — Vue principale du Mission Control
 * Affiche la santé globale, les assets, et les métriques clés.
 */

import React from 'react';
import './Dashboard.css';

interface MissionState {
  missionName: string;
  solNumber: number;
  communicationDelayMinutes: number;
  healthScore: number;
  dataRateMbps: number;
  connected: boolean;
  assets: any[];
  alerts: any[];
  pendingCommands: number;
  deliveredBundles: number;
}

export const Dashboard: React.FC<{ missionState: MissionState }> = ({ missionState }) => {
  const {
    solNumber, communicationDelayMinutes, healthScore,
    dataRateMbps, assets, alerts, pendingCommands, deliveredBundles
  } = missionState;

  const healthColor = healthScore > 0.8 ? '#00ff41' : healthScore > 0.5 ? '#ffaa00' : '#ff0040';
  const activeAlerts = alerts.filter(a => !a.acknowledged).length;

  return (
    <div className="dashboard">
      {/* Health Score — Gauges principales */}
      <div className="dashboard-grid">
        <div className="dashboard-card health-card">
          <h3>Mission Health</h3>
          <div className="health-gauge">
            <svg viewBox="0 0 200 200" className="gauge-svg">
              <circle cx="100" cy="100" r="80" fill="none" stroke="#1a1a2e" strokeWidth="12" />
              <circle
                cx="100" cy="100" r="80" fill="none"
                stroke={healthColor} strokeWidth="12"
                strokeDasharray={`${healthScore * 502} 502`}
                strokeDashoffset="125"
                strokeLinecap="round"
                transform="rotate(-90 100 100)"
                className="gauge-circle"
              />
              <text x="100" y="95" textAnchor="middle" className="gauge-value"
                    style={{ fill: healthColor, fontSize: '36px', fontWeight: 'bold' }}>
                {(healthScore * 100).toFixed(0)}%
              </text>
              <text x="100" y="120" textAnchor="middle" className="gauge-label"
                    style={{ fill: '#8892b0', fontSize: '12px' }}>
                System Health
              </text>
            </svg>
          </div>
        </div>

        {/* Métriques clés */}
        <div className="dashboard-card metrics-card">
          <h3>Key Metrics</h3>
          <div className="metrics-grid">
            <MetricItem label="Sol Number" value={solNumber} unit="" />
            <MetricItem label="Comm Delay" value={communicationDelayMinutes.toFixed(1)} unit="min" />
            <MetricItem label="Data Rate" value={dataRateMbps.toFixed(1)} unit="Mbps" />
            <MetricItem label="Bundles Delivered" value={deliveredBundles} unit="" />
            <MetricItem label="Pending Commands" value={pendingCommands} unit="" color="#ffaa00" />
            <MetricItem label="Active Alerts" value={activeAlerts} unit="" color={activeAlerts > 0 ? '#ff0040' : '#00ff41'} />
          </div>
        </div>

        {/* Assets Overview */}
        <div className="dashboard-card assets-card">
          <h3>Active Assets</h3>
          <div className="assets-list">
            {assets.map((asset: any) => (
              <div key={asset.id} className={`asset-row ${asset.state}`}>
                <div className="asset-icon">
                  {asset.type === 'rover' ? '🚗' : asset.type === 'habitat' ? '🏠' : '🛰'}
                </div>
                <div className="asset-info">
                  <span className="asset-name">{asset.name}</span>
                  <span className="asset-state">{asset.state}</span>
                </div>
                <div className="asset-metrics">
                  <span>Health: {(asset.health_score * 100).toFixed(0)}%</span>
                  <span>Power: {asset.power_level_pct.toFixed(0)}%</span>
                  <span>{asset.temperature_c.toFixed(0)}°C</span>
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
};

const MetricItem: React.FC<{ label: string; value: string | number; unit: string; color?: string }> = ({
  label, value, unit, color
}) => (
  <div className="metric-item">
    <span className="metric-label">{label}</span>
    <span className="metric-value" style={{ color: color || '#00ff41' }}>
      {value} <small className="metric-unit">{unit}</small>
    </span>
  </div>
);
