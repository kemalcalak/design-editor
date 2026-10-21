#!/usr/bin/env python3
"""spec-table-comparison.op — 参数表对比长图（1080×N 竖版）

对比档的「密集数据」一张：两个候选放在一张真表里逐行比，赢的一格用**深底
反白**顶起来，一眼扫下来就知道各自赢在哪几行。

### 最近邻论证（为什么它不是已有的哪一张）

  - **before-after（1600×900 横版）**：那张比的是**同一个东西的两个时刻**，
    主体是两张截图，文字只做注解。这张比的是**两个不同的东西**，主体是
    一张参数表，没有图片位。
  - **pitfall-list-infographic（避坑排行）**：那张是「一列条目按严重度排
    序」，单位是**一条**；这张的单位是**一行两格**，同一指标必须在同一行
    上被两次回答——这正是对比档和清单档的分界。
  - **本批 03 三方案横评**：那张是三张并列卡片（各自讲自己的定位），这张是
    行优先的表（同一指标横着比）。卡片比气质，表比数值。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：从「参数表」这件事本身采——它属于说明书、规格页、比价页，那
    类版面的记忆色是纸白配深墨蓝，不是任何品牌色。
  - **收敛**：单色相靛蓝（H≈232°）的一条七级明度序列 L 0.11 / 0.20 / 0.38 /
    0.63 / 0.88 / 0.92 / 1.0，chroma 随明度升高而降低（深处 0.05、浅处
    0.01），只有一个色相。
  - **论证**：表格的层级全部由「行/列/格」承担，颜色一旦引入第二个色相，
    读者会立刻把它当成语义（红=差、绿=好），而参数表恰恰不该替读者下这个
    判断——1.24kg 和 1.62kg 谁「好」取决于他背不背着走。所以赢的一格只用
    **明度**顶起来（深底反白 = 更重），不用色相说好坏。

### 负约束（本模板明令不做的事）

  - **不用第二个色相。** 整张图只有靛蓝一族 + 中性白，破了它「不替读者判
    断好坏」这条论证就没了。
  - 不用红绿对错配色。参数表最容易滑向「A 列绿 B 列红」，那是价值判断，不
    是参数。
  - 不给赢的一格打勾/星/奖杯图标。图标是结论，表格只出示事实。
  - 不画分隔线节点。格线由「表体底色从单元格缝隙透出」实现，多一层描边就
    会和缝隙对不齐。
  - 不用蓝紫渐变、霓虹线条、伪 3D，不用 emoji 当图标，不用阴影。
  - 一格文案不超过 10 个字：装不下就说明该拆成两行指标，不缩字号。
  - 结尾不硬凑「A 胜出」。四比四就写四比四，再给两句「什么情况选谁」。

硬契约：
  - 内容距边缘 ≥64px（这里 64）
  - 配色全部走 color_vars；单色序列，改一处色相即整张换肤
  - 正文与背景对比度 ≥2.0（最低一对见文件末尾实测表）
  - **CJK 行高**：页头大标题 1.2，区块标题 1.3，正文 1.7
  - **CJK 负字距不超过 -0.02em**；数字与拉丁走 Inter
  - 三列宽度写死并且必须加起来等于 INNER，否则表尾会溢出
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
    "c-bg":        "#F1F2F7",
    "c-card":      "#FFFFFF",
    "c-tint":      "#E6E8F1",
    "c-line":      "#D5D8E4",
    "c-ink":       "#12162A",
    "c-deep":      "#242B4C",
    "c-muted":     "#4E5573",
    # 反白页面上的次级文字：同一个灰压在近黑上会掉到 4 以下，深浅两套次级色
    # 是必要的（做法同 pitfall-list）。
    "c-inv-muted": "#A9AEC2",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W = 1080
EDGE = 64
INNER = W - EDGE * 2          # 952

# 三列宽度 + 两条纵向格线必须正好等于 INNER。表格没有自动列宽，写错就溢出。
#
# GRID 是格线宽度，同时兼两个职责：视觉上它是表格的分隔线（表体底色 c-line
# 从单元格之间透出来），几何上它让相邻单元格拉开 4px —— 几何检测器
# （geometry_spill_diagnostics 的 SIBLING_JAM_GAP=3）把 0px 相邻的两个
# 文本单元格判成「读起来连成一个词」的 jam。真格线正好一并解决两件事。
# 列宽 = INNER 扣掉表体两侧的 GRID 内边距，再扣掉两条纵向格线。
GRID = 4
COL_LABEL, COL_A, COL_B = 276, 330, 330
assert COL_LABEL + COL_A + COL_B + GRID * 4 == INNER

LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.3, 1.7

# 量出来的根高（根设 fit_content 渲一次读 PNG 高度，见文件末尾）。
ROOT_H = 2405

A_NAME, A_TAG = "轻装派", "13.6 英寸 · 1.24kg"
B_NAME, B_TAG = "生产力", "16 英寸 · 1.62kg"

# (指标, A 值, B 值, 谁更强)。"-" 表示这一行不判胜负。
# 每格 ≤10 字：三列都是定宽，超了会换行把行高顶起来，表就不齐了。
SPECS = [
    ("整机重量", "1.24 kg", "1.62 kg", "a"),
    ("实测续航", "11 小时", "7 小时", "a"),
    ("屏幕",     "60Hz 标准屏", "120Hz 高刷", "b"),
    ("视频剪辑", "1080p 顺畅", "4K 不掉帧", "b"),
    ("接口",     "2 个 USB-C", "3 口带读卡器", "b"),
    ("满载噪音", "几乎听不见", "能听见风扇", "a"),
    ("到手价",   "8,999 元", "13,499 元", "a"),
    ("保修",     "一年质保", "两年含上门", "b"),
]

# 两句「什么情况选谁」。四比四的表必须给它，否则读者拿不走结论。
# 每句 ≤14 字：建议卡内容宽 410px，26px 中文一行约 15 字，写到 16 字就会把
# 句号单独甩到第二行变成孤字。
PICKS = [
    ("选 " + A_NAME, "常在路上，写字和开会为主。"),
    ("选 " + B_NAME, "固定工位，剪片调色，外设多。"),
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


def row(name, children, *, gap=0, align="stretch", width="fill_container",
        **props):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="horizontal", gap=gap, alignItems=align, fill=[],
                 **props)
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
        text(ids, "区块标题", title, 44, 700, "$c-ink", family=CJK,
             line_height=LH_HEAD),
        text(ids, "区块说明", note, 26, 400, "$c-muted", family=CJK,
             line_height=LH_BODY),
    ], gap=16)


# ------------------------------------------------------------------ 01 页头
def header():
    return band("01 页头", fill=solid("$c-ink"), pad=[76, EDGE, 64, EDGE],
                gap=26, children=[
        tag("参数对比 · 选购向", bg="$c-card", fg="$c-ink"),
        text(ids, "主标题", "两台机器\n到底差在哪", 76, 700, "$c-card",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.4),
        text(ids, "副标题",
             "八项常用指标逐行摆开，深色那一格是这一行里更强的一方。",
             27, 400, "$c-inv-muted", family=CJK, line_height=LH_BODY),
    ])


# ---------------------------------------------------------------- 02 候选者
def candidate(name, spec, inverted):
    """候选者名片。两张一模一样，只有明度不同——不给任何一方配色相。"""
    ink = "$c-card" if inverted else "$c-ink"
    sub = "$c-inv-muted" if inverted else "$c-muted"
    card = col("候选者", [
        text(ids, "候选者名", name, 40, 700, ink, family=CJK,
             line_height=LH_HEAD),
        text(ids, "候选者规格", spec, 25, 400, sub, family=NUM,
             line_height=1.5),
    ], gap=10, width=(INNER - 20) // 2, padding=[28, 30])
    card["fill"] = solid("$c-deep" if inverted else "$c-card")
    if not inverted:
        card["stroke"] = {"thickness": 2, "fill": solid("$c-line")}
    return card


def candidates():
    return band("02 候选者", fill=[], pad=[56, EDGE, 0, EDGE], gap=0,
                children=[
        row("候选者行", [
            candidate(A_NAME, A_TAG, False),
            candidate(B_NAME, B_TAG, True),
        ], gap=20, align="stretch"),
    ])


# ------------------------------------------------------------------ 03 参数表
def cell(content, width_px, *, weight, color, fill, family=CJK, first=False):
    box = frame(ids, "单元格", width=width_px, height="fit_content",
                layout="vertical", padding=[20, 22], gap=0,
                justifyContent="center", fill=fill)
    box["children"] = [
        text(ids, "单元格文字", content, 27 if first else 28,
             weight, color, family=family, line_height=1.45),
    ]
    return box


def head_row():
    cells = [
        cell("指标", COL_LABEL, weight=600, color="$c-inv-muted",
             fill=solid("$c-deep"), first=True),
        cell(A_NAME, COL_A, weight=700, color="$c-card",
             fill=solid("$c-deep")),
        cell(B_NAME, COL_B, weight=700, color="$c-card",
             fill=solid("$c-deep")),
    ]
    return row("表头", cells, gap=GRID, align="stretch")


def spec_row(label, va, vb, winner, zebra):
    """一行两格。赢的一格换成深底反白——层级由明度给，不由色相给。"""
    base = solid("$c-tint") if zebra else solid("$c-card")
    win_fill = solid("$c-deep")
    cells = [
        cell(label, COL_LABEL, weight=600, color="$c-muted", fill=base,
             first=True),
        cell(va, COL_A, weight=700 if winner == "a" else 400,
             color="$c-card" if winner == "a" else "$c-ink",
             fill=win_fill if winner == "a" else base, family=NUM),
        cell(vb, COL_B, weight=700 if winner == "b" else 400,
             color="$c-card" if winner == "b" else "$c-ink",
             fill=win_fill if winner == "b" else base, family=NUM),
    ]
    return row("参数行", cells, gap=GRID, align="stretch")


def table():
    rows = [head_row()]
    rows += [spec_row(label, va, vb, winner, index % 2 == 1)
             for index, (label, va, vb, winner) in enumerate(SPECS)]
    # 表体底色就是格线色：单元格之间留出的 GRID 缝隙让它透出来，于是格线不是
    # 画上去的，而是「没被单元格盖住的底」——少一层节点，也永远对得齐。
    grid = col("表体", rows, gap=GRID)
    grid["fill"] = solid("$c-line")
    grid["padding"] = [GRID, GRID]
    grid["clipContent"] = True
    grid["cornerRadius"] = 4
    return band("03 参数表", fill=[], pad=[56, EDGE, 0, EDGE], gap=28,
                children=[
        section_head("八行看完", "深色格 = 这一行里更强的那一台，只说事实。"),
        grid,
    ])


# ------------------------------------------------------------------ 04 结论
def tally():
    wins_a = sum(1 for _, _, _, w in SPECS if w == "a")
    wins_b = sum(1 for _, _, _, w in SPECS if w == "b")
    score = row("比分行", [
        text(ids, "比分", f"{wins_a} : {wins_b}", 88, 700, "$c-ink",
             family=NUM, width="fit_content", growth="auto", line_height=1.0,
             spacing=-2),
        text(ids, "比分说明",
             f"{A_NAME}赢四行，{B_NAME}赢四行——这张表不替你选。",
             27, 400, "$c-muted", family=CJK, line_height=LH_BODY),
    ], gap=28, align="center")

    picks = []
    for title, why in PICKS:
        head = text(ids, "选谁", title, 32, 700, "$c-ink", family=CJK,
                    width="fit_content", growth="auto", line_height=LH_HEAD)
        item = col("建议", [
            head,
            text(ids, "理由", why, 26, 400, "$c-muted", family=CJK,
                 line_height=LH_BODY),
        ], gap=10, width="fill_container", padding=[26, 28])
        item["fill"] = solid("$c-card")
        item["stroke"] = {"thickness": 2, "fill": solid("$c-line")}
        picks.append(item)

    return band("04 结论", fill=[], pad=[56, EDGE, 64, EDGE], gap=28,
                children=[
        section_head("四比四", "指标数不是答案，你的使用方式才是。"),
        score,
        row("建议行", picks, gap=20, align="stretch"),
    ])


# ------------------------------------------------------------------ 05 页脚
def footer():
    return band("05 页脚", fill=solid("$c-ink"), pad=[44, EDGE], gap=12,
                children=[
        text(ids, "结语", "比参数之前，先写下你每天真正在做的三件事。",
             30, 600, "$c-card", family=CJK, line_height=LH_HEAD),
        row("署名行", [
            text(ids, "账号名", "@ 你的账号名", 26, 600, "$c-card",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
            text(ids, "更新说明", "每周一张能照着抄的对比表", 24, 400,
                 "$c-inv-muted", family=CJK, width="fit_content",
                 growth="auto", line_height=1.4),
        ], gap=16, width="fill_container"),
    ])


def build():
    page = frame(ids, "参数表对比长图", width=W, height=ROOT_H,
                 layout="vertical", gap=0, fill=solid("$c-bg"),
                 clipContent=True)
    page["children"] = [header(), candidates(), table(), tally(), footer()]
    page["x"], page["y"] = 0, 0
    return [page]


# 对比度（WCAG 相对亮度比，op-design-lint 门槛 2.0；数值由 contrast.py 实测）：
#   c-ink  on c-bg    16.00   c-muted on c-bg      6.54
#   c-ink  on c-card  17.89   c-muted on c-card    7.31
#   c-ink  on c-tint  14.63   c-muted on c-tint    5.98
#   c-card on c-deep  13.77   c-inv-muted on c-deep 6.24
#   c-card on c-ink   17.89   c-inv-muted on c-ink 8.11
# 承载正文的最低一对是 5.98（斑马行里的指标名）。整张图只有靛蓝
# 一个色相，赢的格靠明度顶起来——换肤只需整体旋转这一族的色相。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "参数表对比长图")
