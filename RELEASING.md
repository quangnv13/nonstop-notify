# Releasing Nonstop Notify to WinGet

Fixed package identity:

- GitHub repository: `quangnv13/nonstop-notify`
- WinGet package ID: `quangnv13.nonstop-notify`
- Windows artifact: x64 portable executable

## 1. Install release tools

```powershell
winget install --id Microsoft.WingetCreate --exact --accept-package-agreements --accept-source-agreements
cargo install tauri-cli --version '^2.0.0' --locked
```

Open a new terminal if newly installed commands are not yet visible on `PATH`.

Authenticate once:

```powershell
wingetcreate token --store
```

Use the OAuth flow from `wingetcreate token --store`. Do not place credentials in shared command history.

## 2. Complete pre-release checks

```powershell
node scripts/check-licenses.mjs
npm.cmd --prefix ui-react run build
cargo test --locked
```

## 3. Set release version

Keep `[package].version` in `Cargo.toml` equal to `version` in `tauri.conf.json`. The next release is `0.1.2` with Git tag `v0.1.2`.

## 4. Build and test the portable Windows binary

```powershell
cargo tauri build --no-bundle
./scripts/smoke-test-windows-portable.ps1 -BinaryPath target/release/nonstop-notify.exe

$binary = Get-Item target/release/nonstop-notify.exe
Get-FileHash $binary.FullName -Algorithm SHA256
```

The smoke test runs `nonstop-notify.exe --self-check` from a different working directory. The release asset is the raw `target/release/nonstop-notify.exe`; no installer or sidecar file is required for the default sound.

## 5. Create the GitHub Release

Commit and push release source first. Then create and push the tag:

```powershell
$version = '0.1.2'
git tag -a "v$version" -m "Nonstop Notify v$version"
git push origin "v$version"
```

The `Build` GitHub Actions workflow tests and packages the portable Windows executable plus Linux DEB and AppImage artifacts. After all matrix jobs pass, its tag-only release job creates the GitHub Release and uploads those artifacts from that exact workflow run.

Open the completed Actions run, confirm all jobs passed, then copy the direct HTTPS URL of the uploaded Windows `.exe` asset. The URL must remain immutable for WinGet validation.

## 6. Create and submit the WinGet manifest

```powershell
$artifactUrl = 'https://github.com/quangnv13/nonstop-notify/releases/download/v0.1.2/nonstop-notify.exe'
wingetcreate new --out .winget $artifactUrl
```

Use these metadata values when prompted or when reviewing the generated YAML:

| Field | Value |
| --- | --- |
| PackageIdentifier | `quangnv13.nonstop-notify` |
| PackageVersion | `0.1.2` |
| PackageLocale | `en-US` |
| Publisher | `quangnv13` |
| PackageName | `Nonstop Notify` |
| License | `MIT` |
| ShortDescription | `Desktop notification bridge for scripts, automation, and background services.` |
| PackageUrl | `https://github.com/quangnv13/nonstop-notify` |
| PublisherUrl | `https://github.com/quangnv13` |
| ReleaseNotesUrl | `https://github.com/quangnv13/nonstop-notify/releases/tag/v0.1.2` |
| InstallerType | `portable` |
| Commands | `nonstop-notify` |

The generated installer manifest must use `InstallerType: portable`, the SHA256 of the raw `.exe`, and `Commands: [nonstop-notify]`. Keep `.winget/` only for inspection; generated manifests belong in `microsoft/winget-pkgs`, not this repository.

Validate the manifest, then submit the pull request:

```powershell
winget validate --manifest .winget
wingetcreate submit --prtitle "Add quangnv13.nonstop-notify version 0.1.2" .winget
```

## 7. Verify after merge

```powershell
winget source update
winget show --id quangnv13.nonstop-notify --exact
winget install --id quangnv13.nonstop-notify --exact
```

Close existing terminal windows, open a new terminal, then verify the command:

```powershell
nonstop-notify --self-check
```

For later releases, publish the new raw executable and update the package with:

```powershell
wingetcreate update quangnv13.nonstop-notify --version NEW_VERSION --urls NEW_EXECUTABLE_URL
```
