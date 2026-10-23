# Releasing and automatic updates

Automatic updates use Tauri's signed updater packages and GitHub Releases. The app checks quietly at startup and also exposes a manual **Check for updates** button. It only downloads and installs after the user confirms.

## Signing configuration

The updater key pair is already configured for `kogamitora/momotag`:

- The public key is in `src-tauri/tauri.conf.json` and the `TAURI_UPDATER_PUBLIC_KEY` repository secret.
- The private key and its password are stored in the `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` repository secrets.

GitHub Actions Secrets are for CI use, not a recoverable backup: secret values cannot be read back. Keep an owner-controlled, encrypted offline backup of the private key and its password from the moment the key pair is generated. Never commit the private key or print it in CI logs. If no backup exists yet, rotate the key and create the backup before the first public release; after clients receive updates signed by a key, rotation needs a migration plan because existing clients reject packages signed by a different key.

The workflow needs GitHub Releases enabled and the repository secrets above. Do not regenerate the key as part of routine releases.

## Publish

Update the app version consistently in `package.json`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml`, commit the release, then push a `v*` tag matching that version (for example `v1.0.0`). The workflow builds Windows, macOS, and Linux artifacts, signs updater bundles, and creates a draft GitHub Release with updater metadata. Review and publish that draft after testing the artifacts.

Never reuse a version number: clients compare semantic versions and will not install an update whose version is not newer.
