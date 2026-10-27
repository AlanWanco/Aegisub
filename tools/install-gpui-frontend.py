#!/usr/bin/env python3
"""Install the exact GPUI executable selected by the build, beside its host."""

import os
import shutil
import sys
from pathlib import Path


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: install-gpui-frontend.py <executable> <bindir>")
    binary = Path(sys.argv[1])
    if not binary.is_file():
        raise SystemExit(f"GPUI executable missing: {binary}")
    bindir = Path(sys.argv[2])
    destdir = os.environ.get("DESTDIR", "")
    # Meson stages absolute install paths below DESTDIR (without drive/root).
    if destdir and bindir.is_absolute():
        destination_dir = Path(destdir).joinpath(*bindir.parts[1:])
    elif bindir.is_absolute():
        destination_dir = bindir
    else:
        destination_dir = Path(os.environ["MESON_INSTALL_DESTDIR_PREFIX"]) / bindir
    destination_dir.mkdir(parents=True, exist_ok=True)
    name = "aegisub-gpui-prototype" + (".exe" if binary.suffix == ".exe" else "")
    destination = destination_dir / name
    shutil.copy2(binary, destination)
    destination.chmod(0o755)
    print(f"Installed GPUI front-end: {destination}")


if __name__ == "__main__":
    main()
