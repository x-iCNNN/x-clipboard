## 中文

- 🕷️ **Bug Fix**：优化历史列表重绘与滚动，缓存筛选和分组结果，减少记录增多时的重复计算与行高测量。
- 🔧 **Fix**：后台保留最多 64 条轻量元数据，重新打开先显示首屏，再异步补齐完整历史；后台复制同步更新首屏缓存，关闭后仍释放完整列表、图片与预览。

## English

- 🕷️ **Bug Fix**: Improved history list repainting and scrolling by caching filter and group results and reducing repeated computation and row measurement as history grows.
- 🔧 **Fix**: Keep up to 64 lightweight metadata entries warm so reopening can show the first page before loading the complete history asynchronously. Background copies refresh this cache, while the full list, images, and previews are still released on close.
