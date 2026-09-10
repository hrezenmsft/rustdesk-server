Source: rustdesk-server
Section: net
Priority: optional
Maintainer: Henrique Rezende <hrezenmsft@users.noreply.github.com>
Build-Depends: debhelper (>= 10), pkg-config
Standards-Version: 4.5.0
Homepage: https://github.com/hrezenmsft/rustdeskadmin-server

Package: rustdesk-server-hbbs
Architecture: {{ ARCH }}
Depends: systemd ${misc:Depends}
Description: RustDeskAdmin server, a RustDesk fork
 Self-host your own RustDeskAdmin server. This free and open-source fork is based on RustDesk.

Package: rustdesk-server-hbbr
Architecture: {{ ARCH }}
Depends: systemd ${misc:Depends}
Description: RustDeskAdmin server, a RustDesk fork
 Self-host your own RustDeskAdmin server. This free and open-source fork is based on RustDesk.
 This package contains the RustDeskAdmin relay server.

Package: rustdesk-server-utils
Architecture: {{ ARCH }}
Depends: ${misc:Depends}
Description: RustDeskAdmin server, a RustDesk fork
 Self-host your own RustDeskAdmin server. This free and open-source fork is based on RustDesk.
 This package contains the rustdesk-utils binary.
