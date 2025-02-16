SHELL=/usr/bin/env bash -xeuEo pipefail

.PHONY: srpm
srpm:
ifndef outdir
	$(error outdir is undefined)
endif
ifndef spec
	$(error spec is undefined)
endif
	dnf install -y rpmdevtools
	spectool -g -R $(sepc)
	$(MAKE) -f rpm/Makefile -C rpm srpm
