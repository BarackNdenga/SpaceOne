# SpaceOne Interface Control Document (ICD)

**Document ID:** SO-ICD-001
**Version:** 1.0
**Date:** 2026-08-02

## 1. Introduction

Ce document (Interface Control Document) spécifie les interfaces logicielles et matérielles du système SpaceOne. Il sert de référence unique pour l'intégration entre le Flight Software, le système d'exploitation AsterQuanta, le Mission Control, et le Deep Space Network (DSN).

## 2. Interface Système d'Exploitation (IPC)

SpaceOne communique avec le système d'exploitation AsterQuanta via des sockets UNIX et des sockets AAP.

### 2.1 Socket Superviseur (`aqm-supervisor.sock`)
- **Type:** UNIX Domain Socket
- **Chemin:** `/run/aqm/aqm-supervisor.sock`
- **Protocole:** JSON-RPC 2.0 over Unix Socket
- **Fonctions:**
  - `get_health_status`: Récupère l'état global des services.
  - `trigger_recovery`: Déclenche une procédure de recovery.
  - `isolate_target`: Isole un service (ex: safe mode).

### 2.2 Socket DTN (`aqm-dtnd.aap.sock`)
- **Type:** UNIX Domain Socket (AAP Protocol)
- **Chemin:** `/run/aqm/aqm-dtnd.aap.sock`
- **Protocole:** Application Agent Protocol (RFC 9171)
- **Fonctions:**
  - `send_bundle`: Envoie un bundle DTN vers un EID (Endpoint ID).
  - `receive_bundle`: Récupère les bundles en attente.
  - `status`: Récupère les statistiques de la file DTN.

## 3. Interface Mission Control (REST & WebSocket)

Le Mission Control communique avec le Flight Software via une API REST pour l'état, et des WebSockets pour les données temps réel.

### 3.1 API REST
- **Base URL:** `http://localhost:3420/api`
- **Endpoints:**
  - `GET /telemetry`: État global (Health, Sol, Comm Delay).
  - `GET /assets`: État détaillé des assets (Rover, Habitat, Orbiter).
  - `GET /alerts`: Liste des alertes actives.
  - `POST /commands`: Soumission d'une commande asynchrone.
  - `GET /commands`: Statut de la file de commandes.

### 3.2 WebSocket
- **URL:** `ws://localhost:3420/ws`
- **Format:** JSON
- **Messages:**
  - `{"type": "ping"}`: Heartbeat (toutes les 30s).
  - `{"type": "telemetry_update", "payload": {...}}`: Push temps réel de l'état.

## 4. Interface Réseau (Deep Space Network)

Le système s'interface avec le DSN pour la transmission physique des données.

- **Fréquence:** Ka-Band (32 GHz)
- **Modulation:** PCM/PSK/PM (par défaut)
- **Puissance d'émission:** 43 dBm (20 kW)
- **Protocole:** Bundles DTN encapsulés en trames CCSDS Space Packet Protocol.

## 5. Interface Matérielle (HAL)

Le HAL s'interface avec le matériel embarqué (ex: i.MX8, LEON4).

- **Watchdog:** `/dev/watchdog` (Ping toutes les 10s).
- **Mémoire (EDAC):** `/sys/devices/system/edac/mc/mc0/` (Lecture des erreurs ECC).
- **Thermiques:** `/sys/class/thermal/thermal_zone0/temp` (Lecture de la température SoC).
- **Fréquence CPU:** `/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq`.
