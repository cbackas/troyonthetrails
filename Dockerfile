FROM rust:bookworm as build
ADD ./src /app/src
ADD ./templates /app/templates
ADD ./Cargo.lock /app/Cargo.lock
ADD ./Cargo.toml /app/Cargo.toml
WORKDIR /app
RUN cargo build --release

FROM rust:slim-bookworm as runtime
COPY --from=build /app/target/release/troyonthetrails /usr/local/bin/troyonthetrails
ADD /assets/ /app/assets/

WORKDIR /app
CMD ["troyonthetrails"]
