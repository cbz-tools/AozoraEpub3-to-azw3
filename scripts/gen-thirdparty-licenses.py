#!/usr/bin/env python3
"""Generate the repository's concise third-party license overview."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any, NoReturn

try:
    import tomllib
except ModuleNotFoundError as exc:  # pragma: no cover - depends on the host Python.
    raise SystemExit("Python 3.11 or newer is required (tomllib is unavailable).") from exc


class GenerationError(RuntimeError):
    """Raised when the manifest, lockfile, or cargo-license metadata is invalid."""


def fail(message: str) -> NoReturn:
    raise GenerationError(message)


def read_toml(path: Path) -> dict[str, Any]:
    try:
        with path.open("rb") as stream:
            value = tomllib.load(stream)
    except (OSError, tomllib.TOMLDecodeError) as exc:
        fail(f"could not read TOML file {path}: {exc}")
    if not isinstance(value, dict):
        fail(f"TOML root is not a table: {path}")
    return value


def package_name_from_spec(dependency_name: str, spec: Any, section: str) -> str:
    if isinstance(spec, (str, dict)):
        package_name = dependency_name
        if isinstance(spec, dict) and "package" in spec:
            package_name = spec["package"]
            if not isinstance(package_name, str) or not package_name.strip():
                fail(f"invalid package name for {dependency_name!r} in [{section}]")
        return package_name
    fail(f"unsupported dependency specification for {dependency_name!r} in [{section}]")


def add_dependency_table(
    table: Any,
    section: str,
    classifications: dict[str, set[str]],
    category: str,
) -> None:
    if table is None:
        return
    if not isinstance(table, dict):
        fail(f"[{section}] is not a table")
    for dependency_name, spec in table.items():
        if not isinstance(dependency_name, str) or not dependency_name.strip():
            fail(f"invalid dependency name in [{section}]")
        package_name = package_name_from_spec(dependency_name, spec, section)
        classifications.setdefault(package_name, set()).add(category)


def collect_classifications(manifest: dict[str, Any]) -> tuple[str, dict[str, set[str]]]:
    package = manifest.get("package")
    if not isinstance(package, dict) or not isinstance(package.get("name"), str):
        fail("Cargo.toml has no valid [package].name")
    root_name = package["name"]

    classifications: dict[str, set[str]] = {}
    sections = (
        ("dependencies", "runtime"),
        ("build-dependencies", "build"),
        ("dev-dependencies", "dev"),
    )
    for section, category in sections:
        add_dependency_table(manifest.get(section), section, classifications, category)

    targets = manifest.get("target", {})
    if not isinstance(targets, dict):
        fail("[target] is not a table")
    for target_name, target in targets.items():
        if not isinstance(target, dict):
            fail(f"target {target_name!r} is not a table")
        for section, category in sections:
            qualified_section = f"target.{target_name}.{section}"
            add_dependency_table(target.get(section), qualified_section, classifications, category)

    overlapping = sorted(
        package_name
        for package_name, categories in classifications.items()
        if "runtime" in categories and "build" in categories
    )
    if overlapping:
        fail(
            "dependency is classified as both runtime and build-only: "
            + ", ".join(overlapping)
        )
    return root_name, classifications


def read_license_metadata(input_path: Path | None) -> list[dict[str, Any]]:
    try:
        if input_path is None:
            raw = sys.stdin.buffer.read()
        else:
            raw = input_path.read_bytes()
        value = json.loads(raw)
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
        fail(f"could not parse cargo-license JSON: {exc}")
    if not isinstance(value, list):
        fail("cargo-license JSON must be an array")
    records: list[dict[str, Any]] = []
    for index, record in enumerate(value):
        if not isinstance(record, dict):
            fail(f"cargo-license record {index} is not an object")
        for field in ("name", "version"):
            if not isinstance(record.get(field), str) or not record[field].strip():
                fail(f"cargo-license record {index} has invalid {field}")
        records.append(record)
    return records


def read_lock_versions(lock: dict[str, Any]) -> dict[str, set[str]]:
    packages = lock.get("package")
    if not isinstance(packages, list):
        fail("Cargo.lock has no package records")
    versions: dict[str, set[str]] = {}
    for index, package in enumerate(packages):
        if not isinstance(package, dict):
            fail(f"Cargo.lock package record {index} is not an object")
        name = package.get("name")
        version = package.get("version")
        if not isinstance(name, str) or not name.strip():
            fail(f"Cargo.lock package record {index} has invalid name")
        if not isinstance(version, str) or not version.strip():
            fail(f"Cargo.lock package record {index} has invalid version")
        versions.setdefault(name, set()).add(version)
    return versions


def read_lock_root_dependencies(lock: dict[str, Any], root_name: str) -> list[str]:
    packages = lock.get("package")
    if not isinstance(packages, list):
        fail("Cargo.lock has no package records")
    root_records = [package for package in packages if isinstance(package, dict) and package.get("name") == root_name]
    if len(root_records) != 1:
        fail(f"expected exactly one root package record in Cargo.lock for {root_name}")
    dependencies = root_records[0].get("dependencies", [])
    if not isinstance(dependencies, list) or any(not isinstance(dependency, str) for dependency in dependencies):
        fail(f"Cargo.lock root dependency list is invalid for {root_name}")
    return dependencies


def locked_direct_version(
    name: str,
    root_dependencies: list[str],
    lock_versions: dict[str, set[str]],
) -> str:
    entries = [
        dependency
        for dependency in root_dependencies
        if dependency == name or dependency.startswith(name + " ")
    ]
    if len(entries) != 1:
        fail(f"Cargo.lock has {len(entries)} root dependency entries for direct dependency {name}")
    entry = entries[0]
    versions = lock_versions.get(name, set())
    if entry == name:
        if len(versions) != 1:
            fail(f"Cargo.lock does not disambiguate versions for direct dependency {name}")
        return next(iter(versions))
    version = entry[len(name) + 1 :].split(" ", 1)[0]
    if not version or version not in versions:
        fail(f"Cargo.lock root dependency entry has invalid version for {name}: {entry}")
    return version


def required_records(
    records: list[dict[str, Any]],
    root_name: str,
    classifications: dict[str, set[str]],
    lock_versions: dict[str, set[str]],
    root_dependencies: list[str],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    by_name: dict[str, list[dict[str, Any]]] = {}
    keys: set[tuple[str, str]] = set()
    for record in records:
        name = record["name"]
        version = record["version"]
        key = (name, version)
        if key in keys:
            fail(f"duplicate cargo-license metadata for {name} {version}")
        keys.add(key)
        by_name.setdefault(name, []).append(record)

    root_records = by_name.get(root_name, [])
    if len(root_records) != 1:
        fail(f"expected exactly one root cargo-license record for {root_name}")

    known_names = set(classifications)
    for name in by_name:
        if name != root_name and name not in known_names:
            fail(f"cargo-license returned undeclared direct dependency: {name}")

    runtime: list[dict[str, Any]] = []
    build: list[dict[str, Any]] = []
    for name, categories in classifications.items():
        if not categories & {"runtime", "build"}:
            continue
        matches = by_name.get(name, [])
        if len(matches) == 0:
            fail(f"direct dependency {name} is missing from cargo-license output")
        if len(matches) != 1:
            versions = ", ".join(sorted(record["version"] for record in matches))
            fail(f"ambiguous cargo-license versions for direct dependency {name}: {versions}")
        record = matches[0]
        version = record["version"]
        expected_version = locked_direct_version(name, root_dependencies, lock_versions)
        if version != expected_version:
            fail(
                f"cargo-license version for {name} does not match Cargo.lock root dependency: "
                f"{version} != {expected_version}"
            )
        license_value = record.get("license")
        if not isinstance(license_value, str) or not license_value.strip():
            fail(f"license metadata is missing for direct dependency {name} {version}")
        if "\r" in license_value or "\n" in license_value:
            fail(f"license metadata contains a newline for direct dependency {name} {version}")
        if "runtime" in categories:
            runtime.append(record)
        elif "build" in categories:
            build.append(record)
        else:  # Defensive guard for future classification changes.
            fail(f"unexpected dependency classification for {name}")
    return runtime, build


def markdown_cell(value: str) -> str:
    return value.replace("|", r"\|").replace("\r", " ").replace("\n", " ")


def render_rows(records: list[dict[str, Any]]) -> list[str]:
    rows = sorted(records, key=lambda record: (record["name"].casefold(), record["name"], record["version"]))
    return [
        f"| `{markdown_cell(record['name'])}` | {markdown_cell(record['version'])} | {markdown_cell(record['license'].strip())} |"
        for record in rows
    ]


def render_markdown(runtime: list[dict[str, Any]], build: list[dict[str, Any]]) -> str:
    lines = [
        "# Third-Party Licenses",
        "",
        "This project uses third-party Rust crates. Exact dependency versions are recorded in",
        "`Cargo.lock`.",
        "",
        "## Runtime dependencies",
        "",
        "These direct dependencies are used by the shipped library and CLI:",
        "",
        "| Crate | Version | License |",
        "|---|---:|---|",
        *render_rows(runtime),
        "",
        "## Build-only dependencies",
        "",
        "These direct dependencies are used only while building the project and are not shipped",
        "runtime components:",
        "",
        "| Crate | Version | License |",
        "|---|---:|---|",
        *render_rows(build),
        "",
    ]
    return "\n".join(lines)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, help="cargo-license JSON file; stdin when omitted")
    parser.add_argument("--manifest-path", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    manifest_path = args.manifest_path.resolve()
    output_path = args.output.resolve()
    if not manifest_path.is_file():
        fail(f"Cargo.toml not found: {manifest_path}")
    lock_path = manifest_path.with_name("Cargo.lock")
    if not lock_path.is_file():
        fail(f"Cargo.lock not found: {lock_path}")

    manifest = read_toml(manifest_path)
    lock = read_toml(lock_path)
    root_name, classifications = collect_classifications(manifest)
    records = read_license_metadata(args.input.resolve() if args.input else None)
    lock_versions = read_lock_versions(lock)
    root_dependencies = read_lock_root_dependencies(lock, root_name)
    runtime, build = required_records(records, root_name, classifications, lock_versions, root_dependencies)
    output_path.write_bytes(render_markdown(runtime, build).encode("utf-8"))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GenerationError as exc:
        print(f"gen-thirdparty-licenses.py: error: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc
