#!/usr/bin/env python3
"""notice-board-card.op — 公告通知卡 · 铅字报刊（4:5 单张，1080×1350）

卡片体系的活样张之一：**主题 T3 铅字报刊 × 配方 C1 编号纵列**
（card-system-0808.md §3 / §4.1）。

公告的原生载体就是报纸：报头、双线、编号条款、落款日期。C1 的层级是「编号
→ 条目标题 → 注解」，网格是「编号 2 列 + 内容 10 列，3-5 条」——一份通知
本来就是这么排的，不需要为它发明版式。

### 主题 T3 的色彩推导（引自 spec §3，未改动）

  - **采样**：新闻纸（木浆、微黄偏灰）、铅字油墨（非纯黑，偏暖，在纸上洇
    散）、报头套红。
  - **收敛**：单一有彩色（套红 H32 C0.145）+ 一组暖灰中性序列。**这是 10
    套里唯一的单色主题** —— 报纸本来就是黑白加一色。
  - **论证**：纸色 chroma 0.016 是木浆纸的黄，比奶油纸白更灰更冷一档；墨不
    用 `#000` 而用 L0.22 带 0.010 暖 chroma，模拟铅字油墨在纸上的洇散。两个
    2% 级的决定，是「像报纸」和「像白底黑字」的全部差别。

### 负约束（T3 主题约束，逐条照搬）

  - 不做做旧污渍、不做纸张翻卷、不做半调网点（那是另一个身份）。
  - **套红不做渐变、不做大面积底。**
  - `rule`（栏线色）永远不承载文字。
  - **套印错位每套只用一次，且只用在封面** —— 这张卡就是它的封面，那一次
    给了报头双线（为什么不给标题，见 `misregistered_rule` 的说明）。
  - 第二套色 `indigo.stamp` 出现超过一次即失效：这张只在骑缝编号上用一次。
  - 与「宣纸水墨」的区分线：纸是**木浆新闻纸**，字是**铅字压印**（边缘硬、
    有套印错位），全程无笔触、无墨晕。

### 硬契约（spec §4.0）

  - 画布 1080×1350（4:5），安全区 左右 80 / 上 92 / 下 120
  - 字阶用 4 档：title-1（主标题）/ title-2（条目标题）/ body-l（注解）
    / caption（报头、日期、落款）
  - C1 共同约束：条目 3-7 条；每条注解 ≤1 行；编号/勾选框/节点三种引导符
    一页只用一种（这里只用编号）
  - 顶层 frame 必须显式写 x/y
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from cardlib import NUM, PORTRAIT45, SANS, SERIF, step
from oplib import (Ids, color_vars, frame, rect, solid, stack, text,
                   write_doc)

ids = Ids()

# spec §3 · T3 的色板，oklch 实算值原样搬入。
VARS = color_vars({
    "c-newsprint":       "#F1EDE1",
    "c-newsprint-aged":  "#E5DDCC",
    "c-newsprint-tint":  "#D8CEBB",
    "c-ink":             "#1E1A16",
    "c-ink-soft":        "#544F4A",
    "c-ink-caption":     "#6D6863",
    "c-rule":            "#B3B1AA",
    "c-vermilion":       "#B74A37",
    "c-vermilion-wash":  "#FAD5C8",
    "c-indigo":          "#324673",
})

C = PORTRAIT45
TITLE = "关于调整开放时间的通知"

# (编号, 条目, 注解)。C1 的区间是 3-5 条。
CLAUSES = [
    ("一", "工作日 10:00 – 21:00", "较原时间延后一小时闭馆"),
    ("二", "周末 09:00 – 22:00", "周六下午为团体预约时段"),
    ("三", "每月首个周一闭馆", "设备检修，当日不接待"),
]


def col(name, children, *, gap=20, width="fill_container", align="start",
        **props):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="vertical", gap=gap, alignItems=align, fill=[],
                 **props)
    node["children"] = children
    return node


def row(name, children, *, gap=20, align="start", width="fill_container",
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


def misregistered_rule(thickness):
    """一条带套印错位的报头线：黑版在上，套红版偏移 (3,3) 压在下层。

    **套印错位落在线上而不是标题上，是被检测器逼出来的一个更好的选择。**
    第一版按 spec 的字面做法把标题复制一层填套红、偏移 3px 压底 —— 几何检
    测器（text_collision）判它是「两个文本块压在同一批像素上」，96% 重叠。
    那条前提对真实的套印错位不成立（红版被黑版盖掉 97%，露出的只是 3px 的
    彩色毛边，从来不是要读的第二行字），但检测器看不到「谁盖住了谁」，只看
    到两个文本矩形。

    改画在实心线上之后，同一个工艺指纹用 rect 表达：报纸上的套印错位本来
    也是在实底和线条上最显眼，文字上的错位反而细到看不清。检测器能推理
    rect，我们也不必为一个装饰去动检测器。

    这个替代已同步给检测器工人，作为「遮挡关系」判据的一个真实用例。
    """
    node = frame(ids, "报头线 · 套印错位", width="fill_container",
                 height=thickness + 3, layout="none", fill=[])
    black = rect(ids, "黑版", width="fill_container", height=thickness,
                 fill=solid("$c-ink"))
    black["x"], black["y"] = 0, 0
    red = rect(ids, "套红版", width="fill_container", height=thickness,
               fill=solid("$c-vermilion"))
    red["x"], red["y"] = 3, 3
    # children[0] 画在最上：黑版压住套红版，只在右下露出 3px 的红毛边。
    node["children"] = [black, red]
    return node


def masthead():
    """报头：上粗（3px）下细（1px）双线，跨满栏。两线之间夹一行刊名与日期。

    两条线都带套印错位 —— 整套设计的「一次」用在这里（见
    `misregistered_rule` 的说明）。
    """
    meta = row("刊头行", [
        typed("刊名", "馆务公告", "caption", 600, "$c-ink",
              width="fit_content", growth="auto"),
        typed("日期", "2026-08-09 · 第 118 期", "caption", 400,
              "$c-ink-caption", width="fit_content", growth="auto"),
    ], gap=24, align="center", justifyContent="space_between")
    return col("报头", [
        misregistered_rule(3),
        meta,
        misregistered_rule(1),
    ], gap=14)


def title_block():
    """主标题。单层黑版 —— 套印错位已经在报头线上用掉了那唯一的一次。"""
    return typed("主标题", TITLE, "title-1", 700, "$c-ink", family=SERIF)


def clauses():
    """编号纵列。编号占 2 列、内容占 10 列 —— C1 的网格原文。"""
    items = []
    for index, (number, heading, note) in enumerate(CLAUSES):
        if index:
            items.append(rect(ids, "条目分隔线", width="fill_container",
                              height=1, fill=solid("$c-rule")))
        entry = row(f"条款 {number}", [
            typed("编号", number, "title-2", 700, "$c-vermilion",
                  family=SERIF, width=C.cols(2)),
            col("条款文案", [
                typed("条目", heading, "title-2", 600, "$c-ink"),
                typed("注解", note, "body-l", 400, "$c-ink-soft"),
            ], gap=10, width=C.cols(10)),
        ], gap=16, padding=[26, 0])
        items.append(entry)
    return col("条款列表", items, gap=0)


def sign_off():
    """落款。骑缝编号是整张卡唯一一次用到第二套色（靛蓝）。"""
    stamp = frame(ids, "骑缝编号", width="fit_content", height="fit_content",
                  layout="horizontal", padding=[8, 18], gap=0,
                  alignItems="center", justifyContent="center", fill=[])
    stamp["stroke"] = {"thickness": 2, "fill": solid("$c-indigo")}
    stamp["children"] = [
        typed("编号文字", "NO. 0118", "caption", 600, "$c-indigo",
              family=NUM, width="fit_content", growth="auto"),
    ]
    return row("落款", [
        typed("落款单位", "城南图书馆 · 馆务办公室", "caption", 500,
              "$c-ink-soft", width="fit_content", growth="auto"),
        stamp,
    ], gap=24, align="center", justifyContent="space_between")


def decor():
    """栏线：一条 1px 竖线，贴在编号列与内容列之间的沟槽上。

    它不承载文字（spec 明令），职责只是让「编号是一栏、内容是另一栏」这件
    事在视觉上成立。
    """
    line = rect(ids, "栏线", width=1, height=560, fill=solid("$c-rule"))
    line["x"] = C.pad_x + C.cols(2) + 8
    line["y"] = 470
    return [line]


def card():
    body = frame(ids, "公告卡 · 正文", width="fill_container",
                 height="fill_container", layout="vertical",
                 padding=C.padding, gap=0, justifyContent="space_between",
                 alignItems="start", fill=[])
    body["children"] = [
        col("卡头", [masthead(), title_block()], gap=36),
        clauses(),
        sign_off(),
    ]

    shell = stack(ids, "公告通知卡 · 铅字报刊", body, decor(),
                  width=C.width, height=C.height,
                  fill=solid("$c-newsprint"))
    shell["x"], shell["y"] = 0, 0
    return shell


# 对比度（WCAG 相对亮度比，op-design-lint 的门槛是 2.0；数值实测）：
#   c-ink       on c-newsprint  14.77    c-ink-soft   on c-newsprint  6.92
#   c-ink-caption on c-newsprint 4.71    c-vermilion  on c-newsprint  4.43
#   c-indigo    on c-newsprint   8.02    c-rule       on c-newsprint  1.83
# 承载文字的最低一对是套红 4.43 —— spec 因此限定它 **≥40px 或加粗**：这张
# 卡里套红只出现在 48px 700 的编号和 64px 700 的错位层上，两处都合规。
# 错位层压在黑版下面，本来也不承担可读性。c-rule 1.83 是 1px 线，非文字图
# 形，它的职责是「在那儿」不是「被看见」。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, [card()], "公告通知卡 · 铅字报刊 4:5")
