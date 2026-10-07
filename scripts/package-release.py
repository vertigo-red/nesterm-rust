#!/usr/bin/env python3
"""Package native release binaries and their notices; requires Python 3.11+."""
import argparse
import gzip
import hashlib
import io
import json
import re
import subprocess
import tarfile
import tomllib
from pathlib import Path
import zipfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = (
    "x86_64-unknown-linux-musl",
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-pc-windows-msvc",
)
FALLBACK_NOTICES = {
    ("tetanes-core", "0.17.0"): "tetanes-core-MIT.txt",
    ("block2", "0.6.2"): "objc2-workspace-MIT.txt",
    ("dispatch2", "0.3.1"): "objc2-workspace-MIT.txt",
    ("objc2", "0.6.5"): "objc2-workspace-MIT.txt",
    ("objc2-encode", "4.1.0"): "objc2-workspace-MIT.txt",
}


def output(*command):
    return subprocess.check_output(command, cwd=ROOT, text=True).strip()


def version():
    value = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    if not re.fullmatch(r"\d+\.\d+\.\d+", value):
        raise ValueError("Release version must be a stable MAJOR.MINOR.PATCH")
    return value


def archive_name(target):
    suffix = ".zip" if "windows" in target else ".tar.gz"
    return f"nesterm-v{version()}-{target}{suffix}"


def dependency_notices(target):
    metadata = json.loads(output("cargo", "metadata", "--locked", "--format-version",
                                 "1", "--filter-platform", target))
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    included, pending = set(), [metadata["resolve"]["root"]]
    while pending:
        current = pending.pop()
        if current in included:
            continue
        included.add(current)
        pending.extend(dep["pkg"] for dep in nodes[current]["deps"]
                       if any(kind["kind"] != "dev" for kind in dep["dep_kinds"]))

    files, index = {}, []
    for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
        if package["id"] not in included or package["source"] is None:
            continue
        base = Path(package["manifest_path"]).parent
        prefix = f"licenses/dependencies/{package['name']}-{package['version']}"
        notices = {path for path in base.rglob("*") if path.is_file()
                   and path.name.upper().startswith(
                       ("LICENSE", "LICENCE", "COPYING", "NOTICE", "UNLICENSE"))}
        if package["license_file"]:
            notices.add(base / package["license_file"])
        fallback = FALLBACK_NOTICES.get((package["name"], package["version"]))
        if fallback and not notices:
            files[f"{prefix}/LICENSE-MIT"] = (ROOT / "licenses" / fallback).read_bytes()
        elif not notices:
            raise ValueError(f"No license notice found for {package['name']}")
        for path in sorted(notices):
            files[f"{prefix}/{path.relative_to(base).as_posix()}"] = path.read_bytes()
        # Provide the complete, unmodified covered source for the MPL dependency.
        if package["license"] == "MPL-2.0":
            for path in sorted(base.rglob("*")):
                if path.is_file():
                    files[f"{prefix}/source/{path.relative_to(base).as_posix()}"] = path.read_bytes()
        index.append(f"{package['name']} {package['version']}: {package['license']}")

    files["licenses/dependencies/INDEX.txt"] = ("\n".join(index) + "\n").encode()
    sysroot = Path(output("rustc", "--print", "sysroot"))
    files["licenses/Rust-COPYRIGHT-library.html"] = (
        sysroot / "share/doc/rust/COPYRIGHT-library.html").read_bytes()
    return files


def package(target, destination):
    executable = "nesterm.exe" if "windows" in target else "nesterm"
    binary = ROOT / "target" / target / "release" / executable
    if output(str(binary), "--version") != f"nesterm {version()}":
        raise ValueError("Binary version does not match Cargo.toml")
    files = {executable: binary.read_bytes()}
    for name in ("README.md", "LICENSE", "LICENSES-glyphs.txt", "THIRD_PARTY_NOTICES.md"):
        files[name] = (ROOT / name).read_bytes()
    for directory in ("licenses", "fonts"):
        for path in sorted((ROOT / directory).rglob("*")):
            if path.is_file():
                files[path.relative_to(ROOT).as_posix()] = path.read_bytes()
    files["docs/text-rendering.md"] = (ROOT / "docs/text-rendering.md").read_bytes()
    files.update(dependency_notices(target))
    prefix = f"nesterm-v{version()}-{target}"
    destination.mkdir(parents=True, exist_ok=True)
    archive = destination / archive_name(target)
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as stream:
            for name, data in sorted(files.items()):
                info = zipfile.ZipInfo(f"{prefix}/{name}", date_time=(1980, 1, 1, 0, 0, 0))
                info.create_system = 3
                info.external_attr = (0o100755 if name == executable else 0o100644) << 16
                stream.writestr(info, data, compress_type=zipfile.ZIP_DEFLATED)
    else:
        with archive.open("wb") as raw, gzip.GzipFile(fileobj=raw, mode="wb", filename="", mtime=0) as gz:
            with tarfile.open(fileobj=gz, mode="w") as stream:
                for name, data in sorted(files.items()):
                    info = tarfile.TarInfo(f"{prefix}/{name}")
                    info.size = len(data)
                    info.mode = 0o755 if name == executable else 0o644
                    stream.addfile(info, io.BytesIO(data))
    print(archive)


def checksums(destination):
    expected = {archive_name(target) for target in TARGETS}
    actual = {path.name for path in destination.iterdir() if path.is_file()}
    if actual - {"SHA256SUMS"} != expected:
        raise ValueError(f"Expected four release archives, found {sorted(actual)}")
    lines = []
    for name in sorted(expected):
        path = destination / name
        prefix = name.removesuffix(".tar.gz").removesuffix(".zip")
        if name.endswith(".zip"):
            with zipfile.ZipFile(path) as archive:
                if archive.testzip() is not None:
                    raise ValueError(f"Corrupted ZIP: {name}")
                members = set(archive.namelist())
            executable = "nesterm.exe"
        else:
            with tarfile.open(path, "r:gz") as archive:
                members = set(archive.getnames())
                if archive.getmember(f"{prefix}/nesterm").mode != 0o755:
                    raise ValueError(f"Missing executable permissions: {name}")
            executable = "nesterm"
        required = {f"{prefix}/{file}" for file in (
            executable, "README.md", "LICENSE", "LICENSES-glyphs.txt", "THIRD_PARTY_NOTICES.md",
            "licenses/dependencies/INDEX.txt", "licenses/Rust-COPYRIGHT-library.html",
            "licenses/font8x8-Public-Domain.txt", "fonts/cyrillic-demo.json", "docs/text-rendering.md")}
        if not required <= members or any(not member.startswith(prefix + "/") for member in members):
            raise ValueError(f"Incomplete archive: {name}")
        with path.open("rb") as stream:
            lines.append(f"{hashlib.file_digest(stream, 'sha256').hexdigest()}  {name}")
    (destination / "SHA256SUMS").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--target", choices=TARGETS)
    mode.add_argument("--checksums", action="store_true")
    parser.add_argument("--output", type=Path, default=ROOT / "dist")
    args = parser.parse_args()
    if args.checksums:
        checksums(args.output)
    else:
        package(args.target, args.output)
