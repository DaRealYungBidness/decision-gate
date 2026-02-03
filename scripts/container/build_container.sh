#!/usr/bin/env bash
# scripts/container/build_container.sh
# =============================================================================
# Module: Decision Gate Container Build
# Description: Build the Decision Gate container image with buildx.
# Purpose: Produce local or multi-arch images for deployment.
# Dependencies: bash, docker, docker buildx
# =============================================================================
set -euo pipefail

IMAGE_REPO="${IMAGE_REPO:-decision-gate}"
IMAGE_TAG="${IMAGE_TAG:-dev}"
PLATFORMS="${PLATFORMS:-linux/amd64,linux/arm64}"
PUSH="${PUSH:-0}"

if [[ "${PUSH}" != "1" && "${PLATFORMS}" == *","* ]]; then
  echo "PLATFORMS='${PLATFORMS}' requires PUSH=1 (buildx --load supports a single platform)." >&2
  exit 1
fi

BUILD_ARGS=()
if [[ "${PUSH}" == "1" ]]; then
  BUILD_ARGS+=(--push)
else
  BUILD_ARGS+=(--load)
fi

docker buildx build \
  --platform "${PLATFORMS}" \
  -t "${IMAGE_REPO}:${IMAGE_TAG}" \
  "${BUILD_ARGS[@]}" \
  .
