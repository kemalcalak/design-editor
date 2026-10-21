#!/usr/bin/env python3
"""tradeoff-scale-comparison.op — 优缺点天平（1080×1080 方版 1:1）

对比档里唯一一张**读者自己得出结论**的：一根横梁、一个支点、两个托盘，左
盘装「值得」，右盘装「代价」，每一条前面都有一个空方框。图不告诉你该选
哪边——它只负责把两边都列全，然后把笔交给你。

### 最近邻论证（为什么它不是已有的哪一张）

  - **本批 02 好坏双栏**：那张的两栏有明确的对错，右栏是答案。这张的两栏
    **都是真的**——「时间自己排」和「收入前半年归零」会同时发生在同一个
    人身上。所以那张能给右栏加实心勾，这张只能给两栏一样的空框。
  - **本批 03 三方案横评**：那张替读者标了推荐列。这张刻意不标——利弊的
    权重因人而异，标了就是替别人过日子。
  - **本批 09 场景选择指南**：那张给出**判定**（这种情况选 A），这张给出
    **清单**（两边各四条）。一个替你算完，一个把算式交给你。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：不采。天平这个隐喻本身要求两边**等价**，任何色相都会立刻在
    两盘之间制造不对称。
  - **收敛**：0 个有彩色 + 一条纯中性明度序列 L 0.09 / 0.34 / 0.44 / 0.84 /
    0.90 / 0.94 / 1.0，chroma = 0（真中性灰，不带暖冷偏）。
  - **论证**：两个托盘用**完全相同**的白底、相同的描边、相同的字号字重
    ——它们唯一的差别是标题两个字。这是本模板最重要的一处克制：读者一眼
    看不出「设计师觉得哪边更重」，才会真的自己去勾。

### 负约束（本模板明令不做的事）

  - **不用任何有彩色。** 也不给两盘任何色差、明度差、字重差。
  - 不用红绿。「代价」不是错误，它是价格。
  - **不把横梁画成倾斜的。** 倾斜就是结论，而结论应该由读者勾出来。
  - 不给「值得」那边加勾、加星、加加号。空框必须两边一模一样。
  - 不写「过来人劝你」「三思」这类立场词。每条只写会发生什么。
  - 不用蓝紫渐变、霓虹线条、伪 3D、阴影、emoji 图标。
  - 每条 ≤11 字，两盘条数必须相同（这里各 4 条）。条数不等 = 提前投票。

硬契约：
  - 内容距边缘 ≥64px（这里 64）
  - 固定 1:1 画幅：根高写死 1080，靠 space_between 分配三块之间的空隙
  - 两盘必须 fill_container + stretch，且样式代码走同一条分支（pan() 不接
    受任何区分两边的参数，除了标题和条目）
  - 空方框用 `rectangle` 而不是圆角描边 `frame`：无子节点的圆角描边 frame
    会被 stub_repair::is_empty_decorated_stub 判成废容器（该判定只认
    type=="frame" 且 cornerRadius>0 或有 padding）
  - 配色全部走 color_vars，全是中性明度序列，没有「主色」可换
  - 正文与背景对比度 ≥2.0（最低一对见文件末尾实测表）
  - **CJK 行高**：大标题 1.2，盘标题 1.3，正文 1.6
  - **CJK 负字距不超过 -0.02em**
"""

import os
import sys

sys.path.insert(0, os.path.join(
    os.path.dirname(os.path.dirname(os.path.dirname(
        os.path.dirname(os.path.abspath(__file__))))),
    "templates", "step0", "_generators"))

from oplib import (Ids, color_vars, frame, rect, solid, text, write_doc)

ids = Ids()

VARS = color_vars({
    "c-bg":        "#F3F3F3",
    "c-card":      "#FFFFFF",
    "c-line":      "#D2D2D2",
    "c-ink":       "#131313",
    "c-muted":     "#515151",
    "c-faint":     "#6E6E6E",
    "c-inv-muted": "#A7A7A7",
})

CJK = "Noto Sans SC"

W, H = 1080, 1080
EDGE = 64
BEAM_W = 860

LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.3, 1.6

# 两盘条数必须相同。每条 ≤11 字。
GAINS = [
    "时间自己排",
    "做的东西是自己的",
    "上限没有天花板",
    "会遇到同频的人",
]
COSTS = [
    "前半年收入归零",
    "没人替你排优先级",
    "周末和工作日一样",
    "停更就没有进账",
]


def col(name, children, *, gap=16, width="fill_container", align="start",
        **props):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="vertical", gap=gap, alignItems=align, fill=[],
                 **props)
    node["children"] = children
    return node


def row(name, children, *, gap=16, align="center", width="fill_container",
        justify="start", **props):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="horizontal", gap=gap, alignItems=align,
                 justifyContent=justify, fill=[], **props)
    node["children"] = children
    return node


