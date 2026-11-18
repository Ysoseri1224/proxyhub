# ProxyHub

统一管理入口:**镜像源** + **网络分流**。只做管理与编排,不复刻现有软件;重叠处唤起对应软件。
规格见 [SPEC.md](./SPEC.md)。

## 目录
- `engine/` —— Go 镜像引擎(本地路由代理:多上游池 + 健康检查 + 故障转移 + 磁盘缓存)。✅ 可用
- `app/` —— Tauri app(Apple 原生风格,React + TS),四模块。✅ 可构建运行

## app · 运行
```bash
cd app && npm install && npm run tauri dev     # 开发运行(打开窗口)
# 或打包:npm run tauri build  → src-tauri/target/release/bundle/ 下的 .app/.dmg
```
四模块:**镜像源**(引擎开关/健康看板/缓存/实时诊断)· **网络分流**(规则图形编排/导入Clash/写回+回滚/出口IP检测)· **凭据账号**(Keychain 加密 + gh 多账号切换)· **Cloudflare**(控制台入口 + 域名邮箱 Email Routing)。
> 秘密只进 macOS Keychain,仓库零明文。Cloudflare/邮箱需先在"凭据账号"存入 `CLOUDFLARE_API_TOKEN`。

## 引擎 · 快速使用
```bash
cd engine && go build -o proxyhub . && ./proxyhub      # 默认监听 127.0.0.1:8899
```
把 dev 工具指向它(生态前缀:/pypi/ /npm/ /cargo/ /go/ /brew/ /docker/):
```bash
pip config set global.index-url http://127.0.0.1:8899/pypi/
pip config set global.trusted-host 127.0.0.1
npm config set registry http://127.0.0.1:8899/npm/
go env -w GOPROXY=http://127.0.0.1:8899/go/
```
控制接口(供 UI 调用):
- `GET /_ph/status` —— 各生态上游健康/延迟 + 缓存占用
- `GET /_ph/diagnose` —— 最近路由事件(分流诊断器数据源)
- `GET /_ph/cache/clear` —— 清缓存

配置文件:`~/.config/proxyhub/config.json`(首次运行自动生成)。缓存目录:`~/.cache/proxyhub`。

## 已完成(引擎 v0)
- [x] 6 生态多上游池(含 docker 独立分类)
- [x] 健康检查(并行探测,择优排序)
- [x] 故障转移(当前上游失败自动换下一个)
- [x] 磁盘缓存 + LRU 淘汰(默认上限 10G)
- [x] 分流诊断器数据源(/_ph/diagnose)
- [x] 控制接口(status / diagnose / cache clear)

## 待建
- [ ] 引擎:上游池 CRUD 的写接口;pypi simple 链接改写(让包文件也走缓存);launchd 常驻服务
- [ ] Tauri UI:镜像源管理面板 + 健康看板 + 缓存管理 + 分流诊断器可视化
- [ ] 分流模块:导入 Clash 配置、图形化编排"谁走哪条分流"(6 类)、写回 + diff 预览 + 版本化备份/回滚、唤起 Clash Verge、出口 IP 纯净度检测
