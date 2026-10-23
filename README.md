# MomoTag

MomoTag 是一个桌面音乐标签整理工具，适合整理同人 CD、数字专辑等本地音乐文件。选择专辑文件夹、粘贴曲目表并选择封面后，可以一次性写入专辑名、艺术家、年份、曲名、曲目编号和封面。

界面支持中文、日语和 English。

## 功能

- 扫描专辑文件夹中的音频文件，并按文件名自然排序。
- 从曲目表中解析曲目编号、标题和艺术家，支持直接粘贴混合格式文本。
- 批量写入音频标签：专辑名、专辑艺术家、年份、曲名、曲目编号、曲目总数和封面。
- 可逐条编辑目标文件名、曲名和艺术家；重命名时保留原文件扩展名。
- 支持 JPG、PNG 和 WEBP 封面，可使用本地图片或 HTTP/HTTPS 图片地址。
- 自动识别文件夹中的常见封面文件，例如 `cover.jpg`、`folder.jpg` 和 `front.jpg`。
- 提交前检查未关联的曲目和音频文件，避免不完整写入。
- 写入前创建临时备份；发生错误时尝试回滚已完成的修改。
- 可选择在写入完成后将专辑文件夹重命名为专辑名。
- 启动时后台检查更新，也可以在应用内手动检查；确认后下载并安装更新并重启应用。

## 支持格式

当前会扫描以下音频扩展名：

`mp3`、`flac`、`wav`、`m4a`、`ogg`、`opus`、`aiff`、`aif`、`ape`、`wv`

封面支持：`jpg`、`jpeg`、`png`、`webp`。

## 使用流程

1. 点击“选择文件夹”，选择包含音频文件的专辑目录。
2. 填写专辑名、专辑艺术家和年份；已有标签或文件夹封面时，应用会尝试自动填充。
3. 选择本地封面，或粘贴一个 HTTP/HTTPS 图片地址。
4. 将曲目表粘贴到曲目表输入框，检查解析结果和文件匹配关系。
5. 需要时编辑文件名、标题、艺术家，或排除不参与本次写入的文件。
6. 确认没有未关联项目后，点击“写入标签”。

写入会修改本地音频文件。建议先使用副本测试；如果文件被播放器、资源管理器或其他程序占用，文件夹重命名可能会被跳过，但已完成的标签写入会显示结果。

## 开发

环境要求：

- Windows 10/11（当前主要验证平台）
- [Bun](https://bun.sh/)
- Rust stable、Cargo 和 Tauri 2 所需的系统依赖
- Windows 上需要 WebView2 和 Visual Studio C++ 构建工具

安装依赖并启动开发模式：

```powershell
bun install
bun run tauri dev
```

只验证前端：

```powershell
bun run dev
```

常用检查和构建命令：

```powershell
bun run build
bun run tauri build --no-bundle
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

`bun run tauri build --no-bundle` 会编译应用但不生成安装包，适合快速验证 Rust、前端和 Tauri 配置。

## 发布与自动更新

发布工作流位于 `.github/workflows/release.yml`。推送 `v*` 格式的 Git tag 后，GitHub Actions 会在 Windows、macOS 和 Ubuntu 上构建并创建草稿 Release，同时生成 Tauri updater 元数据。

发布前请阅读 [RELEASING.md](RELEASING.md)，并确保版本号在 `package.json`、`src-tauri/tauri.conf.json` 和 `src-tauri/Cargo.toml` 中保持一致。签名私钥只应保存在 GitHub Actions Secrets 中，不要提交到仓库。

## 项目文档

- [发布与自动更新](RELEASING.md)
- [日本語 README](README_JA.md)
- [English README](README_EN.md)

## 许可证

本项目采用 [MIT License](LICENSE)。该许可证允许个人和商业使用、修改及再分发，但分发时需保留版权声明和许可证文本。第三方依赖仍受其各自许可证约束。
