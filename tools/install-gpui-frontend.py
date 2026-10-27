#!/usr/bin/env python3
"""Install the prebuilt GPUI front-end next to the Aegisub host binary.

The GPUI workspace is a separate Rust crate with its own toolchain and GUI
stack, so Meson cannot build it directly. Builders produce the executable
first; this script copies it into the same install directory as the host
binary so the portable zip, macOS .app bundle and Linux packages all carry it.
"""

import os
import pathlib
import platform
import shutil
import sys

if len(sys.argv) not in (3, 4):
    raise SystemExit("usage: install-gpui-frontend.py <source-root> <bindir> [absolute-bindir]")

source_root = pathlib.Path(sys.argv[1]).resolve()
# Empty for the macOS bundle, where bindir sits directly at the install prefix.
bindir = pathlib.Path(sys.argv[2]) if sys.argv[2] else pathlib.Path()
# The bundle maps bindir to an absolute path inside Contents, outside prefix.
absolute_bindir = pathlib.Path(sys.argv[3]) if len(sys.argv) == 4 and sys.argv[3] else None
# Set by Meson for install scripts; includes DESTDIR and the install prefix.
destdir_prefix = pathlib.Path(os.environ.get("MESON_INSTALL_DESTDIR_PREFIX", os.environ.get("DESTDIR", "")))

exe_name = "aegisub-gpui-prototype"
# Prefer the target matching the host architecture: a tree can hold both the
# native and the Intel cross build after running the release matrices locally.
machine = platform.machine().lower()
target_triples = {
    "arm64": ["aarch64-apple-darwin", "x86_64-apple-darwin"],
    "aarch64": ["aarch64-apple-darwin", "x86_64-apple-darwin"],
    "x86_64": ["x86_64-apple-darwin", "aarch64-apple-darwin"],
    "amd64": ["x86_64-apple-darwin", "aarch64-apple-darwin"],
    "arm64e": ["aarch64-apple-darwin", "x86_64-apple-darwin"],
}.get(machine, [])
target_dirs = [source_root / "gpui-prototype/target/release"]
target_dirs += [source_root / "gpui-prototype/target" / triple / "release" for triple in target_triples]
binary = None
for target_dir in target_dirs:
    for name in (exe_name, exe_name + ".exe"):
        candidate = target_dir / name
        if candidate.is_file():
            binary = candidate
            break
    if binary is not None:
        break

if binary is None:
    # A plain Meson build from an upstream checkout has no Rust toolchain; the
    # host then reports that the GPUI workspace cannot be launched.
    print("GPUI front-end not built; installing the host only")
    raise SystemExit(0)

destination_dir = absolute_bindir if absolute_bindir is not None else destdir_prefix / bindir
destination_dir.mkdir(parents=True, exist_ok=True)
destination = destination_dir / binary.name
shutil.copy2(binary, destination)
os.chmod(destination, 0o755)
print(f"Installed GPUI front-end: {destination}")
