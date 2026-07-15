# Code Signing Policy

Stackr is signed so users can trust that a release came from this project and was
not tampered with. This page documents who may sign, how releases are built and
approved, and what the signed software does — as required by the
[SignPath Foundation](https://signpath.org/).

## Team & roles

Stackr is currently maintained by a single maintainer, **igrdkl**
([github.com/igrdkl](https://github.com/igrdkl)), who holds all of the roles below.
As the project grows and contributors join, these roles will be separated across
different people.

- **Authors** — write and modify source code and open pull requests.
- **Reviewers** — review and approve pull requests before they are merged.
- **Approvers** — authorize each signing request for a release.

Everyone with access to the source repository and to SignPath uses multi-factor
authentication (MFA) on both GitHub and SignPath.

## Build & release process

- Stackr is **built exclusively from the public source** at
  <https://github.com/igrdkl/stackr> — no locally produced or out-of-band artifact
  is ever signed.
- Releases are produced by GitHub Actions (`.github/workflows/release.yml`) when a
  `vX.Y.Z` tag is pushed; the workflow builds the Windows NSIS installer from the
  tagged commit.
- Every release is **manually approved** before signing — unattended or automatic
  signing is not used.
- File metadata (product name and version) is enforced on every signed artifact.

## Signatures

Stackr releases carry two independent signatures:

1. **Authenticode** — the Windows installer (`Stackr_x.y.z_x64-setup.exe`) is code
   signed with a certificate provided by the **SignPath Foundation** (private key
   held on SignPath's HSM). This identifies the publisher to Windows and SmartScreen.
2. **Update signature** — the in-app auto-updater verifies every update against an
   Ed25519 key whose public half is embedded in the app (`src-tauri/tauri.conf.json`).
   The Authenticode signature is applied **before** this update signature is computed.

> **Status:** Authenticode signing via the SignPath Foundation is being set up.
> Until it is live, installers carry only the Ed25519 update signature, and Windows
> may show an "unknown publisher" prompt on first launch. The update channel is
> already cryptographically verified.

## Privacy

Stackr runs entirely on the user's machine. It does **not** collect, transmit, or
sell any personal data or telemetry. The only outbound network requests are:

- downloading development components (PHP, web servers, databases, caches, mail)
  from their official upstream sources, at the user's request;
- fetching the component manifest and the application update manifest
  (`latest.json`) to offer downloads and updates.

There is no analytics or tracking. Stackr can be removed via Windows Settings →
Apps (or its bundled uninstaller); the data folder (`C:\Stackr` by default) can then
be deleted manually.

## Attribution

Free code signing is provided by [SignPath.io](https://signpath.io), with a
certificate issued by the [SignPath Foundation](https://signpath.org).

## Reporting

To report a security issue or a suspected malicious build, see
[SECURITY.md](SECURITY.md). Please do not open a public issue for security problems.
