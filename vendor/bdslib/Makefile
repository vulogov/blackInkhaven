SOURCES=$(wildcard src/*.rs)


all: $(SOURCES) Makefile
	cargo build

rebuild:
	make clean
	make all

test:
	cargo test -- --show-output

clean:
	cargo clean
	cargo update

commit:
	aic -ac
	git push
