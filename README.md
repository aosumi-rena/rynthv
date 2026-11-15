# RynthV & RinstX

<p align="center">
  <img src="assets/branding/app-icon.png" alt="RynthV 图标" width="96" height="96">
</p>

使用 Rust + egui 开发的歌声和乐器合成引擎。

## 构建与运行

需要 Rust 1.99.0，版本由 `rust-toolchain.toml` 指定。

### Windows

需要 MSVC、Windows SDK 和支持 DX12 的显卡。

```powershell
cargo run --release
```

### Linux

支持 X11 / Wayland，使用 Vulkan 或 OpenGL。Ubuntu 24.04 构建示例：

```sh
sudo apt install build-essential pkg-config libasound2-dev libwayland-dev libxkbcommon-dev \
  fontconfig fonts-noto-cjk libvulkan1 libegl1 libgl1 xdg-desktop-portal xdg-desktop-portal-gtk
cargo build --release --locked --bins
./target/release/rynthv
```

需要可用的图形驱动、ALSA 音频设备和桌面对应的 XDG 门户后端（KDE 使用 `xdg-desktop-portal-kde`）。

### macOS

需要 Xcode Command Line Tools 和支持 Metal 的 Mac。首次构建前安装命令行工具：

```sh
xcode-select --install
```

安装完成后，在项目目录构建并运行：

```sh
cargo build --release --locked --bins
./target/release/rynthv
```

## 资源与配置

首次使用请在设置中手动选择声库／乐器目录和发音词典目录，程序不会自动探测资源路径，也不附带声库、乐器音源或词典。

| 类型 | Windows | Linux | macOS |
| --- | --- | --- | --- |
| R1 STD | 支持 | 支持 | 支持 |
| R2 STD | 支持 | 支持 | 支持 |
| R2 AI | 支持 | 支持 | 支持 |
| R2 AI Flat | 支持 | 支持 | 支持 |
| R3 AI | 支持 | 支持 | 支持 |
| R3 AI 2 | 支持 | 支持 | 支持 |
| Inst | 支持 | 支持 | 支持 |

配置目录：
- Windows 为 `%LOCALAPPDATA%/RynthV`  
- Linux 为 `$XDG_CONFIG_HOME/RynthV`（未设置时为 `$HOME/.config/RynthV`）  
- macOS 为 `~/Library/Application Support/RynthV`

命令行合成与诊断工具的用法可通过 `cargo run --release --locked --bin rynthv-synth -- --help` 查看。
