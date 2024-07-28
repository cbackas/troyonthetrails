FROM rust:bookworm as build
# web service files
ADD ./web_service/src /app/web_service/src
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
ADD ./web_service/tailwind.config.cjs /app/tailwind.config.cjs
ADD ./web_service/assets /app/assets
ADD ./web_service/styles /app/styles
ADD ./web_service/templates /app/templates
RUN npm ci
RUN npx tailwindcss -i ./styles/tailwind.css -o ./assets/main.css

FROM rust:slim-bookworm as runtime
COPY --from=build /app/target/release/web_service /usr/local/bin/web_service
COPY --from=web_assets /app/assets /app/assets

WORKDIR /app
CMD ["web_service"]
