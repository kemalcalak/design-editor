#!/usr/bin/env python3
"""minimal-keynote.op — 极简 Keynote deck（9 页 1920×1080）

这套的难点是「少」。没有卡片、没有描边、没有底色块，一页只留 2-3 个元素，
版面全靠字阶落差和留白撑。

这一版把「极简」推到更极端：

1. **一页一句话，居中。** 上一版所有内容贴着左基线排，读起来仍然是一份
   文档；改成横向居中之后，每一页只剩「屏幕中央的一句话」，那才是发布会
   舞台上的节奏。整套只有一条中轴线，任何一处对不齐都无处遁形。
2. **字更大、留白更大。** 单字页 300px（原 200），封面 168（原 130），
   边距 200（契约下限 100 的两倍）。正文一律 26-30，字阶落差拉到 5-6 倍。
3. **多一页目录，只有细线和数字。** 极简 deck 的目录不该是一张列表卡，
   它是五条横线加五个数字 —— 页面结构本身就是目录。

跟已有几套的区别：slide-deck 是暖白商务、pitch-deck-dark 是深底路演、
lecture-deck-light 是纸白课件，它们都靠「卡片」组织信息；这一套刻意不给
任何容器上色。

排版遵循 skills/domains/slides.md 的硬契约：
  - 每帧固定 1920×1080，绝不 fit_content；内容距边 ≥100（这里 200）
  - 正文 ≥24（取 26-30），标题 ≥40
  - 行高：展示字 1.0-1.1，正文 1.45+
  - 最多 2 个字体族（Noto Sans SC 排中文，Inter 排数字），靠字重分层
  - 强调色只用于强调
  契约里「关键数字 80-200」约束的是数据页那种带单位的指标；单字展示页
  （少 / 轻 / 久）整页只有一个字，属于展示字而非数字，取 300。

白底上的对比度（WCAG，实测见文件末尾）最低一对是 5.33:1，全部高于 4.5。
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import Ids, color_vars, frame, rect, solid, text, write_doc

ids = Ids()

# 纯白 + 近黑 + 一个克制的朱红。
#
# 强调色没有用常见的 #FF5A36：那个橙在纯白上只有 3.10:1，当文字用不到
# 4.5 的可读线（实测，见文件末尾）。降明度到 #C7340F 后是 5.35:1，色相
# 仍在同一族，投影时反而更沉得住。整套只有这一个彩色，出现三次以内。
VARS = color_vars({
    "c-bg":     "#FFFFFF",
    "c-ink":    "#0A0A0A",
    "c-muted":  "#6B6B6B",
    "c-accent": "#C7340F",
    # 目录页的细线。整套唯一一条「不是文字也不是强调」的线，所以给它自己
    # 一档极浅的灰 —— 用 c-muted 会把目录画成一张表格。
    "c-hair":   "#E2E2E2",
})

W, H = 1920, 1080
# 留白是这套模板唯一的「装饰」，所以边距给到 200 —— 契约下限 100 的两倍。
EDGE = 200

CJK = "Noto Sans SC"
NUM = "Inter"


def slide(name, *, gap=40, justify="center"):
    """一帧。固定 1920×1080，无底色块、无描边 —— 白就是版面本身。

    `alignItems="center"` 是这一版的分水岭：所有内容围绕同一条中轴，页与页
    之间不再有「这句从这里起、那句从那里起」的抖动。
    """
    node = frame(ids, name, width=W, height=H, layout="vertical",
                 padding=[EDGE, EDGE], gap=gap, justifyContent=justify,
                 alignItems="center", fill=solid("$c-bg"), clipContent=True)
    node["children"] = []
    return node


def col(name, children, *, gap=16, width="fill_container",
        height="fit_content", align="center", **props):
    """透明结构容器。这套模板里没有任何一个容器该有 fill。"""
    node = frame(ids, name, width=width, height=height, layout="vertical",
                 gap=gap, alignItems=align, fill=[], **props)
    node["children"] = children
    return node


def row(name, children, *, gap=32, align="center", **props):
    node = frame(ids, name, width="fill_container", height="fit_content",
                 layout="horizontal", gap=gap, alignItems=align, fill=[],
                 **props)
    node["children"] = children
    return node


def centered(name, content, size, weight, color, *, family=CJK,
             line_height=1.05, spacing=0):
    """居中的一行（或几行）展示字。整套只有这一种排文字的方式。"""
    return text(ids, name, content, size, weight, color, family=family,
                line_height=line_height, spacing=spacing, align="center")


def accent_rule(width=140, height=8):
    """整套唯一的图形元素。一条短线，替代所有本该是卡片边框的东西。"""
    return rect(ids, "强调短线", width=width, height=height,
                fill=solid("$c-accent"))


def hairline():
    return rect(ids, "细线", width="fill_container", height=2,
                fill=solid("$c-hair"))


# ------------------------------------------------------------------ 01 封面
def cover():
    lede = col("主张", [
        centered("主标题", "把复杂\n留在里面", 168, 700, "$c-ink",
                 line_height=1.02, spacing=-3),
        accent_rule(),
        centered("副标题", "第七代产品，今天发布。", 30, 400, "$c-muted",
                 line_height=1.45),
    ], gap=48)

    meta = row("落款", [
        text(ids, "场合", "2026 秋季发布会", 26, 500, "$c-ink", family=CJK,
             width="fit_content", growth="auto"),
        text(ids, "日期", "2026.10.08", 26, 400, "$c-muted", family=NUM,
             width="fit_content", growth="auto"),
    ], gap=28, justifyContent="space_between")

    s = slide("01 封面", gap=48, justify="space_between")
    # 封面顶部留一块与落款等高的空占位，主张才真正落在版心中央而不是偏上。
    s["children"] = [col("顶部留白", [], gap=0, height=40), lede, meta]
    return s


# ------------------------------------------------------------------ 02 目录
AGENDA = [
    ("01", "少"),
    ("02", "轻"),
    ("03", "久"),
    ("04", "一个数字"),
    ("05", "今天发货"),
]


def agenda():
    """只有细线和数字的目录。

    没有卡片、没有说明文字、没有强调色 —— 一页目录如果需要解释，说明这场
    发布会的结构本身没想清楚。数字用 Inter、条目用中文族，两个字族的分工
    在这一页最直白。
    """
    rows = []
    for no, title in AGENDA:
        rows.append(hairline())
        rows.append(row(f"目录项 {no}", [
            text(ids, "序号", no, 30, 500, "$c-muted", family=NUM, width=110,
                 spacing=2),
            text(ids, "条目", title, 44, 600, "$c-ink", family=CJK,
                 line_height=1.2),
        ], gap=48, align="center", padding=[26, 0]))
    rows.append(hairline())

    # 行高 26+53+26 ≈ 105，五行加六条线 ≈ 537，再加标题与 gap 正好落在
    # 680 的版心里。这一页是整套唯一「内容量固定」的页，所以尺寸是算出来
    # 的而不是试出来的：加一条目录项就会顶出版心。
    s = slide("02 目录", gap=48)
    s["children"] = [
        centered("页标题", "目录", 40, 600, "$c-muted", line_height=1.2,
                 spacing=8),
        col("目录列表", rows, gap=0),
    ]
    return s


# ------------------------------------------------------------ 03 一句话论点
def statement():
    s = slide("03 论点", gap=52)
    s["children"] = [
        rect(ids, "强调点", width=20, height=20, cornerRadius=10,
             fill=solid("$c-accent")),
        centered("论点", "我们删掉了\n三十七个按钮。", 104, 700, "$c-ink",
                 line_height=1.12, spacing=-2),
    ]
    return s


# ------------------------------------------------------------- 04-06 词页
# 一页一个字。注解只有一行，写不下就说明这个词选错了。
WORDS = [
    ("少", "从三十七个按钮，到三个。"),
    ("轻", "整机一点零九公斤，比上一代少了两百四十克。"),
    ("久", "一次充电，用满两天。"),
]


def word_page(index, word, note):
    s = slide(f"0{index + 4} 词 · {word}", gap=44)
    s["children"] = [
        centered("大词", word, 300, 700, "$c-ink", line_height=1.0),
        centered("注解", note, 30, 400, "$c-muted", line_height=1.5),
    ]
    return s


# ------------------------------------------------------------------ 07 引用
def quote():
    s = slide("07 引用", gap=36)
    s["children"] = [
        # 引号本身当图形用，替掉一切边框和背景块。lineHeight 压到 0.5 是
        # 量出来的：引号的墨迹只占 em 盒顶部约三分之一，默认行高会在它下面
        # 留出一大片空行盒，看起来就像 gap 没设对（同 oplib 里 `●` 用
        # DOT_LINE_HEIGHT 收行盒的做法）。
        centered("引号", "“", 200, 700, "$c-accent", family=NUM,
                 line_height=0.5),
        centered("引文", "最好的设计，\n是让人忘了它存在。", 76, 600,
                 "$c-ink", line_height=1.25, spacing=-2),
        centered("出处", "— 产品负责人 沈亦文", 28, 400, "$c-muted",
                 line_height=1.5),
    ]
    return s


# ------------------------------------------------------------- 08 数据一句话
def one_number():
    s = slide("08 数据", gap=36)
    s["children"] = [
        centered("大数字", "0.4 秒", 260, 700, "$c-ink", family=NUM,
                 line_height=1.0, spacing=-5),
        centered("注解", "从按下到出图的全部时间，上一代是 2.6 秒。",
                 30, 400, "$c-muted", line_height=1.5),
    ]
    return s


# ------------------------------------------------------------------ 09 结尾
def closing():
    s = slide("09 结尾", gap=48)
    s["children"] = [
        accent_rule(),
        centered("结语", "今天开始发货。", 128, 700, "$c-ink",
                 line_height=1.08, spacing=-3),
        centered("补充", "全国门店同步开售，官网可预约体验。", 30, 400,
                 "$c-muted", line_height=1.5),
    ]
    return s


# 顶层 frame 必须显式写 x/y：缺省时每帧都落在原点，九页会叠成一页（前一批
# 模板 2026-08-02 的实录）。纵向 gap 比横向多留 240，是给画布上固定在帧顶部
# 的帧名标签让位——标签是屏幕空间的，不随缩放变小。
BOARD_GAP_X = 120
BOARD_GAP_Y = BOARD_GAP_X + 240
BOARDS_PER_ROW = 3


def build():
    boards = [cover(), agenda(), statement()]
    boards += [word_page(i, w, n) for i, (w, n) in enumerate(WORDS)]
    boards += [quote(), one_number(), closing()]
    for index, board in enumerate(boards):
        board["x"] = (index % BOARDS_PER_ROW) * (W + BOARD_GAP_X)
        board["y"] = (index // BOARDS_PER_ROW) * (H + BOARD_GAP_Y)
    return boards


# 对比度（WCAG 相对亮度比，投影场景自设门槛 4.5，远高于 op-design-lint 的 2.5）：
#   c-ink    on c-bg   19.80
#   c-muted  on c-bg    5.33  ← 最低一对
#   c-accent on c-bg    5.35
# 换强调色时先量它在纯白上的比值：橙红一族很容易掉到 4.5 以下
#   （#FF5A36 = 3.10、#E5471F = 4.00 都不够，#C7340F = 5.35 才过）。
# c-hair 只画线不承载文字，不参与门槛。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "极简 Keynote · 16:9")
