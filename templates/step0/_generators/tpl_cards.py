#!/usr/bin/env python3
"""knowledge-card-vertical.op + knowledge-card-square.op — 中文知识卡片

两套模板共用一份生成器，因为它们是同一套设计语言的两个画幅：竖版
1080×1440 发小红书图文，方版 1080×1080 发公众号头图和朋友圈。分成两个
文件写，改一处配色就得记得改另一处；合成一个，两张卡永远同源。

版式只做「标题 + 要点 + 署名」三段，是刻意的：卡片场景的用户要改的是
文案，不是结构。多一个装饰区就多一处他们得先看懂再删掉的东西。

字阶已对齐 card-system spec §4.0 的 8 档（`cardlib.SCALE`）。**改动的理由**：
spec 定「任何小于 32px 的文字在本体系里是错误」—— 1080 宽的卡片在约 390pt
的手机上缩约 2.77 倍，32px 落到 11.6pt，那是注释可读的下限。这两张卡原来
的正文是 26-27px（≈9.5pt），署名 23-24px（≈8.5pt），用户在手机上会真实地
觉得「字好小」。现在正文走 body（36）与 body-l（40），署名与标签走
caption（32），一档不低于下限。

行高同时按「中文行高比西文高 0.2」重排：正文 1.6 → 1.7。

硬契约：
  - 内容距边缘 ≥80px（竖版 88/96，方版 84/88）
  - 配色全部走 color_vars，改主色只改 $c-accent 一处
  - 正文与背景对比度 ≥2.0（本配色最低一对是 3.67，见文件末尾注释）
  - 字阶只用 spec 的档位，且单页 ≤4 档
  - CJK 行高：大标题 1.15，条目标题 1.3，正文 1.7
  - 顶层 frame 必须显式写 x/y，否则多帧会全部堆在原点
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from cardlib import step
from oplib import Ids, color_vars, frame, rect, solid, text, write_doc

ids = Ids()

# 暖纸底 + 单一强调色。知识卡片是「读」的东西，底色带一点暖比纯白更耐看，
# 也和已有的 knowledge-carousel（冷调靛蓝）拉开距离。
VARS = color_vars({
    "c-bg":          "#FFF7F0",
    "c-surface":     "#FFFFFF",
    "c-ink":         "#241A12",
    "c-muted":       "#7A6455",
    "c-accent":      "#C8502B",
    "c-accent-soft": "#FAE3D8",
    "c-border":      "#EDDCD0",
})

# 正文用的中文字族。oplib 的默认族按字号/字重挑（<34 且 <600 走 Inter），
# 那条规则是为中英混排写的；这两张卡是纯中文，所以每个文本节点都显式写死
# 中文族，免得小字号正文落到一个没有汉字的字族上再靠 fallback 兜。
CJK = "Noto Sans SC"
# 序号和数字用 Inter：等宽感更强，"01" 这类标号排出来才齐。
NUM = "Inter"


def card(name, *, width, height, pad_y, pad_x, children):
    """一张卡。固定画幅，绝不 fit_content —— 发布尺寸是硬约束。

    `space_between` 把「头部 / 要点 / 署名」三段推到各自该在的位置：
    卡片的留白不该是均匀的 gap，而是三段之间自然撑开的呼吸。
    """
    node = frame(ids, name, width=width, height=height, layout="vertical",
                 padding=[pad_y, pad_x], gap=0,
                 justifyContent="space_between", alignItems="start",
                 fill=solid("$c-bg"), clipContent=True)
    node["children"] = children
    node["x"] = 0
    node["y"] = 0
    return node


def block(name, children, gap=32):
    """透明结构容器。没有 fill 才是结构层；一旦有 fill 它就是一张卡面了。"""
    node = frame(ids, name, width="fill_container", height="fit_content",
                 layout="vertical", gap=gap, alignItems="start", fill=[])
    node["children"] = children
    return node


def eyebrow(label, size=None):
    """小标签。整张卡唯一的胶囊，用来交代这是哪一类内容。"""
    if size is None:
        size = step("caption")[0]
    node = frame(ids, "标签", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[12, 24], gap=0,
                 cornerRadius=999, alignItems="center",
                 justifyContent="center", fill=solid("$c-accent-soft"))
    node["children"] = [
        text(ids, "标签文字", label, size, 600, "$c-accent", family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
    ]
    return node


def rule(width=132, height=12):
    return rect(ids, "强调短线", width=width, height=height,
                cornerRadius=height // 2, fill=solid("$c-accent"))


def numbered(no, size):
    """实心序号圆。白字压在主色上，是卡里对比度最高的一处，读者的视线锚点。"""
    node = frame(ids, f"序号 {no}", width=size, height=size,
                 layout="horizontal", alignItems="center",
                 justifyContent="center", cornerRadius=size // 2,
                 fill=solid("$c-accent"))
    node["children"] = [
        # 序号数字锁在 caption 档：原来按圆径的一半算，落到 26-28px，
        # 低于体系的 32px 下限。圆径反过来跟着数字走。
        text(ids, "序号数字", str(no), step("caption")[0], 700, "#FFFFFF",
             family=NUM, width="fit_content", growth="auto", line_height=1.0),
    ]
    return node


def point(no, title, desc, *, badge, title_size, desc_size, gap):
    """一条要点。`desc=None` 时只排标题。

    方版走无注解的形态，是 spec §5.2 的换规格规则：3:4 → 1:1 内容高度压
    缩 25%，处理方式是**减少行数**而不是缩小字号。字阶提到体系档位之后，
    带注解的三条要点在 1080 高里放不下 —— 砍注解，不砍字号。
    """
    lines = [
        text(ids, "要点标题", title, title_size, 600, "$c-ink",
             family=CJK, line_height=1.3),
    ]
    if desc is not None:
        lines.append(text(ids, "要点说明", desc, desc_size, 400, "$c-muted",
                          family=CJK, line_height=1.7))
    node = frame(ids, f"要点 {no}", width="fill_container",
                 height="fit_content", layout="horizontal", gap=gap,
                 alignItems="center" if desc is None else "start", fill=[])
    node["children"] = [numbered(no, badge), block("要点文案", lines, gap=10)]
    return node


def signature(*, avatar, name_size, slogan_size, gap):
    """署名条。一条细分隔线 + 头像占位 + 名字 + 一句 slogan。

    头像用 frame 而不是 rectangle：frame 会递归渲染子节点，圆里那个字才
    显示得出来；同时 frame 也是合法的图片拖放目标，用户把自己的头像拖上去
    就直接替换掉这个占位。
    """
    disc = frame(ids, "头像占位", width=avatar, height=avatar,
                 layout="horizontal", alignItems="center",
                 justifyContent="center", cornerRadius=avatar // 2,
                 fill=solid("$c-accent-soft"))
    disc["children"] = [
        text(ids, "头像文字", "笔", round(avatar * 0.42), 700, "$c-accent",
             family=CJK, width="fit_content", growth="auto", line_height=1.0),
    ]

    row = frame(ids, "署名行", width="fill_container", height="fit_content",
                layout="horizontal", gap=gap, alignItems="center", fill=[])
    row["children"] = [
        disc,
        block("署名文案", [
            text(ids, "账号名", "@ 你的账号名", name_size, 600, "$c-ink",
                 family=CJK, line_height=1.3),
            text(ids, "一句话简介", "每周更新一条可落地的学习方法",
                 slogan_size, 400, "$c-muted", family=CJK, line_height=1.7),
        ], gap=6),
    ]

    divider = rect(ids, "分隔线", width="fill_container", height=2,
                   fill=solid("$c-border"))
    return block("署名", [divider, row], gap=gap)


# ----------------------------------------------------------- 竖版 1080×1440
# 注解一律收到 ≤20 字。spec 的 C 族共同约束写着「每条注解 ≤1 行」，而
# 36px 正文在 816px 的可用宽里一行放得下 22 字 —— 原来的 25 字注解会折成
# 两行，四条就多出 122px，正好把固定高的卡片顶穿。
VERTICAL_POINTS = [
    ("先提问，再翻开", "带着问题进去，信息才有地方落。"),
    ("合上书，先复述", "讲不出来的地方，就是没懂的地方。"),
    ("一次只带走三条", "记住三个能用的点就够了。"),
    ("两天之内用一次", "用过一次，才算真的学过。"),
]


def vertical():
    head = block("卡头", [
        eyebrow("学习方法 · 01"),
        text(ids, "主标题", "为什么你读了很多书\n却什么都记不住", 88, 700,
             "$c-ink", family=CJK, line_height=1.15),
        rule(),
        text(ids, "副标题", "不是记性差，是你少做了这一步。",
             step("body-l")[0], 400, "$c-muted", family=CJK, line_height=1.7),
    ], gap=24)

    points = block("要点列表", [
        point(no, title, desc, badge=64, title_size=step("title-2")[0],
              desc_size=step("body")[0], gap=24)
        for no, (title, desc) in enumerate(VERTICAL_POINTS, 1)
    ], gap=24)

    return card("知识卡片 · 竖版", width=1080, height=1440, pad_y=96, pad_x=88,
                children=[head, points,
                          signature(avatar=88, name_size=step("body")[0],
                                    slogan_size=step("caption")[0], gap=24)])


# ----------------------------------------------------------- 方版 1080×1080
# 方版只留标题（见 `point` 的说明）。
SQUARE_POINTS = ["先提问，再翻开", "合上书，先复述", "两天之内用一次"]


def square():
    head = block("卡头", [
        eyebrow("学习方法"),
        text(ids, "主标题", "读完就忘？\n先补上这一步", step("display")[0], 700,
             "$c-ink",
             family=CJK, line_height=1.15),
        rule(112, 10),
        text(ids, "副标题", "三个动作，把书里的东西真正带走。",
             step("body-l")[0], 400, "$c-muted", family=CJK, line_height=1.7),
    ], gap=22)

    points = block("要点列表", [
        point(no, title, None, badge=64, title_size=step("title-2")[0],
              desc_size=step("body")[0], gap=22)
        for no, title in enumerate(SQUARE_POINTS, 1)
    ], gap=24)

    return card("知识卡片 · 方版", width=1080, height=1080, pad_y=88, pad_x=84,
                children=[head, points,
                          signature(avatar=80, name_size=step("body")[0],
                                    slogan_size=step("caption")[0], gap=22)])


# 对比度（WCAG 相对亮度比，op-design-lint 的门槛是 2.0）：
#   c-ink   on c-bg          16.09      c-muted on c-bg          5.24
#   c-accent on c-bg          4.27      c-accent on c-accent-soft 3.67
#   #FFFFFF on c-accent       4.53
# 最低一对 3.67 仍高出门槛近一倍，换主色时保留这个余量即可。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, [vertical()], "知识卡片 · 竖版 3:4")
    write_doc(sys.argv[2], VARS, [square()], "知识卡片 · 方版 1:1")
