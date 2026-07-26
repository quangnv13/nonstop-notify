# SignPath Foundation Free Code Signing Checklist

Audit date: **July 26, 2026**

Project: [`quangnv13/nonstop-notify`](https://github.com/quangnv13/nonstop-notify)

This document tracks readiness for the SignPath Foundation free code-signing program. It is an engineering checklist, not confirmation that SignPath Foundation will accept the application. Acceptance remains discretionary and includes a project reputation review.

## Status Legend

- **PASS**: Repository evidence currently satisfies the condition.
- **PARTIAL**: Some evidence exists, but more work or confirmation is required.
- **FAIL**: A required condition is currently missing.
- **UNKNOWN**: Cannot be verified from the public repository.
- **POST-APPROVAL**: Configuration can only be completed after SignPath accepts the project.

## Current Summary

The project has a public MIT-licensed repository, current documentation, a code-signing policy, privacy disclosure, reproducible GitHub-hosted builds, installer metadata, and successful Windows and Linux bundle jobs.

Main blockers before applying:

1. Confirm GitHub multi-factor authentication for every project member.
2. Continue building verifiable public project reputation; SignPath Foundation makes the final discretionary decision.

## Eligibility Checklist

| # | SignPath Foundation condition | Status | Repository evidence / required action |
| --- | --- | --- | --- |
| 1 | Project contains no malware or potentially unwanted program behavior. | **PASS** | Reviewed application purpose and source paths only implement local event ingestion, notification rendering, URL validation, and packaging. No malware or PUA behavior is known. |
| 2 | Every component uses an OSI-approved Open Source license without commercial dual licensing. | **PASS** | `node scripts/check-licenses.mjs` audited 459 Rust and 75 npm dependencies with 0 issues. CI runs the same consolidated license audit. |
| 3 | No proprietary code or proprietary bundled component. | **PASS** | The unlicensed audio asset was removed. `build.rs` now generates the bundled WAV deterministically from project source, and the consolidated dependency audit reports 0 issues. |
| 4 | Project is actively maintained. | **PASS** | Recent commits and successful GitHub Actions runs exist on `master`. |
| 5 | Project is already released in the form to be signed. | **PASS** | Public Release `v0.1.0` contains the NSIS `.exe` installer form intended for future signing, plus DEB and AppImage packages. |
| 6 | Functionality is documented on the project/download page. | **PASS** | `README.md` documents purpose, commands, event schema, configuration, installation, privacy, and release flow. |
| 7 | Team signs only projects it develops and maintains. | **PASS** | Repository and build scripts are owned and maintained by `quangnv13`. |
| 8 | Team signs only binaries built from its own source. | **PASS** | GitHub Actions builds the repository's Rust, React, and packaging source directly. No upstream binary is submitted for signing. |
| 9 | Software is not a hacking or security-circumvention tool. | **PASS** | Application receives JSON events and renders desktop notifications; it does not scan for or exploit vulnerabilities. |
| 10 | Software respects user privacy and security. | **PASS** | `README.md` states that events remain local, telemetry is absent, and network access occurs only when explicitly requested through notification actions. |
| 11 | System changes are announced. | **PASS** | Packaging uses a normal current-user NSIS installer. Installation behavior is documented in `RELEASING.md`. |
| 12 | Installed software provides uninstallation. | **PASS** | Tauri NSIS produces an uninstaller and `RELEASING.md` requires verification through Windows Installed Apps. |
| 13 | All team members use MFA for GitHub and SignPath. | **UNKNOWN** | Repository cannot prove account MFA. `quangnv13` must confirm GitHub 2FA before applying and enable SignPath MFA when invited. |
| 14 | Authors, reviewers, and signing approvers are assigned. | **PASS** | `README.md` lists `quangnv13` as committer/reviewer and approver. |
| 15 | Home page contains a **Code Signing Policy** heading or link. | **PASS** | `README.md` has a `Code Signing Policy` section and the exact SignPath attribution statement. |
| 16 | Policy identifies roles and includes a privacy policy or required privacy statement. | **PASS** | `README.md` names the roles and contains the required no-network-transfer statement with the user-request exception. |
| 17 | Download/release pages reference the code-signing policy. | **PASS** | Release `v0.1.0` begins with a link to the README `Code Signing Policy`; the workflow adds the same link to later release notes. |
| 18 | Product name and version metadata are consistent in signed files. | **PASS** | Tauri product name is `Nonstop Notify`; `Cargo.toml` and `tauri.conf.json` both use version `0.1.1`. SignPath artifact restrictions still need configuration after approval. |
| 19 | Binary artifact is built from source in a verifiable automated build. | **PASS** | GitHub-hosted Windows and Linux jobs plus the release job passed in workflow run `30209450143`. |
| 20 | Unsigned artifact is stored as a GitHub workflow artifact before signing submission. | **PASS** | Build matrix uses `actions/upload-artifact@v4` with a step `id`, exposing `artifact-id` before any future SignPath submission. The future submit action can consume that output after SignPath configuration exists. |
| 21 | Every release receives manual approval before signing. | **POST-APPROVAL** | Configure SignPath signing policy and approver after acceptance. Current workflow performs no signing request. |
| 22 | SignPath GitHub App, project, artifact configuration, and submitter token are configured. | **POST-APPROVAL** | No SignPath GitHub action or `.signpath` policy exists yet. Add these only after organization/project identifiers are issued. |
| 23 | Project accepts SignPath technical constraints and assists with policy investigations. | **PARTIAL** | This checklist records the obligation. It becomes operational when the project applies and accepts the service terms. |
| 24 | Project has sufficient verifiable reputation. | **PARTIAL** | The repository now has public releases and successful public builds, but remains new. SignPath Foundation makes the final discretionary reputation decision. |

## Why `Create GitHub Release` Was Skipped

The successful workflow run was triggered by a push to `master`. Its Git reference was a branch reference such as:

```text
refs/heads/master
```

The release job has this condition:

```yaml
if: startsWith(github.ref, 'refs/tags/v')
```

The condition is false for a normal branch push, so GitHub correctly marks `Create GitHub Release` as **skipped**. The Windows and Linux build jobs can still complete successfully and upload temporary workflow artifacts.

Do not remove this condition. Creating a GitHub Release for every commit to `master` would publish unstable builds as releases.

## Initial Release Evidence

The first public release was created from an annotated version tag:

```powershell
git status --short
git tag -a v0.1.0 -m "Nonstop Notify v0.1.0"
git push origin v0.1.0
```

The tag push matched `v*`, causing the workflow to:

1. Build and test Windows and Linux packages again from the tagged commit.
2. Upload NSIS, DEB, and AppImage artifacts.
3. Run `Create GitHub Release` after both matrix jobs pass.
4. Attach the built artifacts to GitHub Release `v0.1.0`.

Before applying to SignPath, verify:

- Release is public and not a draft.
- Windows release contains the versioned NSIS `.exe`.
- Installer downloads and uninstalls correctly.
- Workflow-generated release notes contain the automatically prepended README `Code Signing Policy` link.
- Release artifact is the same installer format intended for future signing.

## SignPath Application Sequence

1. Publish the initial unsigned `v0.1.0` release. SignPath requires an existing release in the form to be signed.
2. Confirm GitHub MFA and clean dependency-license checks.
3. Submit the SignPath Foundation application with repository, workflow, release, privacy, and role links.
4. Wait for project and reputation review.
5. After acceptance, install the SignPath GitHub App and configure the SignPath project, signing policy, artifact configuration, and submitter token.
6. Update GitHub Actions so the unsigned Windows artifact is uploaded before `signpath/github-action-submit-signing-request@v2` submits it.
7. Require manual approval for every signing request.
8. Publish only the returned signed installer in later GitHub Releases and WinGet manifests.

## Official References

- SignPath Foundation application: https://signpath.org/apply.html
- SignPath Foundation terms and eligibility: https://signpath.org/terms.html
- SignPath trusted GitHub builds: https://docs.signpath.io/trusted-build-systems/github
- SignPath Foundation home: https://signpath.org/

## Next Audit

Re-run this checklist after SignPath responds to the application. Conditions 13, 21, 22, 23, and 24 require account confirmation, acceptance, or SignPath-side configuration.
