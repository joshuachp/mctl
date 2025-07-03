%global crate mctl
# prevent library files from being installed
%global cargo_install_lib 0

Name:           rust-mctl
# x-release-please-start-version
Version:        0.2.1
# x-release-please-end-version
Release:        %autorelease
Summary:        Cli to manage machines and configurations

License:        MIT OR APACHE-2.0
URL:            https://github.com/joshuachp/mctl
Source:         %{url}/releases/download/v%{version}/%{crate}-%{version}.crate
Source:         %{url}/releases/download/v%{version}/%{name}-%{version}-vendor.tar.xz

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
%{bash_completions_dir}/mctl.bash
%{fish_completions_dir}/mctl.fish
%{zsh_completions_dir}/_mctl
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
'%{buildroot}%{_bindir}/mctl' utils completion bash > mctl.bash
'%{buildroot}%{_bindir}/mctl' utils completion fish > mctl.fish
'%{buildroot}%{_bindir}/mctl' utils completion zsh > _mctl
install -Dpm 0644 mctl.bash -t %{buildroot}%{bash_completions_dir}
install -Dpm 0644 mctl.fish -t %{buildroot}%{fish_completions_dir}
install -Dpm 0644 _mctl -t %{buildroot}%{zsh_completions_dir}
mkdir -pm 0755 '%{buildroot}%{_mandir}/man1'
'%{buildroot}%{_bindir}/mctl' utils manpages "%{buildroot}%{_mandir}/man1"

%check
%cargo_test

%files
%license
%doc

%changelog
%autochangelog

