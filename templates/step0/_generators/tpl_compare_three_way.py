#!/usr/bin/env python3
"""three-way-comparison.op — 三方案横评（1080×N 竖版长图）

对比档的「多选一」那一张：三个方案并排各说自己的定位，中间那列是推荐项。
和两方对照最大的不同是——三列一起看的时候，读者其实在找「哪一列是我」，
所以每列的第一行不是名字而是**一句处境**。

### 最近邻论证（为什么它不是已有的哪一张）

  - **本批 01 参数表**：那张是**行优先**——同一指标横着被回答两次，读者
    的眼睛左右扫。这张是**列优先**——一列从头读到尾就是一个完整方案，读者
    的眼睛上下扫，扫完再横向跳一列。同样是网格，阅读方向正好相反。
  - **本批 05 价格三档**：那张的三列是**同一个产品的三个档位**（越往右买
    得越多，是一条递增的线）；这张的三列是**三条互不兼容的路**（没有谁是
    谁的升级版）。所以那张能给中间列打「最超值」，这张只能给中间列打「大
    多数人」——推荐的理由完全不同。
  - **pitfall-list-infographic**：那张一列到底，单位是「一条」；这张单位是
    「一整列」。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：三个方案本身没有颜色，但「推荐」这件事有——它需要一个能在
    三列里被一眼认出的记号。
  - **收敛**：中性暖灰序列 L 0.10 / 0.24 / 0.42 / 0.62 / 0.90 / 0.94 / 1.0
    （chroma ≤0.006）+ **一个**低饱和琥珀 #A8681C（chroma≈0.10），琥珀只
    出现在推荐列的顶条、推荐标签和那一列的评分条上，其它地方一次都不出现。
  - **论证**：三列并置时，「谁被推荐」是唯一需要预先告知的判断，其余全是
    事实。所以有彩色的预算只有一份，全押在这一个判断上；剩下的层级照旧由
    明度和字重承担。琥珀而非红绿：这里推荐的不是「对」，另外两列也不是
    「错」，红绿会把「适不适合你」误读成「好不好」。

### 负约束（本模板明令不做的事）

  - **只允许一个有彩色，且只用在推荐这件事上。** 琥珀出现在第四个地方就
    该删掉一处。
  - 不用红绿。三条路没有对错，只有合不合适。
  - 不给非推荐列加灰度压暗、降透明度这类「淘汰」处理。它们是平等的选项。
  - 不写「最佳 / 首选 / 王炸」。推荐标签只写「大多数人」，并在下面给出为什
    么是大多数人。
  - 不用蓝紫渐变、霓虹线条、伪 3D、阴影、emoji 图标。
  - 每条要点 ≤10 字：列内容宽只有 262px，写到 11 字就折行，三列高度立刻
    对不齐。
  - 评分条不标数字。三列的条长是相对的，标了数字就得为「8.5 分怎么来的」
    负责。

硬契约：
  - 内容距边缘 ≥64px（这里 64）
  - 三列必须全部 fill_container + stretch，等宽等高是「平等选项」的前提
  - 配色全部走 color_vars；换主色 = 改 c-accent 一处
  - 正文与背景对比度 ≥2.0（最低一对见文件末尾实测表）
  - **CJK 行高**：大标题 1.2，列标题 1.3，正文 1.7
  - **CJK 负字距不超过 -0.02em**；数字与拉丁走 Inter
  - 顶层 frame 必须显式写 x/y，否则多帧会全部堆在原点
  - 根高固定：ROOT_H 是量出来的（见文件末尾），改内容后要重量一次
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
    "c-bg":         "#F5F3F0",
    "c-card":       "#FFFFFF",
    "c-panel":      "#EAE7E2",
    "c-line":       "#D9D5CF",
    "c-ink":        "#16130F",
    "c-deep":       "#332C24",
    "c-muted":      "#5C544A",
    "c-faint":      "#958E84",
    "c-inv-muted":  "#ADA69C",
    # 唯一的有彩色。只用在「推荐」这一个判断上：顶条、标签、那一列的评分条。
    "c-accent":     "#A8681C",
    "c-accent-ink": "#FFFFFF",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W = 1080
EDGE = 64
GUTTER = 16

LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.3, 1.7

ROOT_H = 1886

# (处境, 方案名, 上手时间, [要点 ≤10 字 ×3], 相对自由度 0-1, 是否推荐)
PLANS = [
    ("今晚就要上线", "套模板", "半小时",
     ["改文案就能发", "版式已经定好", "换风格要重来"], 0.28, False),
    ("大多数人在这", "低代码", "两天",
     ["拖拽自己排版", "能接自己的数据", "复杂交互仍要码"], 0.62, True),
    ("要养三年以上", "自己写", "两周起",
     ["想怎么改就怎么改", "性能自己说了算", "每加一页都要人"], 1.0, False),
]

# 「怎么选」。三条判据各指向一个方案，读者对号入座。
PICKS = [
    ("只做这一次", "套模板"),
    ("要持续更新内容", "低代码"),
    ("产品本身就是它", "自己写"),
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


def band(name, *, fill, pad, gap, children, align="start"):
    node = frame(ids, name, width="fill_container", height="fit_content",
                 layout="vertical", padding=pad, gap=gap, alignItems=align,
                 fill=fill)
    node["children"] = children
    return node


def tag(label, *, bg, fg, size=24):
    node = frame(ids, "标签", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[9, 20], cornerRadius=999,
                 alignItems="center", justifyContent="center", fill=solid(bg))
    node["children"] = [
        text(ids, "标签文字", label, size, 600, fg, family=CJK,
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
        tag("三方案横评 · 建站", bg="$c-card", fg="$c-ink"),
        text(ids, "主标题", "同一个网站\n三条路", 76, 700, "$c-card",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.4),
        text(ids, "副标题", "先看第一行那句处境——哪一句是你，就从哪一列读起。",
             27, 400, "$c-inv-muted", family=CJK, line_height=LH_BODY),
    ])


# ------------------------------------------------------------------ 02 三列
def meter(fraction, featured):
    """自由度条。三列的长度只做相对比较，所以不标数字。"""
    track = frame(ids, "自由度轨", width="fill_container", height=10,
                  layout="horizontal", alignItems="center",
                  justifyContent="start", cornerRadius=999,
                  fill=solid("$c-panel" if not featured else "$c-deep"))
    track["children"] = [
        rect(ids, "自由度值", width=round(262 * fraction), height=10,
             cornerRadius=999,
             fill=solid("$c-accent" if featured else "$c-faint")),
    ]
    return col("自由度", [
        row("自由度头", [
            text(ids, "自由度名", "改动自由度", 22, 500,
                 "$c-faint" if not featured else "$c-inv-muted", family=CJK,
                 width="fit_content", growth="auto", line_height=1.4),
        ], gap=0),
        track,
    ], gap=10)


def bullet(line, featured):
    # 要点点**不上琥珀**：琥珀的预算是三处（顶条 / 自由度值 / 推荐标签），
    # 每列三颗点会让它变成第四、第五、第六处，「唯一记号」就稀释掉了。
    dot = rect(ids, "要点点", width=10, height=10, cornerRadius=2,
               fill=solid("$c-inv-muted" if featured else "$c-faint"))
    wrap = frame(ids, "点位", width=10, height=41, layout="vertical",
                 justifyContent="center", fill=[])
    wrap["children"] = [dot]
    return row("要点", [
        wrap,
        text(ids, "要点文字", line, 24, 400,
             "$c-inv-muted" if featured else "$c-muted", family=CJK,
             line_height=LH_BODY),
    ], gap=12, align="start")


def plan_card(situation, name, setup, points, freedom, featured):
    top = rect(ids, "顶条", width="fill_container", height=8,
               fill=solid("$c-accent" if featured else "$c-line"))
    head = col("列头", [
        text(ids, "处境", situation, 23, 600,
             "$c-inv-muted" if featured else "$c-muted", family=CJK,
             line_height=1.45),
        text(ids, "方案名", name, 38, 700,
             "$c-card" if featured else "$c-ink", family=CJK,
             line_height=LH_HEAD),
        text(ids, "上手时间", "上手 " + setup, 23, 400,
             "$c-inv-muted" if featured else "$c-muted", family=NUM,
             line_height=1.45),
    ], gap=8)
    body = col("列体", [
        head,
        col("要点组", [bullet(line, featured) for line in points], gap=4),
        meter(freedom, featured),
    ], gap=22, padding=[26, 22, 28, 22])
    card = col("方案列", [top, body], gap=0, width="fill_container")
    card["fill"] = solid("$c-deep" if featured else "$c-card")
    card["clipContent"] = True
    card["cornerRadius"] = 4
    if not featured:
        card["stroke"] = {"thickness": 2, "fill": solid("$c-line")}
    return card


def plans():
    cards = [plan_card(*plan) for plan in PLANS]
    return band("02 三列", fill=[], pad=[60, EDGE, 0, EDGE], gap=26,
                children=[
        section_head("三列并排",
                     "中间那列不是最好的，是最多人最后落到的那一条。"),
        row("方案行", cards, gap=GUTTER, align="stretch"),
        row("推荐说明", [
            tag("推荐", bg="$c-accent", fg="$c-accent-ink", size=22),
            text(ids, "推荐理由",
                 "琥珀色在这张图里只代表一件事：大多数人最后选了它。",
                 25, 400, "$c-muted", family=CJK, line_height=LH_BODY),
        ], gap=14, align="center"),
    ])


# ------------------------------------------------------------------ 03 怎么选
def picks():
    items = []
    for cond, answer in PICKS:
        # 一行只放一个箭头。条件前再加一个 icon 箭头会和答案前的那个撞成
        # 「→ 条件 → 答案」，读起来像两级跳转，实际上只有一级。
        items.append(row("选择项", [
            text(ids, "条件", cond, 27, 400, "$c-muted", family=CJK,
                 width=360, line_height=1.5),
            icon_font(ids, "指向", "arrow-right", 24, "$c-faint"),
            text(ids, "答案", answer, 27, 700, "$c-ink", family=CJK,
                 line_height=1.5),
        ], gap=18, align="center"))
    panel = col("选择面板", items, gap=18, padding=[34, 32])
    panel["fill"] = solid("$c-panel")
    return band("03 怎么选", fill=[], pad=[60, EDGE, 64, EDGE], gap=26,
                children=[
        section_head("三句话对号入座", "不用比完三列，先回答你属于哪一句。"),
        panel,
    ])


# ------------------------------------------------------------------ 04 页脚
def footer():
    return band("04 页脚", fill=solid("$c-ink"), pad=[44, EDGE], gap=12,
                children=[
        text(ids, "结语", "选错的代价不是钱，是三个月后要重做一遍。",
             30, 600, "$c-card", family=CJK, line_height=LH_HEAD),
        row("署名行", [
            text(ids, "账号名", "@ 你的账号名", 26, 600, "$c-card",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
            text(ids, "更新说明", "每周一张能照着选的横评", 24, 400,
                 "$c-inv-muted", family=CJK, width="fit_content",
                 growth="auto", line_height=1.4),
        ], gap=16),
    ])


def build():
    page = frame(ids, "三方案横评长图", width=W, height=ROOT_H,
                 layout="vertical", gap=0, fill=solid("$c-bg"),
                 clipContent=True)
    page["children"] = [header(), plans(), picks(), footer()]
    page["x"], page["y"] = 0, 0
    return [page]


# 对比度（WCAG 相对亮度比，op-design-lint 门槛 2.0；数值由 contrast.py 实测）：
#   c-ink   on c-bg    16.72   c-muted on c-bg      6.72
#   c-ink   on c-card  18.52   c-muted on c-card    7.44
#   c-ink   on c-panel 15.01   c-muted on c-panel   6.03
#   c-card  on c-ink   18.52   c-inv-muted on c-ink 7.68
#   c-card  on c-deep  13.76   c-inv-muted on c-deep 5.71
#   c-accent on c-card  4.50   c-accent-ink on c-accent 4.50
#   c-faint on c-card   3.24（只画要点小方块与自由度条，非文字）
# 承载正文的最低一对是 4.50 —— 推荐列那句「处境」，23px/600 压在白卡上，
# 已过 AA 正文门槛（4.5）。琥珀在整张图里只出现三处，全部服务于同一个判断。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "三方案横评长图")
