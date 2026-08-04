SUMMARY = "SpaceOne Coordination — Multi-Agency Protocol + Autonomous Scheduler"
DESCRIPTION = "Coordination multi-agences (NASA, SpaceX, ESA, CNSA) avec \
consensus Byzantine, scheduler autonome avec résolution de conflits et \
plans de contingence. Intégration avec aqm-supervisor pour les états système."
LICENSE = "Apache-2.0"

SRC_URI = "file://src/ \
           file://Cargo.toml \
           file://spaceone-coordination.service"

DEPENDS = "rust-native"

inherit systemd cargo

SYSTEMD_SERVICE:${PN} = "spaceone-coordination.service"
SYSTEMD_AUTO_ENABLE:${PN} = "enable"

S = "${WORKDIR}"

do_install() {
    install -d ${D}${bindir}
    install -m 0755 ${B}/target/release/spaceone-coordination ${D}${bindir}/

    install -d ${D}${systemd_unitdir}/system
    install -m 0644 ${WORKDIR}/spaceone-coordination.service ${D}${systemd_unitdir}/system/
}

RDEPENDS:${PN} = "systemd aqm-supervisor spaceone-core"

FILES:${PN} += "${systemd_unitdir}/system/spaceone-coordination.service"
