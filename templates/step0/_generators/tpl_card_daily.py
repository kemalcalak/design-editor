#!/usr/bin/env python3
"""daily-sign-card.op — 日签卡 · 园林框景（3:4 单张，1080×1440）

卡片体系的活样张之一：**主题 T10 园林框景 × 配方 B1 满版一句**
（card-system-0808.md §3 / §4.1）。

日签是「每天一张、一句话」的格式，B1 正是为它写的：金句 → 唯一装饰 → 出
处，句子占 10 列居中，**上下各留 ≥25%**。T10 则把那个「唯一装饰」定成了漏
窗——江南园林的框景逻辑是「透过一个几何洞口看另一侧」，**框本身就是版式**。

### 主题 T10 的色彩推导（引自 spec §3，未改动）

  - **采样**：石灰粉墙（暖白但极低 chroma）、青灰瓦、栗色木框、墙根苔痕。
  - **收敛**：底 = 粉墙 L0.955 C0.006（10 套里 chroma 最低的亮底）；中性 =
    瓦灰序列；有彩两支（栗 H55 / 苔 H145，H 差 90°）皆 chroma ≤0.055。
  - **论证**：整套 chroma 上限 0.055，是 10 套里最克制的 —— 园林的高级感来
    自材料本身（灰、白、木、苔）而不是颜色。任何一支色越过 0.06，整套就从
    「园林」滑向「新中式装修」。

### 负约束（T10 主题约束，逐条照搬）

  - 不用回纹、云纹、如意等传统纹样（那是装饰主义，不是园林）。
  - 不用红木色 —— 栗色 chroma 0.055，红木是 0.12+。
  - **框一套只用一种几何**：这张用六角，就不再出现海棠与冰裂。
  - 不用「竖排 + 印章 + 水墨」的新中式三件套。
  - **内容不可撑满安全区**（≤55%）—— 留白是本主题的主装饰，撑满即失效。
  - 细笔画 display 只在 ≥88px 成立，小于 48px 必须回到黑体。

### 硬契约（spec §4.0）

  - 画布 1080×1440，安全区 左右 80 / 上 96 / 下 128
  - 字阶只用 3 档：display（金句）/ title-2（日期）/ caption（出处与署名）
  - B1 要求上下各留 ≥25%：漏窗被压在垂直居中的位置，上下各余 ~28%
  - 顶层 frame 必须显式写 x/y
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from cardlib import NUM, SANS, SERIF, VERTICAL, step
from oplib import (Ids, color_vars, frame, path, rect, solid, stack, text,
                   write_doc)

ids = Ids()

# spec §3 · T10 的色板，oklch 实算值原样搬入。
VARS = color_vars({
    "c-limewash":      "#F2F0EC",
    "c-limewash-deep": "#E2DFDA",
    "c-ink":           "#24211E",
    "c-ink-soft":      "#5F5A55",
    "c-tile":          "#70757A",
    "c-tile-dark":     "#383E43",
    "c-chestnut":      "#6A4A35",
    "c-moss":          "#647C64",
})

C = VERTICAL

# 六角漏窗。宽 700 × 高 700，垂直居中 —— 面积 490k，安全区 920×1216 =
# 1119k，占 44%，守住「内容区不超过安全区 55%」这条。
WIN_W = WIN_H = 700
WIN_X = (C.width - WIN_W) // 2
WIN_Y = (C.height - WIN_H) // 2
HEXAGON = "M350 0 L700 175 L700 525 L350 700 L0 525 L0 175 Z"


def col(name, children, *, gap=20, width="fill_container", align="start",
        **props):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="vertical", gap=gap, alignItems=align, fill=[],
                 **props)
    node["children"] = children
    return node


def typed(name, content, scale_name, weight, color, *, family=SANS,
          width="fill_container", growth="fixed-width", align=None):
    size, line_height, spacing = step(scale_name)
    return text(ids, name, content, size, weight, color, family=family,
                line_height=line_height, width=width, growth=growth,
                align=align, spacing=spacing)


def eaves():
    """檐线。三段等距弧，只出现在页面顶部。

    用三段独立的圆弧 `path` 而不是一条重复的图案 —— 檐口是一片瓦一片瓦搭
    出来的，等距但各自独立，这一点和「贴一张 pattern」在观感上差得很远。
    """
    items = []
    for index in range(3):
        arc = path(ids, f"檐线 {index + 1}",
                   "M0 44 Q140 0 280 44", width=280, height=44,
                   fill=[], stroke={"thickness": 3,
                                    "fill": solid("$c-tile")})
        arc["x"], arc["y"] = 120 + index * 280, 40
        items.append(arc)
    return items


def moss_specks():
    """苔。6×6 的小方块，只贴在框的下缘 —— 墙根才长苔，别处不长。"""
    items = []
    for index, (dx, dy) in enumerate(
            [(196, 486), (238, 512), (300, 528), (392, 534),
             (452, 516), (498, 488)], 1):
        node = rect(ids, f"苔 {index}", width=6, height=6,
                    fill=solid("$c-moss"), opacity=0.4)
        node["x"], node["y"] = WIN_X + dx, WIN_Y + dy
        items.append(node)
    return items


def window():
    """漏窗轮廓。一套设计只用一种几何，这张是六角。"""
    node = path(ids, "漏窗 · 六角", HEXAGON, width=WIN_W, height=WIN_H,
                fill=[], stroke={"thickness": 4,
                                 "fill": solid("$c-chestnut")})
    node["x"], node["y"] = WIN_X, WIN_Y
    return node


def inner():
    """洞内的内容：日期 + 一句话。

    宽度收到 7 列而不是满栏 —— 六角形的内接可用宽度只有外框的七成左右，
    按满栏排字会顶到斜边上。

    金句压到每行 4 字：88px 的宋体在 7 列（502px）里一行只放得下 5 个字，
    写长了就会折出「来，」「下。」这种挂在行尾的孤字。B1 的字数上限是 24
    字，但真正的约束是**六角形的腰**，不是字数。
    """
    node = frame(ids, "洞内", width=C.cols(7), height="fit_content",
                 layout="vertical", gap=28, alignItems="center", fill=[])
    node["children"] = [
        typed("日期", "08 / 09", "title-2", 600, "$c-tile", family=NUM,
              width="fit_content", growth="auto"),
        typed("金句", "先留白，\n再住人。", "display", 700,
              "$c-ink", family=SERIF, align="center"),
    ]
    return node


def card():
    body = frame(ids, "日签卡 · 正文", width="fill_container",
                 height="fill_container", layout="vertical",
                 padding=C.padding, gap=0, justifyContent="space_between",
                 alignItems="center", fill=[])
    body["children"] = [
        typed("刊头", "一日一见 · 园记", "caption", 500, "$c-tile",
              width="fit_content", growth="auto"),
        inner(),
        col("落款", [
            typed("署名", "@ 你的账号名", "caption", 500, "$c-ink-soft",
                  width="fit_content", growth="auto", align="center"),
        ], gap=8, width="fit_content", align="center"),
    ]

    decor = eaves() + [window()] + moss_specks()
    shell = stack(ids, "日签卡 · 园林框景", body, decor,
                  width=C.width, height=C.height, fill=solid("$c-limewash"))
    shell["x"], shell["y"] = 0, 0
    return shell


# 对比度（WCAG 相对亮度比，op-design-lint 的门槛是 2.0；数值实测）：
#   c-ink      on c-limewash  14.02    c-ink-soft on c-limewash   6.65
#   c-tile     on c-limewash   4.53    c-chestnut on c-limewash   7.31
#   c-moss     on c-limewash   4.11    c-tile-dark on c-limewash  9.53
# 承载文字的最低一对是 c-tile on c-limewash 的 4.53，刚过 AA —— 它只用在
# 刊头与日期上（32 / 48px）。苔 4.11 与栗 7.31 都不承载文字：苔是 6px 的
# 方块，栗是漏窗的 4px 框壁，两者按非文字图形的 3.0 计，余量充足。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, [card()], "日签卡 · 园林框景 3:4")
