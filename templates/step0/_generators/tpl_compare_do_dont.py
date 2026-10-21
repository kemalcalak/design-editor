#!/usr/bin/env python3
"""do-dont-comparison.op — 好坏示范双栏（1080×1440 竖版 3:4）

对比档里最经典的那一张：同一件事的两种做法，左边「常见写法」，右边「改成
这样」，中间不画分隔线——**两栏的材质本身就是分隔线**。

### 对错为什么不用红绿（本模板的核心决定）

红/绿是最常见的对错编码，也是最不该用的一种：红绿色盲约占男性 8%，对他们
来说这两栏是同一个颜色，整张图退化成两块看不出差别的方块。所以这里把「对
错」拆成两条各自独立、都不依赖色觉的通道：

  - **明度**：错误栏是纸白（轻、可疑、还没定稿），正确栏是近黑（重、已经
    落定）。灰度打印、截图压缩、色盲滤镜下这条通道全都活着。
  - **图形**：错误栏用**空心**语言——描边圆、虚线框、空心圆点；正确栏用
    **实心**语言——实心方、实心块、实心勾。哪怕整张图退成纯黑白线稿，空
    心和实心也分得开。

两条通道叠加，任何一条单独失效都还剩一条。这就是为什么这张图一个有彩色都
没有，却比红绿版更容易读。

### 最近邻论证（为什么它不是已有的哪一张）

  - **before-after（1600×900）**：那张是同一个东西的**两个时刻**，主体是
    用户拖进来的两张截图。这张是同一件事的**两种做法**，主体是两段写死的
    示例文案，不带图片位——它要能被直接抄走，而不是等用户填素材。
  - **pitfall-list-infographic（避坑排行）**：那张一屏里只出示「错」，改法
    是附在错后面的一句话；这张的错与对是**同一屏里等宽等高的两栏**，读者
    的眼睛必须在两栏之间来回扫——这正是对比档的动作。
  - **本批 06 误区 vs 真相**：那张是六组左右交错的 Z 字（一次读一组），这张
    是两栏到底（一次读一整套）。同为二元对照，节奏完全相反。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：不从内容采色。「标题该怎么写」没有颜色记忆，硬派一个色相等于
    抽签。
  - **收敛**：0 个有彩色 + 一条微暖中性明度序列 L 0.09 / 0.22 / 0.38 / 0.57 /
    0.87 / 0.91 / 0.95 / 1.0，chroma ≤0.005（偏暖一点点，避开冷灰的塑料
    感）。
  - **论证**：见上一节——对错通道必须对色盲成立，所以色相这个维度整个不
    用；省下来的表达力全部还给字号、字重和空/实。

### 负约束（本模板明令不做的事）

  - **不用红绿。** 这是本模板存在的理由，破了它整篇论证就没了。
  - **不用任何有彩色。** 包括「低饱和红绿」那种自以为温和的版本。
  - 不用 emoji 当对错符号（❌✅）。用 icon_font 的 x / check，跟着字色走。
  - 不在两栏中间画竖线。材质已经分开了，再加一条线是三重编码。
  - 不给错误栏加删除线、模糊、倾斜这类「羞辱」处理。错误示范要写得像真的
    有人这么写，读者才认得出自己。
  - **不用 dashPattern 表达「未定稿」。** 字段存在但没有渲染器读它（见
    sample() 里的注释），写了等于自欺。
  - 不用蓝紫渐变、霓虹线条、伪 3D、阴影。
  - 每条要点 ≤14 字，示例文案照抄真实语气，不写 AI 套话。

硬契约：
  - 内容距边缘 ≥64px（这里 64）
  - 固定 3:4 画幅：根高写死 1440，靠 space_between 分配三块之间的空隙
  - 配色全部走 color_vars，全是中性明度序列，没有「主色」可换
  - 正文与背景对比度 ≥2.0（最低一对见文件末尾实测表）
  - **CJK 行高**：大标题 1.2，栏标题 1.3，正文 1.7
  - **CJK 负字距不超过 -0.02em**
  - 两栏必须同宽同高（都 fill_container + stretch），否则「等价对照」这个
    前提在视觉上就先输了
"""

import os
import sys

sys.path.insert(0, os.path.join(
    os.path.dirname(os.path.dirname(os.path.dirname(
        os.path.dirname(os.path.abspath(__file__))))),
    "templates", "step0", "_generators"))

from oplib import (Ids, color_vars, frame, icon_font, rect, solid, text,
                   write_doc)

ids = Ids()

