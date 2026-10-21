#!/usr/bin/env python3
"""quote-frame-card.op — 引用书摘卡 · 绢本青绿（4:5 单张，1080×1350）

卡片体系的活样张之一：**主题 T2 绢本青绿 × 配方 B2 引号框景**
（card-system-0808.md §3 / §4.1）。

B2 的层级是「金句 → 框 → 署名」：金句落在框内 8 列，署名在框外右下。B 族
的共同约束是一页只放一句、字数 ≤24 字、署名字号必须是 caption 档不许和金
句争 —— 这三条决定了这张卡不可能变成「一段话配个边框」。

### 主题 T2 的色彩推导（引自 spec §3，未改动）

  - **采样**：老化绢本的暖黄底、石青、石绿、山脚赭石、绢上墨（偏褐，不是
    纯黑）。锚的是「双色渐层 + 绢黄底」这个色彩结构。
  - **收敛**：底 = 绢黄 L0.93 C0.030；有彩两支（青 H240 / 绿 H158，H 差
    82°）；中性 = 褐墨序列。
  - **论证**：墨色取 L0.30 且带 0.020 暖 chroma —— 绢面吸墨少、反射暖，纯
    黑写在绢上会「浮」，褐墨才坐得住。这一步是本主题和任何「青绿配色」的
    分野。

### 负约束（T2 主题约束，逐条照搬）

  - 不画松、鹤、亭、舟、印章。
  - **青绿渐层不可用于文字色或大面积底** —— 它是「山」，山在下面。所以这
    张卡的渐层只出现在底部 22% 的山形里，一个字都不压在它上面。
  - 不用红色做印章式点缀（那是另一套语汇，会把主题拉去传统海报）。
  - 与「靛青瓷」的区分线：本主题的蓝是**蓝铜矿的青**（H240、chroma 仅
    0.10），不是靛蓝染料的深紫蓝；且永远与绿成对出现，单独用青即偏离主题。

### 字体替身

spec 给 T2 指定 display 源流明体 / body 霞鹜文楷，本机都没有。实测
`Noto Serif SC` 退化成无衬线，所以走 `Songti SC`（真宋体）承接「明体」意
图，body 走黑体保长文可读 —— 映射写在 `cardlib.SERIF` / `SANS`。

### 硬契约（spec §4.0）

  - 画布 1080×1350（4:5），安全区 左右 80 / 上 92 / 下 120
  - 字阶只用 3 档：display（金句）/ body-l（引言导语）/ caption（署名与出处）
  - 金句 ≤24 字；署名走 caption，不与金句争
  - 顶层 frame 必须显式写 x/y
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from cardlib import PORTRAIT45, SANS, SERIF, step
from oplib import (Ids, color_vars, frame, linear, path, rect, solid, stack,
                   text, write_doc)

ids = Ids()

# spec §3 · T2 的色板，oklch 实算值原样搬入。
VARS = color_vars({
    "c-silk":       "#F0E7D2",
    "c-silk-deep":  "#E3D6BC",
    "c-ink":        "#362C24",
    "c-ink-soft":   "#665B54",
    "c-azurite":    "#347CA9",
    "c-azurite-deep": "#114C71",
    "c-malachite":  "#5E9877",
    "c-ochre":      "#B59979",
})

C = PORTRAIT45

# 山形只占页面底部 22%（spec 限定 18-28%），且内容区的下边界停在它上方。
MOUNTAIN_H = 300


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


def quote_frame():
    """引号框。一圈 1px 赭石线 + 框内 8 列的金句。

    框不是装饰边，是 B2 的层级构件：它把金句从整页里**框出来**，署名才有
    「框外」可站。所以框只用最细的一档线，粗了就变成一个色块盒子。
    """
    size, line_height, spacing = step("display")
    node = frame(ids, "引号框", width="fill_container", height="fit_content",
                 layout="vertical", padding=[56, 48], gap=28,
                 alignItems="start", fill=[])
    node["stroke"] = {"thickness": 1, "fill": solid("$c-ochre")}
    node["children"] = [
        # 开引号单独成一个节点：它比正文大一档、颜色退到赭石，是「引用」这
        # 个动作的记号，不是句子的一部分。用直角引号「」而不是弯引号 ——
        # 弯引号在中文字体里是全角占位但形状是西文的，视觉上会漂浮。
        text(ids, "开引号", "「", size, 400, "$c-ochre", family=SERIF,
             line_height=1.0, width="fit_content", growth="auto"),
        text(ids, "金句", "读得慢一点，\n书才会开口。",
             size, 700, "$c-ink", family=SERIF, line_height=line_height,
             spacing=spacing),
    ]
    return node


def head():
    return col("卡头", [
        typed("导语", "书摘 · 第十二则", "caption", 500, "$c-ink-soft"),
        rect(ids, "赭石细线", width=120, height=1, fill=solid("$c-ochre")),
    ], gap=16)


def signature():
    """署名。B2 规定它在框外右下，且必须是 caption 档。"""
    node = col("署名", [
        typed("作者", "—— 《慢读》第 3 章", "caption", 600, "$c-ink",
              width="fit_content", growth="auto"),
        typed("账号", "@ 你的账号名", "caption", 400, "$c-ink-soft",
              width="fit_content", growth="auto"),
    ], gap=8, width="fit_content", align="end")
    wrapper = frame(ids, "署名行", width="fill_container",
                    height="fit_content", layout="horizontal", gap=0,
                    justifyContent="end", fill=[])
    wrapper["children"] = [node]
    return wrapper


def decor():
    """山形 + 绢纹。两样都压在正文层下面，一个字都不遮。"""
    items = []
    # 绢纹：等距横线，间距 16，赭石 10%。只铺到山形上沿为止。
    y = 120
    while y < C.height - MOUNTAIN_H:
        line = rect(ids, "绢纹", width=C.width, height=1,
                    fill=solid("$c-ochre"), opacity=0.1)
        line["x"], line["y"] = 0, y
        items.append(line)
        y += 16

    # 山形：由下而上从石绿叠染到石青 —— 这是 T2 的锚点结构本身。
    # 0° = 自下而上（`.op` 的角度约定），所以深绿在山脚、青在山顶。
    mountain = path(ids, "山形", "M0 300 L0 214 L196 62 L352 168 L560 0 "
                                 "L760 148 L920 66 L1080 186 L1080 300 Z",
                    width=C.width, height=MOUNTAIN_H,
                    fill=linear(0, [(0.0, "$c-malachite"),
                                    (1.0, "$c-azurite")]))
    mountain["x"], mountain["y"] = 0, C.height - MOUNTAIN_H
    items.append(mountain)
    return items


def card():
    body = frame(ids, "书摘卡 · 正文", width="fill_container",
                 height="fill_container", layout="vertical",
                 # 下 padding 让开山形：正文的底不许压到渐层上。
                 padding=[C.pad_top, C.pad_x, MOUNTAIN_H + 48, C.pad_x],
                 gap=0, justifyContent="space_between", alignItems="start",
                 fill=[])
    body["children"] = [head(), quote_frame(), signature()]

    shell = stack(ids, "引用书摘卡 · 绢本青绿", body, decor(),
                  width=C.width, height=C.height, fill=solid("$c-silk"))
    shell["x"], shell["y"] = 0, 0
    return shell


# 对比度（WCAG 相对亮度比，op-design-lint 的门槛是 2.0；数值实测）：
#   c-ink      on c-silk       9.94    c-ink-soft on c-silk      5.02
#   c-ink      on c-silk-deep  8.99    c-ochre    on c-silk      2.09
#   c-azurite  on c-silk       3.44    c-malachite on c-silk     2.85
# 承载文字的最低一对是 c-ink-soft on c-silk 的 5.02。赭石 2.09 只画 1px 的
# 线与开引号 —— 线是非文字图形；开引号是 88px 的记号而非要读的内容，且它
# 的职责恰恰是**退到金句后面**。青绿两支从不承载文字（spec 明令），它们只
# 在底部山形里出现，所以 3.44 / 2.85 不参与可读性判断。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, [card()], "引用书摘卡 · 绢本青绿 4:5")
