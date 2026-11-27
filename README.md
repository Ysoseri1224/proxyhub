<div align="center">

<img src="app/src-tauri/icons/128x128@2x.png" width="112" alt="ProxyHub" />

# ProxyHub

在一个窗口里管理镜像源、网络分流、代理节点和开发凭据的 macOS 应用

[![platform](https://img.shields.io/badge/platform-macOS%20arm64-000000?logo=apple&logoColor=white)](#安装)
[![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-2024-DEA584?logo=rust&logoColor=black)](app/src-tauri)
[![Go](https://img.shields.io/badge/Go-1.25-00ADD8?logo=go&logoColor=white)](engine)
[![mihomo](https://img.shields.io/badge/mihomo-compatible-6E56CF)](https://github.com/MetaCubeX/mihomo)
[![release](https://img.shields.io/github/v/release/Ysoseri1224/proxyhub?sort=semver)](https://github.com/Ysoseri1224/proxyhub/releases)
[![build](https://img.shields.io/github/actions/workflow/status/Ysoseri1224/proxyhub/ci.yml?branch=main&label=build)](https://github.com/Ysoseri1224/proxyhub/actions/workflows/ci.yml)

**中文** · [English](README.en.md)

</div>

ProxyHub 是一个个人项目，为经常在不同网络环境之间切换的开发者而写：开发工具的镜像源、按应用和域名的分流规则、机场节点的质量核验、散落各处的 token 与账号，集中在同一个界面里管理。欢迎提建议、开 issue，或拿去二次开发。

## 功能

### 📦 镜像源

运行在本机的软件包下载代理。pip、npm、go 等工具把它当作软件源，它再从多个国内外镜像里挑一个可用且最快的去下载，用来解决网络环境变化时装依赖慢或失败的问题。

- 覆盖 pip、npm、cargo、go、brew、docker、conda、CRAN 八个生态
- 按健康检查择优，当前上游失败时自动换下一个；包文件落盘缓存
- 一键把各工具指向引擎或还原官方源，镜像和生态都能在界面里增删
- 实时诊断每次下载经过的上游、是否命中缓存

### 🧭 网络分流

Clash Verge 分流规则的图形化编辑器。它决定哪些应用和网站走代理、走哪个节点组，哪些直连，用来让国内服务保持直连、国外服务稳定走代理。

- 按应用（进程）、域名、服务集、IP 段、地区、端口编排去向，去向取自订阅里的策略组
- 从 Clash 导入现有规则，从浏览器书签批量提取域名
- 规则写入 Clash Verge 的 Merge 配置，写入前自动备份、写入后读回校验，支持一键回滚
- 诊断器：输入域名，查看它命中的规则和去向

### 🛰️ 节点核验

代理订阅的体检工具。机场在节点名里标注的地区、线路和 IP 类型都是宣传，这个模块逐个节点实测，用来判断哪些节点名副其实，哪些适合登录 ChatGPT 这类对 IP 敏感的服务。

- 测量入口与出口的 IP、ASN、地区、延迟和丢包
- 对照节点名里的宣传，给出「一致」「不一致」或「未独立核实」，并列出依据
- 评估出口纯净度（机房、住宅、被标记为代理），以及 ChatGPT、Claude、Gemini 在该出口的地区可用性
- 到入口的路由追踪；晚高峰定时采样，对比闲时与忙时

### 🔐 凭据账号

开发凭据与登录状态的保管和切换中心。散落在文本文件、环境变量和各工具配置里的 token、密码统一收进 macOS Keychain，用来随时查看、使用和切换开发账号。

- 凭据的导入、查看、注入终端和编辑
- 在线查询 token 的归属账号、权限和有效期
- GitHub 多账号静默切换，Chrome 多身份一键启动
- 汇总 gh、npm、docker、wrangler 等工具的登录状态

### ☁️ Cloudflare

自有域名的轻量管理台，用来处理日常的域名事务，主要是域名邮箱的转发，省去打开网页控制台的步骤。

- 域名列表，以及控制台各页面的快捷入口
- Email Routing 转发规则和目标地址的管理

## 快速开始

### 安装

从 [Releases](https://github.com/Ysoseri1224/proxyhub/releases) 下载 dmg，拖入「应用程序」。应用未经 Apple 公证，首次打开前在终端执行：

```bash
xattr -dr com.apple.quarantine /Applications/ProxyHub.app
```

运行环境为 Apple Silicon Mac。网络分流和节点核验需要安装 [Clash Verge Rev](https://github.com/clash-verge-rev/clash-verge-rev) 并导入订阅。

### 从源码构建

需要 Go 1.25、Rust stable 和 Node 22。

```bash
git clone https://github.com/Ysoseri1224/proxyhub.git && cd proxyhub
go -C engine build -o ../app/src-tauri/resources/proxyhub-engine .
cd app && npm ci && npm run tauri build
```

安装包生成在 `app/src-tauri/target/release/bundle/`。开发调试用 `npm run tauri dev`。

## 架构

```mermaid
flowchart LR
  UI["界面<br/>React + TypeScript"] --> Core["Tauri 后端<br/>Rust"]
  Tools["pip · npm · go …"] --> Engine
  Core --> Engine["镜像引擎 · Go<br/>127.0.0.1:8899"]
  Engine --> Up[("各生态上游")]
  Core --> Merge["Clash Verge<br/>Merge 配置"]
  Core --> Temp["临时 mihomo<br/>逐节点测量"]
  Core --> KC[("macOS Keychain")]
  Core --> CF["Cloudflare API"]
  LA["launchd<br/>定时采样"] --> Temp
```

## 设计约定

- 凭据只保存在 macOS Keychain，配置文件里记录的是引用。
- 完整的代理面板由 Clash Verge 提供，ProxyHub 负责规则编排，需要时直接唤起 Clash Verge。
- 每个操作的结果和错误都如实显示在界面上。
- 节点核验在独立的临时内核里进行，测量期间 Clash Verge 的当前连接保持原样。
- 跳数、ASN、延迟属于观测线索。线路等级这类需要运营商资料才能确认的宣传，统一标注为「未独立核实」。

## 命令行

```bash
APP=/Applications/ProxyHub.app/Contents/MacOS/app
$APP --verify-report [订阅 uid]   # 核验订阅，输出 JSON 报告
$APP --verify-sample              # 对所有订阅采样一次（launchd 定时调用）
```

镜像引擎也可以单独接入，各生态的路径前缀与名称相同（`/pypi/` 对应 pip）：

```bash
pip config set global.index-url http://127.0.0.1:8899/pypi/
npm config set registry http://127.0.0.1:8899/npm/
go env -w GOPROXY=http://127.0.0.1:8899/go/
```

控制接口：`/_ph/status` 上游健康与缓存占用，`/_ph/diagnose` 最近的路由记录，`/_ph/cache/clear` 清空缓存。

## 使用须知

- 按应用分流依赖进程信息，需要在 Clash Verge 中开启 TUN 模式。
- IP 归属与纯净度来自 ip-api、ipinfo 等公开数据源，结论属于启发式判断。
- 读取 Safari 书签需要在「隐私与安全性 › 完全磁盘访问权限」中授权 ProxyHub。
- Clash Verge 有时会重写 Merge 配置，「网络分流」页的生效状态卡会提示，点「重新下发」即可恢复。
- 配置与数据位于 `~/.config/proxyhub/`，镜像缓存位于 `~/.cache/proxyhub/`。

## 文档

- [SPEC.md](SPEC.md) · 功能规格
- [DESIGN.md](DESIGN.md) · 设计说明

## 许可

[MIT](LICENSE)
