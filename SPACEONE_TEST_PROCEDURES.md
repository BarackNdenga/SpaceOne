# SpaceOne Test Procedures

**Document ID:** SO-TP-001
**Version:** 1.0
**Date:** 2026-08-02

## 1. Introduction

Ce document détaille les procédures de test pour valider le système SpaceOne avant le déploiement sur Mars. Il couvre les tests logiciels, les tests d'intégration QEMU, et les tests Hardware-in-Loop (HIL).

## 2. Tests Logiciels (Unitaires et Intégration)

### 2.1 Compilation et Build
- **Objectif:** Valider que le code source compile sans erreur sur toutes les cibles.
- **Procédure:**
  1. Exécuter `./ground_testing/build_all.sh native release`
  2. Vérifier la présence des binaires dans `build/artifacts`.
  3. Exécuter `./ground_testing/build_all.sh arm64 release` pour valider la cross-compilation.
- **Critère de succès:** Build 0 erreur, 0 warning critique, hash SHA3-256 valides.

### 2.2 Tests Unitaires Rust
- **Objectif:** Valider la logique interne (Scheduler, DTN, Security).
- **Procédure:**
  1. Exécuter `cargo test --release`.
  2. Vérifier les résultats dans `test_results.log`.
- **Critère de succès:** 100% des tests passent.

## 3. Tests d'Intégration QEMU

### 3.1 Boot Complet
- **Objectif:** Vérifier que l'image AsterQuanta+SpaceOne boot correctement.
- **Procédure:**
  1. Lancer `./ground_testing/integration/qemu_integration_tests.sh`.
  2. Observer les logs QEMU.
- **Critère de succès:** Boot complet en < 30 secondes, services systemd actifs.

### 3.2 IPC et Communication
- **Objectif:** Valider que SpaceOne communique avec le Superviseur et le DTN.
- **Procédure:**
  1. Connecter via SSH au QEMU.
  2. Vérifier les sockets dans `/run/aqm/`.
  3. Envoyer un bundle test via `aqm-dtnd`.
- **Critère de succès:** Échange de messages réussi, bundles reçus.

## 4. Tests Hardware-in-Loop (HIL)

### 4.1 Résilience aux Radiations (Simulation)
- **Objectif:** Valider que le TMR et l'ECC corrigent les erreurs mémoire.
- **Procédure:**
  1. Lancer `./ground_testing/hil_automation/run_hil_tests.sh raspberry_pi_4`.
  2. Le script injectera 10 erreurs SEU simulées.
- **Critère de succès:** 10 erreurs corrigées, 0 redémarrage (watchdog non déclenché).

### 4.2 Stress Thermique
- **Objectif:** Vérifier le comportement du CPU sous températures extrêmes (-80°C à +50°C).
- **Procédure:**
  1. Le script HIL simule les effets de la température sur les performances.
- **Critère de succès:** Temps de réponse < 5s à toutes les températures.

### 4.3 Cycles d'Alimentation
- **Objectif:** Valider la robustesse lors des coupures de courant (simule les tempêtes de poussière).
- **Procédure:**
  1. Le script HIL coupe et remet l'alimentation 100 fois.
- **Critère de succès:** 100% des reboots réussis, aucune perte de données BTRFS.

## 5. Tests Mission Control

### 5.1 Commandes Asynchrones
- **Objectif:** Valider l'envoi et le suivi des commandes via le DSN.
- **Procédure:**
  1. Démarrer le serveur et le client Web UI.
  2. Envoyer une commande "Critical" et une "Routine".
- **Critère de succès:** La commande critique demande la double autorisation. La commande routine est immédiatement enfilée.
