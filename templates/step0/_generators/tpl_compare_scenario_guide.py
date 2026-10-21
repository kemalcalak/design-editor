#!/usr/bin/env python3
"""scenario-guide-comparison.op — 场景选择指南（1080×N 竖版长图）

对比档的「替你算完」那一张：不摆参数、不摆卡片，直接给七种处境，每一种后
面挂一个判定标签。读者不需要理解两个选项的全部差别——他只要在七行里找到
自己那一行。

### 最近邻论证（为什么它不是已有的哪一张）

  - **本批 01 参数表**：那张把两个候选的**属性**逐行摆开，读者自己综合。
    这张跳过属性，直接给**结论**。同样是两方对比，那张的行首是「指标」，
    这张的行首是「你的处境」——主语从物换成了人。
  - **本批 08 优缺点天平**：那张明确拒绝下结论，把笔交给读者。这张明确下
    结论，七行七个答案。两张是同一枚硬币的两面，所以必须都在这一档里。
  - **本批 03 三方案横评**：那张的判定藏在最后一节（三句话对号入座），是
    附录；这张把判定升成**正文本身**，附录反而是最后那条「两个都要」。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：两个选项需要各自一个能被反复认出的记号——七行里标签会出现
    七次，靠明度分不开两个都是深色的标签。
  - **收敛**：中性灰序列 + **两个**低饱和色相：靛蓝 #2C4A7C（模板）与赭石
    #8A5124（设计师）。两者在色相环上相距约 175°（互补），且都不在红绿轴
    上——红绿色盲最难分的是 L/M 锥体那一对，靛蓝与赭石在蓝黄轴上分开，是
    少数几种对三类色觉都成立的组合。
  - **论证**：这是本批唯一正当使用两个色相的模板——因为它唯一真正需要
    「同一个标签在七个位置被认出来」。其余模板要么只有一个判断（一个色就
    够），要么根本不该有判断（无彩）。

### 负约束（本模板明令不做的事）

  - **不用红绿。** 两个标签会在七行里反复出现，红绿色盲会把它们读成同一
    个标签，整张图归零。这条比其它任何模板都严格。
  - **只允许这两个色相。** 第三个选项要进来，先把它并进现有两个之一。
  - 不给任一方加「推荐 / 更好」。七行里两边各赢几行，是内容决定的。
  - 不写「无脑选 X」。每行都必须给一句为什么。
  - 不用蓝紫渐变、霓虹线条、伪 3D、阴影、emoji 图标。
  - 处境 ≤12 字，理由 ≤20 字。理由写不下说明这行其实是两种处境。
  - 结尾必须留一条「两个都要」。七选一的图不给例外，读者会觉得被骗。

硬契约：
  - 内容距边缘 ≥72px（这里 72）
  - 两个色相各自成对（c-a / c-a-soft、c-b / c-b-soft），标签底用 soft、
    字用深色——反白标签在小尺寸下会糊
  - 配色全部走 color_vars；换选项配色 = 改 c-a / c-b 两处
  - 正文与背景对比度 ≥2.0（最低一对见文件末尾实测表）
  - **CJK 行高**：大标题 1.2，区块标题 1.3，正文 1.6
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
    "c-bg":        "#F4F4F2",
    "c-card":      "#FFFFFF",
    "c-panel":     "#E7E7E4",
    "c-line":      "#D6D6D2",
    "c-ink":       "#14161A",
    "c-muted":     "#4E5359",
    "c-inv-muted": "#A5A9AF",
    # 选项 A：靛蓝。选项 B：赭石。两者在蓝黄轴上分开，对三类色觉都成立。
    "c-a":         "#2C4A7C",
    "c-a-soft":    "#E1E7F1",
    "c-b":         "#8A5124",
    "c-b-soft":    "#F2E7DC",
})

CJK = "Noto Sans SC"

W = 1080
EDGE = 72

LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.3, 1.6

ROOT_H = 2723

A_NAME, A_DESC = "套模板", "现成版式，改文案就能发"
B_NAME, B_DESC = "找设计师", "从零定制，含一轮沟通"

# (处境 ≤12 字, 选谁 "a"/"b", 理由 ≤20 字)
CASES = [
    ("三天内就要上线", "a", "设计师光第一稿就要三天。"),
    ("一次要出二十张", "a", "量大的时候，一致比独特值钱。"),
    ("这是公司的门面", "b", "第一印象只发生一次。"),
    ("预算五百以内", "a", "定制的起步价通常不止这个数。"),
    ("要一套长期能用的规范", "b", "规范是判断力，模板给不了。"),
    ("自己也说不清要什么", "b", "把需求问出来本身就是那份钱。"),
    ("每周都要发新的", "a", "长期高频，只有模板撑得住。"),
]

# 结尾的例外。七选一的图必须留这一条，否则读者会觉得被骗。
EXCEPTION = ("两个都要", "先请设计师做一套规范，再照着它套模板。")


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


def pill(label, *, bg, fg, size=25):
    node = frame(ids, "判定标签", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[10, 20], cornerRadius=8,
                 alignItems="center", justifyContent="center", fill=solid(bg))
    node["children"] = [
        text(ids, "判定文字", label, size, 700, fg, family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
    ]
    return node


def section_head(title, note):
    return col("区块头", [
        rect(ids, "强调短线", width=72, height=8, fill=solid("$c-ink")),
        text(ids, "区块标题", title, 44, 700, "$c-ink", family=CJK,
             line_height=LH_HEAD),
        text(ids, "区块说明", note, 26, 400, "$c-muted", family=CJK,
             line_height=1.7),
    ], gap=16)


# ------------------------------------------------------------------ 01 页头
def header():
    return band("01 页头", fill=solid("$c-ink"), pad=[76, EDGE, 64, EDGE],
                gap=26, children=[
        pill("场景指南 · 做图", bg="$c-card", fg="$c-ink", size=24),
        text(ids, "主标题", "这次该套模板\n还是找人做", 66, 700, "$c-card",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.3),
        text(ids, "副标题", "不用比参数——在下面七行里找到你现在这一行就行。",
             27, 400, "$c-inv-muted", family=CJK, line_height=1.7),
    ])


# ---------------------------------------------------------------- 02 候选者
def candidate(name, desc, hue, soft):
    top = rect(ids, "候选顶条", width="fill_container", height=8,
               fill=solid(hue))
    body = col("候选体", [
        text(ids, "候选名", name, 34, 700, hue, family=CJK,
             line_height=LH_HEAD),
        text(ids, "候选说明", desc, 24, 400, "$c-muted", family=CJK,
             line_height=1.5),
    ], gap=8, padding=[24, 24, 26, 24])
    card = col("候选者", [top, body], gap=0, width="fill_container")
    card["fill"] = solid(soft)
    card["clipContent"] = True
    card["cornerRadius"] = 6
    return card


def candidates():
    return band("02 候选者", fill=[], pad=[56, EDGE, 0, EDGE], gap=0,
                children=[
        row("候选行", [
            candidate(A_NAME, A_DESC, "$c-a", "$c-a-soft"),
            candidate(B_NAME, B_DESC, "$c-b", "$c-b-soft"),
        ], gap=20, align="stretch"),
    ])


# ------------------------------------------------------------------ 03 七行
def case_row(situation, side, why):
    hue = "$c-a" if side == "a" else "$c-b"
    soft = "$c-a-soft" if side == "a" else "$c-b-soft"
    name = A_NAME if side == "a" else B_NAME
    head_row = row("处境行", [
        text(ids, "处境", situation, 30, 700, "$c-ink", family=CJK,
             width=520, line_height=1.4),
        pill(name, bg=soft, fg=hue),
    ], gap=20, align="center", justify="space_between")
    item = col("判定项", [
        head_row,
        text(ids, "理由", why, 25, 400, "$c-muted", family=CJK,
             line_height=LH_BODY),
    ], gap=10, padding=[24, 26])
    item["fill"] = solid("$c-card")
    item["stroke"] = {"thickness": 2, "fill": solid("$c-line")}
    item["cornerRadius"] = 6
    return item


def cases():
    items = [case_row(*case) for case in CASES]
    return band("03 七行", fill=[], pad=[56, EDGE, 0, EDGE], gap=26,
                children=[
        section_head("七种处境", "先看左边那句是不是你，再看右边的标签。"),
        col("判定列表", items, gap=14),
    ])


# ------------------------------------------------------------------ 04 例外
def exception():
    title, detail = EXCEPTION
    panel = col("例外面板", [
        row("例外头", [
            pill(A_NAME, bg="$c-a-soft", fg="$c-a", size=24),
            text(ids, "加号", "＋", 26, 700, "$c-muted", family=CJK,
                 width="fit_content", growth="auto", line_height=1.4),
            pill(B_NAME, bg="$c-b-soft", fg="$c-b", size=24),
        ], gap=12, align="center"),
        text(ids, "例外标题", title, 34, 700, "$c-ink", family=CJK,
             line_height=LH_HEAD),
        text(ids, "例外说明", detail, 26, 400, "$c-muted", family=CJK,
             line_height=LH_BODY),
    ], gap=14, padding=[32, 30])
    panel["fill"] = solid("$c-panel")
    panel["cornerRadius"] = 6
    return band("04 例外", fill=[], pad=[56, EDGE, 64, EDGE], gap=26,
                children=[
        section_head("还有第八种", "七行里没有你？多半是这一种。"),
        panel,
    ])


# ------------------------------------------------------------------ 05 页脚
def footer():
    return band("05 页脚", fill=solid("$c-ink"), pad=[44, EDGE], gap=12,
                children=[
        text(ids, "结语", "决定不了，通常是因为还没写下截止日期和预算。",
             29, 600, "$c-card", family=CJK, line_height=LH_HEAD),
        row("署名行", [
            text(ids, "账号名", "@ 你的账号名", 26, 600, "$c-card",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
            text(ids, "更新说明", "每周一张能照着判的指南", 24, 400,
                 "$c-inv-muted", family=CJK, width="fit_content",
                 growth="auto", line_height=1.4),
        ], gap=16),
    ])


def build():
    page = frame(ids, "场景选择指南长图", width=W, height=ROOT_H,
                 layout="vertical", gap=0, fill=solid("$c-bg"),
                 clipContent=True)
    page["children"] = [header(), candidates(), cases(), exception(),
                        footer()]
    page["x"], page["y"] = 0, 0
    return [page]


# 对比度（WCAG 相对亮度比，op-design-lint 门槛 2.0；数值由 contrast.py 实测）：
#   c-ink on c-bg    16.45   c-muted on c-bg      7.05
#   c-ink on c-card  18.11   c-muted on c-card    7.76
#   c-a   on c-a-soft 7.11   c-b     on c-b-soft  5.25
#   c-a   on c-card   8.83   c-b     on c-card    6.40
#   c-muted on c-panel 6.26  c-card  on c-ink    18.11
#   c-inv-muted on c-ink 7.67
# 承载正文的最低一对是 5.25（赭石标签压在自己的浅底上）。两个标签色相差
# 约 175°、明度也差 1.9 档（7.11 / 5.25）——就算读者完全没有色觉，两个标
# 签在灰度下仍然一深一浅，认得出来。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "场景选择指南长图")
