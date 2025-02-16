SHELL = /usr/bin/env bash -xeuEo pipefail

ARCH := $(shell uname -m)

TOPDIR := $(shell rpmspec -E '%{_topdir}')
# overwritable
SOURCE_DIR ?= $(TOPDIR)/SOURCES

SPECFILE := rust-mctl.spec

NAME := mctl
VERSION := $(shell rpmspec --query $(SPECFILE) --queryformat '%{version}\n' | head -n1)
RELEASE := $(shell rpmspec --query $(SPECFILE) --queryformat '%{release}\n' | head -n1)

TMPDIR := $(shell mktemp -d)

CARGO_TARGET_DIR ?= $(shell cargo metadata --format-version 1 --no-deps --locked | jq .target_directory --raw-output)

SRCS := ../Cargo.toml ../Cargo.lock
SRCS += $(wildcard ../**/*.rs)

# Overwritten in sources
CRATE := $(SOURCE_DIR)/$(NAME)-$(VERSION).crate
VENDOR := $(SOURCE_DIR)/rust-$(NAME)-$(VERSION)-vendor.tar.xz

SRPM := $(TOPDIR)/SRPMS/rust-$(NAME)-$(VERSION)-$(RELEASE).src.rpm
RPMS := $(TOPDIR)/RPMS/$(ARCH)/rust-$(NAME)-$(VERSION)-$(RELEASE).$(ARCH).rpm \
	$(TOPDIR)/RPMS/$(ARCH)/$(NAME)-$(VERSION)-$(RELEASE).$(ARCH).rpm \
	$(TOPDIR)/RPMS/$(ARCH)/rust-$(NAME)-debugsource-$(VERSION)-$(RELEASE).$(ARCH).rpm \
	$(TOPDIR)/RPMS/$(ARCH)/$(NAME)-debuginfo-$(VERSION)-$(RELEASE).$(ARCH).rpm

.PHONY: build rpm srpm sources

build: $(RPMS)

srpm: $(SRPM)
ifdef $(outputdir)
	cp -v $(SRPM) $(outputdir)
endif

sources: $(CRATE) $(VENDOR)

$(RPMS): $(SRPM)
	rpmbuild -bb $(SPECFILE)

$(SRPM): $(SPECFILE) $(CRATE) $(VENDOR)
	rpmbuild -bs $(SPECFILE)

$(CRATE): $(SRCS)
	cargo package --locked
	cp -v $$CARGO_TARGET_DIR/package/mctl-0.1.0.crate $(CRATE)

$(VENDOR): ../Cargo.toml ../Cargo.lock
	cargo vendor --locked $(TMPDIR)/vendor
	tar -C $(TMPDIR) -cf $(VENDOR) vendor/

.PHONY: clean
clean:
	rm -v $(CRATE) $(VENDOR) $(SRPM) $(RPMS)

