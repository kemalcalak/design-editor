#!/usr/bin/env python3
"""data-report-infographic.op — 数据结论信息长图（1080×N 竖版）

「信息图」这一档和卡片的区别不是画幅，是**阅读方式**：卡片是一眼看完，
长图是从上往下滑着读完。所以这张图的结构是一条阅读动线 ——

    深色页头（结论先行）→ 三个大数 → 横向对比条 → 构成占比 → 三条结论 → 出处

—— 而不是卡片那种「标题 + 要点 + 署名」的静态版式。

图表纪律（dataviz）在这张图里的具体落法：
  - **一个强调色，只给要说的那一条。** 对比条里只有第一名走青绿渐变，其
    余四条统一走中性轨道色。五条都上色等于没上色。
  - **不画图例，直接标在图上。** 数值贴在条的右端，占比标在色块下方，读
    者的视线不需要在图和图例之间来回跳。
  - **不画坐标轴和网格线。** 五条带标签的横条本来就自带比较基准，轴线只
    是噪声。
  - **数字用等宽感的 Inter，文案用 Noto Sans SC。** 全图两个字族封顶。

硬契约：
  - 内容距边缘 ≥80px（这里 80）
  - 配色全部走 color_vars，改主色只改 $c-accent / $c-accent-deep 两处
  - 正文与背景对比度 ≥2.0（本配色最低一对见文件末尾注释）
  - **CJK 行高比西文全线高 0.2**：页头大标题 1.2，区块标题 1.3，正文 1.7
  - **CJK 负字距不超过 -0.02em**（汉字是满格设计，再负就笔画相撞）；只有
    西文数字沿用西文 display 的收紧
  - 汉字走 Noto Sans SC，数字与拉丁走 Inter —— 等价于中文排印规范里
    「西文在前、中文在后」的 fallback 链，只是在 .op 里按节点写死
  - 顶层 frame 必须显式写 x/y，否则多帧会全部堆在原点
  - 根高固定：ROOT_H 是量出来的（见文件末尾），改内容后要重量一次

### 负约束（本模板明令不做的事）

  - 不画坐标轴、网格线、图例。带标签的横条自带比较基准，其余都是噪声。
  - 不给第二条数据上强调色。五条里只有第一名走渐变，「都强调 = 没强调」。
  - 不用蓝紫渐变、霓虹线条、复杂背景纹理（廉价 AI 科技风的三件套）。
  - 不用伪 3D 图标、不用 emoji 当图标、不用装饰性插画。
  - 不靠压字号硬塞：信息密度高时加区块，不缩字。
  - 不写 AI 套话（「赋能 / 洞察闭环 / 一站式」），结论写成能直接排进日程的动作。
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import (Ids, color_vars, frame, icon_font, linear, rect, solid,
                   text, write_doc)

ids = Ids()

VARS = color_vars({
    "c-bg":          "#F4F7F6",
    "c-surface":     "#FFFFFF",
    # 页头与页脚的深底。近黑的墨绿而不是纯黑：它要和青绿强调色同一个色相家
    # 族，长图从头到尾才像一份东西。
    "c-band":        "#0C2E2B",
    "c-band-muted":  "#9FBDB8",
    "c-ink":         "#0E2523",
    "c-muted":       "#5A736F",
    "c-accent":      "#0E9F8B",
    # 小字压在浅底上时用的深一档。强调色本身在 #FFF 上只有 3.3:1，够大数字
    # 不够 24px 的胶囊文字 —— 分两个变量比让 lint 过关更重要。
    "c-accent-deep": "#096A5D",
    "c-accent-soft": "#DCF2ED",
    # 对比条的中性轨道 / 非强调条。它是「其余四条」的颜色，也是底槽的颜色，
    # 同一个值让「没被强调的部分」在视觉上退成一个整体。
    "c-track":       "#DDE7E5",
    "c-track-2":     "#B7C9C6",
    "c-border":      "#E1EAE8",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W = 1080
EDGE = 80
INNER = W - EDGE * 2

# 对比条的三段固定宽度：标签 / 轨道 / 数值。三个兄弟统一写死像素而不是混用
# fill_container —— 混用会让轨道长度随标签字数变化，五条就对不齐了。
BAR_LABEL_W = 210
BAR_VALUE_W = 116
BAR_GAP = 24
BAR_TRACK_W = INNER - BAR_LABEL_W - BAR_VALUE_W - BAR_GAP * 2
BAR_H = 46

# 量出来的根高。内容全是 fit_content，把根设成 fit_content 渲一次读 PNG 高
# 度，再把结果烤回来（同 saas-landing-orange 的 3240）。
ROOT_H = 2684


def band(name, *, fill, pad, gap, children, align="start"):
    """一个通栏区块。fill 决定它是不是一块有颜色的带 —— 结构容器不写 fill。"""
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
    """区块标题。一条短强调线 + 标题 + 一句说明，全图五处一模一样。"""
    return col("区块头", [
        rect(ids, "强调短线", width=72, height=8, cornerRadius=999,
             fill=solid("$c-accent")),
        text(ids, "区块标题", title, 46, 700, "$c-ink", family=CJK,
             line_height=1.3),
        text(ids, "区块说明", note, 27, 400, "$c-muted", family=CJK,
             line_height=1.7),
    ], gap=16)


# ------------------------------------------------------------------ 01 页头
def header():
    return band("01 页头", fill=solid("$c-band"), pad=[76, EDGE, 68, EDGE],
                gap=26, children=[
        chip("数据周报 · 2026 上半年", bg="$c-accent", fg="$c-band"),
        text(ids, "主标题", "三个数字\n说清这半年", 76, 700, "$c-surface",
             family=CJK, line_height=1.2, spacing=-1.4),
        text(ids, "副标题", "全部口径与统计区间写在图片最下方，可直接转发。",
             28, 400, "$c-band-muted", family=CJK, line_height=1.7),
    ])


# ------------------------------------------------------------------ 02 大数
KPIS = [
    ("2.4", "万", "月均活跃创作者", "+62%"),
    ("18", "分钟", "首次上手到出图", "−41%"),
    ("93", "%", "一次通过率", "+9pt"),
]


def kpi_card(value, unit, label, delta):
    """大数字卡。数值走强调色、单位退成中性 —— 一张卡里只有一个视觉重心。"""
    value_row = row("数值", [
        text(ids, "数值", value, 78, 700, "$c-accent-deep", family=NUM,
             width="fit_content", growth="auto", line_height=1.0, spacing=-3),
        text(ids, "单位", unit, 28, 600, "$c-muted", family=CJK,
             width="fit_content", growth="auto", line_height=1.0),
    ], gap=8, align="end", width="fit_content")

    card = col("大数卡", [
        value_row,
        text(ids, "指标名", label, 26, 600, "$c-ink", family=CJK,
             line_height=1.4),
        chip(delta, bg="$c-accent-soft", fg="$c-accent-deep", size=22),
    ], gap=16, padding=[32, 28], cornerRadius=20)
    card["fill"] = solid("$c-surface")
    card["stroke"] = {"thickness": 2, "fill": solid("$c-border")}
    card["height"] = "fill_container"
    return card


def kpis():
    grid = row("大数网格", [kpi_card(*entry) for entry in KPIS], gap=20,
               align="stretch")
    return band("02 大数", fill=[], pad=[64, EDGE, 0, EDGE], gap=32, children=[
        section_head("先看三个数",
                     "只挑能直接下结论的指标，其余都放在文末的口径说明里。"),
        grid,
    ])


# ------------------------------------------------------------------ 03 对比
# (标签, 数值文案, 占满轨道的比例)。比例是画出来的长度，数值是写出来的事实，
# 两者必须自己对齐 —— 没有任何一层会替你检查它们是否一致。
BARS = [
    ("模板起稿", "4.6k", 1.00),
    ("空白起稿", "2.9k", 0.63),
    ("导入截图", "1.7k", 0.37),
    ("复制他人", "0.9k", 0.20),
    ("其他入口", "0.4k", 0.09),
]


def bar_row(label, value, ratio, highlight):
    track = frame(ids, "轨道", width=BAR_TRACK_W, height=BAR_H,
                  layout="horizontal", alignItems="center", gap=0,
                  cornerRadius=BAR_H // 2, clipContent=True,
                  fill=solid("$c-track"))
    track["children"] = [
        rect(ids, "条", width=max(round(BAR_TRACK_W * ratio), BAR_H),
             height=BAR_H, cornerRadius=BAR_H // 2,
             # 90° = 自左向右：深端坐在起点，条尾收到主强调色。只有第一名用
             # 渐变，其余走中性 —— 这就是「一个强调色只给要说的那一条」。
             fill=(linear(90, [(0.0, "$c-accent-deep"), (1.0, "$c-accent")])
                   if highlight else solid("$c-track-2"))),
    ]
    return row("对比行", [
        text(ids, "条标签", label, 27, 500, "$c-ink", family=CJK,
             width=BAR_LABEL_W, line_height=1.4),
        track,
        text(ids, "条数值", value, 30, 700,
             "$c-accent-deep" if highlight else "$c-muted", family=NUM,
             width=BAR_VALUE_W, align="right", line_height=1.4),
    ], gap=BAR_GAP)


def comparison():
    rows = [bar_row(label, value, ratio, index == 0)
            for index, (label, value, ratio) in enumerate(BARS)]
    return band("03 对比", fill=[], pad=[72, EDGE, 0, EDGE], gap=32, children=[
        section_head("新图是从哪儿开始的",
                     "近 30 天新建文档的起稿方式，按数量从多到少。"),
        col("对比列表", rows, gap=22),
    ])


# ------------------------------------------------------------------ 04 构成
SPLIT = [
    ("社交卡片", "52%", 0.52, "$c-accent-deep"),
    ("演示文稿", "31%", 0.31, "$c-accent"),
    ("其他", "17%", 0.17, "$c-track-2"),
]


def composition():
    stacked = frame(ids, "占比条", width="fill_container", height=56,
                    layout="horizontal", gap=0, cornerRadius=28,
                    clipContent=True, fill=solid("$c-track"))
    # 最后一段吃掉四舍五入的余数：三段各自 round 之后总和会比 INNER 少 1-2
    # px，底槽就从右端露出一道细缝，看起来像「占比没加满 100%」。
    widths = [round(INNER * ratio) for _, _, ratio, _ in SPLIT]
    widths[-1] = INNER - sum(widths[:-1])
    stacked["children"] = [
        rect(ids, f"占比段 · {name}", width=width, height=56, fill=solid(color))
        for (name, _, _, color), width in zip(SPLIT, widths)
    ]

    legend_items = []
    for name, pct, _, color in SPLIT:
        legend_items.append(row("图例项", [
            rect(ids, "色点", width=18, height=18, cornerRadius=9,
                 fill=solid(color)),
            text(ids, "图例名", name, 26, 500, "$c-ink", family=CJK,
                 width="fit_content", growth="auto", line_height=1.4),
            text(ids, "图例值", pct, 26, 700, "$c-muted", family=NUM,
                 width="fit_content", growth="auto", line_height=1.4),
        ], gap=12, width="fit_content"))

    return band("04 构成", fill=[], pad=[72, EDGE, 0, EDGE], gap=32, children=[
        section_head("这些图最后变成了什么",
                     "按导出成品分类。一份文档只计一次，取最后一次导出。"),
        col("占比区", [
            stacked,
            row("图例", legend_items, gap=40, width="fill_container"),
        ], gap=24),
    ])


# ------------------------------------------------------------------ 05 结论
TAKEAWAYS = [
    "模板起稿是空白起稿的 1.6 倍，入口比数量更值得投入。",
    "首次出图时间压到 18 分钟之后，一次通过率才开始往上走。",
    "社交卡片占了一半以上的成品，但模板供给还集中在演示文稿。",
]


def takeaways():
    items = []
    for line in TAKEAWAYS:
        items.append(row("结论项", [
            icon_font(ids, "对勾", "check-circle", 30, "$c-accent-deep"),
            text(ids, "结论文字", line, 28, 500, "$c-ink", family=CJK,
                 line_height=1.7),
        ], gap=18, align="start"))

    panel = col("结论面板", items, gap=22, padding=[40, 36], cornerRadius=24)
    panel["fill"] = solid("$c-accent-soft")

    return band("05 结论", fill=[], pad=[72, EDGE, 76, EDGE], gap=32, children=[
        section_head("那么该做什么",
                     "三条能直接排进下个季度的动作，其余留给评论区。"),
        panel,
    ])


# ------------------------------------------------------------------ 06 页脚
def footer():
    return band("06 页脚", fill=solid("$c-band"), pad=[44, EDGE], gap=12,
                children=[
        text(ids, "出处", "数据口径：2026-01-01 至 2026-06-30，去重后按文档计。",
             24, 400, "$c-band-muted", family=CJK, line_height=1.6),
        row("署名行", [
            text(ids, "账号名", "@ 你的账号名", 26, 600, "$c-surface",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
            text(ids, "更新说明", "每月一张数据图", 24, 400, "$c-band-muted",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
        ], gap=16, width="fill_container"),
    ])


def build():
    page = frame(ids, "数据结论长图", width=W, height=ROOT_H, layout="vertical",
                 gap=0, fill=solid("$c-bg"), clipContent=True)
    page["children"] = [header(), kpis(), comparison(), composition(),
                        takeaways(), footer()]
    page["x"], page["y"] = 0, 0
    return [page]


# 对比度（WCAG 相对亮度比，op-design-lint 的门槛是 2.0；数值实测）：
#   c-surface     on c-band         14.57   c-band-muted on c-band         7.25
#   c-ink         on c-bg           14.90   c-muted      on c-bg           4.73
#   c-ink         on c-surface      16.06   c-muted      on c-surface      5.10
#   c-accent-deep on c-surface       6.50   c-band       on c-accent       4.41
#   c-accent-deep on c-accent-soft   5.56   c-ink        on c-accent-soft 13.74
# 最低一对 4.73 仍高于 WCAG AA 的 4.5。强调色本身 (#0E9F8B) 只出现在 78px
# 的大数、色块和 5px 的短线上，从不承载小字 —— 它在 #FFF 上只有 3.31。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "数据结论信息长图")
