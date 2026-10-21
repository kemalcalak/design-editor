#!/usr/bin/env python3
"""lecture-deck-light.op — 浅色课件 deck（封面 / 目标 / 讲解 / 例题 / 对比 / 小结）

课件跟汇报 deck 不是同一件事：听众要跟着做笔记，所以正文比路演更密、更
完整，序号和步骤必须一眼数得清；同时它会被投在教室最后一排也看得见的地
方，所以字号一点不能省。米白纸底 + 墨绿，是为了长时间盯着看不刺眼。

这一版加了三样东西，都是为了把「白底加白卡」做出纸的层次：

1. **整套贯穿一张极淡的坐标网格。** 装饰层里 120px 一格的墨绿细线，压在
   内容之下（`oplib.stack`），所以只在留白处看得见 —— 卡片一压上去就自然
   盖住。这套讲的是函数与导数，坐标纸本身就是内容的一部分，不是花纹。
2. **知识点卡片有左侧色条。** 每张卡最左边一道 10px 的墨绿，是这套课件的
   「知识点」标记；对比表、例题卡这类「不是知识点」的块一律不带，色条因此
   有语义而不只是装饰。
3. **例题页有 ghost 大序号。** 满色 + 6% 图层透明度，压在内容之下。

字阶也拉开了：封面 116 / 正文 30（近 4 倍），页标题 76 / 正文 28，展示字
统一收紧字距。

排版遵循 skills/domains/slides.md 的硬契约：
  - 每帧固定 1920×1080，绝不 fit_content；内容距边缘 ≥100（这里 120）
  - 正文 ≥24（取 26-30），标题 ≥40，行高标题 1.1-1.2、正文 1.4+
  - 最多 2 个字体族（Noto Sans SC 排中文，Inter 排数字与公式）
  - 2-3 个主色 + 中性色，强调色只用于强调

纸白底上的对比度（WCAG，实测见文件末尾）最低一对是 5.01:1，全部高于 4.5。
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import (Ids, color_vars, frame, rect, solid, stack, stroke, text,
                   write_doc)

ids = Ids()

# 米白纸 + 墨绿。底色刻意不是纯白：投影时纯白底会把整间教室照亮，米白既
# 压得住反光，也让白色卡面浮得起来（白卡 / 纸底亮度比 1.16，看得出层次）。
VARS = color_vars({
    "c-bg":          "#F2EEE2",
    "c-surface":     "#FFFFFF",
    "c-ink":         "#17211C",
    "c-muted":       "#55635A",
    "c-accent":      "#1B6B4C",
    "c-accent-soft": "#D8E9DE",
    "c-border":      "#D9D2C2",
    # 坐标网格线。**唯一带 alpha 的变量**：网格是文字的兄弟不是祖先，lint
    # 找底色只走祖先链碰不到它；而同一根线要同时压在米白纸底和白卡边缘上，
    # 只有半透明才两边都成立（实色必然在其中一边露馅）。
    "c-grid":        "#1B6B4C14",
})

W, H = 1920, 1080
EDGE = 120

CJK = "Noto Sans SC"
NUM = "Inter"

# 网格间距。120 = EDGE，所以第一条线正好落在内容的左基线上，网格与版心是
# 同一套尺度而不是两套。
GRID_STEP = 120


def paper_grid():
    """一张极淡的坐标纸。整套课件唯一的装饰语言。

    只画线不画点：点阵在投影仪上会摩尔纹，细线不会。线压在内容之下，所以
    卡片一盖就消失，看得见的永远只是留白里的那几格。
    """
    lines = []
    for index in range(1, (H + GRID_STEP - 1) // GRID_STEP):
        line = rect(ids, "网格横线", width=W, height=2, fill=solid("$c-grid"))
        line["x"], line["y"] = 0, index * GRID_STEP
        lines.append(line)
    for index in range(1, (W + GRID_STEP - 1) // GRID_STEP):
        line = rect(ids, "网格竖线", width=2, height=H, fill=solid("$c-grid"))
        line["x"], line["y"] = index * GRID_STEP, 0
        lines.append(line)
    return lines


def ghost_number(label, x, y, *, size=460):
    """页码做成背景上的巨大数字。

    「满色 + 图层透明度」而不是「把颜色调到接近底色」：换主色时它自己跟着
    走；而且 lint 量的是文字原色（它不看节点 opacity），不会留下一条假的
    低对比记录。
    """
    node = text(ids, "ghost 页码", label, size, 700, "$c-accent", family=NUM,
                width="fit_content", growth="auto", line_height=1.0,
                spacing=-round(size * 0.05))
    node["x"], node["y"] = x, y
    node["opacity"] = 0.06
    return node


def slide(name, *, gap=56, justify="start", fill="$c-bg", extra_decor=()):
    """一帧课件。固定 1920×1080 —— 投影比例是硬约束，不是建议。"""
    body = frame(ids, f"{name} · 正文", width="fill_container",
                 height="fill_container", layout="vertical",
                 padding=[EDGE, EDGE], gap=gap, justifyContent=justify,
                 alignItems="start", fill=[])
    body["children"] = []
    # ghost 数字排在网格之前 = 画在网格之上（index 0 最上），否则细线会从
    # 数字上横穿过去，像划掉了一样。
    shell = stack(ids, name, body, list(extra_decor) + paper_grid(),
                  width=W, height=H, fill=solid(fill))
    shell["content"] = body
    return shell


def col(name, children, *, gap=16, width="fill_container",
        height="fit_content", align="start", fill=None, **props):
    """纵向容器。`fill=None` 才是结构层；一旦有 fill 它就是一张卡面了。"""
    node = frame(ids, name, width=width, height=height, layout="vertical",
                 gap=gap, alignItems=align, fill=fill or [], **props)
    node["children"] = children
    return node


def row(name, children, *, gap=40, align="start", width="fill_container",
        height="fit_content", fill=None, **props):
    node = frame(ids, name, width=width, height=height, layout="horizontal",
                 gap=gap, alignItems=align, fill=fill or [], **props)
    node["children"] = children
    return node


# 左侧色条的宽度。10px 在 1920 宽的版面上刚好读得出是「一条标记」而不是
# 「卡片描边画歪了」。
RAIL = 10


def knowledge_card(children, *, gap=16, height="fit_content", padding=(36, 40),
                   width="fill_container"):
    """带左侧色条的知识点卡。

    色条是一个 fill_container 高的兄弟矩形，不是 per-side 描边：描边会跟着
    圆角走一圈，在 20px 圆角上那一段弧读起来像描边没画完。外层 clipContent
    把直角色条切进圆角里，得到的才是「书页边上贴的那条标签」。
    """
    inner = col("卡内容", children, gap=gap, padding=[padding[0], padding[1]])
    return row("知识点卡", [
        rect(ids, "知识点色条", width=RAIL, height="fill_container",
             fill=solid("$c-accent")),
        inner,
    ], gap=0, align="stretch", width=width, height=height, padding=[0, 0],
        cornerRadius=20, fill=solid("$c-surface"), clipContent=True)


def eyebrow(label):
    """讲次标签。整套课件唯一的胶囊，交代「这是第几讲」。"""
    node = frame(ids, "标签", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[10, 22], gap=0,
                 cornerRadius=999, alignItems="center",
                 justifyContent="center", fill=solid("$c-accent-soft"))
    node["children"] = [
        text(ids, "标签文字", label, 26, 600, "$c-accent", family=CJK,
             width="fit_content", growth="auto", line_height=1.4,
             spacing=1.0),
    ]
    return node


def accent_bar(width=150, height=10):
    return rect(ids, "强调条", width=width, height=height, cornerRadius=999,
                fill=solid("$c-accent"))


def numbered(no, size, *, filled=True):
    """序号圆。白字压在墨绿上是全页对比度最高的一处，是眼睛的落点。"""
    if filled:
        fill, ink, edge = solid("$c-accent"), "#FFFFFF", None
    else:
        fill, ink, edge = solid("$c-surface"), "$c-accent", stroke("$c-accent", 3)
    node = frame(ids, f"序号 {no}", width=size, height=size,
                 layout="horizontal", gap=0, alignItems="center",
                 justifyContent="center", cornerRadius=size // 2, fill=fill,
                 **({"stroke": edge} if edge else {}))
    node["children"] = [
        text(ids, "序号数字", str(no), round(size * 0.46), 700, ink,
             family=NUM, width="fit_content", growth="auto", line_height=1.0),
    ]
    return node


def centered_body(name, child):
    """把内容块撑到页标题以下的整块空间里再垂直居中。

    页标题每页固定在同一高度（重复）是课件读起来整齐的前提；正文块又不该
    被拉长到失真。两个诉求靠这一层透明容器同时满足。
    """
    return col(name, [child], gap=0, height="fill_container",
               justifyContent="center")


def page_head(title, subtitle=None, *, size=76):
    kids = [text(ids, "页标题", title, size, 700, "$c-ink", family=CJK,
                 line_height=1.12, spacing=-2)]
    if subtitle:
        kids.append(text(ids, "页副标题", subtitle, 30, 400, "$c-muted",
                         family=CJK, line_height=1.45))
    return col("页头", kids, gap=18)


# ------------------------------------------------------------------ 01 封面
def cover():
    lede = col("课程标题", [
        text(ids, "课名", "函数的单调性\n与极值判定", 116, 700, "$c-ink",
             family=CJK, line_height=1.08, spacing=-3),
        accent_bar(),
        text(ids, "课程说明", "这一讲我们只解决一个问题：拿到一个函数，怎么"
                             "判断它在哪段往上走。", 30, 400, "$c-muted",
             family=CJK, line_height=1.45),
    ], gap=36)

    meta = row("讲师信息", [
        text(ids, "讲师", "主讲 · 林望舒", 28, 500, "$c-ink", family=CJK,
             width="fit_content", growth="auto"),
        text(ids, "学期", "高等数学（上）· 2026 秋", 28, 400, "$c-muted",
             family=CJK, width="fit_content", growth="auto"),
    ], gap=32, align="center", justifyContent="space_between")

    s = slide("01 课程封面", gap=48, justify="space_between")
    s["content"]["children"] = [eyebrow("第 07 讲"), lede, meta]
    return s


# ------------------------------------------------------------------ 02 目标
OBJECTIVES = [
    ("说得出定义", "用导数的符号说明单调递增、递减到底在讲什么。"),
    ("判得出区间", "给定一个可导函数，能划出它的单调区间。"),
    ("找得到极值", "会用一阶导数的符号变化确定极大值与极小值。"),
]


def objectives():
    items = []
    for index, (title, desc) in enumerate(OBJECTIVES, 1):
        items.append(knowledge_card([
            row(f"目标 {index}", [
                numbered(index, 64),
                col("目标文案", [
                    text(ids, "目标标题", title, 40, 600, "$c-ink",
                         family=CJK, line_height=1.25),
                    text(ids, "目标说明", desc, 28, 400, "$c-muted",
                         family=CJK, line_height=1.5),
                ], gap=12),
            ], gap=28, align="start"),
        ], gap=0))

    s = slide("02 学习目标", gap=56, extra_decor=[ghost_number("02", 1350, 40)])
    s["content"]["children"] = [
        page_head("这节课下课时，你应该能——",
                  "三条，都要能自己动手做出来，不是听懂就算。"),
        centered_body("目标区", col("目标列表", items, gap=24)),
    ]
    return s


# ------------------------------------------------------------------ 03 讲解
def concept():
    body = knowledge_card([
        col("定义组", [
            text(ids, "定义标题", "单调性的判定定理", 40, 600, "$c-ink",
                 family=CJK, line_height=1.25),
            text(ids, "定义正文",
                 "设函数 f(x) 在区间 (a, b) 内可导。若在该区间内恒有 "
                 "f'(x) > 0，则 f(x) 在 (a, b) 上单调递增；若恒有 "
                 "f'(x) < 0，则单调递减。",
                 30, 400, "$c-ink", family=CJK, line_height=1.6),
        ], gap=20),
        col("追问组", [
            rect(ids, "正文分隔线", width="fill_container", height=2,
                 fill=solid("$c-border")),
            text(ids, "提示标题", "为什么成立", 30, 600, "$c-accent",
                 family=CJK, line_height=1.3),
            text(ids, "提示正文",
                 "导数是切线的斜率。斜率一直为正，意味着曲线上任意两点连线"
                 "都在往右上方走——这正是「递增」的几何说法。",
                 28, 400, "$c-muted", family=CJK, line_height=1.6),
        ], gap=20),
    ], gap=40, height="fill_container", padding=(44, 44))

    # 配图位用 frame 而不是 rectangle：frame 会递归渲染子节点，里面那两行
    # 提示才显示得出来；frame 同时是合法的图片拖放目标，把函数图像拖上去
    # 就直接替换掉这个占位。
    figure = col("配图占位", [
        text(ids, "配图提示", "把函数图像拖到这里", 30, 600, "$c-accent",
             family=CJK, width="fit_content", growth="auto",
             line_height=1.3),
        text(ids, "配图说明", "建议画出 f(x) 与 f'(x) 的符号对照",
             26, 400, "$c-muted", family=CJK, width="fit_content",
             growth="auto", line_height=1.45),
    ], gap=12, width=700, height="fill_container", padding=[0, 48],
        align="center", justifyContent="center", cornerRadius=20,
        fill=solid("$c-accent-soft"))

    s = slide("03 概念讲解", gap=56, extra_decor=[ghost_number("03", 1350, 40)])
    s["content"]["children"] = [
        page_head("导数的符号，决定曲线的走向"),
        centered_body("图文区", row("图文两栏", [body, figure], gap=48,
                                 align="stretch", height=560)),
    ]
    return s


# ------------------------------------------------------------------ 04 例题
STEPS = [
    ("求导", "f'(x) = 3x² − 6x = 3x(x − 2)"),
    ("找驻点", "令 f'(x) = 0，得 x = 0 与 x = 2"),
    ("判符号", "x < 0 与 x > 2 时 f'(x) > 0；0 < x < 2 时 f'(x) < 0"),
    ("下结论", "x = 0 处取极大值 f(0) = 1，x = 2 处取极小值 f(2) = −3"),
]


def worked_example():
    prompt = col("题干", [
        text(ids, "题干标签", "例题", 26, 600, "$c-accent", family=CJK,
             width="fit_content", growth="auto", line_height=1.3,
             spacing=1.0),
        text(ids, "题干正文",
             "求函数 f(x) = x³ − 3x² + 1 的单调区间与极值。",
             34, 600, "$c-ink", family=CJK, line_height=1.35),
    ], gap=12, padding=[32, 36], cornerRadius=16, fill=solid("$c-accent-soft"))

    steps = []
    for index, (title, formula) in enumerate(STEPS, 1):
        steps.append(row(f"步骤 {index}", [
            numbered(index, 56),
            col("步骤文案", [
                text(ids, "步骤标题", title, 30, 600, "$c-ink", family=CJK,
                     line_height=1.3),
                text(ids, "步骤算式", formula, 28, 400, "$c-muted",
                     family=NUM, line_height=1.5),
            ], gap=8),
        ], gap=24, align="start"))

    # 题干与解答装进同一张卡：例题在课件里是一个整体，拆成两块平级容器既
    # 削弱了「这是一道题」的读法，也让页面上出现三个圆角不一致的兄弟
    # （mixed-sibling-corner-radius 会照实报出来）。
    exam = knowledge_card([
        prompt,
        col("解答步骤", steps, gap=24),
    ], gap=32, padding=(40, 44))

    s = slide("04 例题", gap=56, extra_decor=[ghost_number("04", 1350, 40)])
    s["content"]["children"] = [
        page_head("按这四步走，一步都不要跳"),
        centered_body("例题区", exam),
    ]
    return s


# ------------------------------------------------------------------ 05 对比
COMPARISON = [
    ("看的是什么", "f'(x) 在整个区间上的符号", "f'(x) 在某一点两侧的符号变化"),
    ("回答的问题", "这段曲线往上还是往下", "这一点是不是拐上去/拐下来的地方"),
    ("容易错在哪", "漏掉不可导的点也要单独讨论",
     "驻点不一定是极值点，要看符号是否真的变了"),
]

# 表格分隔线用 PenStroke 的 per-side 厚度：只画底边，四条边全画会把表格
# 变成田字格，行与行的关系反而看不出来。最后一行不画，免得表格底部多出
# 一条没有下文的线。
def table_row(label, left, right, *, last=False):
    cells = [
        col("行标题", [
            text(ids, "行标题文字", label, 27, 600, "$c-muted", family=CJK,
                 line_height=1.4),
        ], gap=0, width=260),
        col("左列内容", [
            text(ids, "左列文字", left, 28, 400, "$c-ink", family=CJK,
                 line_height=1.5),
        ], gap=0),
        col("右列内容", [
            text(ids, "右列文字", right, 28, 400, "$c-ink", family=CJK,
                 line_height=1.5),
        ], gap=0),
    ]
    props = {"padding": [26, 32]}
    if not last:
        props["stroke"] = {"thickness": {"bottom": 2},
                           "fill": solid("$c-border")}
    return row("对比行", cells, gap=40, align="start", **props)


def comparison():
    header = row("表头", [
        col("表头行标题", [
            text(ids, "表头留白", " ", 27, 600, "$c-muted", family=CJK,
                 line_height=1.4),
        ], gap=0, width=260),
        col("表头左列", [
            text(ids, "表头左列文字", "单调性", 34, 700, "$c-accent",
                 family=CJK, line_height=1.3),
        ], gap=0),
        col("表头右列", [
            text(ids, "表头右列文字", "极值", 34, 700, "$c-accent",
                 family=CJK, line_height=1.3),
        ], gap=0),
    ], gap=40, align="start", padding=[26, 32],
        stroke={"thickness": {"bottom": 3}, "fill": solid("$c-accent")})

    # 对比表刻意不带左侧色条：色条在这套里是「知识点」的标记，一张对照表
    # 不是知识点。装饰语言一旦到处贴，就不再是语言了。
    table = col("对比表", [header] + [
        table_row(*entry, last=index == len(COMPARISON) - 1)
        for index, entry in enumerate(COMPARISON)
    ], gap=0, cornerRadius=20, fill=solid("$c-surface"), clipContent=True)

    s = slide("05 对比", gap=56, extra_decor=[ghost_number("05", 1350, 40)])
    s["content"]["children"] = [
        page_head("两个概念，别再混着用",
                  "单调性说的是一段，极值说的是一点。"),
        centered_body("对比区", table),
    ]
    return s


# ------------------------------------------------------------------ 06 小结
TAKEAWAYS = [
    "先求导，再找驻点，最后判符号——顺序不能换。",
    "驻点只是候选，符号真的变了才是极值点。",
    "不可导的点也要单独拿出来讨论。",
]

HOMEWORK = [
    ("课本 P128", "习题 3.4 第 1、3、5 题"),
    ("补充题", "讨论 f(x) = x·e^(−x) 的单调区间与极值"),
    ("下次课前", "预习凹凸性与拐点（P131–P136）"),
]


def bullet_dot(size=14, line_size=28, line_height=1.55):
    """项目符号。包一层只有上内距的容器，把圆心压到首行的光学中线上。

    `align="start"` 会把圆点顶边对齐到文本盒顶边，而文本首行的中线在
    `fontSize * lineHeight / 2` 处 —— 直接并排会让每个圆点浮在字的上方。
    """
    lift = round(line_size * line_height / 2 - size / 2)
    return col("条目圆点位", [
        rect(ids, "条目圆点", width=size, height=size,
             cornerRadius=size // 2, fill=solid("$c-accent")),
    ], gap=0, width="fit_content", padding=[lift, 0, 0, 0])


def summary():
    left = knowledge_card([
        text(ids, "小结标题", "本讲小结", 44, 700, "$c-ink", family=CJK,
             line_height=1.2),
        col("小结条目", [
            row("小结条", [bullet_dot(),
                text(ids, "条目文字", line, 28, 400, "$c-ink", family=CJK,
                     line_height=1.55)],
                gap=20, align="start")
            for line in TAKEAWAYS
        ], gap=20),
    ], gap=28, height="fill_container", padding=(44, 44))

    right = col("课后作业", [
        text(ids, "作业标题", "课后作业", 44, 700, "$c-ink", family=CJK,
             line_height=1.2),
        col("作业条目", [
            col("作业条", [
                text(ids, "作业出处", source, 26, 600, "$c-accent",
                     family=CJK, line_height=1.3),
                text(ids, "作业内容", detail, 28, 400, "$c-muted",
                     family=CJK, line_height=1.5),
            ], gap=8)
            for source, detail in HOMEWORK
        ], gap=24),
    ], gap=28, width=680, height="fill_container", padding=[44, 44],
        cornerRadius=20, fill=solid("$c-accent-soft"))

    s = slide("06 小结与作业", gap=56,
              extra_decor=[ghost_number("06", 1350, 40)])
    s["content"]["children"] = [
        page_head("带走这三句话就够了"),
        centered_body("小结区", row("小结两栏", [left, right], gap=48,
                                 align="stretch", height=490)),
    ]
    return s


# 顶层 frame 必须显式写 x/y：缺省时每帧都落在原点，六页会叠成一页，另外
# 五页藏在下面（2026-08-02 实录）。纵向 gap 比横向多留 240，是给画布上固定
# 在帧顶部的帧名标签让位——标签是屏幕空间的，不随缩放变小。
BOARD_GAP_X = 120
BOARD_GAP_Y = BOARD_GAP_X + 240
BOARDS_PER_ROW = 3


def build():
    boards = [cover(), objectives(), concept(), worked_example(),
              comparison(), summary()]
    for index, board in enumerate(boards):
        board.pop("content", None)
        board["x"] = (index % BOARDS_PER_ROW) * (W + BOARD_GAP_X)
        board["y"] = (index // BOARDS_PER_ROW) * (H + BOARD_GAP_Y)
    return boards


# 对比度（WCAG 相对亮度比，投影场景自设门槛 4.5，远高于 op-design-lint 的 2.5）：
#   c-ink   on c-bg      14.25    c-ink   on c-surface     16.53
#   c-muted on c-bg       5.46    c-muted on c-surface      6.33
#   c-accent on c-bg      5.56    c-accent on c-surface     6.45
#   c-accent on c-accent-soft  5.11    c-muted on c-accent-soft  5.01  ← 最低
#   #FFFFFF on c-accent   6.45（序号圆里的白字）
# 换主色时先量 c-muted on c-accent-soft：浅色 deck 的下限总是出在
# 「弱化文字压在弱化底色上」这一对，不是出在标题上。
# 网格线与 ghost 页码不参与：前者是 8% 的装饰兄弟，后者是「满色 + 6% 图层
# 透明度」，两者都不承担阅读。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "课件 deck · 浅色 16:9")
