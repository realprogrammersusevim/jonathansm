set dotenv-load

default: linux self

linux:
  cargo build --release --target=x86_64-unknown-linux-gnu

self:
  cargo build --release

alias ar := auto-reload
auto-reload:
  fd -t f -e rs -e html | entr -r cargo run

lint:
  cargo clippy --fix --bin "jonathansm" --allow-dirty -- -D clippy::correctness -W clippy::suspicious -W clippy::complexity -D clippy::perf -W clippy::style -W clippy::pedantic

test:
  cargo test

clean:
  cargo test

deploy:
  rsync -avzP target/x86_64-unknown-linux-gnu/release/jonathansm $DEPLOY_SERVER:$DEPLOY_PATH
  ssh -t "$DEPLOY_SERVER" "sudo systemctl restart jonathansm"
