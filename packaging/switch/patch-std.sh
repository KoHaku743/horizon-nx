#!/usr/bin/env bash
# Patches the toolchain's std source before -Z build-std (packaging/switch/build.sh runs it in the
# container): std::env::current_exe on the Switch is the homebrew loader's argv[0], the NRO's path
# ("sdmc:/switch/horizon/horizon.nro"), where std has "unsupported". The game names its own file in
# many places (mods, the launch arguments it hands its mod runtime). Idempotent.
set -euo pipefail
STD_LIB="$1"
P="$STD_LIB/std/src/sys/paths/unix.rs"
if grep -q 'horizon-nx: current_exe' "$P"; then
    exit 0
fi
OLD='#[cfg(any(target_os = "espidf", target_os = "horizon", target_os = "vita"))]'
if ! grep -qF "$OLD" "$P"; then
    echo "error: $P no longer has std's unsupported current_exe for horizon: update patch-std.sh" >&2
    exit 1
fi
sed -i 's/#\[cfg(any(target_os = "espidf", target_os = "horizon", target_os = "vita"))\]/#[cfg(any(target_os = "espidf", target_os = "vita"))]/' "$P"
cat >> "$P" <<'RS'

// horizon-nx: current_exe is argv[0], which the homebrew loader sets to the NRO's path.
#[cfg(target_os = "horizon")]
pub fn current_exe() -> io::Result<PathBuf> {
    crate::env::args_os()
        .next()
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| io::const_error!(io::ErrorKind::NotFound, "the loader passed no argv[0]"))
}
RS
echo "patch-std: current_exe on horizon is argv[0]"
