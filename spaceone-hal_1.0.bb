SUMMARY = "SpaceOne Radiation-Tolerant HAL"
DESCRIPTION = "Hardware Abstraction Layer avec protection anti-radiations : \
TMR, ECC memory controller, watchdog hardware, power gating, SEL protection. \
Interagit directement avec les drivers kernel Linux d'AsterQuanta."
LICENSE = "Apache-2.0"

SRC_URI = "file://src/ \
           file://Cargo.toml \
           file://spaceone-hal.service"

DEPENDS = "rust-native"

inherit systemd cargo

SYSTEMD_SERVICE:${PN} = "spaceone-hal.service"
SYSTEMD_AUTO_ENABLE:${PN} = "enable"

S = "${WORKDIR}"

do_install() {
    install -d ${D}${bindir}
    install -m 0755 ${B}/target/release/spaceone-hal ${D}${bindir}/

    install -d ${D}${systemd_unitdir}/system
    install -m 0644 ${WORKDIR}/spaceone-hal.service ${D}${systemd_unitdir}/system/
}

RDEPENDS:${PN} = "systemd"

FILES:${PN} += "${systemd_unitdir}/system/spaceone-hal.service"
