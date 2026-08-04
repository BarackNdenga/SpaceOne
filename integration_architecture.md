# SpaceOne Phase 3 — Architecture d'intégration avec AsterQuanta OS

## 1. Vue d'ensemble de l'intégration

L'intégration de SpaceOne avec AsterQuanta OS repose sur une architecture de services distribués exécutés sous systemd. SpaceOne n'est pas un système d'exploitation autonome, mais un middleware de coordination de mission qui s'appuie sur les primitives fournies par AsterQuanta (kernel Linux, systemd, RAUC, uD3TN).

## 2. Mapping des couches logicielles

L'intégration s'effectue à trois niveaux distincts de la pile AsterQuanta :

| Couche AsterQuanta | Composant AsterQuanta | Intégration SpaceOne |
|--------------------|----------------------|----------------------|
| **L2 (Bootloader)** | GRUB + RAUC (A/B slots) | SpaceOne flight software est packagé comme un bundle RAUC additionnel `.raucb` |
| **L4 (Services)** | systemd + journald | SpaceOne expose des units systemd (`spaceone-*.service`) |
| **L5 (Userland AQM)** | aqm-supervisor, aqm-dtnd, aqm-recovery | SpaceOne communique via sockets UNIX et D-Bus avec les services AQM |

## 3. Points d'intégration critiques

### 3.1 Boot Sequence (Séquence de démarrage)

La séquence de démarrage est orchestrée par systemd. SpaceOne démarre après que l'environnement système est stable :

```
1. Firmware → GRUB (lit slot A ou B)
2. Noyau Linux (AsterQuanta)
3. systemd (PID 1)
4. multi-user.target
   ├── aqm-supervisor.service (supervision de base)
   ├── aqm-dtnd.service (réseau DTN)
   ├── aqm-recovery.service (snapshots BTRFS)
   └── spaceone-core.service (nouveau, démarre SpaceOne)
       ├── spaceone-hal.service (protection radiations)
       ├── spaceone-communication.service (gestion DTN++)
       └── spaceone-scheduler.service (planification autonome)
```

### 3.2 Communication Inter-Processus (IPC)

SpaceOne utilise le même modèle IPC qu'AsterQuanta pour la cohérence :

| Espace SpaceOne | Espace AsterQuanta | Mécanisme IPC |
|-----------------|-------------------|---------------|
| Health Manager | aqm-supervisor | Socket UNIX (`/run/spaceone/health.sock`) |
| DTN++ Protocol | aqm-dtnd (uD3TN) | Socket AAP (`/run/aqm/aqm-dtnd.aap.sock`) |
| Safe Mode | aqm-recovery | `systemctl isolate aqm-safe.target` |
| Firmware Update | RAUC | Commande système (`rauc install`) |

### 3.3 Gestion des mises à jour (RAUC)

SpaceOne ne remplace pas RAUC, il l'utilise pour sa propre mise à jour :

1. Le Mission Control envoie un bundle `spaceone-flight.raucb`
2. `aqm-dtnd` reçoit le bundle via le réseau delay-tolerant
3. SpaceOne vérifie la signature (Ed25519) et l'intégrité (DM-Verity)
4. SpaceOne appelle `rauc install` pour écrire le bundle sur le slot inactif
5. Au prochain reboot, le nouveau SpaceOne démarre

### 3.4 Récupération de panne (Recovery)

L'intégration avec le système de recovery d'AsterQuanta est bidirectionnelle :

- **De AsterQuanta vers SpaceOne** : Si le kernel panique ou si le bootloader détecte trop de tentatives d'échec, AsterQuanta bascule sur le slot de recovery. SpaceOne perd son état mais redémarre proprement.
- **De SpaceOne vers AsterQuanta** : Si SpaceOne détecte une défaillance matérielle critique (SEL, perte de puissance), il déclenche `aqmctl safe-mode on` pour isoler les composants et préserver les données persistantes sur la partition BTRFS.

## 4. Structure des recettes Yocto (Production)

Pour une production réelle, l'intégration de SpaceOne dans l'image AsterQuanta nécessite la création de nouvelles recettes Yocto dans `meta-asterquanta/recipes-aqm/` :

| Recette Yocto | Contenu | Rôle |
|---------------|---------|------|
| `spaceone-core_1.0.bb` | Binaire `spaceone-core` + service systemd | Point d'entrée principal |
| `spaceone-hal_1.0.bb` | Binaire `radiation_tolerant_hal` + service systemd | Accès matériel sécurisé |
| `spaceone-communication_1.0.bb` | Binaire `dtn_plus` + service systemd | Gestion des bundles prioritaires |
| `spaceone-coordination_1.0.bb` | Binaires `multi_agency_protocol` + `autonomous_scheduler` + service systemd | Coordination et planification |

Ces recettes seront ajoutées au `IMAGE_INSTALL` de `asterquanta-image.bb` pour être compilées et incluses dans l'image disque finale `.wic`.

## 5. Stratégie de déploiement en Phase 3

La Phase 3 de production sur Mars suit ces étapes :

1. **Build de l'image complète** : Yocto compile l'image `.wic` contenant le kernel, systemd, RAUC, uD3TN et tous les modules SpaceOne.
2. **Flash et Transit** : L'image est flashée sur les plateformes (rover, habitat, orbiteur) avant le lancement.
3. **Boot initial** : Au contact avec Mars, les plateformes démarrent sur le slot A.
4. **Opérations nominales** : SpaceOne coordonne les opérations pendant que AsterQuanta gère le système de base.
5. **Maintenance** : Les mises à jour sont envoyées via DTN et appliquées par RAUC sur le slot B (rollforward).
