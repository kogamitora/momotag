# MomoTag

MomoTag は、同人 CD やデジタルアルバムなどのローカル音楽ファイルを整理するデスクトップタグ編集ツールです。アルバムフォルダを選び、トラックリストとカバー画像を指定すると、アルバム名、アーティスト、年、曲名、曲番号、カバーをまとめて書き込めます。

画面は中文、日本語、English に対応しています。

## 機能

- アルバムフォルダ内の音声ファイルをスキャンし、ファイル名順に並べ替えます。
- 番号、タイトル、アーティストが混在したトラックリストを解析します。
- アルバム名、アルバムアーティスト、年、タイトル、曲番号、曲数、カバーを一括で書き込みます。
- 変更後のファイル名、タイトル、アーティストを編集できます。ファイル名を変更しても拡張子は保持されます。
- JPG、PNG、WEBP のカバーに対応し、ローカル画像または HTTP/HTTPS URL を使用できます。
- `cover.jpg`、`folder.jpg`、`front.jpg` などの一般的なカバーを自動検出します。
- トラックまたは音声ファイルに未対応付けがある場合、書き込みを開始できません。
- 書き込み前に一時バックアップを作成し、エラー時は可能な範囲でロールバックします。
- 書き込み後にアルバムフォルダをアルバム名へ変更できます。
- 起動時に更新を確認し、手動確認にも対応します。確認後にダウンロード、インストール、再起動を行います。

## 対応形式

スキャン対象の音声拡張子：

`mp3`、`flac`、`wav`、`m4a`、`ogg`、`opus`、`aiff`、`aif`、`ape`、`wv`

カバー形式：`jpg`、`jpeg`、`png`、`webp`。

## 使い方

1. 音声ファイルが入ったアルバムフォルダを選択します。
2. アルバム名、アルバムアーティスト、年を入力します。既存タグやフォルダ内のカバーから候補を補完する場合があります。
3. ローカル画像を選択するか、HTTP/HTTPS の画像 URL を貼り付けます。
4. トラックリストを貼り付け、解析結果とファイルの対応を確認します。
5. 必要に応じてファイル名、タイトル、アーティストを編集し、対象外のファイルを除外します。
6. 未対応付けのトラックとファイルをすべて解決してから「タグを書き込む」を実行します。

この操作はローカルの音声ファイルを変更します。最初はコピーで確認してください。別のアプリがファイルを使用中の場合、フォルダ名の変更だけがスキップされ、完了したタグ変更は結果として表示されます。

## 開発

必要な環境：

- Windows 10/11（主な検証環境）
- [Bun](https://bun.sh/)
- Rust stable、Cargo、Tauri 2 に必要なシステム依存関係
- Windows では WebView2 と Visual Studio C++ ビルドツール

依存関係をインストールして開発モードを起動します。

```powershell
bun install
bun run tauri dev
```

フロントエンドだけを起動する場合：

```powershell
bun run dev
```

主な確認コマンド：

```powershell
bun run build
bun run tauri build --no-bundle
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

`bun run tauri build --no-bundle` はインストーラーを作成せずにアプリをコンパイルします。

## リリースと自動更新

リリースワークフローは `.github/workflows/release.yml` にあります。`v*` 形式の Git tag を push すると、GitHub Actions が Windows、macOS、Ubuntu 向けにビルドし、Tauri updater 用メタデータを含む draft Release を作成します。

公開前に [RELEASING.md](RELEASING.md) を確認してください。`package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml` のバージョンを一致させ、updater の秘密鍵は絶対にコミットしないでください。

## ドキュメント

- [中文 README](README.md)
- [English README](README_EN.md)
- [リリースと自動更新](RELEASING.md)

## ライセンス

本プロジェクトは [MIT License](LICENSE) のもとで公開されています。著作権表示とライセンス文を保持すれば、個人・商用利用、改変、再配布が可能です。サードパーティ依存関係には各々のライセンスが適用されます。
