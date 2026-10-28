#!/usr/bin/env python3
"""Reject GPUI packages with a missing or incorrectly targeted executable."""

import argparse
import struct
import zipfile
from pathlib import Path


def pe_header(path: Path) -> tuple[str, int]:
    data = path.read_bytes()
    offset = struct.unpack_from("<I", data, 0x3C)[0]
    if data[offset : offset + 4] != b"PE\0\0":
        raise ValueError(f"Invalid PE header: {path}")
    machine = struct.unpack_from("<H", data, offset + 4)[0]
    # Optional header subsystem: 2 is IMAGE_SUBSYSTEM_WINDOWS_GUI, 3 is CUI.
    optional_header = offset + 24
    subsystem = struct.unpack_from("<H", data, optional_header + 68)[0]
    return {0x8664: "x86_64", 0xAA64: "arm64"}[machine], subsystem


def pe_sections(path: Path) -> list[str]:
    data = path.read_bytes()
    offset = struct.unpack_from("<I", data, 0x3C)[0]
    count = struct.unpack_from("<H", data, offset + 6)[0]
    optional_size = struct.unpack_from("<H", data, offset + 20)[0]
    table = offset + 24 + optional_size
    names = []
    for index in range(count):
        header = table + index * 40
        names.append(data[header : header + 8].rstrip(b"\0").decode("ascii", "replace"))
    return names


def pe_has_icon(path: Path) -> bool:
    """True when the PE resource directory contains an icon group."""
    data = path.read_bytes()
    offset = struct.unpack_from("<I", data, 0x3C)[0]
    count = struct.unpack_from("<H", data, offset + 6)[0]
    optional_size = struct.unpack_from("<H", data, offset + 20)[0]
    table = offset + 24 + optional_size
    for index in range(count):
        header = table + index * 40
        if data[header : header + 8].rstrip(b"\0") != b".rsrc":
            continue
        base = struct.unpack_from("<I", data, header + 20)[0]
        named, ids = struct.unpack_from("<HH", data, base + 12)
        for entry in range(named + ids):
            position = base + 16 + entry * 8
            name = struct.unpack_from("<I", data, position)[0]
            if name & 0x80000000:
                continue
            # RT_ICON (3) and RT_GROUP_ICON (14) both prove an icon exists.
            if name in (3, 14):
                return True
        return False
    return False


def architecture(path: Path) -> str:
    data = path.read_bytes()
    if data[:2] == b"MZ":
        return pe_header(path)[0]
    if data[:4] == b"\x7fELF":
        order = "<" if data[5] == 1 else ">"
        machine = struct.unpack_from(order + "H", data, 18)[0]
        return {62: "x86_64", 183: "arm64"}[machine]
    if data[:4] == b"\xcf\xfa\xed\xfe":
        cpu = struct.unpack_from("<I", data, 4)[0]
        return {0x01000007: "x86_64", 0x0100000C: "arm64"}[cpu]
    raise ValueError(f"Unsupported executable format: {path}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    parser.add_argument("--arch", required=True, choices=["x86_64", "arm64"])
    parser.add_argument("--portable-zip", type=Path)
    args = parser.parse_args()
    root = args.directory
    extension = ".exe" if (root / "aegisub.exe").is_file() else ""
    binaries = [
        root / (name + extension) for name in ("aegisub", "aegisub-gpui-prototype")
    ]
    for binary in binaries:
        if not binary.is_file():
            raise SystemExit(f"GPUI package missing executable: {binary}")
        found = architecture(binary)
        if found != args.arch:
            raise SystemExit(f"{binary}: expected {args.arch}, found {found}")
        print(f"Verified {binary.name}: {found}")
        # A console subsystem would open a black window next to the workspace.
        if binary.suffix == ".exe" and "gpui" in binary.name:
            subsystem = pe_header(binary)[1]
            if subsystem != 2:
                raise SystemExit(
                    f"{binary}: expected GUI subsystem (2), found {subsystem}"
                )
            # Without an embedded resource section Windows shows a generic
            # document glyph in the taskbar and window switcher.
            if not pe_has_icon(binary):
                raise SystemExit(f"{binary}: missing embedded icon resource")
    if args.portable_zip:
        with zipfile.ZipFile(args.portable_zip) as archive:
            names = set(archive.namelist())
            for binary in binaries:
                entry = f"{root.name}/{binary.name}"
                if entry not in names or archive.read(entry) != binary.read_bytes():
                    raise SystemExit(
                        f"Portable archive has missing or different executable: {entry}"
                    )
            for runtime in ("vcruntime140.dll", "msvcp140.dll"):
                if f"{root.name}/{runtime}" not in names:
                    raise SystemExit(f"Portable archive missing runtime: {runtime}")
            if any(name.lower().endswith(".zip") for name in names):
                raise SystemExit("Portable archive contains a nested zip")
        print(
            f"Verified directly extractable portable archive: {args.portable_zip.name}"
        )


if __name__ == "__main__":
    main()
