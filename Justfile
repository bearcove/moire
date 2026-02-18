# cf. https://github.com/casey/just

list:
    just --list

dev:
    cargo run --bin peeps-web -- --dev

example *args:
    cargo run --bin peeps-examples -- {{ args }}

ex *args:
    cargo run --bin peeps-examples -- {{ args }}
