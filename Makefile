ifneq (,$(wildcard ./.env))
	include .env
  export
endif

BUILD_DIR="target/x86_64-unknown-linux-gnu/release"
BIN_NAME="jonathansm"

.PHONY: all pi mac test clean deploy

all: pi mac

pi:
	cargo build --release --target=x86_64-unknown-linux-gnu

mac:
	cargo build --release

test:
	cargo test

clean:
	cargo clean

deploy:
	rsync -avzP $(BUILD_DIR)/$(BIN_NAME) $(DEPLOY_SERVER):$(DEPLOY_PATH)$(BIN_NAME)
	ssh -t "$(DEPLOY_SERVER)" "sudo systemctl restart $(BIN_NAME)"
