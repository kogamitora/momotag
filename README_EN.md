# MomoTag

[中文](README.md) · [日本語](README_JA.md) · [English](README_EN.md)

MomoTag is a desktop music-tagging tool for local albums, including doujin CDs and digital releases. Choose an album folder, paste a tracklist, select a cover, and write album, artist, year, track, and cover metadata in one pass.

The interface supports Chinese, Japanese, and English. The default language follows the system language: Japanese systems use Japanese, Chinese systems use Chinese, and all other systems use English. You can also switch languages manually in the app.

## Features

- Scan audio files in an album folder and sort them naturally by filename.
- Parse track numbers, titles, and artists from pasted tracklists, including compact mixed-format text.
- Write album, album artist, year, title, track number, total tracks, and cover metadata in bulk.
- Edit target filenames, titles, and artists; file renames always preserve the original extension.
- Use JPG, PNG, or WEBP covers from a local path or an HTTP/HTTPS URL.
- Auto-detect common folder covers such as `cover.jpg`, `folder.jpg`, and `front.jpg`.
- Block submission while tracks or audio files are still unpaired.
- Create temporary backups before writing and attempt a rollback when a mutation fails.
- Optionally rename the album folder to the album title after writing.
- Check for updates quietly at startup or manually; after confirmation, the app downloads, installs, and restarts.

## Supported formats

Audio extensions scanned by the app:

`mp3`, `flac`, `wav`, `m4a`, `ogg`, `opus`, `aiff`, `aif`, `ape`, `wv`

Cover formats: `jpg`, `jpeg`, `png`, and `webp`.

## Usage

1. Choose an album folder containing the audio files.
2. Enter the album title, album artist, and year. Existing tags and common folder covers may be used as suggestions.
3. Choose a local cover or paste an HTTP/HTTPS image URL.
4. Paste the tracklist and review the parsed tracks and file matching.
5. Edit filenames, titles, or artists as needed, or exclude files from this write.
6. Resolve every unpaired track or file, then click **Write tags**.

The operation modifies local audio files. Test with a copy first. If another application holds a file open, folder renaming may be skipped and reported while completed tag changes remain visible.

## Development

Requirements:

- Windows 10/11 (the primary validation platform)
- [Bun](https://bun.sh/)
- Stable Rust, Cargo, and the system dependencies required by Tauri 2
- WebView2 and Visual Studio C++ build tools on Windows

Install dependencies and start the desktop app in development mode:

```powershell
bun install
bun run tauri dev
```

Run the frontend only:

```powershell
bun run dev
```

Useful checks and builds:

```powershell
bun run build
bun run tauri build --no-bundle
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

`bun run tauri build --no-bundle` compiles the application without creating an installer and is useful for fast local validation.

## Branching and collaboration

- `main` is the only long-lived integration branch and should remain releasable. Merge changes through pull requests.
- Create short-lived `feature/<name>` branches from the latest `main`. Open a PR back to `main`; merge after review and passing CI.
- Before a release, create a temporary `release/vX.Y.Z` branch from `main`. Use it only for version updates, release preparation, and release-blocking fixes—not new features.
- Merge the release branch into `main`, then create the `vX.Y.Z` tag on the verified `main` commit. Delete the temporary branch after release.
- Do not use a long-lived `develop` branch. Merging to `main` does not publish a release; only pushing a `v*` tag triggers the release workflow.

## Releases and automatic updates

The release workflow is `.github/workflows/release.yml`. Pushing a `v*` tag makes GitHub Actions build Windows, macOS, and Ubuntu artifacts and create a draft Release with Tauri updater metadata. See [RELEASING.md](RELEASING.md) for the complete branch and release procedure.

Read [RELEASING.md](RELEASING.md) before publishing. Keep the version consistent in `package.json`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml`. Never commit the updater private key.

## Documentation

- [发布说明（中文）](README.md)
- [日本語 README](README_JA.md)
- [Releasing and automatic updates](RELEASING.md)

## License

This project is licensed under the [MIT License](LICENSE). It allows personal and commercial use, modification, and redistribution, provided the copyright and license notice are included. Third-party dependencies remain subject to their respective licenses.
