FROM node:bookworm-slim as map_assets
WORKDIR /app
ADD ./map_service/package*.json /app/
RUN npm ci
ADD ./map_service/src/script.js /app/src/script.js
ADD ./map_service/index.html /app/index.html
ADD ./map_service/vite.config.js /app/vite.config.js
RUN npm run build

FROM rust:bookworm as build
# shared util files
ADD ./shared_lib/src /app/shared_lib/src
ADD ./shared_lib/Cargo.toml /app/shared_lib/Cargo.toml
# web service files
ADD ./web_service/src /app/web_service/src
ADD ./web_service/templates /app/web_service/templates
ADD ./web_service/Cargo.toml /app/web_service/Cargo.toml
# map service files
ADD ./map_service/src /app/map_service/src
ADD ./map_service/Cargo.toml /app/map_service/Cargo.toml
COPY --from=map_assets /app/dist /app/map_service/dist/
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
ADD ./web_service/tailwind.config.cjs /app/tailwind.config.cjs
ADD ./web_service/assets /app/assets
ADD ./web_service/styles /app/styles
ADD ./web_service/templates /app/templates
RUN npm ci
RUN npx tailwindcss -i ./styles/tailwind.css -o ./assets/main.css

FROM rust:slim-bookworm as runtime
RUN apt-get update && apt-get install -y chromium-driver chromium dumb-init

COPY --from=build /app/target/release/web_service /usr/local/bin/web_service
COPY --from=build /app/target/release/map_service /usr/local/bin/map_service
COPY --from=web_assets /app/assets /app/assets

WORKDIR /app
ENTRYPOINT [ "dumb-init", "--" ]
CMD ["web_service"]
