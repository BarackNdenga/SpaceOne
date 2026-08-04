SUMMARY = "SpaceOne DTN++ Communication Layer"
DESCRIPTION = "Couche de communication DTN++ avec compression IA, \
store-and-forward, bundles prioritaires. S'appuie sur aqm-dtnd (uD3TN, \
RFC 9171) d'AsterQuanta pour le transport bas niveau."
LICENSE = "Apache-2.0"

SRC_URI = "file://src/ \
           file://Cargo.toml \
           file://spaceone-communication.service"

DEPENDS = "rust-native"

inherit systemd cargo

SYSTEMD_SERVICE:${PN} = "spaceone-communication.service"
SYSTEMD_AUTO_ENABLE:${PN} = "enable"

S = "${WORKDIR}"

do_install() {
    install -d ${D}${bindir}
    install -m 0755 ${B}/target/release/spaceone-communication ${D}${bindir}/

    install -d ${D}${systemd_unitdir}/system
    install -m 0644 ${WORKDIR}/spaceone-communication.service ${D}${systemd_unitdir}/system/
}

RDEPENDS:${PN} = "systemd aqm-dtnd"

FILES:${PN} += "${systemd_unitdir}/system/spaceone-communication.service"
