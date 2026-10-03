"""Allowlisted source and portable-package credential scan. Never prints values."""
from __future__ import annotations
import argparse
import pathlib
import re
import subprocess
import sys
import zipfile

PATTERNS = {
    "private key block": re.compile(r"-----BEGIN\s+(?:PGP |RSA |EC |OPENSSH )?PRIVATE KEY(?: BLOCK)?-----"),
    "literal WireGuard key": re.compile(r"(?im)(?:private[-_]?key|public[-_]?key|presharedkey|pre-shared-key)\s*[:=]\s*[\"']?([A-Za-z0-9+/]{43}=)"),
    "literal Telegram secret": re.compile(r'(?im)[\"\']?secret[\"\']?\s*[:=]\s*[\"\'][a-f0-9]{32}[\"\']'),
    "GitHub credential": re.compile(r"(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{40,})"),
    "private user path": re.compile(r"(?i)[a-z]:[\\/]Users[\\/](?!YOUR_USER(?:[\\/]|\b))[^\\/\s\"']+[\\/]"),
}
FORBIDDEN_NAMES = {"settings.json", "config.json", "installed.json", "installed-paths.json", "installed-data.json", "workspace.json", "urt-location.json", "telegram-private.json", "cache.db", "routing.log", "proxy-backup.json"}

def check(name: str, data: bytes) -> list[str]:
    errors = []
    path = pathlib.PurePosixPath(name.replace("\\", "/"))
    if path.name in FORBIDDEN_NAMES or path.name.endswith(".pid.json"):
        errors.append(f"{name}: runtime file must not be distributed")
    if path.suffix == ".conf" and "examples" not in path.parts:
        errors.append(f"{name}: private profile location")
    text = data.decode("utf-8", errors="ignore")
    for label, pattern in PATTERNS.items():
        if pattern.search(text):
            errors.append(f"{name}: {label}")
    return errors

def scan_repo(root: pathlib.Path) -> list[str]:
    result = subprocess.run(["git", "-C", str(root), "ls-files", "--cached", "--others", "--exclude-standard", "-z"], check=True, capture_output=True)
    errors = []
    for name in sorted(set(result.stdout.decode("utf-8").split("\0")) - {""}):
        path = root / name
        if path.is_symlink():
            errors.append(f"{name}: source symlinks are not allowed")
        elif path.is_file():
            errors.extend(check(name, path.read_bytes()))
    return errors

def scan_zip(archive: pathlib.Path) -> list[str]:
    errors = []
    with zipfile.ZipFile(archive) as package:
        for entry in package.infolist():
            if entry.is_dir():
                continue
            parts = pathlib.PurePosixPath(entry.filename).parts
            if ".." in parts or entry.filename.startswith(("/", "\\")):
                errors.append(f"{archive.name}: unsafe archive path")
            else:
                errors.extend(check(f"{archive.name}/{entry.filename}", package.read(entry)))
    return errors

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=pathlib.Path)
    parser.add_argument("--zip", nargs="*", type=pathlib.Path, default=[])
    args = parser.parse_args()
    errors = scan_repo(args.repo.resolve()) if args.repo else []
    for archive in args.zip:
        errors.extend(scan_zip(archive))
    for error in errors:
        print(error, file=sys.stderr)
    if errors:
        return 1
    print("Source/package credential scan passed (values are never printed).")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
