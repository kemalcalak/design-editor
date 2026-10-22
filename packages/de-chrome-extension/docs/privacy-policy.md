# Privacy policy — Design Editor (Chrome extension)

_Last updated: 2026-10-22. Applies to the Chrome extension **Design Editor**,
version 0.8.5 and later (listed as "Design Editor Web Capture" before 0.8.3)._

This document is the source text for the privacy policy URL required by the
Chrome Web Store listing. Publish it verbatim; if the extension's behaviour
changes, change this file in the same commit.

---

## English

### The short version

The extension reads a web page only when you press one of its buttons, and
sends what it read only to a destination on your own computer: the Design
Editor running there, or a file in your downloads folder. It has no account,
no sign-in and no remote server of its own. **Extract design.md** sends bounded de-identified style evidence with
the page title removed to your local Design Editor, which may use the AI provider
you configured there. The extension never contacts that provider or receives
its credentials. There is no analytics, telemetry or advertising.

### What is collected, and when

**Page content — only on your action.** Pressing **Capture full page** or
**Capture element** runs the Design Editor DOM extractor in the tab that is
currently open. It reads the rendered document: element geometry, computed
styles, text content, and images (which are rasterized into the snapshot).
That means a capture can contain anything visible on the page at that moment,
including text you typed into a form and content behind a login. It is
produced in memory, in your browser, on your press, and it is never captured
in the background, on a schedule, or on pages you have not acted on.

**De-identified design-style evidence — only when you press Extract design.md.**
This is not the content-bearing snapshot. A one-pass collector samples at most
12,000 visible elements and aggregates colours, typography roles, spacing,
padding, gaps, integer radii, shadows, gradients, semantic component kinds,
de-identified style samples, safe CSS variables and readable media queries.
The result is stably ordered and limited to 256 KiB. The copy sent to
Design Editor and the configured provider does not contain page text or title,
URLs/href/src values, HTML, image bytes, class names or ids. A sanitised page
title of at most 120 characters is retained only inside the extension for the
heading of the deterministic local fallback, and is blanked before any request.

**Nothing is read from tabs you do not act on.** The extension holds no
`<all_urls>` permission. It uses `activeTab`, which Chrome grants for one tab
at the moment you click the toolbar button and revokes afterwards.

**Browsing history is not collected.** The extension does not observe
navigation, does not enumerate your tabs, and keeps no record of the pages you
have captured.

### Where a capture goes

You choose one of two destinations, both on your own computer:

1. **A Design Editor on this computer** (the default). The snapshot is POSTed to
   the loopback address in the popup's settings — `127.0.0.1:3100` unless you
   changed it. The extension can only address loopback: the destination is
   validated against a whitelist of `127.0.0.1` and `localhost` (rewritten to
   `127.0.0.1`), and
   the manifest grants no other host for this purpose. The data never leaves
   your machine.
2. **A file on this computer.** **Download .op** writes a ready-to-open `.op`
   document to your downloads folder and sends it nowhere.
   **Design generation is separate from those snapshot destinations.** The
   extension POSTs only bounded style evidence with its title field blanked to
   `/api/generate/design-md` at the loopback Design Editor endpoint, then polls that
   local job without resending the evidence. Design Editor may forward that evidence
   to a compatible built-in API provider configured in the app; the provider's
   own privacy and retention terms apply. Tool-capable CLI and ACP adapters are
   not used for this extension-triggered route. The extension does not know the
   provider URL or credentials and cannot contact it directly. If that intelligent
   route is unavailable, the bundled local logic produces a deterministic
   `design.md` without sending the evidence beyond your computer.

### What is stored on your device

In `chrome.storage.local`, on your machine only:

| Key            | What                                                     |
| -------------- | -------------------------------------------------------- |
| `endpoint`     | The loopback address you typed.                          |
| `uiLocale`     | The popup's language.                                    |
| `lastAction`   | Whether your last delivery was "send" or "download".     |
| `pickResult`   | The outcome of one element capture, until you read it.   |
| `designResult` | The outcome of one design.md request, until you read it. |

No token or password is ever written to storage. Uninstalling the extension removes all of it.

### Permissions, and why each one exists

| Permission           | Why                                                                                                      |
| -------------------- | -------------------------------------------------------------------------------------------------------- |
| `activeTab`          | Read the page you pressed the button on — one tab, one action, revoked afterwards.                       |
| `scripting`          | Inject the snapshot extractor, de-identified style collector, transfer harness and picker into that tab. |
| `downloads`          | Save the `.op` document or generated `design.md`.                                                        |
| `storage`            | Remember the settings in the table above.                                                                |
| `http://127.0.0.1/*` | Deliver a snapshot or bounded design evidence to Design Editor on your computer.                         |

There is deliberately **no** `<all_urls>`, no `cookies`, no `history`, no
`tabs`, and no `webRequest` permission.

### What is not done

No analytics or telemetry of any kind. No advertising or advertising
identifiers. No user tracking. No selling or transferring data to third
parties by the extension. The extension does not use your data to train models;
if your configured AI provider receives design evidence, its terms apply. No
remote code: everything the extension runs ships inside the package, as the
Manifest V3 rules require.

### Contact

Questions, or a report: <https://github.com/kemalcalak/design-editor/issues>.

