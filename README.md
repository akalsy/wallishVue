# Wallish

用每日壁纸学英语的 macOS 桌面应用。界面是 Vue 3，窗口、托盘和更换桌面壁纸用 Tauri 2。

每天一张 Bing 壁纸，配上一个英语单词。可以换成往日壁纸，也可以用自己的图片。单词卡片的位置、底色和字号可以在设置里调整。

## 功能

- 今日壁纸、往日壁纸、自定义壁纸
- 收藏壁纸，并随时换回已收藏的一张
- 查单词、看单词
- 关掉窗口后留在菜单栏，从托盘菜单退出

## 环境

- macOS
- Node.js 18 或更新版本
- Rust stable（https://rustup.rs）

## 启动

```bash
npm install
npm run tauri dev
```

开发窗口地址是 `http://127.0.0.1:5174`。改页面会自动刷新。改 `src-tauri/` 里的 Rust 代码后，Tauri 会重新编译。

## 打包

```bash
npm run tauri build
```

未签名的应用在 `src-tauri/target/release/bundle/macos/Wallish.app`。
