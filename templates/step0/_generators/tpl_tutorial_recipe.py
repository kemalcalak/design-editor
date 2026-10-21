#!/usr/bin/env python3
"""recipe-four-step.op — 菜谱四步卡（1080×1350，4:5 单卡·2×2 四宫格）

教程档里唯一一张**四宫格**。别的教程都是「一步一屏」，这张反着来：四步全
放在一张卡上，一眼看完、截图存相册就能照着做——菜谱的真实使用场景是站在
灶台前单手划手机，翻页是负担。

风格取「高级白底杂志风」：暖白纸、细线、字重对比撑层级，不靠色块堆。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：从「食物本身」采——葱油面这类家常面食的记忆色是酱色与焦香
    的暖褐红，不是餐饮 App 那种高饱和番茄红。取一档偏砖的赤陶 #B8452E。
  - **收敛**：一个有彩色（赤陶）+ 一档极淡的同色相底（#F6E4DE，只给序号
    与提示条）+ 五档暖中性（纸 / 卡 / 线 / 次级 / 墨）。
  - **论证**：菜谱卡上真正该有颜色的是**照片**。版面给到第二个有彩色，四
    张成品图就会跟版面打架；所以有彩色收到只剩序号圆与那条提示带，其余全
    部让位。暖中性而不是冷灰：冷灰底会把食物照片衬得发青。

### 负约束（本模板明令不做的事）

  - **不给菜谱配「营养成分表」式的伪数据。** 卡路里 / 蛋白质克数没有可信
    来源就是编的，编出来的数字比没有数字更糟。只给时间、份量、难度这三个
    做饭的人自己就能验证的量。
  - 不用第二个有彩色。四张照片就是这张卡的颜色。
  - 不用阴影。卡片边界由 1px 暖灰 hairline 给。
  - 不用 emoji（🍜🔥）当装饰，不用手写体标题，不做「厨房黑板」贴纸感。
  - 步骤说明一行封顶。写不下说明这一步该拆，不缩字号。
  - 不写「秘制 / 灵魂 / 绝了」这类种草黑话，每步都写成可执行的动作 + 一个
    可验证的量（秒数、火力、水位）。

硬契约：
  - 内容距边缘 ≥72px（这里 72）
  - 配色全部走 color_vars；换主色只需改 c-accent 与 c-accent-soft
  - 正文与背景对比度 ≥2.0（实测表见文件末尾）
  - **CJK 行高比西文全线高 0.2**：display 1.2，标题 1.35，正文 1.7
  - **CJK 负字距不超过 -0.02em**（72px 标题 → -1.4px = -0.019em）
  - 汉字走 Noto Sans SC，数字与拉丁走 Inter
  - 顶层 frame 显式写 x/y
  - 四张图位实测各 416×235（1.77:1，≈16:9）——高度写死，见 step_card
    注释；改页头/指标条/页脚任何一处高度都要重算这个数
  - 同一行的两张卡必须同为 fill_container，否则 taffy 会让先声明的那张吃
    掉整行宽度
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import (Ids, color_vars, frame, icon_font, rect, solid, stroke,
                   text, write_doc)

ids = Ids()

VARS = color_vars({
    "c-bg":          "#FBF7F0",
    "c-card":        "#FFFFFF",
    "c-slot":        "#F1EADF",
    "c-line":        "#E5DACA",
    "c-ink":         "#221C16",
    "c-muted":       "#6E6156",
    "c-accent":      "#B8452E",
    "c-accent-soft": "#F7E5DF",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W, H = 1080, 1350
EDGE = 72

LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.35, 1.7

# 四个图位的固定高度（见 step_card 注释）。
SLOT_H = 235

# 三个做饭的人自己能验证的量。刻意不给卡路里——见负约束。
META = [("时间", "10 分钟"), ("份量", "2 人"), ("难度", "新手可做")]

# (序号, 步骤名, 一行动作说明, 图位提示)
# 标题与说明都按「一行封顶」写：标题可用宽 322px / 27px ≈ 11 字，说明可用
# 宽 376px / 22px ≈ 17 字。超一个字就换行，四张卡的高度立刻不齐 —— 第一版
# 就是这么把页脚顶出画布的（说明写了 17 字，两行）。
STEPS = [
    ("1", "葱切段，白绿分开",
     "葱白耐炸先下，葱绿后放不发苦。", "葱段特写"),
    ("2", "冷油下葱白熬 6 分钟",
     "油面冒小泡是对的，冒烟就过了。", "熬油过程"),
    ("3", "两勺生抽一勺糖调汁",
     "糖不为甜，是压住生抽的咸边。", "调汁碗"),
    ("4", "面煮 3 分钟，捞进碗拌",
     "带一勺面汤，酱汁才挂得住面。", "成品面"),
]


def col(name, children, *, gap=16, width="fill_container", align="start",
        height="fit_content", **props):
    node = frame(ids, name, width=width, height=height,
                 layout="vertical", gap=gap, alignItems=align, fill=[],
                 **props)
    node["children"] = children
    return node


def row(name, children, *, gap=20, align="center", width="fill_container",
        height="fit_content", **props):
    node = frame(ids, name, width=width, height=height,
                 layout="horizontal", gap=gap, alignItems=align, fill=[],
                 **props)
    node["children"] = children
    return node


# ------------------------------------------------------------------ 页头
def header():
    tag = frame(ids, "分类标签", width="fit_content", height="fit_content",
                layout="horizontal", padding=[10, 20], cornerRadius=6,
                alignItems="center", justifyContent="center",
                fill=solid("$c-accent-soft"))
    tag["children"] = [
        text(ids, "分类文字", "家常面 · 四步", 24, 600, "$c-accent",
             family=CJK, width="fit_content", growth="auto", line_height=1.4),
    ]
    return col("页头", [
        tag,
        text(ids, "菜名", "十分钟葱油拌面", 72, 700, "$c-ink",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.4),
    ], gap=20)


# ------------------------------------------------------------------ 指标条
def meta_bar():
    kids = []
    for index, (label, value) in enumerate(META):
        if index:
            kids.append(rect(ids, "竖分隔", width=2, height=40,
                             fill=solid("$c-line")))
        kids.append(col("指标", [
            text(ids, "指标名", label, 22, 400, "$c-muted", family=CJK,
                 width="fit_content", growth="auto", line_height=1.4),
            text(ids, "指标值", value, 28, 600, "$c-ink", family=CJK,
                 width="fit_content", growth="auto", line_height=1.35),
        ], gap=4, width="fit_content"))
    bar = row("指标条", kids, gap=28, padding=[24, 28])
    bar["fill"] = solid("$c-card")
    bar["cornerRadius"] = 14
    bar["stroke"] = stroke("$c-line", 2)
    return bar


# ------------------------------------------------------------------ 四宫格
def step_card(no, title, desc, hint):
    # 图位高度写死而不是 fill_container：四张卡的标题/说明各自换行数不同，
    # 交给 flex 分配余量会让四个图位高度互不相等（实测 186 / 149），拼成
    # 四宫格时那点差值一眼可见。写死 200 后余量落到卡片底部，看不出来。
    slot = frame(ids, "图位", width="fill_container", height=SLOT_H,
                 layout="vertical", gap=10, alignItems="center",
                 justifyContent="center", cornerRadius=10,
                 fill=solid("$c-slot"))
    slot["children"] = [
        icon_font(ids, "图位图标", "camera", 34, "$c-muted"),
        text(ids, "图位提示", hint, 22, 500, "$c-muted", family=CJK,
             align="center", line_height=1.4),
    ]

    badge = frame(ids, "序号圆", width=40, height=40, layout="horizontal",
                  alignItems="center", justifyContent="center",
                  cornerRadius=999, fill=solid("$c-accent"))
    badge["children"] = [
        text(ids, "序号", no, 22, 700, "$c-card", family=NUM,
             width="fit_content", growth="auto", line_height=1.0),
    ]

    card = col("步骤卡", [
        slot,
        row("步骤标题行", [
            badge,
            text(ids, "步骤名", title, 27, 600, "$c-ink", family=CJK,
                 line_height=LH_HEAD),
        ], gap=14, align="center"),
        text(ids, "步骤说明", desc, 22, 400, "$c-muted", family=CJK,
             line_height=LH_BODY),
    ], gap=16, height="fill_container", padding=[20, 20])
    card["fill"] = solid("$c-card")
    card["cornerRadius"] = 16
    card["stroke"] = stroke("$c-line", 2)
    return card


def grid():
    cards = [step_card(*s) for s in STEPS]
    top = row("上排", cards[:2], gap=24, align="stretch",
              height="fill_container")
    bottom = row("下排", cards[2:], gap=24, align="stretch",
                 height="fill_container")
    node = col("四宫格", [top, bottom], gap=24, height="fill_container")
    return node


# ------------------------------------------------------------------ 页脚
def footer():
    left = row("提示左", [
        icon_font(ids, "提示图标", "lightbulb", 26, "$c-accent"),
        text(ids, "提示文字", "灰块是图片位，把过程图直接拖进去",
             23, 500, "$c-ink", family=CJK, width="fit_content",
             growth="auto", line_height=1.5),
    ], gap=12, align="center", width="fit_content")
    # 账号名并进提示条而不是单独一行：单独一行要多吃 50px，那 50px 直接从
    # 四个图位里扣。
    tip = row("页脚", [
        left,
        text(ids, "账号名", "@ 你的账号名", 22, 500, "$c-muted", family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
    ], gap=16, align="center", justifyContent="space_between",
        padding=[16, 20])
    tip["fill"] = solid("$c-accent-soft")
    tip["cornerRadius"] = 12
    return tip


def build():
    page = frame(ids, "菜谱四步卡", width=W, height=H, layout="vertical",
                 padding=[64, EDGE], gap=28, fill=solid("$c-bg"),
                 clipContent=True)
    page["children"] = [header(), meta_bar(), grid(), footer()]
    page["x"], page["y"] = 0, 0
    return [page]


# 对比度（WCAG 相对亮度比，op-design-lint 门槛 2.0；数值由脚本实测）：
#   c-ink    on c-bg      15.79   c-muted on c-bg      5.61
#   c-ink    on c-card    16.86   c-muted on c-card    5.99
#   c-ink    on c-slot    14.11   c-muted on c-slot    5.01
#   c-accent on c-bg       5.01   c-card  on c-accent  5.35
#   c-ink    on c-accent-soft 13.83   c-accent on c-accent-soft 4.39
# 承载正文的最低一对是 5.01（图位提示压在 c-slot 上），高于 AA 正文门槛
# 4.5。4.39 那一对只出现在两处：24px/600 的分类标签与 26px 的提示图标，
# 都落在 WCAG「大字 / 非文字图形」的 3.0 门槛下，且远高于 lint 的 2.0。
# c-line 只用于 hairline 描边，是非文字图形。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "菜谱四步卡 · 4:5 四宫格")
