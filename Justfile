# cf. https://github.com/casey/just

list:
    just --list

dev:
    cargo run --bin moire-web -- --dev

example *args:
    cargo run --bin moire-examples -- {{ args }}

ex *args:
    just kill-port # fuck you too, vite
    cargo run --bin moire-examples -- {{ args }}

kill-port port="9132":
    lsof -ti:{{ port }} -sTCP:LISTEN | xargs kill -9
