# SpaceOne Failure Modes and Effects Analysis (FMEA)

**Document ID:** SO-FMEA-001
**Version:** 1.0
**Date:** 2026-08-02

## 1. Introduction

Cette analyse (FMEA) identifie les modes de défaillance potentiels du système SpaceOne, leurs effets, et les mécanismes de mitigation mis en œuvre. L'objectif est d'assurer la résilience du système en environnement martien hostile (radiations, froid extrême, déconnexions fréquentes).

## 2. Matrice de Risque

Les risques sont classés selon leur Sévérité (S), Occurrence (O), et Détectabilité (D). Le Risk Priority Number (RPN) est calculé comme S × O × D (maximum 1000).

| Composant | Mode de Défaillance | Effet | Sévérité (1-10) | Occurrence (1-10) | Détectabilité (1-10) | RPN | Mitigation |
|-----------|---------------------|-------|-----------------|-------------------|----------------------|-----|------------|
| **Hardware (SoC)** | Single Event Upset (SEU) | Bit flip en mémoire, crash calcul | 8 | 7 | 3 | 168 | TMR Hardware + ECC Mémoire |
| **Hardware (SoC)** | Single Event Latchup (SEL) | Court-circuit, destruction SoC | 10 | 2 | 2 | 40 | Circuit de protection SEL, Power Gating |
| **Software (Scheduler)** | Corruption de la file de tâches | Tâches perdues, mission bloquée | 7 | 3 | 5 | 105 | Double file persistante (BTRFS snapshots) |
| **Software (Coordination)** | Perte de consensus MAP | Incohérence inter-agences | 9 | 4 | 6 | 216 | Timeout + Default Safe Action |
| **Communication (DTN)** | Panne du lien radio (DSN) | Isolation totale | 10 | 5 | 2 | 100 | Store-and-forward local, Buffer DTN |
| **Network (IPC)** | Panne du socket superviseur | Perte de santé système | 8 | 2 | 4 | 64 | Watchdog indépendant, Redémarrage aqm-supervisor |
| **Thermal** | Surchauffe du SoC (>50°C) | Throttling, crash | 8 | 4 | 1 | 32 | Power Management, Throttling CPU, Safe Mode |
| **Power** | Chute de tension (<3.0V) | Redémarrage, perte de données | 9 | 3 | 1 | 27 | Batterie tampon, Arrêt gracieux (Graceful Shutdown) |

## 3. Modes de Défaillance Spécifiques

### 3.1 Radiation (SEU et SEL)
Les processeurs embarqués sont hautement sensibles aux rayonnements cosmiques. Un SEU peut modifier un bit en mémoire, altérant la logique. Un SEL peut créer un court-circuit mortel.
**Mitigation:** Le HAL implémente la Triple Modular Redundancy (TMR) pour les calculs critiques et l'Error Correcting Code (ECC) pour la mémoire. Le SEL est géré par une coupure d'alimentation physique.

### 3.2 Perte de Communication
Mars et la Terre peuvent être séparés par le Soleil, bloquant toute communication (Blackout).
**Mitigation:** Le protocole DTN++ utilise le mode "Store-and-Forward". Les données sont compressées par IA et stockées localement sur BTRFS jusqu'à ce que le lien soit rétabli.

### 3.3 Panne Logicielle (Crash)
Un bug dans le code Rust peut provoquer un panic.
**Mitigation:** SpaceOne est configuré avec `panic = "abort"`. En cas de crash, le watchdog matériel redémarre le service ou le système complet si le problème persiste.

## 4. Plan de Recovery

Le système dispose d'un système de recovery en escalier géré par le Health Management :
1. **Niveau 1:** Redémarrage du service (Soft Restart).
2. **Niveau 2:** Redémarrage de l'OS (Hard Restart).
3. **Niveau 3:** Bascule sur la slot B (Firmware Recovery via RAUC).
4. **Niveau 4:** Safe Mode (Isolement total, survie uniquement).
