#!/usr/bin/env python3
"""ledger-tick-deck.op — 账簿勾格 · 竞品矩阵档（1920×1080 · 7 页）

deck 体系 D12。落格 medium-high formality × medium-high density × light。
定位：把「我们和别人各自能做什么」摊成一张能逐格核对的账 —— 三态记号、
自家列高亮、合计行给结论。

### 锚点与色彩推导（采样 → 收敛 → 论证）

  - **采样**：手工账簿。淡黄账页、蓝红双色栏线（蓝画格、红画表头与合计）、
    逐格勾画的记号、合计行的双线。锚的是**「逐格核对」这个动作本身**。
  - **收敛**：底 = 账页 oklch(0.960 0.016 92)；线色两支（蓝 H245 / 红
    H25）**只作线不作文字**；记号三态用绿 + 中灰 + 浅灰。
  - **论证一**：**「不满足」用留空的浅灰短横，不画叉**。红叉是对竞品的价
    值判断，会让整份材料读作攻击性推销而不是评估；留空是事实陈述。这条同
    时规避了「红绿对立评分」这个已知的可信度杀手。
  - **论证二**：**只有自家列被着色，竞品列全部走中性**。着色即立场，立场
    只表达一次 —— 整份 deck 里 `own.wash` 只出现在主矩阵的那一列。
  - **论证三**：蓝红两支线色的对比只有 3.13 级，这是刻意的。它们是**线不
    是字**，对比一旦拉高就会与记号争夺注意力；几何质检对非文字装饰线
    （rectangle 且宽或高 ≤3px）按节点类型豁免，不按色值白名单豁免。

### 招牌母题

  1. **三态记号 tri-tick** —— 满足 = 一个 26px 的绿勾（两段折线，2.5px
     描边，非圆点）；部分 = 一个 24×24 的中灰**半格**（正方形左半填充）；
     不满足 = 一条 20×2px 的浅灰短横。**三态在形状上就可分辨，不依赖颜
     色** —— 这是无障碍地板，也是与「红绿打分表」的分野。
  2. **自家列 own-column** —— 整列 `own.wash` 底 + 2px 绿色外框（框从表头
     顶到合计线），列头一枚绿色实心 chip。
  3. **合计双线与结论行 tally-rule** —— 表尾上方 1px + 2px 红色双线，其下
     一行不是数字合计而是**一句结论**。账簿的合计惯例被借来托住「所以
     呢」。

### 负约束（Strictly avoid，逐条对应 spec §2 D12）

  - 不画红叉、不用红绿对立评分；不满足用留空短横。
  - 不放竞品 logo、不放竞品截图、不放任何可被认作贬低的视觉。
  - 只有自家列着色；竞品列一律中性，且列宽、行高、字号与自家列完全相同。
  - 蓝线与红线永远不承载文字。
  - 不用圆角、阴影、渐变。
  - 三态记号形状可分辨，不做成「三个不同颜色的圆点」。
  - 每一格只放一个记号字形，不放文字说明 —— 需要说明就进 05 / 06 页。
  - 评估维度必须在 02 页给出可核实的定义；无定义的维度不进矩阵。
  - 矩阵行数上限 8、列数上限 5；超了拆页，绝不缩行高。
  - 结论行是句子不是分数。不做加权总分 —— 总分把所有取舍藏进一个数字里，
    正是矩阵要避免的。

### 工程约束（spec §6，逐条踩过的坑）

  - **字体走保底层**：只写 `Noto Sans SC` 与 `Inter`。首选字体（思源黑 /
    霞鹜新晰黑 / IBM Plex Mono）不在渲染字体包内，写了会静默回退到 Roboto
    并触发「量 A 字体画 B 字体」的裁切偏心。
  - **数值与记号列的列宽写死**：渲染栈没有 tabular 数字，靠 fit_content 让
    格子自己对齐必然抖。
  - **不用 line 节点、不用 dashPattern**：格线、双线、刻度全部是
    rectangle；勾是两段 rectangle + rotation（rotation 是**度**，绕节点中
    心）。
  - **格与格之间放 1px 真格线节点**，不靠相邻底色差分格：两个同色矩形贴在
    一起会渲成一整块；插进去的 rule 同时让相邻单元格不再是「相邻」，
    `SIBLING_JAM_GAP` 也就无从谈起。
  - **叠放层 children[0] 在最上**：自家列外框写在数组第一个，否则会被表体
    盖住。
  - **rectangle 不递归渲染子节点**：图例里的「记号 + 说明」一律用 frame /
    group，不用 rectangle 当容器。
  - **顶层一次性给出整棵 7 帧树**，每帧显式写 x/y（板位 2040 × 1440）。

字号（§3.4 硬地板：任意文本 ≥20、注释 ≥22、全 deck 最大 ≥60、最大 ≥ 2.5 ×
正文）：封面主标 88 / 1.10 / −2（= 3.14 × 正文）· 页标题 56 / 1.20 · 条目标
题 32 · 表头与行标签 28 · 正文 28 / 1.70 · 脚注与页码 24 / 1.5。

对比度实测见文件末尾；承载文字的最低一对是 4.14。
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import Ids, color_vars, frame, group, rect, solid, text, write_doc

ids = Ids()

VARS = color_vars({
    "c-paper":     "#F5F2E6",   # 页面底 · 账页
    "c-band":      "#EBE6D8",   # 斑马行
    "c-ink":       "#221F1B",   # 主文字        14.63 : paper
    "c-ink-soft":  "#5A5652",   # 次级文字       6.48
    "c-ink-faint": "#78746F",   # 脚注 / 页码    4.14
    "c-rule-blue": "#88A2B9",   # 账簿蓝格线（非文字）
    "c-rule-red":  "#BD7670",   # 表头线 / 合计双线（非文字）
    "c-tick":      "#2F7442",   # 记号：满足 / 自家框   5.06
    "c-tick-half": "#888682",   # 记号：部分满足（非文字）
    "c-tick-pale": "#C6C4C1",   # 记号：不满足（留空短横）
    "c-own":       "#DFF3E2",   # 自家列整列底  14.11（c-ink）
})

CJK = "Noto Sans SC"
NUM = "Inter"

W, H = 1920, 1080
PAGES = 7
DECK_LABEL = "客服工单系统 · 能力核对表"

PAD_TOP, PAD_BOTTOM, PAD_X = 88, 104, 96
INNER = W - PAD_X * 2                      # 1728

FS_COVER, FS_TITLE, FS_LEAD = 88, 56, 44
FS_ITEM, FS_BODY, FS_META = 32, 28, 24
LH_DISPLAY, LH_TITLE, LH_BODY, LH_META = 1.10, 1.20, 1.70, 1.50

# 矩阵。行标签列固定 360，其余四列等分；四条 1px 竖格线也要算进总宽，
# 否则表尾会溢出画幅（列宽没有自动分配，写错就是溢出）。
COL_LABEL, MATRIX_COLS = 360, 4
COL_DATA = (INNER - COL_LABEL - MATRIX_COLS) // MATRIX_COLS      # 341
ROW_H = 72
assert COL_LABEL + MATRIX_COLS * COL_DATA + MATRIX_COLS == INNER


# ------------------------------------------------------------------ 骨架
def col(name, children, *, gap=0, width="fill_container", fill=None, **props):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="vertical", gap=gap, fill=fill or [], **props)
    node["children"] = children
    return node


def row(name, children, *, gap=0, align="start", width="fill_container",
        fill=None, height="fit_content", **props):
    node = frame(ids, name, width=width, height=height, layout="horizontal",
                 gap=gap, alignItems=align, fill=fill or [], **props)
    node["children"] = children
    return node


def hrule(width, thickness=1, color="$c-rule-blue"):
    """一条横线。**只用 rectangle**：`line` 在 layout:none 下吃的是文档绝对
    坐标，会跑到画布的另一个角落去。"""
    return rect(ids, "格线", width=width, height=thickness, fill=solid(color))


def vrule(height=ROW_H, thickness=1, color="$c-rule-blue"):
    return rect(ids, "竖格线", width=thickness, height=height,
                fill=solid(color))


# 拉丁与数字在 CJK 字体里约占半个字身。取 0.6 而不是 0.5 是留一档余量：
# 这个系数只决定断言的松紧，宁可早报一行也不要漏一行。
LATIN_EM = 0.6


def line_em(line):
    """一行文字的宽度，单位是「字」。CJK 记 1，其余记 LATIN_EM。"""
    return sum(1.0 if ch > "⿿" else LATIN_EM for ch in line)


def fits(content, size, width):
    """产稿期的折行断言：改文案当场报错，不必跑完闸再回来猜哪一行折了。

    量的是**渲染真实宽度**（拉丁半宽），不是 cjkcheck 的 1em 保守模型 ——
    后者会把数字混排句判成折行，而它实际排得下。两者是互补的：这里守「不
    许发生自动折行」，闸守「万一折了会不会出孤字末行 / 行首标点」。

    断点一律写死在文案里，所以每一硬行都要自己排得下。
    """
    if not isinstance(width, (int, float)) or not isinstance(content, str):
        return
    if not content.strip():
        return
    per = width / size
    for line in content.split("\n"):
        if line_em(line) > per:
            raise ValueError(
                f"断行超容量：{line!r} 折合 {line_em(line):.1f} 字 > 每行 "
                f"{per:.1f} 字（宽 {width} · 字号 {size}）。按 §3.1 缩短文"
                f"案、换页型或拆页，不缩字号。")


def body(content, *, size=FS_BODY, weight=400, color="$c-ink", width=None,
         lh=LH_BODY, family=CJK, spacing=0, align=None, name="正文"):
    """一段文字。断点写死在文案里 —— 当前 wrap_text 对 CJK 是逐字符断行、
    没有标点避头尾，交给自动折行会在「。」前面断开。"""
    fits(content, size, width)
    return text(ids, name, content, size, weight, color, family=family,
                line_height=lh, width=width or "fill_container",
                growth="fixed-width", align=align, spacing=spacing)


def mono(content, *, size=FS_META, weight=500, color="$c-ink-faint",
         spacing=0, name="标注"):
    """数字与拉丁一律走西文族 —— 让中文字体去渲染数字会字宽不齐，列就抖
    了。"""
    return text(ids, name, content, size, weight, color, family=NUM,
                line_height=LH_META, width="fit_content", growth="auto",
                spacing=spacing)


def index_mark(number, *, color="$c-rule-red", width=46):
    """序号。**宽度写死** —— Inter 在当前栈里不保证等宽数字，`01` 比 `03`
    窄，跟着它排的标题就会一行一个起点（实测可见）。"""
    return text(ids, "序号", f"{number:02d}", FS_META, 500, color,
                family=NUM, line_height=LH_META, width=width,
                growth="fixed-width")


def footer(index):
    """每页同一位置的页脚。位置锁死（§3.3 的「页码与页标位置不许跨页
    变」），内容左右分置。"""
    return row("页脚", [
        text(ids, "册名", DECK_LABEL, FS_META, 400, "$c-ink-faint",
             family=CJK, width="fit_content", growth="auto",
             line_height=LH_META),
        mono(f"p. {index} / {PAGES}", name="页码"),
    ], gap=48, width="fill_container", justifyContent="space_between")


def page(name, index, children, *, gap=32, decor=(), anchor="start"):
    """一页。固定 1920×1080，绝不 fit_content：投影比例是硬约束。

    页脚永远是正文层的最后一个孩子 + `space_between`，所以内容从上边距往下
    排、页脚贴着下边距，不需要为每一页手算高度。
    """
    content = frame(ids, f"{name} · 正文", width="fill_container",
                    height="fill_container", layout="vertical",
                    padding=[PAD_TOP, PAD_X, PAD_BOTTOM, PAD_X],
                    # 正文与页脚之间是固定的一档间距，不跟着页内节奏走：
                    # 页脚和脚注同为 24px 灰字，靠得近会读成同一段。
                    gap=56, justifyContent="space_between", fill=[])
    stacked = [col("内容", list(children), gap=gap), footer(index)]
    if anchor == "end":
        # 构图重心下沉：一个撑满剩余高度的空撑子把内容顶到页脚上方。
        # 不许页页都锚在同一个 y 上，收尾页尤其不该和内页一个重心。
        spacer = frame(ids, "留白撑子", width="fill_container",
                       height="fill_container", layout="none", fill=[])
        stacked.insert(0, spacer)
    content["children"] = stacked
    shell = frame(ids, name, width=W, height=H, layout="none",
                  fill=solid("$c-paper"), clipContent=True)
    if decor:
        ornament = frame(ids, f"{name} · 账页格线", width="fill_container",
                         height="fill_container", layout="none", fill=[])
        ornament["children"] = list(decor)
        # children[0] 在最上：正文层写在前面，格线永远在字的下面。
        shell["children"] = [content, ornament]
    else:
        shell["children"] = [content]
    return shell


def page_title(title):
    return body(title, size=FS_TITLE, weight=700, lh=LH_TITLE, name="页标题")


# ------------------------------------------------------------------ 三态记号
#
# 三态在**形状**上就可分辨：勾（两段折线）/ 半格（正方形左半填充）/ 短横。
# 颜色只是第二重线索 —— 色觉障碍读者、黑白打印稿都要能读出这三态。
TICK_BOX = 26
TICK_STROKE = 2.5


def _segment(name, x1, y1, x2, y2, color):
    """一段带角度的笔画。rectangle + rotation（度，绕节点中心）——
    `line` 节点在 layout:none 下会跑到文档绝对坐标去。"""
    length = math.hypot(x2 - x1, y2 - y1)
    angle = math.degrees(math.atan2(y2 - y1, x2 - x1))
    seg = rect(ids, name, width=round(length, 2), height=TICK_STROKE,
               fill=solid(color))
    seg["x"] = round((x1 + x2) / 2 - length / 2, 2)
    seg["y"] = round((y1 + y2) / 2 - TICK_STROKE / 2, 2)
    seg["rotation"] = round(angle, 2)
    return seg


def tick_full():
    """满足：一个绿勾。两段折线，不是圆点、不是对号字符。"""
    box = group(ids, "记号 · 满足", width=TICK_BOX, height=TICK_BOX,
                layout="none", fill=[])
    box["children"] = [
        _segment("勾 · 长笔", 9.5, 20.5, 23.0, 5.0, "$c-tick"),
        _segment("勾 · 短笔", 3.0, 14.0, 10.0, 21.0, "$c-tick"),
    ]
    return box


def tick_half():
    """部分满足：一个正方形，左半填实。"""
    outline = rect(ids, "半格 · 外框", width=24, height=24, fill=[],
                   stroke={"thickness": 1.5, "fill": solid("$c-tick-half")})
    outline["x"], outline["y"] = 1, 1
    filled = rect(ids, "半格 · 左半", width=12, height=24,
                  fill=solid("$c-tick-half"))
    filled["x"], filled["y"] = 1, 1
    box = group(ids, "记号 · 部分满足", width=TICK_BOX, height=TICK_BOX,
                layout="none", fill=[])
    box["children"] = [outline, filled]
    return box


def tick_none():
    """不满足：一条留空的短横。**不画叉** —— 叉是价值判断，横是事实。"""
    bar = rect(ids, "短横", width=20, height=2, fill=solid("$c-tick-pale"))
    bar["x"], bar["y"] = 3, 12
    box = group(ids, "记号 · 不满足", width=TICK_BOX, height=TICK_BOX,
                layout="none", fill=[])
    box["children"] = [bar]
    return box


TICKS = {"full": tick_full, "half": tick_half, "none": tick_none}


# ------------------------------------------------------------------ 主矩阵
DIMENSIONS = [
    ("工单自动分派", "按技能组、负载与优先级自动指派，\n无需人工转派即计满足。"),
    ("多渠道接入", "邮件、网页表单、即时通讯、电话四类中，\n支持三类及以上即计满足。"),
    ("时效承诺", "可按客户分级设定响应与解决时限，\n并在超时前自动触发升级。"),
    ("知识库联动", "坐席能在工单内检索并插入知识条目，\n且能把新写的条目回写。"),
    ("数据可导出", "全量工单与会话可导出为结构化文件，\n导出不限频次、不限字段。"),
    ("私有化部署", "可部署在客户自有服务器，\n且功能与云端版本一致。"),
]

# (维度, 我方, 甲, 乙, 丙)。自家列一样只是三态之一 —— 六项里我们也有一项
# 只做到部分满足，写出来比写满六个勾可信。
MATRIX = [
    ("工单自动分派", "full", "full", "half", "none"),
    ("多渠道接入", "full", "full", "full", "half"),
    ("时效承诺", "full", "half", "none", "half"),
    ("知识库联动", "full", "half", "full", "none"),
    ("数据可导出", "full", "full", "half", "full"),
    ("私有化部署", "half", "none", "none", "full"),
]
COLUMN_HEADS = ["本产品", "竞品甲", "竞品乙", "竞品丙"]


def matrix_cell(state, *, own=False, zebra=False):
    fill = solid("$c-own") if own else (solid("$c-band") if zebra else [])
    cell = frame(ids, "记号格", width=COL_DATA, height=ROW_H,
                 layout="horizontal", justifyContent="center",
                 alignItems="center", gap=0, fill=fill)
    cell["children"] = [TICKS[state]()]
    return cell


def matrix_head():
    """表头行。自家列的列头是一枚绿色实心 chip，其余三列走中性文字 ——
    着色即立场，立场只表达一次。"""
    label = frame(ids, "表头 · 维度", width=COL_LABEL, height=ROW_H,
                  layout="horizontal", alignItems="center", padding=[0, 24],
                  gap=0, fill=[])
    label["children"] = [
        body("评估维度", size=FS_BODY, weight=600, color="$c-ink-soft",
             lh=1.4, width=COL_LABEL - 48, name="表头文字"),
    ]
    cells = [label]
    for index, name in enumerate(COLUMN_HEADS):
        own = index == 0
        box = frame(ids, "表头格", width=COL_DATA, height=ROW_H,
                    layout="horizontal", justifyContent="center",
                    alignItems="center", gap=0,
                    fill=solid("$c-own") if own else [])
        if own:
            # 圆角字段整套一个都不写（默认就是直角）。写 `cornerRadius: 0`
            # 语义上没错，但会让「grep 圆角零命中」这道机器核多一条噪声。
            chip = frame(ids, "自家标记", width="fit_content",
                         height="fit_content", layout="horizontal",
                         padding=[8, 18], gap=0,
                         justifyContent="center", alignItems="center",
                         fill=solid("$c-tick"))
            chip["children"] = [
                text(ids, "自家标记文字", name, FS_BODY, 600, "$c-paper",
                     family=CJK, width="fit_content", growth="auto",
                     line_height=1.3),
            ]
            box["children"] = [chip]
        else:
            box["children"] = [
                text(ids, "表头格文字", name, FS_BODY, 500, "$c-ink-soft",
                     family=CJK, width="fit_content", growth="auto",
                     line_height=1.4),
            ]
        cells.append(vrule())
        cells.append(box)
    return row("矩阵表头", cells, gap=0, height=ROW_H, align="stretch")


def matrix_row(label, states, zebra):
    fill = solid("$c-band") if zebra else []
    label_cell = frame(ids, "行标签", width=COL_LABEL, height=ROW_H,
                       layout="horizontal", alignItems="center",
                       padding=[0, 24], gap=0, fill=fill)
    label_cell["children"] = [
        # 定宽写死是为了让行标签折行在产稿期就报出来：行标签一折行，整行
        # 72 的行高就被顶开，六行矩阵会跟着错位。
        body(label, size=FS_BODY, weight=500, lh=1.4, width=COL_LABEL - 48,
             name="行标签文字"),
    ]
    cells = [label_cell]
    for index, state in enumerate(states):
        cells.append(vrule())
        cells.append(matrix_cell(state, own=index == 0, zebra=zebra))
    return row("矩阵行", cells, gap=0, height=ROW_H, align="stretch")


def matrix():
    """整张矩阵 + 自家列外框。

    外框是一个叠在表体上的空描边 frame，**写在 children[0]**（layout:none
    的 z-order 是第一个在最上）；它没有填充，所以不会盖住格子里的记号。
    """
    rows = [hrule(INNER), matrix_head(), hrule(INNER, 2, "$c-rule-red")]
    height = 1 + ROW_H + 2
    for index, (label, *states) in enumerate(MATRIX):
        if index:
            rows.append(hrule(INNER))
            height += 1
        rows.append(matrix_row(label, states, zebra=index % 2 == 1))
        height += ROW_H
    table = col("表体", rows, gap=0)
    own_frame = frame(ids, "自家列外框", width=COL_DATA + 2, height=height,
                      layout="none", fill=[],
                      stroke={"thickness": 2, "fill": solid("$c-tick")})
    own_frame["x"], own_frame["y"] = COL_LABEL, 0
    stack = frame(ids, "主矩阵", width=INNER, height=height, layout="none",
                  fill=[])
    stack["children"] = [own_frame, table]
    return stack


def legend():
    items = []
    for state, label in (("full", "满足"), ("half", "部分满足"),
                         ("none", "不满足")):
        item = row("图例项", [
            TICKS[state](),
            text(ids, "图例文字", label, FS_META, 400, "$c-ink-soft",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=LH_META),
        ], gap=12, align="center", width="fit_content")
        items.append(item)
    items.append(text(ids, "图例说明",
                      "记号形状可分辨，不依赖颜色；「不满足」不画叉。",
                      FS_META, 400, "$c-ink-faint", family=CJK,
                      width="fit_content", growth="auto",
                      line_height=LH_META))
    return row("三态图例", items, gap=44, align="center")


def tally(sentence):
    """合计双线 + 结论行。账簿在这里写合计，我们写一句话 —— 不做加权总
    分，总分会把所有取舍藏进一个数字里。"""
    return col("合计", [
        hrule(INNER, 1, "$c-rule-red"),
        frame(ids, "双线间隙", width=INNER, height=4, layout="none", fill=[]),
        hrule(INNER, 2, "$c-rule-red"),
        body(sentence, size=FS_ITEM, weight=500, lh=1.5, name="结论行"),
    ], gap=0)


# ------------------------------------------------------------------ 01 封面
def cover():
    # 账页格线：右侧一片留空的账格。它是实线、满不透明、大到会被画幅裁切
    # ——「4-6% 不透明度的装饰形状」那条指纹的反面。
    ruling = []
    for i in range(9):
        line = hrule(760, 1)
        line["x"], line["y"] = 1064, 268 + i * 72
        ruling.append(line)
    top = hrule(760, 2, "$c-rule-red")
    top["x"], top["y"] = 1064, 196
    ruling.insert(0, top)
    for i in range(1, 4):
        column = rect(ids, "账格竖线", width=1, height=9 * 72,
                      fill=solid("$c-rule-blue"))
        column["x"], column["y"] = 1064 + i * 190, 196
        ruling.append(column)

    return page("01 封面", 1, [
        col("封面文字", [
            # 中文与拉丁各自一个节点：混在一个 Inter 节点里，中文会掉进回退
            # 字体，同一行出现两种字面。
            row("册头", [
                text(ids, "册头中文", "竞品能力核对", FS_META, 500,
                     "$c-ink-faint", family=CJK, width="fit_content",
                     growth="auto", line_height=LH_META),
                mono("· 2026 Q3", spacing=2, name="册头周期"),
            ], gap=10, align="center", width="fit_content"),
            body("把能做与不能做\n摊在同一张账上", size=FS_COVER, weight=700,
                 lh=LH_DISPLAY, spacing=-2, width=1000, name="主标题"),
            body("六项能力、四家产品，逐格核对。\n"
                 "记号只陈述事实，不打分、不排名。",
                 size=30, color="$c-ink-soft", width=1000, name="副标题"),
            body("评估范围：面向 50–500 人团队的客服工单系统，\n"
                 "依据公开文档与试用账号实测，数据截止 2026-07-31。",
                 size=FS_BODY, color="$c-ink-soft", width=1000, lh=1.6,
                 name="评估范围"),
        ], gap=28),
    ], gap=32, decor=ruling)


# ------------------------------------------------------------------ 02 口径
def criteria():
    cells = []
    for index, (term, rule_text) in enumerate(DIMENSIONS):
        item = col("维度定义", [
            row("维度名", [
                index_mark(index + 1),
                text(ids, "维度术语", term, FS_ITEM, 600, "$c-ink",
                     family=CJK, width="fit_content", growth="auto",
                     line_height=1.35),
            ], gap=0, align="center"),
            # 口径与术语对齐到同一条左线：序号列宽 46，正文让出同样多。
            col("口径正文", [
                body(rule_text, color="$c-ink-soft", width=734, lh=1.6,
                     name="判定口径"),
            ], padding=[0, 0, 0, 46]),
        ], gap=10, width=780)
        cells.append(item)
    left = col("口径左列", cells[0::2], gap=52, width=780)
    right = col("口径右列", cells[1::2], gap=52, width=780)
    return page("02 评估口径", 2, [
        page_title("先说清楚每一格是怎么判的"),
        body("六项维度、六条口径。判定只回答「做到没做到」，"
             "不回答「做得好不好」——后者需要的证据这张表给不了。",
             color="$c-ink-soft", lh=1.6, name="口径引导"),
        row("口径两列", [left, right], gap=INNER - 780 * 2),
        body("口径与记号规则在评估开始前确定，评估过程中未作调整；"
             "无定义的维度不进矩阵。",
             size=FS_META, color="$c-ink-faint", lh=LH_META, name="口径脚注"),
    ], gap=40)


# ------------------------------------------------------------------ 03 矩阵
def board():
    return page("03 主矩阵", 3, [
        page_title("六项能力，逐格核对"),
        matrix(),
        legend(),
        tally("六项里我们满足五项，唯一的缺口是私有化部署；"
              "今天必须私有部署的客户，应当选丙。"),
    ], gap=28)


# ------------------------------------------------------------------ 04 分位
SEG_W = 299
BAR_W = SEG_W * 4 + 3
QUARTILES = [
    ("工单首响时长", 0.62,
     "落在第三四分位：比同类的中位数快，但没有到最快的那一档。"),
    ("单座席月成本", 0.28,
     "落在第一四分位：自动分派省下的人力，直接反映在这一格上。"),
    ("私有化部署能力", 0.12,
     "落在第一四分位的左端：这是我们承认的短板，排在明年一季度。"),
]


def quartile_bar(position):
    """四等分刻度条 + 一枚位置标。四段之间是 1px 真格线，不靠底色差分段
    ——两个同色矩形贴在一起会被渲成一整块。"""
    segments = []
    for index in range(4):
        if index:
            segments.append(vrule(40))
        segments.append(rect(ids, "四分位段", width=SEG_W, height=40,
                             fill=solid("$c-band")))
    track = row("刻度轨", segments, gap=0, width=BAR_W, height=40,
                align="stretch")
    track["x"], track["y"] = 0, 8
    marker = rect(ids, "位置标", width=6, height=56, fill=solid("$c-tick"))
    marker["x"], marker["y"] = round(position * BAR_W) - 3, 0
    bar = frame(ids, "四分位条", width=BAR_W, height=56, layout="none",
                fill=[])
    bar["children"] = [marker, track]
    return bar


def quartile_scale():
    """四分位刻度头。四段各自具名，位置标的含义写在同一行 —— 记号在这一档
    从不靠图注之外的东西解释自己。"""
    cells = []
    for index in range(4):
        if index:
            cells.append(vrule(28, color="$c-paper"))
        box = frame(ids, "刻度名", width=SEG_W, height=28,
                    layout="horizontal", justifyContent="center",
                    alignItems="center", gap=0, fill=[])
        box["children"] = [mono(f"Q{index + 1}", name="刻度文字")]
        cells.append(box)
    scale = row("刻度头", cells, gap=0, width=BAR_W, height=28,
                align="stretch")
    note = row("位置标说明", [
        rect(ids, "说明标", width=6, height=24, fill=solid("$c-tick")),
        text(ids, "说明文字", "本产品所在位置", FS_META, 400, "$c-ink-soft",
             family=CJK, width="fit_content", growth="auto",
             line_height=LH_META),
    ], gap=12, align="center", width="fit_content")
    return row("刻度行", [scale, note], gap=48, align="center")


def quartiles():
    groups = []
    for name, position, reading in QUARTILES:
        groups.append(col("分位组", [
            row("分位名", [
                text(ids, "分位标题", name, FS_ITEM, 600, "$c-ink",
                     family=CJK, width="fit_content", growth="auto",
                     line_height=1.35),
                mono(f"{int(position * 100)}", color="$c-ink-faint",
                     name="分位读数"),
                text(ids, "分位单位", "分位", FS_META, 400, "$c-ink-faint",
                     family=CJK, width="fit_content", growth="auto",
                     line_height=LH_META),
            ], gap=10, align="center"),
            quartile_bar(position),
            body(reading, color="$c-ink-soft", width=1200, lh=1.6,
                 name="读法"),
        ], gap=16))
    return page("04 分位刻度", 4, [
        page_title("同类产品里，我们各自站在哪一段"),
        quartile_scale(),
        col("分位组列", groups, gap=52),
    ], gap=32)


# ------------------------------------------------------------------ 05 差距
GAPS = [
    ("私有化部署尚未开放",
     "现状：仅提供专属云，客户不能把数据放在自己机房。",
     "计划：2027 年一季度发布可交付的私有部署包，先对金融与医疗行业开放。"),
    ("电话渠道依赖第三方",
     "现状：电话接入要另接一家服务商，工单里只能看到通话记录链接。",
     "计划：四季度完成自建话务网关，通话录音与转写直接落在工单里。"),
    ("知识库回写要管理员确认",
     "现状：坐席写的新条目需要管理员审核后才进知识库，平均滞后 2 天。",
     "计划：三季度上线分级权限，高级坐席可直接发布，审核转为抽查。"),
]


def gaps():
    items = []
    for index, (title, now, plan) in enumerate(GAPS):
        if index:
            items.append(hrule(INNER))
        items.append(row("差距条", [
            tick_half(),
            col("差距文字", [
                text(ids, "差距标题", title, FS_ITEM, 600, "$c-ink",
                     family=CJK, width="fit_content", growth="auto",
                     line_height=1.35),
                body(now, color="$c-ink-soft", width=1400, lh=1.55,
                     name="现状"),
                body(plan, color="$c-ink", width=1400, lh=1.55, name="计划"),
            ], gap=8),
        ], gap=28, align="start"))
    return page("05 差距", 5, [
        page_title("三处我们还没做到的"),
        body("差距按能被验证的方式写：现状是今天的事实，计划带月份。"
             "没有计划的差距不写进这一页，写了也只是姿态。",
             color="$c-ink-soft", lh=1.6, name="差距引导"),
        col("差距列", items, gap=40),
    ], gap=40)


# ------------------------------------------------------------------ 06 优势
# 断点写死在文案里：列宽 532、字号 28 时一行 18 字，这里每行 ≤17 字。
STRENGTHS = [
    ("自动分派命中率",
     "工单进队列后按技能组\n与实时负载自动指派，\n不需要人工转派。",
     "试用期 3200 单，\n人工转派 47 单，占 1.5%。"),
    ("导出不设限",
     "全量工单、会话与附件\n元数据可随时导出为\n结构化文件。",
     "单次导出 12 万条耗时 96 秒，\n字段与后台完全一致。"),
    ("超时前先升级",
     "按客户分级设定时限，\n剩余两成时长时自动\n升级到组长。",
     "首响超时率从 8.4% 降到 2.1%，\n已连续两个季度。"),
]


def strengths():
    columns = []
    width = 532
    for index, (title, what, proof) in enumerate(STRENGTHS):
        if index:
            # 列与列之间是一条真的账簿竖线，不是纯留白 —— 这一页因此仍读作
            # 「账」，而不是三张并排的卡片。
            columns.append(vrule(392))
        fits(title, FS_ITEM, width)
        columns.append(col("优势列", [
            tick_full(),
            text(ids, "优势标题", title, FS_ITEM, 600, "$c-ink", family=CJK,
                 width=width, growth="fixed-width", line_height=1.35),
            body(what, color="$c-ink-soft", width=width, lh=1.7,
                 name="说明"),
            hrule(72, 2, "$c-rule-red"),
            row("证据行", [
                text(ids, "证据标签", "证据", FS_META, 500, "$c-ink-faint",
                     family=CJK, width="fit_content", growth="auto",
                     line_height=1.6),
                body(proof, color="$c-ink", width=width - 88, lh=1.6,
                     name="证据"),
            ], gap=16),
        ], gap=18, width=width))
    return page("06 优势", 6, [
        page_title("三处我们做到了，并且能被验证"),
        body("每一条都配一个可复核的数字；没有数字的优势不写进这一页。",
             color="$c-ink-soft", lh=1.6, name="优势引导"),
        row("优势三列", columns, gap=32, align="start"),
    ], gap=40)


# ------------------------------------------------------------------ 07 结论
ACTIONS = [
    "把私有化部署排进 2027 年一季度的交付计划，本月给出范围。",
    "在报价单里写明电话渠道当前依赖第三方，不含糊带过。",
    "把这张核对表随合同附送，三个月更新一次，口径不变。",
]


def closing():
    actions = []
    for index, line in enumerate(ACTIONS):
        actions.append(row("动作条", [
            index_mark(index + 1),
            body(line, width=1400, lh=1.55, name="动作"),
        ], gap=0, align="start"))
    return page("07 结论", 7, [
        page_title("这张账读完之后"),
        body("能力上的差距只有一处，\n而它恰好决定了一类客户能不能签。",
             size=FS_LEAD, weight=600, lh=1.3, width=1400, name="总判断"),
        # 合计双线，与主矩阵表尾同一支装置：它托住的是「所以呢」，不是给标
        # 题加下划线。
        col("结论合计线", [
            hrule(INNER, 1, "$c-rule-red"),
            frame(ids, "双线间隙", width=INNER, height=4, layout="none",
                  fill=[]),
            hrule(INNER, 2, "$c-rule-red"),
        ], gap=0),
        col("动作列", actions, gap=24),
        body("来源：各产品公开文档与试用账号实测，数据截止 2026-07-31；"
             "口径见第 2 页，逐格记号见第 3 页。",
             size=FS_META, color="$c-ink-faint", lh=LH_META, name="来源脚注"),
    ], gap=32, anchor="end")


# ------------------------------------------------------------------ 板位
# 3 板一行。行距 1440（而不是 1200）：画布在帧上方以固定屏幕偏移画帧名，
# 在 3 宽 deck 铺满屏幕的缩放下，120 文档 px 的行距只合约 16 屏幕 px，第二
# 行的帧名会压到上一行的板上。
BOARD_STEP_X, BOARD_STEP_Y, PER_ROW = 2040, 1440, 3


def build():
    boards = [cover(), criteria(), board(), quartiles(), gaps(),
              strengths(), closing()]
    for index, node in enumerate(boards):
        node["x"] = (index % PER_ROW) * BOARD_STEP_X
        node["y"] = (index // PER_ROW) * BOARD_STEP_Y
    return boards


# 对比度（WCAG 相对亮度比）：
#   c-ink       on c-paper / c-band / c-own   14.63 / 13.16 / 14.11
#   c-ink-soft  on c-paper / c-band / c-own    6.48 /  5.83 /  6.25
#   c-ink-faint on c-paper                     4.14   ← 最低一对（≥24px 注释）
#   c-tick      on c-paper / c-own             5.06 /  4.90
#   c-paper     on c-tick（自家 chip 的反白字）  5.08
# c-rule-blue (3.13) / c-rule-red (3.13) / c-tick-half (3.24) 不参与：它们
# 全部是**非文字**的线与记号（rectangle 且宽或高 ≤3px，或纯几何记号），按
# §4.4 的豁免规则按节点类型排除 —— 对比一旦拉高，格线就会跟记号抢注意力。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "账簿勾格 · 竞品矩阵 deck")
