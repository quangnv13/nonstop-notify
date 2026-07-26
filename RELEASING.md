# Releasing Nonstop Notify to WinGet

Fixed package identity:

- GitHub repository: `quangnv13/nonstop-notify`
- WinGet package ID: `quangnv13.nonstop-notify`
- Installer: x64 NSIS, current-user scope

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

Use OAuth flow from `wingetcreate token --store`. Do not place credentials in shared command history.

## 2. Set release version

Keep `[package].version` in `Cargo.toml` equal to `version` in `tauri.conf.json`. First release uses `0.1.0` and Git tag `v0.1.0`.

## 3. Build installer

```powershell
npm --prefix ui-react run build
cargo tauri build --bundles nsis
```

```powershell
$installers = @(Get-ChildItem target/release/bundle/nsis -Filter '*.exe')
if ($installers.Count -ne 1) { throw "Expected one installer, found $($installers.Count)" }
$installer = $installers[0].FullName
Get-FileHash $installer -Algorithm SHA256
```

Test silent installation on a clean Windows user:

```powershell
Start-Process -FilePath $installer -ArgumentList '/S' -Wait
```

Confirm app appears under Windows installed apps and can be removed normally. NSIS uninstall also supports `/S` for unattended cleanup.

## 4. Create GitHub Release

Commit and push release source first. Then create and push tag:

```powershell
$version = '0.1.0'
git tag "v$version"
git push origin "v$version"
```

The `Build` GitHub Actions workflow tests and packages Windows and Linux. After both matrix jobs pass, its tag-only release job creates the GitHub Release and uploads NSIS, DEB, and AppImage artifacts from that exact workflow run.

Open the completed Actions run, confirm all jobs passed, then copy the direct HTTPS URL of the uploaded Windows `.exe` asset from the release page. URL must stay immutable for WinGet validation.

## 5. Create and submit WinGet manifest

```powershell
$installerUrl = 'https://github.com/quangnv13/nonstop-notify/releases/download/v0.1.0/INSTALLER_FILE.exe'
wingetcreate new --out .winget $installerUrl
```

Use these metadata values when prompted:

| Field | Value |
| --- | --- |
| PackageIdentifier | `quangnv13.nonstop-notify` |
| PackageVersion | `0.1.0` |
| PackageLocale | `en-US` |
| Publisher | `quangnv13` |
| PackageName | `Nonstop Notify` |
| License | `MIT` |
| ShortDescription | `Desktop notification bridge for scripts, automation, and background services.` |
| PackageUrl | `https://github.com/quangnv13/nonstop-notify` |
| PublisherUrl | `https://github.com/quangnv13` |
| ReleaseNotesUrl | `https://github.com/quangnv13/nonstop-notify/releases/tag/v0.1.0` |

`wingetcreate new` validates generated YAML and offers pull request submission after authentication. Keep `.winget/` only for inspection; generated manifests belong in `microsoft/winget-pkgs`, not this repository.

## 6. Verify after merge

```powershell
winget show --id quangnv13.nonstop-notify --exact
winget install --id quangnv13.nonstop-notify --exact
```

For later releases:

```powershell
wingetcreate update quangnv13.nonstop-notify --version NEW_VERSION --urls NEW_INSTALLER_URL
```
