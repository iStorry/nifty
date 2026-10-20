# Keeps the local Nifty setup in step with this checkout. Run `make` after pulling or
# switching branches; `make help` lists the targets.

ZSHRC := $(HOME)/.zshrc
CARGO_BIN := $(HOME)/.cargo/bin
WINDOWS_TARGET := x86_64-pc-windows-msvc

.PHONY: all install zsh check help

all: zsh install
	@echo
	@echo "Up to date: $$($(CARGO_BIN)/nf version)"
	@echo "- Open a new terminal or run 'source $(ZSHRC)' so nf uses this checkout."
	@echo "- In Zed, run 'editor: restart language server' to pick up the new nf lsp."

## install: build nf (the CLI and the language server) and nf-runner into ~/.cargo/bin
install:
	cargo install --locked --path crates/nifty-cli

## zsh: point NIFTY_HOME in ~/.zshrc at this checkout
zsh:
	@touch "$(ZSHRC)"
	@if grep -q '^NIFTY_HOME=' "$(ZSHRC)"; then \
		NIFTY_HOME_LINE='NIFTY_HOME="$(CURDIR)"' perl -i -pe 's/^NIFTY_HOME=.*$$/$$ENV{NIFTY_HOME_LINE}/' "$(ZSHRC)"; \
	else \
		printf '\nNIFTY_HOME="%s"\n' "$(CURDIR)" >> "$(ZSHRC)"; \
	fi
	@echo "NIFTY_HOME in $(ZSHRC) is $(CURDIR)"

## check: every check CLAUDE.md asks for before a change is done
check:
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo clippy --workspace --all-targets --target $(WINDOWS_TARGET) -- -D warnings
	cargo test --workspace
	cd editors/tree-sitter-nifty && npm run generate && npm test
	cd editors/vscode && npm test

## help: list the targets
help:
	@printf 'make %-8s %s\n' "" "update ~/.zshrc, then install nf and nf-runner"
	@grep '^## ' $(MAKEFILE_LIST) | sed 's/^## //' | awk -F': ' '{ printf "make %-8s %s\n", $$1, $$2 }'
