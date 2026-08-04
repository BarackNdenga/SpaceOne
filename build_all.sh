#!/bin/bash
# ============================================================================
# SpaceOne — Build Complet du Système de Production
# Compile tout : flight software, mission control, image Yocto
# ============================================================================

set -euo pipefail

# Couleurs
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET="${1:-native}"  # native | arm64 | riscv
BUILD_TYPE="${2:-release}"

log()  { echo -e "${CYAN}[BUILD]${NC} $*"; }
success() { echo -e "${GREEN}[OK]${NC} $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*"; }

echo "============================================================"
echo "  SpaceOne — Build Complet"
echo "  Target: $TARGET"
echo "  Build type: $BUILD_TYPE"
echo "  Project: $PROJECT_ROOT"
echo "============================================================"

# ─── Étape 1: Vérification des dépendances ───

log "Vérification des dépendances..."

command -v cargo >/dev/null 2>&1 || { error "Rust/Cargo non installé"; exit 1; }
command -v gcc >/dev/null 2>&1 || { error "GCC non installé"; exit 1; }

if [ "$TARGET" = "arm64" ]; then
    command -v aarch64-linux-gnu-gcc >/dev/null 2>&1 || {
        error "Cross-compiler ARM64 non installé"
        echo "  sudo apt install gcc-aarch64-linux-gnu"
        exit 1
    }
fi

success "Dépendances OK"

# ─── Étape 2: Build Flight Software (Rust) ───

log "Build Flight Software (Cargo workspace)..."

cd "$PROJECT_ROOT"

if [ "$TARGET" = "native" ]; then
    cargo build --$BUILD_TYPE 2>&1 | tee "$PROJECT_ROOT/build_flight_software.log"
    success "Flight Software buildé (native/$BUILD_TYPE)"
elif [ "$TARGET" = "arm64" ]; then
    # Ajouter le target ARM64
    rustup target add aarch64-unknown-linux-gnu 2>/dev/null || true
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc

    cargo build --$BUILD_TYPE --target aarch64-unknown-linux-gnu \
        2>&1 | tee "$PROJECT_ROOT/build_flight_software_arm64.log"
    success "Flight Software buildé (ARM64/$BUILD_TYPE)"
elif [ "$TARGET" = "riscv" ]; then
    # Ajouter le target RISC-V
    rustup target add riscv64gc-unknown-linux-gnu 2>/dev/null || true
    export CARGO_TARGET_RISCV64GC_UNKNOWN_LINUX_GNU_LINKER=riscv64-linux-gnu-gcc

    cargo build --$BUILD_TYPE --target riscv64gc-unknown-linux-gnu \
        2>&1 | tee "$PROJECT_ROOT/build_flight_software_riscv.log"
    success "Flight Software buildé (RISC-V/$BUILD_TYPE)"
fi

# ─── Étape 3: Build Mission Control ───

log "Build Mission Control Server..."

cd "$PROJECT_ROOT/tools/mission_control/server"
cargo build --$BUILD_TYPE 2>&1 | tee "$PROJECT_ROOT/build_mission_control.log"
success "Mission Control Server buildé"

log "Build Mission Control Client (TUI)..."

cd "$PROJECT_ROOT/tools/mission_control/client"
cargo build --$BUILD_TYPE 2>&1 | tee "$PROJECT_ROOT/build_mc_client.log"
success "Mission Control Client buildé"

# ─── Étape 4: Build Yocto Image (si pas en cross-compilation) ───

if [ "$TARGET" = "native" ]; then
    log "Build Yocto Image (asterquanta-image.wic)..."

    cd "$PROJECT_ROOT"

    # Vérifier que kas est disponible
    if command -v kas >/dev/null 2>&1; then
        kas build kas-asterquanta.yml 2>&1 | tee "$PROJECT_ROOT/build_yocto.log"
        success "Image Yocto buildée"
    else
        error "kas non trouvé — installation requise pour le build Yocto"
        echo "  pip3 install kas"
        echo "  Ou build via le pipeline CI/CD"
    fi
fi

# ─── Étape 5: Tests unitaires ───

log "Exécution des tests unitaires..."

cd "$PROJECT_ROOT"
cargo test --$BUILD_TYPE 2>&1 | tee "$PROJECT_ROOT/test_results.log"
success "Tests unitaires terminés"

# ─── Étape 6: Packaging ───

log "Packaging des artefacts..."

ARTIFACTS_DIR="$PROJECT_ROOT/build/artifacts"
mkdir -p "$ARTIFACTS_DIR"

if [ "$TARGET" = "native" ]; then
    BUILD_DIR="$PROJECT_ROOT/target/$BUILD_TYPE"
else
    BUILD_DIR="$PROJECT_ROOT/target/$(rustup show | grep 'Default host' | awk '{print $3}')-$BUILD_TYPE"
fi

# Copier les binaires
cp -f "$BUILD_DIR/spaceone-core" "$ARTIFACTS_DIR/" 2>/dev/null || true
cp -f "$BUILD_DIR/spaceone-hal" "$ARTIFACTS_DIR/" 2>/dev/null || true
cp -f "$BUILD_DIR/spaceone-communication" "$ARTIFACTS_DIR/" 2>/dev/null || true
cp -f "$BUILD_DIR/spaceone-coordination" "$ARTIFACTS_DIR/" 2>/dev/null || true
cp -f "$BUILD_DIR/spaceone-health" "$ARTIFACTS_DIR/" 2>/dev/null || true
cp -f "$BUILD_DIR/spaceone-mission-control" "$ARTIFACTS_DIR/" 2>/dev/null || true
cp -f "$BUILD_DIR/spaceone-mission-control-client" "$ARTIFACTS_DIR/" 2>/dev/null || true

# Calculer les hashes SHA3-256
cd "$ARTIFACTS_DIR"
for f in *; do
    if [ -f "$f" ]; then
        sha256sum "$f" >> "$ARTIFACTS_DIR/checksums.sha256"
    fi
done

success "Artefacts packagés dans $ARTIFACTS_DIR"

# ─── Résumé ───

echo ""
echo "============================================================"
echo "  BUILD TERMINÉ"
echo "  Target: $TARGET | Type: $BUILD_TYPE"
echo "  Artefacts: $ARTIFACTS_DIR"
echo "  Logs: $PROJECT_ROOT/build_*.log"
echo "============================================================"
