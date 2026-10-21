#!/usr/bin/env python3
"""storage-makeover-steps.op — 家居收纳改造步骤（1080×1440，3:4 六帧）

教程档里唯一一张**每一步都给「完成判定」**的模板。收纳教程最大的问题是没
有终点：读者不知道「分类」到什么程度算分完，于是永远停在半途。所以每一步
除了动作和图位，固定给一条判定标准和一个耗时预算——做到那个状态就可以进
下一步，不用纠结。

风格取「温暖生活 / 白底杂志风」的克制版：燕麦纸底 + 墨绿 + 细线。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：从收纳这件事的成品采——藤编与棉麻收纳箱的燕麦色、标签纸的
    白、深色木架的墨绿褐。不是家居广告那种莫兰迪灰粉。
  - **收敛**：四档暖中性（纸 0.90 / 卡 1.0 / 图位 0.84 / 线 0.79）+ 1 个
    有彩色墨绿 #3B5A46 + 它的一档极淡底。
  - **论证**：墨绿只标**「判定标准」**——每一步那个「做到这样才算完成」的
    框、序号徽章和耗时数字。改造过程的照片会带进大量杂色（衣服、盒子、木
    纹），版面必须比照片更安静，所以只留一个低饱和深色相；墨绿的明度
    （0.29）足够低，压在燕麦纸上是稳的，不会像浅绿那样和照片抢。
  - 和同为暖纸底的手工那张（牛皮 #E8DCC6 + 墨蓝）刻意错开：这张的纸更浅
    更黄（#F4F0E6），墨是绿而不是蓝，两张并排陈列一眼能分。

### 负约束（本模板明令不做的事）

  - **不做 before / after 对比。** 仓里已经有 `before-after` 那一档，这张
    走的是「过程分步」；两张放一起必须是互补而不是同题。每一步只给一个图
    位，拍的是**动作中**的状态。
  - 不用莫兰迪灰粉、不用 ins 风奶油滤镜、不用木纹贴图。
  - 不用第二个有彩色。墨绿只标判定标准与耗时。
  - 不给「收纳神器」清单，也不给任何商品链接位——那是带货，不是教程。
  - 每一步的判定标准必须是**能用眼睛验证的状态**（地面空出来了、同类都在
    一个盒子里），不能是「整理好了」这种自我评价。
  - 一步一屏，四步四屏。加第五步就再开一张卡。

硬契约：
  - 内容距边缘 ≥72px（这里 72）
  - 配色全部走 color_vars；换主色只需改 c-accent 与 c-accent-soft
  - 正文与背景对比度 ≥2.0（实测表见文件末尾）
  - **CJK 行高比西文全线高 0.2**：display 1.2，标题 1.35，正文 1.7
  - **CJK 负字距不超过 -0.02em**（68px 标题 → -1.3px = -0.019em）
  - 汉字走 Noto Sans SC，数字与拉丁走 Inter
  - 顶层 frame 显式写 x/y，六帧按 W+GAP 横排
  - 步骤页图位实测 936×842（1.11:1）；高度由页面余量反解，见 SLOT_H
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import (Ids, color_vars, frame, icon_font, rect, solid, stroke,
                   text, write_doc)

ids = Ids()

VARS = color_vars({
    "c-bg":          "#F4F0E6",
    "c-card":        "#FFFFFF",
    "c-slot":        "#E7E2D3",
    "c-line":        "#DCD6C4",
    "c-ink":         "#22261E",
    # 次级墨。第一版 #6B6D5E 压在 c-slot 上只有 4.08，图位那行 23px/400
    # 的规格说明正好落在上面，差一点到 AA 正文门槛，整体压深一档。
    "c-muted":       "#5D5F4C",
    "c-accent":      "#3B5A46",
    "c-accent-soft": "#E1E8DF",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W, H, GAP = 1080, 1440, 120

# 3 板一行 —— 与 deck 体系（deckkit.BOARDS_PER_ROW）同一约定：多板模板在画布上
# 分行铺开，而不是拖成一长排。行间距比列间距多 240 不是手滑：画布在帧上方以
# **屏幕空间**固定偏移画帧名，缩到能整屏看时 120 文档像素只剩十几个屏幕像素，
# 第二行的帧名会压到上一行的板上。
BOARDS_PER_ROW = 3
ROW_GAP = GAP + 240
EDGE = 72
PAGE_GAP = 34

LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.35, 1.7

TOTAL = 6

# 图位高度是量出来的：把它设成任意值渲一遍，读 snapshot_layout 里各块高度
# 的和跟 1440 的差额直接回补（两次收敛）。改页头/判定框/要点区任何一行都
# 要重量——超了会被 clipContent 裁掉，页脚先没。
SLOT_H = 842

# (序号, 步骤名, 耗时, 图位提示, 判定标准, 要点, 常见错误)
STEPS = [
    ("01", "全部倒出来", "20 分钟", "东西全摊在地上",
     "柜子里空到能看见底板，一件不剩。",
     "一次只做一个柜子。倒两个柜子你会中途放弃。",
     "别边倒边整理。倒和分是两件事，混着做两件都做不完。"),
    ("02", "只分三堆", "30 分钟", "地上分好的三堆",
     "地上只剩三堆：留、送走、扔。没有第四堆。",
     "拿不准的一律进「送走」堆，一个月后没想起来就真送走。",
     "别设「以后再说」堆。那一堆最后会原样塞回柜子。"),
    ("03", "按拿取频率归位", "25 分钟", "柜内分区示意",
     "每天要用的都在伸手就够到的那一层。",
     "常用的放腰到肩之间，不常用的放最高最低两层。",
     "别按颜色或大小排。好看维持不过两周，顺手能维持两年。"),
    ("04", "贴标签", "15 分钟", "贴好标签的收纳盒",
     "每个不透明的盒子外面都有一张能看清的标签。",
     "标签写「里面是什么」，不写「谁的」——归位的人才看得懂。",
     "别用手写便利贴。三个月后会卷边掉下来，然后没人补。"),
]

RULES = [
    ("每天花两分钟归位", "两分钟维持得住，周末两小时补不回来。"),
    ("进一件出一件", "柜子的容量是固定的，这条不遵守迟早再来一遍。"),
    ("空出一格不要填", "留白是缓冲，塞满的柜子第一天就开始乱。"),
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


def page(name, children, *, index):
    node = frame(ids, name, width=W, height=H, layout="vertical",
                 padding=[76, EDGE, 68, EDGE], gap=PAGE_GAP,
                 fill=solid("$c-bg"), clipContent=True)
    node["children"] = children
    node["x"] = (index % BOARDS_PER_ROW) * (W + GAP)
    node["y"] = (index // BOARDS_PER_ROW) * (H + ROW_GAP)
    return node


def footer(no):
    return row("页脚", [
        text(ids, "账号名", "@ 你的账号名", 24, 600, "$c-muted", family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
        text(ids, "页码", f"{no:02d} / {TOTAL:02d}", 24, 600, "$c-muted",
             family=NUM, width="fit_content", growth="auto", line_height=1.4),
    ], gap=16, justifyContent="space_between")


def tag(label, *, bg, fg):
    node = frame(ids, "标签", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[10, 20], cornerRadius=6,
                 alignItems="center", justifyContent="center", fill=solid(bg))
    node["children"] = [
        text(ids, "标签文字", label, 24, 600, fg, family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
    ]
    return node


# ------------------------------------------------------------------ 01 封面
def cover():
    head = col("封面头", [
        tag("收纳改造 · 四步", bg="$c-accent-soft", fg="$c-accent"),
        text(ids, "封面标题", "一个柜子\n一个下午", 68, 700, "$c-ink",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.3),
        text(ids, "封面副标题",
             "每一步都给一条判定标准：做到那个样子就往下走，不用纠结。",
             27, 400, "$c-muted", family=CJK, line_height=LH_BODY),
    ], gap=22)

    items = []
    for no, name, cost, *_rest in STEPS:
        badge = frame(ids, "序号方", width=52, height=52, layout="horizontal",
                      alignItems="center", justifyContent="center",
                      cornerRadius=10, fill=solid("$c-accent-soft"))
        badge["children"] = [
            text(ids, "序号", no, 23, 700, "$c-accent", family=NUM,
                 width="fit_content", growth="auto", line_height=1.0),
        ]
        entry = row("步骤预告", [
            badge,
            text(ids, "预告名", name, 32, 600, "$c-ink", family=CJK,
                 line_height=LH_HEAD),
            text(ids, "预告耗时", cost, 24, 600, "$c-accent", family=CJK,
                 width="fit_content", growth="auto", line_height=1.4),
        ], gap=18, padding=[22, 26])
        entry["fill"] = solid("$c-card")
        entry["cornerRadius"] = 14
        entry["stroke"] = stroke("$c-line", 2)
        items.append(entry)

    total = row("合计条", [
        icon_font(ids, "合计图标", "clock", 26, "$c-accent"),
        text(ids, "合计文字", "四步合计 90 分钟，中间可以停。", 25, 500,
             "$c-ink", family=CJK, line_height=1.5),
    ], gap=12, padding=[18, 22])
    total["fill"] = solid("$c-accent-soft")
    total["cornerRadius"] = 12

    body = col("封面主体", [head, col("预告区", items, gap=14), total],
               gap=40, height="fill_container",
               justifyContent="space_between")
    return page("01 封面", [body, footer(1)], index=0)


# ------------------------------------------------------------- 02-05 步骤页
def step_page(index, no, name, cost, hint, done, cue, mistake):
    badge = frame(ids, "步骤徽章", width=56, height=56, layout="horizontal",
                  alignItems="center", justifyContent="center",
                  cornerRadius=12, fill=solid("$c-accent"))
    badge["children"] = [
        text(ids, "步骤序号", no, 25, 700, "$c-card", family=NUM,
             width="fit_content", growth="auto", line_height=1.0),
    ]
    head = row("页头", [
        badge,
        text(ids, "步骤名", name, 50, 700, "$c-ink", family=CJK,
             line_height=LH_HEAD),
        text(ids, "耗时", cost, 26, 600, "$c-accent", family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
    ], gap=18, align="center")

    slot = frame(ids, "图位", width="fill_container", height=SLOT_H,
                 layout="vertical", gap=14, alignItems="center",
                 justifyContent="center", cornerRadius=16,
                 fill=solid("$c-slot"))
    slot["children"] = [
        icon_font(ids, "图位图标", "camera", 44, "$c-muted"),
        text(ids, "图位提示", hint, 28, 600, "$c-ink", family=CJK,
             align="center", line_height=LH_HEAD),
        text(ids, "图位规格", "拍过程中的样子，别等收拾完再补拍", 23, 400,
             "$c-muted", family=CJK, align="center", line_height=LH_BODY),
    ]

    # 判定框：这张模板的招牌组件。每一步都长一样，位置也一样。
    check = row("判定框", [
        icon_font(ids, "判定图标", "circle-check", 28, "$c-accent"),
        col("判定文案", [
            text(ids, "判定标题", "做到这样才算完成", 22, 700, "$c-accent",
                 family=CJK, line_height=1.4),
            text(ids, "判定标准", done, 25, 500, "$c-ink", family=CJK,
                 line_height=LH_BODY),
        ], gap=4),
    ], gap=14, align="start", padding=[22, 24])
    check["fill"] = solid("$c-accent-soft")
    check["cornerRadius"] = 12

    def note(icon, label):
        return row("要点", [
            icon_font(ids, "要点图标", icon, 24, "$c-muted"),
            text(ids, "要点文字", label, 24, 400, "$c-muted", family=CJK,
                 line_height=LH_BODY),
        ], gap=12, align="start")

    notes = col("要点区", [note("check", cue), note("circle-x", mistake)],
                gap=14)
    return page(f"{index:02d} {name}",
                [head, slot, check, notes, footer(index)], index=index - 1)


# ------------------------------------------------------------------ 06 收尾
def closing():
    head = col("页头", [
        tag("维持", bg="$c-ink", fg="$c-card"),
        text(ids, "收尾标题", "收拾完只是开始\n维持才是本体", 58, 700,
             "$c-ink", family=CJK, line_height=LH_DISPLAY, spacing=-1.1),
    ], gap=20)

    items = []
    for title, desc in RULES:
        card = col("规则卡", [
            text(ids, "规则标题", title, 30, 600, "$c-ink", family=CJK,
                 line_height=LH_HEAD),
            text(ids, "规则说明", desc, 24, 400, "$c-muted", family=CJK,
                 line_height=LH_BODY),
        ], gap=8, padding=[26, 28])
        card["fill"] = solid("$c-card")
        card["cornerRadius"] = 16
        card["stroke"] = stroke("$c-line", 2)
        items.append(card)

    cta = col("关注卡", [
        text(ids, "关注标题", "改造完拍一张", 36, 700, "$c-card", family=CJK,
             line_height=LH_HEAD),
        text(ids, "关注副文案", "评论区贴出来，我帮你看看哪一层还能再省。",
             24, 400, "$c-accent-soft", family=CJK, line_height=LH_BODY),
    ], gap=10, padding=[32, 34])
    cta["fill"] = solid("$c-accent")
    cta["cornerRadius"] = 18

    body = col("收尾主体", [col("规则区", items, gap=16), cta], gap=32,
               height="fill_container", justifyContent="space_between")
    return page("06 维持", [head, body, footer(6)], index=5)


def build():
    pages = [cover()]
    for index, step in enumerate(STEPS, 2):
        pages.append(step_page(index, *step))
    pages.append(closing())
    return pages


# 对比度（WCAG 相对亮度比，op-design-lint 门槛 2.0；数值由脚本实测）：
#   c-ink    on c-bg      13.53   c-muted on c-bg       5.75
#   c-ink    on c-card    15.40   c-muted on c-card      6.55
#   c-ink    on c-slot    11.89   c-muted on c-slot      5.06
#   c-accent on c-bg       6.74   c-accent on c-card     7.67
#   c-card   on c-accent   7.67   c-accent on c-accent-soft 6.14
# 承载正文的最低一对是 5.06（图位规格压在 c-slot 上），高于 AA 正文门槛
# 4.5。墨绿这一档在三种底上都在 6 以上——低饱和深色相的收益。c-line 只用
# 于 hairline 描边，是非文字图形。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "家居收纳改造步骤 · 3:4 六帧")
