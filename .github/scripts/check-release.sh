#!/usr/bin/env bash
# Release preflight: refuse to build a version that already has a release.
#
# GitHub only creates a release's tag when the draft is published, so checking git tags
# misses unpublished drafts, and a second run would create a duplicate draft. This asks
# the Releases API instead, which lists drafts too (needs a token with contents access).
#
# Env: VERSION (from tauri.conf.json), REF (the ref the run started on), GH_REPO (owner/name),
#      GH_TOKEN.
set -euo pipefail

tag="v${VERSION:?}"

if [[ "${REF:?}" != "refs/heads/main" ]]; then
  echo "::error::Start releases from main (this run is on ${REF})."
  exit 1
fi

existing=$(gh api "repos/${GH_REPO:?}/releases" --paginate \
  --jq ".[] | select(.tag_name == \"${tag}\") | if .draft then \"draft\" else \"published\" end")

case "$existing" in
  *published*)
    echo "::error::${tag} is already released. Bump the version (tauri.conf.json, Cargo.toml, package.json) and update CHANGELOG.md first."
    exit 1 ;;
  *draft*)
    echo "::error::A draft release for ${tag} already exists. Publish or delete it on the Releases page before starting another build of this version."
    exit 1 ;;
esac

echo "OK: no release for ${tag} yet."
