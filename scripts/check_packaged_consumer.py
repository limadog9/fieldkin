#!/usr/bin/env python3
"""Run the packaged example as a fresh downstream dependency, then check features."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib


def main():
    if len(sys.argv) != 2:
        raise SystemExit("usage: check_packaged_consumer.py PATH_TO_FIELDKIN_CRATE")
    artifact = Path(sys.argv[1]).resolve(strict=True)
    with artifact.open("rb") as stream:
        print(f"Artifact: {artifact}\nSHA256: {hashlib.file_digest(stream, 'sha256').hexdigest()}",
              flush=True)
    with tempfile.TemporaryDirectory(prefix="fieldkin-consumer-") as temporary:
        root = Path(temporary).resolve()
        if root.is_relative_to(Path(__file__).resolve().parents[1]):
            raise RuntimeError("consumer temporary directory must be outside the repository")
        unpacked = root / "unpacked"
        unpacked.mkdir()
        with tarfile.open(artifact, "r:gz") as archive:
            archive.extractall(unpacked, filter="data")
        packages = list(unpacked.iterdir())
        if len(packages) != 1 or not packages[0].is_dir():
            raise ValueError("crate must contain exactly one package directory")
        package = packages[0]
        metadata = tomllib.loads((package / "Cargo.toml").read_text())
        if metadata["package"]["name"] != "fieldkin":
            raise ValueError("expected the fieldkin package")
        consumer = root / "consumer"
        (consumer / "src").mkdir(parents=True)
        source = consumer / "src" / "main.rs"
        shutil.copyfile(package / "examples" / "valentine.rs", source)
        (consumer / "Cargo.toml").write_text(
            '[package]\nname = "fieldkin-packaged-consumer"\nversion = "0.0.0"\n'
            'edition = "2024"\n\n[dependencies]\n'
            f'fieldkin = {{ path = {json.dumps(str(package))} }}\n'
        )
        environment = {**os.environ, "CARGO_TARGET_DIR": str(root / "target")}

        def run(*arguments):
            print("$ cargo " + " ".join(arguments), flush=True)
            subprocess.run(["cargo", *arguments], cwd=consumer, env=environment, check=True)

        print(f"Consumer: {consumer}; package version: {metadata['package']['version']}", flush=True)
        # The dependency's packaged lockfile is not the consumer's lockfile.
        run("generate-lockfile")
        print("Running the actual packaged example with default features", flush=True)
        run("run", "--locked")
        # These API references compile only; constructing a model is never executed.
        with source.open("a") as stream:
            stream.write('\n#[allow(dead_code)]\nfn optional_api_check() {\n'
                         '    let _ = |frame| fieldkin::Table::from_polars("consumer", frame);\n'
                         '    let _ = fieldkin::embeddings::FastEmbedProvider::all_minilm_l6_v2;\n}\n')
        features = "fieldkin/polars,fieldkin/embeddings"
        print("Optional Polars and embeddings: compile check only", flush=True)
        resolved = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--locked", "--format-version", "1", "--features", features],
            cwd=consumer, env=environment,
        ))
        print("Resolved downstream dependencies (including optional features):", flush=True)
        for dependency in sorted(resolved["packages"], key=lambda item: (item["name"], item["version"])):
            print(f"  {dependency['name']} {dependency['version']} "
                  f"(rust-version={dependency.get('rust_version') or 'unspecified'})")
        run("check", "--locked", "--features", features)
        print("Packaged default execution and optional-feature compilation passed", flush=True)


if __name__ == "__main__":
    main()
