.PHONY: all build release test install clean check-linux

all: release

build:
	cargo build

release:
	cargo build --release

test:
	cargo test

install:
	./install.sh

clean:
	cargo clean
