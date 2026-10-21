#!/usr/bin/env python3
"""metric-single-card.op — 数据单值卡 · 网格汉字（1:1 单张，1080×1080）

卡片体系的活样张之一：**主题 T5 网格汉字 × 配方 D1 单值大屏**
（card-system-0808.md §3 / §4.1）。

一张卡只说一个数。D1 的层级是「数值 → 单位 → 解释 → 来源」，T5 把它放进
瑞士国际主义的骨架里：纯白、纯黑、一支信号红，严格 12 列网格，**装饰为零**
—— 所有视觉张力来自对齐、字重、留白和一个红方块。

### 主题 T5 的色彩推导（引自 spec §3，未改动）

  - **采样**：**不采**。这是 10 套主题里唯一的无采样主题 —— 国际主义的前
    提就是色彩不承载文化信息，只承载信号。
  - **收敛**：中性序列 L 1.00 / 0.96 / 0.86 / 0.52 / 0.18（chroma 全 0，真
    中性）；单支信号红 H28 C0.190。
  - **论证**：信号红的 chroma 拉到 0.19（超出主色区间上沿）是刻意的 —— 在
    一个完全无彩的系统里，红是唯一的信号，它必须是纯信号强度而不是「一个
    红」。这与印刷 chroma 纪律冲突，冲突处以「只用小面积（≤3%）」化解。

### 一处主题 × 配方冲突，按 spec §6.1 判「主题赢」

D1 写的是「数值居中 8 列」，T5 写的是「标题不居中（国际主义是左对齐系
统）」。两条直接打架。spec 的裁决规则是**主题赢**——一套图文里主题跨页恒
定、配方逐页可换，所以这张卡**全部左对齐**，不居中。

### 负约束（T5 主题约束，逐条照搬）

  - 圆角一律 0。
  - 无阴影、无渐变、无纹理、无图标填充（图标只用线性且 stroke 2px）。
  - **红只出现一次。**
  - 标题不居中 —— 国际主义是左对齐系统。
  - 不用任何非 90° 的旋转。
  - 允许的图形只有三个：1px 分隔线、实心黑横杠（高 8-12，宽为列宽整数
    倍）、信号红实心方块（边长 = 基线单位 ×3 = 24）。**其余一切图形在本主
    题里都是错误。**

### 硬契约（spec §4.0 / §5）

  - 画布 1080×1080，安全区 左右 80 / 上 88 / 下 112
  - 字阶用满 4 档上限：display-xl（数值）/ title-2（单位）/ body-l（解释）
    / caption（标签与来源）
  - 数字走西文族并靠等宽感对齐；汉字走黑体
  - 数据类配方必须有来源行（spec D 族共同约束）
  - 顶层 frame 必须显式写 x/y
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from cardlib import NUM, SANS, SQUARE, step
from oplib import Ids, color_vars, frame, rect, solid, text, write_doc

ids = Ids()

# spec §3 · T5 的色板。chroma 全 0 的真中性 + 一支信号红。
VARS = color_vars({
    "c-white":  "#FFFFFF",
    "c-paper":  "#F2F2F2",
    "c-rule":   "#D1D1D1",
    "c-grey":   "#696969",
    "c-black":  "#121212",
    "c-signal": "#CC342C",
})

C = SQUARE
# 基线单位 8；信号方块边长 = 8 × 3。spec 把这个尺寸写死，是为了让「唯一的
# 红」在任何一张 T5 卡片上都是同一个大小 —— 它是符号，不是装饰元素。
SIGNAL = 24


def col(name, children, *, gap=16, width="fill_container", align="start",
        **props):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="vertical", gap=gap, alignItems=align, fill=[],
                 **props)
    node["children"] = children
    return node


def row(name, children, *, gap=16, align="center", width="fill_container",
        **props):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="horizontal", gap=gap, alignItems=align, fill=[],
                 **props)
    node["children"] = children
    return node


def typed(name, content, scale_name, weight, color, *, family=SANS,
          width="fill_container", growth="fixed-width"):
    size, line_height, spacing = step(scale_name)
    return text(ids, name, content, size, weight, color, family=family,
                line_height=line_height, width=width, growth=growth,
                spacing=spacing)


def slab(cols):
    """实心黑横杠。宽必须是列宽的整数倍 —— 它是网格的可见证据。"""
    return rect(ids, "黑横杠", width=C.cols(cols), height=10,
                fill=solid("$c-black"))


def hairline():
    return rect(ids, "分隔线", width="fill_container", height=1,
                fill=solid("$c-rule"))


def head():
    """黑横杠 + 红方块 + 标签。

    红方块放在**页首**而不是贴着巨数字：国际主义里这种方块是「章节记号」，
    它标的是这一页的身份。第一版把它塞在 168px 数字的左下角，尺寸差 7 倍，
    读出来是一粒掉在那儿的灰尘而不是一个信号。
    """
    mark = row("记号行", [
        slab(3),
        rect(ids, "信号方块", width=SIGNAL, height=SIGNAL,
             fill=solid("$c-signal")),
    ], gap=16, align="center", width="fit_content")
    return col("卡头", [
        mark,
        typed("标签", "季度指标 · 2026 Q2", "caption", 500, "$c-grey"),
    ], gap=20)


def metric():
    """数值 + 单位。单位与数值同一行、底对齐 —— 它们是一个词组不是两件事。"""
    size, line_height, spacing = step("display-xl")
    unit_size, _, unit_spacing = step("title-2")
    value_row = row("数值行", [
        text(ids, "数值", "68", size, 700, "$c-black", family=NUM,
             line_height=line_height, width="fit_content", growth="auto",
             spacing=spacing),
        text(ids, "单位", "%", unit_size, 600, "$c-grey", family=NUM,
             line_height=1.0, width="fit_content", growth="auto",
             spacing=unit_spacing),
    ], gap=20, align="end", width="fit_content")
    return value_row


def body():
    return col("释义", [
        typed("解释", "模板起稿的人里，68% 在两周内做出了第二张图。",
              "body-l", 400, "$c-black"),
    ], gap=16)


def source():
    return col("来源", [
        hairline(),
        typed("来源行", "口径：2026-04-01 至 06-30 新建文档，按人去重。",
              "caption", 400, "$c-grey"),
    ], gap=20)


def card():
    node = frame(ids, "数据单值卡 · 网格汉字", width=C.width, height=C.height,
                 layout="vertical", padding=C.padding, gap=0,
                 justifyContent="space_between", alignItems="start",
                 fill=solid("$c-white"), clipContent=True)
    node["children"] = [head(), metric(), body(), source()]
    node["x"], node["y"] = 0, 0
    return node


# 对比度（WCAG 相对亮度比，op-design-lint 的门槛是 2.0；数值实测）：
#   c-black  on c-white   18.10    c-grey   on c-white    5.74
#   c-black  on c-paper   16.58    c-grey   on c-paper    5.26
#   c-signal on c-white    5.07    c-rule   on c-white    1.60
# 承载文字的最低一对是 c-grey on c-white 的 5.74。c-signal 不承载文字（它是
# 一个 24px 的实心方块），5.07 只说明它作为图形在白底上足够醒目。c-rule 是
# 1px 分隔线，非文字图形，1.60 是它该有的重量 —— 它的职责是「在那儿」，
# 不是「被看见」。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, [card()], "数据单值卡 · 网格汉字 1:1")
