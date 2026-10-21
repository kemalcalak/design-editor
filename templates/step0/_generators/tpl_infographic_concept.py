#!/usr/bin/env python3
"""concept-contrast-infographic.op — 概念对比科普长图（1080×N 竖版）

信息图这一档的第五张，回答**「A 和 B 到底差在哪」** —— 一张图讲清一组常
被混为一谈的概念。

### 最近邻差异（为什么它不是 comparison 档搬进长图）

comparison（before-after 那一档）是**单屏对开**：左右两半同时进入视野，读
者做的是「一眼比大小」。这张是**长图叙事**，读者是滑着读完的，所以结构不
是对开，是一条推进线：

    先给结论 → 再给两边各自是什么 → 再逐维度拆 → 最后给选择判据

对开版式在长图里会失效：手机上 1080 宽的两栏，每栏只剩 440，放不下一句完
整的原理。所以这里**只有维度表是两栏，且每栏只放一句话**；定义卡是上下两
张全宽卡，靠明度反差（一张深莓反白、一张白底描边）而不是左右位置来区分。

和另外四张的分工：data-report 讲「数字是多少」，steps 讲「怎么做」，
pitfall 讲「别做什么」，timeline 讲「怎么走到今天」，这张讲**「这两个词不
是一回事」**。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：从「两个相邻但不同的东西」这个母题取色 —— 需要一个能自然分
    出深浅两档、又不带是非判断的色相。莓红（~335°）满足：它的深档庄重、
    浅档柔和，且不属于任何一方。
  - **收敛**：一个色相 + 中性暖灰序列。A 侧用 $c-accent-deep 实底反白，B
    侧用 $c-surface + 描边 —— 这是**明度差**，不是色相差。
  - **论证**：对比图最容易滑向「红=错、绿=对」。这一组概念没有对错（两种
    降噪各有各的适用场景），所以配色必须**拒绝携带价值判断**。同色相两档
    明度只表达「这是两个东西」，读者不会从颜色里读出谁更好。

### 负约束（本模板明令不做的事）

  - **不用红绿（或任何一冷一暖两个色相）分 A/B。** 那会给一组中性概念强
    行安上对错，是这张图的核心禁令。
  - 不用左右对开的单屏版式（理由见上），也不画中间那条「VS」大字。
  - 不用渐变、霓虹线条、复杂背景纹理（廉价 AI 科技风的三件套）。
  - 不用 emoji 当图标、不用伪 3D、不用示意插画代替文字 —— 原理靠一句话讲
    清，讲不清就是没想清楚。
  - 一个维度只放一句话，两栏各一句；写不下就加一个维度，不缩字号。
  - 不写 AI 套话（「本质区别 / 底层逻辑 / 认知升级」），每一句都写成可验证
    的事实或可当场做的动作。

硬契约：
  - 内容距边缘 ≥80px（这里 80）
  - 配色全部走 color_vars，改主色只改 $c-accent / $c-accent-deep 两处
  - 正文与背景对比度 ≥2.0（本配色最低一对见文件末尾注释）
  - **CJK 行高比西文全线高 0.2**：页头大标题 1.2，区块标题 1.3，正文 1.7
  - **CJK 负字距不超过 -0.02em**；西文与数字沿用西文 display 的收紧
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
    "c-bg":          "#FBF5F7",
    "c-surface":     "#FFFFFF",
    "c-band":        "#2C0F1D",
    "c-band-muted":  "#B79AA5",
    "c-ink":         "#23101A",
    "c-muted":       "#6E5460",
    "c-accent":      "#A6215C",
    # A 侧那张反白卡的实底，也是浅莓底上小字的颜色。主强调色在 #F7DDE8 上
    # 只有 5.2，够用但余量小；深一档留给正文尺寸的文字。
    "c-accent-deep": "#7E1745",
    "c-accent-soft": "#F7DDE8",
    "c-border":      "#EEDDE4",
    # 维度表里那条竖分隔线。比 c-border 深一档 —— 它要在白卡上被看见，而
    # c-border 在白底上只有 1.2，等于没画（第一版实测如此）。
    "c-rule":        "#C9A9B7",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W = 1080
EDGE = 80
INNER = W - EDGE * 2

LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.3, 1.7

# 维度表两栏之间那条竖线的宽度，以及两栏各自的固定宽度。卡片内边距 34，
# 所以可用宽度是 INNER - 68；减去竖线与两侧 gap 后对半分。
CARD_PAD = 34
RULE_W = 2
CELL_GAP = 26
CELL_W = (INNER - CARD_PAD * 2 - RULE_W - CELL_GAP * 2) // 2

# 量出来的根高（做法同同档另外四张：根设 fit_content 渲一次读 PNG 高度）。
ROOT_H = 3060

SIDE_A = ("主动降噪", "ANC", "waves",
          "麦克风先录下环境噪声，喇叭再放一段反相声波把它抵消掉。")
SIDE_B = ("被动隔音", "Passive", "shield",
          "靠耳罩和耳塞的材料把声音挡在外面，不通电也一直在起作用。")

# (维度, A 侧一句话, B 侧一句话)
DIMENSIONS = [
    ("擅长哪种噪声",
     "低频连续：引擎、空调、地铁",
     "高频突发：键盘、餐具、人声"),
    ("代价是什么",
     "要电、有轻微底噪，一部分人戴着会耳压不适。",
     "要压耳，戴久了会闷热夹头，夏天尤其明显。"),
    ("怎么当场验证",
     "按一次开关，轰鸣明显变小。",
     "先别开电源，戴上安静下来的那部分就是隔音。"),
]

CHOICES = [
    ("通勤一小时以上、常坐地铁和飞机", "优先看主动降噪，这是它的主场。"),
    ("在开放办公室里想挡人声", "先换耳塞尺寸，把耳道封住比开降噪管用。"),
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
        chip("科普 · 一图讲清", bg="$c-accent", fg="$c-surface"),
        text(ids, "主标题", "降噪耳机\n降的到底是什么", 76, 700, "$c-surface",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.4),
        text(ids, "副标题", "两个常被混着说的词，其实是两套完全不同的做法。",
             28, 400, "$c-band-muted", family=CJK, line_height=LH_BODY),
    ])


# ------------------------------------------------------------------ 02 结论
def verdict():
    """结论先行。长图的第一屏必须给答案，剩下的都是它的展开。"""
    panel = col("结论面板", [
        text(ids, "结论", "它降的是低频、连续、稳定的噪声。", 44, 700,
             "$c-accent-deep", family=CJK, line_height=LH_HEAD, spacing=-0.8),
        text(ids, "结论支撑",
             "所以在地铁和飞机上最管用，在开放办公室里最不管用 —— "
             "人说话是高频突发声，它基本抵消不掉。",
             28, 400, "$c-ink", family=CJK, line_height=LH_BODY),
    ], gap=16, padding=[40, 36], cornerRadius=24)
    panel["fill"] = solid("$c-accent-soft")
    return band("02 结论", fill=[], pad=[64, EDGE, 0, EDGE], gap=0,
                children=[panel])


# ------------------------------------------------------------------ 03 定义
def define_card(name, latin, glyph, principle, *, filled):
    """两张定义卡。区分靠明度（实底反白 / 白底描边），不靠色相。"""
    ink = "$c-surface" if filled else "$c-ink"
    sub = "$c-accent-soft" if filled else "$c-muted"

    head = row("定义卡头", [
        icon_font(ids, "图标", glyph, 40, ink),
        text(ids, "概念名", name, 40, 700, ink, family=CJK,
             width="fit_content", growth="auto", line_height=LH_HEAD),
        text(ids, "西文名", latin, 24, 500, sub, family=NUM,
             width="fit_content", growth="auto", line_height=1.4),
    ], gap=16)

    card = col("定义卡", [
        head,
        text(ids, "原理", principle, 28, 400, sub, family=CJK,
             line_height=LH_BODY),
    ], gap=14, padding=[32, 34], cornerRadius=22)
    card["fill"] = solid("$c-accent-deep" if filled else "$c-surface")
    if not filled:
        card["stroke"] = {"thickness": 2, "fill": solid("$c-border")}
    return card


def definitions():
    return band("03 定义", fill=[], pad=[68, EDGE, 0, EDGE], gap=32,
                children=[
        section_head("先分清是哪两个东西",
                     "一句话说清各自的做法，剩下的差异都是从这一句推出来的。"),
        col("定义组", [
            define_card(*SIDE_A, filled=True),
            define_card(*SIDE_B, filled=False),
        ], gap=18),
    ])


# ------------------------------------------------------------------ 04 维度
def dimension_block(title, left, right, *, last=False):
    """一个维度：标题横跨整宽，两栏各一句，中间一条竖线。

    竖线写 `height: fill_container`，靠外层 `alignItems="stretch"` 拿到确定
    高度 —— 两栏哪一栏文案长，线就跟着长，不需要为它写死像素。
    """
    cells = row("维度两栏", [
        text(ids, "A 侧", left, 27, 400, "$c-ink", family=CJK, width=CELL_W,
             line_height=LH_BODY),
        rect(ids, "竖分隔", width=RULE_W, height="fill_container",
             fill=solid("$c-rule")),
        text(ids, "B 侧", right, 27, 400, "$c-ink", family=CJK, width=CELL_W,
             line_height=LH_BODY),
    ], gap=CELL_GAP, align="stretch")

    kids = [
        text(ids, "维度名", title, 26, 600, "$c-accent-deep", family=CJK,
             line_height=1.4),
        cells,
    ]
    if not last:
        kids.append(rect(ids, "维度分割线", width="fill_container", height=2,
                         fill=solid("$c-border")))
    return col("维度块", kids, gap=18)


def dimensions():
    header_row = row("表头", [
        text(ids, "A 名", SIDE_A[0], 30, 700, "$c-accent-deep", family=CJK,
             width=CELL_W, line_height=1.4),
        rect(ids, "表头竖线占位", width=RULE_W, height=2, fill=[]),
        text(ids, "B 名", SIDE_B[0], 30, 700, "$c-ink", family=CJK,
             width=CELL_W, line_height=1.4),
    ], gap=CELL_GAP)

    blocks = [header_row,
              rect(ids, "表头下线", width="fill_container", height=2,
                   fill=solid("$c-rule"))]
    for index, (title, left, right) in enumerate(DIMENSIONS):
        blocks.append(dimension_block(title, left, right,
                                      last=index == len(DIMENSIONS) - 1))

    table = col("维度表", blocks, gap=22, padding=[CARD_PAD, CARD_PAD],
                cornerRadius=24)
    table["fill"] = solid("$c-surface")
    table["stroke"] = {"thickness": 2, "fill": solid("$c-border")}

    return band("04 维度", fill=[], pad=[68, EDGE, 0, EDGE], gap=32,
                children=[
        section_head("三个维度，一行一句",
                     "左边是主动降噪，右边是被动隔音。每格只放一句能验证的话。"),
        table,
    ])


# ------------------------------------------------------------------ 05 选择
def choices():
    items = []
    for scene, action in CHOICES:
        items.append(row("判据项", [
            icon_font(ids, "箭头", "corner-down-right", 28, "$c-accent-deep"),
            col("判据文案", [
                text(ids, "场景", scene, 28, 600, "$c-ink", family=CJK,
                     line_height=1.4),
                text(ids, "动作", action, 27, 400, "$c-muted", family=CJK,
                     line_height=LH_BODY),
            ], gap=8),
        ], gap=16, align="start"))
    panel = col("判据面板", items, gap=24, padding=[36, 34], cornerRadius=22)
    panel["fill"] = solid("$c-accent-soft")
    return band("05 选择", fill=[], pad=[68, EDGE, 68, EDGE], gap=32,
                children=[
        section_head("那你该买哪个",
                     "按自己一天里待得最久的那个场景选，不按参数表选。"),
        panel,
    ])


# ------------------------------------------------------------------ 06 页脚
def footer():
    return band("06 页脚", fill=solid("$c-band"), pad=[44, EDGE], gap=12,
                children=[
        text(ids, "口径",
             "以消费级头戴与入耳耳机的常见实现为准，职业听力防护另有标准。",
             24, 400, "$c-band-muted", family=CJK, line_height=1.6),
        row("署名行", [
            text(ids, "账号名", "@ 你的账号名", 26, 600, "$c-surface",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
            text(ids, "更新说明", "每周一张讲清一个概念", 24, 400,
                 "$c-band-muted", family=CJK, width="fit_content",
                 growth="auto", line_height=1.4),
        ], gap=16, width="fill_container"),
    ])


def build():
    page = frame(ids, "概念对比科普长图", width=W, height=ROOT_H,
                 layout="vertical", gap=0, fill=solid("$c-bg"),
                 clipContent=True)
    page["children"] = [header(), verdict(), definitions(), dimensions(),
                        choices(), footer()]
    page["x"], page["y"] = 0, 0
    return [page]


# 对比度（WCAG 相对亮度比，op-design-lint 的门槛是 2.0；数值实测）：
#   c-ink         on c-bg          16.81   c-muted      on c-bg          6.29
#   c-ink         on c-surface     18.10   c-muted      on c-surface     6.77
#   c-surface     on c-band        17.60   c-band-muted on c-band        6.85
#   c-accent-deep on c-surface     10.02   c-accent     on c-surface     7.02
#   c-accent-deep on c-accent-soft  7.86   c-ink        on c-accent-soft 14.19
#   c-surface     on c-accent-deep 10.02   c-accent-soft on c-accent-deep 7.86
#   c-surface     on c-accent       7.02   c-rule       on c-surface     2.15
# 承载文字的最低一对是 6.29。c-rule 只画两条 2px 分隔线（非文字图形），
# 2.15 低于 3.0 是刻意的：分隔线要能被看见但不能和文字抢，这里按「结构线」
# 而非「信息图形」处理 —— 拿掉它版面依然可读，它只是让两栏更好扫。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "概念对比科普长图")
