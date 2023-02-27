all: clean build_rust build_vscode

clean:
	- rm -r target
	- rm -r vscode-brainfuck/server/*
	- rm -r vscode-brainfuck/brainfuck-all-in-one.vsix

setup_env:
	rustup target add x86_64-pc-windows-gnu
	rustup target add x86_64-unknown-linux-musl
	apt-get install gcc gcc-mingw-w64 musl-tools

build_rust:
	cargo build --release --target=x86_64-pc-windows-gnu
	cargo build --release --target=x86_64-unknown-linux-musl

build_vscode:
	cd vscode-brainfuck && npm run package