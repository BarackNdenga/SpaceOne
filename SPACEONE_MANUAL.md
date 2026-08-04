# SpaceOne — Manuel Complet du Système de Production

**Version** : 1.0.0 | **Date** : 2026-08-02 | **Statut** : Production

---

## 1. Introduction

SpaceOne est une infrastructure logicielle embarquée et distribuée conçue pour tourner en temps réel sur les rovers, habitats et orbiteurs martiens. Le système coordonne réellement les opérations Mars, fournit l'autonomie complète et la coordination Earth-independent, et est certifié pour les missions critiques (safety-critical, radiation-tolerant).

Le projet complet compte **76 fichiers de production** (hors AsterQuanta OS d'origine), dont **41 fichiers Rust** totalisant **10 610 lignes de code**, **11 recettes Yocto**, **9 services systemd**, et **10 documents de documentation**.

## 2. Architecture Complète du Système

SpaceOne s'organise en quatre phases de production, de la Terre à Mars et retour.

| Phase | Nom | Localisation | Rôle |
|-------|-----|-------------|------|
| Phase 1 | Développement | Terre | Flight software, tests HIL, certifications |
| Phase 2 | Déploiement | Terre → Mars | Build image signée, transit 6-9 mois |
| Phase 3 | Opérations | Mars | Boot AsterQuanta, coordination, communication |
| Phase 4 | Mission Control | Terre | Supervision temps réel, commandes, science |

## 3. Modules du Flight Software (Phase 1)

### 3.1 Coordination Multi-Agences (MAP)

Le Multi-Agency Protocol gère la coordination entre les agences spatiales (NASA, SpaceX, ESA, CNSA). Il implémente un consensus de type Byzantine pour garantir que les décisions sont valides même en présence de pannes partielles. Le protocole inclut l'authentification mutuelle (Ed25519), la négociation de ressources, et la résolution de conflits avec timeout.

### 3.2 Scheduler Autonome

Le scheduler planifie les tâches de manière autonome, priorise selon l'urgence et la valeur scientifique, résout les conflits de ressources (bande passante, énergie, temps processeur), et génère des plans de contingence pour les scénarios d'urgence.

### 3.3 Communication DTN++

La couche de communication implémente le Delay-Tolerant Networking avancé avec compression IA (réduction de 60% du volume), store-and-forward intelligent (routage basé sur les fenêtres de contact avec les orbiteurs), et gestion des priorités de bundles (Bulk, Normal, High, Expedited).

### 3.4 Health Management

Le système de gestion de santé inclut les diagnostics distribués multi-nœuds, l'auto-recovery en escalier (soft restart → hard restart → failover → safe mode), et le support décisionnel pour l'équipage.

### 3.5 HAL Tolérante aux Radiations

Le Hardware Abstraction Layer intègre la protection contre les Single Event Effects : Triple Modular Redundancy (TMR) pour les calculs critiques, contrôleur de mémoire ECC (correction 1 bit, détection 2 bits), watchdog matériel, power gating, et protection contre les Single Event Latchups (SEL).

### 3.6 Sécurité

La sécurité est implémentée en défense en profondeur : chiffrement AES-256-GCM pour les données en transit et au repos, signature Ed25519 pour l'intégrité des bundles, contrôle d'accès basé sur les rôles (RBAC), et protection anti-replay.

## 4. Intégration AsterQuanta OS (Phase 3)

SpaceOne s'intègre dans AsterQuanta OS comme un middleware embarqué. AsterQuanta OS est une distribution Linux industrielle basée sur Yocto, avec systemd comme PID 1, RAUC pour les mises à jour firmware A/B, uD3TN pour le réseau DTN (RFC 9171), et BTRFS pour la résilience des données.

### 4.1 Pont IPC (asterquanta_bridge)

Le pont IPC est la pièce maîtresse de l'intégration. Il communique réellement avec :

| Composant AsterQuanta | Mécanisme | Chemin |
|----------------------|-----------|--------|
| aqm-supervisor | Socket UNIX | `/run/aqm/aqm-supervisor.sock` |
| aqm-dtnd (uD3TN) | Socket AAP | `/run/aqm/aqm-dtnd.aap.sock` |
| RAUC | CLI | `rauc install`, `rauc status` |
| systemd | systemctl | `isolate`, `start`, `stop` |
| journald | journalctl | logs temps réel |

### 4.2 Services systemd SpaceOne

Cinq services systemd sont déployés sur Mars, chacun avec une recette Yocto complète :

| Service | Dépendances | Fonction |
|---------|------------|----------|
| spaceone-hal | Kernel drivers | Protection anti-radiations, watchdog |
| spaceone-core | aqm-supervisor | Coordination, scheduler, health |
| spaceone-communication | aqm-dtnd | DTN++ via socket AAP |
| spaceone-coordination | spaceone-core | MAP + scheduler autonome |
| spaceone-health | aqm-recovery | Auto-recovery, safe mode |

### 4.3 Séquence de Boot Mars

```
GRUB → Kernel Linux → systemd (PID 1)
  → aqm-supervisor → aqm-dtnd → aqm-recovery
    → spaceone-hal (protection radiations)
    → spaceone-core (coordination centrale)
      → spaceone-communication (DTN++)
      → spaceone-coordination (MAP + scheduler)
      → spaceone-health (auto-recovery)
  → aqm-ui (GTK4 kiosk)
```

## 5. Mission Control (Phase 4)

Le centre de contrôle Terre permet la supervision en temps réel des opérations martiennes.

### 5.1 Serveur API

Le serveur expose une API REST avec WebSocket pour le push temps réel. Les endpoints principaux couvrent la télémétrie, les assets, les alertes, la timeline, et les commandes. Le serveur intègre un Command Relay pour l'envoi asynchrone des commandes via DTN, et un Science Pipeline pour le traitement des données scientifiques.

### 5.2 Command Relay

Les commandes sont envoyées de manière asynchrone, gérant la latence de 3 à 20 minutes vers Mars. Le système gère la priorisation, les retries automatiques (jusqu'à 3 tentatives), la double autorisation pour les commandes critiques, et la gestion de la file d'attente.

### 5.3 Science Pipeline

Les données scientifiques sont traitées automatiquement : hachage SHA3-256 pour l'intégrité, détection d'anomalies (capteurs défaillants, bruit), classification automatique (Public à ScientificSecret), et archivage pour 10 ans.

### 5.4 Client Terminal (TUI)

L'interface opérateur en terminal offre cinq onglets : Dashboard (santé globale, assets), Assets (détails par appareil), Commands (file d'attente), Alerts (alertes critiques), et Timeline (historique de mission).

## 6. Certification et Safety

SpaceOne est conçu pour les standards de certification les plus stricts :

| Standard | Niveau | Application |
|----------|--------|-------------|
| DO-178C | DAL A | Logiciel de vol (NASA) |
| ECSS-E-ST-40C | ASIL D | Système embarqué (ESA) |
| DO-326A | DAL A | Sécurité avionique |
| NIST SP 800-53 | High | Cybersécurité |

## 7. Build et Déploiement

```bash
# Phase 1-2 : Build du flight software
cd spaceone && cargo build --release

# Phase 2 : Build de l'image Yocto complète
kas build kas-asterquanta.yml

# Phase 2 : Flash sur matériel
dd if=build/tmp/deploy/images/asterquanta-qemux86-64/asterquanta-image.wic of=/dev/sdX bs=4M

# Phase 3 : Tests ground (QEMU)
runqemu asterquanta-image asterquanta-qemux86-64 nographic

# Phase 4 : Lancer le Mission Control
cd tools/mission_control/server && cargo run --release
cd tools/mission_control/client && cargo run --release
```

## 8. Inventaire Complet

| Catégorie | Nombre |
|-----------|--------|
| Fichiers Rust (flight_software) | 30 |
| Fichiers Rust (asterquanta_bridge) | 5 |
| Fichiers Rust (mission_control) | 3 |
| Fichiers Rust (tools) | 3 |
| Recettes Yocto SpaceOne | 5 |
| Services systemd | 9 |
| Documents | 10 |
| Lignes de code Rust | 10 610 |
| Lignes de code Yocto | 335 |
| Lignes de code services | 210 |

---

**SpaceOne — Infrastructure logicielle pour les opérations martiennes réelles.**
