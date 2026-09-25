# ADR-011：Google Drive 双数据布局与 Portable Vault Bundle

## 状态

Accepted（产品负责人于 2026-07-21 明确批准）

## 背景

ADR-010 已确定 Google Drive 是 Mac 到 Mac 的主 E2E 交接介质，local provider 是离线与开发补充路径。产品语义需要进一步拆清：

- Google Drive 直接对象布局不要求用户选择本地 ciphertext vault；用户只选择现有 source，恢复时选择 target。
- `vault` 是密文仓库，不是密钥目录。恢复短语与恢复密钥必须与密文仓库分开。
- 除直接对象布局外，还需要验证一种可搬运的单文件布局：先生成完整 local ciphertext vault，再把该密文闭包封装成一个文件上传 Google Drive；目标端下载、展开并通过既有 local restore 链恢复。

第二条路径不是为了提高压缩率。认证密文通常不可有效压缩；它的价值是把多对象密文仓库变成一个可下载、复制、校验和归档的 portable artifact。

## 决策

### 1. 两条 Google Drive 产品路线

v0.2 必须分别实现和验证：

1. `google-drive`：Core 把 canonical encrypted objects、manifests、shards 与 commits 直接写入 Google Drive，并从 Google Drive 直接读取恢复。
2. `google-drive-portable`：Core 先在用户批准的空 local ciphertext vault 中完成 backup 与 fresh-reopen 校验，再生成一个 portable vault bundle，作为单一对象上传 Google Drive。恢复端只能从重新下载的 bundle 构造 fresh local ciphertext vault，不得复用源端 live handle、原 bundle、原 local vault 或进程内 authority。

现有 direct argv 保持不变；portable 路线拟新增精确 closed variants：

```text
migration-vault backup create --agent --provider google-drive-portable --format json
migration-vault restore create --agent --provider google-drive-portable --format json
```

JSON provider 固定为 `google_drive_portable`。批准后由 grammar/Schema/状态机与 runtime 同步实现；任何一层尚未闭合时 readiness 必须保持 false。

### 2. 用户看到的目录与资产

| 路线 | Backup 由用户选择 | Restore 由用户选择 | 用户不需要选择 |
|---|---|---|---|
| Google Drive direct | source | target；两份恢复因素 | local vault、bundle staging |
| Google Drive portable | source、空 local ciphertext vault | target；两份恢复因素 | 下载目录、展开目录、bundle staging |
| Local offline | source、空 local ciphertext vault | 已有 local ciphertext vault、target；两份恢复因素 | Google Drive、bundle staging |

恢复因素是两个独立 secret 文件或等价安全载体，不构成第四种业务目录。测试可以把两个文件放在一个隔离 fixture parent 中，但 production 指引不得建议把两个因素与 ciphertext vault 或 portable bundle 存在同一位置。

### 3. Portable Vault Bundle 安全边界

portable bundle 是版本化 Core-owned 密文容器，暂定扩展名 `.mvb`。它必须：

- 只包含已经完成并 fresh-reopen 验证的 local ciphertext vault 闭包；不得包含 source 明文、明文路径/文件名、恢复短语、恢复 key、OAuth token 或 Keychain material；
- 只允许 canonical vault-relative entry class；拒绝绝对路径、`..`、`.`、空 component、非 NFC、重复 entry、symlink、hardlink、device、FIFO、socket 与未知 entry type；
- 具有 closed magic/version、entry count、每 entry 长度与 ciphertext digest、总长度和完整 bundle digest；所有整数、累计长度和分配都受上限约束；
- 以 streaming 方式创建、上传、下载与展开，不要求把完整 vault 或 bundle 读入内存；
- 生成到 Core-private `0700` parent 与 `0600` 临时文件，完成 fsync、最终 digest 与 Google read-back 校验后才能记录 provider commit；
- 下载到 fresh Core-private staging，先验证 framing/digest/resource limits，再安全展开；展开结果还必须通过 local vault namespace、manifest graph、object authentication 与 2-of-2 解锁；
- 不承诺密文压缩率。压缩层只能是有界、版本固定、可流式验证的封装细节；压缩收益为零仍属于正常成功；
- 使用与 direct 路线相同的 Google OAuth/PKCE 与平台 credential store，但 Drive 中只发布一个 canonical bundle data object，不发布 direct-layout 的独立 vault objects。

### 4. 状态、失败与清理

- local vault 未完成或 fresh-reopen 验证失败：不得开始 bundle publication。
- bundle 生成中失败：不得出现 Google final object；临时文件必须清理或进入 Core-private quarantine。
- Google 上传/read-back 失败：local ciphertext vault 保持可独立恢复，task 不得报告完成。
- 下载、bundle framing、digest、展开或内层认证失败：target mutation count 必须为 0。
- 恢复成功后，下载 bundle 与展开 staging 必须清理；清理失败只能产生明确 warning/quarantine，不得把 staging 暴露为恢复结果。
- Core restart 继续遵守 ADR-008/ADR-010：所有 live root/session authority 失效；不得从未认证的临时 bundle 或展开目录透明续跑。

## 不采用的方案

### 把恢复因素一起打进 bundle

拒绝。取得一个 Google Drive 对象即同时取得密文与完整解密材料，会破坏 2-of-2 分离。

### 直接上传任意 zip/tar

拒绝。通用 archive 支持过多 entry type 与路径语义，容易引入 traversal、symlink/hardlink、重复路径、解压炸弹和跨平台规范化问题，也没有绑定当前 vault 的 closed format。

### 让 Agent 读取 local vault 后自行压缩上传

拒绝。Agent/CLI 不得取得 raw path、directory handle、ciphertext closure 或 OAuth token；bundle 生成与 provider I/O 必须留在 Core。

### 用 portable 路线取代 direct 路线

拒绝。direct 路线验证真实对象级 Google provider；portable 路线验证单文件交接与 local-vault 恢复。两者覆盖不同故障面，必须分别保留。

## 影响

- v0.2 Google Drive E2E 从一条扩展为两条强制路线。
- public CLI/provider closed set、contracts、status/report、task journal 与 readiness 新增 `google-drive-portable`，并按已批准验收案例执行 red → green → refactor → verify。
- local ciphertext vault 只在 local offline 与 Google portable backup 中成为用户选择项；Google direct 主流程不显示该选择。
- 双路线都必须在普通目录和用户授权敏感目录上验证，形成 2 × 2 的 E2E 矩阵。
