# v0.3.0 WebDAV 发布验证

验证平台：Windows x64。使用隔离的合成记录与回环 WsgiDAV 4.4.0 / cheroot 11.1.2 服务，不涉及真实用户账号或剪贴板。

## 已完成

- `cargo test --locked --no-default-features --lib`：36 项通过，2 项显式忽略（性能探针、发布后 GitHub 包下载探针）。
- `cargo clippy --locked --all-targets --features ui-capture -- -D warnings`、格式检查与 Release 构建通过。
- 协议故障注入：并发首写、ETag 竞争重试、错误密码/账号、损坏密文、上传失败、删除后离线重连、独立标记合并、重复制、容量清理与加密配置重开通过。
- 首次上传并发回归：上传第一个内容块期间，主动删除或容量清理移除第二条记录；修复前复现远端缺失内容块 404，修复后只发布可读取的内容引用，两种情况均通过。
- 数据库并发回归：先持有后台同步事务，再提交剪贴板捕获；修复前复现 `database is locked`，共享写入串行化后捕获正常等待并保存。
- HTTP 测试夹具：Windows 接收套接字继承监听器的非阻塞模式，最初误把 WouldBlock 当成 EOF，造成 10053 断连。显式设置接收套接字阻塞后，原完整同步测试和重复目录请求稳定通过。
- 独立 WsgiDAV 探针：真实 Basic 认证、文本/HTML 多格式、PNG、文件路径引用、启动后台同步、20 次并发重复采集、真实 60 秒周期、关闭自动同步后手动推送通过。
- 实际远端检查：36 个索引/内容文件均为加密封套，未发现合成正文、格式名称或登录/同步密码明文。
- 原生 GPUI 表单：短同步密码拒绝、提交密码清空、真实 WebDAV 连接同步、自动开关、禁用自动后的手动同步通过。结果为 `native-screenshots/webdav-native/webdav-native-result.json`；已检查明暗主题连接成功截图。
- 旧版升级：复制 v0.2.0 合成数据库与 DPAPI 密钥后，以新版本打开并首次同步，原记录 ID、时间、标记、排序及格式指纹保持不变。
- 最终安装包：自定义安装、覆盖升级、卸载、快捷方式及卸载注册项清理通过，合成用户文件保留；包中 EXE 与当前 Release EXE 校验一致。

## 发行资产

| 文件 | SHA-256 |
| --- | --- |
| `x-clipboard-v0.3.0-windows-x64.zip` | `c3868257839ffe9551553d884a616e173ff0554029221c5e6869d3ec4fd6df78` |
| `x-clipboard-v0.3.0-windows-x64-setup.exe` | `8bbf5f8829cd6f47c913584bd28fea285b4d96d10d5f31a2805fcf778c17cb09` |

协议范围、端到端加密、容量清理、逻辑删除、远端密文保留和大小限制见 [WEBDAV.md](WEBDAV.md)。未声称所有商用 WebDAV、真实用户账号或 macOS/Linux 原生运行均已验收。发布后仍需核对公开 Release 资产和在线更新接口。
