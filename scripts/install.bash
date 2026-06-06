#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
release_dir="$repo_root/target/release"
service_name="reflexd.service"
service_src="$repo_root/crates/reflexd/$service_name"
bin_dir="/usr/local/bin"
systemd_dir="/etc/systemd/system"

require_command() {
    local command_name="$1"
    if ! command -v "$command_name" >/dev/null 2>&1; then
        echo "install.bash: missing required command: $command_name" >&2
        exit 1
    fi
}

sudo_cmd=()
if [[ "${EUID}" -ne 0 ]]; then
    require_command sudo
    sudo_cmd=(sudo)
fi

require_command cargo
require_command install
require_command systemctl

if [[ ! -f "$service_src" ]]; then
    echo "install.bash: missing service file: $service_src" >&2
    exit 1
fi

cd "$repo_root"

echo "install.bash: building release binaries"
cargo build --release --bin reflex --bin reflexd

echo "install.bash: installing reflex and reflexd to $bin_dir"
"${sudo_cmd[@]}" install -Dm755 "$release_dir/reflex" "$bin_dir/reflex"
"${sudo_cmd[@]}" install -Dm755 "$release_dir/reflexd" "$bin_dir/reflexd"

echo "install.bash: installing $service_name to $systemd_dir"
"${sudo_cmd[@]}" install -Dm644 "$service_src" "$systemd_dir/$service_name"

echo "install.bash: enabling and restarting $service_name"
"${sudo_cmd[@]}" systemctl daemon-reload
"${sudo_cmd[@]}" systemctl enable "$service_name"
"${sudo_cmd[@]}" systemctl restart "$service_name"

echo "install.bash: installed reflex at $bin_dir/reflex"
echo "install.bash: installed reflexd at $bin_dir/reflexd"
echo "install.bash: $service_name is active: $("${sudo_cmd[@]}" systemctl is-active "$service_name")"
