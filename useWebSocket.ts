/**
 * Hook WebSocket — Connexion temps réel au serveur SpaceOne Mission Control
 * Gère la reconnexion automatique, le heartbeat, et la désérialisation.
 */

import { useState, useEffect, useRef, useCallback } from 'react';

interface UseWebSocketOptions {
  onMessage: (data: any) => void;
  onConnect?: () => void;
  onDisconnect?: () => void;
  reconnectInterval?: number;
  heartbeatInterval?: number;
}

export function useWebSocket(url: string, options: UseWebSocketOptions) {
  const [isConnected, setIsConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout>>();
  const heartbeatIntervalRef = useRef<ReturnType<typeof setInterval>>();

  const connect = useCallback(() => {
    try {
      const ws = new WebSocket(url);
      wsRef.current = ws;

      ws.onopen = () => {
        setIsConnected(true);
        setError(null);
        options.onConnect?.();

        // Démarrer le heartbeat (ping toutes les 30s)
        heartbeatIntervalRef.current = setInterval(() => {
          if (ws.readyState === WebSocket.OPEN) {
            ws.send(JSON.stringify({ action: 'ping' }));
          }
        }, options.heartbeatInterval || 30000);
      };

      ws.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);
          options.onMessage(data);
        } catch (e) {
          console.error('[WS] Parse error:', e);
        }
      };

      ws.onerror = (event) => {
        setError('WebSocket connection error');
      };

      ws.onclose = () => {
        setIsConnected(false);
        options.onDisconnect?.();
        clearInterval(heartbeatIntervalRef.current);

        // Reconnexion automatique
        reconnectTimeoutRef.current = setTimeout(
          connect,
          options.reconnectInterval || 5000
        );
      };
    } catch (e) {
      setError(`Connection failed: ${e}`);
      reconnectTimeoutRef.current = setTimeout(
        connect,
        options.reconnectInterval || 5000
      );
    }
  }, [url, options]);

  useEffect(() => {
    connect();

    return () => {
      clearTimeout(reconnectTimeoutRef.current);
      clearInterval(heartbeatIntervalRef.current);
      wsRef.current?.close();
    };
  }, [connect]);

  // Fonction pour envoyer une commande
  const sendCommand = useCallback((command: any) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      wsRef.current.send(JSON.stringify({ action: 'command', payload: command }));
    } else {
      setError('Cannot send command — not connected');
    }
  }, []);

  return { isConnected, error, sendCommand };
}
