# 桌面查找

一个纯本地运行的 Windows 桌面小工具，基于 Tauri 2 + Rust。按 `Alt + Space` 唤出搜索框，快速定位并高亮桌面图标，同时附带轻量的「今日计划」管理。

## 功能

- **桌面图标搜索**：枚举当前用户桌面与公共桌面项目，前端实时过滤；点击结果或回车即调用后端定位。
- **图标高亮**：用透明 overlay 边框框住选中的桌面图标。
- **系统托盘常驻**：左键单击托盘图标切换窗口显示/隐藏，右键菜单提供「显示搜索 / 退出」。
- **今日计划**：在搜索窗内切换标签，增删日程、标记完成，数据保存于 `%APPDATA%\Desktop Find\schedules.json`。

## 快捷键

- `Alt + Space`：唤出 / 隐藏搜索窗口
- `↑` / `↓`：在搜索结果间移动
- `Enter`：定位并高亮选中项
- `Esc`：关闭窗口

## 构建

需要 Node.js、npm、Rust/Cargo 及 Tauri 依赖环境。

```powershell
npm install
npm run dev    # 开发调试
npm run build  # 打包安装包
```
