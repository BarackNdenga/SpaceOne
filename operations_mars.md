# SpaceOne Phase 3 — Opérations Mars (Production)

## 1. Vue d'ensemble des opérations

La Phase 3 représente le début des opérations réelles sur la surface martienne. SpaceOne n'est plus un logiciel isolé, mais un middleware embarqué dans une distribution Linux industrielle (AsterQuanta OS). Le système est conçu pour être "Earth-independent" (autonome), capable de gérer les pannes, les radiations et les déconnexions réseau prolongées avec la Terre.

## 2. Séquence de Boot Réelle (Mars)

La séquence de démarrage sur Mars est orchestrée par le bootloader GRUB et systemd. Elle est conçue pour être résiliente aux Single Event Upsets (SEU).

1. **Firmware & Bootloader** : GRUB lit le slot actif (A ou B) via la variable d'environnement RAUC.
2. **Kernel Linux (AsterQuanta)** : Le noyau démarre, monte la rootfs en lecture seule (overlay pour `/etc` et `/var`).
3. **systemd (PID 1)** : Orchestre le démarrage des services.
4. **Services de base (L4)** : `aqm-supervisor`, `aqm-dtnd` (uD3TN), `aqm-recovery` (snapshots BTRFS).
5. **SpaceOne Core (L5)** : `spaceone-core.service` démarre après que la supervision est stable.
6. **Modules SpaceOne** :
   - `spaceone-hal.service` (Protection radiations : ECC, TMR)
   - `spaceone-communication.service` (Gestion DTN++)
   - `spaceone-coordination.service` (Scheduler autonome)
   - `spaceone-health.service` (Auto-recovery)
7. **GUI Kiosk (L6)** : `aqm-ui` s'affiche sur l'écran (si présent).

## 3. Gestion des Pannes et Safe Mode

Le système utilise une approche de défense en profondeur (Defense in Depth) :

### 3.1 Pannes Logicielles (Deadlocks, Panics)
- **Détection** : `aqm-supervisor` vérifie les heartbeat des services via `systemctl show`.
- **Action** : systemd redémarre automatiquement le service défaillant (`Restart=always`).

### 3.2 Pannes Matérielles (Radiations, SEL)
- **Détection** : `spaceone-hal` surveille les erreurs ECC mémoire et les surintensités (SEL).
- **Action** :
  1. Soft Recovery : Restart du composant affecté.
  2. Hard Recovery : Redémarrage hardware via watchdog.
  3. Failover : Bascule sur le composant redondant.
  4. Safe Mode : Déclenchement de `aqmctl safe-mode on`.

### 3.3 Le Safe Mode (aqm-safe.target)
Lorsque le Safe Mode est activé :
- Le système isole `aqm-safe.target` (via `systemctl isolate`).
- La GUI (`aqm-ui`) et les services lourds (coordination, science) sont arrêtés.
- Seuls les services critiques restent actifs : `aqm-supervisor`, `sshd`, `aqm-shell` (CLI).
- `spaceone-health` préserve les données sur la partition `/data` (BTRFS).

## 4. Mise à jour Firmware (Rollforward / Rollback)

Les mises à jour OTA (Over-The-Air) sont gérées par RAUC, intégrée dans AsterQuanta OS.

1. **Réception** : `aqm-dtnd` reçoit le bundle `.raucb` signé via le réseau delay-tolerant.
2. **Vérification** : `spaceone-communication` vérifie la signature Ed25519 et l'intégrité.
3. **Installation** : RAUC installe le bundle sur le slot inactif (ex: Slot B si A est actif).
4. **Reboot** : Le système redémarre sur le nouveau slot.
5. **Validation** : SpaceOne confirme le boot sain (`rauc status mark-good`).
6. **Rollback** : Si le boot échoue ou si le watchdog se déclenche, GRUB/RAUC rebascule automatiquement sur l'ancien slot.

## 5. Architecture Réseau (DTN++)

Le réseau martien est "delay-tolerant" (latence de 3 à 20 minutes, déconnexions fréquentes).

- **Transport** : `aqm-dtnd` (uD3TN, RFC 9171) gère le bundle protocol de bas niveau.
- **SpaceOne DTN++** : S'appuie sur `aqm-dtnd` via le socket AAP (`/run/aqm/aqm-dtnd.aap.sock`).
- **Priorités** : SpaceOne gère les priorités (Expedited pour safe mode, Bulk pour la science).
- **Store-and-Forward** : Les bundles sont stockés sur la partition `/data` en attendant un contact avec l'orbiteur relay.

## 6. Commandes Mission Control (CLI)

L'administration du système se fait via `aqmctl` (aqm-shell), qui pilote les services systemd et RAUC.

| Commande | Action Système |
|----------|----------------|
| `aqmctl status` | Interroge `aqm-supervisor` via socket UNIX |
| `aqmctl update install <bundle>` | Déclenche RAUC pour installer sur le slot inactif |
| `aqmctl safe-mode on` | Bascule vers `aqm-safe.target` |
| `aqmctl safe-mode off` | Reprise normale (`multi-user.target`) |
| `aqmctl recovery` | Bascule vers le kernel de recovery minimal |
| `aqmctl dtn status` | Vérifie la connectivité DTN |

## 7. Récupération des Données (Recovery)

La partition `/data` utilise le système de fichiers BTRFS pour la résilience.

- **Snapshots** : `aqm-recoveryd` prend des snapshots automatiques au boot et avant chaque update.
- **Restauration** : En cas de corruption logique, `aqmctl recovery` permet de monter un snapshot précédent.
- **Intégrité** : Les données scientifiques sont hachées (SHA3-256) avant stockage.
