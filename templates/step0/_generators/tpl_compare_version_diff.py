#!/usr/bin/env python3
"""version-diff-comparison.op — 新旧版本变化（1080×1080 方版 1:1）

对比档里唯一一张**行内**对照：不分左右两栏，每一行自己就是一次「旧 → 新」。
读者不需要在两栏之间来回扫，只要顺着列表往下滑，每一行都在一处完成对比。

### 最近邻论证（为什么它不是已有的哪一张）

  - **before-after（1600×900）**：那张也是「旧 → 新」，但只有**一组**，主
    体是两张大截图。这张是**五组**，每组只有一对短语——它比的不是画面而是
    条目，所以能塞进一个方版里滑着看。
  - **本批 02 好坏双栏**：那张把「错」和「对」各自聚成一栏，眼睛左右扫；
    这张把「旧」和「新」在**同一行内**并置，眼睛只往下走。同为二元对照，
    版式的骨架完全不同。
  - **本批 07 时间对比**：那张是一年前 / 现在的**整体气质**变化，主体是两
    组并列的状态；这张是**逐条变更**，主体是一份可以照着核对的清单。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：从「版本记录」这件事采——它属于终端、日志、更新说明那一类
    版面，记忆色是墨绿黑底上的浅字，不是任何品牌色。
  - **收敛**：单色相墨绿（H≈175°）的一条七级明度序列 L 0.09 / 0.21 / 0.38 /
    0.59 / 0.87 / 0.92 / 1.0，chroma 深处 0.03、浅处 0.008，只有一个色相。
  - **论证**：一行里有两个值在争注意力，如果给「新」配一个高饱和色，五行
    下来就是五个亮点，等于没有重点。改用**明度**：旧值是浅底弱字（已经退
    场），新值是深底反白（已经生效）。五行的深块自然连成一条视觉纵线，读
    者第一眼看到的就是「新版是什么样」。

### 负约束（本模板明令不做的事）

  - **不用第二个色相。** 整张图只有墨绿一族 + 中性白。
  - 不用红绿。版本变化没有对错，旧值当时也是对的。
  - **不给旧值加删除线。** 删除线在中文里会压在字身上、糊成一片；「退场」
    由浅底弱字表达就够了。
  - 不写「重磅 / 史诗级 / 全新升级」。每行只写变成了什么。
  - 不用蓝紫渐变、霓虹线条、伪 3D、阴影、emoji 图标。
  - 每个值 ≤8 字：新旧两个 chip 等宽，写长了两边会一高一低，纵线就断了。
  - 不列超过六行。列表一长，读者就会开始跳读，「逐条核对」这个前提就没了。

硬契约：
  - 内容距边缘 ≥64px（这里 64）
  - 固定 1:1 画幅：根高写死 1080，靠 space_between 分配三块之间的空隙
  - 新旧两个 chip 必须都是 fill_container，等宽是「同一件事的两个取值」的
    视觉前提
  - 配色全部走 color_vars；单色序列，改一处色相即整张换肤
  - 正文与背景对比度 ≥2.0（最低一对见文件末尾实测表）
  - **CJK 行高**：大标题 1.2，行标题 1.3，正文 1.6
  - **CJK 负字距不超过 -0.02em**；版本号与数字走 Inter
"""

import os
import sys

sys.path.insert(0, os.path.join(
    os.path.dirname(os.path.dirname(os.path.dirname(
        os.path.dirname(os.path.abspath(__file__))))),
    "templates", "step0", "_generators"))

from oplib import (Ids, color_vars, frame, icon_font, rect, solid, text,
                   write_doc)

ids = Ids()

VARS = color_vars({
    "c-bg":        "#EEF2F1",
    "c-card":      "#FFFFFF",
    "c-panel":     "#DCE4E2",
    "c-line":      "#C7D2CF",
    "c-ink":       "#0C1817",
    "c-deep":      "#1D3733",
    "c-muted":     "#465F5B",
    "c-faint":     "#7C918D",
    "c-inv-muted": "#9CB0AC",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W, H = 1080, 1080
EDGE = 64
LABEL_W = 168

LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.3, 1.6

OLD_VER, NEW_VER = "v1.4", "v2.0"

# (类别, 旧值, 新值)。每个值 ≤8 字：两个 chip 等宽，长短不一会把纵线撑歪。
CHANGES = [
    ("首页加载", "3.4 秒", "0.9 秒"),
    ("导航层级", "三层菜单", "一层直达"),
    ("深色模式", "没有", "跟随系统"),
    ("导出格式", "只有 PNG", "还能存 PDF"),
    ("免费额度", "10 次 / 月", "50 次 / 月"),
]


def col(name, children, *, gap=16, width="fill_container", align="start",
        **props):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="vertical", gap=gap, alignItems=align, fill=[],
                 **props)
    node["children"] = children
    return node


def row(name, children, *, gap=16, align="center", width="fill_container",
        **props):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="horizontal", gap=gap, alignItems=align, fill=[],
                 **props)
    node["children"] = children
    return node


