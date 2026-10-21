#!/usr/bin/env python3
"""myth-truth-comparison.op — 误区 vs 真相（1080×N 竖版长图）

对比档的「逐组交错」那一张：五组「大家都这么说 / 其实是这样」，误区块靠
左、偏窄、浅底，真相块靠右、偏宽、深底，五组下来是一条 Z 字。读者一次只
处理一组，读完一组才进下一组——这和把错与对各聚一栏的读法完全不同。

### 最近邻论证（为什么它不是已有的哪一张）

  - **本批 02 好坏双栏**：那张把「错」和「对」各自聚成一栏，一屏看完，眼
    睛横向来回。这张是**纵向 Z 字**：一组误区 + 一组真相，读完再往下。同
    为二元对照，那张考的是「同时对比」，这张考的是「逐条纠正」——所以那张
    只能装一组示例，这张能装五个。
  - **pitfall-list-infographic（避坑排行）**：那张是「一条坑 + 一句改法」，
    单位是**一条**且有排名；这张的单位是**一对**，且没有排名——五个误区
    一样常见，谁在前面只取决于叙事顺序。
  - **本批 01 参数表**：那张比的是可量化的值，这张比的是**说法**。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：「说法对不对」没有颜色。硬派色相等于抽签。
  - **收敛**：0 个有彩色 + 一条微冷中性明度序列 L 0.08 / 0.21 / 0.35 / 0.56 /
    0.89 / 0.92 / 0.96 / 1.0，chroma ≤0.004。
  - **论证**：误区/真相是**二值**信息，二值信息用二值明度表达最直接。更要
    紧的是——一旦给误区配红，读者会觉得这些说法「愚蠢」，而它们恰恰是聪
    明人也会信的话；浅灰只说「这是流传的版本」，不说「说这话的人蠢」。语
    气是靠明度调的。

### 负约束（本模板明令不做的事）

  - **不用任何有彩色。** 一个都不给。
  - 不用红绿对错配色。这里没有对错，只有过时的说法和更准的说法。
  - 不写「99% 的人都错了」「震惊」这类钩子。误区块照抄真实语气。
  - 误区块不加引号图标、不加删除线、不做模糊处理。
  - 不用蓝紫渐变、霓虹线条、伪 3D、阴影、emoji 图标。
  - 误区 ≤14 字，真相 ≤22 字（一行装得下）。写不下就说明它其实是两条。
  - 五组不排名、不编号大小。编号只用来记位置，不表示严重度。

硬契约：
  - 内容距边缘 ≥80px（这里 80）
  - 误区块窄（MYTH_W）、真相块宽（TRUTH_W），且分别靠左 / 靠右对齐 ——
    Z 字是靠这两处宽度差 + 两个 justifyContent 做出来的，不是靠 x/y
  - 配色全部走 color_vars，全是中性明度序列，没有「主色」可换
  - 正文与背景对比度 ≥2.0（最低一对见文件末尾实测表）
  - **CJK 行高**：大标题 1.2，区块标题 1.3，正文 1.7
  - **CJK 负字距不超过 -0.02em**
  - 顶层 frame 必须显式写 x/y，否则多帧会全部堆在原点
  - 根高固定：ROOT_H 是量出来的（见文件末尾），改内容后要重量一次
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
    "c-bg":        "#F2F2F4",
    "c-card":      "#FFFFFF",
    "c-panel":     "#E4E4E7",
    "c-ink":       "#111113",
    "c-muted":     "#4E4E53",
    "c-inv-muted": "#A2A2A8",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W = 1080
EDGE = 80
MYTH_W = 600                  # 误区块：窄、靠左
TRUTH_W = 760                 # 真相块：宽、靠右

LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.3, 1.7

ROOT_H = 2593

# (序号, 误区 ≤14 字, 真相 ≤22 字)。序号只标位置，不表示严重度。
#
# 真相块内容宽 700px，30px 中文一行约 23 字。写到 24 字就会把最后一两个字
# （常常正好是句号）单独甩到第二行——五组下来就是五个孤字。所以这里全部
# 压到一行以内：真相本来就该是一句能被记住的话，写不下说明它是两条。
PAIRS = [
    ("01", "粉丝多了自然有收入",
     "会付钱的一千人，比路过的十万人值钱。"),
    ("02", "必须日更才有流量",
     "读者记住的是「周三更新」，不是「天天有」。"),
    ("03", "设备越好内容越好",
     "手机拍清楚就够了，卡住你的是选题。"),
    ("04", "爆一条就起来了",
     "爆款只带来一次曝光，留住人的是第二条。"),
    ("05", "要找一个没人做的方向",
     "没人做通常是因为没人看，挤进有人的地方。"),
]


def band(name, *, fill, pad, gap, children, align="start"):
    node = frame(ids, name, width="fill_container", height="fit_content",
                 layout="vertical", padding=pad, gap=gap, alignItems=align,
                 fill=fill)
    node["children"] = children
    return node


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


def section_head(title, note):
    return col("区块头", [
        rect(ids, "强调短线", width=72, height=8, fill=solid("$c-ink")),
        text(ids, "区块标题", title, 46, 700, "$c-ink", family=CJK,
             line_height=LH_HEAD),
        text(ids, "区块说明", note, 27, 400, "$c-muted", family=CJK,
             line_height=LH_BODY),
    ], gap=16)


# ------------------------------------------------------------------ 01 页头
def header():
    return band("01 页头", fill=solid("$c-ink"), pad=[76, EDGE, 68, EDGE],
                gap=26, children=[
        tag("误区对照 · 内容向", bg="$c-card", fg="$c-ink"),
        text(ids, "主标题", "五句听多了\n就以为是真的", 72, 700, "$c-card",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.4),
        text(ids, "副标题", "浅色那句是流传的版本，深色那句是实际发生的事。",
             28, 400, "$c-inv-muted", family=CJK, line_height=LH_BODY),
    ])


# ------------------------------------------------------------------ 02 五组
def myth_block(index, line):
    """误区块：窄、浅底、常规字重——语气是「有人这么说」，不是「你真蠢」。"""
    box = col("误区", [
        # 序号与小标签都走 c-muted，不再多加一档更浅的灰：浅灰压在
        # c-panel 上只有 2.93，24px 粗体也够不到 AA 大字门槛 3.0。这一块
        # 的层级改由字号（24 / 32）和字重承担，明度只分「浅底块 / 深底块」
        # 两级——这也正是本模板的论证：二值信息只需要二值明度。
        row("误区头", [
            text(ids, "序号", index, 26, 700, "$c-muted", family=NUM,
                 width="fit_content", growth="auto", line_height=1.3),
            text(ids, "误区名", "常听到的说法", 24, 600, "$c-muted",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
        ], gap=12, align="center"),
        text(ids, "误区正文", line, 32, 400, "$c-muted", family=CJK,
             line_height=1.45),
    ], gap=12, width=MYTH_W, padding=[26, 28])
    box["fill"] = solid("$c-panel")
    return box


def truth_block(line):
    """真相块：宽、深底、反白——它比误区宽，是因为它需要更多字才说得准。"""
    box = col("真相", [
        text(ids, "真相名", "实际上", 24, 600, "$c-inv-muted", family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
        text(ids, "真相正文", line, 30, 500, "$c-card", family=CJK,
             line_height=LH_BODY),
    ], gap=12, width=TRUTH_W, padding=[28, 30])
    box["fill"] = solid("$c-ink")
    return box


def pair_group(index, myth, truth):
    return col("一组", [
        row("误区行", [myth_block(index, myth)], gap=0, justify="start",
            align="start"),
        row("真相行", [truth_block(truth)], gap=0, justify="end",
            align="start"),
    ], gap=12)


def pairs():
    groups = [pair_group(*pair) for pair in PAIRS]
    return band("02 五组", fill=[], pad=[64, EDGE, 0, EDGE], gap=34,
                children=[
        section_head("一组一组来", "每组只有两句：先看你信的那句，再看下一句。"),
        col("组列表", groups, gap=40),
    ])


# ------------------------------------------------------------------ 03 页脚
def footer():
    return band("03 页脚", fill=solid("$c-ink"), pad=[44, EDGE], gap=12,
                children=[
        text(ids, "结语", "第六句是「这五句我早就知道了」。", 30, 600,
             "$c-card", family=CJK, line_height=LH_HEAD),
        row("署名行", [
            text(ids, "账号名", "@ 你的账号名", 26, 600, "$c-card",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
            text(ids, "更新说明", "每周拆一句听腻了的话", 24, 400,
                 "$c-inv-muted", family=CJK, width="fit_content",
                 growth="auto", line_height=1.4),
        ], gap=16),
    ])


def build():
    page = frame(ids, "误区与真相长图", width=W, height=ROOT_H,
                 layout="vertical", gap=0, fill=solid("$c-bg"),
                 clipContent=True)
    page["children"] = [header(), pairs(), footer()]
    page["x"], page["y"] = 0, 0
    return [page]


# 对比度（WCAG 相对亮度比，op-design-lint 门槛 2.0；数值由 contrast.py 实测）：
#   c-ink  on c-bg    16.87   c-muted on c-bg      7.40
#   c-muted on c-panel 6.52   c-card  on c-ink    18.86
#   c-inv-muted on c-ink 7.43
# 承载正文的最低一对是 6.52（误区块正文压在 c-panel 上）。整张图零有彩色，
# 只有两级明度：浅底块 = 流传的说法，深底块 = 实际发生的事。第一版给误区
# 小标签用了更浅的一档灰（2.93 on c-panel），那行字实际上是看不见的——所以
# 这里删掉了那一档，层级全交给字号和字重。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "误区与真相长图")
