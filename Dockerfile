FROM rust:bookworm as build
# shared util files
ADD ./shared_lib/src /app/shared_lib/src
ADD ./shared_lib/Cargo.toml /app/shared_lib/Cargo.toml
# web service files
ADD ./web_service/src /app/web_service/src
ADD ./web_service/assets /app/web_service/assets
ADD ./web_service/templates /app/web_service/templates
ADD ./web_service/Cargo.toml /app/web_service/Cargo.toml
# beacon worker files
# ADD ./beacon_worker/src /app/beacon_worker/src
# ADD ./beacon_worker/Cargo.toml /app/beacon_worker/Cargo.toml
# common files
ADD ./Cargo.lock /app/Cargo.lock
ADD ./Cargo.toml /app/Cargo.toml
WORKDIR /app
RUN cargo build --release

FROM node:bookworm-slim as web_assets
WORKDIR /app
ADD ./web_service/package.json /app/package.json
ADD ./web_service/package-lock.json /app/package-lock.json
ADD ./web_service/assets /app/assets
ADD ./web_service/styles /app/styles
ADD ./web_service/templates /app/templates
RUN npm ci
RUN npm run tailwind:generate

FROM debian:bookworm-slim as runtime
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    dumb-init \
    && \
    rm -rf /var/lib/apt/lists/*

COPY --from=build /app/target/release/web_service /usr/local/bin/web_service
COPY --from=web_assets /app/assets /app/assets

COPY entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

WORKDIR /app
ENTRYPOINT [ "entrypoint.sh" ]
CMD ["web_service"]
