## 中文

- 🕷️ **Bug Fix**：修复相同文字因普通文本、HTML 或 RTF 格式不同而重复保存的问题，按完整文字去重，保留最近一次复制的完整格式、来源和时间，以及原有置顶、收藏状态。
- 🔧 **Fix**：为已有历史补建文字去重索引，再次复制相同文字时合并旧重复记录；同步标记被替换的旧格式，防止旧版本重新出现。大小写和空白不同的文字仍分别保存，图片和文件仍按完整内容去重。

## English

- 🕷️ **Bug Fix**: Fixed duplicate history entries caused by plain-text, HTML, or RTF format differences. Identical text now keeps the latest complete formats, source, and copy time while preserving pinned and favorite status.
- 🔧 **Fix**: Backfill text deduplication indexes for existing history and merge older duplicates when the same text is copied again. Mark superseded formats as deleted for synchronization to prevent their return. Text case and whitespace remain significant; images and files retain full-content deduplication.
