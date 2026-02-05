# Container Script

Container image build helper.

- `build_container.sh`: Build the Decision Gate image with Docker buildx.

Environment variables:

- `IMAGE_REPO`: Image repository name (default: decision-gate).
- `IMAGE_TAG`: Image tag (default: dev).
- `PLATFORMS`: Comma-separated platforms (default: linux/amd64,linux/arm64).
- `PUSH`: Set to 1 to push multi-arch images (default: 0).

Example:

- `IMAGE_REPO=decision-gate IMAGE_TAG=dev scripts/container/build_container.sh`
