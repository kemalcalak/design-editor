#!/usr/bin/env python3
"""software-step-tutorial.op — 软件操作步骤卡（1080×1350，4:5 单卡）

教程档九张里的「工具/软件」那一张，也是唯一一张**深色**教程。

### 和既有 screenshot-tutorial 的区别（为什么不是重复选题）

仓里已有 `screenshot-tutorial`：暖米底 3:4、五帧轮播、三个整页截图位，教
的是「怎么做一套教程图」。这一张换了三个维度，不是同一张的改色版：

  - **画幅与帧数**：4:5 **单卡**。一张图讲完一件事，适合评论区直接甩链接
    的场景；轮播那张适合当系列首图。
  - **信息主元素**：主元素是**菜单路径条**（文件 › 导出为 › PNG 2x），不
    是截图。软件教程真正难的是「那个按钮在哪」，路径条把它一行说清；截图
    退成佐证，只留一个。
  - **色温**：深色 UI 风。软件截图九成是浅色界面，压在深底上边界自己就出
    来了，不需要再给它画边框——暖米底那张必须靠 3px 描边才框得住截图。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：从「代码编辑器 / 设计工具的深色主题」这一族取色：底不是纯
    黑而是带一点青的石板灰，面板逐级提亮 3 档，强调色取终端绿系。
  - **收敛**：中性 5 档（L 0.09 / 0.13 / 0.17 / 0.24 / 0.65 / 0.96）+ 1 个
    有彩色（青绿 #35D6A4，chroma 0.13）。
  - **论证**：深色界面里最贵的资源是「亮」——把亮度留给三样东西：标题、
    截图、路径条末端那个真正要点的按钮。其余全部压在 0.09-0.24 之间，读
    者的眼睛自然被推到该看的地方。青绿而不是蓝紫：蓝紫在深底上是「廉价 AI
    科技风」的签名色，青绿在终端/编辑器语境里有真实出处。

### 负约束（本模板明令不做的事）

  - **不用蓝紫渐变、不用外发光。** 深色 + 发光是廉价科技风的固定搭配，本
    模板的层级全部由**面板明度**给，一处发光都没有。
  - 不画伪 UI。除了那一个截图位，不虚构窗口标题栏、假按钮、假代码——虚构
    的界面细节永远不可读，读者一眼就知道是假的。
  - 不用 emoji 当图标；图标只用单色线性 lucide，且只出现在路径分隔与占位。
  - 不做圆角以外的装饰形状（不加斜切、不加网格纹理、不加扫描线）。
  - 步骤最多 3 条。第 4 条就该拆成第二张卡，不缩字号硬塞。
  - 说明句不写「点击此处即可轻松完成」这类零信息量的套话，每句都要带一个
    可验证的具体值（倍数、格式、勾选项）。

硬契约：
  - 内容距边缘 ≥72px（这里 72）
  - 配色全部走 color_vars；换主色只需改 c-accent 与 c-accent-ink 两个
  - 正文与背景对比度 ≥2.0（实测表见文件末尾）
  - **CJK 行高比西文全线高 0.2**：display 1.2，标题 1.35，正文 1.7
  - **CJK 负字距不超过 -0.02em**（64px 标题 → -1.2px = -0.019em）
  - 汉字走 Noto Sans SC，数字与拉丁走 Inter（等价于「西文在前中文在后」的
    fallback 链，在 .op 里按节点写死）
  - 顶层 frame 显式写 x/y
  - 截图位实测 936×387（2.42:1）——改上下任何一块的高度都要重量
  - 截图位是 frame（合法图片拖放目标），提示做成它的子节点：拖图后整块占位
    连提示一起被替换
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import (Ids, color_vars, frame, icon_font, solid, stroke, text,
                   write_doc)

ids = Ids()

VARS = color_vars({
    "c-bg":         "#14171A",
    "c-panel":      "#1D2126",
    "c-card":       "#262C33",
    "c-line":       "#3A424B",
    "c-ink":        "#F3F6F8",
    "c-muted":      "#9AA6B0",
    "c-accent":     "#35D6A4",
    # 强调块上的字。深底模板最容易翻车的一处：把 c-ink 直接放到 accent 上
    # 只有 1.9:1。这里给一个专用的深字色。
    "c-accent-ink": "#08231B",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W, H = 1080, 1350
EDGE = 72

LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.35, 1.7

# 菜单路径。末尾那一段是真正要点的东西，所以只有它反白成实心 chip。
PATH = ["文件", "导出为", "PNG · 2x"]

# (序号, 一句话动作, 带具体值的说明)
STEPS = [
    ("1", "先把画板选中，再打开导出",
     "没选中画板时导出的是整个画布，边上会多出一圈空白。"),
    ("2", "倍数选 2x，格式选 PNG",
     "1x 发出去在手机上发虚；JPG 会把文字边缘压出灰边。"),
    ("3", "勾掉「包含背景」再导",
     "留白交给发布平台去铺，导出时带底色会和它的底打架。"),
]


def col(name, children, *, gap=16, width="fill_container", align="start",
        **props):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="vertical", gap=gap, alignItems=align, fill=[],
                 **props)
    node["children"] = children
    return node


def row(name, children, *, gap=20, align="center", width="fill_container",
        **props):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="horizontal", gap=gap, alignItems=align, fill=[],
                 **props)
    node["children"] = children
    return node


def chip(label, *, bg, fg, weight=600, size=26):
    node = frame(ids, "路径段", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[12, 22], cornerRadius=10,
                 alignItems="center", justifyContent="center", fill=solid(bg))
    node["children"] = [
        text(ids, "路径段文字", label, size, weight, fg, family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
    ]
    return node


# ------------------------------------------------------------------ 页头
def header():
    tag = frame(ids, "档位标签", width="fit_content", height="fit_content",
                layout="horizontal", padding=[10, 20], cornerRadius=999,
                alignItems="center", justifyContent="center",
                fill=solid("$c-card"))
    tag["children"] = [
        text(ids, "档位文字", "操作教程 · 共 3 步", 24, 600, "$c-accent",
             family=CJK, width="fit_content", growth="auto", line_height=1.4),
    ]
    return col("页头", [
        tag,
        text(ids, "主标题", "三步把设计稿\n导成能直接发的图", 64, 700,
             "$c-ink", family=CJK, line_height=LH_DISPLAY, spacing=-1.2),
        text(ids, "副标题", "菜单藏得深，记住下面这条路径就够了。", 26, 400,
             "$c-muted", family=CJK, line_height=LH_BODY),
    ], gap=20)


# ------------------------------------------------------------------ 路径条
def path_bar():
    kids = []
    for index, seg in enumerate(PATH):
        if index:
            kids.append(icon_font(ids, "路径分隔", "chevron-right", 22,
                                  "$c-muted"))
        last = index == len(PATH) - 1
        kids.append(chip(seg,
                         bg="$c-accent" if last else "$c-panel",
                         fg="$c-accent-ink" if last else "$c-ink"))
    bar = row("菜单路径条", kids, gap=12, padding=[16, 20])
    bar["fill"] = solid("$c-panel")
    bar["cornerRadius"] = 16
    bar["stroke"] = stroke("$c-line", 2)
    return bar


# ------------------------------------------------------------------ 截图位
def shot():
    node = frame(ids, "截图占位", width="fill_container",
                 height="fill_container", layout="vertical", gap=16,
                 alignItems="center", justifyContent="center",
                 cornerRadius=20, fill=solid("$c-card"),
                 stroke=stroke("$c-line", 2))
    node["children"] = [
        icon_font(ids, "占位图标", "image", 56, "$c-muted"),
        text(ids, "占位标题", "把导出面板的截图拖进来", 30, 600, "$c-ink",
             family=CJK, align="center", line_height=LH_HEAD),
        text(ids, "占位规格", "占位比例 2.4:1，导出前记得关掉通知",
             24, 400, "$c-muted", family=CJK, align="center",
             line_height=LH_BODY),
    ]
    return node


# ------------------------------------------------------------------ 步骤
def step_row(no, title, desc):
    box = frame(ids, "序号底", width=48, height=48, layout="horizontal",
                alignItems="center", justifyContent="center", cornerRadius=12,
                fill=solid("$c-panel"))
    box["children"] = [
        text(ids, "序号", no, 26, 700, "$c-accent", family=NUM,
             width="fit_content", growth="auto", line_height=1.0),
    ]
    return row("步骤", [
        box,
        col("步骤文案", [
            text(ids, "步骤标题", title, 28, 600, "$c-ink", family=CJK,
                 line_height=LH_HEAD),
            text(ids, "步骤说明", desc, 24, 400, "$c-muted", family=CJK,
                 line_height=LH_BODY),
        ], gap=6),
    ], gap=18, align="start")


def steps():
    return col("步骤区", [step_row(*s) for s in STEPS], gap=18)


# ------------------------------------------------------------------ 页脚
def footer():
    return row("页脚", [
        text(ids, "账号名", "@ 你的账号名", 24, 600, "$c-muted", family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
        text(ids, "用法提示", "深色块是图片位，直接把截图拖上去", 22, 400,
             "$c-muted", family=CJK, width="fit_content", growth="auto",
             line_height=1.4),
    ], gap=16, align="center", justifyContent="space_between")


def build():
    page = frame(ids, "软件操作步骤卡", width=W, height=H, layout="vertical",
                 padding=[64, EDGE], gap=32, fill=solid("$c-bg"),
                 clipContent=True)
    page["children"] = [header(), path_bar(), shot(), steps(), footer()]
    page["x"], page["y"] = 0, 0
    return [page]


# 对比度（WCAG 相对亮度比，op-design-lint 门槛 2.0；数值由脚本实测）：
#   c-ink        on c-bg      16.58   c-muted on c-bg      7.24
#   c-ink        on c-panel   14.91   c-muted on c-panel   6.52
#   c-ink        on c-card    12.98   c-muted on c-card    5.67
#   c-accent     on c-bg       9.69   c-accent on c-panel  8.71
#   c-accent     on c-card     7.59   c-accent-ink on c-accent 8.93
# 承载正文的最低一对是 5.67（占位规格压在 c-card 上），仍高于 AA 正文
# 门槛 4.5。c-line 只用于描边，是非文字图形，不参与文字对比度。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "软件操作步骤卡 · 4:5 单卡")
