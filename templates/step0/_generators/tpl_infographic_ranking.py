#!/usr/bin/env python3
"""ranking-board-infographic.op — 榜单 TOP N 长图（1080×N 竖版）

信息图这一档的第六张，回答**「值得的是哪几个，按顺序」** —— 一张带名次徽
章的推荐榜。

### 最近邻差异（为什么它不是 pitfall-list 换个色）

pitfall-list 也是排行，两者的分界线是**排的是什么**，而这决定了三处结构：

  - **pitfall 排的是「别做」，这张排的是「值得」。** 前者每条给「错在哪 +
    改成这样」，是一对否定与修正；这张每条给「什么时候用它 + 用得多勤」，
    是一条推荐与口径。
  - **pitfall 的名次是明度，这张的名次是徽章。** pitfall 刻意不给排名任何
    装饰（前三名靠反白块表达重灾区）；榜单的名次本身就是读者要的信息，所
    以它拿到一个实心圆徽章，前三名大、四到八名小且改描边 —— 尺寸阶梯就是
    名次阶梯。
  - **pitfall 全无彩，这张是墨底黑金。** 两张并排时不会认错。

和其余四张的分工：data-report 讲「数字是多少」，steps 讲「怎么做」，
timeline 讲「怎么走到今天」，concept 讲「这两个词不是一回事」，这张讲
**「先看这几个」**。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：榜单的原型是颁奖 —— 奖牌、绶带、名次牌。暖金（~45°）是这件
    事唯一有共识的颜色，不需要读者学习编码。
  - **收敛**：一个色相 + 一条暖墨明度序列。金只出现在三处：页头整块、名次
    徽章、区块短线。正文一律走米白与暖灰。
  - **论证**：金色一旦铺开就滑向「土味课程海报」。这里把它压在**两块实底
    带**（页头、页脚）和**八枚徽章**上，中间的正文区一点金都不给 —— 让金
    只做「这是个榜」的信号，不做装饰。深底是必需的：金在浅底上会发灰，只
    有压在近黑上才立得住。

### 负约束（本模板明令不做的事）

  - **不用金属渐变、不用高光、不用反光描边。** 金色的廉价感全从这三样来。
    全图只有实色块。
  - 不给前三名换三种颜色（金银铜）。三色一上，「名次」就变成了「阵营」，
    而且第四名之后无色可用。名次差异只由**徽章尺寸与实心/描边**表达。
  - 不用皇冠、奖杯、星星堆叠等奖项插画，不用 emoji 当图标。
  - 不用蓝紫渐变、霓虹线条、复杂背景纹理。
  - 一条只推一个东西，理由只写一句「什么时候用它」；写不下就减条目，不缩
    字号。
  - 不写 AI 套话（「神器 / 效率翻倍 / 一站式」），每条理由都写成能当场照
    做的动作。
  - 评选口径必须写在图里 —— 不写口径的榜单是广告。

硬契约：
  - 内容距边缘 ≥80px（这里 80）
  - 配色全部走 color_vars，改主色只改 $c-accent 一处（$c-accent-dim 是它
    的暗一档，换主色时要一起量）
  - 正文与背景对比度 ≥2.0（本配色最低一对见文件末尾注释）
  - **CJK 行高比西文全线高 0.2**：页头大标题 1.2，区块标题 1.3，正文 1.7
  - **CJK 负字距不超过 -0.02em**；只有西文名次数字沿用西文 display 的收紧
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
    "c-bg":          "#12100B",
    "c-surface":     "#1E1B12",
    "c-ink":         "#F8F4E9",
    "c-muted":       "#A79C82",
    "c-accent":      "#E0AB4A",
    # 四到八名那圈描边徽章的颜色。金本身画描边会和前三名的实心徽章一样抢
    # 眼，名次阶梯就没了 —— 暗一档只够「看得见」。
    "c-accent-dim":  "#8A6F2E",
    "c-accent-soft": "#2A2415",
    # 卡片描边。比 surface 亮一档，用来在墨底上分出卡片边界（不用阴影）。
    "c-border":      "#332E20",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W = 1080
EDGE = 80

LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.3, 1.7

# 徽章的两档尺寸。名次阶梯全靠这两个数 + 实心/描边，不靠颜色。
BADGE_TOP = 78
BADGE_REST = 56

# 量出来的根高（做法同同档另外五张：根设 fit_content 渲一次读 PNG 高度）。
ROOT_H = 3154

# (名次, 名称, 什么时候用它, 用得多勤)
PICKS = [
    ("01", "手机自带的备忘录",
     "记选题。能最快打开的那个才有用，别为它换软件。", "每天"),
    ("02", "一个全文可搜的笔记本",
     "找三个月前写过的那句话。搜不到的笔记等于没写。", "每天"),
    ("03", "计时器",
     "开工前先按 25 分钟。难的是开始，不是坚持。", "每天"),
    ("04", "一个固定的起稿模板",
     "新建都从它开始，署名和出处就不会再漏。", "每周"),
    ("05", "截图标注工具",
     "回问题时直接圈出来，比打三行字快。", "每周"),
    ("06", "剪贴板历史",
     "找回十分钟前复制的那段，不用回去重翻一遍。", "每周"),
    ("07", "稍后读的收件箱",
     "看到就丢进去，当天不读。它防的是分心，不是遗忘。", "每周"),
    ("08", "纸和笔",
     "想不清楚就关掉屏幕。列不出三条，说明还没想明白。", "每月"),
]

CRITERIA = [
    ("连续用满 12 个月才进榜，试用期一律不算。", "calendar-check"),
    ("按打开次数排，不按付费金额排。", "list-ordered"),
    ("同类只留一个 —— 留下的那个是最后活下来的。", "filter"),
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


def chip(label, *, bg, fg, size=23, stroke=None):
    node = frame(ids, "胶囊", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[9, 20], cornerRadius=999,
                 alignItems="center", justifyContent="center",
                 fill=solid(bg) if bg else [])
    if stroke:
        node["stroke"] = {"thickness": 2, "fill": solid(stroke)}
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


def badge(rank, *, top):
    """名次徽章。前三名实心大号，四到八名描边小号 —— 名次差异只在这里。"""
    size = BADGE_TOP if top else BADGE_REST
    node = frame(ids, f"徽章 {rank}", width=size, height=size,
                 layout="horizontal", alignItems="center",
                 justifyContent="center", cornerRadius=999,
                 fill=solid("$c-accent") if top else [])
    if not top:
        node["stroke"] = {"thickness": 3, "fill": solid("$c-accent-dim")}
    node["children"] = [
        text(ids, "名次数字", rank, 34 if top else 26, 700,
             "$c-bg" if top else "$c-accent", family=NUM,
             width="fit_content", growth="auto", line_height=1.0,
             spacing=-1),
    ]
    return node


# ------------------------------------------------------------------ 01 页头
def header():
    """整块金底。全图只有页头和页脚上金，中间的榜单区一点金都不铺。"""
    return band("01 页头", fill=solid("$c-accent"), pad=[76, EDGE, 68, EDGE],
                gap=26, children=[
        chip("年度榜单 · TOP 8", bg="$c-bg", fg="$c-accent"),
        text(ids, "主标题", "用满一年\n还留着的八个", 76, 700, "$c-bg",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.4),
        text(ids, "副标题", "评选口径写在图片最下方，可直接对着这张单子抄。",
             28, 400, "$c-accent-soft", family=CJK, line_height=LH_BODY),
    ])


# ------------------------------------------------------------------ 02 前三
def top_card(rank, name, why, freq):
    card = row(f"榜首卡 {rank}", [
        badge(rank, top=True),
        col("榜首文案", [
            text(ids, "名称", name, 36, 600, "$c-ink", family=CJK,
                 line_height=LH_HEAD),
            text(ids, "理由", why, 27, 400, "$c-muted", family=CJK,
                 line_height=LH_BODY),
            chip(freq, bg="$c-accent-soft", fg="$c-accent"),
        ], gap=12),
    ], gap=26, align="start", padding=[32, 32], cornerRadius=22)
    card["fill"] = solid("$c-surface")
    card["stroke"] = {"thickness": 2, "fill": solid("$c-border")}
    return card


def podium():
    cards = [top_card(*entry) for entry in PICKS[:3]]
    return band("02 前三", fill=[], pad=[68, EDGE, 0, EDGE], gap=32,
                children=[
        section_head("先看前三",
                     "这三个每天都会打开。少了任何一个，当天的流程就断一截。"),
        col("前三列表", cards, gap=18),
    ])


# ------------------------------------------------------------------ 03 其余
def rest_row(rank, name, why, freq):
    return row(f"条目 {rank}", [
        badge(rank, top=False),
        col("条目文案", [
            row("条目标题行", [
                text(ids, "名称", name, 30, 600, "$c-ink", family=CJK,
                     width="fit_content", growth="auto", line_height=1.4),
                text(ids, "频次", freq, 23, 500, "$c-accent", family=CJK,
                     width="fit_content", growth="auto", line_height=1.4),
            ], gap=14),
            text(ids, "理由", why, 26, 400, "$c-muted", family=CJK,
                 line_height=LH_BODY),
        ], gap=8),
    ], gap=22, align="start")


def rest():
    items = []
    for index, entry in enumerate(PICKS[3:]):
        if items:
            items.append(rect(ids, "条目分割线", width="fill_container",
                              height=2, fill=solid("$c-border")))
        items.append(rest_row(*entry))
    panel = col("其余列表", items, gap=22, padding=[34, 32], cornerRadius=22)
    panel["fill"] = solid("$c-surface")
    panel["stroke"] = {"thickness": 2, "fill": solid("$c-border")}
    return band("03 其余", fill=[], pad=[68, EDGE, 0, EDGE], gap=32,
                children=[
        section_head("第四到第八",
                     "这五个不是每天用，但每次用都省下一段整块的时间。"),
        panel,
    ])


# ------------------------------------------------------------------ 04 口径
def criteria():
    items = []
    for line, glyph in CRITERIA:
        items.append(row("口径项", [
            icon_font(ids, "图标", glyph, 28, "$c-accent"),
            text(ids, "口径文字", line, 27, 500, "$c-ink", family=CJK,
                 line_height=LH_BODY),
        ], gap=16, align="start"))
    panel = col("口径面板", items, gap=18, padding=[36, 34], cornerRadius=22)
    panel["fill"] = solid("$c-accent-soft")
    return band("04 口径", fill=[], pad=[68, EDGE, 68, EDGE], gap=32,
                children=[
        section_head("这个榜是怎么排的",
                     "先说清楚规则，再看名次 —— 不写口径的榜单是广告。"),
        panel,
    ])


# ------------------------------------------------------------------ 05 页脚
def footer():
    return band("05 页脚", fill=solid("$c-accent"), pad=[44, EDGE], gap=12,
                children=[
        text(ids, "说明", "统计区间：2025-08 至 2026-07，按每日打开次数取中位数。",
             24, 400, "$c-accent-soft", family=CJK, line_height=1.6),
        row("署名行", [
            text(ids, "账号名", "@ 你的账号名", 26, 600, "$c-bg", family=CJK,
                 width="fit_content", growth="auto", line_height=1.4),
            text(ids, "更新说明", "每年更新一次这张榜", 24, 400,
                 "$c-accent-soft", family=CJK, width="fit_content",
                 growth="auto", line_height=1.4),
        ], gap=16, width="fill_container"),
    ])


def build():
    page = frame(ids, "榜单 TOP N 长图", width=W, height=ROOT_H,
                 layout="vertical", gap=0, fill=solid("$c-bg"),
                 clipContent=True)
    page["children"] = [header(), podium(), rest(), criteria(), footer()]
    page["x"], page["y"] = 0, 0
    return [page]


# 对比度（WCAG 相对亮度比，op-design-lint 的门槛是 2.0；数值实测）：
#   c-ink        on c-bg       17.30   c-muted      on c-bg          6.99
#   c-ink        on c-surface  15.65   c-muted      on c-surface     6.32
#   c-accent     on c-bg        9.14   c-accent     on c-surface     8.27
#   c-bg         on c-accent    9.14   c-surface    on c-accent      8.27
#   c-ink        on c-soft     14.03   c-muted      on c-accent-soft 5.67
#   c-accent     on c-soft      7.41   c-accent-dim on c-bg          3.98
# 承载文字的最低一对是 5.67。c-accent-dim 只画四到八名徽章那圈 3px 描边
# （非文字图形，AA 门槛 3.0），3.98 有余量；徽章里的数字走 c-accent
# （9.14）。页脚那行小字是 c-accent-soft 压在金底上，实测 8.27。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "榜单 TOP N 长图")
