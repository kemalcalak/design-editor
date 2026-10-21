#!/usr/bin/env python3
"""banxin-rule-deck.op — 版心界行 · 中文原生排版档（7 页 1920×1080）

spec: openpencil-docs/openpencil/generation/deck-system-0809.md §2 · D9
落格：medium-high formality × medium-high density × **mixed**（深浅书挡）

这一档跟仓里其它 deck 的分野只有一句话：**中文正文本身是主角**。所以它
不是「中国风模板」——没有水墨、没有印章、没有回纹。身份全部来自版面几何：

  版心   一页里可以落墨的那块地，天头 144 / 地脚 108，**上大于下**
  界行   分栏用的 1px 竖线，贯穿固定高度，不随文字长短伸缩
  天头   批注带（注疏层），正文之外的第三层文本层级
  鱼尾   地脚居中的页码标记，每页复现，是跨页恒定的身份锚

### 三条硬规则（这一档的命）

1. **正文每行 ≤30 汉字。** 1680 的版心在 32px 下是 52 字/行，远超舒适区，
   所以正文块一律收窄到 960（=30 字）或分两栏 —— 「界行」这个母题存在的
   功能理由就在这里，它不是装饰。
2. **中文标题绝不用省略号截断，也绝不交给自动折行。** 所有断点在文案里写
   死 `\\n`；`cjk_text()` 会在产稿时逐行断言字数没有越过该块的容量，越了
   直接 AssertionError，而不是等 QA 闸去抓。
3. **行高按字号分段，不按「标题/正文」分类**（spec §3.5 / §5.3 C1）：
   ≥64 → 1.02-1.15；48-63 → 1.15-1.25；40-47 → 1.3-1.4；正文 → 1.7-1.8。
   字距同理（§5.3 C2）：<48 一律 0 绝不为负；≥64 才允许 round(fs×-0.02)。

### 工程约束（spec §6，逐条对应过一次真实事故）

- 6.1 `dashPattern` 是死字段 —— 本档零虚线，界行全部实线。
- 6.2 相邻同色块之间要有真格线 —— 表格行靠斑马 + 1px 界行，且行内单元格
  之间给 24 的真 gap（`SIBLING_JAM_GAP = 3` 的余量），不靠 padding 撑。
- 6.3 `line` 在 layout:none 下吃文档绝对坐标 —— 界行、朱砂短线、表格线全部
  用 `rectangle` 画，一根 `line` 都没有。
- 6.4 layout:none 的 children[0] 在最上 —— 每页先写内容、后写装饰。
- 6.9 字体两层，产稿一律用保底层：`Noto Sans SC` + `Inter`。首选的源流明体
  / 霞鹜文楷不在渲染字体包内，写了会静默回退并触发「量 A 画 B」的裁切。
- 6.13 rectangle 不递归渲染子节点 —— 引文块、朱砂淡底块一律 `frame`。
- 6.15 板位 x = (i%3)*2040、y = (i//3)*1440。

### 深浅书挡

深底只在 01/02（开头）与 07（结尾）出现，中间四页全亮底，绝不交替
（spec §2 D9 Strictly avoid 8）。

对比度实测见文件末尾，最低一对 3.72（`ink.faint`，只承载 ≥24px 的批注/
页码，属 spec §4.4 的合法例外）。
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import Ids, color_vars, frame, path, rect, solid, text, write_doc

ids = Ids()

# ---------------------------------------------------------------- 色板
#
# spec §2 D9 的 oklch 表逐条换算而来（hex 由 spec 给出，未再自行取整）。
# 冷暖分裂是这一档的物理关系：纸（H86-88 暖）对墨（H60 暖）对函套（H60 暖）
# —— 全暖向，因为纸面反射是暖的，纯黑写在暖白纸上会「浮」起一层。
VARS = color_vars({
    "c-xuan":        "#F3F0E9",  # 宣白 · 内容页底
    "c-xuan-deep":   "#E5E1D7",  # 引文块 / 表格斑马
    "c-case":        "#1E1813",  # 函套墨 · 封面/篇目/收束页底
    "c-ink":         "#2A241F",  # 主文字
    "c-ink-soft":    "#5F5A55",  # 次级文字
    "c-ink-faint":   "#807A76",  # 批注 / 页码（仅 ≥24px）
    "c-xuan-on":     "#F0ECE7",  # 深页主文字
    "c-xuan-dim":    "#B4B0AA",  # 深页次级文字
    "c-jie":         "#D0CBC5",  # 界行线（非文字）
    "c-cinnabar":    "#AA4331",  # 唯一强调：鱼尾 / 页码标记 / 单页一处
    "c-cinnabar-wash": "#F9D7CE",  # 朱砂淡版（承载墨字）
})

W, H = 1920, 1080

# 天头 144 / 地脚 108，比例 1.33:1。古籍如此是为了留批注；我们保留它有两个
# 当代理由 —— 投影下沿常被人头遮挡，且上下不对称当场打破「万物居中且四边
# 等距」这条 AI 指纹（spec §3.7 第 8 条）。
HEAD, FOOT, SIDE = 144, 108, 120
BANXIN_TOP, BANXIN_BOTTOM = HEAD, H - FOOT          # 144 .. 972
INNER = W - SIDE * 2                                # 1680

# 界行的固定跨度。**不随文字长短伸缩**是这个母题的定义性质，所以它是常量
# 而不是算出来的：从页标题下方一格起，到地脚上沿止，每一页同一段。
RULE_TOP, RULE_BOTTOM = 264, BANXIN_BOTTOM
RULE_H = RULE_BOTTOM - RULE_TOP                     # 708

# 正文行长闸：30 汉字。所有正文块宽度由它反推，不由容器宽度决定。
MAX_HAN_PER_LINE = 30
BODY_FS = 32
BODY_W = MAX_HAN_PER_LINE * BODY_FS                 # 960

CJK = "Noto Sans SC"
NUM = "Inter"


# ---------------------------------------------------------------- 字阶
#
# (fontSize, lineHeight, letterSpacing)。分段规则见模块 docstring 第 3 条。
COVER = (96, 1.08, -2)      # round(96 × -0.02) = -2，不得更负
CLOSE = (64, 1.12, -1)      # round(64 × -0.02) = -1
TITLE = (56, 1.20, 0)       # 48-63 档，字距 0
LEAD = (40, 1.35, 0)        # 篇名 / 栏题
BODY = (BODY_FS, 1.75, 0)   # 正文，CJK 1.7-1.8
QUOTE = (36, 1.80, 0)       # 引文
CELL = (28, 1.70, 0)        # 表格正文
NOTE = (26, 1.70, 0)        # 批注 / 旁注 / 出处
FOLIO = (24, 1.20, 0)       # 页码 / 落款 mono


def th(step, lines=1):
    """一个字阶排 n 行的渲染高度。jian 量文字高度就是 fs × lh × 行数，
    绝对定位的每一块都靠它反推 y —— 这一档没有一处靠「试出来」的坐标。"""
    size, line_height, _ = step
    return round(size * line_height * lines)


# ---------------------------------------------------------------- 文本
def cjk_text(name, content, step, weight, color, *, inner, family=CJK,
             align=None, width="fill_container"):
    """一个中文文本节点，**产稿时就把行长闸执行掉**。

    `inner` 是这个节点真正能占的像素宽（父块宽减 padding）。断言按 QA 闸
    `qa/cjkcheck.py` 的同一个模型来算：每字 1em、逐字符断行、每行容量
    `inner // fontSize`。只要每一条硬换行都不越容量，引擎就不会再产生任何
    「软折行」，行首标点与孤字末行两类事故因此在源头就不可能发生 ——
    QA 闸是复核，不是防线。

    注意 cjkcheck 只检查汉字占比 ≥0.6 的内容（拉丁半宽，按 1em 算是假阳
    性），所以断言也照同一个门控走。
    """
    size, line_height, spacing = step
    per = int(inner // size)
    han = sum(1 for ch in content if "一" <= ch <= "鿿")
    body = content.replace("\n", "")
    if body and han / len(body) >= 0.6:
        for line in content.split("\n"):
            assert len(line) <= per, (
                f"{name}: 「{line}」{len(line)} 字 > 该块容量 {per} 字"
                f"（inner={inner} fs={size}）—— 改文案或收窄行长，不许缩字号"
            )
    # `fit_content` 的节点是收缩包裹的，必须配 `auto` —— 配 fixed-width 会
    # 让 jian 把它压到比内容还窄（实测 9 字 24px 的落款被压到 188px），文字
    # 于是折出一个「评议」这样的孤字末行。
    return text(ids, name, content, size, weight, color, family=family,
                line_height=line_height, spacing=spacing, width=width,
                growth="auto" if width == "fit_content" else "fixed-width",
                align=align)


def mono(name, content, step, weight, color, *, width="fit_content",
         align=None, spacing=None):
    """数字与拉丁一律走西文族（spec §3.8 第 5 条）：让中文字体去渲染数字，
    字宽不齐，跨行跨列的列就会抖。"""
    size, line_height, default_spacing = step
    return text(ids, name, content, size, weight, color, family=NUM,
                line_height=line_height,
                spacing=default_spacing if spacing is None else spacing,
                width=width, growth="auto" if width == "fit_content" else "fixed-width",
                align=align)


# ---------------------------------------------------------------- 结构
def page(name, bg):
    """一页。固定 1920×1080、layout:none —— 版心档的每一块都要落在算得出
    的坐标上，交给 flex 排就等于把版心交出去了。"""
    node = frame(ids, name, width=W, height=H, layout="none",
                 fill=solid(bg), clipContent=True)
    node["children"] = []
    return node


def block(name, x, y, width, children, *, gap=24, padding=None, fill=None,
          align="start"):
    """版心里的一块。height 恒为 fit_content —— 写死高度会在文案改一个字时
    悄悄裁切。"""
    node = frame(ids, name, width=width, height="fit_content",
                 layout="vertical", gap=gap, alignItems=align,
                 fill=fill or [])
    if padding:
        node["padding"] = padding
    node["x"], node["y"] = x, y
    node["children"] = children
    return node


def row(name, x, y, width, children, *, gap=24, padding=None, fill=None,
        align="center", justify=None):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="horizontal", gap=gap, alignItems=align,
                 fill=fill or [])
    if padding:
        node["padding"] = padding
    if justify:
        node["justifyContent"] = justify
    if x is not None:
        node["x"], node["y"] = x, y
    node["children"] = children
    return node


def jie_rule(x, *, y=RULE_TOP, height=RULE_H, color="$c-jie"):
    """界行。1px 竖线，**贯穿固定高度**。

    这是与「卡片分栏」的根本区别：卡片框住内容（框随内容长短变形），界行
    标记版面（线的位置与长度是版面的属性，与内容无关）。
    """
    node = rect(ids, "界行", width=1, height=height, fill=solid(color))
    node["x"], node["y"] = x, y
    return node


def hair(x, y, width, *, thickness=1, color="$c-jie"):
    """横向界行 / 表格线。用 rectangle 不用 line —— `line` 在 layout:none 下
    吃的是文档绝对坐标，会跑到画布另一个角落去（spec §6.3）。"""
    node = rect(ids, "界行 · 横", width=width, height=thickness,
                fill=solid(color))
    node["x"], node["y"] = x, y
    return node


# ---------------------------------------------------------------- 鱼尾
#
# 古籍版心中缝的页码标记。两个等腰三角尖端相对、留一道缝 —— 原创造型，
# 不画鱼、不画任何具象物（spec §2 D9 Strictly avoid 1）。
# 每页复现且位置零漂移，它是这一档跨页恒定的身份锚。
FOLIO_W, FOLIO_TRI_H, FOLIO_GAP = 20, 9, 4


def fish_tail(x, y, *, scale=1):
    """鱼尾本体。`path` 的 d 会被拉伸到节点框，所以坐标写 1:1 即可。"""
    w = round(FOLIO_W * scale)
    tri = round(FOLIO_TRI_H * scale)
    gap = round(FOLIO_GAP * scale)
    upper = path(ids, "鱼尾 · 上", f"M0 0 L{w} 0 L{w / 2} {tri} Z",
                 width=w, height=tri, fill=solid("$c-cinnabar"))
    upper["x"], upper["y"] = x, y
    lower = path(ids, "鱼尾 · 下", f"M{w / 2} 0 L{w} {tri} L0 {tri} Z",
                 width=w, height=tri, fill=solid("$c-cinnabar"))
    lower["x"], lower["y"] = x, y + tri + gap
    return [upper, lower]


def folio_mark(index, total, *, dark=False):
    """地脚居中的鱼尾 + 页码。深页换字色，位置一格不动。"""
    color = "$c-xuan-dim" if dark else "$c-ink-faint"
    nodes = fish_tail(round((W - FOLIO_W) / 2), 996)
    number = mono("页码", f"{index} / {total}", FOLIO, 400, color,
                  width=200, align="center")
    number["x"], number["y"] = round((W - 200) / 2), 1026
    nodes.append(number)
    return nodes


# ================================================================ 01 封面
def cover():
    """函套。深底、左下重心 —— 整套七页里唯一一个「左下」重心，
    与 07 的居中、03 的左偏、05 的居中收束共同构成跨页的重心变化。"""
    p = page("01 封面 · 函套", "$c-case")
    title = cjk_text("主标题", "汉字排版\n在屏幕上的四次退让", COVER, 700,
                     "$c-xuan-on", inner=INNER)
    p["children"].append(block("主标", SIDE, 430, INNER, [title]))

    # 整套封面唯一的朱砂。96×2，短到不可能被读成分隔线，实到不可能被读成
    # 4% 不透明度的「假装在设计」（spec §3.7 第 5 条）。
    mark = rect(ids, "朱砂短线", width=96, height=2,
                fill=solid("$c-cinnabar"))
    mark["x"], mark["y"] = SIDE, 685
    p["children"].append(mark)

    p["children"].append(block("副标", SIDE, 725, INNER, [
        cjk_text("副标题", "写给编辑与界面设计者的一份版面评议", BODY, 400,
                 "$c-xuan-dim", inner=INNER),
    ]))
    p["children"].append(row("落款", SIDE, 880, INNER, [
        cjk_text("落款 · 单位", "版面组 · 内部评议", FOLIO, 400,
                 "$c-xuan-dim", inner=400, width="fit_content"),
        mono("落款 · 日期", "2026.08.09", FOLIO, 400, "$c-xuan-dim"),
    ], gap=32))
    p["children"] += folio_mark(1, 7, dark=True)
    return p


# ================================================================ 02 篇目
CHAPTERS = [
    ("01", "版心：先给一页定出可以落墨的地方"),
    ("02", "界行：分栏是为了缩短行，不是分盒子"),
    ("03", "行长：三十个字是一道生理线"),
    ("04", "标点：断行处最见功夫"),
    ("05", "收束：把四次退让逐项收回"),
]


def contents():
    """篇目。条与条之间只有一根 1px 界行，没有卡片、没有底色块 ——
    目录如果需要一张卡片来分组，说明这份材料的结构本身没想清楚。"""
    p = page("02 篇目 · 函套", "$c-case")
    p["children"].append(block("眉题", SIDE, HEAD, INNER, [
        cjk_text("眉题", "篇目", BODY, 500, "$c-xuan-dim", inner=INNER),
    ]))

    items = []
    for index, (no, name) in enumerate(CHAPTERS):
        if index:
            items.append(rect(ids, "界行 · 条间", width="fill_container",
                              height=1, fill=solid("$c-jie")))
        items.append(row(f"篇目 {no}", None, None, "fill_container", [
            mono("序号", no, FOLIO, 400, "$c-xuan-dim", width=96),
            cjk_text("篇名", name, LEAD, 600, "$c-xuan-on",
                     inner=INNER - 96 - 32),
        ], gap=32, padding=[36, 0]))
    # 行高 36+54+36 = 126，五条加四根界行 = 634，从 264 起正好收在 898 ——
    # 篇目页的高度是算出来的：加一条就会顶出版心，那正是「该拆页」的信号。
    p["children"].append(block("篇目列表", SIDE, RULE_TOP, INNER, items, gap=0))
    p["children"] += folio_mark(2, 7, dark=True)
    return p


# ============================================================ 03 论述页
def discourse():
    """单栏正文 + 界行右侧旁注。62:38 的不对称分栏 —— 50/50 稳定但静止
    （spec §3.6）。正文块 960 = 30 字，是行长闸算出来的，不是版心宽度。"""
    p = page("03 论述 · 版心", "$c-xuan")
    p["children"].append(block("天头批注", SIDE, 56, INNER, [
        cjk_text("批注", "版心先于版式；边界不定，谈不上呼吸", NOTE, 400,
                 "$c-ink-faint", inner=INNER),
    ]))
    p["children"].append(block("页标题", SIDE, HEAD, INNER, [
        cjk_text("标题", "版心不是边距，是一页里可以落墨的那块地", TITLE,
                 700, "$c-ink", inner=INNER),
    ]))
    p["children"].append(block("正文", SIDE, RULE_TOP, BODY_W, [
        cjk_text("正文", "\n".join([
            "屏幕上的版面把版心简化成了四边等距的内边距，",
            "于是每一页都从同一个位置开始，也在同一处结束。",
            "古籍的做法相反：天头留给批注，地脚留给页码，",
            "两者从来不等高，页面因此有了上下方向。",
            "这一档把天头定为 144、地脚定为 108，",
            "多出来的那一截，是给投影时被人头遮住的下沿的。",
        ]), BODY, 400, "$c-ink", inner=BODY_W),
    ]))
    # 正文之下的三条做法。用横向界行与正文隔开，而不是另起一张卡片 ——
    # 同一页里的两层内容，靠线与留白分层就够了。
    p["children"].append(hair(SIDE, 648, BODY_W))
    clauses = []
    for index, clause in enumerate([
        "天头留给批注，地脚留给页码与鱼尾",
        "正文块收窄到三十字，宁可分栏也不放宽",
        "界行的长度由版面定，与文字多少无关",
    ]):
        clauses.append(row(f"做法 {index + 1}", None, None, "fill_container", [
            mono("序号", f"0{index + 1}", FOLIO, 400, "$c-ink-faint", width=56),
            cjk_text("做法", clause, BODY, 400, "$c-ink",
                     inner=BODY_W - 56 - 24),
        ], gap=24, padding=[14, 0]))
    p["children"].append(block("三条做法", SIDE, 672, BODY_W, clauses, gap=0))

    p["children"].append(block("旁注", 1200, RULE_TOP, 600, [
        cjk_text("旁注", "\n".join([
            "版心之外的留白不是废纸。",
            "天头承批注，地脚承页码，",
            "两者各有职务在身，",
            "所以不必、也不该等高。",
        ]), NOTE, 400, "$c-ink-soft", inner=600),
    ]))
    p["children"].append(jie_rule(1140))
    p["children"] += folio_mark(3, 7)
    return p


# ============================================================ 04 双栏对读
def dialogue():
    """其说 / 我见。两栏等宽、中缝一根界行 —— 这一页是「界行」这个母题的
    正面论证：把同一件事的两种说法并排，靠线分，不靠卡片分。"""
    p = page("04 双栏对读", "$c-xuan")
    p["children"].append(block("页标题", SIDE, HEAD, INNER, [
        cjk_text("标题", "分栏是为了把行缩短，不是把内容装进两个盒子",
                 TITLE, 700, "$c-ink", inner=INNER),
    ]))

    col_w = 808
    p["children"].append(block("左栏 · 其说", SIDE, RULE_TOP, col_w, [
        cjk_text("栏题", "其说", LEAD, 600, "$c-ink-soft", inner=col_w),
        cjk_text("左栏正文", "\n".join([
            "通行的做法是给每一栏加一个",
            "圆角卡片，再用底色把两边分开。",
            "分是分开了，读者读到的却是",
            "两张卡片，而不是一篇文章的",
            "两个部分。",
        ]), BODY, 400, "$c-ink", inner=col_w),
        cjk_text("左栏续", "\n".join([
            "卡片还有一个副作用：它必须有内边距，",
            "于是同一块版心里，能排的字更少，",
            "行却没有因此变短。",
        ]), BODY, 400, "$c-ink-soft", inner=col_w),
    ], gap=32))

    # 右栏首句压朱砂淡底 —— 本页唯一一次强调。第二次出现就不再是强调
    # （spec §3.6），所以整页别处一点朱砂都没有。
    wash = block("首句", 0, 0, "fill_container", [
        cjk_text("首句", "界行只标记分栏，不圈占内容。", BODY, 500,
                 "$c-ink", inner=col_w - 48),
    ], padding=[24, 24], fill=solid("$c-cinnabar-wash"))
    wash.pop("x"), wash.pop("y")
    p["children"].append(block("右栏 · 我见", SIDE + col_w + 64, RULE_TOP,
                               col_w, [
        cjk_text("栏题", "我见", LEAD, 600, "$c-ink-soft", inner=col_w),
        wash,
        cjk_text("右栏正文", "\n".join([
            "线画在版面上，卡片扣在内容上。",
            "界行贯穿整个版心，不随文字长短",
            "伸缩；它分的是版面，不是归属。",
        ]), BODY, 400, "$c-ink", inner=col_w),
        cjk_text("右栏续", "\n".join([
            "把分栏交给线之后，栏宽就能完全由",
            "行长决定：三十字一行，多出来的宽度",
            "留给留白，而不是留给边框。",
        ]), BODY, 400, "$c-ink-soft", inner=col_w),
    ], gap=32))
    p["children"].append(jie_rule(960))
    p["children"] += folio_mark(4, 7)
    return p


# ============================================================== 05 引文页
def quotation():
    """引文居中，左右各一根界行框住。整套唯一一页居中重心的亮页 ——
    「不许页页居中」的另一面是「也不许一页都不居中」，节奏靠对比产生。"""
    p = page("05 引文", "$c-xuan")
    p["children"].append(block("天头批注", SIDE, 56, INNER, [
        cjk_text("批注", "回行的距离，是版面替眼睛付的成本", NOTE, 400,
                 "$c-ink-faint", inner=INNER),
    ]))
    p["children"].append(block("页标题", SIDE, HEAD, INNER, [
        cjk_text("标题", "一行超过三十个汉字，眼睛就会在回行时丢位置",
                 TITLE, 700, "$c-ink", inner=INNER),
    ]))

    # 引文块落在版心的下二分之一而不是紧贴标题：这一页只有一个读点，
    # 上下留白大致相等时它才读作「被托住」，而不是「掉在标题下面」。
    quote_w, quote_pad = 1080, 64
    p["children"].append(block("引文块", round((W - quote_w) / 2), 372,
                               quote_w, [
        cjk_text("引文", "\n".join([
            "行的长度决定读者每次回行要走多远。",
            "走得太远，眼睛会落到上一行或下一行，",
            "于是要重读；重读不是理解慢，是版面失职。",
        ]), QUOTE, 400, "$c-ink", inner=quote_w - quote_pad * 2),
    ], padding=[quote_pad, quote_pad], fill=solid("$c-xuan-deep")))

    p["children"].append(block("出处", round((W - quote_w) / 2), 732,
                               quote_w, [
        cjk_text("出处", "——《版面与阅读》第二章 · 行长与回行", NOTE, 400,
                 "$c-ink-soft", inner=quote_w, align="right"),
    ]))
    p["children"].append(jie_rule(360))
    p["children"].append(jie_rule(1560))
    p["children"] += folio_mark(5, 7)
    return p


# ============================================================== 06 表列页
# 列宽写死（spec §6.10）：渲染栈没有 tabular 数字，靠 fit_content 指望列
# 自己对齐，一行长一点整张表就抖。
TABLE_COLS = (280, 388, 376, 532)
TABLE_PAD = 16
TABLE_GAP = 24          # ≥ SIBLING_JAM_GAP(3) 的真 gap，不靠 padding 撑
TABLE_HEAD = ("现象", "当前引擎行为", "后果", "我们的做法")
TABLE_ROWS = [
    ("行首标点", "逐字断行，不避头尾", "「。」被甩到行首", "标题写死断点，正文控字数"),
    ("孤字末行", "末行只剩一两个字", "版面出现残行", "收窄栏宽，或改写文案"),
    ("中英混排", "两侧不自动加间隙", "数字与汉字粘连", "文案里补半角空格"),
    ("数值对齐", "没有等宽数字", "列与列之间发抖", "数字走西文族，列宽写死"),
    ("省略号截断", "直接截去后半句", "读者读到半截话", "不截断，改写或换页型"),
]


def table_row(name, cells, *, weight, color, fill=None):
    kids = []
    for width, content in zip(TABLE_COLS, cells):
        kids.append(cjk_text(f"{name} · 格", content, CELL, weight, color,
                             inner=width, width=width))
    return row(name, None, None, "fill_container", kids, gap=TABLE_GAP,
               padding=[20, TABLE_PAD], fill=fill, align="start")


def table_page():
    """表格。表头 2px 墨线、行间 1px 界行、**末行无线**、斑马走宣深。
    整框边线一根没有 —— 表格的结构应该由横线与留白给出，不由一个框给出。"""
    p = page("06 表列", "$c-xuan")
    p["children"].append(block("天头批注", SIDE, 56, INNER, [
        cjk_text("批注", "避头尾在引擎里缺席，只能由文案补上", NOTE, 400,
                 "$c-ink-faint", inner=INNER),
    ]))
    p["children"].append(block("页标题", SIDE, HEAD, INNER, [
        cjk_text("标题", "断行处最见功夫，而排版引擎并不替我们避头尾",
                 TITLE, 700, "$c-ink", inner=INNER),
    ]))

    rows = [table_row("表头", TABLE_HEAD, weight=600, color="$c-ink"),
            rect(ids, "表头线", width="fill_container", height=2,
                 fill=solid("$c-ink"))]
    for index, cells in enumerate(TABLE_ROWS):
        if index:
            rows.append(rect(ids, "界行 · 行间", width="fill_container",
                             height=1, fill=solid("$c-jie")))
        rows.append(table_row(f"表行 {index + 1}", cells, weight=400,
                              color="$c-ink",
                              fill=solid("$c-xuan-deep") if index % 2 else None))
    p["children"].append(block("表格", SIDE, RULE_TOP, INNER, rows, gap=0))

    p["children"].append(block("来源", SIDE, 848, INNER, [
        cjk_text("来源", "来源：版面组对内部三十份中文文档的抽样，"
                 "2026 年 7 月", FOLIO, 400, "$c-ink-faint", inner=INNER),
    ]))
    p["children"] += folio_mark(6, 7)
    return p


# ============================================================== 07 收束
def closing():
    """函套收尾。放大的鱼尾在收束句之上 —— 页尾那枚小鱼尾照旧在地脚，
    位置零漂移；放大的这一枚是构图，不是页码。"""
    p = page("07 收束 · 函套", "$c-case")
    p["children"] += fish_tail(round((W - FOLIO_W * 4) / 2), 300, scale=4)
    p["children"].append(block("收束句", SIDE, 470, INNER, [
        cjk_text("收束", "把四次退让逐项收回来，\n从一行三十字开始。",
                 CLOSE, 700, "$c-xuan-on", inner=INNER, align="center"),
    ]))
    p["children"].append(row("落款", SIDE, 700, INNER, [
        cjk_text("落款 · 单位", "版面组 · 内部评议", FOLIO, 400,
                 "$c-xuan-dim", inner=400, width="fit_content"),
        mono("落款 · 日期", "2026.08.09", FOLIO, 400, "$c-xuan-dim"),
    ], gap=32, justify="center"))
    p["children"] += folio_mark(7, 7, dark=True)
    return p


# ---------------------------------------------------------------- 板位
# spec §6.15：3 板一行；行距 1440 而不是 1200 —— 画布在帧上方以固定屏幕
# 偏移画帧名，行距不够时第二行的帧名会压到上一行的板上。
BOARD_X, BOARD_Y, PER_ROW = 2040, 1440, 3


def build():
    boards = [cover(), contents(), discourse(), dialogue(), quotation(),
              table_page(), closing()]
    for index, board in enumerate(boards):
        board["x"] = (index % PER_ROW) * BOARD_X
        board["y"] = (index // PER_ROW) * BOARD_Y
    return boards


# 对比度（WCAG 相对亮度比，spec §2 D9 色板表实算值）：
#   c-ink        on c-xuan   13.46
#   c-ink-soft   on c-xuan    5.99
#   c-ink-faint  on c-xuan    3.72  ← 仅承载 ≥24px 的批注/页码/来源
#   c-ink        on c-xuan-deep 11.73（引文块、斑马行）
#   c-ink        on c-cinnabar-wash 11.41（04 页首句）
#   c-xuan-on    on c-case   14.94
#   c-xuan-dim   on c-case    8.14  ← 深页次级文字与页码
#   c-cinnabar   on c-xuan    5.19（本档朱砂只作图形与短线，不承载正文）
#   c-jie 只画线不承载文字，按 spec §4.4 第 2 条豁免（非文字装饰线）。
#
# 字号地板（spec §3.4）：最小 24（批注/页码/来源，全部 ≥24 的合法例外区），
# 最大 96 ≥ 60 的层级线，且 96 / 32 = 3.0 ≥ 2.5 倍的层级下限。
# 单页字阶档数：01=3 / 02=3 / 03=4 / 04=4 / 05=4 / 06=4 / 07=2，均 ≤4。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "版心界行 · 中文原生排版档")
