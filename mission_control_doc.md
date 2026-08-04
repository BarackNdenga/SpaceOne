# SpaceOne Phase 4 — Mission Control (Centre de Contrôle Terre)

## 1. Vue d'ensemble du Centre de Contrôle

Le Mission Control de SpaceOne est le centre de supervision des opérations martiennes depuis la Terre. Il est conçu pour gérer la latence de communication (3 à 20 minutes aller simple) et les déconnexions fréquentes inhérentes aux communications interplanétaires. Le système est entièrement asynchrone et ne repose sur aucune simulation.

## 2. Architecture du Système

Le centre de contrôle est divisé en deux composants principaux :

### 2.1 Serveur API (Backend)
- **Technologie** : Rust (Axum, Tokio)
- **Fonctions** :
  - Exposition d'une API REST pour les données de télémétrie.
  - WebSocket pour le push temps réel vers les dashboards.
  - File de commandes asynchrone (Command Relay) via DTN.
  - Pipeline de traitement des données scientifiques (Science Pipeline).

### 2.2 Client Terminal (Frontend)
- **Technologie** : Rust (Ratatui, Crossterm)
- **Fonctions** :
  - Interface utilisateur en terminal (TUI) pour les opérateurs.
  - Affichage temps réel des assets (rover, habitat, orbiteur).
  - Gestion des alertes et validation des commandes.

## 3. Commandes Asynchrones (DTN Command Relay)

Puisque les communications avec Mars ne sont pas instantanées, le système utilise un **Relay DTN** pour gérer les commandes.

- **Enfilement** : Les opérateurs créent une commande (ex: `move forward 10m`).
- **Validation** : Le système vérifie le RBAC (Role-Based Access Control) et la faisabilité.
- **Priorisation** : Les commandes sont classées (Routine, High, Critical, SafeMode).
- **Transmission** : Le relay envoie la commande via le réseau DTN.
- **Retry** : En cas d'échec de transmission, le système tente jusqu'à 3 reprises.
- **Accusé de réception** : Le système attend la confirmation d'exécution, qui peut prendre plusieurs heures à parvenir sur Terre.

## 4. Traitement des Données Scientifiques

Le **Science Pipeline** traite les données envoyées par Mars :

1. **Réception** : Les bundles DTN sont reçus et décompressés.
2. **Vérification d'intégrité** : Hachage SHA3-256 pour valider les données.
3. **Détection d'anomalies** : Algorithmes pour détecter les capteurs défaillants (données nulles, saturées, ou bruit).
4. **Classification** : Classification automatique (Public, Internal, Confidential, ScientificSecret).
5. **Stockage** : Archivage pour une durée de 10 ans minimum.

## 5. Sécurité et Contrôle d'Accès

Le centre de contrôle intègre des mesures de sécurité strictes :
- **RBAC** : Rôles définis (MissionCommander, FlightDirector, Scientist, Engineer, Operator).
- **MFA** : Authentification multi-facteurs requise.
- **Double Autorisation** : Les commandes critiques (ex: Safe Mode, Redémarrage) nécessitent l'approbation de deux opérateurs différents.
- **Audit** : Journalisation complète de toutes les actions pour traçabilité.

## 6. Intégration avec SpaceOne (Mars)

Le Mission Control communique avec le système embarqué SpaceOne via le protocole DTN++ :
- **Télémétrie** : SpaceOne envoie régulièrement des `MissionTelemetry` bundles.
- **Alertes** : SpaceOne déclenche des alertes en cas de score de santé critique.
- **Commandes** : Le Mission Control envoie des commandes qui sont exécutées par le `spaceone-coordination` sur Mars.
