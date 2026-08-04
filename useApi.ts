/**
 * Hook API — Appels REST vers le serveur SpaceOne Mission Control
 * Fetch les données initiales et rafraîchit périodiquement.
 */

import { useState, useEffect, useCallback } from 'react';

export function useApi(baseUrl: string, refreshInterval = 5000) {
  const [telemetry, setTelemetry] = useState<any>(null);
  const [assets, setAssets] = useState<any[]>([]);
  const [alerts, setAlerts] = useState<any[]>([]);
  const [timeline, setTimeline] = useState<any[]>([]);
  const [commands, setCommands] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchAll = useCallback(async () => {
    try {
      const [telRes, assetsRes, alertsRes, timelineRes, commandsRes] = await Promise.allSettled([
        fetch(`${baseUrl}/telemetry`),
        fetch(`${baseUrl}/assets`),
        fetch(`${baseUrl}/alerts`),
        fetch(`${baseUrl}/timeline`),
        fetch(`${baseUrl}/commands`),
      ]);

      if (telRes.status === 'fulfilled' && telRes.value.ok) {
        setTelemetry(await telRes.value.json());
      }
      if (assetsRes.status === 'fulfilled' && assetsRes.value.ok) {
        const data = await assetsRes.value.json();
        setAssets(Object.values(data));
      }
      if (alertsRes.status === 'fulfilled' && alertsRes.value.ok) {
        setAlerts(await alertsRes.value.json());
      }
      if (timelineRes.status === 'fulfilled' && timelineRes.value.ok) {
        setTimeline(await timelineRes.value.json());
      }
      if (commandsRes.status === 'fulfilled' && commandsRes.value.ok) {
        const data = await commandsRes.value.json();
        setCommands([...data.pending, ...data.in_transit]);
      }

      setLoading(false);
    } catch (e) {
      console.error('[API] Fetch error:', e);
    }
  }, [baseUrl]);

  // Fetch initial + refresh périodique
  useEffect(() => {
    fetchAll();
    const interval = setInterval(fetchAll, refreshInterval);
    return () => clearInterval(interval);
  }, [fetchAll, refreshInterval]);

  // Fonction pour envoyer une commande
  const sendCommand = useCallback(async (command: any) => {
    const response = await fetch(`${baseUrl}/commands`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(command),
    });
    return response.json();
  }, [baseUrl]);

  // Fonction pour ack une alerte
  const acknowledgeAlert = useCallback(async (alertId: string) => {
    const response = await fetch(`${baseUrl}/alerts/acknowledge`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ id: alertId }),
    });
    return response.json();
  }, [baseUrl]);

  return { telemetry, assets, alerts, timeline, commands, loading, sendCommand, acknowledgeAlert };
}
