default: tailwind-generate-watch watch

tailwind-generate:
    rm assets/main.css
    npx tailwindcss -i ./styles/tailwind.css -o ./assets/main.css
tailwind-generate-watch:
    npx tailwindcss -i ./styles/tailwind.css -o ./assets/main.css --watch

build:
    cargo build

run:
    cargo run

watch:
    cargo watch -x run
