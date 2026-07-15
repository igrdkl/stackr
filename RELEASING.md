# Releasing Stackr

Stackr auto-updates via Tauri's updater. Because the `stackr` source repo is
**private** and the in-app updater fetches its endpoint **unauthenticated**, the
release artifacts (installer + `latest.json`) are published to the **public**
[`stackr-manifest`](https://github.com/igrdkl/stackr-manifest) repo. The app
checks:

```
https://github.com/igrdkl/stackr-manifest/releases/latest/download/latest.json
```

and offers the update in **Settings → Updates**.

## One-time setup

The **public** signing key is committed in `src-tauri/tauri.conf.json`
(`plugins.updater.pubkey`). The **private** key was generated locally at:

```
%USERPROFILE%\.tauri\stackr-updater.key
```

It is **not** in the repo and must never be committed. Add these three secrets to
the **`stackr`** repo (Settings → Secrets and variables → Actions):

| Secret | Value |
|--------|-------|
| `TAURI_SIGNING_PRIVATE_KEY` | entire contents of `%USERPROFILE%\.tauri\stackr-updater.key` |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | the key's password (empty — it was generated without one) |
| `MANIFEST_RELEASE_TOKEN` | a Personal Access Token with **contents: write** on `stackr-manifest` |

> The PAT is needed because the workflow runs in `stackr` but publishes the
> release to a different repo; the default `GITHUB_TOKEN` can't write across repos.
> Use a fine-grained PAT scoped to `stackr-manifest` only.
>
> Keep a backup of the private key. If it's lost, existing installs can no longer
> verify updates and users must reinstall manually.

## Cutting a release

1. Bump the version in **three** files (keep them in sync):
   `package.json`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`.
2. Commit, then tag and push:
   ```bash
   git tag v0.3.0
   git push origin v0.3.0
   ```
3. The **release** workflow builds + signs the installer in `stackr`, generates
   `latest.json`, and publishes both to a **`stackr-manifest`** Release named for
   the tag. (It publishes directly — no draft — so the updater endpoint resolves
   immediately.)

Existing installs pick up the update on their next launch (or via
**Settings → Updates → Check for updates**).
