run: githooks
	SKIP_WASM_BUILD= cargo run -- --dev --execution=native -lruntime=debug

toolchain:
	./scripts/init.sh

build-wasm: githooks
	WASM_BUILD_TYPE=release cargo build

check: githooks
	SKIP_WASM_BUILD= cargo check

check-debug:
	RUSTFLAGS="-Z external-macro-backtrace" BUILD_DUMMY_WASM_BINARY= cargo +nightly check

check-dummy:
	BUILD_DUMMY_WASM_BINARY= cargo check

build: githooks
	SKIP_WASM_BUILD= cargo build

purge: target/debug/flowchain
	target/debug/flowchain purge-chain --dev -y

restart: purge run

target/debug/flowchain: build

GITHOOKS_SRC = $(wildcard githooks/*)
GITHOOKS_DEST = $(patsubst githooks/%, .git/hooks/%, $(GITHOOKS_SRC))

.git/hooks:
	mkdir .git/hooks

.git/hooks/%: githooks/%
	cp $^ $@

githooks: .git/hooks $(GITHOOKS_DEST)

init: toolchain submodule build-wasm

submodule:
	git submodule update --init --recursive

update-orml:
	cd orml && git checkout master && git pull
	git add orml

update: update-orml
	cargo update
	make check
