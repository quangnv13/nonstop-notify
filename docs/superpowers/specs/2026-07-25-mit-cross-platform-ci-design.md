# MIT License and Cross-Platform CI Design

## Goal

License `nonstop-notify` under MIT and add one GitHub Actions workflow that tests and packages Windows and Linux builds, uploads CI artifacts, and creates a GitHub Release for version tags.

## License

- Add standard MIT text in `LICENSE`.
- Copyright holder: `quangnv13`.
- Copyright year: `2026`.
- Add `license = "MIT"`, repository URL, description, and author metadata to `Cargo.toml`.
- Change WinGet release metadata in `RELEASING.md` from `Proprietary` to `MIT`.

## Frontend Dependency Lock

- Keep existing `ui-react/package-lock.json` and verify it includes platform-specific optional dependencies.
- GitHub Actions uses `npm ci --prefix ui-react` for deterministic installs.
- Do not add a root JavaScript package or duplicate frontend scripts.

## Cross-Platform Bundle Icon

- Keep existing Windows `icons/icon.ico`.
- Add square `icons/128x128.png` derived from existing icon for Linux AppImage metadata.
- Declare both files in Tauri bundle configuration.

## Workflow

Create `.github/workflows/build.yml` with one build matrix and one conditional release job.

### Triggers

- Pull requests.
- Pushes to `master`.
- Tags matching `v*`.
- Manual `workflow_dispatch` runs.

### Permissions and Concurrency

- Default workflow permission is `contents: read`.
- Only release job gets `contents: write`.
- Cancel older branch and pull-request builds for same ref.
- Never cancel tag builds.

### Build Matrix

Windows job:

- Runner: `windows-latest`.
- Bundle command: `cargo tauri build --bundles nsis`.
- Artifact: `target/release/bundle/nsis/*.exe`.

Linux job:

- Runner: `ubuntu-22.04` for broader glibc compatibility.
- Install Tauri WebKitGTK, AppIndicator, SVG, OpenSSL, XDo, Patchelf, FUSE, and ALSA development packages.
- Bundle command: `cargo tauri build --bundles deb,appimage`.
- Artifacts: `target/release/bundle/deb/*.deb` and `target/release/bundle/appimage/*.AppImage`.

Both jobs:

- Check out source.
- Install Node.js 22 with npm cache tied to `ui-react/package-lock.json`.
- Install stable Rust.
- Cache Rust build dependencies.
- Run `npm ci --prefix ui-react`.
- Run `npm --prefix ui-react run build` before compiling Rust.
- Run `cargo test --locked`.
- Build platform bundles with official `tauri-apps/tauri-action@v1`, avoiding a repeated global CLI compilation on each runner.
- Upload platform artifacts for 14 days and fail if no bundle exists.

### Release Job

- Run only for tags matching `v*`.
- Wait for all matrix builds.
- Download and merge Windows and Linux artifacts.
- Fail if no release files exist.
- Use preinstalled GitHub CLI with `GITHUB_TOKEN` to create release from exact tag.
- Upload NSIS, DEB, and AppImage files from same successful workflow run.
- Generate release notes and verify tag exists.

## Deliberate Limits

- Do not add code-signing secrets or signing steps.
- Do not publish Linux packages to a package repository.
- Do not publish WinGet manifests from GitHub Actions.
- Do not add a reusable workflow until another repository needs same pipeline.

## Verification

- MIT text and Cargo metadata agree.
- `npm ci --prefix ui-react` succeeds from clean dependency state.
- `npm --prefix ui-react run build` creates `ui-react/dist` before Rust compilation.
- `cargo test --locked` succeeds on Windows locally.
- Windows NSIS build succeeds locally.
- Workflow YAML parses and passes `actionlint` when available.
- Linux package commands and dependencies match Tauri Linux prerequisites.
- Tauri has a square PNG icon required by AppImage bundling.
- Workflow artifact globs cover NSIS, DEB, and AppImage outputs.
