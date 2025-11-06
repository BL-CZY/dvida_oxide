# Nuke built-in rules and variables.
override MAKEFLAGS += -rR

ifeq ($(RUST_TARGET),)
    override RUST_TARGET := x86_64-unknown-none
endif

ifeq ($(RUST_PROFILE),)
    override RUST_PROFILE := release
endif

override RUST_PROFILE_SUBDIR := $(RUST_PROFILE)
ifeq ($(RUST_PROFILE),dev)
    override RUST_PROFILE_SUBDIR := debug
endif

OS := $(shell uname -s | tr A-Z a-z)

CMD := find -maxdepth 1 -perm -111 -type f
ifeq ($(OS),darwin)
	override CMD := find
endif

override CARGO_TARGET_DIR=.

EXPAND_DIR=src/drivers/fs/mod.rs

# Default target.
.PHONY: all
all:
		cargo build --target $(RUST_TARGET) --profile $(RUST_PROFILE)
		cp target/$(RUST_TARGET)/$(RUST_PROFILE_SUBDIR)/kernel kernel

.PHONY: all-test
all-test:
		rm -rf target/$(RUST_TARGET)/$(RUST_PROFILE_SUBDIR)/deps/*kernel*
		cargo build --tests --keep-going --target $(RUST_TARGET) --profile $(RUST_PROFILE)
		cp target/$(RUST_TARGET)/$(RUST_PROFILE_SUBDIR)/deps/$$(cd target/$(RUST_TARGET)/$(RUST_PROFILE_SUBDIR)/deps && find . -maxdepth 1 -type f -regex "./kernel-................") kernel

.PHONY: expand
expand:
	cargo expand --profile $(RUST_PROFILE) --target $(RUST_TARGET)

# Remove object files and the final executable.
.PHONY: clean
clean:
		cargo clean
		rm -rf kernel

.PHONY: distclean
distclean: clean
