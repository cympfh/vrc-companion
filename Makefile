.PHONY: build test clean fmt lint

WINDOWS_TARGET := x86_64-pc-windows-gnu

# WSL の GPU パススルー無しの環境では GUI が正常に動かないため、
# Windows ネイティブ実行ファイルをビルドして Windows 側で実行する
build:
	rustup target add $(WINDOWS_TARGET)
	cargo build --release --target $(WINDOWS_TARGET)
	@echo "生成物: target/$(WINDOWS_TARGET)/release/vrc-companion.exe"

run: build
	./target/$(WINDOWS_TARGET)/release/vrc-companion.exe

test:
	cargo test

clean:
	cargo clean

fmt:
	cargo fmt

lint:
	cargo clippy --all-targets -- -D warnings