def tag(label, *, bg, fg):
    node = frame(ids, "标签", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[10, 22], cornerRadius=999,
                 alignItems="center", justifyContent="center", fill=solid(bg))
    node["children"] = [
        text(ids, "标签文字", label, 24, 600, fg, family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
    ]
    return node


# --------------------------------------------------------------------- 页头
def head():
    return col("页头", [
        tag("利弊天平 · 全职做内容", bg="$c-ink", fg="$c-card"),
        text(ids, "主标题", "先把两边\n都摆出来", 70, 700, "$c-ink",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.4),
        text(ids, "副标题", "八条都是真的，会同时发生。勾完再看哪边压得住。",
             26, 400, "$c-muted", family=CJK, line_height=1.7),
    ], gap=18)


# --------------------------------------------------------------------- 天平
def beam():
    """横梁 + 支点。刻意保持水平——倾斜就是替读者下结论。"""
    pivot = {"type": "polygon", "id": ids("pg"), "name": "支点",
             "polygonCount": 3, "width": 46, "height": 30,
             "fill": solid("$c-ink")}
    return col("横梁组", [
        rect(ids, "横梁", width=BEAM_W, height=8, fill=solid("$c-ink")),
        pivot,
    ], gap=0, align="center")


def checkbox():
    """空方框。两盘用的是同一个函数、同一套参数——没有任何一边被预先勾上。

    用 `rectangle` 而不是圆角描边 `frame`：后者无子节点时会被判成「画了壳
    没填内容」的废容器（stub_repair），rectangle 不在那条规则的范围里。
    """
    return rect(ids, "勾选框", width=24, height=24, fill=[],
                stroke={"thickness": 3, "fill": solid("$c-faint")})


def entry(line):
    wrap = frame(ids, "框位", width=24, height=42, layout="vertical",
                 justifyContent="center", fill=[])
    wrap["children"] = [checkbox()]
    return row("条目", [
        wrap,
        text(ids, "条目文字", line, 26, 400, "$c-ink", family=CJK,
             line_height=LH_BODY),
    ], gap=14, align="start")


def pan(title, entries):
    """一个托盘。这个函数不接受任何「哪一边」的参数——两盘由同一条代码路径
    产出，所以视觉上不可能出现偏袒。"""
    body = col("盘体", [
        text(ids, "盘标题", title, 34, 700, "$c-ink", family=CJK,
             line_height=LH_HEAD),
        rect(ids, "盘分线", width="fill_container", height=2,
             fill=solid("$c-line")),
        col("条目组", [entry(line) for line in entries], gap=8),
    ], gap=18, padding=[28, 26, 30, 26])
    card = col("托盘", [body], gap=0, width="fill_container")
    card["fill"] = solid("$c-card")
    card["stroke"] = {"thickness": 2, "fill": solid("$c-line")}
    card["cornerRadius"] = 6
    return card


def scale():
    return col("天平", [
        beam(),
        row("两盘", [pan("值得", GAINS), pan("代价", COSTS)], gap=20,
            align="stretch"),
    ], gap=18, align="center")


# --------------------------------------------------------------------- 页脚
def tail():
    band = col("页脚", [
        text(ids, "结语", "两边一样多。差别是：左边是概率，右边是确定。",
             29, 600, "$c-card", family=CJK, line_height=LH_HEAD),
        row("署名行", [
            text(ids, "账号名", "@ 你的账号名", 25, 600, "$c-card",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
            text(ids, "更新说明", "把决定拆成能勾的条目", 23, 400,
                 "$c-inv-muted", family=CJK, width="fit_content",
                 growth="auto", line_height=1.4),
        ], gap=14),
    ], gap=12, padding=[30, 32])
    band["fill"] = solid("$c-ink")
    return band


def build():
    page = frame(ids, "优缺点天平", width=W, height=H, layout="vertical",
                 padding=[56, EDGE], gap=32, justifyContent="space_between",
                 alignItems="start", fill=solid("$c-bg"), clipContent=True)
    page["children"] = [head(), scale(), tail()]
    page["x"], page["y"] = 0, 0
    return [page]


# 对比度（WCAG 相对亮度比，op-design-lint 门槛 2.0；数值由 contrast.py 实测）：
#   c-ink  on c-bg    16.74   c-muted on c-bg      7.15
#   c-ink  on c-card  18.58   c-card  on c-ink    18.58
#   c-inv-muted on c-ink 7.72 c-faint on c-card    5.10
# 承载正文的最低一对是 7.15。c-faint 只画空方框的描边（非文字），5.10 是
# 为了让空框在白卡上清楚可见但不抢过条目文字——它必须像一个等着被勾的框，
# 不像一个已经被强调的元素。两盘的条目一律 c-ink 18.58，完全等重。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "优缺点天平")
