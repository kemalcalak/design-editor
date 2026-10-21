#!/usr/bin/env python3
"""milestone-timeline-infographic.op — 时间线大事记长图（1080×N 竖版）

信息图这一档的第四张。前三张回答「看数字」「学方法」「抄清单」，这一张回
答**「它是怎么走到今天的」** —— 唯一一张以时间为轴的长图。

### 最近邻差异（为什么它不是 steps-flow 换个皮）

steps-flow 也是有序的，两者的分界线是**时态**，而时态又决定了三处结构：

  - **steps 讲将来，timeline 讲过去。** steps 每条是「你去做」，配的是耗时
    上限；timeline 每条是「那时发生了」，配的是年月。所以 timeline 有一列
    steps 没有的东西：年份刻度列。
  - **steps 不画连接线，timeline 必须画。** steps 的取舍写在它自己的负约
    束里 —— 跨卡片的竖线要写死像素。这张图绕开了那个陷阱：轴线不跨卡片，
    它是**每一行内部的一列**（`alignItems="stretch"` + `height:
    fill_container`），行与行 gap=0 才连成一条。改文案时行长了，那一段轴
    自己就长了。这是同一个问题的另一个解，不是同一个解。
  - **steps 是浅底暖橙，timeline 是深底靛蓝，且页头反相。** 前三张都是
    「深色页头 + 浅底正文」，这张是「**浅色页头 + 深底正文**」—— 整档并
    排时，它是唯一一张反着来的。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：从「夜里回看一年」这件事上取色 —— 深靛蓝是天光落尽之后的
    颜色，年份点像刻度上的星。不从科技模板里抽蓝紫。
  - **收敛**：一个色相（靛蓝 ~235°）+ 一条中性偏冷的明度序列。强调色只有
    一个 #7C93FF，只出现在年份、轴点和短线上。
  - **论证**：时间线的层级天然由**位置**给（越上越早），不需要颜色再分一
    次层。所以颜色只做两件事：把「轴」和「正文」分开，把「已发生」和「下
    一站」分开（实心点 vs 空心点）。

### 负约束（本模板明令不做的事）

  - **不用渐变。** 深底 + 蓝紫色相是「廉价 AI 科技风」的高发区，渐变一上
    就掉进去。全图只有实色块。
  - 不画霓虹发光线、扫描线、网格暗纹、粒子背景。深底的可读性靠留白给。
  - 不给轴线加箭头、不给节点加光晕、不做时间轴的 3D 透视。
  - 一行只记一件事：写不下就拆成两行，不缩字号（信息密度高时加行）。
  - 不用 emoji 当图标、不用装饰性插画。
  - 不写 AI 套话（「里程碑式跨越 / 全新征程」），每一条都写具体做了什么、
    以及那件事之后改了什么做法。

硬契约：
  - 内容距边缘 ≥80px（这里 80）
  - 配色全部走 color_vars，改主色只改 $c-accent 一处（轴线 $c-rail 是它
    的暗一档，换主色时要一起量）
  - 正文与背景对比度 ≥2.0（本配色最低一对见文件末尾注释）
  - **CJK 行高比西文全线高 0.2**：页头大标题 1.2，区块标题 1.3，正文 1.7
  - **CJK 负字距不超过 -0.02em**；只有西文年份沿用西文 display 的收紧
  - 汉字走 Noto Sans SC，数字与拉丁走 Inter
  - 顶层 frame 必须显式写 x/y，否则多帧会全部堆在原点
  - 根高固定：ROOT_H 是量出来的（见文件末尾），改内容后要重量一次
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import (Ids, color_vars, frame, icon_font, rect, solid, text,
                   write_doc)

ids = Ids()

VARS = color_vars({
    "c-bg":          "#0E1330",
    "c-surface":     "#1A2150",
    "c-ink":         "#F1F3FF",
    "c-muted":       "#A6AEDC",
    "c-accent":      "#7C93FF",
    # 反相页头那一块浅底，以及它上面的深字 / 次级字。整张图只有页头是亮
    # 的，所以这三个变量只服务那一处，不参与正文。
    "c-band":        "#E7EAFF",
    "c-band-ink":    "#131A44",
    "c-band-muted":  "#4A5490",
    # 轴线。强调色的暗一档：轴要看得见但不能和年份抢 —— 用 c-accent 本身画
    # 3px 竖线，整条轴会比年份还亮（实测第一版如此）。
    "c-rail":        "#5766B5",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W = 1080
EDGE = 80

# CJK 行高阶梯（西文 +0.2）。全篇只用这三档。
LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.3, 1.7

# 轴的三段固定宽度：年份列 / 轴列 / 事件卡。前两列写死像素，事件卡吃剩下
# 的 —— 年份列若用 fit_content，「2020.03」和「现在」会算出两种宽度，整条
# 轴就歪了。
YEAR_W = 150
AXIS_W = 38
ROW_GAP = 22
# 轴点所在的那一格高度。它要和年份那行文字的视觉中线对齐：年份 40px、行高
# 1.0，上方留 6px，所以 46。
DOT_BOX_H = 46
DOT_D = 22

# 量出来的根高（做法同另外三张：根设 fit_content 渲一次读 PNG 高度）。
ROOT_H = 3018

# (年月, 标签, 那时做了什么, 之后改了什么做法)
EVENTS = [
    ("2020.03", "起点", "把第一篇发出去",
     "写了两周、改了七版。起作用的是按下发布那一下。"),
    ("2020.11", "转折", "把更新时间写进日历",
     "固定每周三更新之后，比靠状态更新多撑了半年。"),
    ("2021.06", "第一次被看见", "一篇只回答一个问题",
     "放弃「什么都写一点」，一篇讲清一件事，转发的人立刻多起来。"),
    ("2022.02", "开始省时间", "把重复的版式存成模板",
     "同一个版式做到第八次才存模板。第二次就该存。"),
    ("2023.05", "第一笔收入", "先做出来，再定价",
     "免费发满 40 期之后才收费，报价按自己做一期的实际时长算。"),
    ("2024.09", "停更三个月", "停之前先发一条说明",
     "说清楚什么时候回来，回来时读者还在 —— 消失才是真的掉人。"),
    ("2026.01", "现在", "一周一张，一天只干一件",
     "选题、做图、发布拆到三天，每天只碰一件事，出错的地方少了一多半。"),
]


def band(name, *, fill, pad, gap, children, align="start"):
    """一个通栏区块。fill 决定它是不是一块有颜色的带 —— 结构容器不写 fill。"""
    node = frame(ids, name, width="fill_container", height="fit_content",
                 layout="vertical", padding=pad, gap=gap, alignItems=align,
                 fill=fill)
    node["children"] = children
    return node


def col(name, children, *, gap=16, width="fill_container", align="start",
        height="fit_content", **props):
    node = frame(ids, name, width=width, height=height, layout="vertical",
                 gap=gap, alignItems=align, fill=[], **props)
    node["children"] = children
    return node


def row(name, children, *, gap=24, align="center", width="fill_container",
        **props):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="horizontal", gap=gap, alignItems=align, fill=[],
                 **props)
    node["children"] = children
    return node


def chip(label, *, bg, fg, size=24):
    node = frame(ids, "胶囊", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[10, 22], cornerRadius=999,
                 alignItems="center", justifyContent="center", fill=solid(bg))
    node["children"] = [
        text(ids, "胶囊文字", label, size, 600, fg, family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
    ]
    return node


def section_head(title, note):
    """区块标题。一条短强调线 + 标题 + 一句说明，两处一模一样。"""
    return col("区块头", [
        rect(ids, "强调短线", width=72, height=8, cornerRadius=999,
             fill=solid("$c-accent")),
        text(ids, "区块标题", title, 46, 700, "$c-ink", family=CJK,
             line_height=LH_HEAD),
        text(ids, "区块说明", note, 27, 400, "$c-muted", family=CJK,
             line_height=LH_BODY),
    ], gap=16)


# ------------------------------------------------------------------ 01 页头
def header():
    """唯一的浅色区块。整档四张里只有这张的页头是亮的 —— 反相是它的标识。"""
    return band("01 页头", fill=solid("$c-band"), pad=[76, EDGE, 68, EDGE],
                gap=26, children=[
        chip("大事记 · 六年七件事", bg="$c-band-ink", fg="$c-band"),
        text(ids, "主标题", "一条线\n看完这六年", 76, 700, "$c-band-ink",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.4),
        text(ids, "副标题", "每一行都记一件具体做过的事，以及那之后改了什么。",
             28, 400, "$c-band-muted", family=CJK, line_height=LH_BODY),
    ])


# ------------------------------------------------------------------ 02 轴
def axis_column(*, hollow=False, tail=True):
    """年份与卡片之间那一列：一个点 + 一条竖线。

    竖线写 `height: fill_container`，而这一列在 `alignItems="stretch"` 的
    行里被拉到整行高 —— 于是卡片长高多少，这一段轴就长高多少。行与行之间
    gap 必须是 0，否则轴会断成七截（这就是 steps-flow 放弃画竖线的那个问
    题，在「轴属于行内部」这个前提下才有解）。
    """
    dot = rect(ids, "轴点", width=DOT_D, height=DOT_D, cornerRadius=999,
               fill=[] if hollow else solid("$c-accent"))
    if hollow:
        dot["stroke"] = {"thickness": 4, "fill": solid("$c-accent")}
    dot_box = frame(ids, "轴点格", width=AXIS_W, height=DOT_BOX_H,
                    layout="horizontal", alignItems="center",
                    justifyContent="center", fill=[])
    dot_box["children"] = [dot]
    line = rect(ids, "轴线", width=3, height="fill_container",
                fill=solid("$c-rail") if tail else [])
    return col("轴列", [dot_box, line], gap=0, width=AXIS_W,
               height="fill_container", align="center")


def event_row(when, tag, title, desc, *, last=False):
    year_col = col("年份列", [
        text(ids, "年月", when, 38, 700, "$c-accent", family=NUM,
             align="right", line_height=1.0, spacing=-1),
        text(ids, "阶段标签", tag, 23, 500, "$c-muted", family=CJK,
             align="right", line_height=1.4),
    ], gap=10, width=YEAR_W, align="end")

    card = col("事件卡", [
        text(ids, "事件标题", title, 34, 600, "$c-ink", family=CJK,
             line_height=LH_HEAD),
        text(ids, "事件说明", desc, 27, 400, "$c-muted", family=CJK,
             line_height=LH_BODY),
    ], gap=12, padding=[26, 30], cornerRadius=20)
    card["fill"] = solid("$c-surface")

    # 行间距做在卡片外层的下内边距上，而不是行与行的 gap 上 —— gap 会在轴
    # 上留出七道缺口。
    slot = col("事件槽", [card], gap=0, padding=[0, 0, 0 if last else 30, 0])

    return row("轴行", [year_col, axis_column(tail=not last), slot],
               gap=ROW_GAP, align="stretch")


def next_row():
    """收尾那一行：空心点、无轴尾。它标的是「还没发生」。"""
    year_col = col("年份列", [
        text(ids, "年月", "下一站", 34, 700, "$c-muted", family=CJK,
             align="right", line_height=1.0),
    ], gap=10, width=YEAR_W, align="end")

    card = col("待续卡", [
        text(ids, "待续标题", "这条线可以换成你自己的", 34, 600, "$c-ink",
             family=CJK, line_height=LH_HEAD),
        text(ids, "待续说明",
             "换掉年份和事件即可，复制一行轴会自己接上。",
             27, 400, "$c-muted", family=CJK, line_height=LH_BODY),
    ], gap=12, padding=[26, 30], cornerRadius=20)
    card["fill"] = []
    card["stroke"] = {"thickness": 2, "fill": solid("$c-rail"),
                      "dashPattern": [10, 8]}

    return row("待续行", [year_col, axis_column(hollow=True, tail=False), card],
               gap=ROW_GAP, align="stretch")


def timeline():
    rows = [event_row(*entry, last=False) for entry in EVENTS]
    rows.append(next_row())
    return band("02 时间轴", fill=[], pad=[68, EDGE, 0, EDGE], gap=36,
                children=[
        section_head("从第一篇到现在",
                     "按时间从早到晚。跳着看也行，每一行都能单独读完。"),
        col("轴", rows, gap=0),
    ])


# ------------------------------------------------------------------ 03 收尾
def closing():
    lines = [
        ("把日期写下来，比记住它更有用。", "calendar-check"),
        ("六年里真正改变结果的只有三四件事，其余都是维持。", "filter"),
    ]
    items = []
    for line, glyph in lines:
        items.append(row("收尾项", [
            icon_font(ids, "图标", glyph, 28, "$c-accent"),
            text(ids, "收尾文字", line, 27, 500, "$c-ink", family=CJK,
                 line_height=LH_BODY),
        ], gap=16, align="start"))
    panel = col("收尾面板", items, gap=18, padding=[36, 34], cornerRadius=22)
    panel["fill"] = solid("$c-surface")
    return band("03 收尾", fill=[], pad=[64, EDGE, 64, EDGE], gap=32,
                children=[
        section_head("回看时才看得出来的两件事",
                     "当时都不觉得是转折，是排成一条线之后才认出来的。"),
        panel,
    ])


# ------------------------------------------------------------------ 04 页脚
def footer():
    return band("04 页脚", fill=solid("$c-band"), pad=[44, EDGE], gap=12,
                children=[
        text(ids, "口径", "时间以公开发布的那天计，同月多件只留改变做法的那件。",
             24, 400, "$c-band-muted", family=CJK, line_height=1.6),
        row("署名行", [
            text(ids, "账号名", "@ 你的账号名", 26, 600, "$c-band-ink",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
            text(ids, "更新说明", "每年更新一次这张图", 24, 400,
                 "$c-band-muted", family=CJK, width="fit_content",
                 growth="auto", line_height=1.4),
        ], gap=16, width="fill_container"),
    ])


def build():
    page = frame(ids, "时间线大事记长图", width=W, height=ROOT_H,
                 layout="vertical", gap=0, fill=solid("$c-bg"),
                 clipContent=True)
    page["children"] = [header(), timeline(), closing(), footer()]
    page["x"], page["y"] = 0, 0
    return [page]


# 对比度（WCAG 相对亮度比，op-design-lint 的门槛是 2.0；数值实测）：
#   c-ink        on c-bg      16.46   c-muted      on c-bg      8.41
#   c-ink        on c-surface 13.80   c-muted      on c-surface 7.05
#   c-accent     on c-bg       6.47   c-accent     on c-surface 5.43
#   c-band-ink   on c-band    13.99   c-band-muted on c-band     5.95
#   c-band-ink   on c-accent   5.94   c-rail       on c-bg       3.44
# 承载文字的最低一对是 5.43（surface 卡上的强调色，实际只用在 38px 年份
# 上）；正文最低是 7.05。c-rail 是 3px 轴线与虚线描边，非文字图形按 3.0
# 量，3.44 过线。换主色时先量 c-accent/c-bg 与 c-rail/c-bg 这两对。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "时间线大事记长图")
