# SpaceOne — Architecture Technique

## 1. Vue d'Ensemble

SpaceOne est une infrastructure logicielle **safety-critical** et **radiation-tolerant** conçue pour les opérations martiennes Earth-independent. Le système est organisé en trois couches principales :

| Couche | Composants | Fonction |
|--------|-----------|----------|
| L1 — Kernel | AsterQuanta OS | Temps réel, tolérance radiations, bare-metal |
| L2 — Flight Software | Coordination, Communication, Health | Logique missionnelle embarquée |
| L3 — Ground Software | Mission Control, Data Processor | Supervision et analyse depuis la Terre |

## 2. Modèle de Concurrence

Tous les modules flight software utilisent le modèle `async/await` de Tokio avec des coroutines isolées par tâche. Chaque composant fonctionne comme un acteur indépendant avec une file de messages interne, garantissant :

- **Pas de data races** (garanti par le borrow checker de Rust)
- **Déterminisme** pour la certification safety-critical
- **Isolation des fautes** : un module défaillant ne propage pas la panne

## 3. Module de Coordination

### 3.1 Multi-Agency Protocol (MAP)

Le protocole MAP permet à des entités de différentes agences spatiales (NASA, SpaceX, ESA, CNSA) de coordonner leurs opérations sur Mars sans dépendance centralisée.

```
Rover NASA ──MAP──→ Habitat ESA ──MAP──→ Orbiteur SpaceX ──MAP──→ Drone CNSA
   │                    │                    │                      │
   └────────────────────┴────────────────────┴──────────────────────┘
                              Consensus Distribué
```

Le MAP implémente :
- **Query** : Demande d'état/position/ressources
- **Command** : Ordre avec priorité et deadline
- **Response** : Confirmation/échec avec telemetry
- **Negotiation** : Résolution de conflits de ressources

### 3.2 Autonomous Scheduler

Le planificateur autonome utilise un modèle ONNX embarqué (radiation-tolerant par triple modular redundancy) pour :
- Prioriser les tâches en temps réel
- Résoudre les conflits entre agences
- Générer des plans de contingence (safe mode, re-planning)

### 3.3 Distributed PNT

Système de Positioning, Navigation et Timing distribué qui fonctionne sans GPS (inexistant sur Mars) :
- **Relative Localization** : Triangulation entre rovers/orbiteur
- **Distributed Timing** : Horloges synchronisées par échange de messages
- **Orbit Determination** : Calcul orbital distribué pour l'orbiteur

## 4. Module de Communication

### 4.1 DTN++ (Delay-Tolerant Networking Avancé)

Extension du standard RFC 9171 (Bundle Protocol) avec :
- **Priority Bundles** : Classification en 8 niveaux de priorité
- **AI Compression** : Compression adaptative par réseau neuronal embarqué
- **Store-and-Forward** : Buffering intelligent en cas de blackout de communication

### 4.2 Mesh Network

Réseau maillé ad-hoc entre tous les assets martiens :
- **Routing** : Protocole de routage adaptatif au terrain martien
- **Discovery** : Découverte automatique de nouveaux noeuds
- **Topology** : Gestion dynamique de la topologie réseau

## 5. Health Management

Le système de gestion de santé est la pierre angulaire de la safety-critical :

```
┌─────────────────────────────────────────────┐
│           Health Management                 │
├──────────┬──────────┬───────────────────────┤
│ Diagnostique│ Auto-  │ Crew Decision         │
│ Distribué   │Recovery│ Support               │
├──────────┼──────────┼───────────────────────┤
│ • Capteurs │ • Reboot│ • Alertes priorisées  │
│ • Logs     │ • Roll- │ • Procédures          │
│ • Metrics  │   back  │   d'urgence           │
│            │ • Safe  │ • Redondances         │
│            │   Mode  │   suggérées           │
└──────────┴──────────┴───────────────────────┘
```

## 6. Sécurité

- **Chiffrement** : AES-256-GCM pour les données en transit, cryptographie post-quantique (Kyber/Dilithium) pour les clés
- **Signature** : Bundles RAUC signés avec Ed25519
- **Contrôle d'accès** : RBAC avec capabilities (capabilities-based security)
- **Intégrité mémoire** : ECC + scrubbing continu contre les SEU (Single Event Upsets)

## 7. Tolérance aux Radiations

| Technique | Implémentation | Protection |
|-----------|---------------|------------|
| TMR (Triple Modular Redundancy) | 3 copies de chaque calcul critique | SEU, SET |
| ECC Memory | Code correcteur 64 bits | Bit flips mémoire |
| Memory Scrubbing | Nettoyage périodique des pages | Accumulation d'erreurs |
| Watchdog Hardware | Timer indépendant reset | Blocages logiciels |
| Power Gating | Isolation des circuits non-critiques | SEL (Latch-up) |

## 8. Dépendances Externes

| Dépendance | Usage | Pourquoi |
|-----------|-------|----------|
| Tokio | Runtime async | Performance temps réel, gestion threads |
| Serde | Sérialisation | Interopérabilité bundles DTN |
| AES-GCM | Chiffrement | Standard NIST pour données sensibles |
| SHA-3 | Hashing | Intégrité des bundles |
| ONNX Runtime | IA embarquée | Inference locale du scheduler |
| RAUC | Mise à jour OTA | Bootloader A/B avec signature |

## 9. Flux de Données

```
Terre ──(6-9 mois)──→ Mars
  │                      │
  ├─ Command Bundles ────→│
  │                      ├─ Telemetry Bundles ──→
  │                      │
  │    Rover ←──DTN++──→ Habitat ←──DTN++──→ Orbiteur
  │     │                    │                    │
  │     └──────────Mesh Network───────────────┘
  │
  └─ Mission Control ←──(6-9 mois)──┘
```

## 10. Spécifications de Performance

| Paramètre | Valeur | Justification |
|-----------|--------|---------------|
| Latence max commande | 200ms | Temps critique rover |
| Débit DTN min | 1 kbps | Distance Terre-Mars |
| MTBF | 10 000 heures | Mission longue durée |
| Memory ECC | 100% pages | Protection radiations |
| Redondance | Triple (TMR) | Safety-critical DAL-A |
| Boot time | < 5 secondes | Recovery rapide |
