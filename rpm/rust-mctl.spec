%bcond check 0

# prevent library files from being installed
%global cargo_install_lib 0

%global crate mctl

Name:           rust-mctl
Version:        0.1.0
Release:        %autorelease
Summary:        Cli to manage machines and configurations

License:        MIT OR APACHE-2.0
URL:            https://github.com/joshuachp/mctl
Source:         %{crates_source}
Source:         %{name}-%{version}-vendor.tar.xz

BuildRequires:  cargo-rpm-macros >= 26

%global _description %{expand:
Cli to manage machines and configurations}

%description %{_description}

%package     -n %{crate}
Summary:        %{summary}
License:        MIT OR APACHE-2.0
# LICENSE.dependencies contains a full license breakdown

%description -n %{crate} %{_description}

%files       -n %{crate}
%license LICENSE-MIT
%license LICENSE-APACHE-2.0
%license LICENSE.dependencies
%license cargo-vendor.txt
%{_bindir}/mctl
%{_datadir}/fish/vendor_completions.d/mctl.fish
%{_mandir}/man1/mctl*

%prep
%autosetup -n %{crate}-%{version} -p1 -a1
# fix shebangs in vendor
%cargo_prep -v vendor
    find ./vendor -type f -executable -name '*.rs' -exec chmod -x '{}' \;


%build
%cargo_build
%{cargo_license_summary}
%{cargo_license} > LICENSE.dependencies
%{cargo_vendor_manifest}


%install
%cargo_install
mkdir -p "$RPM_BUILD_ROOT%{_datadir}/fish/vendor_completions.d"
"$RPM_BUILD_ROOT%{_bindir}/mctl" utils completion fish > "$RPM_BUILD_ROOT%{_datadir}/fish/vendor_completions.d/mctl.fish"
mkdir -p "$RPM_BUILD_ROOT%{_mandir}/man1"
"$RPM_BUILD_ROOT%{_bindir}/mctl" utils manpages "$RPM_BUILD_ROOT%{_mandir}/man1"


%if %{with check}
%check
%cargo_test
%endif

%files
%license
%doc


%changelog
%autochangelog

