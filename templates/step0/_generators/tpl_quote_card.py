#!/usr/bin/env python3
"""punch-quote-card.op — 金句卡 · 大字报（3:4 单张，1080×1440）

和已有的 knowledge-card 系列是**同一画幅的反面**：那套是暖纸底上读四条要
点，这套是墨底上被一句话砸一下。两者都是「中文卡片」，但用途不同 —— 要点
卡给的是方法，金句卡给的是态度，所以差异必须做在气质上而不是配色上：

  - 底从暖纸 (#FFF7F0) 翻到近黑 (#131110)，明度关系整个反过来；
  - 字阶从 88 推到 118，且只讲**一句话**，没有编号列表；
  - 唯一的高饱和块是那条亮黄高亮条 —— 大字报的「刷标语」动作。

风格对口的是中文图文视觉体系里的「黑白灰 + 一个强调色冲击风」：黑白灰为
主，强调色只用于关键词、线条、编号，巨大标题加强留白，**一个**颜色作为视
觉焦点。第一版在署名右端还放了一枚朱红圆钮 —— 删了：那一档的原话是「一个
关键词作为视觉焦点」，第二个饱和色一出现，焦点就变成两个（顺带也解决了白
箭头压朱红只有 3.31 的那对对比度）。

### 负约束（本模板明令不做的事）

  - 不用第二个有彩色。整张卡只有一个黄，出现三次：标签、标语条、落地线。
  - 不用蓝紫渐变、霓虹线条、复杂背景纹理 —— 这三样是「廉价 AI 科技风」的
    全部构成，也是这套体系的禁止项第一条。
  - 装饰不许压到文字。两枚光晕都渐到全透明且与底色同族，标题区永远是干净
    的高对比区。
  - 不用 emoji 当图标，不用巨大页码抢戏。
  - 正文不超过 4 行（体系对手机端的硬要求），这里 2 行。
  - 不写 AI 套话（「赋能 / 无缝 / 一站式」），只写会被人转发的那种话。

硬契约（与 knowledge-card 一致）：
  - 内容距边缘 ≥80px（这里 96/88；体系给 1080 宽的下限是左右 72 / 上下 80）
  - 配色全部走 color_vars，改主色只改 $c-accent 一处
  - 正文与背景对比度 ≥2.0（承载文字的最低一对是 8.49，见文件末尾注释）
  - **字阶走 card-system spec §4.0 的档位**（`cardlib.SCALE`）：巨标题
    display-l(120) / 标语 display(88) / 说明 body-l(40) / 署名 body(36) /
    标签与简介 caption(32)。原来的 24-30px 正文低于体系的 32px 下限 ——
    1080 宽的卡片在手机上缩 2.77 倍，24px 只有 8.7pt。
  - **CJK 行高比西文全线高 0.2**：巨标题 1.15，标语 1.25，正文 1.7
  - **CJK 负字距不超过 -0.02em**（汉字是满格设计，再负就笔画相撞）
  - 汉字走 Noto Sans SC，数字与拉丁走 Inter
  - 顶层 frame 必须显式写 x/y，否则多帧会全部堆在原点
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from cardlib import step
from oplib import (Ids, color_vars, frame, radial, rect, solid, stack, text,
                   write_doc)

ids = Ids()

# 近黑而不是纯黑：#000000 在手机屏上和 UI 的黑边分不开，卡片会「没有边界」。
# 带一点暖（红通道最高）让它和亮黄站在同一个色温里。
VARS = color_vars({
    "c-bg":        "#131110",
    "c-surface":   "#1E1B19",
    "c-ink":       "#FFFFFF",
    "c-muted":     "#B5ADA2",
    "c-accent":    "#FFE04D",
    "c-border":    "#33302C",
    # 装饰光晕的芯色与外沿。**唯一带 alpha 的一组变量**：光晕要渐到全透明才
    # 不会在墨底上露出一圈硬边。装饰是正文的兄弟不是祖先，lint 找底色只走祖
    # 先链，碰不到这里（同 tpl_slides 的 c-arc）。
    "c-glow":      "#FFE04D22",
    "c-glow-out":  "#FFE04D00",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W, H = 1080, 1440
PAD_Y, PAD_X = 96, 88

# CJK 行高阶梯（西文 +0.2）。全篇只用这三档。
LH_DISPLAY, LH_HEAD, LH_BODY = 1.15, 1.25, 1.7


def block(name, children, gap=28, align="start"):
    """透明结构容器。没有 fill 才是结构层；一旦有 fill 它就是一张卡面了。"""
    node = frame(ids, name, width="fill_container", height="fit_content",
                 layout="vertical", gap=gap, alignItems=align, fill=[])
    node["children"] = children
    return node


def eyebrow(label):
    """亮黄小胶囊。黄色的第一次出现，也是最小的一次。"""
    node = frame(ids, "标签", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[12, 26], gap=0,
                 cornerRadius=999, alignItems="center",
                 justifyContent="center", fill=solid("$c-accent"))
    node["children"] = [
        text(ids, "标签文字", label, step("caption")[0], 700, "$c-bg",
             family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
    ]
    return node


def slogan(line):
    """高亮标语条 —— 大字报的那一下「刷」，也是全卡的视觉焦点。

    做成 fit_content 的黄底盒子而不是整行通栏：通栏黄条会变成一个页眉，
    贴着字收边才读得出「这句被荧光笔划过」。
    """
    node = frame(ids, "标语条", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[18, 36], gap=0,
                 cornerRadius=14, alignItems="center",
                 justifyContent="center", fill=solid("$c-accent"))
    node["children"] = [
        text(ids, "标语文字", line, 88, 700, "$c-bg", family=CJK,
             width="fit_content", growth="auto", line_height=LH_HEAD),
    ]
    return node


def signature():
    """署名条。一条细线 + 头像占位 + 名字，到此为止。

    结尾视觉应该像品牌签名而不是行动按钮：第一版右端那枚朱红圆钮既引入了
    第二个饱和色，又承诺了一个单张卡根本没有的「下一页」。
    """
    disc = frame(ids, "头像占位", width=84, height=84, layout="horizontal",
                 alignItems="center", justifyContent="center",
                 cornerRadius=42, fill=solid("$c-surface"))
    disc["children"] = [
        text(ids, "头像文字", "笔", 34, 700, "$c-accent", family=CJK,
             width="fit_content", growth="auto", line_height=1.0),
    ]

    who = block("署名文案", [
        text(ids, "账号名", "@ 你的账号名", step("body")[0], 600, "$c-ink",
             family=CJK, line_height=1.3),
        text(ids, "一句话简介", "每周一句，只讲能立刻用上的那种",
             step("caption")[0], 400, "$c-muted", family=CJK,
             line_height=1.7),
    ], gap=6)

    row = frame(ids, "署名行", width="fill_container", height="fit_content",
                layout="horizontal", gap=24, alignItems="center", fill=[])
    row["children"] = [disc, who]

    divider = rect(ids, "分隔线", width="fill_container", height=3,
                   fill=solid("$c-border"))
    return block("署名", [divider, row], gap=26)


def glow(size, x, y, alpha=1.0):
    """一枚渐到全透明的光晕。整张卡唯一的装饰语言，出现两次，同一个色族。"""
    node = rect(ids, "光晕", width=size, height=size, cornerRadius=size // 2,
                fill=radial([(0.0, "$c-glow"), (1.0, "$c-glow-out")]))
    node["x"], node["y"] = x, y
    if alpha < 1.0:
        node["opacity"] = alpha
    return node


def bottom_rule():
    """贴着下边缘的一道粗黄条 —— 海报的落地线，黄色的第三次（最后）出现。

    高度 20 而不是更粗：再粗就成了第二个标语条，会和上面那条抢。
    """
    node = rect(ids, "落地线", width=W, height=20, fill=solid("$c-accent"))
    node["x"], node["y"] = 0, H - 20
    return node


def card():
    body = frame(ids, "金句卡 · 正文", width="fill_container",
                 height="fill_container", layout="vertical",
                 padding=[PAD_Y, PAD_X], gap=0,
                 justifyContent="space_between", alignItems="start", fill=[])
    body["children"] = [
        block("卡头", [
            eyebrow("反常识 · 03"),
            text(ids, "主标题", "你不是没时间\n你是没开始",
                 step("display-l")[0], 700, "$c-ink", family=CJK,
                 line_height=LH_DISPLAY, spacing=-2),
        ], gap=30),
        block("主张", [
            slogan("先做五分钟"),
            text(ids, "说明", "把「等我准备好」换成「我先做五分钟」。\n"
                              "剩下的事，会在做的过程里自己长出来。",
                 step("body-l")[0], 400, "$c-muted", family=CJK,
                 line_height=LH_BODY),
        ], gap=30),
        signature(),
    ]

    shell = stack(ids, "金句卡 · 大字报", body, [
        glow(940, 560, 900),
        glow(720, -240, -200, alpha=0.9),
        bottom_rule(),
    ], width=W, height=H, fill=solid("$c-bg"))
    shell["x"], shell["y"] = 0, 0
    return shell


# 对比度（WCAG 相对亮度比，op-design-lint 的门槛是 2.0；数值实测）：
#   c-ink    on c-bg      18.83      c-muted on c-bg       8.49  ← 最低
#   c-accent on c-bg      14.36      c-bg    on c-accent  14.36
#   c-accent on c-surface 13.07      c-ink   on c-surface 17.13
# 最低一对 8.49，余量极大 —— 墨底本来就是给对比度用的，去掉朱红之后整张卡
# 再没有一对低于 AA。换主色时先量「$c-bg on $c-accent」：标语条把这一对放
# 到了 88px，它坏了整张卡就坏了。
# 主标题的 -2px 字距在 120px 上是 -0.017em，卡在 CJK 允许的 -0.02em 之内。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, [card()], "金句卡 · 大字报 3:4")
