# cf. https://github.com/casey/just

list:
    just --list

dev:
    cargo run --bin peeps-web -- --dev

example *args:
    ./scripts/run-example {{ args }}

ex *args:
    just example {{ args }}
