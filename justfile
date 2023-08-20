default: tailwind-generate-watch watch

tailwind-generate:
    npx tailwindcss -i ./styles/tailwind.css -o ./assets/main.css
tailwind-generate-watch:
    npx tailwindcss -i ./styles/tailwind.css -o ./assets/main.css --watch

build:
    cargo build

run:
    cargo run

watch:
    cargo watch -s 'just tailwind-generate && just run'
