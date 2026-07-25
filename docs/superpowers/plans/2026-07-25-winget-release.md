# WinGet Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce a current-user NSIS installer and document manual submission of `quangnv13.nonstop-notify` to WinGet.

**Architecture:** Keep release flow manual. Tauri owns installer generation, GitHub Releases hosts immutable installer assets, and `wingetcreate` generates and submits community manifests.

**Tech Stack:** Rust, Tauri v2, React/Vite, NSIS, GitHub Releases, WinGet Create.

---

### Task 1: Configure NSIS bundling

**Files:**
- Modify: `tauri.conf.json`

- [ ] **Step 1: Confirm current config lacks an active bundle**

Run:

```powershell
$config = Get-Content tauri.conf.json -Raw | ConvertFrom-Json
if ($config.bundle.active -ne $false) { throw 'Expected inactive bundle before change' }
```

Expected: exit code `0`.

- [ ] **Step 2: Apply minimal standalone build and NSIS config**

Set build command to `npm run build` because Tauri executes it from `ui-react`. Enable only `nsis`, set publisher and copyright to `quangnv13`, and set `bundle.windows.nsis.installMode` to `currentUser`.

- [ ] **Step 3: Validate configured values**

Run:

```powershell
$config = Get-Content tauri.conf.json -Raw | ConvertFrom-Json
if ($config.build.beforeBuildCommand -ne 'npm run build') { throw 'Wrong UI build command' }
if (-not $config.bundle.active) { throw 'Bundle inactive' }
if ($config.bundle.targets.Count -ne 1 -or $config.bundle.targets[0] -ne 'nsis') { throw 'Wrong bundle target' }
if ($config.bundle.windows.nsis.installMode -ne 'currentUser') { throw 'Wrong install mode' }
```

Expected: exit code `0`.

### Task 2: Document release workflow

**Files:**
- Create: `RELEASING.md`

- [ ] **Step 1: Document prerequisites and version synchronization**

Include GitHub CLI authentication, Tauri CLI installation, WinGet Create installation, and matching versions in `Cargo.toml` and `tauri.conf.json`.

- [ ] **Step 2: Document installer build and local checks**

Use `cargo tauri build --bundles nsis`, locate one artifact under `target/release/bundle/nsis/`, calculate SHA256, and test NSIS silent installation with `/S`.

- [ ] **Step 3: Document GitHub Release and WinGet submission**

Create tag `v0.1.0`, upload installer, run `wingetcreate new` with release asset URL, use `quangnv13.nonstop-notify`, submit generated PR, and verify exact-ID installation after merge.

- [ ] **Step 4: Check required release tokens exist**

Run:

```powershell
rg -n "quangnv13\.nonstop-notify|cargo tauri build|wingetcreate new|gh release create|Get-FileHash|/S" RELEASING.md
```

Expected: every required workflow token is present.

### Task 3: Build and verify installer

**Files:**
- Verify: `Cargo.toml`
- Verify: `tauri.conf.json`
- Verify: `ui-react/package.json`
- Verify: `target/release/bundle/nsis/*.exe`

- [ ] **Step 1: Install missing Tauri CLI**

Run only when `cargo tauri --version` fails:

```powershell
cargo install tauri-cli --version '^2.0.0' --locked
```

Expected: `cargo tauri --version` prints a Tauri CLI 2.x version.

- [ ] **Step 2: Build frontend independently**

Run:

```powershell
npm --prefix ui-react run build
```

Expected: Vite exits with code `0`.

- [ ] **Step 3: Build NSIS installer**

Run:

```powershell
cargo tauri build --bundles nsis
```

Expected: Tauri exits with code `0` and reports an NSIS installer path.

- [ ] **Step 4: Verify artifact identity**

Run:

```powershell
$installers = @(Get-ChildItem target/release/bundle/nsis -Filter '*.exe')
if ($installers.Count -ne 1) { throw "Expected one NSIS installer, found $($installers.Count)" }
if ($installers[0].Name -notmatch '0\.1\.0') { throw "Installer version missing from $($installers[0].Name)" }
Get-FileHash $installers[0].FullName -Algorithm SHA256
```

Expected: one versioned installer and a SHA256 hash.

- [ ] **Step 5: Review final diff and status**

Run:

```powershell
git diff --check
git status --short
```

Expected: no whitespace errors; only intended source and documentation changes plus pre-existing untracked project files.
