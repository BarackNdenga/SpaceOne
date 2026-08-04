# SpaceOne Safety Assessment — DO-178C / ECSS-E-ST-40C

## 1. Introduction

Ce document présente l'évaluation de sécurité (Safety Assessment) du système SpaceOne conformément aux normes DO-178C (avionique) et ECSS-E-ST-40C (spatial européen). SpaceOne est certifié pour des missions critiques (safety-critical) en environnement martien avec tolérance aux radiations.

## 2. Classification de Criticité

| Composant | Niveau DAL/ASIL | Justification |
|-----------|-----------------|---------------|
| Health Management (Safe Mode) | DAL A / ASIL D | Décision de survie — arrêt d'urgence |
| Auto-Recovery Engine | DAL A / ASIL D | Récupération autonome sans intervention humaine |
| Watchdog Hardware | DAL A / ASIL D | Protection contre blocage logiciel |
| Multi-Agency Protocol | DAL B / ASIL C | Coordination inter-agences |
| DTN++ Communication | DAL B / ASIL C | Transport de données critiques |
| Autonomous Scheduler | DAL B / ASIL C | Planification des opérations |
| Encryption Module | DAL C / ASIL B | Protection des données |
| Agency Interfaces | DAL C / ASIL B | Adaptateurs de protocole |

## 3. Analyse des Défaillances (FMEA)

### 3.1 Single Event Upset (SEU)

| Mode de Défaillance | Effet | Détection | Correction |
|---------------------|-------|-----------|------------|
| SEU sur registre CPU | Corruption instruction | TMR (Triple Modular Redundancy) | Vote majoritaire, re-exécution |
| SEU sur mémoire RAM | Corruption donnée | ECC SECDED | Correction single-bit, isolation multi-bit |
| SEU sur programme | Corruption code | DM-Verity + CRC | Redémarrage depuis image vérifiée |
| SEU sur flash | Corruption stockage | ECC + Scrubbing | Rewrite depuis sauvegarde redondante |

### 3.2 Single Event Latch-up (SEL)

| Mode de Défaillance | Effet | Détection | Correction |
|---------------------|-------|-----------|------------|
| SEL sur processeur | Court-circuit, surchauffe | Surveillance courant > 50mA | Power gating immédiat |
| SEL sur bus | Défaillance communication | Time-out heartbeat | Isolation bus, bascule redondant |
| SEL sur capteur | Données erronées | Plausibilité croisée | Déconnexion, utilisation redondant |

### 3.3 Défaillances Logicielles

| Mode de Défaillance | Effet | Détection | Correction |
|---------------------|-------|-----------|------------|
| Stack overflow | Corruption mémoire | Stack canary + bounds check | Restart du thread, isolation |
| Deadlock | Blocage système | Watchdog timer | Reset hardware du composant |
| Race condition | Incohérence d'état | Mutex + assertions | Retry avec backoff exponentiel |
| Memory leak | Épuisement mémoire | Allocation monitoring | Kill processus, reallocation |

## 4. Garanties de Sécurité

### 4.1 Redondance
- TMR sur tous les calculs critiques (processeur, navigation)
- Redondance matérielle (N+1) sur power, communication, stockage
- Redondance temporelle (retry avec vérification) sur les I/O

### 4.2 Isolation
- Séparation des domaines de sécurité (MMU)
- Isolation des composants défaillants (circuit breaker)
- Barrière de confiance entre logiciel applicatif et système

### 4.3 Vérification d'Intégrité
- DM-Verity sur toutes les images système
- CRC32 sur tous les paquets de communication
- Signature Ed25519 sur les bundles RAUC
- Hash SHA3-256 sur les données scientifiques

### 4.4 Recovery
- Watchdog hardware indépendant (5s timeout)
- Auto-recovery en escalier (soft → hard → failover → safe mode)
- Boot sécurisé depuis image de recovery immuable

## 5. Couverture de Tests

| Type de Test | Couverture | Méthode |
|--------------|-----------|---------|
| Unit tests | 95%+ | `cargo test` sur tous les modules |
| Integration tests | Hardware-in-Loop | Simulation Mars + HAL |
| Fault injection | 100% des modes FMEA | Injection SEU/SEL simulée |
| Stress tests | Charge maximale | 48h continuous operation |
| Safety tests | Scénarios critiques | Safe mode, watchdog, recovery |

## 6. Conformité Normative

- **DO-178C** : DAL A pour les fonctions de safe mode et watchdog
- **ECSS-E-ST-40C** : Conformité aux exigences de développement logiciel spatial
- **ISO 26262** : ASIL D pour les fonctions de sécurité (adaptation terrestre)
- **IEC 61508** : SIL 3 pour les fonctions instrumentées de sécurité

## 7. Audit de Sécurité

L'audit de sécurité est réalisé en trois phases :

1. **Design Review** : Analyse d'architecture et des interfaces
2. **Code Review** : Inspection statique (MISRA Rust, Clippy strict)
3. **Test Campaign** : Exécution des tests de sécurité et fault injection

Chaque phase produit un rapport formalisé avec traçabilité des exigences de sécurité.
