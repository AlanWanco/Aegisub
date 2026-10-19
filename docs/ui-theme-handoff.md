# wxWidgets 跨平台主题原型审查交接

## 审查范围

此记录审查 `ui/modern-wx-prototype` 上尚未提交的界面主题改动，并为后续接手的编码代理列出已知状态、验证范围与修复顺序。审查期间已按要求停止源码编辑；本文件是交接文档。

- 仓库：`/Users/alanwanco/Workspace/Aegisub`
- 分支：`ui/modern-wx-prototype`
- 基线：`25e21b9fbcdc1f222a6770dca2126e5c81456192`
- 当前源码改动：28 个文件，尚未提交。
- 设计边界：继续使用 C++、wxWidgets、现有文档模型、媒体后端与 Lua Automation。

## 当前实现

- `App/Theme` 提供跟随系统、浅色、深色三种模式；当前默认配置设为深色（`Theme = 2`）。
- 外观选择已接入 Preferences 和 View 菜单。
- `src/ui_theme.h` 集中定义配色、主题窗口初始化、默认颜色映射、音频配色和跨平台图标选择。
- 主题色已接入主工作区、字幕网格和编辑器、音频显示、视频控件、工具栏及若干对话框。
- Windows 分支读取 `AppsUseLightTheme` 并调用 DWM 深色标题栏属性；Meson 为 Windows 链接 `advapi32` 和 `dwmapi`。
- Windows 菜单主题接入尚不完整：`ApplyAppearanceToMenu()` 当前只遍历子菜单，没有实际设置菜单颜色。`wxMenuBar` 的前景和背景颜色是否能改变 Windows 原生菜单外观也尚未验证。

## 已验证结果

- **macOS 编译：通过。** 使用 `SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk ninja -C build-ui -j 4 aegisub` 编译并链接成功。仍有 Boost、`typeid` 和重复链接库警告。
- **配置：通过。** `default_config.json` 和 `default_menu.json` 可解析；`msgfmt --check --check-format -o /dev/null po/zh_CN.po` 通过。
- **视觉检查：有限。** 已在本机真实窗口截过一次深色工作区；该截图是在最后一批修订之前完成的，不能代表当前最终源码。
- **Windows：未验证当前改动。** 当前未在 Windows 上编译或运行。此前 CI 的成功记录属于较早的提交，不覆盖这里尚未提交的主题改动。
- **Linux：未验证。**

## 高优先级问题

### 1. 更改外观偏好时，确认现存窗口和各子控件都能重绘

`src/main.cpp` 的 `App/Theme` 订阅会调用原生 appearance 更新及 `ui::ApplyThemeToAllWindows()`。应在运行中的应用实际切换跟随系统、浅色、深色，检查已打开对话框、音频视图、字幕网格、Scintilla 编辑器与工具栏都会同步；本机截图早于最后修订，尚未覆盖该交互。

**接手建议：** 在 macOS、Windows 和 Linux 窗口中执行实际切换；特别留意偏好回调遍历窗口树并同步发送主题事件时的窗口创建/销毁行为。

### 2. 手动浅色/深色样式可能覆盖应用自有颜色

`ui::ApplyWindow()` 会按控件类型设置窗口前景和背景。它不知道某些控件是否由功能逻辑指定了特殊颜色，例如颜色状态、错误状态、关键帧指示或自定义绘制用途。新建窗口和主题树刷新时都可能再次设置控件属性。

**接手建议：** 审计 `SetBackgroundColour`、`SetForegroundColour` 和自绘色的调用点；区分主题表面与功能状态色。对颜色按钮、错误提示、只读预览、禁用控件、键帧字段及对话框输入框逐项检查，确认浅色、深色和系统模式都有可读对比。

### 3. 跟随系统并非所有自绘区域都跟随系统

通用 wxWidgets 控件在 `ThemeMode::System` 下会恢复空颜色，但音频波形/时间轴、字幕网格、自定义滑块和部分静态区域仍使用 `ui::Canvas()`、`ui::Raised()` 或主题默认映射。系统模式下不同区域可能混用应用固定配色与平台颜色。

**接手建议：** 明确系统模式的承诺：跟随系统原生控件外观，还是让应用所有自绘区域都依据系统明暗变化。对 macOS、Windows、GTK 分别定义可验证的结果，并使自绘区域在系统切换时刷新。

### 4. Windows 菜单和按钮外观尚未证实

`ApplyAppearanceToMenu()` 没有实际应用配色；普通 Windows 菜单通常由原生菜单 API 绘制。不能据现有 setter 调用宣称菜单栏、弹出菜单和菜单文字已经纳入完整深色模式。标准按钮、选择框、旋钮和滑块能否采用指定颜色，也取决于 wxMSW 和系统主题。

**接手建议：** 在目标 wxWidgets 版本的 Windows 构建中检查真实菜单与控件。若原生控件拒绝配色，决定是否维持系统外观、采用 wxWidgets 支持的主题接口，或单独实现自绘控件；避免只检查 `GetBackgroundColour()` 这类属性而不看屏幕结果。

## Windows 堆栈溢出背景

这项历史问题与本次主题完整性审查并行处理，不要把 macOS 构建结果当作 Windows 运行验证。

- 转储 `9912` 和 `9913` 是 Windows x64 `0xC00000FD` 堆栈溢出，堆栈反复出现 wxWidgets 尺寸计算与 sizer 帧。早先怀疑 `Toolbar::OnIdle()` 重入，但后续转储仍复现，说明当时的诊断没有得到验证。
- 将 `wxWrapSizer` 回退的提交是 `3d5c295ce`；转储 `9914` 正是从这个提交生成，因而不能验证其后的视频尺寸事件修正。
- `55107b648` 移除了视频尺寸事件对视觉工具栏的刷新；`25e21b9fb` 又在刷新前检查工具是否存在及状态是否变化。
- CI run `39146755342` 在较早版本通过 Windows MSVC Release 构建和测试，但不等于在 Windows 桌面上复现原操作，也不覆盖当前未提交改动。
- 当前没有针对 `55107b648`、`25e21b9fb` 或当前主题差异的 Windows 桌面运行结果。

**Windows 验收清单：** 使用与待验提交相符的 Windows 构建，检查冷启动、主窗口尺寸变化、载入/关闭视频、显示模式切换、工具栏状态变化和矢量裁剪子工具切换。记录执行顺序；如再崩溃，保存新转储并与 `9912–9914` 的重复栈形态比较。

## 后续工作顺序

1. 修复并验证主题偏好变更对所有现存窗口的刷新路径。
2. 在 Windows 目标环境编译当前差异，修正 wxWidgets 3.2 / Windows SDK 的兼容问题；本机编译使用的是 wxWidgets 3.3.4，项目 wrap 固定为 wxWidgets 3.2。
3. 用真实 Windows 窗口逐项验收菜单、表单控件、对话框、网格、音频和编辑器的深色与浅色效果；单独记录跟随系统行为。
4. 在 macOS 和 Linux 检查系统模式与手动模式切换，确保平台差异不会破坏编辑和播放工作流。
5. 完成 Windows 堆栈溢出的桌面复现验证后，再总结是否修复；CI 结果单独报告。

## 交接时的约束

- 保留其他工作，只审查和选择性处理本次主题文件。
- 提交前向用户展示完整 diff 并取得明确确认；提交信息使用中文。
- 不要把 Windows CI 构建通过描述为 Windows 运行行为已验证。
- `docs/ui-prototype.md` 仍记录早期原型状态；完成跨平台验收后应一起更新其关于原生控件和主题覆盖范围的描述。
