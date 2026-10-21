#!/usr/bin/env python3
"""pricing-tiers-comparison.op — 价格档位对比（1080×1440 竖版 3:4）

对比档的「递增序列」那一张：免费 / Pro / 团队三档并排，越往右买得越多。
它和其它多方对照的根本差别是——**三列之间有序**，右边那列包含左边那列，所
以每一列的第一行不是定位而是**价格**，读者是拿价格当锚点往下读的。

### 最近邻论证（为什么它不是已有的哪一张）

  - **本批 03 三方案横评**：那张的三列是三条**互不兼容**的路，谁也不是谁的
    升级版，所以推荐理由只能是「大多数人在这」。这张的三列是**一条递增的
    线**，右边严格包含左边，所以中间那列可以正当地打「最多人选」——多花的
    钱换到了什么，是能逐条数出来的。
  - **本批 01 参数表**：那张按指标横着比两台机器，指标之间没有包含关系；
    这张按档位竖着列，每一档的清单是**累加**的（下面一档默认包含上面）。
  - **本批 09 场景选择指南**：那张回答「什么情况选哪个」，单位是处境；这张
    回答「多少钱买到什么」，单位是价格。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：从「定价页」这件事采——纸质价目表、菜单、票价牌，那一族的记
    忆色是米黄纸配深棕墨，不是 SaaS 官网那种蓝。
  - **收敛**：单色相暖棕（H≈40°）的一条七级明度序列 L 0.10 / 0.22 / 0.40 /
    0.62 / 0.88 / 0.93 / 1.0，chroma 深处 0.04、浅处 0.012，只有一个色相。
  - **论证**：定价页最容易犯的错是给推荐档配一个高饱和色，读者会把那当成
    促销标签而不是信息。这里推荐档改用**明度反转**（唯一一块深色），它在
    三列里天然最重，却不带任何「打折 / 限时」的暗示；剩下的层级由字号（价
    格数字最大）和字重承担。

### 负约束（本模板明令不做的事）

  - **不用第二个色相。** 整张图只有暖棕一族 + 中性白。
  - 不用红绿。价格没有对错。
  - 不写「限时 / 立省 / 原价 ¥199」这类促销话术，也不画划掉的原价。这张图
    的任务是让人看懂差别，不是催人下单。
  - 不给推荐档加皇冠、火焰、星星。它凭「唯一一块深色」被认出来就够了。
  - 不用蓝紫渐变、霓虹线条、伪 3D、阴影、emoji 图标。
  - 每条包含项 ≤9 字：列内容宽 262px，23px 中文一行约 11 字，留两字余量。
  - 三档的条目数必须相同（这里各 4 条），否则三列一高一低，「递增」看起来
    像「随便列了列」。

硬契约：
  - 内容距边缘 ≥64px（这里 64）
  - 固定 3:4 画幅：根高写死 1440，靠 space_between 分配三块之间的空隙
  - 三列必须全部 fill_container + stretch
  - 配色全部走 color_vars；单色序列，改一处色相即整张换肤
  - 正文与背景对比度 ≥2.0（最低一对见文件末尾实测表）
  - **CJK 行高**：大标题 1.2，档名 1.3，正文 1.6
  - **CJK 负字距不超过 -0.02em**；价格数字走 Inter
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
    "c-bg":        "#F7F3EC",
    "c-card":      "#FFFFFF",
    "c-line":      "#DCD1BF",
    "c-ink":       "#1E1710",
    "c-deep":      "#3B2E1E",
    "c-muted":     "#6A5945",
    "c-faint":     "#9C8B75",
    # 深卡上的正文与次级文字要分两档。只有一档时，推荐列的清单会比左右两列
    # 的黑字看起来更"轻"，视觉上反而像被淘汰的那一列——正好和它的身份相反。
    "c-inv-body":  "#E4DAC9",
    "c-inv-muted": "#B6A78F",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W, H = 1080, 1440
EDGE = 64
GUTTER = 16

LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.3, 1.6

# (档名, 价格, 适合谁, [包含项 ≤9 字 ×4], 累加标, 是否推荐)
# 三档的条目数必须一样多，否则三列不等高。
TIERS = [
    ("免费", "0", "先试试水",
     ["每月 10 张", "带小水印", "标准清晰度", "只存七天"],
     "从这里开始", False),
    ("Pro", "29", "每周都要发",
     ["每月 300 张", "去掉水印", "高清导出", "永久保存"],
     "含免费档全部", True),
    ("团队", "99", "三个人以上",
     ["张数不限", "共享素材库", "成员权限", "优先答复"],
     "含 Pro 全部", False),
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


def tag(label, *, bg, fg, size=22):
    node = frame(ids, "标签", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[8, 18], cornerRadius=999,
                 alignItems="center", justifyContent="center", fill=solid(bg))
    node["children"] = [
        text(ids, "标签文字", label, size, 600, fg, family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
    ]
    return node


# --------------------------------------------------------------------- 页头
def head():
    return col("页头", [
        tag("价格档位 · 三档", bg="$c-ink", fg="$c-card", size=24),
        text(ids, "主标题", "多花的钱\n买到了什么", 72, 700, "$c-ink",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.4),
        text(ids, "副标题", "右边一档默认包含左边一档，只看新增的那几行就够。",
             26, 400, "$c-muted", family=CJK, line_height=LH_BODY),
    ], gap=20)


# --------------------------------------------------------------------- 三档
def price_line(amount, featured):
    """价格行。数字最大、单位最小——层级完全由字号给。"""
    return row("价格", [
        text(ids, "货币", "¥", 28, 600,
             "$c-inv-muted" if featured else "$c-faint", family=NUM,
             width="fit_content", growth="auto", line_height=1.1),
        text(ids, "金额", amount, 62, 700,
             "$c-card" if featured else "$c-ink", family=NUM,
             width="fit_content", growth="auto", line_height=1.0,
             spacing=-1.5),
        text(ids, "单位", "/月", 24, 400,
             "$c-inv-muted" if featured else "$c-muted", family=CJK,
             width="fit_content", growth="auto", line_height=1.1),
    ], gap=6, align="end", width="fit_content")


def feature(line, featured):
    mark = icon_font(ids, "含", "check", 20,
                     "$c-inv-body" if featured else "$c-ink")
    wrap = frame(ids, "标位", width=20, height=37, layout="vertical",
                 justifyContent="center", fill=[])
    wrap["children"] = [mark]
    return row("包含项", [
        wrap,
        text(ids, "包含项文字", line, 23, 400,
             "$c-inv-body" if featured else "$c-muted", family=CJK,
             line_height=LH_BODY),
    ], gap=10, align="start")


def carry_pill(label, featured):
    """「含上一档全部」。这一行是本模板的论点本身——三列是一条递增的线，
    右边严格包含左边。没有它，三列就退化成三个互不相干的选项。"""
    node = frame(ids, "累加标", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[7, 16], cornerRadius=6,
                 alignItems="center", justifyContent="center", fill=[],
                 stroke={"thickness": 2,
                         "fill": solid("$c-inv-muted" if featured
                                       else "$c-line")})
    node["children"] = [
        text(ids, "累加标文字", label, 21, 600,
             "$c-inv-body" if featured else "$c-muted", family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
    ]
    return node


def tier_card(name, amount, who, features, carry, featured):
    head_block = col("档头", [
        text(ids, "档名", name, 34, 700,
             "$c-card" if featured else "$c-ink", family=CJK,
             line_height=LH_HEAD),
        price_line(amount, featured),
        text(ids, "适合谁", who, 23, 400,
             "$c-inv-muted" if featured else "$c-muted", family=CJK,
             line_height=1.45),
    ], gap=12)
    body = col("档体", [
        head_block,
        rect(ids, "分线", width="fill_container", height=2,
             fill=solid("$c-inv-muted" if featured else "$c-line")),
        carry_pill(carry, featured),
        col("包含组", [feature(line, featured) for line in features], gap=10),
    ], gap=22, padding=[32, 22, 34, 22])
    card = col("档位", [body], gap=0, width="fill_container")
    card["fill"] = solid("$c-deep" if featured else "$c-card")
    card["cornerRadius"] = 6
    if not featured:
        card["stroke"] = {"thickness": 2, "fill": solid("$c-line")}
    return card


def tiers():
    cards = [tier_card(*tier) for tier in TIERS]
    return col("三档", [
        row("档位行", cards, gap=GUTTER, align="stretch"),
        row("推荐说明", [
            tag("最多人选", bg="$c-ink", fg="$c-card"),
            text(ids, "推荐理由", "中间那档是唯一一块深色——不是促销，是位置。",
                 24, 400, "$c-muted", family=CJK, line_height=LH_BODY),
        ], gap=14, align="center"),
    ], gap=22)


# --------------------------------------------------------------------- 页脚
def tail():
    band = col("页脚", [
        text(ids, "结语", "先按每月要发几张挑档，再看别的。", 29, 600,
             "$c-card", family=CJK, line_height=LH_HEAD),
        row("署名行", [
            text(ids, "账号名", "@ 你的账号名", 25, 600, "$c-card",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
            text(ids, "更新说明", "价格调整会重发一版", 23, 400,
                 "$c-inv-muted", family=CJK, width="fit_content",
                 growth="auto", line_height=1.4),
        ], gap=14),
    ], gap=12, padding=[32, 32])
    band["fill"] = solid("$c-ink")
    return band


def build():
    page = frame(ids, "价格档位对比", width=W, height=H, layout="vertical",
                 padding=[76, EDGE], gap=40, justifyContent="space_between",
                 alignItems="start", fill=solid("$c-bg"), clipContent=True)
    page["children"] = [head(), tiers(), tail()]
    page["x"], page["y"] = 0, 0
    return [page]


# 对比度（WCAG 相对亮度比，op-design-lint 门槛 2.0；数值由 contrast.py 实测）：
#   c-ink   on c-bg    16.02   c-muted on c-bg      6.07
#   c-ink   on c-card  17.72   c-muted on c-card    6.71
#   c-card  on c-deep  13.16   c-inv-body on c-deep  9.51
#   c-inv-muted on c-deep 5.59 c-card on c-ink     17.72
#   c-inv-muted on c-ink 7.52  c-faint on c-card    3.30
# 承载正文的最低一对是 5.59（推荐档的「适合谁」与货币符号）。清单正文走
# c-inv-body 的 9.51 —— 推荐列必须比左右两列更实，否则唯一的深色反而读成
# 「被划掉的那一列」。c-faint 只画货币符号旁的非推荐档小字与卡片描边。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "价格档位对比")
