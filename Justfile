default:
    just --list

fmt:
    cargo clippy --fix --bin "reflex"
    cargo fmt --all

lint:
    cargo fmt -- --check
    cargo clippy --all-targets -- -D warnings
    cargo xwin clippy --all-targets -- -D warnings

test:
    cargo nextest run --all
    cargo xwin test run --all --target x86_64-pc-windows-msvc


run *args:
    sudo cargo run --bin "reflex" -- {{args}}

prepare version:
    lash run scripts/release/prepare.lash {{version}}

promote:
    just lint
    just test
    lash run scripts/release/promote.lash

publish version:
    lash run scripts/release/publish.lash {{version}}
    git switch dev
