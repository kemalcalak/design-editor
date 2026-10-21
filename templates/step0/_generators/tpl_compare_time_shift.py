#!/usr/bin/env python3
"""time-shift-comparison.op — 时间对比 一年前 / 现在（1080×1440 竖版 3:4）

对比档的「中轴镜像」那一张：一条居中的标签脊柱，左边是一年前，右边是现
在，同一项的两个取值在同一行上左右分开。读者的视线沿脊柱往下走，每停一
次就同时读到两侧。

### 最近邻论证（为什么它不是已有的哪一张）

  - **本批 01 参数表**：那张的指标名在**最左列**，两个取值都在它右边——
    读法是「从左往右」。这张的指标名在**正中**，两个取值分居两侧——读法
    是「从中间往两边」。同样三列，重心完全不同：那张比的是两个东西，这张
    比的是同一个人的两个时刻，中轴就是时间本身。
  - **before-after（1600×900）**：那张也是同一主体的两个时刻，但主体是两
    张截图、只有一组。这张是**五项并行**的状态变化，没有图片位。
  - **本批 04 版本变化**：那张是产品的**逐条变更**（客观、可核对）；这张是
    人的**习惯变化**（主观、成序列）。所以那张一行一行读，这张必须两侧同
    时读——「六小时 → 九十分钟」只有并置才有说服力。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：从时间本身采——「过去」在版面语言里是褪色的（旧照片、存档、
    灰度），「现在」是有颜色的。
  - **收敛**：一年前那一侧**完全无彩**（中性灰阶 L 0.22 / 0.42 / 0.62），
    现在那一侧只加**一个**低饱和赤陶 #A94E24（chroma≈0.11），页面其余部分
    共用同一条中性序列。
  - **论证**：这张图唯一要传达的判断是「有变化，而且方向是对的」。把有彩
    色全部押在「现在」这一侧，色相本身就成了那个判断，不需要任何箭头或
    「↑」符号来补充。反过来说，如果两侧都上色，读者就得先分辨哪个色代表
    好——又回到了红绿那套需要图例的编码。

### 负约束（本模板明令不做的事）

  - **只允许一个有彩色，且只出现在「现在」这一侧。** 一年前那一侧必须是
    纯灰阶——它的褪色就是论证。
  - 不用红绿。这里没有对错，只有变化。
  - 不画「↑ 提升 300%」这类增长箭头。并置本身已经说完了。
  - 不写「蜕变 / 逆袭 / 脱胎换骨」。每一项只写它变成了什么。
  - 不用蓝紫渐变、霓虹线条、伪 3D、阴影、emoji 图标。
  - 每个取值 ≤8 字：两侧列宽固定 336px，写长了两侧会一高一低，脊柱就歪了。
  - 中轴标签必须等宽（SPINE_W），它是这张图的对齐基准。

硬契约：
  - 内容距边缘 ≥64px（这里 64）
  - 固定 3:4 画幅：根高写死 1440，靠 space_between 分配三块之间的空隙
  - 三列宽度 + 两条间距必须正好等于 INNER
  - 左列文字右对齐、右列文字左对齐——镜像是靠 textAlign 做的
  - 配色全部走 color_vars；换主色 = 改 c-now 一处
  - 正文与背景对比度 ≥2.0（最低一对见文件末尾实测表）
  - **CJK 行高**：大标题 1.2，标签 1.3，正文 1.5
  - **CJK 负字距不超过 -0.02em**；数字走 Inter
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
    "c-bg":        "#F4F3F1",
    "c-card":      "#FFFFFF",
    "c-panel":     "#E7E5E1",
    "c-ink":       "#14120F",
    "c-muted":     "#57534C",
    "c-faint":     "#6B665E",
    "c-inv-muted": "#A9A49B",
    # 唯一的有彩色，只出现在「现在」那一侧。
    "c-now":       "#A94E24",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W, H = 1080, 1440
EDGE = 64
INNER = W - EDGE * 2          # 952
SPINE_W = 200
SIDE_W = 336
SPINE_GAP = 40
assert SIDE_W * 2 + SPINE_W + SPINE_GAP * 2 == INNER

LH_DISPLAY, LH_HEAD = 1.2, 1.3

THEN_LABEL, NOW_LABEL = "一年前", "现在"

# (中轴标签, 一年前, 现在)。每个取值 ≤8 字，否则两侧行高会不一致。
SHIFTS = [
    ("更新节奏", "想起来才发", "每周三固定"),
    ("单条耗时", "六个小时", "九十分钟"),
    ("选题来源", "刷到什么写什么", "读者的提问"),
    ("封面", "每次重做", "一套模板"),
    ("收藏率", "1.2%", "6.8%"),
    ("读者留言", "偶尔一两条", "每条都有人问"),
]

# 收尾那句。六行并置容易读成「样样都进步了」，这一句把因果关系点破。
SPINE_NOTE = "六项里只有第一项是主动改的，其余五项都是它带来的。"


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


def tag(label, *, bg, fg, size=24):
    node = frame(ids, "标签", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[10, 22], cornerRadius=999,
                 alignItems="center", justifyContent="center", fill=solid(bg))
    node["children"] = [
        text(ids, "标签文字", label, size, 600, fg, family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
    ]
    return node


# --------------------------------------------------------------------- 页头
def head():
    return col("页头", [
        tag("时间对比 · 一年", bg="$c-ink", fg="$c-card"),
        text(ids, "主标题", "一年时间\n到底变了什么", 70, 700, "$c-ink",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.4),
        text(ids, "副标题", "左边是去年，右边是现在。有颜色的那一侧是当下。",
             26, 400, "$c-muted", family=CJK, line_height=1.7),
    ], gap=20)


# --------------------------------------------------------------------- 中轴
def era_header():
    """两侧的时代标签。左侧纯灰、右侧上色——整张图的色彩规则在这里声明。"""
    return row("时代行", [
        col("左时代", [
            text(ids, "去年标签", THEN_LABEL, 30, 700, "$c-faint", family=CJK,
                 align="right", line_height=LH_HEAD),
        ], gap=0, width=SIDE_W),
        col("轴头", [], gap=0, width=SPINE_W),
        col("右时代", [
            text(ids, "现在标签", NOW_LABEL, 30, 700, "$c-now", family=CJK,
                 align="left", line_height=LH_HEAD),
        ], gap=0, width=SIDE_W),
    ], gap=SPINE_GAP, align="center")


def spine_label(label):
    """中轴上的一格。等宽是整张图的对齐基准，所以它写死 SPINE_W。"""
    node = frame(ids, "轴标签", width=SPINE_W, height=64,
                 layout="horizontal", alignItems="center",
                 justifyContent="center", cornerRadius=6,
                 fill=solid("$c-panel"))
    node["children"] = [
        text(ids, "轴标签文字", label, 24, 600, "$c-muted", family=CJK,
             width="fit_content", growth="auto", align="center",
             line_height=1.4),
    ]
    return node


def shift_row(label, then, now):
    return row("变化行", [
        col("去年值", [
            text(ids, "去年文字", then, 30, 400, "$c-faint", family=NUM,
                 align="right", line_height=1.5),
        ], gap=0, width=SIDE_W, align="end"),
        spine_label(label),
        col("现在值", [
            text(ids, "现在文字", now, 30, 700, "$c-now", family=NUM,
                 align="left", line_height=1.5),
        ], gap=0, width=SIDE_W, align="start"),
    ], gap=SPINE_GAP, align="center")


def spine_note():
    node = frame(ids, "中轴收尾", width="fill_container", height="fit_content",
                 layout="horizontal", padding=[22, 26], cornerRadius=6,
                 alignItems="center", justifyContent="center",
                 fill=solid("$c-panel"))
    node["children"] = [
        text(ids, "收尾文字", SPINE_NOTE, 25, 500, "$c-muted", family=CJK,
             width="fit_content", growth="auto", align="center",
             line_height=1.5),
    ]
    return node


def spine():
    rows = [shift_row(*shift) for shift in SHIFTS]
    return col("中轴", [era_header(), col("变化列表", rows, gap=14),
                       spine_note()], gap=22)


# --------------------------------------------------------------------- 页脚
def tail():
    band = col("页脚", [
        text(ids, "结语", "变的不是能力，是把哪件事固定了下来。", 29, 600,
             "$c-card", family=CJK, line_height=LH_HEAD),
        row("署名行", [
            text(ids, "账号名", "@ 你的账号名", 25, 600, "$c-card",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
            text(ids, "更新说明", "每年这个时候复盘一次", 23, 400,
                 "$c-inv-muted", family=CJK, width="fit_content",
                 growth="auto", line_height=1.4),
        ], gap=14),
    ], gap=12, padding=[32, 32])
    band["fill"] = solid("$c-ink")
    return band


def build():
    page = frame(ids, "时间对比 一年前与现在", width=W, height=H,
                 layout="vertical", padding=[68, EDGE], gap=40,
                 justifyContent="space_between", alignItems="start",
                 fill=solid("$c-bg"), clipContent=True)
    page["children"] = [head(), spine(), tail()]
    page["x"], page["y"] = 0, 0
    return [page]


# 对比度（WCAG 相对亮度比，op-design-lint 门槛 2.0；数值由 contrast.py 实测）：
#   c-ink  on c-bg    16.86   c-muted on c-bg      6.89
#   c-faint on c-bg    5.14   c-now   on c-bg      4.98
#   c-muted on c-panel 6.08   c-card  on c-ink    18.70
#   c-inv-muted on c-ink 7.54
# 承载正文的最低一对是 4.98（「现在」那一侧的取值），刚好过 AA 正文门槛。
# 「一年前」那一侧最初用的是更浅的一档灰（3.26 on c-bg）——褪色的观感对
# 了，但 30px 常规字够不到 4.5，于是压到 5.14：仍然明显轻于 16.86 的正文
# 黑与有色的当下，褪色靠的是「比谁都浅」，不是「浅到读不出来」。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "时间对比 一年前与现在")
