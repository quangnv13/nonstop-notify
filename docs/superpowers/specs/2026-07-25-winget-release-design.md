# WinGet Release Design

## Goal

Prepare `nonstop-notify` version `0.1.0` for a GitHub Release and submission to Windows Package Manager Community Repository as `quangnv13.nonstop-notify` using `wingetcreate`.

## Scope

- Enable Tauri bundling for an x64 NSIS installer.
- Install for current user without requiring Administrator access.
- Add installer publisher metadata.
- Document manual build, GitHub Release, manifest creation, and WinGet pull request workflow.
- Verify React UI build and Tauri installer output.

Do not add GitHub Actions, release scripts, signing certificate, or separate manifest repository.

## Release Identity

- GitHub repository: `quangnv13/nonstop-notify`
- WinGet PackageIdentifier: `quangnv13.nonstop-notify`
- Product name: `Nonstop Notify`
- Initial version: `0.1.0`
- Git tag: `v0.1.0`
- Architecture: `x64`
- Installer type: `NSIS`
- Install scope: current user

## Changes

### `tauri.conf.json`

- Replace the stale monorepo UI build command with `npm run build`; Tauri executes it from `ui-react`.
- Set `bundle.active` to `true`.
- Limit `bundle.targets` to `nsis`.
- Set NSIS `installMode` to `currentUser`.
- Set publisher and copyright metadata to `quangnv13`.

### `RELEASING.md`

Document version sync, Tauri build, artifact checks, silent install testing, GitHub Release creation, `wingetcreate new`, WinGet pull request submission, and post-merge installation checks.

## Verification

- `npm --prefix ui-react run build` exits with code `0`.
- Tauri resolves the frontend build command from this standalone repository.
- `cargo tauri build --bundles nsis` exits with code `0`.
- Exactly one version `0.1.0` installer exists under `target/release/bundle/nsis/`.
- Installer supports standard NSIS silent installation.
- `wingetcreate` generates `PackageIdentifier: quangnv13.nonstop-notify` and `PackageVersion: 0.1.0`.

## Deliberate Limits

First release stays manual so WinGet review feedback can be incorporated without maintaining automation. Add CI when releases become frequent. Add code signing when Windows SmartScreen materially blocks distribution.
