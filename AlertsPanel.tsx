/**
 * Alerts Panel — Gestion des alertes mission
 * Affiche les alertes par sévérité avec acknowledgment et escalation.
 */

import React from 'react';

interface Alert {
  id: string;
  severity: 'info' | 'warning' | 'critical' | 'emergency';
  asset_id: string;
  message: string;
  timestamp: string;
  acknowledged: boolean;
}

const severityConfig: Record<string, { color: string; icon: string; label: string }> = {
  info: { color: '#00b4d8', icon: 'ℹ', label: 'INFO' },
  warning: { color: '#ffaa00', icon: '⚠', label: 'WARNING' },
  critical: { color: '#ff0040', icon: '🔴', label: 'CRITICAL' },
  emergency: { color: '#ff0040', icon: '🚨', label: 'EMERGENCY' },
};

export const AlertsPanel: React.FC<{ alerts: Alert[]; onAcknowledge?: (id: string) => void }> = ({
  alerts,
  onAcknowledge,
}) => {
  const unacknowledged = alerts.filter(a => !a.acknowledged);
  const acknowledged = alerts.filter(a => a.acknowledged);

  const criticalCount = alerts.filter(a => a.severity === 'critical' || a.severity === 'emergency').length;

  return (
    <div className="alerts-panel">
      <div className="alerts-summary">
        <div className="summary-card critical">
          <span className="summary-number">{criticalCount}</span>
          <span className="summary-label">Critical</span>
        </div>
        <div className="summary-card pending">
          <span className="summary-number">{unacknowledged.length}</span>
          <span className="summary-label">Pending</span>
        </div>
        <div className="summary-card total">
          <span className="summary-number">{alerts.length}</span>
          <span className="summary-label">Total</span>
        </div>
      </div>

      <div className="alerts-list">
        <h3>Active Alerts ({unacknowledged.length})</h3>
        {unacknowledged.length === 0 ? (
          <p className="no-alerts">No active alerts</p>
        ) : (
          unacknowledged.map(alert => {
            const config = severityConfig[alert.severity];
            return (
              <div key={alert.id} className={`alert-item ${alert.severity}`}>
                <div className="alert-header">
                  <span className="alert-severity" style={{ color: config.color }}>
                    {config.icon} {config.label}
                  </span>
                  <span className="alert-time">{new Date(alert.timestamp).toLocaleString()}</span>
                </div>
                <div className="alert-body">
                  <span className="alert-message">{alert.message}</span>
                  <span className="alert-asset">Asset: {alert.asset_id}</span>
                </div>
                <div className="alert-actions">
                  <button
                    className="btn-acknowledge"
                    onClick={() => onAcknowledge?.(alert.id)}
                  >
                    Acknowledge
                  </button>
                </div>
              </div>
            );
          })
        )}

        <h3>Acknowledged ({acknowledged.length})</h3>
        {acknowledged.map(alert => {
          const config = severityConfig[alert.severity];
          return (
            <div key={alert.id} className={`alert-item acknowledged ${alert.severity}`}>
              <div className="alert-header">
                <span className="alert-severity" style={{ color: config.color, opacity: 0.6 }}>
                  {config.icon} {config.label}
                </span>
                <span className="alert-time">{new Date(alert.timestamp).toLocaleString()}</span>
              </div>
              <div className="alert-body">
                <span className="alert-message">{alert.message}</span>
                <span className="alert-asset">Asset: {alert.asset_id}</span>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
};
