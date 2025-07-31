# Makefile for Deloxide C Tests with Static Linking

RUST_PROFILE = release
RUST_TARGET  = target/$(RUST_PROFILE)
DEL_LIB      = $(RUST_TARGET)/libdeloxide.a

CFLAGS  = -Iinclude -pthread
LDFLAGS = -L$(RUST_TARGET) -ldeloxide -pthread

.PHONY: all rustlib c_tests test clean

all: rustlib c_tests

rustlib:
	cargo build --profile $(RUST_PROFILE)

c_tests: \
	bin/dining_philosophers_deadlock \
	bin/two_thread_deadlock \
	bin/random_ring_deadlock \
	bin/rwlock_multiple_readers_no_deadlock \
	bin/rwlock_upgrade_deadlock \
	bin/rwlock_writer_waits_for_readers_no_deadlock \
	bin/three_thread_rwlock_deadlock

bin/%: c_tests/%.c include/deloxide.h $(DEL_LIB)
	mkdir -p bin
	gcc $(CFLAGS) -o $@ $< $(LDFLAGS)

test: all
	@echo "\n--- Running C deadlock tests ---"
	- bin/dining_philosophers_deadlock              || exit 1
	- bin/two_thread_deadlock                       || exit 1
	- bin/random_ring_deadlock                      || exit 1
	- bin/rwlock_multiple_readers_no_deadlock       || exit 1
	- bin/rwlock_upgrade_deadlock                   || exit 1
	- bin/rwlock_writer_waits_for_readers_no_deadlock || exit 1
	- bin/three_thread_rwlock_deadlock              || exit 1
	@echo "\nAll C tests passed!"

clean:
	rm -rf bin
	cargo clean
