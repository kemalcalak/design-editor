#!/usr/bin/env python3
"""steps-flow-infographic.op — 流程步骤信息长图（1080×N 竖版）

和同档的 data-report-infographic 是**一对**：那张回答「发生了什么」，靠图
表；这张回答「怎么做」，靠序号。两张共用信息图的阅读方式（从上往下滑完）
和结构语法（深色页头 + 若干区块 + 深色收尾），但色温、节奏和主元素全部相
反 —— 一张冷调青绿、以横条为主；一张暖调柑橘、以卡片为主。

流程图最容易坏在两个地方，这里都做了取舍：

  - **不画连接线。** 用 flex 排的卡片高度是内容决定的，一根需要跨越卡片
    去连接下一张的竖线只能靠写死像素，改一句文案就断。这里用「卡片之间一
    枚向下的箭头」代替：它是 flex 的兄弟，永远跟着走。
  - **序号不是装饰。** 每张卡左侧那个实心橙圆是全图唯一的高饱和块之一，
    它承担的是「你现在读到第几步」，所以尺寸、颜色、位置五张完全一致。

硬契约：
  - 内容距边缘 ≥80px（这里 80）
  - 配色全部走 color_vars，改主色只改 $c-accent 一处
  - 正文与背景对比度 ≥2.0（本配色最低一对见文件末尾注释）
  - **CJK 行高比西文全线高 0.2**：页头大标题 1.2，步骤标题 1.3，正文 1.7
  - **CJK 负字距不超过 -0.02em**（汉字是满格设计，再负就笔画相撞）；只有
    西文数字沿用西文 display 的收紧
  - 汉字走 Noto Sans SC，数字与拉丁走 Inter —— 等价于中文排印规范里
    「西文在前、中文在后」的 fallback 链，只是在 .op 里按节点写死
  - 顶层 frame 必须显式写 x/y，否则多帧会全部堆在原点
  - 根高固定：ROOT_H 是量出来的（见文件末尾），改内容后要重量一次

### 负约束（本模板明令不做的事）

  - 不画跨卡片的连接竖线（理由见上），也不画箭头以外的任何流程装饰。
  - 不用第二个有彩色。柑橘橙之外只有中性明度序列。
  - 不用蓝紫渐变、霓虹线条、复杂背景纹理（廉价 AI 科技风的三件套）。
  - 不用 emoji 当图标、不用装饰性插画、不用伪 3D。
  - 一步只讲一件事：说明超过两行就该拆成两步，不缩字号。
  - **说明控制在一行内**：27px 正文在 754px 可用宽里一行放 27 字，写到 30
    字就会折出「的问题。」这种四字末行。中文断行没有连字符可依靠，末行短
    到三四个字时读者会以为那是新的一段。
  - 不写 AI 套话（「赋能 / 一站式 / 打造闭环」），每一步都写成能照着做的动作。
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import (Ids, color_vars, frame, icon_font, rect, solid, text,
                   write_doc)

ids = Ids()

VARS = color_vars({
    "c-bg":          "#FBF7F2",
    "c-surface":     "#FFFFFF",
    # 收尾与页头的深底。暖石墨而不是中性黑：全图是暖调的，一块中性黑压在奶
    # 油底上会显脏。
    "c-band":        "#1C1A17",
    "c-band-muted":  "#ADA396",
    "c-ink":         "#221F1B",
    "c-muted":       "#6B6459",
    "c-accent":      "#D14D08",
    # 小字压在浅橙底上时用的深一档。主强调色在 #FCE7D6 上只有 3.67:1，够
    # 28px 的图标不够 23px 的胶囊文字 —— 分两个变量比放宽门槛便宜。
    "c-accent-deep": "#A83D06",
    "c-accent-soft": "#FCE7D6",
    "c-border":      "#ECE3D7",
    # 卡片之间那枚向下箭头的颜色。比 c-border 深一档：描边只要「在那儿」就
    # 够，箭头要被看见 —— 用同一个值它会淡到读不出流向（第一版实测如此）。
    "c-rail":        "#96876F",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W = 1080
EDGE = 80

# 量出来的根高（做法同 tpl_infographic_data：根设 fit_content 渲一次读高度）。
ROOT_H = 3085

STEPS = [
    ("挑一个你已经做过的场景",
     "别选没做过的事。用自己熟的，才分得清哪儿是工具的问题。",
     "10 分钟"),
    ("把结论写成一句话",
     "先写那句话，再找支撑它的内容。顺序反了就成了流水账。",
     "15 分钟"),
    ("套一个现成模板起稿",
     "从模板改比从空白建快一倍，也不会漏掉署名和出处。",
     "5 分钟"),
    ("只改文案，先别动版式",
     "把所有文字换成自己的，再回头看还有哪里不顺眼。多数时候一处都不用改。",
     "30 分钟"),
    ("导出前通读一遍",
     "念出声地读一遍。读不顺的句子，图上也一定看着别扭。",
     "5 分钟"),
]

TIPS = [
    "卡在第二步是正常的，那一步本来就最费脑子。",
    "一次只做一张。做完再开下一张，比同时开三张快。",
]


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


def chip(label, *, bg, fg, size=23, icon=None):
    node = frame(ids, "胶囊", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[9, 20], gap=10,
                 cornerRadius=999, alignItems="center",
                 justifyContent="center", fill=solid(bg))
    node["children"] = []
    if icon:
        node["children"].append(icon_font(ids, "胶囊图标", icon, size, fg))
    node["children"].append(
        text(ids, "胶囊文字", label, size, 600, fg, family=CJK,
             width="fit_content", growth="auto", line_height=1.4))
    return node


def section_head(title, note):
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
        chip("上手指南 · 第一次做图", bg="$c-accent", fg="$c-surface"),
        text(ids, "主标题", "五步做完\n第一张图", 76, 700, "$c-surface",
             family=CJK, line_height=1.2, spacing=-1.4),
        text(ids, "副标题", "全程约一小时，中间可以停。每一步都能单独做完。",
             28, 400, "$c-band-muted", family=CJK, line_height=1.7),
    ])


# ------------------------------------------------------------------ 02 步骤
def badge(index):
    """实心橙圆里的序号。五张卡完全一致 —— 它是进度，不是装饰。"""
    node = frame(ids, f"序号 {index}", width=76, height=76,
                 layout="horizontal", alignItems="center",
                 justifyContent="center", cornerRadius=38,
                 fill=solid("$c-accent"))
    node["children"] = [
        text(ids, "序号数字", f"{index:02d}", 32, 700, "$c-surface",
             family=NUM, width="fit_content", growth="auto", line_height=1.0),
    ]
    return node


def step_card(index, title, desc, cost):
    card = row(f"步骤 {index}", [
        badge(index),
        col("步骤文案", [
            text(ids, "步骤标题", title, 34, 600, "$c-ink", family=CJK,
                 line_height=1.3),
            text(ids, "步骤说明", desc, 27, 400, "$c-muted", family=CJK,
                 line_height=1.7),
            chip(cost, bg="$c-accent-soft", fg="$c-accent-deep",
                 icon="clock"),
        ], gap=12),
    ], gap=26, align="start", padding=[32, 32], cornerRadius=22)
    card["fill"] = solid("$c-surface")
    card["stroke"] = {"thickness": 2, "fill": solid("$c-border")}
    return card


def connector():
    """两张卡之间的向下箭头。它是 flex 的兄弟，所以卡片长高了它自己会跟着走
    —— 这正是不画竖线的理由。"""
    return row("连接", [icon_font(ids, "向下", "chevron-down", 32, "$c-rail")],
               gap=0, align="center", justifyContent="center")


def steps():
    items = []
    for index, (title, desc, cost) in enumerate(STEPS, 1):
        if items:
            items.append(connector())
        items.append(step_card(index, title, desc, cost))
    return band("02 步骤", fill=[], pad=[64, EDGE, 0, EDGE], gap=32, children=[
        section_head("按顺序来", "每一步后面那个时间是给自己留的上限，不是目标。"),
        col("步骤列表", items, gap=14),
    ])


# ------------------------------------------------------------------ 03 提示
def tips():
    items = []
    for line in TIPS:
        items.append(row("提示项", [
            icon_font(ids, "星标", "sparkles", 28, "$c-accent-deep"),
            text(ids, "提示文字", line, 27, 500, "$c-ink", family=CJK,
                 line_height=1.7),
        ], gap=16, align="start"))
    panel = col("提示面板", items, gap=18, padding=[36, 34], cornerRadius=22)
    panel["fill"] = solid("$c-accent-soft")
    return band("03 提示", fill=[], pad=[64, EDGE, 72, EDGE], gap=28, children=[
        section_head("两句实话", "做过的人都会撞上的两件事，先说在前面。"),
        panel,
    ])


# ------------------------------------------------------------------ 04 收尾
def closing():
    return band("04 收尾", fill=solid("$c-band"), pad=[64, EDGE, 56, EDGE],
                gap=24, children=[
        text(ids, "收尾标题", "做完记得存成自己的模板", 48, 700, "$c-surface",
             family=CJK, line_height=1.3),
        text(ids, "收尾说明", "下一张就只剩改文案了 —— 这五步只需要走一次。",
             27, 400, "$c-band-muted", family=CJK, line_height=1.7),
        chip("收藏这张图", bg="$c-accent", fg="$c-surface", size=27,
             icon="favorite"),
    ])


# ------------------------------------------------------------------ 05 页脚
def footer():
    return band("05 页脚", fill=[], pad=[36, EDGE, 44, EDGE], gap=8, children=[
        row("署名行", [
            text(ids, "账号名", "@ 你的账号名", 26, 600, "$c-ink", family=CJK,
                 width="fit_content", growth="auto", line_height=1.4),
            text(ids, "更新说明", "每周一张能照着做的图", 24, 400, "$c-muted",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
        ], gap=16, width="fill_container"),
    ])


def build():
    page = frame(ids, "流程步骤长图", width=W, height=ROOT_H, layout="vertical",
                 gap=0, fill=solid("$c-bg"), clipContent=True)
    page["children"] = [header(), steps(), tips(), closing(), footer()]
    page["x"], page["y"] = 0, 0
    return [page]


# 对比度（WCAG 相对亮度比，op-design-lint 的门槛是 2.0；数值实测）：
#   c-surface     on c-band        17.36   c-band-muted on c-band         6.99
#   c-ink         on c-bg          15.38   c-muted      on c-bg           5.48
#   c-ink         on c-surface     16.41   c-muted      on c-surface      5.85
#   c-surface     on c-accent       4.40   c-ink        on c-accent-soft 13.71
#   c-accent-deep on c-accent-soft  5.26   c-rail       on c-bg           3.29
# 承载文字的最低一对是 c-surface on c-accent 的 4.40 —— 那是序号圆里 32px
# 的白色数字，AA 对 ≥24px 粗体的门槛是 3.0，余量充足。c-rail 是卡片之间那
# 枚箭头，非文字图形按 3.0 量，3.29 刚过。换主色时先量这两对。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "流程步骤信息长图")
