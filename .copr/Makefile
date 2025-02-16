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
	spectool -g -R $(spec)
	$(MAKE) -f $(dir $(spec))/Makefile srpm outdir=$(outdir)
