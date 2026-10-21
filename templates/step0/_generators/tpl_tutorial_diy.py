#!/usr/bin/env python3
"""diy-blueprint-guide.op — 手工 DIY 图解长图（1080×N 竖版）

教程档里唯一一张**带材料规格表**的模板。手工教程失败通常不在手上，在准备
上：木条买短了 5 厘米、螺丝买错了型号。所以这张图把「材料 + 规格 + 数量」
放在最前面，占的篇幅和步骤一样多。

风格取「蓝图 / 施工图」：牛皮纸底 + 墨蓝线稿 + 尺寸标注。这是九张教程里唯
一一张**只有两个色相**的（纸的暖褐 + 墨的蓝），因为蓝图本来就只有两色——
这个约束不是省事，是这套视觉语言的定义。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：从实物采——牛皮纸包装的暖褐、施工图晒图的墨蓝、裁切标记的
    白。三者都来自「动手做东西」这件事本身的物料。
  - **收敛**：三档暖褐（纸 0.86 / 卡 0.93 / 线 0.75）+ 两档墨蓝（墨 0.19 /
    次级 0.38）+ 一档白。**没有第三个色相，也没有「强调色」这个角色**。
  - **论证**：层级不靠颜色，靠**线宽和实心块**：区块头是一条 8px 墨蓝短
    线，重点是整块墨蓝反白，尺寸标注是 2px 细线。这正是施工图的做法——图
    纸上从来只有一种墨，全靠线型区分含义。多给一个强调色，尺寸标注和步骤
    序号就会开始抢同一个「重要」的位置。

### 负约束（本模板明令不做的事）

  - **不引入第三个色相。** 破了这条，蓝图这套语言就散了（见配色论证）。
  - 不用手账贴纸、胶带、图钉、木纹贴图这些「手作感」装饰——它们把牛皮纸底
    变成廉价拼贴，而且全都要占位置。
  - 不做圆角超过 8px 的卡片。施工图上的框是直角，8px 已经是妥协。
  - 不用 emoji 工具图标；工具与材料一律走单色线性 lucide。
  - 材料规格必须写单位和数量，写不出具体规格的材料就不该进清单。
  - 步骤说明一到两行；需要三行说明的步骤要拆成两步。
  - 不写「超简单 / 零基础 / 十分钟搞定」——手工教程的时间随熟练度差三倍，
    给一个数就是骗人。用「难度」和「需要的工具」代替。

硬契约：
  - 内容距边缘 ≥80px（这里 80）
  - 配色全部走 color_vars；这张没有「主色」可换，要改只能整体换墨色
  - 正文与背景对比度 ≥2.0（实测表见文件末尾）
  - **CJK 行高比西文全线高 0.2**：display 1.2，标题 1.3，正文 1.7
  - **CJK 负字距不超过 -0.02em**（74px 标题 → -1.4px = -0.019em）
  - 汉字走 Noto Sans SC，数字与拉丁走 Inter（规格表全是数字，最吃这条）
  - 顶层 frame 显式写 x/y
  - 根高固定：ROOT_H 是量出来的（根设 fit_content 渲一次读高度）
  - 六个步骤图位实测各 410×230（1.78:1）；成品图位 920×420；根高 4016 由 fit_content
    量出后回填
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import (Ids, color_vars, frame, icon_font, rect, solid, stroke,
                   text, write_doc)

ids = Ids()

VARS = color_vars({
    "c-bg":        "#E8DCC6",
    "c-card":      "#F6EFE1",
    "c-slot":      "#DDCFB6",
    "c-line":      "#C4B294",
    "c-ink":       "#1B3350",
    # 次级墨。第一版 #4E6076 压在 c-slot 上只有 4.20，差一点点到 AA 正文
    # 门槛；成品图位那行 23px/400 的规格说明正好落在那上面，所以整体
    # 压深一档到 4.82。
    "c-muted":     "#475769",
    "c-inv":       "#F6EFE1",
    "c-inv-muted": "#A9BACC",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W = 1080
EDGE = 80
INNER = W - EDGE * 2

LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.3, 1.7

# 量出来的根高（根设 fit_content 渲一次读高度回填）。
ROOT_H = 4016

STEP_SLOT_H = 230
HERO_SLOT_H = 420

# (材料, 规格, 数量)。写不出规格的不进清单——见负约束。
MATERIALS = [
    ("松木条", "2×4 cm 截面 · 长 60 cm", "4 根"),
    ("胶合板", "厚 12 mm · 40×30 cm", "1 块"),
    ("自攻螺丝", "3.5×35 mm", "16 颗"),
    ("砂纸", "180 目 + 320 目", "各 1 张"),
    ("木蜡油", "透明色 · 100 ml", "1 罐"),
]

TOOLS = [("手锯", "hammer"), ("电钻", "drill"), ("卷尺", "ruler"),
         ("角尺", "pencil-ruler")]

# (序号, 步骤名, 一到两行说明, 图位提示)
STEPS = [
    ("01", "量好再画线", "四根木条都量成 60 cm，用角尺画满一圈再锯。",
     "画线特写"),
    ("02", "锯口留在线外", "锯片压着线的外侧走，锯完刚好到线。", "锯切中"),
    ("03", "先打引导孔", "先打 3 mm 孔，直接上螺丝会裂。", "钻孔位置"),
    ("04", "拧螺丝固定框", "对角线交替拧，一次只拧到八成紧。", "拧螺丝"),
    ("05", "上面板", "胶合板压在框上，四边各两颗螺丝。", "面板压合"),
    ("06", "打磨上油", "180 目磨平，320 目收光，再薄涂两遍油。", "打磨中"),
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
    """区块头。一条 8px 墨蓝短线 —— 全篇唯一的「强调」手段。"""
    return col("区块头", [
        rect(ids, "强调短线", width=72, height=8, fill=solid("$c-ink")),
        text(ids, "区块标题", title, 44, 700, "$c-ink", family=CJK,
             line_height=LH_HEAD),
        text(ids, "区块说明", note, 26, 400, "$c-muted", family=CJK,
             line_height=LH_BODY),
    ], gap=14)


# ------------------------------------------------------------------ 01 页头
def header():
    tag = frame(ids, "标签", width="fit_content", height="fit_content",
                layout="horizontal", padding=[10, 20], cornerRadius=4,
                alignItems="center", justifyContent="center",
                fill=solid("$c-inv"))
    tag["children"] = [
        text(ids, "标签文字", "木工 · 入门件", 24, 700, "$c-ink", family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
    ]

    metas = []
    for index, (label, value) in enumerate((("难度", "入门"),
                                            ("需要电动工具", "只要电钻"),
                                            ("成品尺寸", "60×40×35 cm"))):
        if index:
            metas.append(rect(ids, "竖分隔", width=2, height=46,
                              fill=solid("$c-muted")))
        metas.append(col("总览项", [
            text(ids, "总览名", label, 21, 400, "$c-inv-muted", family=CJK,
                 width="fit_content", growth="auto", line_height=1.4),
            text(ids, "总览值", value, 27, 700, "$c-inv", family=CJK,
                 width="fit_content", growth="auto", line_height=1.3),
        ], gap=4, width="fit_content"))

    return band("01 页头", fill=solid("$c-ink"), pad=[76, EDGE, 68, EDGE],
                gap=26, children=[
        tag,
        text(ids, "主标题", "一张边几\n六步做完", 74, 700, "$c-inv",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.4),
        text(ids, "副标题",
             "先把材料按规格备齐，剩下六步就只是重复：量、锯、钻、拧。",
             27, 400, "$c-inv-muted", family=CJK, line_height=LH_BODY),
        row("总览条", metas, gap=26, width="fit_content"),
    ])


# ------------------------------------------------------------------ 02 材料
def materials():
    rows = []
    for index, (name, spec, count) in enumerate(MATERIALS):
        if index:
            rows.append(rect(ids, "横分隔", width="fill_container", height=2,
                             fill=solid("$c-line")))
        rows.append(row("材料行", [
            col("材料名区", [
                text(ids, "材料名", name, 29, 600, "$c-ink", family=CJK,
                     line_height=LH_HEAD),
                text(ids, "材料规格", spec, 23, 400, "$c-muted", family=CJK,
                     line_height=1.5),
            ], gap=4),
            text(ids, "材料数量", count, 27, 700, "$c-ink", family=CJK,
                 width=120, align="right", line_height=1.3),
        ], gap=20, align="center", padding=[18, 0]))

    sheet = col("材料表", rows, gap=0, padding=[26, 28])
    sheet["fill"] = solid("$c-card")
    sheet["stroke"] = stroke("$c-line", 2)
    sheet["cornerRadius"] = 8

    tool_chips = []
    for label, glyph in TOOLS:
        chip = row("工具", [
            icon_font(ids, "工具图标", glyph, 24, "$c-ink"),
            text(ids, "工具名", label, 24, 600, "$c-ink", family=CJK,
                 width="fit_content", growth="auto", line_height=1.4),
        ], gap=10, width="fit_content", padding=[12, 18])
        chip["stroke"] = stroke("$c-ink", 2)
        chip["cornerRadius"] = 6
        tool_chips.append(chip)

    return band("02 材料", fill=[], pad=[64, EDGE, 0, EDGE], gap=26,
                children=[
        section_head("先备料，别边做边买",
                     "规格照抄就行；差 5 mm 也能装，差 5 cm 装不上。"),
        sheet,
        row("工具条", tool_chips, gap=14, width="fill_container"),
    ])


# ------------------------------------------------------------------ 03 步骤
def step_card(no, name, desc, hint):
    slot = frame(ids, "步骤图位", width="fill_container", height=STEP_SLOT_H,
                 layout="vertical", gap=8, alignItems="center",
                 justifyContent="center", cornerRadius=6,
                 fill=solid("$c-slot"))
    slot["children"] = [
        icon_font(ids, "图位图标", "camera", 32, "$c-muted"),
        text(ids, "图位提示", hint, 23, 600, "$c-ink", family=CJK,
             align="center", line_height=1.4),
    ]
    card = col("步骤卡", [
        slot,
        row("步骤头", [
            text(ids, "步骤序号", no, 30, 700, "$c-ink", family=NUM,
                 width=54, line_height=1.2),
            text(ids, "步骤名", name, 28, 600, "$c-ink", family=CJK,
                 line_height=LH_HEAD),
        ], gap=8, align="center"),
        text(ids, "步骤说明", desc, 23, 400, "$c-muted", family=CJK,
             line_height=LH_BODY),
    ], gap=14, padding=[20, 20], height="fill_container")
    card["fill"] = solid("$c-card")
    card["cornerRadius"] = 8
    card["stroke"] = stroke("$c-line", 2)
    return card


def steps():
    cards = [step_card(*s) for s in STEPS]
    grid_rows = []
    for start in (0, 2, 4):
        grid_rows.append(row("步骤排", cards[start:start + 2], gap=20,
                             align="stretch"))
    return band("03 步骤", fill=[], pad=[64, EDGE, 0, EDGE], gap=26,
                children=[
        section_head("六步，按顺序来",
                     "每一步做完都停下来量一次，错了这时候还能救。"),
        col("步骤网格", grid_rows, gap=20),
    ])


# ------------------------------------------------------------------ 04 成品
def result():
    slot = frame(ids, "成品图位", width="fill_container", height=HERO_SLOT_H,
                 layout="vertical", gap=12, alignItems="center",
                 justifyContent="center", cornerRadius=8,
                 fill=solid("$c-slot"))
    slot["children"] = [
        icon_font(ids, "成品图标", "image", 46, "$c-muted"),
        text(ids, "成品提示", "把你做完的成品拍进来", 28, 600, "$c-ink",
             family=CJK, align="center", line_height=LH_HEAD),
        text(ids, "成品规格", "建议 16:9，光从侧面打，木纹才看得出",
             23, 400, "$c-muted", family=CJK, align="center",
             line_height=LH_BODY),
    ]
    return band("04 成品", fill=[], pad=[64, EDGE, 0, EDGE], gap=26,
                children=[
        section_head("做完长这样", "尺寸可以改，结构不要改——四条腿的受力靠框。"),
        slot,
    ])


# ------------------------------------------------------------------ 05 页脚
CHECKS = [
    "锯之前把线画满一圈，只画一面必歪。",
    "螺丝拧不进去先加深引导孔，别硬拧。",
    "上油前把粉尘擦干净，擦不净会留白点。",
]


def footer():
    items = []
    for line in CHECKS:
        box = frame(ids, "方框", width=36, height=36, layout="horizontal",
                    alignItems="center", justifyContent="center",
                    fill=solid("$c-inv"))
        box["children"] = [icon_font(ids, "勾", "check", 22, "$c-ink")]
        items.append(row("自检项", [
            box,
            text(ids, "自检文字", line, 25, 400, "$c-inv-muted", family=CJK,
                 line_height=LH_BODY),
        ], gap=16, align="start"))

    return band("05 页脚", fill=solid("$c-ink"), pad=[60, EDGE, 52, EDGE],
                gap=24, children=[
        text(ids, "页脚标题", "开工前再看一眼", 40, 700, "$c-inv", family=CJK,
             line_height=LH_HEAD),
        col("自检列表", items, gap=14),
        row("署名行", [
            text(ids, "账号名", "@ 你的账号名", 25, 600, "$c-inv", family=CJK,
                 width="fit_content", growth="auto", line_height=1.4),
            text(ids, "更新说明", "每月一件能自己做的家具", 23, 400,
                 "$c-inv-muted", family=CJK, width="fit_content",
                 growth="auto", line_height=1.4),
        ], gap=16, justifyContent="space_between"),
    ])


def build():
    page = frame(ids, "手工图解长图", width=W, height=ROOT_H, layout="vertical",
                 gap=0, fill=solid("$c-bg"), clipContent=True)
    page["children"] = [header(), materials(), steps(), result(), footer()]
    page["x"], page["y"] = 0, 0
    return [page]


# 对比度（WCAG 相对亮度比，op-design-lint 门槛 2.0；数值由脚本实测）：
#   c-ink   on c-bg     9.46   c-muted on c-bg     5.46
#   c-ink   on c-card  11.22   c-muted on c-card   6.47
#   c-ink   on c-slot   8.36   c-muted on c-slot   4.82
#   c-inv   on c-ink   11.22   c-inv-muted on c-ink 6.47
# 承载正文的最低一对是 4.82（成品图位的规格说明压在 c-slot 上），高于 AA
# 正文门槛 4.5。整张图只有两个色相，所以这张表也只有四行——没有「强调色
# 压在什么底上」这一类需要单独交代的组合。c-line 只用于 hairline 与分隔，
# 是非文字图形。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "手工 DIY 图解长图")
