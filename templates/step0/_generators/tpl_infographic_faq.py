#!/usr/bin/env python3
"""faq-thread-infographic.op — 问答 FAQ 长图（1080×N 竖版）

信息图这一档的第七张，回答**「被问了很多遍的那几个问题」** —— 一张把私信
里重复出现的提问一次答完的长图。

### 最近邻差异（为什么它不是 pitfall-list 或 concept 换个壳）

  - **对 pitfall-list：谁在说话不一样。** pitfall 是作者单方面的断言（「别
    这么做」），条目之间没有对话关系；FAQ 每一格都是**两个声部**，问句必
    须以读者的口吻写、答句才是作者的。所以这张图的最小单元不是「一条」，
    是「一问一答」，视觉上必须有两枚身份标记（Q 实心 / A 描边）和一条把
    它们分开的横线。
  - **对 concept：它答的不是同一类问题。** concept 拆的是一组概念的内部
    差异（A 与 B），是横向的；FAQ 是六个互不相干的问题竖着排，读者可以只
    读其中一条就走。所以它没有表头、没有分栏，也不需要读完。
  - **对 steps：没有顺序。** steps 的第三步依赖第二步；FAQ 的第五问和第一
    问互不相欠，所以不编号、不画箭头、不做进度感 —— 编号会假装出一条并不
    存在的路径。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：问答的气质是「有人耐心地回你」，不能太冷（灰）也不能太急
    （橙红）。紫罗兰（~275°）在这一档里是唯一没被占的中间地带：比蓝暖、
    比莓红静。
  - **收敛**：一个色相 + 一条冷灰序列。紫只出现在三处：Q 徽标、区块短线、
    页头胶囊。答句一律走中性墨色。
  - **论证**：FAQ 的层级是**问 > 答**，而这个层级已经由字号（问 32 / 答
    27）和字重（600 / 400）表达完了。颜色再去强调一次就是重复编码 —— 所
    以紫只标「这是一个问题的开始」，不参与正文。

### 负约束（本模板明令不做的事）

  - **不用蓝紫渐变。** 紫色是「廉价 AI 科技风」的重灾色相，一上渐变立刻
    掉进去。全图只有实色块，且紫的总面积不超过一成。
  - 不给问答编号、不画连接箭头、不做折叠面板的三角标 —— 静态图片里的折叠
    标记是假交互。
  - 不用问号气泡、不用对话框尖角、不用头像 —— 两枚 Q / A 方标已经把「谁在
    说话」说清楚了。
  - 不用 emoji 当图标、不用装饰性插画、不用伪 3D。
  - 一问一答，答句不超过两行；答不完就拆成两问，不缩字号。
  - 不写 AI 套话（「因人而异 / 视情况而定 / 保持热爱」），每个答句都要给
    出一个可执行的判断标准或动作。

硬契约：
  - 内容距边缘 ≥80px（这里 80）
  - 配色全部走 color_vars，改主色只改 $c-accent / $c-accent-deep 两处
  - 正文与背景对比度 ≥2.0（本配色最低一对见文件末尾注释）
  - **CJK 行高比西文全线高 0.2**：页头大标题 1.2，问句 1.3，答句 1.7
  - **CJK 负字距不超过 -0.02em**；只有西文字母标记沿用西文的收紧
  - 汉字走 Noto Sans SC，数字与拉丁走 Inter
  - 顶层 frame 必须显式写 x/y，否则多帧会全部堆在原点
  - 根高固定：ROOT_H 是量出来的（见文件末尾），改内容后要重量一次
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import (Ids, color_vars, frame, icon_font, rect, solid, text,
                   write_doc)

ids = Ids()

VARS = color_vars({
    "c-bg":          "#F6F3FB",
    "c-surface":     "#FFFFFF",
    "c-band":        "#201741",
    "c-band-muted":  "#A99EC8",
    "c-ink":         "#1F1730",
    "c-muted":       "#625A78",
    "c-accent":      "#7A3FBF",
    # A 标记与浅紫底上小字用的深一档。主强调色在 #EDE4FB 上只有 5.0，够
    # 32px 的字母不够 26px 的正文。
    "c-accent-deep": "#5C2C95",
    "c-accent-soft": "#EDE4FB",
    "c-border":      "#E5DCF2",
    # 问与答之间那条横线。比 c-border 深一档 —— c-border 在白卡上只有
    # 1.32，画在卡内几乎看不见（第一版实测如此）。
    "c-rule":        "#CDBFE4",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W = 1080
EDGE = 80

LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.3, 1.7

# Q / A 两枚方标的边长。两者同尺寸、只差实心与描边 —— 尺寸差会暗示层级，
# 而问与答在版面上是平级的。
MARK = 46

# 量出来的根高（做法同同档另外六张：根设 fit_content 渲一次读 PNG 高度）。
ROOT_H = 2789

# (问, 答)。问句用读者的口吻，答句必须给出可执行的判断标准。
QA = [
    ("没人看还要不要继续发？",
     "先定一个与播放量无关的目标：连发十期，第十期之后再决定。"),
    ("多久发一次合适？",
     "选一个你在最忙的那一周也能做到的频率。做不到的频率会让你在第五周"
     "直接停掉。"),
    ("要不要先攒够素材再开始？",
     "不要。攒素材是最舒服的一种拖延，发出去之后才知道该攒什么。"),
    ("封面和内容哪个更重要？",
     "封面决定点不点进来，内容决定下次还点不点。前十期先做内容。"),
    ("被说得不好听怎么办？",
     "只回带具体建议的那一条，其余不回。回一条要花掉二十分钟，"
     "值不值这个时间自己算。"),
    ("什么时候可以开始收费？",
     "有人第二次主动来找你的时候。第一次可能是好奇，第二次才是需求。"),
]

INVITE = [
    ("这六个问题占了私信的八成，其余多是这六个的变体。", "message-square"),
    ("还有没答上的，写在评论区 —— 下一张从被问得最多的那条开始。", "pen-line"),
]


def band(name, *, fill, pad, gap, children, align="start"):
    """一个通栏区块。fill 决定它是不是一块有颜色的带 —— 结构容器不写 fill。"""
    node = frame(ids, name, width="fill_container", height="fit_content",
                 layout="vertical", padding=pad, gap=gap, alignItems=align,
                 fill=fill)
    node["children"] = children
    return node


def col(name, children, *, gap=16, width="fill_container", align="start",
        height="fit_content", **props):
    node = frame(ids, name, width=width, height=height, layout="vertical",
                 gap=gap, alignItems=align, fill=[], **props)
    node["children"] = children
    return node


def row(name, children, *, gap=24, align="center", width="fill_container",
        **props):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="horizontal", gap=gap, alignItems=align, fill=[],
                 **props)
    node["children"] = children
    return node


def chip(label, *, bg, fg, size=24):
    node = frame(ids, "胶囊", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[10, 22], cornerRadius=999,
                 alignItems="center", justifyContent="center", fill=solid(bg))
    node["children"] = [
        text(ids, "胶囊文字", label, size, 600, fg, family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
    ]
    return node


def section_head(title, note):
    return col("区块头", [
        rect(ids, "强调短线", width=72, height=8, cornerRadius=999,
             fill=solid("$c-accent")),
        text(ids, "区块标题", title, 46, 700, "$c-ink", family=CJK,
             line_height=LH_HEAD),
        text(ids, "区块说明", note, 27, 400, "$c-muted", family=CJK,
             line_height=LH_BODY),
    ], gap=16)


def mark(letter, *, filled):
    """Q / A 身份标记。同尺寸、同位置，只差实心与描边。"""
    node = frame(ids, f"{letter} 标", width=MARK, height=MARK,
                 layout="horizontal", alignItems="center",
                 justifyContent="center", cornerRadius=12,
                 fill=solid("$c-accent") if filled else [])
    if not filled:
        node["stroke"] = {"thickness": 2, "fill": solid("$c-accent")}
    node["children"] = [
        text(ids, "标记字母", letter, 24, 700,
             "$c-surface" if filled else "$c-accent-deep", family=NUM,
             width="fit_content", growth="auto", line_height=1.0),
    ]
    return node


# ------------------------------------------------------------------ 01 页头
def header():
    return band("01 页头", fill=solid("$c-band"), pad=[76, EDGE, 68, EDGE],
                gap=26, children=[
        chip("常见问题 · 六问六答", bg="$c-accent", fg="$c-surface"),
        text(ids, "主标题", "被问最多的\n六个问题", 76, 700, "$c-surface",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.4),
        text(ids, "副标题", "每条都能单独读完，挑你现在正卡住的那条看就行。",
             28, 400, "$c-band-muted", family=CJK, line_height=LH_BODY),
    ])


# ------------------------------------------------------------------ 02 问答
def qa_card(question, answer):
    """一问一答。两枚方标 + 一条横线，是这张图唯一的重复结构。"""
    ask = row("问", [
        mark("Q", filled=True),
        text(ids, "问句", question, 32, 600, "$c-ink", family=CJK,
             line_height=LH_HEAD),
    ], gap=20, align="start")

    reply = row("答", [
        mark("A", filled=False),
        text(ids, "答句", answer, 27, 400, "$c-muted", family=CJK,
             line_height=LH_BODY),
    ], gap=20, align="start")

    card = col("问答卡", [
        ask,
        rect(ids, "问答分割线", width="fill_container", height=2,
             fill=solid("$c-rule")),
        reply,
    ], gap=22, padding=[32, 32], cornerRadius=22)
    card["fill"] = solid("$c-surface")
    card["stroke"] = {"thickness": 2, "fill": solid("$c-border")}
    return card


def questions():
    cards = [qa_card(*entry) for entry in QA]
    return band("02 问答", fill=[], pad=[68, EDGE, 0, EDGE], gap=32,
                children=[
        section_head("挑一条现在就想问的",
                     "六条互不相欠，跳着看也行，没有先后顺序。"),
        col("问答列表", cards, gap=18),
    ])


# ------------------------------------------------------------------ 03 收尾
def invite():
    items = []
    for line, glyph in INVITE:
        items.append(row("收尾项", [
            icon_font(ids, "图标", glyph, 28, "$c-accent-deep"),
            text(ids, "收尾文字", line, 27, 500, "$c-ink", family=CJK,
                 line_height=LH_BODY),
        ], gap=16, align="start"))
    panel = col("收尾面板", items, gap=18, padding=[36, 34], cornerRadius=22)
    panel["fill"] = solid("$c-accent-soft")
    return band("03 收尾", fill=[], pad=[68, EDGE, 68, EDGE], gap=32,
                children=[
        section_head("还有别的问题吗",
                     "这张图会随问题更新，被问到第三次的会被加进来。"),
        panel,
    ])


# ------------------------------------------------------------------ 04 页脚
def footer():
    return band("04 页脚", fill=solid("$c-band"), pad=[44, EDGE], gap=12,
                children=[
        text(ids, "口径", "问题取自近半年私信，同义合并后按出现次数从多到少排。",
             24, 400, "$c-band-muted", family=CJK, line_height=1.6),
        row("署名行", [
            text(ids, "账号名", "@ 你的账号名", 26, 600, "$c-surface",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
            text(ids, "更新说明", "每季度更新一次这张问答", 24, 400,
                 "$c-band-muted", family=CJK, width="fit_content",
                 growth="auto", line_height=1.4),
        ], gap=16, width="fill_container"),
    ])


def build():
    page = frame(ids, "问答 FAQ 长图", width=W, height=ROOT_H,
                 layout="vertical", gap=0, fill=solid("$c-bg"),
                 clipContent=True)
    page["children"] = [header(), questions(), invite(), footer()]
    page["x"], page["y"] = 0, 0
    return [page]


# 对比度（WCAG 相对亮度比，op-design-lint 的门槛是 2.0；数值实测）：
#   c-ink         on c-bg          15.64   c-muted      on c-bg          5.88
#   c-ink         on c-surface     17.16   c-muted      on c-surface     6.46
#   c-surface     on c-band        16.66   c-band-muted on c-band        6.67
#   c-accent-deep on c-surface      9.34   c-accent     on c-surface     6.38
#   c-accent-deep on c-accent-soft  7.60   c-ink        on c-accent-soft 13.97
#   c-surface     on c-accent       6.38   c-muted      on c-accent-soft 5.25
#   c-rule        on c-surface      1.73   c-border     on c-surface     1.32
# 承载文字的最低一对是 5.25。c-rule / c-border 是结构线与卡片描边（非文字
# 图形）：它们只负责把问与答、卡与卡分开，拿掉之后版面依然可读，所以按结
# 构线而非信息图形处理，不套 3.0 门槛。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "问答 FAQ 长图")
