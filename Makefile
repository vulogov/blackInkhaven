SOURCES=$(wildcard src/*.rs)


all: $(SOURCES) Makefile
	cargo build

rebuild:
	make clean
	make all
release:
	cargo build --release

test:
	cargo test -- --show-output

clean:
	cargo clean
	cargo update

sweep:
	cargo install cargo-sweep
	cargo sweep --time 3 --recursive

commit:
	aic -ac
	git push
