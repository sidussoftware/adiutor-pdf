#!/usr/bin/env bash
# Generate the Rust crate notices (cargo-about) into <out.html> and check the clarified copyright lines.
set -euo pipefail

out="${1:?usage: $0 <out.html>}"
cargo about generate --locked --fail -o "$out" about.hbs

# cargo-about only warns when a clarification in about.toml fails (e.g. a failed fetch) and then ships the
# generic text, so check that each clarified crate's block still holds its upstream line.
# Update this list together with the clarifications in about.toml.
expected=(
  "block2 0.5.1|Copyright (c) Steven Sheldon"
  "objc-sys 0.3.5|Copyright (c) Steven Sheldon"
  "objc2 0.5.3|Copyright (c) Steven Sheldon"
  "objc2-app-kit 0.2.2|Copyright (c) Steven Sheldon"
  "objc2-foundation 0.2.2|Copyright (c) Steven Sheldon"
  "zvariant_utils 4.2.0|Copyright (c) 2024 Zeeshan Ali Khan"
  "winnow 1.0.5|Copyrights in winnow are retained by their contributors"
  "wayland-protocols-plasma 0.3.12|Copyright (c) 2015 Elinor Berger"
  "dpi 0.1.2|Copyright (c) 2018 Jorge Aparicio"
)
for e in "${expected[@]}"; do
  # Each record is one "Used by" block: the crate list and its license text.
  awk -v RS='<h3>Used by</h3>' -v c="<li>${e%%|*}</li>" -v s="${e#*|}" \
    'index($0, c) && index($0, s) { found = 1 } END { exit !found }' "$out" ||
    { echo "$out: no '${e#*|}' for ${e%%|*} (check about.toml and this list)" >&2; exit 1; }
done
