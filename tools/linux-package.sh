#!/usr/bin/env bash
# Build a self-contained Linux binary tarball from a Meson install tree.
#
# The archive unpacks to a single directory containing the host binary, the GPUI
# front-end and the shared libraries they need, so it runs without installing.
set -euo pipefail

BUILD_DIR="${1:?usage: linux-package.sh <build-dir> <arch> [output-dir]}"
ARCH="${2:?usage: linux-package.sh <build-dir> <arch> [output-dir]}"
OUTPUT_DIR="${3:-${BUILD_DIR}}"

VERSION="$(sed -n 's/^#define BUILD_GIT_VERSION_STRING "\(.*\)"$/\1/p' "${BUILD_DIR}/git_version.h")"
if [ -z "${VERSION}" ]; then
  echo "Could not read the build version from ${BUILD_DIR}/git_version.h" >&2
  exit 1
fi

# Discover the configured prefix before installing so the install tree path is known.
PREFIX="$(python3 - "${BUILD_DIR}/meson-info/intro-buildoptions.json" <<'PY'
import json, sys
options = json.load(open(sys.argv[1]))
print(next(o["value"] for o in options if o["name"] == "prefix"))
PY
)"

STAGE="$(mktemp -d)"
trap 'rm -rf "${STAGE}"' EXIT

ROOT_NAME="Aegisub-${VERSION}-${ARCH}"
APP_DIR="${STAGE}/${ROOT_NAME}"
mkdir -p "${APP_DIR}"

DESTDIR="${STAGE}/install" meson install -C "${BUILD_DIR}" --no-rebuild --skip-subprojects >/dev/null
INSTALL_ROOT="${STAGE}/install${PREFIX}"

if [ ! -x "${INSTALL_ROOT}/bin/aegisub" ]; then
  echo "Host binary missing from install tree: ${INSTALL_ROOT}/bin/aegisub" >&2
  exit 1
fi
cp -a "${INSTALL_ROOT}/bin/." "${APP_DIR}/"

if [ ! -x "${APP_DIR}/aegisub-gpui-prototype" ]; then
  echo "GPUI front-end missing from the install tree" >&2
  exit 1
fi

# Ship the resources the host reads at runtime.
for dir in share/locale share/aegisub; do
  if [ -d "${INSTALL_ROOT}/${dir}" ]; then
    mkdir -p "${APP_DIR}/share"
    cp -a "${INSTALL_ROOT}/${dir}" "${APP_DIR}/${dir}"
  fi
done

# Bundle non-system shared libraries so the tarball runs on a clean machine.
if command -v ldd >/dev/null 2>&1; then
  mkdir -p "${APP_DIR}/lib"
  copy_libs() {
    ldd "$1" 2>/dev/null | awk '/=> \//{print $3}' | while read -r lib; do
      case "$(basename "${lib}")" in
        libc.so.*|libm.so.*|libdl.so.*|libpthread.so.*|librt.so.*|ld-linux*|libgcc_s.so.*|libstdc++.so.*) continue ;;
      esac
      cp -nL "${lib}" "${APP_DIR}/lib/" 2>/dev/null || true
    done
  }
  copy_libs "${APP_DIR}/aegisub"
  copy_libs "${APP_DIR}/aegisub-gpui-prototype"
fi

cat > "${APP_DIR}/README.txt" <<EOF
Aegisub ${VERSION} (${ARCH}) - portable build

Run ./aegisub. The GPUI workspace opens from View > GPUI Workspace Prototype.

This archive bundles its own libraries. If the binary cannot find them, start it
with: LD_LIBRARY_PATH="\$PWD/lib" ./aegisub
EOF

mkdir -p "${OUTPUT_DIR}"
ARCHIVE="${OUTPUT_DIR}/Aegisub-${VERSION}-${ARCH}.tar.gz"
tar -C "${STAGE}" -czf "${ARCHIVE}" "${ROOT_NAME}"
echo "Created ${ARCHIVE}"
