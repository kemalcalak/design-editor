#!/usr/bin/env python3
"""sounding-navy-deck.op — 测深海图 · 咨询策略档（D7，7 页 1920×1080）

落格：high formality × medium density × **mixed**（深浅书挡）。
定位：把判断当水深报出来的策略交付档 —— 每页一个结论式标题、一条可追溯的
证据、一行带来源的读数。

### 锚点

航海测深海图。海图纸的暖白、等深线的钢蓝、深水区的墨蓝、浅滩警戒的赭黄。
锚的是**「图上每一个数字都是有人下去量过的」**这件事 —— 这正是策略档要传达
的：结论不是观点，是读数。不锚船、锚、罗盘、舵轮等任何具象符号。

### 三个招牌母题

1. **深度剖面条**（04 页）—— 条不浮空，从顶部一条 2px 基线**向下垂**，读作
   水深。与业界通行的浮动瀑布图的区别是刻意的：浮动条读作「位置」，下垂条
   读作「深度」，后者与「损失/消耗」的语义同向，也让基线成为唯一的对齐锚。
2. **航迹目录条**（02 页展开 / 03–06 页页脚缩略）—— 一条 2px 航迹线上等距
   排 5 个圆点，当前节实心。展开态给全局，缩略态是跨页的进度锚。
3. **读数条**（03 页）—— 页脚上方一条 3px 钢蓝上边线，线下左侧 mono 数值 +
   右侧一句结论。**每页至多一条**，它就是这一页的 takeaway 落点。放页脚不放
   要点框，是因为要点框会和正文争视觉权重，而页脚位置天然是「读完之后」。

### 负约束（spec §2 D7 Strictly avoid，逐条落实）

  - 零圆角。圆角一出现，本档立刻从「海图」滑向「SaaS 落地页」。
  - 零阴影、零渐变、零玻璃拟态 —— 唯一的「深度」是剖面条表达的语义深度。
  - 内容不装卡片。分区靠 1px 线与留白；`chart.paper.deep` 面整套只出现在
    06 页右栏（05 页表格改用真格线，连斑马都不用）。
  - **`shoal.ochre` 整套只出现一次**（06 页右栏顶部那条 96×4 短线）。警戒色
    出现两次就不再是警戒。
  - 标题只写结论式完整句，不写话题式；标题下不画横线。
  - 图无图例、无网格线、无 y 轴；数值直接标在图形上；非 key 系列走中性灰。
  - 深页只在开头两页与结尾一页成对出现（书挡），中间内容页一律亮底。
  - 不出现按钮态 / tab / pill / badge / 导航条等任何暗示可交互的元素。
  - 出现数字的页面必须有来源行。
  - 钢蓝与赭黄之外不用第三支有彩色。

### 密度核对（§3.2，槽位按「一条要点计 1」）

    01 封面 3/4 · 02 航迹 6/6 · 03 结论 6/6 · 04 证据 5/6
    05 数据 9/10 · 06 取舍 6/6 · 07 行动 5/5

页码与页脚眉标是母版家具（§4.1 边距豁免名单里的 footer/page 角色），不占槽位。

对比度实测见文件末尾。
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from deckkit import (CJK, H, NUM, W, assert_type_scale, audit_document,
                     cjk_width, latin_track, lh, place_boards, track)
from oplib import Ids, color_vars, frame, rect, solid, text, write_doc

ids = Ids()

# 色板全部来自 spec §2 D7 的色板表（oklch 实算，非估值）。
#
# 两条推导必须留在这里，否则将来「顺手统一一下色相」会把这一档的身份抹掉：
#   · 深底不用纯黑，用 chroma 0.048 的墨蓝。纯黑封面配白字是所有 AI deck 的
#     默认出厂设置；带 chroma 的深底让钢蓝在其上读作「同一族颜色的深处」。
#   · 亮底的 chroma 走 H85 暖向、深底走 H250 冷向。海图纸是暖的、海水是冷的，
#     这个冷暖分裂是「纸 vs 水」的物理关系，让 mixed 的深浅两态有真实材质差
#     别，而不是同一个色的明暗两级。
VARS = color_vars({
    "c-paper":        "#F7F4EE",   # 内容页底
    "c-paper-deep":   "#EBE7DF",   # 分区面（整套只用于 06 页右栏）
    "c-navy":         "#091E32",   # 封面 / 分节 / 收尾页底
    "c-navy-low":     "#031223",   # 深页上的更深块
    "c-ink":          "#1D2228",   # 亮页主文字        14.58 : paper
    "c-ink-soft":     "#565B61",   # 亮页次级文字       6.24
    "c-ink-faint":    "#7C8186",   # 脚注 / 页码        3.58（仅 ≥24px）
    "c-on-navy":      "#F0F4F7",   # 深页主文字        15.27 : navy
    "c-on-navy-dim":  "#B3B8BE",   # 深页次级文字       8.46
    "c-steel":        "#266EA4",   # 主 accent          4.97 : paper
    "c-steel-deep":   "#044A7D",   # 承载白字的钢蓝     8.39（白字）
    "c-steel-lift":   "#83B0D7",   # 深页上的 accent    7.36 : navy
    "c-ochre":        "#A46D1A",   # 第二色·警戒，整套一次  4.00
    "c-rule":         "#D3D1CD",   # 栏线 / 表格线（非文字）
})

# §2 D7 版式语法：下边距 > 上边距。投影下沿常被前排人头遮挡 —— 这是真实
# 约束不是趣味，同时它顺手打破了「万物居中且四边等距」这条 AI 指纹。
PAD_TOP, PAD_BOTTOM, PAD_X = 96, 120, 120
INNER = W - PAD_X * 2          # 1680
GUTTER = 24                    # gap 基准，本档所有间距都是它的整数倍

# 线宽只有三档。多一档就要解释「为什么这里 2.5」，而海图上没有 2.5。
HAIR, RULE, HEAVY = 1, 2, 3

# 字号（§2 D7）。全部过 assert_type_scale 的地板自检。
FS_COVER = 104        # 封面主标（区间 100–116）
FS_TITLE = 64         # 结论式页标题
FS_CLOSE = 72         # 收尾页收束句
FS_LEDE = 36          # 引导句
FS_ITEM = 40          # 论据标题 / 剖面注解标题
FS_BODY = 32          # 正文
FS_TABLE = 30         # 表格正文
FS_NOTE = 24          # 脚注 / 来源 / 页码（本档唯一允许 24 的位置）
FS_READOUT = 48       # 读数条的 mono 数值

# 本档零圆角。写成常量是为了让「有没有人偷偷加了圆角」在 diff 里可数。
RADIUS = 0

# 五个测点与 03–07 五张内容页一一对应：航迹缩略的「当前节」因此是
# 页码的另一种写法，不是另一套编号。
SECTIONS = ["总体结论", "毛利去向", "安装网络", "取舍判断", "行动清单"]


# ------------------------------------------------------------------ 骨架
def slide(name, *, dark=False, gap=GUTTER * 2, justify="start"):
    """一帧。外壳 layout:none + 内层 flex，与本仓既有 deck 同构。

    外壳退成 layout:none 的理由：正文层是一个 flex 容器，任何背景装饰挂进去
    都会变成参与排版的兄弟把内容挤走。本档虽然装饰极少（只有几条线），但结构
    保持一致，将来补装饰时不必回头改。
    """
    body = frame(ids, f"{name} · 正文", width="fill_container",
                 height="fill_container", layout="vertical",
                 padding=[PAD_TOP, PAD_X, PAD_BOTTOM, PAD_X], gap=gap,
                 justifyContent=justify, alignItems="start", fill=[])
    body["children"] = []
    shell = frame(ids, name, width=W, height=H, layout="none",
                  fill=solid("$c-navy" if dark else "$c-paper"),
                  clipContent=True)
    shell["children"] = [body]
    shell["content"] = body
    return shell


def col(name, children, *, gap=GUTTER, width="fill_container",
        height="fit_content", align="start", fill=None, **props):
    node = frame(ids, name, width=width, height=height, layout="vertical",
                 gap=gap, alignItems=align, fill=fill or [], **props)
    node["children"] = children
    return node


def row(name, children, *, gap=GUTTER, width="fill_container",
        height="fit_content", align="start", fill=None, **props):
    node = frame(ids, name, width=width, height=height, layout="horizontal",
                 gap=gap, alignItems=align, fill=fill or [], **props)
    node["children"] = children
    return node


def hrule(color="$c-rule", thickness=HAIR, width="fill_container"):
    """横线一律用 rectangle。

    §6.3：`line` 在 layout:none 容器下吃的是文档绝对坐标而不是父容器局部
    坐标，会让线出现在画布的另一个角落。本档所有线（航迹线、表格线、读数条
    上边线、中缝）都是 rect，不要「优化」成 line。
    """
    return rect(ids, "横线", width=width, height=thickness, fill=solid(color))


def vrule(height, color="$c-rule", thickness=HAIR):
    return rect(ids, "竖线", width=thickness, height=height,
                fill=solid(color))


def title(content, *, dark=False):
    """结论式页标题。≤2 行，断点在文案里显式写 \\n。

    §3.5：允许 ≤2 行的标题必须显式指定断点，不交给自动折行 —— 当前渲染栈对
    CJK 是逐字符断行、没有标点避头尾，断在「，」前面是可能的。
    """
    return text(ids, "页标题", content, FS_TITLE, 700,
                "$c-on-navy" if dark else "$c-ink", family=CJK,
                line_height=lh(FS_TITLE), spacing=track(FS_TITLE),
                width=INNER)


def note(content, *, dark=False, width=None):
    return text(ids, "来源脚注", content, FS_NOTE, 400,
                "$c-on-navy-dim" if dark else "$c-ink-faint", family=CJK,
                line_height=lh(FS_NOTE), width=width or cjk_width(FS_NOTE,
                                                                  INNER))


def sounding_dot(ring, size, *, filled):
    """航迹点。实心 = 当前节，空心 = 其余节。

    空心态靠 1.5px 描边而不是「填一个底色接近页底的圆」：换页底时描边自己
    还在，填色版会在深浅两态之间各错一次。
    """
    props = {"width": size, "height": size, "cornerRadius": size // 2}
    if filled:
        props["fill"] = solid(ring)
    else:
        props["fill"] = []
        props["stroke"] = {"thickness": 1.5, "fill": solid(ring)}
    return rect(ids, "航迹点", **props)


def track_dots(active, *, width=INNER, dark=True, dot=12):
    """航迹目录条的**缩略态**：一条线 + 5 个点，当前节实心。

    放在内容页页脚，是这个母题作为「跨页进度锚」的落点。缩略态不带节名 ——
    节名在 02 页给过一次，页脚再写一遍就成了每页重复的文字噪声。

    叠放顺序：§6.4，`layout:none` 的 children[0] 在**最上**（与业界惯例相
    反）。所以圆点行写在数组第一个，航迹线写在后面。
    """
    ring = "$c-steel-lift" if dark else "$c-steel"
    base = "$c-on-navy-dim" if dark else "$c-rule"
    lane = (width - dot) / (len(SECTIONS) - 1)
    dots = []
    for index in range(len(SECTIONS)):
        node = sounding_dot(ring, dot, filled=index == active)
        node["x"] = round(index * lane, 2)
        node["y"] = 0
        dots.append(node)
    dot_layer = frame(ids, "航迹点层", width=width, height=dot,
                      layout="none", fill=[])
    dot_layer["children"] = dots

    line = rect(ids, "航迹线", width=width, height=RULE, fill=solid(base))
    line["x"], line["y"] = 0, (dot - RULE) / 2

    shell = frame(ids, "航迹条", width=width, height=dot, layout="none",
                  fill=[])
    shell["children"] = [dot_layer, line]
    return shell


def footer(page, *, dark=False, active=None, source=None):
    """页脚家具：眉标 + 航迹缩略 + 页码。

    §3.3 把「页码与页标位置」列进**不允许跨页变**的一栏，所以这一条在 02–07
    页的 y 上完全相同；01 封面不带页脚（封面是陈述不是信息）。

    航迹缩略放在这里，是「航迹目录条」这个母题作为**跨页进度锚**的落点：
    02 页给一次全展开，之后每一页在同一个位置复现一次缩略态。观众任何时候
    抬头都能看出现在讲到第几个测点，而不必记住议程。
    """
    faint = "$c-on-navy-dim" if dark else "$c-ink-faint"
    label = "明澜 · 渠道诊断"
    if active is not None:
        label += f"｜{SECTIONS[active]}"
    left = text(ids, "页脚眉标", label, FS_NOTE, 400, faint, family=CJK,
                width="fit_content", growth="auto", line_height=lh(FS_NOTE))
    folio = text(ids, "页码", f"{page:02d} / 07", FS_NOTE, 500, faint,
                 family=NUM, width="fit_content", growth="auto",
                 line_height=lh(FS_NOTE), spacing=latin_track("07"))
    kids = [left]
    if active is not None:
        kids.append(track_dots(active, width=200, dark=dark, dot=8))
    kids.append(folio)
    bar = row("页脚", kids, gap=GUTTER, align="center",
              justifyContent="space_between")
    if source is None:
        return bar
    return col("页脚区", [note(source, dark=dark), bar], gap=GUTTER)


# ------------------------------------------------------------------ 01 封面
def cover():
    code = text(ids, "项目代号", "MLN · 26H1 渠道诊断", FS_NOTE, 500,
                "$c-steel-lift", family=CJK, width="fit_content",
                growth="auto", line_height=lh(FS_NOTE))

    lede = col("主张", [
        text(ids, "主标题", "换渠道救不回毛利\n漏水口在安装这一段",
             FS_COVER, 700, "$c-on-navy", family=CJK,
             line_height=lh(FS_COVER), spacing=track(FS_COVER), width=INNER),
        rect(ids, "短线", width=120, height=HEAVY,
             fill=solid("$c-steel-lift")),
    ], gap=GUTTER * 2)

    meta = row("落款", [
        text(ids, "委托方", "明澜家用水处理 · 战略办公室", 26, 400,
             "$c-on-navy-dim", family=CJK, width="fit_content", growth="auto",
             line_height=lh(26)),
        text(ids, "日期", "2026.08", 26, 500, "$c-on-navy-dim", family=NUM,
             width="fit_content", growth="auto", line_height=lh(26),
             spacing=latin_track("2026.08")),
    ], gap=GUTTER, align="center", justifyContent="space_between")

    s = slide("01 封面", dark=True, gap=GUTTER * 2, justify="space_between")
    s["content"]["children"] = [code, lede, meta]
    return s


# -------------------------------------------------------------- 02 议题航迹
def agenda():
    """分节总览：航迹目录条**全展开**，五个节点全部带中文节名，无正文。

    五个节名各占一列、列宽均分。列与列之间留 GUTTER 的真间隙而不是靠居中
    对齐挤出来 —— §6.2：相邻同族元素贴到 0px 会被几何检测器判成「读起来连成
    一个词」的 jam，也确实读不出分栏。
    """
    # 圆点与节名走**同一套等分列**：五个 fill_container 列 + 同样的沟槽，各自
    # 在列内居中。早先的写法把点按 (INNER − dot)/4 的绝对步长摆、名字交给等分
    # 列，两套算法的中心差到 150px —— 目录条上点与名对不上，整个母题就废了。
    dot = 20
    dot_cells = []
    for index in range(len(SECTIONS)):
        cell = frame(ids, "航迹点位", width="fill_container", height=dot,
                     layout="horizontal", gap=0, alignItems="center",
                     justifyContent="center", fill=[])
        cell["children"] = [sounding_dot("$c-steel-lift", dot,
                                         filled=index == 0)]
        dot_cells.append(cell)
    dot_row = row("航迹点行", dot_cells, gap=GUTTER, align="center")

    names = []
    for index, label in enumerate(SECTIONS):
        names.append(text(ids, "节名", label, FS_ITEM, 600,
                          "$c-on-navy" if index == 0 else "$c-on-navy-dim",
                          family=CJK, line_height=lh(FS_ITEM),
                          align="center"))
    name_row = row("节名行", names, gap=GUTTER, align="start")

    lane = frame(ids, "航迹线", width=INNER, height=RULE,
                 fill=solid("$c-on-navy-dim"))
    board = col("航迹目录", [lane, dot_row, name_row], gap=GUTTER)

    s = slide("02 议题航迹", dark=True, gap=GUTTER * 2,
              justify="space_between")
    s["content"]["children"] = [
        title("这次诊断，我们只下了五个测点", dark=True),
        board,
        footer(2, dark=True),
    ]
    return s


# -------------------------------------------------------------- 03 结论页
# 说明句一律压到一行（≤30 汉字）。这是 §3.1 铁律一的正面执行：装不下时
# 第一个动作是**砍文案**，不是缩字号、不是把行距压掉、更不是加 clipContent。
# 被砍掉的对照数字挪进了 05 页那张表 —— 它本来就该在表里被逐行核对。
ARGUMENTS = [
    ("上门当天装完的门店，复购没掉过",
     "华东 91% 的工单当天闭环，华南 88%。"),
    ("被改约一次，客户基本就走了",
     "改约过的工单，年内复购只剩 9%。"),
    ("装不好不是缺人，是缺件",
     "西南 47% 的改约理由是配件不在车上。"),
]


def readout(value, conclusion):
    """读数条。3px 钢蓝上边线，线下左 mono 数值、右一句结论。

    数值走 `c-steel-deep` 而不是 accent 本色：一页只允许 accent 出现一次，
    那一次给了这条上边线 —— 它是把「这一页的读数」从正文里分出来的那个动作。
    数值本身跟着走深一档，读作墨重而不是第二次强调。
    """
    line = rect(ids, "读数上边线", width="fill_container", height=HEAVY,
                fill=solid("$c-steel"))
    body = row("读数", [
        text(ids, "读数数值", value, FS_READOUT, 700, "$c-steel-deep",
             family=NUM, width="fit_content", growth="auto",
             line_height=1.05, spacing=latin_track(value)),
        text(ids, "读数结论", conclusion, FS_BODY, 500, "$c-ink", family=CJK,
             width=cjk_width(FS_BODY, 1160), align="right"),
    ], gap=GUTTER * 2, align="center", justifyContent="space_between")
    return col("读数条", [line, body], gap=GUTTER)


def conclusion():
    items = []
    for index, (head, desc) in enumerate(ARGUMENTS, 1):
        items.append(row(f"论据 {index}", [
            text(ids, "论据序号", f"{index:02d}", FS_ITEM, 500,
                 "$c-ink-faint", family=NUM, width=72, growth="fixed-width",
                 line_height=lh(FS_ITEM), spacing=latin_track("01")),
            col("论据文案", [
                text(ids, "论据标题", head, FS_ITEM, 600, "$c-ink",
                     family=CJK, line_height=lh(FS_ITEM),
                     width=cjk_width(FS_ITEM, INNER - 96)),
                text(ids, "论据说明", desc, FS_BODY, 400, "$c-ink-soft",
                     family=CJK, line_height=lh(FS_BODY, body=True),
                     width=cjk_width(FS_BODY, INNER - 96)),
            ], gap=8),
        ], gap=GUTTER, align="start"))

    s = slide("03 结论", gap=GUTTER * 2, justify="space_between")
    s["content"]["children"] = [
        col("论证", [
            title("复购只发生在\n上门当天能装完的门店"),
            text(ids, "引导句", "同一款机器、同一个价，差别不在渠道，在人。",
                 FS_LEDE, 400, "$c-ink-soft",
                 family=CJK, line_height=lh(FS_LEDE, body=True),
                 width=cjk_width(FS_LEDE, INNER)),
            col("论据列表", items, gap=GUTTER),
        ], gap=GUTTER * 2),
        col("页尾", [
            readout("3.1×", "复购差距来自安装，不来自价格。"),
            footer(3, active=0),
        ], gap=GUTTER + 8),
    ]
    return s


# -------------------------------------------------------------- 04 证据页
# (名称, 数值, 相对水深, 是否 key)。首尾总量实心钢蓝深，中间减项走中性灰，
# 唯一的 key 项走 accent —— 一页一次强调，那一次要落在页面真正的论点上。
#
# 减项不用 `c-ochre`：赭黄整套只出现一次，那一次给了 06 页的取舍代价。
# §2 D7 的母题描述里写的是「减项用 shoal.ochre」，与页型清单里「06 页是唯一
# 一次」冲突；页型清单是结构契约、母题描述是表现建议，取前者。中性灰同样满足
# 「非 key 系列走中性阶」这条更硬的规则。
PROFILE = [
    ("出厂毛利", "42.0", 420, "anchor"),
    ("渠道返点", "6.4", 64, "plain"),
    ("仓配摊销", "3.1", 31, "plain"),
    ("安装返工", "11.8", 118, "key"),
    ("退换保修", "2.7", 27, "plain"),
    ("到手毛利", "18.0", 180, "anchor"),
]

PROFILE_COLORS = {"anchor": "$c-steel-deep", "key": "$c-steel",
                  "plain": "$c-ink-faint"}

CHART_W = 1152
ANNOTATE_W = INNER - CHART_W - GUTTER * 2      # 480


def depth_profile():
    """深度剖面条。

    条**不浮空**：全部从顶部那条 2px 基线向下垂，读作水深。这与业界通行的
    浮动瀑布图不同，而且是刻意的 —— 浮动条读作「位置」，下垂条读作「深度」，
    后者与「损失 / 消耗」的语义同向，也让基线成为整张图唯一的对齐锚。

    数值紧贴每条的下端而不是排在图外的一行：§3.6 图表去装饰要求数值直接标在
    图形上；而「每个数字停在自己那一档水深上」正好就是测深图的读法。
    """
    bar_w = (CHART_W - GUTTER * (len(PROFILE) - 1)) // len(PROFILE)
    columns = []
    for label, value, depth, kind in PROFILE:
        columns.append(col(f"{label} 列", [
            rect(ids, "剖面条", width=bar_w, height=depth,
                 fill=solid(PROFILE_COLORS[kind])),
            text(ids, "剖面数值", value, 28, 600,
                 "$c-ink" if kind != "plain" else "$c-ink-soft", family=NUM,
                 line_height=1.2, align="center",
                 spacing=latin_track(value)),
            text(ids, "剖面标签", label, FS_NOTE, 400, "$c-ink-faint",
                 family=CJK, line_height=lh(FS_NOTE), align="center"),
        ], gap=8, width=bar_w, align="start"))

    bars = row("剖面列", columns, gap=GUTTER, width=CHART_W, align="start")
    baseline = rect(ids, "剖面基线", width=CHART_W, height=RULE,
                    fill=solid("$c-ink"))
    content = col("剖面内容", [baseline, bars], gap=0, width="fill_container",
                  height="fill_container")

    # 这里曾经画过一条「到手毛利水位」的虚线横贯全图。删掉了：每条的数值与
    # 名称停在自己那一档水深上，任何一条水平参考线都必然从某一组标签中间穿
    # 过去，读起来像给文字打了删除线。**一条划过文字的参考线比没有参考线更
    # 糟** —— 水位这件事已经由「到手毛利」那根实心条自己说清楚了。
    chart_h = RULE + max(p[2] for p in PROFILE) + 8 + 34 + 8 + 32
    shell = frame(ids, "深度剖面条", width=CHART_W, height=chart_h,
                  layout="none", fill=[])
    shell["children"] = [content]
    return shell


# 每一条说明的断点都写死在文案里（§3.5 / §6.11）。当前渲染栈对 CJK 是逐字符
# 断行、**没有标点避头尾**，交给自动折行就会出现「。」独占一行这种事故；而
# 注解栏只有 480 宽、30px 一行放得下 16 字，正是最容易踩的那一档。
ANNOTATIONS = [
    ("返工的钱不在财务账上",
     "它记在售后工时里，\n季度报表看不见，\n年底盘账才发现。"),
    ("换渠道省不下这一段",
     "经销商换一轮，\n安装还是那批人。\n返点让掉两个点，返工照旧。"),
]


def evidence():
    blocks = []
    for index, (head, desc) in enumerate(ANNOTATIONS):
        if blocks:
            blocks.append(hrule())
        blocks.append(col("注解组", [
            text(ids, "注解标题", head, FS_LEDE, 600, "$c-ink", family=CJK,
                 line_height=lh(FS_LEDE), width=ANNOTATE_W),
            text(ids, "注解说明", desc, FS_TABLE, 400, "$c-ink-soft",
                 family=CJK, line_height=lh(FS_TABLE, body=True),
                 width=ANNOTATE_W),
        ], gap=8))

    band = row("证据区", [
        depth_profile(),
        col("注解栏", blocks, gap=GUTTER + 8, width=ANNOTATE_W),
    ], gap=GUTTER * 2, align="start")

    s = slide("04 证据", gap=GUTTER * 2, justify="space_between")
    s["content"]["children"] = [
        col("论证", [title("毛利不是被渠道拿走的\n是被返工吃掉的"), band],
            gap=GUTTER * 2),
        footer(4, active=1,
               source="数据来源：明澜 2026 年 1–6 月工单与售后台账，单位 元/台，"
                      "样本 626 家门店。"),
    ]
    return s


# -------------------------------------------------------------- 05 数据页
# 列宽写死并且必须加起来等于 INNER。表格没有自动列宽，写错就溢出；而
# §6.10「渲染栈无 tabular 数字」意味着数值列也不能靠 fit_content 指望数字
# 自己对齐 —— 定宽 + 右对齐是今天唯一能让一列数字排齐的写法。
TABLE_COLS = [384, 240, 300, 300, 360]
assert sum(TABLE_COLS) + GUTTER * (len(TABLE_COLS) - 1) == INNER

TABLE_HEAD = ["区域", "门店数", "平均上门（天）", "一次装成率", "年内复购"]
TABLE_ROWS = [
    ("华东", "186", "2.4", "91%", "34%", False),
    ("华南", "142", "3.1", "88%", "29%", False),
    ("华北", "128", "4.6", "74%", "18%", False),
    ("华中", "96", "5.2", "69%", "15%", False),
    ("西南", "74", "6.8", "61%", "11%", False),
    ("合计", "626", "4.1", "78%", "22%", True),
]


def table_row(cells, *, size, weight, color, head=False):
    kids = []
    for index, (value, width) in enumerate(zip(cells, TABLE_COLS)):
        first = index == 0
        kids.append(text(ids, "表头单元" if head else "表格单元", value, size,
                         weight, color,
                         family=CJK if first or head else NUM,
                         line_height=lh(size), width=width,
                         align=None if first else "right",
                         spacing=0 if (first or head) else
                         latin_track(value)))
    return row("表行", kids, gap=GUTTER, align="center")


def data_table():
    parts = [
        table_row(TABLE_HEAD, size=26, weight=600, color="$c-ink-soft",
                  head=True),
        # 表头下边线是这一页唯一的 accent 出现。数值一律走墨色 —— 把一整列
        # 数字染成强调色，是「用了一个强调色但它出现了十一次」那种失败。
        hrule("$c-steel", RULE),
    ]
    for index, (*cells, total) in enumerate(TABLE_ROWS):
        parts.append(table_row(cells, size=FS_TABLE,
                               weight=600 if total else 400,
                               color="$c-ink" if total else "$c-ink-soft"))
        # 末行无线。表格的最后一条线画出去，表就读作一个被框住的块而不是
        # 一列还能往下续的记录。
        if index < len(TABLE_ROWS) - 1:
            parts.append(hrule())
    return col("区域表", parts, gap=GUTTER - 6)


def data_page():
    s = slide("05 数据", gap=GUTTER * 2, justify="space_between")
    s["content"]["children"] = [
        col("论证", [title("装得快的区域，复购就没有掉过"), data_table()],
            gap=GUTTER * 2),
        footer(5, active=2,
               source="数据来源：明澜工单系统 2026 年 1–6 月，一次装成率指首次"
                      "上门即完成安装并通过验收的比例。"),
    ]
    return s


# -------------------------------------------------------------- 06 取舍页
# 两栏 7:3 的不对称。§3.6：两栏页禁止 50/50 —— 50/50 稳定但静止，不对称
# 才产生视觉方向；这一页的方向是「证据在左、判断在右」。
LEFT_W = 1140
# 三个孩子（左栏 / 中缝 / 右栏）之间有**两条**沟槽，右栏宽必须把两条都扣
# 掉。少扣一条的后果不是「差一点」，是整行宽出 24px 被判成溢出。
RIGHT_W = INNER - LEFT_W - HAIR - GUTTER * 2   # 491 ≈ 7.0 : 3.0

TRADEOFFS = [
    ("自有师傅的一次装成率是 94%",
     "外包队伍 68%。\n同样的培训，差别在返修要不要自己扛。"),
    ("改约主要卡在配件不在车上",
     "不是人手不够。\n配件调度这一段本来就在我们自己手里。"),
    ("经销商并不反对我们自己装",
     "他们要的是卖机器那笔返点，\n安装对他们一直是成本中心。"),
]


def tradeoff():
    left_items = []
    for head, desc in TRADEOFFS:
        if left_items:
            left_items.append(hrule())
        left_items.append(col("证据条", [
            text(ids, "证据标题", head, FS_LEDE, 600, "$c-ink", family=CJK,
                 line_height=lh(FS_LEDE), width=cjk_width(FS_LEDE, LEFT_W)),
            text(ids, "证据说明", desc, FS_TABLE, 400, "$c-ink-soft",
                 family=CJK, line_height=lh(FS_TABLE, body=True),
                 width=cjk_width(FS_TABLE, LEFT_W)),
        ], gap=8))

    # 赭黄整套唯一的一次。给的是一条 96×4 的短线而不是一段染色文字：警戒色
    # 压到 4.00 才换来可读性，把它花在文字上就必须再论证一次对比度；而一条
    # 实心短线不承担阅读，反而更像海图上那道浅滩警戒。
    right = col("判断栏", [
        rect(ids, "警戒短线", width=96, height=4, fill=solid("$c-ochre")),
        text(ids, "判断标题", "这一步的代价", FS_BODY, 600, "$c-ink",
             family=CJK, line_height=lh(FS_BODY),
             width=cjk_width(FS_BODY, RIGHT_W - 64)),
        text(ids, "判断说明",
             "自建安装要多背\n三年四千三百万固定成本。\n换来的是复购，\n不是当期毛利。",
             FS_TABLE, 400, "$c-ink", family=CJK,
             line_height=lh(FS_TABLE, body=True),
             width=cjk_width(FS_TABLE, RIGHT_W - 64)),
    ], gap=GUTTER, width=RIGHT_W, height="fill_container",
        padding=[32, 32], fill=solid("$c-paper-deep"))

    band = row("取舍区", [
        col("证据栏", left_items, gap=GUTTER, width=LEFT_W),
        vrule("fill_container"),
        right,
    ], gap=GUTTER, align="stretch")

    s = slide("06 取舍", gap=GUTTER * 2, justify="space_between")
    s["content"]["children"] = [
        col("论证", [title("该换的不是经销商\n是把安装收回来自己做"), band],
            gap=GUTTER * 2),
        footer(6, active=3),
    ]
    return s


# -------------------------------------------------------------- 07 行动页
ACTIONS = [
    "九月底前接管\n华南两个仓的配件调度",
    "十月起在广深\n自招一百二十名安装师傅",
    "年底复盘一次装成率，\n到不了九成就停",
]


def action():
    action_w = (INNER - GUTTER * 2 * 2) // 3
    cards = []
    for index, line in enumerate(ACTIONS, 1):
        cards.append(col(f"行动 {index}", [
            text(ids, "行动序号", f"{index:02d}", FS_ITEM, 500,
                 "$c-steel-lift", family=NUM, line_height=lh(FS_ITEM),
                 spacing=latin_track("01"), width=action_w),
            text(ids, "行动文案", line, FS_BODY, 500, "$c-on-navy",
                 family=CJK, line_height=lh(FS_BODY, body=True),
                 width=cjk_width(FS_BODY, action_w)),
        ], gap=GUTTER - 8, width=action_w))

    s = slide("07 行动", dark=True, gap=GUTTER * 2, justify="space_between")
    s["content"]["children"] = [
        text(ids, "收束句", "先把华南的安装收回来\n再谈全国", FS_CLOSE, 700,
             "$c-on-navy", family=CJK, line_height=lh(FS_CLOSE),
             spacing=track(FS_CLOSE), width=INNER),
        row("行动区", cards, gap=GUTTER * 2, align="start"),
        col("页尾", [
            text(ids, "落款", "明澜家用水处理 · 战略办公室｜2026 年 8 月 12 日",
                 26, 400, "$c-on-navy-dim", family=CJK, line_height=lh(26),
                 width=cjk_width(26, INNER)),
            footer(7, dark=True, active=4),
        ], gap=GUTTER),
    ]
    return s


# 字号地板自检（§3.4）。产稿时跑一遍比渲染完看检测器报告便宜得多。
assert_type_scale([FS_COVER, FS_TITLE, FS_CLOSE, FS_LEDE, FS_ITEM, FS_BODY,
                   FS_TABLE, FS_NOTE, FS_READOUT, 26, 28],
                  where="sounding-navy-deck")


def build():
    boards = [cover(), agenda(), conclusion(), evidence(), data_page(),
              tradeoff(), action()]
    for board in boards:
        board.pop("content", None)
    place_boards(boards)
    # 本档零圆角：`allowed_radii` 传空集，只有正圆（航迹点）能过。约束的是
    # 圆角矩形容器 —— 圆点要圆就必须写 cornerRadius，那不是同一件事。
    return audit_document(boards, where="sounding-navy-deck",
                          allowed_radii=frozenset())


# 对比度（WCAG 相对亮度比，实算）。deck 是投影物，自设门槛远高于
# op-design-lint 的 2.5：
#   c-ink        on c-paper       14.58     c-ink-soft on c-paper      6.24
#   c-ink-faint  on c-paper        3.58  ← 只承载 ≥24px 的脚注/页码/图注
#   c-ink        on c-paper-deep  13.34     c-ink-soft on c-paper-deep 5.71
#   c-on-navy    on c-navy        15.27     c-on-navy-dim on c-navy    8.46
#   c-steel-lift on c-navy         7.36     c-steel    on c-paper      4.97
#   c-steel-deep on c-paper        9.24     c-ochre    on c-paper      4.00
# `c-ochre` 的 4.00 是刻意压出来的：初版取 L0.700 时对纸只有 2.47，一个读不清
# 的「警戒色」是自相矛盾。本档它只用在一条实心短线上，不承担阅读。
# `c-rule` 与剖面基线是**线不是字**（§4.4 对比豁免第 2 条），不参与对比核算。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "测深海图 · 咨询策略档 16:9")
