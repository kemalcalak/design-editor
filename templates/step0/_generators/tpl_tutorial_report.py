#!/usr/bin/env python3
"""weekly-report-lesson.op — 职场周报小课长图（1080×N 竖版）

教程档里唯一一张**给可填模板**的：讲完四段结构之后，直接给一张带下划线空
格的周报骨架，读者截图就能照着往里填。教方法的内容最后必须落到一个能抄的
东西上，否则读完只剩「有道理」。

风格取「课程讲义 / 高级黑板风」：墨绿板面 + 粉笔米白 + 琥珀重点。九张里唯
一一张**反白长图**（另一张深色的是软件那张，但它是 4:5 单卡、青绿冷调）。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：从真实黑板采——不是纯黑板，是学校那种偏绿的搪瓷板；粉笔白
    带一点米；重点用黄粉笔。
  - **收敛**：三档墨绿（板 0.14 / 面 0.18 / 卡 0.22）+ 两档米白（0.90 /
    0.68）+ 1 个琥珀 #E8B14C。
  - **论证**：琥珀只标**「该照抄的东西」**——四段结构的段名、模板里的空
    格、反例改成正例的那一句。读者滚动时琥珀出现的位置，就是可以直接搬走
    的位置。墨绿而不是纯黑：长图要滚三千多像素，纯黑底上的米白字对比过硬
    （19:1）看久了刺眼，墨绿把它压到 11-12，读起来松一档。

### 负约束（本模板明令不做的事）

  - **不做粉笔手写体、不做粉笔灰噪点、不画黑板擦。** 「黑板」是配色和层
    级的隐喻，不是贴图。加了质感这张图立刻从讲义掉成班级板报。
  - 不用第二个有彩色。琥珀只标可抄的部分。
  - 不写「向上管理 / 对齐颗粒度 / 抓手」这类黑话——周报教程写黑话是自我否
    定。每句都用能直接写进周报的说法。
  - 不给虚构的 KPI 数字当范例（「转化率提升 32%」）。范例里的数字用占位下
    划线，让读者填自己的。
  - 四段封顶。第五段就该拆成另一节课。
  - 反例只给一条。列三条反例会把注意力从「怎么写对」拉走。

硬契约：
  - 内容距边缘 ≥80px（这里 80）
  - 配色全部走 color_vars；换主色只需改 c-accent 与 c-accent-ink
  - 正文与背景对比度 ≥2.0（实测表见文件末尾）
  - **CJK 行高比西文全线高 0.2**：display 1.2，标题 1.3，正文 1.7
  - **CJK 负字距不超过 -0.02em**（76px 标题 → -1.4px = -0.018em）
  - 汉字走 Noto Sans SC，数字与拉丁走 Inter
  - 顶层 frame 显式写 x/y
  - 根高固定：ROOT_H = 3116，量出来的（根设 fit_content 渲一次读高度）
  - 模板骨架里的空格是 2px 琥珀下划线 rect，不是文本里的下划线字符——后者
    在 CJK 字体里宽度不可控
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import (Ids, color_vars, frame, icon_font, rect, solid, stroke,
                   text, write_doc)

ids = Ids()

VARS = color_vars({
    "c-board":      "#1E2E27",
    "c-panel":      "#26382F",
    "c-card":       "#2C4139",
    "c-line":       "#3D5449",
    "c-chalk":      "#F0EBDF",
    "c-muted":      "#A6B5AB",
    "c-accent":     "#E8B14C",
    "c-accent-ink": "#2A1E04",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W = 1080
EDGE = 80
INNER = W - EDGE * 2

LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.3, 1.7

# 量出来的根高（根设 fit_content 渲一次读高度回填）。
ROOT_H = 3116

# 为什么周报难写。三条，每条一句。
PAINS = [
    "写成流水账：把日程表复制一遍，读的人看不出你解决了什么。",
    "只报喜不报忧：卡住的地方藏着，等到爆雷时才第一次被看见。",
    "没有下一步：写完这周就结束了，读的人不知道该给你什么支持。",
]

# (段名, 一句要求, 反面提示)
SECTIONS = [
    ("做成了什么", "写结果，不写过程。一条一件事，动词开头。",
     "别写「推进了 XX 项目」——推进不是结果。"),
    ("数据怎么样", "给一个能对比的数：跟上周比，或跟目标比。",
     "别只给绝对值，没有对照的数字说明不了任何事。"),
    ("卡在哪里", "写清楚卡点 + 你需要谁做什么。",
     "别写「有一些困难」——那等于没写。"),
    ("下周做什么", "最多三件，按优先级排，写清交付物。",
     "别列十件。列十件等于告诉别人你没排过优先级。"),
]

# 可填骨架。`None` 表示这里是一条待填空格（琥珀下划线）。
SKELETON = [
    ("本周做成", ["完成了", None, "，", "已交付给", None]),
    ("关键数据", [None, "从", None, "到", None, "（对比上周）"]),
    ("当前卡点", [None, "卡住，需要", None, "在", None, "前确认"]),
    ("下周计划", ["1.", None, "　2.", None, "　3.", None]),
]

CLOSING = [
    "周五下班前发，不要拖到周一——周一发的周报没人有空看。",
    "长度控制在手机一屏半，超了就把细节挪到附件。",
    "先写「卡在哪」，再回头写「做成了什么」，顺序反着来更容易写。",
]


def col(name, children, *, gap=16, width="fill_container", align="start",
        height="fit_content", **props):
    node = frame(ids, name, width=width, height=height, layout="vertical",
                 gap=gap, alignItems=align, fill=[], **props)
    node["children"] = children
    return node


def row(name, children, *, gap=20, align="center", width="fill_container",
        height="fit_content", **props):
    node = frame(ids, name, width=width, height=height, layout="horizontal",
                 gap=gap, alignItems=align, fill=[], **props)
    node["children"] = children
    return node


def band(name, children, *, fill, pad, gap):
    node = frame(ids, name, width="fill_container", height="fit_content",
                 layout="vertical", padding=pad, gap=gap, alignItems="start",
                 fill=fill)
    node["children"] = children
    return node


def section_head(title, note):
    return col("区块头", [
        rect(ids, "强调短线", width=72, height=8, fill=solid("$c-accent")),
        text(ids, "区块标题", title, 46, 700, "$c-chalk", family=CJK,
             line_height=LH_HEAD),
        text(ids, "区块说明", note, 26, 400, "$c-muted", family=CJK,
             line_height=LH_BODY),
    ], gap=14)


# ------------------------------------------------------------------ 01 页头
def header():
    tag = frame(ids, "课次标签", width="fit_content", height="fit_content",
                layout="horizontal", padding=[10, 20], cornerRadius=4,
                alignItems="center", justifyContent="center",
                fill=solid("$c-accent"))
    tag["children"] = [
        text(ids, "课次文字", "职场小课 · Lesson 01", 24, 700,
             "$c-accent-ink", family=CJK, width="fit_content", growth="auto",
             line_height=1.4),
    ]
    return band("01 页头", fill=solid("$c-panel"), pad=[80, EDGE, 70, EDGE],
                gap=26, children=[
        tag,
        text(ids, "主标题", "周报不是流水账\n是一次汇报", 76, 700, "$c-chalk",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.4),
        text(ids, "副标题",
             "四段结构 + 一张可以直接填的骨架。这节课学完就能用在这周五。",
             28, 400, "$c-muted", family=CJK, line_height=LH_BODY),
    ])


# ------------------------------------------------------------------ 02 痛点
def pains():
    items = []
    for index, line in enumerate(PAINS, 1):
        items.append(row("痛点项", [
            text(ids, "痛点序号", f"{index:02d}", 28, 700, "$c-accent",
                 family=NUM, width=64, line_height=1.5),
            text(ids, "痛点文字", line, 26, 400, "$c-muted", family=CJK,
                 line_height=LH_BODY),
        ], gap=12, align="start"))
    return band("02 痛点", fill=[], pad=[64, EDGE, 0, EDGE], gap=26,
                children=[
        section_head("先说清楚它为什么难写",
                     "三条里只要中一条，你的周报就白写了。"),
        col("痛点列表", items, gap=18),
    ])


# ------------------------------------------------------------------ 03 结构
def structure():
    items = []
    for index, (name, need, avoid) in enumerate(SECTIONS, 1):
        head = row("段头", [
            text(ids, "段序号", f"{index:02d}", 30, 700, "$c-accent",
                 family=NUM, width=62, line_height=1.3),
            text(ids, "段名", name, 34, 700, "$c-chalk", family=CJK,
                 line_height=LH_HEAD),
        ], gap=8, align="center")
        warn = row("反面提示", [
            icon_font(ids, "反面图标", "circle-x", 24, "$c-muted"),
            text(ids, "反面文字", avoid, 23, 400, "$c-muted", family=CJK,
                 line_height=LH_BODY),
        ], gap=12, align="start")
        card = col("结构段", [
            head,
            text(ids, "段要求", need, 26, 500, "$c-chalk", family=CJK,
                 line_height=LH_BODY),
            warn,
        ], gap=14, padding=[28, 30])
        card["fill"] = solid("$c-card")
        card["cornerRadius"] = 12
        items.append(card)
    return band("03 结构", fill=[], pad=[64, EDGE, 0, EDGE], gap=26,
                children=[
        section_head("四段，按这个顺序排",
                     "段名可以换措辞，顺序不要换——读的人是按这个顺序找信息的。"),
        col("结构列表", items, gap=18),
    ])


# ------------------------------------------------------------------ 04 骨架
def blank(width_px):
    """一条待填空格。2px 琥珀下划线，不是文本下划线——见硬契约。"""
    holder = col("空格", [
        rect(ids, "空格底线", width=width_px, height=2,
             fill=solid("$c-accent")),
    ], gap=0, width="fit_content", height=34, justifyContent="end")
    return holder


def skeleton():
    lines = []
    for label, parts in SKELETON:
        chunks = []
        for part in parts:
            if part is None:
                chunks.append(blank(150))
            else:
                chunks.append(text(ids, "骨架文字", part, 25, 400,
                                   "$c-chalk", family=CJK,
                                   width="fit_content", growth="auto",
                                   line_height=1.4))
        entry = col("骨架行", [
            text(ids, "骨架段名", label, 24, 700, "$c-accent", family=CJK,
                 line_height=1.4),
            row("骨架内容", chunks, gap=10, align="center",
                width="fill_container"),
        ], gap=10)
        lines.append(entry)

    sheet = col("骨架纸", lines, gap=24, padding=[32, 32])
    sheet["fill"] = solid("$c-panel")
    sheet["cornerRadius"] = 12
    sheet["stroke"] = stroke("$c-line", 2)

    return band("04 骨架", fill=[], pad=[64, EDGE, 0, EDGE], gap=26,
                children=[
        section_head("直接抄这张骨架",
                     "琥珀色下划线的地方填你自己的内容，别的字一个都不用改。"),
        sheet,
    ])


# ------------------------------------------------------------------ 05 收尾
def footer():
    items = []
    for line in CLOSING:
        box = frame(ids, "方框", width=36, height=36, layout="horizontal",
                    alignItems="center", justifyContent="center",
                    cornerRadius=6, fill=solid("$c-accent"))
        box["children"] = [icon_font(ids, "勾", "check", 22, "$c-accent-ink")]
        items.append(row("收尾项", [
            box,
            text(ids, "收尾文字", line, 25, 400, "$c-chalk", family=CJK,
                 line_height=LH_BODY),
        ], gap=16, align="start"))

    return band("05 收尾", fill=solid("$c-panel"), pad=[64, EDGE, 56, EDGE],
                gap=26, children=[
        text(ids, "收尾标题", "写之前，先记这三条", 42, 700, "$c-chalk",
             family=CJK, line_height=LH_HEAD),
        col("收尾列表", items, gap=16),
        row("署名行", [
            text(ids, "账号名", "@ 你的账号名", 25, 600, "$c-chalk",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
            text(ids, "更新说明", "每周一节能当天用上的职场课", 23, 400,
                 "$c-muted", family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
        ], gap=16, justifyContent="space_between"),
    ])


def build():
    page = frame(ids, "周报小课长图", width=W, height=ROOT_H, layout="vertical",
                 gap=0, fill=solid("$c-board"), clipContent=True)
    page["children"] = [header(), pains(), structure(), skeleton(), footer()]
    page["x"], page["y"] = 0, 0
    return [page]


# 对比度（WCAG 相对亮度比，op-design-lint 门槛 2.0；数值由脚本实测）：
#   c-chalk  on c-board  11.97   c-muted on c-board   6.66
#   c-chalk  on c-panel  10.45   c-muted on c-panel   5.81
#   c-chalk  on c-card    9.18   c-muted on c-card    5.11
#   c-accent on c-board   7.34   c-accent on c-panel  6.41
#   c-accent-ink on c-accent 8.42
# 承载正文的最低一对是 5.11。这张刻意把最高对比压到 11.97 而不是纯黑底的
# 19：长图要滚三千多像素，19:1 的米白压纯黑看久了会刺眼。c-line 只用于
# hairline 描边，是非文字图形。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "职场周报小课长图")
