.PHONY: run deploy

ARGS := $(filter-out run,$(MAKECMDGOALS))
ifeq ($(strip $(ARGS)),)
ARGS := --help
endif

BIN_DIR ?= $(HOME)/.local/bin
BIN := ovmd

run:
	cargo run -- $(ARGS)

deploy:
	cargo build --release
	install -d "$(BIN_DIR)"
	install "target/release/$(BIN)" "$(BIN_DIR)/$(BIN)"
	"$(BIN_DIR)/$(BIN)" --version

%:
	@:
