# MIT License and Cross-Platform CI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add MIT licensing and a GitHub Actions pipeline that tests and packages Windows NSIS plus Linux DEB and AppImage artifacts, then creates releases for version tags.

**Architecture:** One matrix build job owns all platform tests and bundles. One tag-only release job downloads exact artifacts from successful matrix builds and creates the GitHub Release with least privilege.

**Tech Stack:** MIT, Rust, Cargo, Tauri v2, Node.js 22, npm, GitHub Actions, GitHub CLI, NSIS, DEB, AppImage.

---

### Task 1: Add MIT licensing metadata

**Files:**
- Create: `LICENSE`
- Modify: `Cargo.toml`
- Modify: `RELEASING.md`

- [ ] **Step 1: Add standard MIT license text**

Create `LICENSE` with `Copyright (c) 2026 quangnv13` and unmodified MIT grant, disclaimer, and warranty text.

- [ ] **Step 2: Add Cargo package metadata**

Add these fields below package version:

```toml
authors = ["quangnv13"]
description = "Desktop notification companion for Nonstop test runs."
repository = "https://github.com/quangnv13/nonstop-notify"
license = "MIT"
```

- [ ] **Step 3: Align WinGet release metadata**

Change `RELEASING.md` package license value from `Proprietary` to `MIT`.

- [ ] **Step 4: Validate license consistency**

Run:

```powershell
rg -n "MIT License|Copyright \(c\) 2026 quangnv13" LICENSE
rg -n '^license = "MIT"$|^repository = "https://github.com/quangnv13/nonstop-notify"$' Cargo.toml
rg -n '\| License \| `MIT` \|' RELEASING.md
```

Expected: all three files report matching MIT metadata.

### Task 2: Add cross-platform GitHub Actions workflow

**Files:**
- Create: `.github/workflows/build.yml`
- Create: `icons/128x128.png`
- Modify: `tauri.conf.json`
- Verify: `ui-react/package-lock.json`

- [ ] **Step 1: Confirm deterministic frontend install input**

Run:

```powershell
npm.cmd --prefix ui-react ci --ignore-scripts
```

Expected: command exits with code `0` using existing `ui-react/package-lock.json`.

- [ ] **Step 2: Add workflow triggers and least-privilege defaults**

Define `pull_request`, `push` for `master` and `v*` tags, and `workflow_dispatch`. Set default `contents: read` and concurrency that cancels branch builds but preserves tag builds.

- [ ] **Step 3: Add Windows and Linux matrix build**

Use matrix entries:

```yaml
include:
  - name: windows-x64
    os: windows-latest
    bundles: nsis
    artifactPath: target/release/bundle/nsis/*.exe
  - name: linux-x64
    os: ubuntu-22.04
    bundles: deb,appimage
    artifactPath: |
      target/release/bundle/deb/*.deb
      target/release/bundle/appimage/*.AppImage
```

Install Linux system packages only on Linux. Both matrix entries install Node.js 22, stable Rust, Rust cache, frontend dependencies, run `cargo test --locked`, build requested bundles with `tauri-apps/tauri-action@v1`, and upload artifacts for 14 days with missing-file failure.

- [ ] **Step 4: Add tag-only release job**

Wait for matrix job, grant only this job `contents: write`, download all build artifacts, verify at least one file exists, and call `gh release create` with tag verification and generated notes.

- [ ] **Step 5: Validate workflow structure**

Run a YAML parser and `actionlint` when available. Assert workflow contains Windows, Linux, NSIS, DEB, AppImage, artifact upload, artifact download, tag condition, and release command.

- [ ] **Step 6: Provide Linux bundle icon**

Add a square `icons/128x128.png` derived from existing `icons/icon.ico` and list both files in `bundle.icon`. This satisfies AppImage icon requirements without inventing new branding.

### Task 3: Verify both platform paths

**Files:**
- Verify: `.github/workflows/build.yml`
- Verify: `target/release/bundle/nsis/*.exe`
- Verify through WSL: Linux Cargo dependency graph and package commands

- [ ] **Step 1: Run Windows validation**

Run:

```powershell
npm.cmd --prefix ui-react ci --ignore-scripts
cargo test --locked
cargo tauri build --bundles nsis
```

Expected: all commands exit with code `0`, with one NSIS installer under `target/release/bundle/nsis/`.

- [ ] **Step 2: Validate Linux dependency installation in WSL**

Use Ubuntu 24.04 WSL to install or confirm workflow packages and verify `pkg-config` resolves WebKitGTK 4.1, AppIndicator, and ALSA.

- [ ] **Step 3: Run Linux tests and bundle build in WSL**

Copy repository to Linux filesystem or build from mounted source after ensuring Linux-native Node.js and Rust are active. Run:

```bash
npm ci --prefix ui-react --ignore-scripts
cargo test --locked
cargo tauri build --bundles deb,appimage
```

Expected: Rust tests pass and Linux DEB plus AppImage files are produced.

- [ ] **Step 4: Audit final deliverables**

Check `LICENSE`, Cargo metadata, WinGet license metadata, workflow syntax, workflow permissions, artifact globs, local Windows artifact, WSL Linux artifacts, and `git status --short`. Do not commit or push.
