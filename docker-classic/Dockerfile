FROM scratch
ARG VERSION
ARG VCS_REF
LABEL org.opencontainers.image.title="RustDeskAdmin Server - RustDesk Fork" \
      org.opencontainers.image.description="Rendezvous, relay, and administrator-presence utilities" \
      org.opencontainers.image.authors="Henrique Rezende" \
      org.opencontainers.image.source="https://github.com/hrezenmsft/rustdeskadmin-server" \
      org.opencontainers.image.licenses="AGPL-3.0-only" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.revision="${VCS_REF}"
COPY hbbs /usr/bin/hbbs
COPY hbbr /usr/bin/hbbr
# Admin-presence customization: the CI workflow (.github/workflows/build.yaml,
# docker-classic job) already downloads rustdesk-utils into this build context
# alongside hbbs/hbbr, but it was never copied into the image. Without this,
# `docker run/compose ... rustdesk-utils ...` fails with "no such file" even
# though the binary is right there in the build context.
COPY rustdesk-utils /usr/bin/rustdesk-utils
COPY RELEASE-NOTICE.txt LICENSE /usr/share/doc/rustdeskadmin-server/
WORKDIR /root
ENV HOME=/root