VARS = color_vars({
    "c-bg":        "#F4F3F1",
    "c-card":      "#FFFFFF",
    "c-panel":     "#E8E6E3",
    "c-line":      "#D6D4D0",
    "c-ink":       "#121110",
    "c-muted":     "#54524E",
    "c-faint":     "#8B8985",
    "c-inv-muted": "#A6A4A0",
})

CJK = "Noto Sans SC"

W, H = 1080, 1440
EDGE = 64
GUTTER = 24

LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.3, 1.7

# 左栏（错）与右栏（对）的示例封面标题。两句必须是**同一个选题**的两种写
# 法，否则读者比的是内容不是写法。
BAD_TITLE = "关于如何高效使用 AI 工具的一些方法分享"
GOOD_TITLE = "AI 工具只留三个"

# 每栏三条判据，逐条一一对应（第 n 条对第 n 条），≤14 字。
BAD_POINTS = [
    "把主题当成了标题",
    "读完才知道有没有用",
    "十九个字，手机上三行",
]
GOOD_POINTS = [
    "先写结论那一句",
    "一眼知道能拿走什么",
    "十个字以内，一行读完",
]


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


def tag(label, *, bg, fg):
    node = frame(ids, "标签", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[10, 22], cornerRadius=999,
                 alignItems="center", justifyContent="center", fill=solid(bg))
    node["children"] = [
        text(ids, "标签文字", label, 24, 600, fg, family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
    ]
    return node


# --------------------------------------------------------------------- 页头
def head():
    return col("页头", [
        tag("示范对比 · 封面标题", bg="$c-ink", fg="$c-card"),
        text(ids, "主标题", "同一件事\n两种写法", 78, 700, "$c-ink",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.5),
        text(ids, "副标题", "左边是大多数人的第一版，右边是改完之后的样子。",
             27, 400, "$c-muted", family=CJK, line_height=LH_BODY),
    ], gap=22)


# --------------------------------------------------------------------- 双栏
def marker(good):
    """栏首徽记。对 = 实心方块 + 实心勾；错 = 描边圆 + 描边里的叉。

    形状（方/圆）、材质（实心/空心）两处都不同，任何一条通道单独失效都还
    剩另一条——这就是不用红绿的代价被摊掉的地方。
    """
    if good:
        box = frame(ids, "对 · 实心徽记", width=52, height=52,
                    layout="horizontal", alignItems="center",
                    justifyContent="center", cornerRadius=8,
                    fill=solid("$c-ink"))
        box["children"] = [icon_font(ids, "勾", "check", 30, "$c-card")]
        return box
    box = frame(ids, "错 · 空心徽记", width=52, height=52,
                layout="horizontal", alignItems="center",
                justifyContent="center", cornerRadius=999, fill=[],
                stroke={"thickness": 3, "fill": solid("$c-ink")})
    box["children"] = [icon_font(ids, "叉", "x", 28, "$c-ink")]
    return box


def sample(good):
    """示例封面。错的一版是虚线空框，对的一版是实心黑块——空 / 实。"""
    title_color = "$c-ink" if not good else "$c-card"
    note_color = "$c-muted" if not good else "$c-inv-muted"
    box = col("示例封面", [
        text(ids, "示例标题", GOOD_TITLE if good else BAD_TITLE,
             34 if good else 28, 700 if good else 500, title_color,
             family=CJK, line_height=LH_HEAD),
        rect(ids, "示例分线", width=64, height=4,
             fill=solid("$c-inv-muted" if good else "$c-line")),
        text(ids, "示例小字",
             "一年攒下的工具清单" if good else "工具推荐 / 效率 / 方法论",
             24, 400, note_color, family=CJK, line_height=1.5),
    ], gap=14, padding=[30, 28])
    box["height"] = 262
    box["justifyContent"] = "center"
    if good:
        box["fill"] = solid("$c-ink")
    else:
        box["fill"] = solid("$c-card")
        # 这里**不要**写 dashPattern。`PenStroke` 的 schema 里有这个字段
        # （jian-ops-schema/src/style.rs 的 `dash_pattern`），但整条渲染链
        # 上没有任何一处读它 —— 全仓只有 op-html 的 mapper 会产出它。写了会
        # 画成实线，于是「虚线 = 未定稿」这层暗示只存在于源码里，读者看不
        # 见。空 / 实这条通道改由「只有描边 vs 整块实心」承担，那是真的画得
        # 出来的差别。
        box["stroke"] = {"thickness": 3, "fill": solid("$c-line")}
    return box


def point(line, good):
    """一条判据。对 = 实心小方；错 = 空心小圆。图形语言第二次出现。"""
    if good:
        bullet = rect(ids, "实心点", width=14, height=14, cornerRadius=3,
                      fill=solid("$c-ink"))
    else:
        # 空心圆必须是 ellipse 而不是「圆角 frame + 描边」：无子节点的圆角
        # 描边 frame 会被 stub_repair::is_empty_decorated_stub 判成「画了壳
        # 却没填内容」的废容器（该判定只认 type=="frame"）。ellipse 是这个
        # 形状本来就该用的节点类型，顺带绕开误判。
        bullet = {"type": "ellipse", "id": ids("e"), "name": "空心点",
                  "width": 14, "height": 14, "fill": [],
                  "stroke": {"thickness": 3, "fill": solid("$c-faint")}}
    # 圆点要对齐正文第一行的视觉中线：26px × 1.7 的行盒高 44，(44-14)/2 = 15。
    wrap = frame(ids, "点位", width=14, height=44, layout="vertical",
                 justifyContent="center", fill=[])
    wrap["children"] = [bullet]
    return row("判据", [
        wrap,
        text(ids, "判据文字", line, 26, 500 if good else 400,
             "$c-ink" if good else "$c-muted", family=CJK,
             line_height=LH_BODY),
    ], gap=14, align="start")


def outcome(good):
    """每栏的落点。判据讲的是「哪里不一样」，这一行讲「所以会怎样」——
    对比图缺了它就只是把两个东西摆在一起，读者拿不走结论。"""
    line = rect(ids, "落点上线", width="fill_container", height=2,
                fill=solid("$c-line"))
    body = row("落点", [
        icon_font(ids, "落点箭头", "corner-down-right", 24,
                  "$c-ink" if good else "$c-faint"),
        text(ids, "落点文字",
             "读者点开看看" if good else "读者直接划走", 26,
             600 if good else 400, "$c-ink" if good else "$c-muted",
             family=CJK, line_height=1.5),
    ], gap=12, align="center")
    return col("落点组", [line, body], gap=16)


def column(good):
    label = "改成这样" if good else "常见写法"
    points = GOOD_POINTS if good else BAD_POINTS
    body = col("栏内容", [
        row("栏首", [
            marker(good),
            text(ids, "栏标题", label, 34, 700, "$c-ink", family=CJK,
                 line_height=LH_HEAD),
        ], gap=16, align="center"),
        sample(good),
        col("判据组", [point(line, good) for line in points], gap=6),
        outcome(good),
    ], gap=24)
    shell = col("栏", [body], gap=0, width="fill_container", padding=[28, 26])
    shell["fill"] = solid("$c-panel" if good else "$c-bg")
    shell["stroke"] = {"thickness": 2, "fill": solid("$c-line")}
    return shell


def columns():
    return row("双栏", [column(False), column(True)], gap=GUTTER,
               align="stretch")


# --------------------------------------------------------------------- 结语
def tail():
    band = col("结语", [
        text(ids, "结语正文", "把左边那句话删到只剩结论，就是右边那句。",
             30, 600, "$c-card", family=CJK, line_height=LH_HEAD),
        row("署名行", [
            text(ids, "账号名", "@ 你的账号名", 25, 600, "$c-card",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
            text(ids, "更新说明", "每周一组能照着抄的示范", 23, 400,
                 "$c-inv-muted", family=CJK, width="fit_content",
                 growth="auto", line_height=1.4),
        ], gap=14),
    ], gap=12, padding=[34, 34])
    band["fill"] = solid("$c-ink")
    return band


def build():
    page = frame(ids, "好坏示范双栏", width=W, height=H, layout="vertical",
                 padding=[64, EDGE], gap=40, justifyContent="space_between",
                 alignItems="start", fill=solid("$c-bg"), clipContent=True)
    page["children"] = [head(), columns(), tail()]
    page["x"], page["y"] = 0, 0
    return [page]


# 对比度（WCAG 相对亮度比，op-design-lint 门槛 2.0；数值由 contrast.py 实测）：
#   c-ink  on c-bg    17.01   c-muted on c-bg     7.03
#   c-ink  on c-card  18.86   c-muted on c-card   7.80
#   c-ink  on c-panel 15.14   c-muted on c-panel  6.26
#   c-card on c-ink   18.86   c-inv-muted on c-ink 7.58
#   c-faint on c-bg    3.15（只画空心圆点，非文字图形）
# 承载正文的最低一对是 6.26（左栏判据的灰字若换到右栏底上）。整张图零有彩色，
# 「对 / 错」由明度 + 空实两条通道同时编码，任一条失效都还剩一条。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "好坏示范双栏")
