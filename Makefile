# Build the compiler, compile example.src → output.s, link with clang, run.
# The compiler is invoked via `cargo run` so the same target dir Cargo uses is always picked up.

SRC      ?= example.src
OUT_ASM  := output.s
OUT_BIN  := example.run

.PHONY: all run build clean

all: run

build:
	cargo build

# Always rebuild asm when the source or compiler sources change; cargo run rechecks deps.
$(OUT_ASM): $(SRC) $(wildcard src/*.rs) Cargo.toml
	cargo run -q -- $(SRC)

run: $(OUT_ASM)
	clang $(OUT_ASM) -o $(OUT_BIN) && ./$(OUT_BIN)

clean:
	rm -f $(OUT_ASM) $(OUT_BIN)
	cargo clean
