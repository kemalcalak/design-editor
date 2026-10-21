#!/usr/bin/env python3
"""spine-culture-card.op — 竖排书脊卡 · 鸣沙矿彩（3:4 单张，1080×1440）

卡片体系的活样张之一：**主题 T1 鸣沙矿彩 × 配方 A5 竖排书脊**
（card-system-0808.md §3 / §4.1）。主题给色彩、字体、材质、装饰语言；配方
给信息层级与网格占位。两层正交，这张卡是它们的第一个交点。

竖排是中文独有的排版能力 —— 西文没有对应物，所以这一档在任何抄西文模板的
产品里都不会出现。A5 的层级是「竖排标题 → 留白 → 横排副标」，网格是「标题
竖排贴右 2 列，左 9 列留白或主视觉」。

### 主题 T1 的色彩推导（引自 spec §3，未改动）

  - **采样**：西北石窟壁画的矿物颜料本身 —— 石青、石绿、朱砂、蛤粉白，以
    及壁画剥落后露出的泥壁底。锚的是颜料的色度与剥落的层叠。
  - **收敛**：底取泥壁赭（暗、chroma 0.028）；有彩三支 H32 / H245 / H165，
    两两 H 差 ≥80°；中性 = 蛤粉白明度序列 0.62 / 0.78 / 0.94。
  - **论证**：底色不用「敦煌黄」这个印象色，用剥落壁面的赭泥 —— 印象色会
    把整套推向廉价文旅海报；泥壁色让矿彩浮上来，符合壁画「重彩浮于素壁」
    的物理关系。

### 强调色预算（spec R2：记出现次数，不是记支数）

  - 朱砂 2 处：竖排标题首字、底部短线 —— 到上限。
  - 石青 1 处：角标胶囊底。
  - 石绿 0 处：**一页最多两支矿彩，这张不出石绿。**
  - 金箔不计入预算：spec 把它限死在细描边与角标，不做文字色。

### 负约束（T1 主题约束 + 本模板追加）

  - 不画飞天、藻井、莲花、驼铃、沙丘 —— 一旦出现具象符号，主题就从「矿物
    颜料体系」塌成「文旅海报」。
  - 三支矿彩不可同页出齐；每支最多 2 处（见上）。
  - 金色不做文字色，只走描边与角标。
  - 剥落形状不可重复使用同一个 path —— 重复即图案，图案即廉价。
  - 与「宣纸水墨」的区分线：**没有墨、没有晕、没有飞白、没有毛笔笔触**。
    质感来自不透明矿物颜料的覆盖与剥落，是「厚涂」不是「渗透」。
  - 不用渐变做底（壁龛光是唯一的 radial，且渐到全透明）。

### 字体替身

spec 给 T1 指定的是汇文明朝体（display）。本机没有该字族，且实测
`Noto Serif SC` 会退化成无衬线 —— **衬线意图完全丢失**。所以走 `Songti SC`
（真宋体），理由与映射表写在 `cardlib.SERIF` 上。

### 硬契约（spec §4.0）

  - 画布 1080×1440，安全区 左右 80 / 上 96 / 下 128（下大于上是刻意的）
  - 12 列 × 62 + 11 沟槽 × 16 = 920
  - 字阶只用 3 档：display-l（竖排标题）/ body-l（副标）/ caption（角标与出处）
  - 任何小于 32px 的文字是错误，不是「小字」
  - 顶层 frame 必须显式写 x/y，否则多帧会全部堆在原点
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from cardlib import NUM, SANS, SERIF, VERTICAL, step
from oplib import (Ids, color_vars, frame, path, radial, rect, solid, stack,
                   text, write_doc)

ids = Ids()

# spec §3 · T1 的色板，oklch 实算值原样搬入（未改动，改了就不是这套主题）。
VARS = color_vars({
    "c-wall":       "#21120B",
    "c-plaster":    "#312018",
    "c-raised":     "#412E25",
    "c-shell":      "#EFEBE2",
    "c-shell-dim":  "#BBB7AD",
    "c-shell-faint": "#8A867D",
    "c-cinnabar":   "#C54F3B",
    "c-azurite-deep": "#105482",
    "c-gold":       "#D5AA55",
    # 壁龛光的芯与沿。唯一带 alpha 的一组：光要渐到全透明才不会在壁面上留
    # 一圈硬边。装饰是正文的兄弟不是祖先，lint 找底色只走祖先链，碰不到。
    "c-niche":      "#8A867D1F",
    "c-niche-out":  "#8A867D00",
})

C = VERTICAL
SPINE_TITLE = "山与河的来路"


def block(name, children, *, gap=24, width="fill_container", align="start",
          **props):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="vertical", gap=gap, alignItems=align, fill=[],
                 **props)
    node["children"] = children
    return node


def caption(name, content, color, *, family=SANS, width="fill_container",
            growth="fixed-width"):
    size, line_height, spacing = step("caption")
    return text(ids, name, content, size, 400, color, family=family,
                line_height=line_height, width=width, growth=growth,
                spacing=spacing)


# ------------------------------------------------------------- 竖排标题
def spine():
    """竖排标题。一个字一个文本节点，纵向排成一列。

    `.op` 没有 writing-mode，竖排只能靠「每字一节点 + vertical 容器」搭出
    来。这不是权宜之计：竖排本来就是逐字定位的，字间距（这里 gap 10）也确
    实要独立于横排行距来调。

    首字走朱砂 —— 全卡两处朱砂里的第一处，也是视线的落点。
    """
    size, line_height, _ = step("display-l")
    column = frame(ids, "竖排标题", width=C.cols(2), height="fit_content",
                   layout="vertical", gap=10, alignItems="center", fill=[])
    column["children"] = [
        text(ids, f"标题字 {index + 1}", char, size, 700,
             "$c-cinnabar" if index == 0 else "$c-shell", family=SERIF,
             line_height=line_height, width="fit_content", growth="auto",
             align="center")
        for index, char in enumerate(SPINE_TITLE)
    ]
    return column


# ------------------------------------------------------------- 左栏
def corner_tag(label):
    """角标。石青底 + 金线框 —— 金只在这里和页码上出现，不做文字色。"""
    node = frame(ids, "角标", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[10, 22], gap=0,
                 alignItems="center", justifyContent="center",
                 fill=solid("$c-azurite-deep"))
    node["stroke"] = {"thickness": 1, "fill": solid("$c-gold")}
    node["children"] = [
        caption("角标文字", label, "$c-shell", width="fit_content",
                growth="auto"),
    ]
    return node


def left_column():
    """左 9 列：角标压顶、大留白、副标与出处沉底。

    A5 的留白不是「没排满」，是版式的一部分 —— 竖排标题需要一整片安静的
    左侧才立得住，所以这一栏用 space_between 把两端推开。
    """
    size, line_height, spacing = step("body-l")
    subtitle = text(ids, "副标", "把一条河讲完，需要先讲清楚\n它流过的那些石头。",
                    size, 400, "$c-shell-dim", family=SANS,
                    line_height=line_height, spacing=spacing)

    foot = block("落款", [
        rect(ids, "朱砂短线", width=96, height=6, fill=solid("$c-cinnabar")),
        subtitle,
        caption("出处", "壁上 · 第三期 · 河西走廊", "$c-shell-faint"),
    ], gap=20)

    node = frame(ids, "左栏", width=C.cols(9), height="fill_container",
                 layout="vertical", gap=0, justifyContent="space_between",
                 alignItems="start", fill=[])
    node["children"] = [corner_tag("壁上 · 03"), foot]
    return node


# ------------------------------------------------------------- 装饰层
# 剥落：每一块都是独立写出来的多边形，**没有一块复用另一块的 path**。
# 边缘刻意不落在 8px 基线上 —— 剥落是随机的，对齐了就假。
#
# 位置全部避开文字：竖排标题占 x 788-928 / y 96-920，角标占左上，副标与出
# 处占 y 1060-1300 的左 700px。朱砂压在剥落上只有 2.77:1、注释压上去 3.52，
# 都掉到门槛以下 —— 装饰退到文字之外，比给文字加底板便宜得多。
FLAKES = [
    (0, 300, 300, 214, "M0 26 L118 0 L243 34 L299 141 L162 214 L37 176 Z"),
    (430, 186, 262, 188, "M14 0 L262 41 L237 149 L96 188 L0 96 Z"),
    (352, 700, 196, 152, "M31 0 L196 22 L167 118 L52 152 L0 63 Z"),
    (860, 1150, 220, 176, "M0 58 L96 0 L220 47 L196 176 L46 163 Z"),
]

# 矿彩颗粒：4×4 的小方块，只用朱砂一支，撒在竖排标题左右 40px 内。
# 位置写死而不是随机 —— 生成器每次跑出来的文件必须逐字节一致。
GRAINS = [
    (752, 486), (774, 604), (758, 712), (780, 838), (766, 946),
    (936, 520), (958, 662), (942, 794), (964, 902), (948, 1010),
    (770, 1052), (952, 1096),
]


def decor():
    items = []
    # 壁龛光：cx 0.5 / cy 0.28，整套设计只用一次（spec 限定）。
    glow = rect(ids, "壁龛光", width=1240, height=1240, cornerRadius=620,
                fill=radial([(0.0, "$c-niche"), (1.0, "$c-niche-out")],
                            cx=0.5, cy=0.28))
    glow["x"], glow["y"] = -80, -300
    items.append(glow)

    for index, (x, y, w, h, d) in enumerate(FLAKES, 1):
        node = path(ids, f"剥落 {index}", d, width=w, height=h,
                    fill=solid("$c-raised"))
        node["x"], node["y"] = x, y
        items.append(node)

    for index, (x, y) in enumerate(GRAINS, 1):
        node = rect(ids, f"矿彩颗粒 {index}", width=4, height=4,
                    fill=solid("$c-cinnabar"), opacity=0.2)
        node["x"], node["y"] = x, y
        items.append(node)
    return items


def card():
    body = frame(ids, "书脊卡 · 正文", width="fill_container",
                 height="fill_container", layout="horizontal",
                 padding=C.padding, gap=16, alignItems="stretch", fill=[])
    body["children"] = [left_column(), spine()]

    shell = stack(ids, "竖排书脊卡 · 鸣沙矿彩", body, decor(),
                  width=C.width, height=C.height, fill=solid("$c-wall"))
    shell["x"], shell["y"] = 0, 0
    return shell


# 对比度（WCAG 相对亮度比，op-design-lint 的门槛是 2.0；数值实测）：
#   c-shell       on c-wall     15.27    c-shell-dim on c-wall      9.07
#   c-shell-faint on c-wall      5.01    c-cinnabar  on c-wall      3.94
#   c-shell       on c-azurite-deep 6.76 c-shell     on c-plaster  12.04
# 朱砂对壁底只有 3.94，低于 AA 的 4.5 —— spec 因此把它限死在 **≥48px**。
# 本卡里朱砂只出现在 120px 的首字和一条 6px 的实心短线上（短线是图形不是
# 文字），两处都在限制之内。副标与出处走蛤粉白序列，9.07 / 5.01 都有余量。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, [card()], "竖排书脊卡 · 鸣沙矿彩 3:4")
