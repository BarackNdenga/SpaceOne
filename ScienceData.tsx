/**
 * Science Data — Traitement et visualisation des données scientifiques
 * Pipeline de données reçues de Mars via DTN.
 */

import React, { useState } from 'react';

interface ScienceBundle {
  id: string;
  instrument: string;
  asset_id: string;
  data_type: string;
  classification: 'Public' | 'Internal' | 'Confidential' | 'ScientificSecret';
  received_at: string;
  size_kb: number;
  integrity_verified: boolean;
  anomaly_detected: boolean;
  processing_status: 'received' | 'processing' | 'complete' | 'failed';
}

const instruments = [
  'Mastcam-Z', 'SuperCam', 'PIXL', 'SHERLOC', 'MEDA', 'MOXIE',
  'WATSON', 'SUPERCAM', 'PIXL', 'RIMFAX', 'Mars Environmental Monitor',
];

const dataTypes = ['Image', 'Spectra', 'Atmospheric', 'Seismic', 'Chemical', 'Terrain'];

const classificationColors: Record<string, string> = {
  Public: '#00ff41',
  Internal: '#00b4d8',
  Confidential: '#ffaa00',
  ScientificSecret: '#ff0040',
};

export const ScienceData: React.FC = () => {
  const [filter, setFilter] = useState<string>('all');
  const [bundles, setBundles] = useState<ScienceBundle[]>([
    {
      id: 'SCI-001',
      instrument: 'Mastcam-Z',
      asset_id: 'rover-01',
      data_type: 'Image',
      classification: 'Public',
      received_at: new Date().toISOString(),
      size_kb: 4520,
      integrity_verified: true,
      anomaly_detected: false,
      processing_status: 'complete',
    },
    {
      id: 'SCI-002',
      instrument: 'SuperCam',
      asset_id: 'rover-01',
      data_type: 'Spectra',
      classification: 'ScientificSecret',
      received_at: new Date().toISOString(),
      size_kb: 1280,
      integrity_verified: true,
      anomaly_detected: false,
      processing_status: 'processing',
    },
    {
      id: 'SCI-003',
      instrument: 'MEDA',
      asset_id: 'habitat-01',
      data_type: 'Atmospheric',
      classification: 'Internal',
      received_at: new Date().toISOString(),
      size_kb: 256,
      integrity_verified: true,
      anomaly_detected: true,
      processing_status: 'complete',
    },
  ]);

  const totalSizeMB = bundles.reduce((acc, b) => acc + b.size_kb / 1024, 0);
  const verified = bundles.filter(b => b.integrity_verified).length;
  const anomalies = bundles.filter(b => b.anomaly_detected).length;

  return (
    <div className="science-data">
      <div className="science-header">
        <h3>Science Data Pipeline</h3>
        <div className="science-stats">
          <span>Total: {bundles.length} bundles</span>
          <span>Verified: {verified}/{bundles.length}</span>
          <span>Anomalies: {anomalies}</span>
          <span>Size: {totalSizeMB.toFixed(1)} MB</span>
        </div>
      </div>

      <div className="filter-bar">
        <button className={filter === 'all' ? 'active' : ''} onClick={() => setFilter('all')}>All</button>
        {dataTypes.map(dt => (
          <button key={dt} className={filter === dt ? 'active' : ''} onClick={() => setFilter(dt)}>
            {dt}
          </button>
        ))}
      </div>

      <table className="science-table">
        <thead>
          <tr>
            <th>ID</th>
            <th>Instrument</th>
            <th>Type</th>
            <th>Classification</th>
            <th>Size</th>
            <th>Integrity</th>
            <th>Anomaly</th>
            <th>Status</th>
          </tr>
        </thead>
        <tbody>
          {bundles
            .filter(b => filter === 'all' || b.data_type === filter)
            .map(bundle => (
              <tr key={bundle.id}>
                <td>{bundle.id}</td>
                <td>{bundle.instrument}</td>
                <td>{bundle.data_type}</td>
                <td style={{ color: classificationColors[bundle.classification] }}>
                  {bundle.classification}
                </td>
                <td>{bundle.size_kb} KB</td>
                <td>{bundle.integrity_verified ? '✓' : '✗'}</td>
                <td style={{ color: bundle.anomaly_detected ? '#ff0040' : '#00ff41' }}>
                  {bundle.anomaly_detected ? '⚠ Anomaly' : 'Clean'}
                </td>
                <td>{bundle.processing_status}</td>
              </tr>
            ))}
        </tbody>
      </table>
    </div>
  );
};

/**
 * Timeline — Historique chronologique de la mission
 */
interface TimelineEvent {
  id: string;
  timestamp: string;
  type: 'command' | 'telemetry' | 'alert' | 'science' | 'system';
  description: string;
  asset_id?: string;
}

export const Timeline: React.FC<{ events: TimelineEvent[] }> = ({ events }) => {
  const typeColors: Record<string, string> = {
    command: '#00b4d8',
    telemetry: '#00ff41',
    alert: '#ff0040',
    science: '#ffaa00',
    system: '#8892b0',
  };

  const [filter, setFilter] = useState<string>('all');

  return (
    <div className="timeline-view">
      <h3>Mission Timeline</h3>

      <div className="filter-bar">
        <button className={filter === 'all' ? 'active' : ''} onClick={() => setFilter('all')}>All</button>
        {Object.keys(typeColors).map(t => (
          <button key={t} className={filter === t ? 'active' : ''} onClick={() => setFilter(t)}>
            {t.charAt(0).toUpperCase() + t.slice(1)}
          </button>
        ))}
      </div>

      <div className="timeline-container">
        {events
          .filter(e => filter === 'all' || e.type === filter)
          .sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime())
          .map(event => (
            <div key={event.id} className="timeline-event">
              <div className="event-marker" style={{ background: typeColors[event.type] }} />
              <div className="event-content">
                <span className="event-time">{new Date(event.timestamp).toLocaleString()}</span>
                <span className="event-type" style={{ color: typeColors[event.type] }}>
                  [{event.type.toUpperCase()}]
                </span>
                <span className="event-description">{event.description}</span>
                {event.asset_id && <span className="event-asset">({event.asset_id})</span>}
              </div>
            </div>
          ))}
      </div>
    </div>
  );
};
