/**
 * Command Center — Gestion des commandes asynchrones vers Mars
 * Envoi de commandes via DTN avec priorisation et double autorisation.
 */

import React, { useState } from 'react';
import './CommandCenter.css';

interface Command {
  id: string;
  command: string;
  targetAsset: string;
  priority: 'routine' | 'high' | 'critical' | 'safe_mode';
  status: 'queued' | 'in_transit' | 'delivered' | 'executing' | 'completed' | 'failed';
  createdAt: string;
  sentAt?: string;
}

export const CommandCenter: React.FC<{ commands: Command[] }> = ({ commands }) => {
  const [formData, setFormData] = useState({
    command: '',
    targetAsset: '',
    priority: 'routine',
  });
  const [secondaryAuth, setSecondaryAuth] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    // Double autorisation requise pour les commandes critiques
    if (formData.priority === 'critical' || formData.priority === 'safe_mode') {
      if (!secondaryAuth) {
        alert('Double authorization required for critical commands');
        return;
      }
    }

    const response = await fetch('http://localhost:3420/api/commands', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        ...formData,
        id: `CMD-${Date.now()}`,
        created_at: new Date().toISOString(),
        status: formData.priority === 'critical' || formData.priority === 'safe_mode'
          ? 'in_transit' : 'queued',
      }),
    });

    const result = await response.json();
    console.log('Command sent:', result);
    setFormData({ command: '', targetAsset: '', priority: 'routine' });
    setSecondaryAuth(false);
  };

  const getStatusColor = (status: string) => {
    const colors: Record<string, string> = {
      queued: '#8892b0',
      in_transit: '#00b4d8',
      delivered: '#00ff41',
      executing: '#ffaa00',
      completed: '#00ff41',
      failed: '#ff0040',
    };
    return colors[status] || '#8892b0';
  };

  const getPriorityColor = (priority: string) => {
    const colors: Record<string, string> = {
      routine: '#8892b0',
      high: '#ffaa00',
      critical: '#ff0040',
      safe_mode: '#ff0040',
    };
    return colors[priority] || '#8892b0';
  };

  return (
    <div className="command-center">
      <div className="cc-grid">
        {/* Nouvelle commande */}
        <div className="cc-card cc-form-card">
          <h3>Send Command to Mars</h3>
          <form onSubmit={handleSubmit} className="cc-form">
            <div className="form-group">
              <label>Command</label>
              <textarea
                value={formData.command}
                onChange={(e) => setFormData({ ...formData, command: e.target.value })}
                placeholder="e.g., move forward 10m"
                required
              />
            </div>

            <div className="form-group">
              <label>Target Asset</label>
              <select
                value={formData.targetAsset}
                onChange={(e) => setFormData({ ...formData, targetAsset: e.target.value })}
                required
              >
                <option value="">Select target...</option>
                <option value="rover-01">Perseverance-X (Rover)</option>
                <option value="habitat-01">Ares Base Alpha (Habitat)</option>
                <option value="orbiter-01">Hermes Relay (Orbiter)</option>
              </select>
            </div>

            <div className="form-group">
              <label>Priority</label>
              <select
                value={formData.priority}
                onChange={(e) => setFormData({ ...formData, priority: e.target.value as any })}
              >
                <option value="routine">Routine</option>
                <option value="high">High</option>
                <option value="critical">Critical (dual auth required)</option>
                <option value="safe_mode">Safe Mode (dual auth required)</option>
              </select>
            </div>

            {(formData.priority === 'critical' || formData.priority === 'safe_mode') && (
              <div className="form-group dual-auth">
                <label>
                  <input
                    type="checkbox"
                    checked={secondaryAuth}
                    onChange={(e) => setSecondaryAuth(e.target.checked)}
                  />
                  Secondary authorization confirmed
                </label>
              </div>
            )}

            <button type="submit" className="btn-send">
              Send Command (DTN Relay)
            </button>
            <p className="cc-hint">
              Latence Mars-Terre: ~{14}min aller simple. Confirmation d'exécution dans ~{28}min.
            </p>
          </form>
        </div>

        {/* File de commandes */}
        <div className="cc-card cc-list-card">
          <h3>Command Queue ({commands.length})</h3>
          <div className="cc-queue">
            {commands.length === 0 ? (
              <p className="empty-queue">No pending commands</p>
            ) : (
              <table className="cc-table">
                <thead>
                  <tr>
                    <th>ID</th>
                    <th>Command</th>
                    <th>Target</th>
                    <th>Priority</th>
                    <th>Status</th>
                  </tr>
                </thead>
                <tbody>
                  {commands.map((cmd) => (
                    <tr key={cmd.id}>
                      <td className="cmd-id">{cmd.id}</td>
                      <td>{cmd.command}</td>
                      <td>{cmd.targetAsset}</td>
                      <td style={{ color: getPriorityColor(cmd.priority) }}>{cmd.priority}</td>
                      <td style={{ color: getStatusColor(cmd.status) }}>{cmd.status}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};
