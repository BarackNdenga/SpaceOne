# SpaceOne

> Infrastructure logicielle embarquée et distribuée pour les opérations martiennes en temps réel.

**SpaceOne** est un système de production certifié pour mission critique (safety-critical, radiation-tolerant) qui coordonne les opérations Mars en mode Earth-independent. Il fonctionne en temps réel sur les rovers, habitats et orbiteurs martiens.

## Architecture

```
SpaceOne = AsterQuanta OS + Flight Software + Ground Tools
```

| Composant | Langage | Rôle |
|-----------|---------|------|
| AsterQuanta OS | Rust | Système d'exploitation temps réel radiation-tolerant |
| Flight Software | Rust | Logiciel embarqué (vole dans l'espace) |
| Mission Control | Rust | Supervision depuis la Terre |
| Data Processor | Python | Analyse données science |
| Deployment Tools | Shell/Rust | Build d'image signée RAUC |

## Flux de Production

### Phase 1 : Développement (Terre)
Le flight software est compilé en image RAUC signée. Les tests hardware-in-loop valident le comportement avec le matériel réel. Les certifications safety-critical et security audit sont produites.

### Phase 2 : Déploiement (Terre → Mars)
L'image signée est intégrée au hardware et embarquée sur les véhicules (rover, orbiteur, habitat). Le transit Terre-Mars dure 6-9 mois.

### Phase 3 : Opérations (Mars)
AsterQuanta OS boote → SpaceOne flight software démarre. Coordination multi-agences, communication DTN++, health management auto-recovery.

### Phase 4 : Mission Control (Terre)
Supervision asynchrone via bundles DTN. Analyse des données science. Commandes envoyées sous forme de bundles.

## Modules Core

### Coordination
- **Multi-Agency Protocol (MAP)** : Protocole de coordination entre agences spatiales
- **Autonomous Scheduler** : Planification autonome avec IA embarquée (radiation-tolerant)
- **Distributed PNT** : Positioning, Navigation, Timing distribué sans GPS

### Communication
- **DTN++** : Delay-Tolerant Networking avancé avec compression IA
- **Mesh Network** : Réseau maillé rover-habitat-orbiteur

### Health Management
- Diagnostics distribués, auto-recovery, safe mode, support décisionnel équipage

## Intégrations Agences

| Agence | Plateforme | Statut |
|--------|-----------|--------|
| NASA/JPL | Perseverance, Artemis | Interface spec définie |
| SpaceX | Starship, Dragon | Interface spec définie |
| ESA | ExoMars, Rosalind Franklin | Interface spec définie |
| CNSA | Tianwen-3 | Interface spec définie |

## Déploiements Cibles

| Mission | Date | Plateforme |
|---------|------|------------|
| Mars Orbiter | 2028 | Orbiteur de relais |
| Mars Rover | 2030 | Rover tout-terrain |
| Mars Habitat | 2032 | Base habitée |

## Certification

SpaceOne est conçu pour les standards :
- **DO-178C** DAL-A (Software)
- **DO-254** (Hardware)
- **ECSS-E-ST-40C** (Space Software)
- **NIST SP 800-161** (Cybersécurité supply chain)
- Tolérance aux radiations : TMR, ECC, scrubbing mémoire

## Licence

Apache-2.0. Voir [LICENSE](LICENSE) pour les détails.

## Contributeurs

Voir [CONTRIBUTING.md](CONTRIBUTING.md).