---

## 中文

### 一句话版本

只有当你按下扩展的按钮时，它才会读取网页；读到的内容只会发往你自己配置的目的
地，且都在你自己的电脑上：本机运行的 Design Editor，或下载文件夹中的一个文件。扩展
没有账号、没有登录，也没有自己的远程服务器。按下 **提取 design.md** 时，移除页面标题后的有界去标识化样式证据会发给本机 Design Editor；
Design Editor 可能调用你在那里配置的 AI 服务。扩展本身不会联系该服务，也不会取得其
凭据。没有任何统计、遥测或广告。

### 收集什么，何时收集

**网页内容 —— 仅在你操作时。** 按下 **捕获整页** 或 **捕获元素** 时，扩展会在
当前标签页中运行 Design Editor 的 DOM 提取器，读取渲染后的文档：元素几何、计算样
式、文本内容和图片（图片会被栅格化进快照）。因此一次捕获可能包含该时刻页面上
一切可见的内容，包括你填入表单的文字以及登录后才能看到的内容。它在你的浏览器
内存中生成，由你的按键触发；扩展不会在后台、按计划或在你没有操作过的页面上进
行捕获。

**去标识化设计样式证据 —— 仅在你按下“提取 design.md”时。** 这不是包含网页内容的
快照。采集器单遍最多检查 12,000 个可见元素，只聚合颜色用途、字体角色、间距、
padding、gap、整数圆角、阴影、渐变、语义组件类型、去标识化样式样本、安全 CSS
变量和可读取的媒体查询。结果稳定排序，最多 256 KiB；发送给 Design Editor 和已配置
AI 提供商的副本不包含页面文字或标题、URL、href/src、HTML、图片字节、class 或
id。最多 120 字符的已清理页面标题仅在扩展内部用于本地确定性回退的标题，任何请求
发出前都会把该字段清空。

**不会读取你没有操作的标签页。** 扩展没有 `<all_urls>` 权限，它使用
`activeTab`：Chrome 只在你点击工具栏按钮的那一刻为该标签页授权，之后即收回。

**不收集浏览记录。** 扩展不监听导航、不枚举标签页，也不保存你捕获过哪些页面。

### 捕获的内容去往哪里

两个目的地，都在你自己的电脑上：

1. **本机的 Design Editor**（默认）。快照会 POST 到弹窗设置中的回环地址，未修改时
   为 `127.0.0.1:3100`。扩展只能寻址回环：目标地址会经过 `127.0.0.1` 与
   `localhost`（会改写为 `127.0.0.1`）的白名单校验，清单文件也没有为此授予其他主机。数据不会离开你的
   电脑。
2. **本机的一个文件。** **下载 .op** 会把可直接打开的 `.op` 文档写入下载文件夹，不发往任何地方。
   **设计生成与上述快照目的地相互独立。** 扩展只会把有界样式证据 POST 到本机
   Design Editor 回环端点的 `/api/generate/design-md`（发送前页面标题会被清空），随后轮询
   本机任务且不会重复发送证据。Design Editor 可能将其交给你在应用中配置、且兼容这条
   隔离路径的内置 API 提供商；该提供商自己的隐私与保留条款适用。带工具能力的 CLI
   和 ACP 适配器不会用于扩展触发的智能生成。扩展不知道提供商地址或凭据，也不能直接
   联系它。智能路径不可用时，安装包内的本地逻辑会确定性生成 `design.md`，证据不会离开
   你的电脑。

### 在你设备上保存了什么

仅保存在你本机的 `chrome.storage.local` 中：

| 键             | 内容                                |
| -------------- | ----------------------------------- |
| `endpoint`     | 你填写的回环地址。                  |
| `uiLocale`     | 弹窗语言。                          |
| `lastAction`   | 上次是"发送"还是"下载"。            |
| `pickResult`   | 一次元素捕获的结果，读过即清。      |
| `designResult` | 一次 design.md 请求结果，读过即清。 |

任何令牌和密码都不会写入存储。卸载扩展会一并删除以上全部内容。

### 各项权限及其理由

| 权限                 | 理由                                                                 |
| -------------------- | -------------------------------------------------------------------- |
| `activeTab`          | 读取你按下按钮的那个页面：一个标签页、一次操作，随后收回。           |
| `scripting`          | 向该标签页注入快照提取器、去标识化样式采集器、传输桥和元素选取浮层。 |
| `downloads`          | 保存 `.op` 文档或生成的 `design.md`。                                |
| `storage`            | 记住上表中的设置。                                                   |
| `http://127.0.0.1/*` | 把快照或有界设计证据送到你电脑上运行的 Design Editor。               |

扩展刻意**没有** `<all_urls>`、`cookies`、`history`、`tabs`、`webRequest` 权限。

### 不做什么

不做任何统计或遥测。没有广告和广告标识符。不做用户追踪。不向第三方出售或转移数
据。扩展不会使用你的数据训练模型；若你配置的 AI 提供商收到设计证据，则适用其条
款。不加载远程代码：扩展运行的一切都随安装包一起交付，这也是 Manifest V3 的要
求。

### 联系方式

问题与反馈：<https://github.com/kemalcalak/design-editor/issues>。
