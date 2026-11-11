# The Linux machine that `bun test.ts` runs VS Code in, on a virtual display, when the host isn't Linux.
FROM rust:1.98-bookworm

RUN apt-get update && apt-get install -y --no-install-recommends \
  xvfb xauth unzip libgtk-3-0 libnss3 libasound2 libgbm1 libxss1 libsecret-1-0 libxkbfile1 \
  && rm -rf /var/lib/apt/lists/*

RUN curl -fsSL https://bun.sh/install | bash -s bun-v1.4.4
ENV PATH=/root/.bun/bin:$PATH
