#!/usr/bin/env python3
"""challenge-tracker-infographic.op — 30 天打卡挑战长图（1080×N 竖版）

信息图这一档的第九张，回答**「接下来三十天每天做什么」** —— 一张能存进相
册、每天划掉一格的打卡表。

### 最近邻差异（为什么它不是 steps-flow 拉长）

steps-flow 也讲「怎么做」，分界线是**做几次**，而这决定了三处结构：

  - **steps 是一次性的五步，这张是重复三十次的同一件事。** steps 每一步内
    容不同、必须按序完成；这张三十格内容完全相同，缺的不是顺序而是**次
    数**。所以它的主元素不是卡片队列，是**六列五行的格子阵** —— 一眼能看
    出「还剩多少」，这是卡片队列给不了的。
  - **steps 给每步一个耗时上限，这张给三个里程碑。** 三十天里真正需要说话
    的只有第 7、15、30 天，其余二十七天不该有任何文案 —— 空格子本身就是要
    填的东西。
  - **它是一张要被用坏的图。** 版式因此要留出「能划掉」的余地：格子做到
    140×120，手指在手机上也点得准；里程碑格反白，划掉之后仍认得出。

和其余七张的分工：data-report 讲「数字是多少」，story 讲「数字连起来说明
什么」，timeline 讲「怎么走到今天」，concept 讲「这两个词不是一回事」，
ranking 讲「先看这几个」，faq 讲「被问最多的」，pitfall 讲「别做什么」，
这张讲**「从明天起，连做三十天」**。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：打卡的隐喻是养一株活的东西 —— 每天一点，看得见长。鼠尾草绿
    （~130°，黄绿一侧）是这个隐喻的自然色，而且它离 data-report 的青绿
    （~170°，蓝绿一侧）有 40°，两张并排不会认错。
  - **收敛**：一个色相 + 一条暖白到墨绿的明度序列。绿只出现在三处：三格里
    程碑、区块短线、页头胶囊。二十七个普通格子一律白底描边。
  - **论证**：格子阵最怕「全都有颜色」—— 三十格若都上绿，「今天到第几
    天」就没法一眼读出。所以颜色在这里承担的是**锚点**而不是装饰：只有三
    格有色，读者数格子时以它们为参照。

### 负约束（本模板明令不做的事）

  - **不给三十格填任何进度色、不做已完成/未完成两态。** 这是一张空表，进
    度由用户自己划。预填进度会让它变成截图，而不是工具。
  - 不画连接线、不画箭头、不做日历的星期表头 —— 挑战从任意一天开始都成
    立，钉死星期会把它变成某一个月的专属。
  - 不用第二个有彩色。鼠尾草绿之外只有中性明度序列。
  - 不用蓝紫渐变、霓虹线条、复杂背景纹理，不用 emoji 当图标、不用伪 3D。
  - 规则不超过三条，里程碑不超过三个；想加就换掉现有的，不叠加。
  - 不写 AI 套话（「自律给你自由 / 坚持就是胜利」），每条规则都要能在断掉
    的那天照着执行。

硬契约：
  - 内容距边缘 ≥80px（这里 80）
  - 配色全部走 color_vars，改主色只改 $c-accent / $c-accent-deep 两处
  - 正文与背景对比度 ≥2.0（本配色最低一对见文件末尾注释）
  - **CJK 行高比西文全线高 0.2**：页头大标题 1.2，区块标题 1.3，正文 1.7
  - **CJK 负字距不超过 -0.02em**；只有西文日号沿用西文 display 的收紧
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
    "c-bg":          "#F2F6EF",
    "c-surface":     "#FFFFFF",
    "c-band":        "#1B2A1C",
    "c-band-muted":  "#A3B49E",
    "c-ink":         "#17231A",
    "c-muted":       "#55665A",
    "c-accent":      "#4C8A3F",
    # 浅绿底上小字与里程碑说明用的深一档。主强调色在 #E2EFDC 上只有 3.5，
    # 够 34px 的日号不够 27px 的正文。
    "c-accent-deep": "#3A6B30",
    "c-accent-soft": "#E2EFDC",
    "c-border":      "#DDE7D8",
    # 格子的描边。比 c-border 深一档 —— 三十个格子并排时，1.27:1 的描边会
    # 让整块阵看起来是一片空白（第一版实测如此）。
    "c-grid":        "#C2D2BC",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W = 1080
EDGE = 80
INNER = W - EDGE * 2

LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.3, 1.7

# 格子阵：六列五行共三十格。格宽是算出来的，不是拍的。
GRID_COLS = 6
GRID_GAP = 16
CELL_W = (INNER - GRID_GAP * (GRID_COLS - 1)) // GRID_COLS
CELL_H = 120
TOTAL_DAYS = 30

# 需要说话的三天。其余二十七天不给任何文案 —— 空格子本身就是要填的东西。
MILESTONES = {7, 15, 30}

# 量出来的根高（做法同同档另外八张：根设 fit_content 渲一次读 PNG 高度）。
ROOT_H = 2692

RULES = [
    ("固定同一个时间", "哪怕只做五分钟。时间固定了，要不要做就不用每天重想。",
     "alarm-clock"),
    ("做完立刻划掉", "不补昨天不提前",
     "check"),
    ("断了不重来", "漏一天就往下走，别从第一天重来。",
     "refresh-cw"),
]

MILESTONE_NOTES = [
    ("第 7 天", "第一次想放弃通常在这天。把当天的量减到一半，但别停。"),
    ("第 15 天", "过半了。回头看第 1 天做的东西，差距就是继续的理由。"),
    ("第 30 天", "挑三件这三十天做出来的东西写下来，再决定要不要续下一轮。"),
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
    return band("01 页头", fill=solid("$c-band"), pad=[76, EDGE, 68, EDGE],
                gap=26, children=[
        chip("打卡挑战 · 30 天", bg="$c-accent", fg="$c-surface"),
        text(ids, "主标题", "三十天\n只做同一件事", 76, 700, "$c-surface",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.4),
        text(ids, "副标题", "把这张图存进相册，做完一天划掉一格，别的都不用记。",
             28, 400, "$c-band-muted", family=CJK, line_height=LH_BODY),
    ])


# ------------------------------------------------------------------ 02 规则
def rule_card(title, desc, glyph):
    card = col("规则卡", [
        icon_font(ids, "规则图标", glyph, 32, "$c-accent-deep"),
        text(ids, "规则标题", title, 28, 600, "$c-ink", family=CJK,
             line_height=1.4),
        text(ids, "规则说明", desc, 25, 400, "$c-muted", family=CJK,
             line_height=LH_BODY),
    ], gap=12, padding=[28, 26], cornerRadius=20)
    card["fill"] = solid("$c-surface")
    card["stroke"] = {"thickness": 2, "fill": solid("$c-border")}
    card["height"] = "fill_container"
    return card


def rules():
    grid = row("规则网格", [rule_card(*entry) for entry in RULES], gap=18,
               align="stretch")
    return band("02 规则", fill=[], pad=[64, EDGE, 0, EDGE], gap=32,
                children=[
        section_head("开始之前，三条规则",
                     "三条都是为了「断掉那天还能接着走」写的，不是为了自律。"),
        grid,
    ])


# ------------------------------------------------------------------ 03 格子
def day_cell(day):
    """一格。里程碑三格反白，其余二十七格白底描边 —— 表是空的，进度用户自己划。"""
    milestone = day in MILESTONES
    cell = frame(ids, f"第 {day} 天", width=CELL_W, height=CELL_H,
                 layout="horizontal", alignItems="center",
                 justifyContent="center", cornerRadius=16,
                 fill=solid("$c-accent" if milestone else "$c-surface"))
    if not milestone:
        cell["stroke"] = {"thickness": 2, "fill": solid("$c-grid")}
    cell["children"] = [
        text(ids, "日号", str(day), 34, 700,
             "$c-surface" if milestone else "$c-muted", family=NUM,
             width="fit_content", growth="auto", line_height=1.0,
             spacing=-1),
    ]
    return cell


def grid():
    rows = []
    for start in range(1, TOTAL_DAYS + 1, GRID_COLS):
        cells = [day_cell(day)
                 for day in range(start, start + GRID_COLS)]
        rows.append(row(f"第 {start} 行", cells, gap=GRID_GAP))
    return band("03 格子", fill=[], pad=[68, EDGE, 0, EDGE], gap=32,
                children=[
        section_head("三十格，一天一格",
                     "深色那三格是要停下来看一眼的日子，说明在下一节。"),
        col("格子阵", rows, gap=GRID_GAP),
    ])


# ------------------------------------------------------------------ 04 节点
def milestones():
    items = []
    for label, note in MILESTONE_NOTES:
        if items:
            items.append(rect(ids, "节点分割线", width="fill_container",
                              height=2, fill=solid("$c-border")))
        items.append(row("节点项", [
            text(ids, "节点日", label, 27, 700, "$c-accent-deep", family=CJK,
                 width=140, line_height=1.4),
            text(ids, "节点说明", note, 27, 400, "$c-ink", family=CJK,
                 line_height=LH_BODY),
        ], gap=20, align="start"))
    panel = col("节点面板", items, gap=20, padding=[36, 34], cornerRadius=22)
    panel["fill"] = solid("$c-accent-soft")
    return band("04 节点", fill=[], pad=[68, EDGE, 68, EDGE], gap=32,
                children=[
        section_head("三个要停一下的日子",
                     "其余二十七天不需要任何说明 —— 照着做就行。"),
        panel,
    ])


# ------------------------------------------------------------------ 05 页脚
def footer():
    return band("05 页脚", fill=solid("$c-band"), pad=[44, EDGE], gap=12,
                children=[
        text(ids, "用法", "存图后用标注工具划格；换挑战只需改标题和三条规则。",
             24, 400, "$c-band-muted", family=CJK, line_height=1.6),
        row("署名行", [
            text(ids, "账号名", "@ 你的账号名", 26, 600, "$c-surface",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
            text(ids, "更新说明", "每月开一轮新的三十天", 24, 400,
                 "$c-band-muted", family=CJK, width="fit_content",
                 growth="auto", line_height=1.4),
        ], gap=16, width="fill_container"),
    ])


def build():
    page = frame(ids, "30 天打卡挑战长图", width=W, height=ROOT_H,
                 layout="vertical", gap=0, fill=solid("$c-bg"),
                 clipContent=True)
    page["children"] = [header(), rules(), grid(), milestones(), footer()]
    page["x"], page["y"] = 0, 0
    return [page]


# 对比度（WCAG 相对亮度比，op-design-lint 的门槛是 2.0；数值实测）：
#   c-ink         on c-bg          14.87   c-muted      on c-bg          5.59
#   c-ink         on c-surface     16.26   c-muted      on c-surface     6.11
#   c-surface     on c-band        15.06   c-band-muted on c-band        6.87
#   c-accent-deep on c-surface      6.31   c-accent     on c-surface     4.19
#   c-accent-deep on c-accent-soft  5.30   c-ink        on c-accent-soft 13.64
#   c-surface     on c-accent       4.19   c-muted      on c-accent-soft 5.13
#   c-grid        on c-surface      1.59   c-border     on c-surface     1.27
# 承载正文的最低一对是 5.13。c-surface on c-accent 的 4.19 只出现在三格里
# 程碑的 34px 粗体日号上（AA 对 ≥24px 粗体的门槛是 3.0，余量充足）；
# c-accent 本身从不承载小字。c-grid 是二十七个空格子的 2px 描边（非文字图
# 形），它只需要把格子分出来，1.59 在浅底上已经成阵。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "30 天打卡挑战长图")
