#!/usr/bin/env python3
"""data-story-infographic.op — 数据故事长图（1080×N 竖版）

信息图这一档的第八张，回答**「这几个数字连起来说明了什么」** —— 四个小
数据串成一条因果线，最后收到一句能直接改做法的结论。

### 最近邻差异（为什么它不是 data-report 换个色）

两张都摆数字，分界线是**数字之间的关系**，而这决定了三处结构：

  - **data-report 是并列，这张是串联。** 前者的三个大数、五条对比、一根
    占比条互不依赖，删掉任何一块其余照样成立；这张的第二段以第一段为分
    母（「点进来的人里……」），顺序换了句子就不通。所以每段末尾都有一句
    「所以」，它是**连接词**，不是点评。
  - **可视化语言不同。** data-report 用横条（比大小）和堆叠条（看构成）；
    这张一律用**十格方块阵**：每格固定代表 10%，读者数格子就行，不用回头
    对轴。同一份数据用两种图形讲，是为了不让两张图长得像。
  - **一冷一暖不够分，明度也得分。** data-report 是浅底青绿，这张是墨底青
    柠 —— 深浅相反、色相隔 90°。而且这张**整页没有页头带**：深底上的带子
    只有比正文更亮才看得见，一亮就和卡片撞成同一个面，所以页头的身份改由
    一条四段横杠来给（见 `chapter_rail`）。
  - **结论的位置不同。** data-report 把三条结论收在末尾的面板里；这张每一
    段都自带结论，末尾那句只是把四句合成一句。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：数据故事的读法是「在暗处顺着一条线往下走」。墨蓝底给「暗
    处」，青柠给「线」—— 它是这一档里唯一的高明度冷绿，和青绿隔得开。
  - **收敛**：一个强调色 + 一条冷墨明度序列。青柠只出现在三处：大数、方块
    阵里被填满的格子、区块短线。
  - **论证**：方块阵天生需要两种颜色（填满 / 没填满），这正是深底的用处
    —— 未填满的格子用 $c-track（比卡片亮两档的冷灰蓝），它必须**看得见但
    不发亮**，否则十格全在抢注意力，比例就读不出来了。

### 负约束（本模板明令不做的事）

  - **不画折线、不画柱、不画饼、不画漏斗。** 全图只有方块阵一种图形语言。
    漏斗尤其要拒绝：它会把「上一段的分母」画成面积，而这四段的分母各不相
    同，面积会撒谎。
  - 不画坐标轴、网格线、图例。每格固定 10%，数字就写在格子上方。
  - **不用霓虹发光、不用扫描线、不用发光描边。** 深底 + 亮绿是「赛博风」
    的高发区，只靠实色块和留白撑住。
  - 不用第二个有彩色。青柠之外只有冷墨明度序列。
  - 一段一个数，一个数一句「所以」；写不下就减一段，不缩字号。
  - 不写 AI 套话（「数据驱动 / 增长飞轮 / 用户心智」），每句「所以」都要
    能直接改成下一次的做法。

硬契约：
  - 内容距边缘 ≥80px（这里 80）
  - 配色全部走 color_vars，改主色只改 $c-accent 一处
  - 正文与背景对比度 ≥2.0（本配色最低一对见文件末尾注释）
  - **CJK 行高比西文全线高 0.2**：页头大标题 1.2，段落标题 1.3，正文 1.7
  - **CJK 负字距不超过 -0.02em**；只有西文数字沿用西文 display 的收紧
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
    "c-bg":          "#0B1220",
    "c-surface":     "#141E33",
    "c-ink":         "#E9F1FC",
    "c-muted":       "#93A3BE",
    "c-accent":      "#A8E04A",
    "c-accent-soft": "#1E2E14",
    # 方块阵里没被填满的那些格子。比卡片亮两档：它代表「剩下的人」，是信
    # 息不是背景 —— 第一版用了 #24314C（1.28:1），十格看起来像只有填满的
    # 那几格存在，比例就读不出来了。
    "c-track":       "#3D5079",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W = 1080
EDGE = 80
INNER = W - EDGE * 2

LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.3, 1.7

# 方块阵：十格，每格固定代表 10%。格宽是算出来的，不是拍的 —— 卡片左右各
# 34 内边距，十格九隙，隙宽 8。
CARD_PAD = 34
CELL_N = 10
CELL_GAP = 8
CELL_W = (INNER - CARD_PAD * 2 - CELL_GAP * (CELL_N - 1)) // CELL_N
CELL_H = 26

# 量出来的根高（做法同同档另外七张：根设 fit_content 渲一次读 PNG 高度）。
ROOT_H = 2711

# (段号, 这一段的分母, 数值, 填满几格, 陈述, 所以)。填格数 × 10% 必须等于
# 数值 —— 没有任何一层会替你检查这两者是否一致，写的时候自己对一遍。
STEPS = [
    ("01", "点进来的人里", "30%", 3,
     "只有三成读完了第一屏，其余在前三行就离开。",
     "第一屏决定了七成人走不走 —— 把结论放进前三行。"),
    ("02", "读完第一屏的人里", "70%", 7,
     "七成会一路滑到底，中途几乎不流失。",
     "中段不用再加钩子，把力气留给开头。"),
    ("03", "滑到底的人里", "20%", 2,
     "两成会点关注，其余看完就走。",
     "关注发生在读完之后 —— 引导语放结尾比放开头管用。"),
    ("04", "关注的人里", "10%", 1,
     "一周内只有一成会主动再回来一次。",
     "复访靠的是更新节奏，说清楚下次什么时候更。"),
]

CONCLUSION = ("把力气放在第一屏和更新节奏上。",
              "中间那一段自己会走完，四个数字里有两个都在说这件事。")


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


def waffle(filled):
    """十格方块阵。每格固定 10%，所以「填几格」就是「百分之几十」。

    不用横条：横条的长度要和轴比才有意义，而这四段的分母各不相同，摆在一
    起会被当成同一根轴上的四个值。格子是可数的，天然带自己的分母。
    """
    cells = []
    for index in range(CELL_N):
        cells.append(rect(ids, f"格 {index + 1}", width=CELL_W, height=CELL_H,
                          cornerRadius=6,
                          fill=solid("$c-accent" if index < filled
                                     else "$c-track")))
    return row("方块阵", cells, gap=CELL_GAP)


# ------------------------------------------------------------------ 01 页头
def chapter_rail():
    """四段章节条。它替代了同档另外七张都有的那块「深色页头带」。

    深底长图上，页头带只能靠**比正文更亮**才看得见（更暗那一版实测与页面
    只差 1.06:1，等于没画）；而更亮的一块又会和卡片撞成同一个面。所以这张
    干脆不要带：整页是一片连续的暗场，页头的身份由这条四段横杠给。
    它刻意做成四格且全部填满 —— 与十格方块阵的形状、格数、高度都不同，不
    会被误读成「四成」。
    """
    seg_w = (INNER - 10 * 3) // 4
    segs = [rect(ids, f"章节 {i + 1}", width=seg_w, height=8,
                 cornerRadius=999, fill=solid("$c-accent"))
            for i in range(4)]
    return row("章节条", segs, gap=10)


def header():
    return band("01 页头", fill=[], pad=[76, EDGE, 0, EDGE], gap=26,
                children=[
        chip("数据故事 · 四段一条线", bg="$c-accent", fg="$c-bg"),
        text(ids, "主标题", "四个数字\n串成一条线", 76, 700, "$c-ink",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.4),
        text(ids, "副标题", "每一段的分母都是上一段的结果，顺序不能换。",
             28, 400, "$c-muted", family=CJK, line_height=LH_BODY),
        chapter_rail(),
    ])


# ------------------------------------------------------------------ 02 故事
def step_card(index, base, value, filled, claim, so):
    head = row("段头", [
        text(ids, "段号", index, 24, 700, "$c-accent", family=NUM, width=44,
             line_height=1.4),
        text(ids, "分母", base, 26, 500, "$c-muted", family=CJK,
             line_height=1.4),
    ], gap=10)

    figure = row("数值行", [
        text(ids, "数值", value, 72, 700, "$c-accent", family=NUM,
             width=180, line_height=1.0, spacing=-3),
        text(ids, "陈述", claim, 27, 400, "$c-ink", family=CJK,
             line_height=LH_BODY),
    ], gap=20, align="center")

    so_row = row("所以", [
        icon_font(ids, "箭头", "corner-down-right", 26, "$c-accent"),
        text(ids, "所以文字", so, 27, 600, "$c-ink", family=CJK,
             line_height=LH_BODY),
    ], gap=14, align="start")

    card = col("故事段", [
        head,
        figure,
        waffle(filled),
        rect(ids, "段内分割线", width="fill_container", height=2,
             fill=solid("$c-track")),
        so_row,
    ], gap=20, padding=[30, CARD_PAD], cornerRadius=22)
    card["fill"] = solid("$c-surface")
    return card


def story():
    cards = [step_card(*entry) for entry in STEPS]
    return band("02 故事", fill=[], pad=[68, EDGE, 0, EDGE], gap=32,
                children=[
        section_head("顺着往下读",
                     "每格代表一成。数格子就够了，不用回头对刻度。"),
        col("故事列表", cards, gap=18),
    ])


# ------------------------------------------------------------------ 03 结论
def conclusion():
    panel = col("结论面板", [
        text(ids, "结论", CONCLUSION[0], 44, 700, "$c-accent", family=CJK,
             line_height=LH_HEAD, spacing=-0.8),
        text(ids, "结论支撑", CONCLUSION[1], 28, 400, "$c-ink", family=CJK,
             line_height=LH_BODY),
    ], gap=16, padding=[40, 36], cornerRadius=24)
    panel["fill"] = solid("$c-accent-soft")
    return band("03 结论", fill=[], pad=[68, EDGE, 68, EDGE], gap=32,
                children=[
        section_head("四句合成一句",
                     "上面每段末尾那句「所以」，合起来只剩这一条要做的事。"),
        panel,
    ])


# ------------------------------------------------------------------ 04 页脚
def footer():
    return band("04 页脚", fill=[], pad=[0, EDGE, 44, EDGE], gap=20,
                children=[
        rect(ids, "页脚上线", width="fill_container", height=2,
             fill=solid("$c-track")),
        text(ids, "口径",
             "统计区间：近 90 天，同一人多次访问按首次计，四舍五入到一成。",
             24, 400, "$c-muted", family=CJK, line_height=1.6),
        row("署名行", [
            text(ids, "账号名", "@ 你的账号名", 26, 600, "$c-ink",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
            text(ids, "更新说明", "每季度重跑一次这条线", 24, 400, "$c-muted",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
        ], gap=16, width="fill_container"),
    ])


def build():
    page = frame(ids, "数据故事长图", width=W, height=ROOT_H,
                 layout="vertical", gap=0, fill=solid("$c-bg"),
                 clipContent=True)
    page["children"] = [header(), story(), conclusion(), footer()]
    page["x"], page["y"] = 0, 0
    return [page]


# 对比度（WCAG 相对亮度比，op-design-lint 的门槛是 2.0；数值实测）：
#   c-ink    on c-bg      16.45   c-muted on c-bg          7.33
#   c-ink    on c-surface 14.61   c-muted on c-surface     6.51
#   c-accent on c-bg      11.97   c-accent on c-surface   10.63
#   c-bg     on c-accent  11.97   c-accent on c-accent-soft 9.22
#   c-ink    on c-soft    12.67   c-muted on c-accent-soft  5.65
#   c-track  on c-surface  2.08   c-track on c-bg           2.34
# 承载文字的最低一对是 5.65。c-track 是方块阵里没填满的格子与段内分割线，
# 它是「剩下的部分」这条信息本身，按信息图形量（2.08 / 2.34）—— 再亮就会
# 和填满的格子抢，比例反而读不出来。卡片不描边也不加阴影：$c-surface 与
# $c-bg 只差 1.13，卡的边界靠 18px 的行间隙与圆角给，不靠对比度。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "数据故事长图")
