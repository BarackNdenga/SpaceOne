# SpaceOne — Deployment Guide (Terre → Mars)

## 1. Vue d'ensemble du flux de déploiement

Le déploiement de SpaceOne suit un flux en 4 étapes : build sur Terre, intégration avec le lanceur, transit Terre-Mars (6-9 mois), et déploiement sur la surface martienne.

## 2. Phase de Build (Terre)

### 2.1 Prérequis

| Élément | Version | Rôle |
|---------|---------|------|
| Rust Toolchain | 1.78+ | Compilation des binaires |
| RAUC | 1.8+ | Création des bundles firmware |
| OpenSSL | 3.0+ | Signature cryptographique |
| Docker | 24+ | Containerisation de l'environnement de build |

### 2.2 Build des Images

Pour chaque plateforme cible, une image distincte est générée :

```bash
# Rover
./tools/deployment_tools/build_flight_image.sh mars_rover

# Habitat
./tools/deployment_tools/build_flight_image.sh mars_habitat

# Orbiteur
./tools/deployment_tools/build_flight_image.sh orbiter
```

Chaque image contient :
- Le binaire SpaceOne compilé (release, LTO activé)
- La configuration plateforme spécifique (`platform_config.toml`)
- Les clés de vérification DM-Verity
- Le manifest RAUC signé

### 2.3 Signature et Vérification

Chaque image est signée avec Ed25519 et vérifiée par :
1. Le Mission Control Ground Station (Terre)
2. L'orbiteur relay (pendant le transit)
3. Le bootloader sécurisé de la plateforme cible

## 3. Phase de Transit (6-9 mois)

### 3.1 Intégration avec le Lanceur

SpaceOne est pré-installé dans la mémoire flash protégée de chaque plateforme. Pendant le transit, le système reste en mode hibernation avec :
- Watchdog hardware actif (surveille les défaillances)
- Memory scrubbing périodique (anti-SEU)
- Surveillance thermique passive

### 3.2 Updates pendant le Transit

Pendant le transit Terre-Mars, des mises à jour mineures peuvent être envoyées via les communications deep space :
- Correctifs de bugs critiques
- Ajustements de paramètres de trajectoire
- Mises à jour des clés cryptographiques

## 4. Phase de Déploiement (Mars)

### 4.1 Séquence de Boot

Lors de l'arrivée sur Mars, la séquence de boot est la suivante :

```
1. Bootloader sécurisé → vérifie la signature de l'image
2. DM-Verity → vérifie l'intégrité de la rootfs
3. AsterQuanta OS → démarre le kernel
4. SpaceOne init → démarre les services :
   a. HAL (radiation protection)
   b. Health Management (monitoring)
   c. DTN++ (communication)
   d. Scheduler (planification)
   e. Multi-Agency Protocol (coordination)
```

### 4.2 Auto-Configuration

Au premier boot sur Mars, SpaceOne exécute :
- Détection automatique de l'environnement (capteurs, bus)
- Calibration des instruments scientifiques
- Établissement des communications avec l'orbiteur relay
- Synchronisation de l'heure avec le DSN

### 4.3 Rollback en cas d'échec

Si le boot échoue (checksum invalide, panique kernel) :
1. Le bootloader bascule sur la partition de recovery
2. L'image de recovery démarre en mode safe minimum
3. Les diagnostics sont envoyés à la Terre
4. Une nouvelle image peut être uploadée via l'orbiteur

## 5. Commandes de Mission Control

### 5.1 Commandes standard

| Commande | Description | Risque |
|----------|-------------|--------|
| `status` | État courant de la plateforme | Aucune |
| `health` | Rapport de santé complet | Aucune |
| `update` | Mise à jour firmware | Moyen |
| `safe_mode` | Entrée en mode safe | Faible |
| `reboot` | Redémarrage | Moyen |
| `command` | Commande arbitraire | Élevé |

### 5.2 Protocole de commande

Chaque commande suit le protocole :
1. Envoi depuis Mission Control (DTN bundle)
2. Transit via l'orbiteur relay (5-20 min)
3. Réception et vérification de signature
4. Contrôle RBAC (authorization check)
5. Exécution
6. Accusé de réception (telemetry retour)

## 6. Maintenance en Mission

### 6.1 Mises à jour OTA

Les mises à jour Over-The-Air sont gérées par RAUC :
- Download du bundle via DTN++
- Vérification de signature (offline)
- Installation sur la partition inactive
- Reboot et vérification
- Rollback automatique si échec

### 6.2 Récupération de panne

En cas de panne logicielle :
1. Watchdog détecte le blocage (< 5s)
2. Reset hardware du composant affecté
3. Auto-recovery (soft → hard → failover)
4. Si échec : safe mode + notification Terre

### 6.3 Monitoring continu

Le Mission Control maintient :
- Dashboard de santé en temps réel (latence Mars-Terre)
- Alertes automatiques (anomalies critiques)
- Logs immutables (audit trail hash-chaîné)
- Métriques de performance (CPU, mémoire, réseau)
