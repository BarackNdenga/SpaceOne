# SpaceOne Operational Procedures

**Document ID:** SO-OP-001
**Version:** 1.0
**Date:** 2026-08-02

## 1. Introduction

Ce manuel d'opération est destiné aux opérateurs du Mission Control. Il détaille les procédures quotidiennes pour la supervision des assets martiens, la gestion des alertes, et l'envoi de commandes.

## 2. Séquence d'Ouverture de Quart (Shift Handover)

1. **Connexion au Système:**
   - Ouvrir le navigateur web et accéder à `http://localhost:3420`.
   - S'authentifier avec MFA (TOTP).
   - Vérifier que le WebSocket est "CONNECTED" (voyant vert).
2. **Vérification Globale:**
   - Consulter le Dashboard principal.
   - Vérifier le "Mission Health" (doit être > 80%).
   - Vérifier le nombre d'alertes actives (Pending Alerts).
3. **État des Assets:**
   - Aller dans l'onglet "Assets".
   - Vérifier l'état de chaque Rover, Habitat et Orbiteur (Nominal, Degraded, Safe Mode).
   - Vérifier le niveau de batterie et l'espace disque (Storage).
4. **File de Commandes:**
   - Aller dans l'onglet "Command Center".
   - Vérifier les commandes en transit (In Transit) et celles en attente d'exécution (Delivered/Executing).

## 3. Gestion des Alertes

1. **Détection:**
   - Une notification sonore et visuelle apparaît sur le Dashboard.
   - L'alerte apparaît dans l'onglet "Alerts".
2. **Évaluation:**
   - Lire le message de l'alerte et identifier l'asset concerné.
   - Vérifier la sévérité (Info, Warning, Critical, Emergency).
3. **Action:**
   - **Info/Warning:** Surveiller l'évolution.
   - **Critical/Emergency:** Requiert une intervention immédiate (envoi de commande Safe Mode ou Recovery).
4. **Acquittement:**
   - Cliquer sur le bouton "Acknowledge" après avoir pris connaissance de l'alerte.

## 4. Envoi de Commandes (Command Relay)

1. **Rédaction:**
   - Aller dans l'onglet "Command Center".
   - Remplir le formulaire : Commande, Asset Cible, Priorité.
2. **Autorisation:**
   - Si la priorité est "Critical" ou "Safe Mode", une double autorisation est requise.
   - Un second opérateur (Flight Director ou Mission Commander) doit confirmer.
3. **Envoi:**
   - Cliquer sur "Send Command (DTN Relay)".
   - La commande passe en statut "In Transit".
4. **Suivi:**
   - Attendre la confirmation d'exécution (peut prendre jusqu'à 40 minutes aller-retour).
   - Ne jamais renvoyer la même commande tant qu'elle n'est pas déclarée "Failed" ou "Completed".

## 5. Procédure d'Urgence (Emergency Response)

1. **Perte de Communication (DSN Down):**
   - Vérifier le statut du DSN.
   - Le système passe automatiquement en mode Store-and-Forward.
   - Continuer à envoyer les commandes, elles seront livrées dès que le lien sera rétabli.
2. **Safe Mode Déclenché par l'Asset:**
   - Le rover/habitat s'isole pour protéger le matériel.
   - Le Mission Control reçoit une alerte "Safe Mode".
   - Envoyer la commande de diagnostic pour comprendre la cause.
   - Envoyer la commande de "Recovery" si le problème est résolu.
3. **Panne Matérielle Majeure:**
   - Si le score de santé tombe à 0%.
   - Isoler l'asset du réseau pour éviter la propagation.
   - Préparer l'image de Firmware via RAUC (Slot B) pour une réparation logicielle.
