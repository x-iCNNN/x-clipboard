# Contributing / 贡献指南

Thanks for taking the time to improve X Clipboard. This project handles clipboard data, so correctness, privacy, and honest platform claims matter as much as UI polish.

感谢你帮助改进 X Clipboard。本项目会处理剪贴板数据，因此正确性、隐私和准确的平台能力说明与界面体验同样重要。

## Before opening an issue / 提交 Issue 前

- Search existing issues and documentation, especially [known validation gaps](docs/NATIVE-VALIDATION.md).
- Describe one problem per issue when possible. Include OS/version, app build or commit, reproduction steps, expected result, and actual result.
- For feature proposals, explain the user problem and how the change fits [current product decisions](design/DECISIONS.md).
- Never attach real clipboard databases, key files, passwords, tokens, or private clipboard contents. Use synthetic examples.

- 先搜索已有 Issue 和文档，尤其是[已知验证缺口](docs/NATIVE-VALIDATION.md)。
- 尽量一个 Issue 描述一个问题；注明操作系统/版本、应用构建或提交、复现步骤、预期结果和实际结果。
- 功能建议请说明用户问题，以及它如何符合[当前产品约定](design/DECISIONS.md)。
- 请勿附上真实剪贴板数据库、密钥文件、密码、令牌或私人剪贴板内容；请使用合成示例。

## Pull requests / Pull Request

1. Keep changes focused and explain the user-visible behavior or technical reason.
2. Update docs when behavior, platform support, data handling, or validation status changes.
3. Do not claim a platform is supported based only on source compilation or CI configuration. Record the actual target OS and tested scenario.
4. Avoid committing build output, user data, database files, keys, or generated diagnostics containing private content.
5. Include the checks you ran and any checks you could not run.

1. 保持改动聚焦，说明用户可见变化或技术原因。
2. 行为、平台支持、数据处理或验证状态变化时同步更新文档。
3. 不能仅凭源码可编译或存在 CI 配置就声称某平台已支持；请记录实际系统和测试场景。
4. 不要提交构建产物、用户数据、数据库、密钥或含私人内容的诊断文件。
5. 写明已运行和未能运行的检查。

## Local checks / 本地检查

For Windows development, the commonly used checks are:

Windows 开发常用检查：

```powershell
cargo fmt --all -- --check
cargo test --locked --no-default-features --lib
cargo clippy --locked --all-targets -- -D warnings
cargo build --locked
```

System clipboard probes may modify the clipboard temporarily. Run them only when you can tolerate that side effect and use synthetic content. See [native validation](docs/NATIVE-VALIDATION.md) for platform-specific commands and limitations.

系统剪贴板探针可能临时改写剪贴板。只有在可以接受该影响时才运行，并使用合成内容。平台命令和边界见[原生验证记录](docs/NATIVE-VALIDATION.md)。

## Scope and design / 范围与设计

The HTML prototype is design-review material, not a production UI implementation. Preserve the behaviors and data semantics recorded in [DECISIONS.md](design/DECISIONS.md). Discuss significant changes to clipboard capture, encryption/key handling, sync, or platform support before implementation.

HTML 原型用于设计评审，不是生产界面实现。请遵守 [DECISIONS.md](design/DECISIONS.md) 中记录的行为和数据语义。剪贴板采集、加密/密钥处理、同步或平台支持等重大变化，请先讨论再实施。
