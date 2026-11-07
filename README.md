<p align="center">
  <img src="packaging/android/res/drawable-nodpi/launcher_icon.png" width="128" height="128" alt="Horizon Android Icon" />
</p>

# <p align="center">Horizon Android</p>

<p align="center">
  <a href="https://github.com/GoyDevv/Horizon-Android/releases"><img src="https://img.shields.io/badge/Release-v1.0.0-7C4DFF?style=for-the-badge&logo=android&logoColor=white" alt="Release v1.0.0" /></a>
  <a href="https://discord.gg/CjyWBu2mC7"><img src="https://img.shields.io/badge/Discord-Community-5865F2?style=for-the-badge&logo=discord&logoColor=white" alt="Discord" /></a>
  <a href="https://github.com/GoyDevv/Horizon-Android"><img src="https://img.shields.io/badge/Minecraft%20Java-26.3-5D9C3F?style=for-the-badge&logo=curseforge&logoColor=white" alt="Minecraft Java 26.3" /></a>
  <a href="https://github.com/GoyDevv/Horizon-Android"><img src="https://img.shields.io/badge/Rust-2024%20(wgpu)-B7410E?style=for-the-badge&logo=rust&logoColor=white" alt="Rust 2024" /></a>
  <a href="https://github.com/GoyDevv/Horizon-Android"><img src="https://img.shields.io/badge/Android-API%2028--35-3DDC84?style=for-the-badge&logo=android&logoColor=white" alt="Android 9.0+" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-4527A0?style=for-the-badge" alt="License: MIT" /></a>
</p>

A high-performance native Android port of **Horizon**, rewriting Minecraft: Java Edition 26.3 in pure Rust with `wgpu` (Vulkan & OpenGL ES).

**Android 9+ (API 28–35) · Vulkan 1.0+ / GLES 3.0 · Full SAF Read & Write · In-App GitHub Updater · Material 3 Launcher · 260 FPS Uncapped · Java Multiplayer**

[Download Latest Release (v1.0.0)](https://github.com/GoyDevv/Horizon-Android/releases) · [Discord](https://discord.gg/CjyWBu2mC7) · [Build Guide](docs/android.md) · [Contributing](CONTRIBUTING_ANDROID.md) · [Report Bug](https://github.com/GoyDevv/Horizon-Android/issues)

---

## What's New in v1.0.0

- 🚀 **Full Read/Write Storage Access Framework (SAF)**: Full read, write, copy, move, rename, and directory deletion support across external storage and file managers.
- 📁 **Integrated Mini File Manager**: Inside the launcher's **Installed** tab, manage your files directly. Includes **Import Folder** (recursive tree import) and **Export Folder** (compresses to zip for easy backup and export).
- 🔄 **In-App GitHub Auto-Updater**: Instant GitHub Releases detection on startup, background download with animated progress bar, full changelog viewer, and automatic one-tap `.apk` installation.
- ⚡ **Performance & Frame Uncapping**: Uncapped framerates up to 260 FPS, Battery Saver mode (60 FPS VSync), PowerVR Rogue 32-bit GL compatibility fallback, Fast Chunk Upload pipeline, and dynamic resolution scaling (25%–100%).
- 🎮 **Next-Gen Mobile Touch Controls**: 4 control styles including MCPE Bedrock touch scheme, dynamic floating joystick, floating Horizon H menu bubble, custom on-screen button editor with opacity and sizing.
- 🌐 **Java Multiplayer**: Direct connection to Minecraft Java Edition servers and LAN worlds with online/offline mode support.

---

## Visual Showcase

| **Joystick and Crosshair** | **D-pad and Touch Aiming** |
| :---: | :---: |
| ![Fox in a forest with joystick controls](docs/media/android/joystick-fox.png) | ![River and forest with D-pad controls](docs/media/android/dpad-river.png) |

| **Snowy Biome** | **Hypixel Arcade · Java Multiplayer** |
| :---: | :---: |
| ![Snowy landscape on Android](docs/media/android/snow-landscape.png) | ![Hypixel Arcade lobby on Android](docs/media/android/hypixel-arcade.png) |

| **Third-Person View** | **Desert Panorama** |
| :---: | :---: |
| ![Steve on Android with third-person camera](docs/media/android/third-person-desert.png) | ![Desert terrain on Android with touch controls](docs/media/android/desert-panorama.png) |

---

## Key Features

### 🎮 Custom Mobile Controls
- **Four Control Styles**: Dynamic Joystick, Fixed D-pad, MCPE Bedrock controls, and Virtual Pointer.
- **On-Screen Layout Editor**: Freely reposition, resize, and adjust the opacity of every touch widget.
- **Quick-Action Horizon Menu**: Centered floating menu bubble with instant access to settings, inventory, and perspective toggles.
- **Hardware Input**: Full support for physical keyboards, mice, Bluetooth controllers, and USB gamepads.

### ⚡ Engine & Rendering
- **Vulkan & GLES Backends**: Optimized Vulkan renderer with fallback to OpenGL ES for legacy 32-bit and PowerVR devices.
- **Resolution Scaling**: Scale internal render resolution from 25% to 100% to maximize performance on any chipset.
- **Low-Latency Audio & Haptics**: High-fidelity sound powered by AAudio, with customizable haptic vibration feedback.
- **Cave & Entity Culling**: Aggressive GPU-driven occlusion culling to sustain smooth frame pacing in complex scenes.

### 📦 Content & File Management
- **SAF Document Provider**: Direct read & write access for third-party file managers (`content://com.goydevv.horizon.documents`).
- **One-Tap Folder Import**: Import custom worlds, resource packs, and shader packs directly from phone storage.
- **Zip Folder Export**: Compress and export any folder directly through Android's system share/file picker.
- **Sandboxed Modding**: Support for native Horizon `.hmod` sandboxed content packages.

---

## Building from Source

### Prerequisites
- **Android SDK** (API 35, Build-Tools 35.0.0+)
- **Android NDK** (28.2.13676358 / r28+)
- **Rust Toolchain** (1.90+ with `aarch64-linux-android` and `armv7-linux-androideabi` targets)
- **Java Development Kit** (JDK 17 or 21)
- **Python** (3.10+)

### Build APK
```sh
# Build ARM64 optimized APK
python3 tools/android_build.py --abi arm64-v8a --profile release

# Build ARMv7 32-bit APK
python3 tools/android_build.py --abi armeabi-v7a --profile release
```

The output APK will be generated under `packaging/android/build/outputs/apk/`.

For complete developer documentation, see [docs/android.md](docs/android.md).

---

## Contributing

Contributions, bug reports, and optimizations are welcome!
- Review [CONTRIBUTING_ANDROID.md](CONTRIBUTING_ANDROID.md) before opening a pull request.
- Run tests and self-checks:
  ```sh
  python3 -m unittest discover -s tests -p 'test_android*.py'
  python3 tools/check_public_tree.py
  ```

---

## License & Legal

- Code is licensed under the **[MIT License](LICENSE)**.
- Minecraft is a trademark of Mojang Synergies AB. This project is not affiliated with, endorsed by, or associated with Mojang or Microsoft.
- No proprietary game assets, jars, or binary resources are stored within this repository.
