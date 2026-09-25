# ADR-010：Google Drive 为主 E2E，人工授权与后台执行分离

## 状态

Accepted（产品负责人于 2026-07-20 明确纠正并批准）

## 局部取代

第 2 条与“影响”中的 **macOS Keychain 持久 token store** 已由 [ADR-015](015-core-private-oauth-token-store.md)（2026-08-19）取代为 **Core-private persistent token store**。本 ADR 其余内容——Google Drive 为主 E2E、人工授权与后台执行分离、vault 不是密钥目录——继续有效。

## 背景

ADR-009 为尽快打通第一条正式数据链，把同机 local provider roundtrip 作为首个 vertical slice。这个切片能验证 Core 的扫描、加密、密文仓库、恢复和目标写入，但不能单独证明产品最初定义的 Google Drive 跨机器交接任务成立。

此前手工测试还把整个任务都要求为未锁屏状态，并把测试用 local ciphertext vault 简称为 `migration-vault` 目录，容易造成两个误解：后台数据执行依赖前台 UI；密文仓库可以与恢复因素放在同一“密钥目录”。

## 决策

1. v0.2 Mac 到 Mac 验收以 Google Drive backup/restore 为主路径；local provider 保留为离线恢复、故障诊断和独立可复现补充路径。两条路径都必须经过正式 Skill + CLI + Core task/journal，不得用测试 bridge 代替。
2. Google Drive 的 CLI grammar、OAuth/PKCE 合同或状态机存在，不等于 provider runtime 可用。只有 Core-private OAuth、macOS Keychain 持久 token store、Drive put/get/list/read-back 和完整 backup/restore 链全部通过，才可声明 Google Drive E2E ready。
3. native directory picker、OAuth browser callback、恢复因素输入及冲突确认等人工动作要求当前用户会话已解锁。动作完成后，扫描、加密、上传、下载、认证、解密、target commit 和 fresh verification 必须允许屏幕锁定后继续。
4. 运行中的不可中断数据阶段持有最小范围的 macOS 防空闲系统睡眠 assertion，并在完成、失败或取消时释放。产品不阻止用户主动锁屏，也不绕过登录窗口；锁屏期间需要新人工动作时，任务进入等待态，解锁后继续授权。
5. ciphertext vault 是加密对象仓库，不是密钥目录。Google Drive 路线由 Core 管理远端仓库；local provider 路线由用户选择真实持久目录。恢复短语和恢复 key 是 2-of-2 因素，必须与密文仓库分开保存，产品不得建议把二者共置于同一目录或同一云端位置。
6. `/private/tmp` 只允许作为隔离自动化夹具，不构成最终产品验收。正式 local-provider 手工验收使用用户明确选择的持久目录或外置介质，并保持 source、ciphertext vault、recovery-factor location 和 restore target 相互独立。

## 影响

- ADR-009 的 local roundtrip 仍有效，但不再是 Mac 到 Mac 完成声明的充分条件。
- Google Drive runtime 和 macOS Keychain token store 从“后续项”提升为 v0.2 Mac 到 Mac 主验收阻塞项。
- E2E 证据必须分别覆盖解锁时人工授权、锁屏后的后台传输/处理，以及需要再次授权时的安全等待。
- 任何证据、CLI 输出或日志仍不得包含 OAuth URL/token、账户、路径、文件名、恢复因素或明文内容。

## 2026-07-21 扩展说明

[ADR-011](011-dual-google-drive-layouts-and-portable-vault-bundle.md) 提议把 Google Drive 主验收拆成两种互不替代的数据布局：

- **Direct**：Core 直接在 Google Drive 管理 encrypted objects；用户在 backup 选择 source，在 restore 选择 target，不选择 local ciphertext vault。
- **Portable**：Core 先在用户选择的 local ciphertext vault 完成 backup，再封装为一个 Core-owned portable bundle 上传 Google Drive；fresh restore 必须重新下载、验证、展开并恢复。

该扩展不改变本 ADR 的 OAuth、锁屏、恢复因素分离和 Core-private provider I/O 决策。Portable provider、CLI argv 与容器格式在修订验收案例明确批准前仍是 proposed，不属于当前已实现合同。
