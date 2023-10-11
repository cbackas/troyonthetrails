FROM rust:bookworm as build
ADD ./src /app/src
ADD ./templates /app/templates
ADD ./Cargo.lock /app/Cargo.lock
ADD ./Cargo.toml /app/Cargo.toml
WORKDIR /app
RUN cargo build --release

FROM node:bookworm-slim as assets
WORKDIR /app
ADD ./package.json /app/package.json
ADD ./package-lock.json /app/package-lock.json
RUN npm ci
ADD ./styles /app/styles
ADD ./templates /app/templates
RUN npx tailwindcss -i ./styles/tailwind.css -o ./assets/main.css

FROM rust:slim-bookworm as runtime
COPY --from=build /app/target/release/troyonthetrails /usr/local/bin/troyonthetrails
ADD ./assets /app/assets
COPY --from=assets /app/assets/main.css /app/assets/main.css

WORKDIR /app
CMD ["troyonthetrails"]
