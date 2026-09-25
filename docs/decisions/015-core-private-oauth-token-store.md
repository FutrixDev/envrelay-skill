# ADR-015：Google OAuth refresh token 改存 Core-private 文件，不再使用平台钥匙串

## 状态

Accepted

## 日期

2026-08-19

## 背景

Google Drive 路线的 refresh token 原本写入 macOS login Keychain（`SecItemAdd`/`SecItemCopyMatching`）。真实运行暴露出一个结构性问题，而不是一次偶发故障：

Keychain item 的 ACL 绑定的是**写入它的那个 binary 的 code identity**。开发期每次 `cargo build` 都会产出一个新的未签名可执行文件，因此每次重建后 Core 读取自己写入的 item 都会触发一次系统 ACL 弹窗（“允许 / 始终允许”）。Core 是 `gui/<uid>` 下的后台 service，弹窗出现时没有前台上下文可以引导用户；`SecItemCopyMatching` 就地阻塞，task 卡在 `waiting_for_local_action` 且没有任何可推进的 action。[v0.2 实现状态](../implementation-status-v0.2.md) 曾长期把“等待用户对新构建 binary 点击 Keychain 始终允许”记为 Direct 路线的当前阻塞项。

可选的规避手段都不成立：稳定的 designated requirement 需要 Apple Developer ID 签名与 notarization，而 [ADR-009](009-open-source-assurance-and-local-e2e.md) 已明确签名/公证不阻塞同机产品 E2E，反过来也就不能被当作 E2E 的前置；`kSecAttrAccessible` 与 ACL 是两个正交维度，放宽前者不消除后者的弹窗；`SecTrustedApplication` 系列 API 已废弃。

于是要回答一个更基本的问题：**这个 token 值不值得放进钥匙串。**

它不值得。EnvRelay 的密钥模型是 2-of-2（[ADR-012](012-user-phrase-and-qr-recovery-factors.md)）：master key 由用户短语与恢复二维码共同解封，两个因素都只在用户手上，Core 不留任何持久副本。因此 refresh token 不是 vault 密钥，它只是一个**授权凭据**——它能代表用户访问自己 Drive 上的 `drive.file` 对象，但那些对象全部是密文，没有两个恢复因素就解不开。把它放进钥匙串并不多保护任何明文，只是为每次重启和每次重建增加一次 ACL 弹窗。

同类桌面工具的做法一致：`gcloud` 与 `rclone` 都把 refresh token 存为用户目录下的 0600 普通文件。

## 决策

### 1. refresh token 存 Core-private 文件

后端换成 [`crates/migration-core-service/src/credential.rs`](../../crates/migration-core-service/src/credential.rs) 的 `GoogleRefreshTokenStore`，写入既有的 Core-private state root：

```text
/private/var/tmp/dev.envrelay.v02.<uid>/core-v1/google-refresh-v1.<sha256(client_id) 十六进制>
```

安全姿态与 `TaskRegistry` 的 `journal.key` 完全一致，复用同一套 `private_fs` 原语：

- 目录 0700、文件 0600，全部经 `O_NOFOLLOW` 打开，逐级校验 uid 与 `FileIdentity`；
- 只接受单链接（`nlink == 1`）普通文件，symlink、目录、hardlink、非本人所有或权限漂移一律 fail closed；
- 上限 4 KiB；超限即拒绝，不截断；
- 写入是 `<name>.new` → `write_all` → `sync_all` → `renameat` 原位替换 → 目录 `sync_all`，随后读回并用 `subtle::ConstantTimeEq` 比对，读回缓冲 zeroize。

文件名按 client_id 的 SHA-256 派生，因此不同 OAuth client profile 互不覆盖，同时文件名本身不泄露 client_id。

### 2. 不再链接任何会弹窗的凭据 API

`security-framework`、`security-framework-sys`、`core-foundation` 从 workspace 与 `migration-core-service` 移除。`credential.rs` 带一条源码自省测试，断言该模块不出现 `security_framework` / `SecKeychain` / `SecItem`，防止后续 patch 悄悄把钥匙串加回来。

Run 被接受之后不得重新打开 token store：数据面只使用已签发的 access token，凭据过期就 fail closed 并要求一次新的本地授权动作，而不是在后台去够任何交互面。

### 3. `token_store` wire 值更名

`system-probe/v1`、`system-probe/v2` 与 `release-support/v1` 的 macOS `token_store` 由 `macos_keychain` 改为 `core_private_file`。probe 的职责是如实报告实际能力；后端已经不是钥匙串，继续报 `macos_keychain` 就是在合同层撒谎。

Windows 的 `windows_credential_manager` 与 Linux 的 `linux_secret_service` 保持原值不变——那两个平台目前是 `"status": "unsupported"`，尚未实现，等实现时再按同一判据决定。

`token_store != unavailable` 仍是 Google Drive 任务的 readiness 前提；local provider 依旧不要求 token store。

### 4. 这不是 Tier C 改名

按 [ADR-014](014-envrelay-rename-and-frozen-serialization-namespace.md) 的判据，`token_store` 的取值是 probe capability 报告，不参与密钥派生、容器 magic 或已冻结的密文格式。它出现在 closed schema 中，因此需要同步 schema、正负例 fixture 与 `docs/release-support-v0.1.json`，但不需要 grammar 升版，也不影响任何已有 vault 的解密。

## 已知后果

### A. token 落在 Core-private 文件系统，而不是钥匙串

具有该用户 uid 的进程可以读到这个文件。这一点没有被隐藏：同一 uid 下的进程原本也能通过已授权的钥匙串 ACL 拿到同一个 token，而 Core-private state root 里的 `journal.key` 早已是同样的信任级别。真正的边界在别处——vault 明文由两个用户自持因素守卫，token 拿不到明文。

### B. 状态 root 位于 `/private/var/tmp`

该路径由 macOS 按启动周期清理。清理只会导致下一次 Google Drive 任务需要重新走一次 OAuth，不会丢失任何已上传的密文或恢复能力。

### C. ADR-010 的对应表述被局部取代

[ADR-010](010-google-drive-primary-e2e-and-session-power.md) 把 "macOS Keychain token store" 列为 v0.2 主验收阻塞项。该条被本 ADR 取代为“Core-private persistent token store”。ADR-010 的其余内容（Google Drive 为主 E2E、人工授权、锁屏后执行边界）继续有效。

[2026-07-22 macOS Keychain 零弹窗计划](../plans/2026-07-22-macos-keychain-zero-prompt.md) 整体作废：它要解决的问题已经不存在。
