.PHONY: srpm

srpm:
ifndef $(outputdir)
	$(error outputdir is undefined)
endif
ifndef $(spec)
	$(error spec is undefined)
endif
	dnf install -y rpmdevtools
	spectools -g -R $(sepc)
	$(MAKE) -f rpm/MAKEFILE -C rpm srpm
