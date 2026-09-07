FROM scratch
COPY hbbs /usr/bin/hbbs
COPY hbbr /usr/bin/hbbr
# Admin-presence customization: the CI workflow (.github/workflows/build.yaml,
# docker-classic job) already downloads rustdesk-utils into this build context
# alongside hbbs/hbbr, but it was never copied into the image. Without this,
# `docker run/compose ... rustdesk-utils ...` fails with "no such file" even
# though the binary is right there in the build context.
COPY rustdesk-utils /usr/bin/rustdesk-utils
WORKDIR /root
ENV HOME=/root
