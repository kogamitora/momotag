# Releasing and automatic updates

Automatic updates use Tauri's signed updater packages and GitHub Releases. The app checks quietly at startup; users can also choose **Check for updates** from the system tray's right-click menu. It only downloads and installs after the user confirms.

## Signing configuration

The updater key pair is already configured for `kogamitora/momotag`:

- The public key is in `src-tauri/tauri.conf.json` and the `TAURI_UPDATER_PUBLIC_KEY` repository secret.
- The private key and its password are stored in the `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` repository secrets.

GitHub Actions Secrets are for CI use, not a recoverable backup: secret values cannot be read back. Keep an owner-controlled, encrypted offline backup of the private key and its password from the moment the key pair is generated. Never commit the private key or print it in CI logs. If no backup exists yet, rotate the key and create the backup before the first public release; after clients receive updates signed by a key, rotation needs a migration plan because existing clients reject packages signed by a different key.

The workflow needs GitHub Releases enabled and the repository secrets above. Do not regenerate the key as part of routine releases.

## Branching model

Use one long-lived branch: `main`. Keep it releasable and merge work through pull requests with passing CI.

- Create a short-lived `feature/<name>` branch from the latest `main` for each feature or related change. Open a PR back to `main`; review and CI must pass before merge.
- Do not use a long-lived `develop` or `release` branch. Merging code to `main` does not publish it.
- For a release, create a temporary `release/vX.Y.Z` branch from `main`. Freeze feature work on this branch; allow only version/release preparation and fixes required to ship that release.
- After validation, open a PR from the release branch to `main` and merge it. Create the version tag on the resulting verified `main` commit, then delete the temporary release branch.
- For urgent production fixes, create `hotfix/<name>` from `main`, merge it back through a PR, and publish a new patch version using the same tag process.

## Release procedure

1. Confirm all changes intended for this release are merged to `main` and CI is passing.
2. Create and push a temporary branch from the latest `main`, for example `release/v0.2.0`.
3. Update the app version consistently in `package.json`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml`. Make only release-related fixes on this branch and validate them.
4. Open a PR from `release/vX.Y.Z` to `main`. After review and CI pass, merge the PR.
5. On the resulting `main` commit, create and push the matching tag, for example `v0.2.0`. The tag must match the app version and must never be reused.
6. The workflow builds Windows, macOS, and Linux artifacts, signs updater bundles, and creates a draft GitHub Release with updater metadata. Test the artifacts, publish the draft, and delete the temporary release branch.

Never reuse a version number: clients compare semantic versions and will not install an update whose version is not newer.
