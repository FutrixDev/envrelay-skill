# ADR-012：用户短语与二维码组成唯一的 2-of-2 恢复合同

## 状态

Accepted（产品负责人于 2026-07-21 明确确认并批准修订验收案例）

## 背景

项目最初目标 `mg.md` 已明确冻结恢复材料的职责与载体：

- 备份时由用户提供恢复短语；
- 软件独立生成长恢复密钥，并以二维码图片交付用户保存；
- 恢复时用户再次提供恢复短语与二维码图片；
- 两个因素缺一不可，丢失任一因素即永久不可恢复，不提供托管恢复或厂商后门。

当前 Core 已实现 `phrase_key || recovery_key` 的 2-of-2 KEK 组合，但 production local action 偏离了产品合同：软件自动生成七词短语，并要求把短语和 Bech32m 恢复密钥分别保存为两个文本文件；恢复也从两个文本文件读取。这不是目标产品流程，也不能作为正式 Mac 到 Mac E2E 的通过证据。

## 决策

### 1. 备份 create-new

1. 可信本地界面要求用户输入恢复短语，并再次输入确认；软件不得替用户自动生成短语，不得把短语导出成文件。
2. v1 的短语 canonical 规则暂时保持现有安全格式：恰好七个 EFF Long Wordlist 小写 ASCII 单词，以单个 ASCII 空格分隔。输入与确认必须在 canonical validation 后逐字节相同。
3. Core 使用 OS CSPRNG 独立生成 32-byte `recovery_key`，并按 `security-format-v1.md` 生成绑定当前 `vault_id` 的 62-byte canonical CBOR payload。
4. 本地界面把 canonical CBOR payload 直接编码为二维码并渲染为 PNG。用户通过 native save dialog 保存 PNG；正式流程不得要求另存 recovery-key 文本文件。
5. 保存后的 PNG 必须重新从磁盘读取、解码二维码、canonical parse，并验证 payload 与当前 vault 完全一致，才允许任务进入 Ready。取消、写入失败或回读失败均保持 provider mutation 为零。
6. 恢复短语只存在于短时、可清零的 Core-private factor session；二维码 payload、PNG 与路径不得进入 Agent、CLI DTO、日志、journal、report 或遥测。

### 2. fresh restore

1. 可信本地界面要求用户重新输入恢复短语，并通过 native open panel 选择二维码 PNG。
2. Core 以有界方式读取图片，解码出唯一二维码，取得 canonical CBOR payload并验证 checksum、version、`vault_id` 与长度。
3. Core 对恢复短语执行与备份完全相同的 canonical validation 和 Argon2id，对二维码中的 `recovery_key` 执行与备份完全相同的 parser；两者继续按既有 HKDF 2-of-2 规范组合并解封随机 `master_key`。
4. 任一因素错误、图片损坏、图片中没有二维码、存在多个二维码、二维码属于其他 vault，均不得泄露是哪一个因素错误；target mutation 必须为零。
5. 相机扫描和 Bech32m 人工串可以作为未来或辅助输入方式，但不能替代本轮 Mac 到 Mac E2E 对“保存的二维码 PNG → fresh restore”的验证。

### 3. 不改变的密码学合同

- 保留随机 32-byte master key、Argon2id phrase KDF、`phrase_key || recovery_key`、HKDF-SHA256 KEK、XChaCha20-Poly1305 envelope wrap；
- Direct Google Drive 与 portable vault bundle 使用同一 security suite、同一 envelope、同一因素 parser，不建立 E2E 专用格式或旁路；
- 恢复短语、二维码和解码后的恢复密钥始终位于 ciphertext vault、portable bundle 与 Google Drive 数据对象之外。

## 修订验收案例

### A. 备份因素建立

1. **正常路径**：用户输入并确认合法短语；Core 生成独立 recovery key；保存出的 PNG 能被重新解码成唯一 canonical CBOR，并与当前 vault 绑定；完成后任务才进入 Ready。
2. **短语输入边界**：空输入、不是七词、非 EFF 单词、大小写/双空格/首尾空格、超过 1 KiB 均在 Argon2 和任何 provider mutation 前拒绝；两次输入不一致同样拒绝。
3. **二维码持久化边界**：取消 save、目标已存在、目录不可写、写入中断、落盘后 bit flip、回读时不是 PNG/没有 QR/多个 QR，均不得进入 Ready，也不得产生 Google Drive final object。
4. **输出收缩**：正式备份只要求用户保存一个二维码 PNG；软件不生成恢复短语文件或 recovery-key 文本文件；PNG 使用新建写入、private file mode，并且不静默覆盖。

### B. fresh restore 解锁

5. **正常路径**：fresh Core 不继承 backup 的因素内存、bootstrap、token 或 local handle；用户输入原短语并选择原二维码 PNG，成功解封同一 master key，随后才能下载/解密并写入用户批准 target。
6. **错误组合**：正确短语+错误 QR、错误短语+正确 QR、两者都错误、其他 vault 的合法 QR，全部返回同一稳定的因素无效错误，target mutation 为零。
7. **图片输入安全**：空文件、超限图片、截断 PNG、恶意尺寸、无二维码、多个二维码、非 canonical CBOR、错误 checksum/version/vault ID 均 fail closed，且不会把图片内容或 payload 写入日志。
8. **同规范回环**：backup 保存并回读二维码所用的 CBOR parser，与 restore 解锁所用 parser 完全相同；Direct 和 Portable 路线不能使用测试文本因子或专用 bypass。

### C. 真实 E2E 与保密性

9. **四条正向 E2E**：Direct × 普通/敏感目录、Portable × 普通/敏感目录都执行“用户输入短语 → 保存二维码 PNG → fresh Core 输入同一短语并选择该 PNG → restore tree digest 等于 source”。
10. **权限与状态顺序**：因素完成前 provider mutation 为零；敏感 source/target 未授权时停在明确等待态；授权后新 revision 重扫；因素输入/二维码选择需要解锁会话，后续加密、上传、下载和解密允许锁屏继续。
11. **零秘密证据**：stdout/stderr、Core 日志、CLI、task report、journal、Google Drive metadata/object、portable bundle 和 E2E evidence 对恢复短语、QR payload、Bech32m 串及因素文件路径零命中。
12. **永久丢失语义**：只有短语或只有二维码时明确无法恢复；没有 reset、联系客服、Google 找回、厂商托管或单因素降级路径。

## 影响

- 当前自动生成短语、保存两个文本文件、从两个文本文件恢复的 local-action 实现必须替换；
- crypto v1 的 2-of-2、envelope 和数据加密格式不因交互修正而取消；
- 当前已经保存的两份测试文本材料只能作为旧实现诊断材料，不能计入正式 E2E 通过证据。
