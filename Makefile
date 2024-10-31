ifneq (,$(wildcard ./.env))
	include .env
  export
endif

BUILD_DIR="target/aarch64-unknown-linux-musl/release"
BIN_NAME="jonathansm"

.PHONY: all pi mac test clean deploy

all: pi mac

pi:
	cargo build --release --target=aarch64-unknown-linux-musl

mac:
	cargo build --release

test:
	cargo test

clean:
	cargo clean

deploy:
	rsync -avzP $(BUILD_DIR)/$(BIN_NAME) $(DEPLOY_SERVER):$(DEPLOY_PATH)$(BIN_NAME)
	ssh -t "$(DEPLOY_SERVER)" "sudo systemctl restart website"
