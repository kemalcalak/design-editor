#!/usr/bin/env python3
"""price-tier-card.op — 促销价格卡 · 霓虹骑楼（1:1 单张，1080×1080）

卡片体系的活样张之一：**主题 T4 霓虹骑楼 × 配方 E3 表格两列**
（card-system-0808.md §3 / §4.1）。

价格卡要回答的是「几档、各多少钱」，E3 表格两列就是为这种参数/功能/价格
对照写的：表头 → 行 → 斑马底，3-5 行 × 2 列。主题选霓虹骑楼不是为了「炫」
—— 夜市与街边店铺的价目本来就写在霓虹招牌上，这是内容自带的语境。

### 主题 T4 的色彩推导（引自 spec §3，未改动）

  - **采样**：被光污染染色的夜空、霓虹管三色（洋红 / 青 / 琥珀）、亚克力招
    牌面板的浊白。
  - **收敛**：底 = 墨蓝紫 L0.19 C0.038（守大面积底色的 chroma 纪律）；有彩
    三支 H5 / H195 / H75，两两 H 差 ≥80°；点睛允许 chroma 0.14-0.21，**但
    只作 2px 描边不作填充**。
  - **论证**：底不用纯黑用 L0.19 的墨蓝紫 —— 纯黑上的霓虹是贴纸，被光染过
    的夜空上的霓虹才有「空气在散射」的层次。这一条决定整套是「高级夜景」
    还是「低质赛博朋克」。

### 负约束（T4 主题约束，逐条照搬）

  - **霓虹色不做填充、不做文字底、不做渐变铺底** —— 只作 2px 描边 + 外散射。
  - 一页最多两支灯管色：这张只出洋红与青，琥珀与玉色不出现。
  - **不用紫蓝渐变**（廉价 AI 科技风的头号指纹）。底是实色，不是渐变。
  - 不画雨滴、不画赛博朋克机械体、不用 glitch 效果。
  - 与「靛青瓷」的区分线：本主题的蓝是**底**不是主角（chroma 仅 0.038，退
    到背景），主角是灯管色。
  - 追加：不做「原价划掉」的促销套路 —— 删除线在 32px 以下读不清，且它把
    卡片从「价目」拉成「打折广告」。档位差本身就说明了价格差。

### 硬契约（spec §4.0 / §5）

  - 画布 1080×1080，安全区 左右 80 / 上 88 / 下 112
  - 字阶用 4 档：display（招牌字）/ title-2（档位名与价格）/ body-l（说明）
    / caption（表头与备注）
  - 数字走西文族；汉字走黑体
  - 顶层 frame 必须显式写 x/y
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from cardlib import NUM, SANS, SQUARE, step
from oplib import (Ids, color_vars, frame, linear, radial, rect, solid, stack,
                   text, write_doc)

ids = Ids()

# spec §3 · T4 的色板，oklch 实算值原样搬入。
VARS = color_vars({
    "c-night":         "#0F1225",
    "c-night-deep":    "#060816",
    "c-panel":         "#1B2036",
    "c-panel-raised":  "#282F47",
    "c-acrylic":       "#E3E8EE",
    "c-acrylic-dim":   "#ABB2BA",
    "c-acrylic-faint": "#757E88",
    "c-magenta":       "#FA618E",
    "c-cyan":          "#44D6D5",
    # 外散射与雨夜反光。带 alpha 的三个变量，全部只出现在装饰层：
    # 装饰是正文的兄弟不是祖先，lint 找底色只走祖先链，碰不到。
    "c-glow-magenta":  "#FA618E1F",
    "c-glow-cyan":     "#44D6D51F",
    "c-glow-out":      "#FA618E00",
})

C = SQUARE

# (档位, 价格, 说明)。三行 —— E3 的区间是 3-5 行。
TIERS = [
    ("单次体验", "¥ 38", "含一杯手冲与一份点心"),
    ("五次联票", "¥ 160", "折合每次 32，三个月内有效"),
    ("月度畅饮", "¥ 288", "每日一杯，不限品类"),
]


def col(name, children, *, gap=20, width="fill_container", align="start",
        **props):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="vertical", gap=gap, alignItems=align, fill=[],
                 **props)
    node["children"] = children
    return node


def row(name, children, *, gap=20, align="center", width="fill_container",
        **props):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="horizontal", gap=gap, alignItems=align, fill=[],
                 **props)
    node["children"] = children
    return node


def typed(name, content, scale_name, weight, color, *, family=SANS,
          width="fill_container", growth="fixed-width", align=None):
    size, line_height, spacing = step(scale_name)
    return text(ids, name, content, size, weight, color, family=family,
                line_height=line_height, width=width, growth=growth,
                align=align, spacing=spacing)


def tube(width, height, x, y, color, glow):
    """一支灯管：2px 描边的圆角矩形，**内部不填充**，下面压一层外散射。

    返回 (散射, 管) 两个节点，调用方按这个顺序放进装饰层 —— 散射必须在管
    的下层，否则光会盖住管壁。
    """
    halo = rect(ids, "外散射", width=round(width * 2.2),
                height=round(height * 2.5), cornerRadius=height,
                fill=radial([(0.0, glow), (1.0, "$c-glow-out")]))
    halo["x"] = round(x - width * 0.6)
    halo["y"] = round(y - height * 0.75)
    node = rect(ids, "灯管", width=width, height=height,
                cornerRadius=height // 2, fill=[],
                stroke={"thickness": 2, "fill": solid(color)})
    node["x"], node["y"] = x, y
    return halo, node


def header():
    return col("卡头", [
        typed("店招", "夜灯咖啡 · 价目", "caption", 500, "$c-acrylic-faint"),
        typed("主张", "开到最后一班地铁", "display", 700, "$c-acrylic"),
    ], gap=18)


def table():
    """E3 表格两列：表头 + 三行，斑马底靠 panel 与 panel-raised 交替。

    价格列固定宽而不是 fit_content —— 三行价格必须右端对齐成一条线，
    这是「价目表」和「三句话」的分界。
    """
    head = row("表头", [
        typed("表头 · 档位", "档位", "caption", 600, "$c-acrylic-faint"),
        typed("表头 · 价格", "价格", "caption", 600, "$c-acrylic-faint",
              width=200, align="right"),
    ], gap=20, padding=[0, 28, 12, 28])

    rows = [head]
    for index, (name, price, note) in enumerate(TIERS):
        cell = col("档位单元", [
            typed("档位名", name, "title-2", 600, "$c-acrylic"),
            typed("档位说明", note, "caption", 400, "$c-acrylic-dim"),
        ], gap=8)
        line = row(f"价目行 {index + 1}", [
            cell,
            typed("价格", price, "title-2", 700, "$c-acrylic", family=NUM,
                  width=200, align="right"),
        ], gap=20, align="center", padding=[22, 28])
        # 斑马底：奇数行抬一档，不加线 —— 表格线在深底上会变成一排亮条。
        line["fill"] = solid("$c-panel-raised" if index % 2 else "$c-panel")
        rows.append(line)

    return col("价目表", rows, gap=0)


def footer():
    return typed("备注", "价格含税，不与其他优惠同享。", "caption", 400,
                 "$c-acrylic-faint")


def decor():
    items = []
    # 竖排招牌的灯管：贴右缘的一支长管（洋红）。竖排招牌是 T4 的签名，
    # spec 限定只在封面页用一次 —— 单张卡就是它的封面。
    # 贴到右缘之外一点，让它半出血 —— 第一版放在 x=940，正好压在价目表的
    # 价格列后面，管子被表格切成两截，读起来像画错了。招牌本来就该在墙外。
    halo_a, tube_a = tube(96, 640, 1004, 120, "$c-magenta", "$c-glow-magenta")
    # 主张下方的一支横管（青），压在标题与表格之间当分隔。
    halo_b, tube_b = tube(360, 8, 80, 300, "$c-cyan", "$c-glow-cyan")
    items.extend([halo_a, tube_a, halo_b, tube_b])

    # 雨夜反光：底部一条渐到透明的深色带。0° = 自下而上，所以最深处在底边。
    wash = rect(ids, "雨夜反光", width=C.width, height=200,
                fill=linear(0, [(0.0, "$c-night-deep"), (1.0, "$c-glow-out")]))
    wash["x"], wash["y"] = 0, C.height - 200
    items.append(wash)
    return items


def card():
    body = frame(ids, "价目卡 · 正文", width="fill_container",
                 height="fill_container", layout="vertical",
                 padding=C.padding, gap=0, justifyContent="space_between",
                 alignItems="start", fill=[])
    body["children"] = [header(), table(), footer()]

    shell = stack(ids, "促销价格卡 · 霓虹骑楼", body, decor(),
                  width=C.width, height=C.height, fill=solid("$c-night"))
    shell["x"], shell["y"] = 0, 0
    return shell


# 对比度（WCAG 相对亮度比，op-design-lint 的门槛是 2.0；数值实测）：
#   c-acrylic       on c-night  15.04    c-acrylic-dim   on c-night   8.65
#   c-acrylic-faint on c-night   4.50    c-acrylic       on c-panel  12.98
#   c-acrylic-dim   on c-panel   7.47    c-acrylic-faint on c-panel   3.89
#   c-magenta       on c-night   6.32    c-cyan          on c-night  10.43
# 承载文字的最低一对是 c-acrylic-faint 压在斑马行的 panel 上，3.89 ——
# 它只用在表头与备注（32px）。**这是本卡唯一低于 AA 的文字对**：把它提到
# c-acrylic-dim 会让表头和档位名争权重，而表头本来就该退。两支灯管色
# 6.32 / 10.43 都只作 2px 描边，从不承载文字。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, [card()], "促销价格卡 · 霓虹骑楼 1:1")
