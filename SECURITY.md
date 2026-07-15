# Security Policy

## Supported versions

Stackr is a rolling release — only the latest version receives fixes. Update
in-app via **Settings → Updates**.

## Reporting a vulnerability

Please **do not** open a public issue for security problems. Report privately via:

- GitHub's [**Report a vulnerability**](https://github.com/igrdkl/stackr/security/advisories/new)
  (repo **Security → Advisories**), or
- email **igordikal1@gmail.com**.

Include the affected version, reproduction steps, and impact. You'll get an
acknowledgement within a few days.

## Threat model / scope

Stackr is a **local, single-user development tool for Windows**. By design it:

- binds every bundled service (web servers, databases, caches, mail) to
  **loopback only** (`127.0.0.1` / `::1`);
- runs a local certificate authority for optional HTTPS on `.test` / `.localhost`
  domains — the CA private key never leaves the machine and is trusted only in the
  **current user's** store;
- edits the Windows `hosts` file and can add a Windows Defender exclusion — both
  only through explicit, **UAC-elevated** actions the user initiates.

Development databases use a **passwordless local root** on purpose (loopback is the
boundary). Do **not** expose Stackr-managed services to a public network or use them
in production.
