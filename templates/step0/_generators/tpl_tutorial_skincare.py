#!/usr/bin/env python3
"""skincare-routine-cards.op — 护肤步骤卡（1080×1350，4:5 六帧）

教程档里唯一一张**带用量刻度**的模板。护肤教程翻车九成不在顺序上，在用量
和间隔上：精华挤了半张脸的量、面霜抹完立刻叠下一层。所以每一步固定给三个
数：用量、停留多久、早晚哪次用。

风格取「温暖生活 / 电商详情页风」的克制版：玫瑰灰纸底 + 深梅强调 + 白卡。
和菜谱那张同属暖色系，但一个偏赤陶（黄相）、一个偏梅（红紫相），并排陈列
分得开。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：从护肤品本身的包装语言采——不是化妆品广告那种高饱和玫红，
    是精华瓶玻璃的灰梅、纸盒的暖白、印刷 logo 的深梅。
  - **收敛**：五档暖中性（纸 / 卡 / 图位 / 线 / 墨）+ 1 个有彩色深梅
    #6B3A52 + 它的一档极淡底。
  - **论证**：深梅（明度 0.26）而不是亮玫红：这类内容的读者要在**卫生间
    灯光下、屏幕调暗时**看，亮玫红在低亮度下会糊成一团粉，深梅的明度足够
    低，任何亮度下都能和纸底分开。有彩色只出现在序号、用量数字和步骤徽章
    上——瓶子照片才是这张卡的颜色来源。

### 负约束（本模板明令不做的事）

  - **不写成分功效。** 「烟酰胺 5% 提亮」这类断言涉及功效宣称，模板里预置
    一句就是替用户背书。只讲**顺序、用量、间隔**——这三件事对任何产品都成
    立，换品牌不用改。
  - 不写「早 C 晚 A」这类需要额外前提的公式；模板要对新手成立。
  - 不用亮玫红 / 粉紫渐变 / 闪粉质感。
  - 不用 emoji，不用手写体，不做「小仙女」语气。
  - 一步一屏，四步四屏。想加第五步就再开一张卡。
  - 步骤说明两行封顶，超了就把内容挪到「用量」那一格里。

硬契约：
  - 内容距边缘 ≥72px（这里 72）
  - 配色全部走 color_vars；换主色只需改 c-accent 与 c-accent-soft
  - 正文与背景对比度 ≥2.0（实测表见文件末尾）
  - **CJK 行高比西文全线高 0.2**：display 1.2，标题 1.35，正文 1.7
  - **CJK 负字距不超过 -0.02em**（72px 标题 → -1.4px = -0.019em）
  - 汉字走 Noto Sans SC，数字与拉丁走 Inter
  - 顶层 frame 显式写 x/y，六帧按 W+GAP 横排
  - 步骤页图位实测 936×702（1.33:1，4:3）——由页面余量反解，见 SLOT_H
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import (Ids, color_vars, frame, icon_font, rect, solid, stroke,
                   text, write_doc)

ids = Ids()

VARS = color_vars({
    "c-bg":          "#F8F2F3",
    "c-card":        "#FFFFFF",
    "c-slot":        "#EFE2E5",
    "c-line":        "#E3D3D7",
    "c-ink":         "#241A1E",
    "c-muted":       "#6F5A60",
    "c-accent":      "#6B3A52",
    "c-accent-soft": "#F0E1E7",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W, H, GAP = 1080, 1350, 120

# 3 板一行 —— 与 deck 体系（deckkit.BOARDS_PER_ROW）同一约定：多板模板在画布上
# 分行铺开，而不是拖成一长排。行间距比列间距多 240 不是手滑：画布在帧上方以
# **屏幕空间**固定偏移画帧名，缩到能整屏看时 120 文档像素只剩十几个屏幕像素，
# 第二行的帧名会压到上一行的板上。
BOARDS_PER_ROW = 3
ROW_GAP = GAP + 240
EDGE = 72
PAGE_GAP = 46

LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.35, 1.7

# 图位高度由页面余量反解，不是挑的：固定部分（页头 70 + 参数区 115 +
# 要点区 101 + 页脚 34 = 320）+ 上下 padding 144 + 四个 46 的 gap = 648，
# 1350 减掉就是 702（936×702 ≈ 4:3）。改这四块里任何一行的字号或行数，
# 都要把这个数重解一次，否则内容会被 clipContent 悄悄裁掉。
SLOT_H = 702

TOTAL = 6

# (序号, 步骤名, 图位提示, 用量, 停留, 频次, 要点, 常见错误)
STEPS = [
    ("01", "洁面", "洗面奶挤在手心", "一颗黄豆", "揉 40 秒", "早晚各一次",
     "先在手心打出泡再上脸，别拿泡沫直接搓。",
     "水温调到不烫手。热水洗完那种「特别干净」是屏障被洗掉了。"),
    ("02", "爽肤水", "化妆水倒在掌心", "覆盖满掌心", "拍到吸收", "早晚各一次",
     "洗完脸 30 秒内上，脸还是潮的时候效果最好。",
     "别用化妆棉反复擦。擦的是角质，不是脏东西。"),
    ("03", "精华", "精华滴管特写", "2 到 3 滴", "等 1 分钟", "早晚各一次",
     "点在额头、两颊、下巴四处，再由内向外推开。",
     "叠第二支精华前一定要等干，湿着叠等于互相稀释。"),
    ("04", "面霜", "面霜挖取量", "半个指节", "轻按 20 秒", "晚上一定要",
     "最后一步用来封住前面所有层，别省。",
     "白天出门前还得再加防晒，面霜不替代它。"),
]

CLOSING = [
    ("顺序记不住就记一句", "从最稀的到最稠的，一路往下叠。"),
    ("换新产品一次只换一个", "同时换两支，出问题你不知道是哪支。"),
    ("刺痛立刻停", "泛红发痒不是「在起效」，是不耐受。"),
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


def page(name, children, *, index, fill="$c-bg"):
    node = frame(ids, name, width=W, height=H, layout="vertical",
                 padding=[72, EDGE], gap=PAGE_GAP, fill=solid(fill),
                 clipContent=True)
    node["children"] = children
    node["x"] = (index % BOARDS_PER_ROW) * (W + GAP)
    node["y"] = (index // BOARDS_PER_ROW) * (H + ROW_GAP)
    return node


def footer(no, *, ink="$c-muted"):
    return row("页脚", [
        text(ids, "账号名", "@ 你的账号名", 24, 600, ink, family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
        text(ids, "页码", f"{no:02d} / {TOTAL:02d}", 24, 600, ink, family=NUM,
             width="fit_content", growth="auto", line_height=1.4),
    ], gap=16, justifyContent="space_between")


def tag(label, *, bg, fg):
    node = frame(ids, "标签", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[10, 22], cornerRadius=999,
                 alignItems="center", justifyContent="center", fill=solid(bg))
    node["children"] = [
        text(ids, "标签文字", label, 24, 600, fg, family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
    ]
    return node


# ------------------------------------------------------------------ 01 封面
def cover():
    head = col("封面头", [
        tag("护肤基础 · 四步", bg="$c-accent-soft", fg="$c-accent"),
        text(ids, "封面标题", "顺序对了\n用量才有意义", 72, 700, "$c-ink",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.4),
        text(ids, "封面副标题",
             "不讲成分，不推产品。只把四步的顺序、用量和间隔说清楚。",
             27, 400, "$c-muted", family=CJK, line_height=LH_BODY),
    ], gap=22)

    items = []
    for no, name, *_rest in STEPS:
        badge = frame(ids, "序号圆", width=52, height=52, layout="horizontal",
                      alignItems="center", justifyContent="center",
                      cornerRadius=999, fill=solid("$c-accent-soft"))
        badge["children"] = [
            text(ids, "序号", no, 24, 700, "$c-accent", family=NUM,
                 width="fit_content", growth="auto", line_height=1.0),
        ]
        entry = row("步骤预告", [
            badge,
            text(ids, "预告名", name, 34, 600, "$c-ink", family=CJK,
                 line_height=LH_HEAD),
            icon_font(ids, "预告箭头", "chevron-right", 24, "$c-muted"),
        ], gap=18, padding=[22, 26])
        entry["fill"] = solid("$c-card")
        entry["cornerRadius"] = 14
        entry["stroke"] = stroke("$c-line", 2)
        items.append(entry)

    body = col("封面主体", [head, col("步骤预告区", items, gap=14)], gap=52,
               height="fill_container", justifyContent="space_between")
    return page("01 封面", [body, footer(1)], index=0)


# ------------------------------------------------------------- 02-05 步骤页
def spec_grid(amount, dwell, when):
    """三个数并排。护肤教程真正稀缺的信息就是这三格。"""
    cells = []
    for index, (label, value) in enumerate((("用量", amount),
                                            ("停留", dwell),
                                            ("频次", when))):
        if index:
            cells.append(rect(ids, "竖分隔", width=2, height=52,
                              fill=solid("$c-line")))
        cells.append(col("参数格", [
            text(ids, "参数名", label, 21, 400, "$c-muted", family=CJK,
                 line_height=1.4),
            text(ids, "参数值", value, 28, 700, "$c-accent", family=CJK,
                 line_height=1.35),
        ], gap=4))
    grid = row("参数区", cells, gap=24, padding=[22, 26], align="center")
    grid["fill"] = solid("$c-card")
    grid["cornerRadius"] = 14
    grid["stroke"] = stroke("$c-line", 2)
    return grid


def step_page(index, no, name, hint, amount, dwell, when, cue, caution):
    badge = frame(ids, "步骤徽章", width=56, height=56, layout="horizontal",
                  alignItems="center", justifyContent="center",
                  cornerRadius=999, fill=solid("$c-accent"))
    badge["children"] = [
        text(ids, "步骤序号", no, 25, 700, "$c-card", family=NUM,
             width="fit_content", growth="auto", line_height=1.0),
    ]
    head = row("页头", [
        badge,
        text(ids, "步骤名", name, 52, 700, "$c-ink", family=CJK,
             line_height=LH_HEAD),
    ], gap=20, align="center")

    slot = frame(ids, "图位", width="fill_container", height=SLOT_H,
                 layout="vertical", gap=14, alignItems="center",
                 justifyContent="center", cornerRadius=18,
                 fill=solid("$c-slot"))
    slot["children"] = [
        icon_font(ids, "图位图标", "camera", 46, "$c-muted"),
        text(ids, "图位提示", hint, 28, 600, "$c-ink", family=CJK,
             align="center", line_height=LH_HEAD),
        text(ids, "图位规格", "拍手部特写，能看出量最好", 23, 400,
             "$c-muted", family=CJK, align="center", line_height=LH_BODY),
    ]

    def note(icon, label):
        return row("要点", [
            icon_font(ids, "要点图标", icon, 25, "$c-accent"),
            text(ids, "要点文字", label, 25, 400, "$c-muted", family=CJK,
                 line_height=LH_BODY),
        ], gap=14, align="start")

    notes = col("要点区", [note("check", cue), note("info", caution)], gap=16)
    return page(f"{index:02d} {name}",
                [head, spec_grid(amount, dwell, when), slot, notes,
                 footer(index)], index=index - 1)


# ------------------------------------------------------------------ 06 收尾
def closing():
    head = col("页头", [
        tag("收尾", bg="$c-ink", fg="$c-card"),
        text(ids, "收尾标题", "记住这三句\n比记住品牌有用", 60, 700, "$c-ink",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.1),
    ], gap=20)

    items = []
    for title, desc in CLOSING:
        card = col("收尾卡", [
            text(ids, "收尾小标题", title, 30, 600, "$c-ink", family=CJK,
                 line_height=LH_HEAD),
            text(ids, "收尾说明", desc, 25, 400, "$c-muted", family=CJK,
                 line_height=LH_BODY),
        ], gap=8, padding=[28, 28])
        card["fill"] = solid("$c-card")
        card["cornerRadius"] = 16
        card["stroke"] = stroke("$c-line", 2)
        items.append(card)

    cta = col("关注卡", [
        text(ids, "关注标题", "把你的顺序发出来", 36, 700, "$c-card",
             family=CJK, line_height=LH_HEAD),
        text(ids, "关注副文案", "评论区写下你现在的四步，我帮你看看顺序。",
             25, 400, "$c-accent-soft", family=CJK, line_height=LH_BODY),
    ], gap=10, padding=[34, 34])
    cta["fill"] = solid("$c-accent")
    cta["cornerRadius"] = 18

    body = col("收尾主体", [col("收尾列表", items, gap=16), cta], gap=32,
               height="fill_container", justifyContent="space_between")
    return page("06 收尾", [head, body, footer(6)], index=5)


def build():
    pages = [cover()]
    for index, step in enumerate(STEPS, 2):
        pages.append(step_page(index, *step))
    pages.append(closing())
    return pages


# 对比度（WCAG 相对亮度比，op-design-lint 门槛 2.0；数值由脚本实测）：
#   c-ink    on c-bg      15.29   c-muted on c-bg       5.74
#   c-ink    on c-card    16.91   c-muted on c-card     6.35
#   c-ink    on c-slot    13.44   c-muted on c-slot     5.04
#   c-accent on c-bg       8.07   c-accent on c-card     8.93
#   c-card   on c-accent   8.93   c-accent-soft on c-accent 7.07
#   c-accent on c-accent-soft 7.07   c-card on c-ink    16.91
# 承载正文的最低一对是 5.04（图位规格压在 c-slot 上），高于 AA 正文门槛
# 4.5。深梅这一档在四种底上都在 7 以上——这正是选深梅而不是亮玫红的收益。
# c-line 只用于 hairline 描边与竖分隔，是非文字图形。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "护肤步骤卡 · 4:5 六帧")
