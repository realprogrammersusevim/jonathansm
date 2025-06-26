set dotenv-load

default: linux self

linux:
  cargo build --release --target=x86_64-unknown-linux-gnu

self:
  cargo build --release

alias ar := auto-reload
auto-reload:
  fd -t f -e rs -e html | entr -r cargo run

test:
  cargo test

clean:
  cargo test

deploy:
  rsync -avzP target/x86_64-unknown-linux-gnu/release/jonathansm $DEPLOY_SERVER:$DEPLOY_PATH
  ssh -t "$DEPLOY_SERVER" "sudo systemctl restart jonathansm"