def version_chip(label, current):
    """版本号徽记。旧版描边、新版实心——形状语言在页头就先声明一次。"""
    node = frame(ids, "版本徽记", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[9, 20], cornerRadius=8,
                 alignItems="center", justifyContent="center",
                 fill=solid("$c-ink") if current else [])
    if not current:
        node["stroke"] = {"thickness": 2, "fill": solid("$c-line")}
    node["children"] = [
        text(ids, "版本号", label, 26, 700,
             "$c-card" if current else "$c-muted", family=NUM,
             width="fit_content", growth="auto", line_height=1.4),
    ]
    return node


# --------------------------------------------------------------------- 页头
def head():
    return col("页头", [
        row("版本行", [
            version_chip(OLD_VER, False),
            icon_font(ids, "版本箭头", "arrow-right", 26, "$c-faint"),
            version_chip(NEW_VER, True),
        ], gap=14, align="center", width="fit_content"),
        text(ids, "主标题", "这版改了什么", 72, 700, "$c-ink", family=CJK,
             line_height=LH_DISPLAY, spacing=-1.4),
        text(ids, "副标题", "五处能被立刻用上的变化，深色那一半是现在的样子。",
             26, 400, "$c-muted", family=CJK, line_height=LH_BODY),
    ], gap=20)


# ------------------------------------------------------------------- 变更行
def value_chip(content, current):
    """一个取值。旧 = 浅底弱字（退场），新 = 深底反白（生效）。"""
    box = frame(ids, "取值", width="fill_container", height=64,
                layout="horizontal", padding=[0, 22], gap=0,
                alignItems="center", justifyContent="start", cornerRadius=8,
                fill=solid("$c-deep" if current else "$c-panel"))
    box["children"] = [
        text(ids, "取值文字", content, 27, 700 if current else 400,
             "$c-card" if current else "$c-muted", family=NUM,
             line_height=1.4),
    ]
    return box


def change_row(label, old, new):
    return row("变更行", [
        text(ids, "类别", label, 26, 600, "$c-ink", family=CJK,
             width=LABEL_W, line_height=1.4),
        value_chip(old, False),
        icon_font(ids, "变为", "arrow-right", 24, "$c-faint"),
        value_chip(new, True),
    ], gap=14, align="center")


def changes():
    items = [change_row(*change) for change in CHANGES]
    return col("变更列表", items, gap=14)


# --------------------------------------------------------------------- 页脚
def tail():
    band = col("页脚", [
        text(ids, "结语", "另有六项小修复，都写在更新日志里。", 29, 600,
             "$c-card", family=CJK, line_height=LH_HEAD),
        row("署名行", [
            text(ids, "账号名", "@ 你的账号名", 25, 600, "$c-card",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
            text(ids, "更新说明", "每次发版都出一张", 23, 400, "$c-inv-muted",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
        ], gap=14),
    ], gap=12, padding=[32, 32])
    band["fill"] = solid("$c-ink")
    return band


def build():
    page = frame(ids, "新旧版本变化", width=W, height=H, layout="vertical",
                 padding=[56, EDGE], gap=36, justifyContent="space_between",
                 alignItems="start", fill=solid("$c-bg"), clipContent=True)
    page["children"] = [head(), changes(), tail()]
    page["x"], page["y"] = 0, 0
    return [page]


# 对比度（WCAG 相对亮度比，op-design-lint 门槛 2.0；数值由 contrast.py 实测）：
#   c-ink  on c-bg    16.06   c-muted on c-bg      6.10
#   c-muted on c-panel 5.33   c-card  on c-deep   12.75
#   c-card on c-ink   18.12   c-inv-muted on c-ink 7.96
#   c-faint on c-bg    2.96（只画两处箭头与旧版号描边，非正文）
# 承载正文的最低一对是 5.33 —— 旧值压在浅底 chip 上，过 AA 正文门槛（4.5）
# 但明显轻于同行的新值（12.75），「退场 / 生效」的落差就是这么来的。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "新旧版本变化")
