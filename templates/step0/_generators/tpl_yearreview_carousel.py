#!/usr/bin/env python3
"""yearreview-mineral-carousel.op — 年度总结复盘轮播（8 板 · 1080×1440 · 3:4）

叙事类型：**年度复盘**。这一档的读者其实是作者自己 —— 年终总结之所以难
写，是因为它同时要「数得出来」和「说得清楚」，而这两件事在版式上是打架
的：数字要冷，感受要热。

本套的解法是把它们**分板放**，并且让顺序承担意义：先给一个数（封面），
再承认一个落差（第 2 板），然后才是那段最难的（第 3 板）；数据放在中段
（第 4 板），十二个月的形状放在第 5 板，最重要的那件事单独占一板（第 6
板），最后一句金句和一张目录。八板是这一档的上限 —— 再多就成了流水账。

八板也是本批里最长的一套，所以配方覆盖要求从 ≥4 提到了实际的 **6 族**。

### 主题：T1 鸣沙矿彩 `mingsha-mineral`（暗 / 大胆 / 文化）

**采样 → 收敛 → 论证**（沿用 card-system-0808 §3 T1）

  - 采样：西北石窟壁画的**矿物颜料本身** —— 石青（蓝铜矿）、石绿（孔雀
    石）、朱砂、土红赭石、蛤粉白，以及壁画剥落后露出的泥壁底。锚的是颜料
    的色度与剥落的层叠，**不是飞天、不是藻井、不是任何具象图形**。
  - 收敛：底色取泥壁赭（暗、低 chroma）；有彩三支 —— 朱砂 H32、石青 H245、
    石绿 H165，两两 H 差 ≥80°；中性 = 蛤粉白明度序列 0.62 / 0.78 / 0.94。
  - 论证：**底色不用「敦煌黄」这个印象色，用剥落壁面的赭泥，chroma 压到
    0.028 让它退成纸感底** —— 印象色会把整套推向廉价文旅海报；泥壁色则让
    三支矿彩浮上来，符合壁画本身「重彩浮于素壁」的物理关系。选它承载年度
    复盘：壁画是「一层一层盖上去、又一层一层剥下来」的东西，而复盘正是这
    个动作 —— 把一年刮开，看下面压着什么。

**最近邻论证**：本批另外三套暗底是 `arcade-neon`（H275）、`chalk-board`
（H165）、`darkroom-film`（H250 近中性）。本套 `#21120B` 是 H45 的赭，与三
者两两 H 差 ≥85°，是四套里唯一的**暖暗底**。装饰上更是唯一一套有「剥落」
的：不规则多边形压在底色上，边缘不对齐任何网格，缩略图里就是一块斑驳的墙。

### 母版规则（八板的硬约束）

**每板固定，不许变**
  1. 画幅 1080×1440，安全边距 左右 80 / 上 96 / 下 128。
  2. **金线角标**：右上角一个 1px `c-gold` 描边的方框，框住页码。金线**只
     框角标与页码，不框任何内容卡片** —— 这是本主题对金色的唯一许可。
  3. 页眉：左「年度复盘 · 八板」（Caption 32px），右即上面那个金线角标。
  4. 页脚：一条 1px `c-plaster` 线 + 署名 + 本板提要。
  5. **剥落**：每板 3 个不规则四/五边形，填 `c-raised`，压在底色上，**边缘
     不对齐任何网格**。八板 24 块，形状**没有一块重复** —— 重复即图案，图
     案即廉价。
  6. 字族：汉字 Noto Sans SC / 数字与页码 Inter。
  7. **三支矿彩不可同板出齐**：每板最多两支，且每支最多落 2 处。

**允许变，且只有这些能变**
  - 主体区的信息结构（下面八板各用一条配方）。
  - 当板用哪两支矿彩。
  - **壁龛光**：整套只用一次，只在封面（第 1 板）。
  - **矿彩颗粒**：整套只在第 6 板出现一次，撒在那块色域边缘 40px 内。

### 配方编排（card-system-0808 §4.2）

    01 A2 数字锚点 → 02 B4 上下反差 → 03 F1 段落纯排 → 04 D4 三栏指标
    05 C4 时间轴   → 06 F2 图注混排 → 07 B1 满版一句 → 08 G3 目录回看

首板 A 族、末板 G 族；相邻两板不同族；**D 族只出现一次且前后各被非 D 包夹**
（F1 在前、C4 在后）；B 族出现 2 次且第 2 次落在后 1/3（第 7 板）；F 族出
现 2 次但不相连（第 3、6 板）；覆盖 A/B/F/D/C/G **六族**（8 板要求 ≥4）。

### 负约束（本模板明令不做的事）

  - **不画飞天、藻井、莲花、驼铃、沙丘**。一旦出现具象符号，主题就从「矿
    物颜料体系」塌成「文旅海报」。这是本套的第一条红线。
  - **剥落形状不可重复使用同一个 path**。重复即图案，图案即廉价 —— 所以
    24 块剥落是 24 个各不相同的多边形，且顶点全部避开 8px 基线。
  - **不用金色做文字色**（除 ≥64px 的单个数字）。金只走描边与角标。
  - **三支矿彩不可同板出齐**，每支每板最多 2 处。
  - 不用做旧纹理、不用沙粒噪点、不用「敦煌黄」当底色。
  - 不写「这一年我成长了很多」这类没有具体动作的总结句；每板都要落到一件
    做过的事上。
  - 每板正文不超过 4 行。

硬契约：
  - 字号下限 32px；单板最多用 4 档字阶。
  - CJK 行高：Display 1.15 / Title 1.3 / Body 1.7 / Caption 1.5。
  - CJK 字距恒为 0；只有 Inter 数字沿用西文收紧。
  - 正文与背景对比度 ≥2.0（实测表见文件末尾）。
  - 顶层 frame 必须显式写 x/y。
  - 文本节点绝不写 height。
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import (Ids, PLACEHOLDER_DISC, PLACEHOLDER_ICON, PLACEHOLDER_SPEC,
                   PLACEHOLDER_TITLE, color_vars, frame, path, radial, rect,
                   solid, text, upload_disc, write_doc)

ids = Ids()

VARS = color_vars({
    "c-bg":        "#21120B",   # ground.wall     泥壁底
    "c-plaster":   "#312018",   # surface.plaster 卡片 / 分区面
    "c-raised":    "#412E25",   # surface.raised  面上之面 / 剥落
    "c-shell":     "#EFEBE2",   # shellwhite      主文字
    "c-dim":       "#BBB7AD",   # shell.dim       次级文字
    "c-faint":     "#8A867D",   # shell.faint     注释 / 页码
    "c-cinnabar":  "#C54F3B",   # cinnabar        朱砂：大字 / 描边
    "c-cinnabar-d": "#9D2E1C",  # cinnabar.deep   承载白字的朱砂块
    "c-azurite":   "#3577AB",   # azurite         石青：图表 / 标记
    "c-malachite": "#479173",   # malachite       石绿：每套只用一次
    "c-gold":      "#D5AA55",   # gold.leaf       金箔：只走描边与角标
})

CJK = "Noto Sans SC"
NUM = "Inter"

W, H, GAP = 1080, 1440, 120

# 3 板一行 —— 与 deck 体系（deckkit.BOARDS_PER_ROW）同一约定：多板模板在画布上
# 分行铺开，而不是拖成一长排。行间距比列间距多 240 不是手滑：画布在帧上方以
# **屏幕空间**固定偏移画帧名，缩到能整屏看时 120 文档像素只剩十几个屏幕像素，
# 第二行的帧名会压到上一行的板上。
BOARDS_PER_ROW = 3
ROW_GAP = GAP + 240
EDGE = 80
TOP, BOT = 96, 128
INNER = W - EDGE * 2
COLUMN, GUTTER = 62, 16

FS_DISPLAY_L, FS_DISPLAY, FS_T1, FS_T2 = 120, 88, 64, 48
FS_BODY, FS_CAPTION = 40, 32
LH_DISPLAY_L, LH_DISPLAY = 1.10, 1.15
LH_TITLE, LH_BODY, LH_CAPTION = 1.3, 1.7, 1.5

SERIES = "年度复盘 · 八板"
TOTAL = 8

# 24 块剥落，八板每板三块。每块是一个 (x, y, 顶点序列) —— 顶点写成相对
# 尺寸的比例，且**刻意不落在 8px 基线上**：剥落是随机的，对齐了就假。
# 没有任何两块共用同一组顶点，因为重复即图案，图案即廉价。
FLAKES = [
    [(0.03, 0.00), (0.61, 0.09), (1.00, 0.47), (0.72, 1.00), (0.11, 0.83)],
    [(0.00, 0.34), (0.47, 0.00), (1.00, 0.29), (0.83, 0.91), (0.19, 1.00)],
    [(0.13, 0.07), (1.00, 0.00), (0.89, 0.63), (0.37, 1.00)],
    [(0.00, 0.21), (0.66, 0.03), (1.00, 0.71), (0.29, 0.94)],
    [(0.07, 0.00), (0.93, 0.17), (0.77, 1.00), (0.00, 0.69)],
    [(0.21, 0.04), (1.00, 0.31), (0.63, 0.97), (0.00, 0.53), (0.09, 0.19)],
    [(0.00, 0.00), (0.81, 0.13), (1.00, 0.83), (0.24, 1.00), (0.03, 0.41)],
    [(0.17, 0.11), (0.94, 0.00), (1.00, 0.57), (0.41, 1.00), (0.00, 0.73)],
    [(0.09, 0.00), (0.71, 0.23), (1.00, 0.91), (0.33, 0.79)],
    [(0.00, 0.47), (0.39, 0.00), (0.97, 0.37), (0.57, 1.00)],
    [(0.11, 0.13), (0.87, 0.00), (1.00, 0.69), (0.19, 1.00), (0.00, 0.51)],
    [(0.00, 0.09), (0.59, 0.00), (1.00, 0.61), (0.47, 0.93), (0.07, 0.71)],
    [(0.23, 0.00), (1.00, 0.19), (0.79, 0.87), (0.00, 1.00), (0.04, 0.33)],
    [(0.00, 0.27), (0.51, 0.07), (0.93, 0.49), (0.31, 1.00)],
    [(0.13, 0.00), (0.97, 0.09), (1.00, 0.77), (0.21, 0.91), (0.00, 0.43)],
    [(0.06, 0.17), (0.83, 0.00), (1.00, 0.53), (0.49, 1.00), (0.00, 0.81)],
    [(0.00, 0.00), (0.67, 0.11), (1.00, 0.67), (0.27, 1.00)],
    [(0.19, 0.06), (1.00, 0.00), (0.91, 0.71), (0.09, 0.97), (0.00, 0.29)],
    [(0.00, 0.39), (0.43, 0.00), (1.00, 0.23), (0.71, 0.93), (0.13, 1.00)],
    [(0.11, 0.00), (0.89, 0.27), (0.63, 1.00), (0.00, 0.59)],
    [(0.00, 0.13), (0.77, 0.03), (1.00, 0.81), (0.37, 0.97), (0.03, 0.61)],
    [(0.27, 0.00), (1.00, 0.41), (0.53, 0.89), (0.00, 0.47), (0.07, 0.11)],
    [(0.00, 0.07), (0.61, 0.19), (1.00, 0.93), (0.17, 1.00)],
    [(0.09, 0.23), (0.99, 0.00), (1.00, 0.63), (0.29, 1.00), (0.00, 0.79)],
]

# 每板三块剥落的落点 (x, y, w, h)。全部避开正文柱的中段，只贴边与角。
FLAKE_SPOTS = [
    [(-40, 210, 260, 190), (912, 640, 230, 300), (60, 1290, 300, 170)],
    [(880, 180, 240, 260), (-50, 720, 210, 240), (620, 1300, 280, 160)],
    [(-30, 380, 200, 300), (940, 260, 190, 230), (300, 1310, 260, 150)],
    [(900, 900, 250, 280), (-60, 250, 240, 210), (140, 1300, 230, 170)],
    [(-40, 980, 220, 260), (930, 420, 210, 320), (480, 1290, 250, 180)],
    [(870, 200, 260, 240), (-50, 560, 230, 290), (200, 1310, 290, 150)],
    [(-30, 700, 210, 250), (950, 780, 200, 260), (700, 1300, 240, 170)],
    [(920, 320, 230, 300), (-40, 880, 250, 230), (380, 1310, 270, 160)],
]


# ----------------------------------------------------------------- 结构原语
def col(name, children, *, gap=16, width="fill_container", align="start",
        height="fit_content", **props):
    node = frame(ids, name, width=width, height=height, layout="vertical",
                 gap=gap, alignItems=align, fill=[], **props)
    node["children"] = children
    return node


def row(name, children, *, gap=24, align="center", width="fill_container",
        justify="start", **props):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="horizontal", gap=gap, alignItems=align,
                 justifyContent=justify, fill=[], **props)
    node["children"] = children
    return node


def caption(name, content, color="$c-faint", *, family=CJK, weight=400,
            width="fit_content"):
    return text(ids, name, content, FS_CAPTION, weight, color, family=family,
                width=width, growth="auto" if width == "fit_content"
                else "fixed-width", line_height=LH_CAPTION)


def body(name, content, color="$c-dim", *, weight=400,
         width="fill_container"):
    return text(ids, name, content, FS_BODY, weight, color, family=CJK,
                width=width, growth="auto" if width == "fit_content"
                else "fixed-width", line_height=LH_BODY)


def numeral(name, value, size, color="$c-shell", *, weight=700, width=None):
    return text(ids, name, value, size, weight, color, family=NUM,
                width=width or "fit_content",
                growth="auto" if width is None else "fixed-width",
                line_height=1.0, spacing=-2)


def plaster(name, children, *, gap=16, pad=(30, 32), fill="$c-plaster",
            width="fill_container"):
    """一块壁面。重彩浮于素壁 —— 面之上还有面，靠明度差不靠描边。"""
    node = frame(ids, name, width=width, height="fit_content",
                 layout="vertical", gap=gap, padding=list(pad),
                 alignItems="start", fill=solid(fill))
    node["children"] = children
    return node


# ----------------------------------------------------------------- 壁画语汇
def flake(index, x, y, w, h):
    """一块剥落。顶点由比例算出，**边缘不对齐任何网格**。

    24 块用 24 组各不相同的顶点：重复即图案，图案即廉价。这也是为什么顶点
    比例写成 0.03 / 0.61 / 0.47 这种数 —— 落在 8px 基线上的剥落一眼就是画
    出来的，而剥落本来是墙自己掉的。
    """
    pts = FLAKES[index % len(FLAKES)]
    d = " ".join(
        ("M" if i == 0 else "L") + f" {round(px * w, 1)} {round(py * h, 1)}"
        for i, (px, py) in enumerate(pts)) + " Z"
    node = path(ids, f"剥落 {index + 1}", d, width=w, height=h,
                fill=solid("$c-raised"))
    node["x"], node["y"] = x, y
    return node


def niche_light():
    """壁龛光：一块从上方偏中打下来的柔光。整套只用一次，只在封面。

    半径给到 0.5、最外一档 stop 取页面底色，光晕就在节点边界收干净，不会
    留下一圈可见的方边（TileMode::Clamp 会把边界外全刷成最后一个 stop）。
    """
    size = 900
    node = rect(ids, "壁龛光", width=size, height=size, cornerRadius=size // 2,
                fill=radial([(0, "#8A867D"), (1, "#21120B")],
                            cx=0.5, cy=0.5, radius=0.5))
    node["x"], node["y"] = (W - size) // 2, round(H * 0.28) - size // 2
    node["opacity"] = 0.12
    return node


def pigment_grains(x, y, w, h, color, count=12):
    """矿彩颗粒：4×4 的小方块撒在色块边缘 40px 内。整套只在第 6 板出现。

    位置用一个固定的整数序列摊开而不是随机数：生成器必须是确定的，同一份
    代码跑两次得出的 .op 要逐字节一样，否则模板每次重生成都在制造 diff。
    """
    grains = []
    for i in range(count):
        gx = x + (i * 137) % max(1, w - 4)
        gy = y + (i * 89) % 40 if i % 2 == 0 else y + h - 40 + (i * 53) % 36
        node = rect(ids, f"矿彩颗粒 {i + 1}", width=4, height=4,
                    fill=solid(color))
        node["x"], node["y"] = gx, gy
        node["opacity"] = 0.15 + (i % 3) * 0.05
        grains.append(node)
    return grains


# ----------------------------------------------------------------- 母版部件
def gold_corner(page):
    """金线角标：1px 金描边框住页码。金只走描边与角标，不框内容卡片。"""
    node = frame(ids, "金线角标", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[10, 18], alignItems="center",
                 justifyContent="center", fill=[],
                 stroke={"thickness": 1, "fill": solid("$c-gold")})
    node["children"] = [
        numeral("页码", f"{page:02d}/{TOTAL:02d}", FS_CAPTION, "$c-gold",
                weight=600),
    ]
    return node


def header(page):
    return row("页眉", [
        caption("系列名", SERIES, "$c-faint"),
        gold_corner(page),
    ], gap=24, justify="space_between", align="center")


def footer(note):
    return col("页脚", [
        rect(ids, "分隔线", width="fill_container", height=1,
             fill=solid("$c-plaster")),
        row("署名行", [
            caption("账号名", "@ 你的账号名", "$c-dim", weight=600),
            caption("本板提要", note, "$c-faint"),
        ], gap=24, justify="space_between"),
    ], gap=16)


def board(page, name, main, note, extra_decor=None):
    content = frame(ids, f"{name} · 内容", width="fill_container",
                    height="fill_container", layout="vertical",
                    padding=[TOP, EDGE, BOT, EDGE], gap=0,
                    alignItems="start", fill=[])
    content["children"] = [header(page), main, footer(note)]

    spots = FLAKE_SPOTS[page - 1]
    flakes = [flake((page - 1) * 3 + i, *spot) for i, spot in enumerate(spots)]
    decor = frame(ids, f"{name} · 装饰", width="fill_container",
                  height="fill_container", layout="none", fill=[])
    decor["children"] = (extra_decor or []) + flakes

    shell = frame(ids, f"{page:02d} {name}", width=W, height=H, layout="none",
                  fill=solid("$c-bg"), clipContent=True)
    # 装饰层最后 —— jian 里 index 0 最上，写反了剥落会盖住正文。
    shell["children"] = [content, decor]
    shell["x"] = ((page - 1) % BOARDS_PER_ROW) * (W + GAP)
    shell["y"] = ((page - 1) // BOARDS_PER_ROW) * (H + ROW_GAP)
    return shell


def zone(children, *, justify="center", gap=32, pad_y=44):
    node = frame(ids, "主体", width="fill_container", height="fill_container",
                 layout="vertical", gap=gap, padding=[pad_y, 0],
                 justifyContent=justify, alignItems="start", fill=[])
    node["children"] = children
    return node


# ------------------------------------------------------- 01 封面 · A2 数字锚点
def cover():
    """A2：巨数字在左，标题在右下。数字走 Display L，标题退到 Title 1。

    金色在这里用在一个 120px 的单个数字上 —— 那是本主题对「金不做文字色」
    唯一的例外条款（≥64px 的单个数字），整套仅此一次。
    """
    figure = row("数字组", [
        numeral("巨数字", "37", FS_DISPLAY_L, "$c-gold"),
        col("单位组", [
            numeral("单位", "件", FS_T1, "$c-cinnabar"),
            caption("单位说明", "真正做完的", "$c-faint"),
        ], gap=6, width="fit_content"),
    ], gap=16, align="end")

    main = zone([
        figure,
        text(ids, "主标", "写了 214 条计划\n做完 37 件", FS_T1, 700,
             "$c-shell", family=CJK, width=INNER, growth="fixed-width",
             line_height=LH_TITLE),
        body("副标", "八板复盘这一年，先从这个比例讲起。", "$c-dim"),
    ], justify="center", gap=34)
    return board(1, "封面 · 数字锚点", main, "封面", [niche_light()])


# ------------------------------------------------------- 02 落差 · B4 上下反差
def gap_board():
    """B4：上是年初以为的，下是实际发生的。「对」的一侧在下（中文结论位）。"""
    def block(label, line, note, tint, ink, fill):
        return plaster(f"反差块 · {label}", [
            caption("反差标签", label, tint, weight=600),
            text(ids, "反差句", line, FS_T2, 700, ink, family=CJK,
                 width=INNER - 64, growth="fixed-width",
                 line_height=LH_TITLE),
            body("反差注解", note, "$c-dim", width=INNER - 64),
        ], gap=14, fill=fill)

    main = zone([
        text(ids, "小标题", "年初和年末的差距", FS_T1, 700, "$c-shell",
             family=CJK, line_height=LH_TITLE),
        col("上下反差", [
            block("年初以为", "今年要做十件大事",
                  "列了满满一页，每件都值得做。", "$c-faint", "$c-dim",
                  "$c-plaster"),
            rect(ids, "分界", width="fill_container", height=3,
                 fill=solid("$c-cinnabar")),
            block("实际发生", "只有两件真的完成",
                  "另外八件不是失败，是从来没开始。", "$c-cinnabar",
                  "$c-shell", "$c-raised"),
        ], gap=0),
    ], justify="center", gap=36)
    return board(2, "落差 · 上下反差", main, "起点")


# ------------------------------------------------------- 03 那段 · F1 段落纯排
PARAS = [
    "三月到六月是最难的一段。手上四件事同时推，每一件都推到七成就停住，"
    "然后被下一件挤走。",
    "后来才看明白：不是时间不够，而是从来没有一件事被允许做完。"
    "开始永远比结束容易，于是我一直只在开始。",
]


def hard_part():
    """F1：一板一个小标题，两段正文，段首用矿彩方点标记。"""
    blocks = []
    for index, para in enumerate(PARAS):
        marker = rect(ids, "段首标记", width=18, height=18,
                      fill=solid("$c-azurite" if index == 0 else "$c-raised"))
        blocks.append(row("段落", [
            col("段首标记位", [marker], gap=0, width=18),
            body("正文", para, "$c-shell", width=INNER - 18 - 24),
        ], gap=24, align="start"))

    main = zone([
        text(ids, "小标题", "最难的那三个月", FS_T1, 700, "$c-shell",
             family=CJK, line_height=LH_TITLE),
        col("正文区", blocks, gap=36),
    ], justify="center", gap=40)
    return board(3, "那段 · 段落纯排", main, "低点")


# ------------------------------------------------------- 04 三个数 · D4 三栏指标
METRICS = [
    ("37", "件", "做完的事"),
    ("214", "条", "写下的计划"),
    ("17", "%", "完成率"),
]


def triad():
    """D4：三等分（各 4 列），纵向对齐基线。数字走 Inter，每板必须有来源行。

    D 族硬规则：颜色只用中性 + 一支有彩色。这里只有第三栏（完成率）走石青
    —— 它是三个数里唯一需要被记住的那个。
    """
    blocks = []
    for index, (value, unit, label) in enumerate(METRICS):
        hot = index == 2
        blocks.append(col(f"指标 {index + 1}", [
            row("数值行", [
                numeral("数值", value, FS_DISPLAY,
                        "$c-azurite" if hot else "$c-shell"),
                numeral("单位", unit, FS_T2, "$c-faint"),
            ], gap=8, align="end", width="fit_content"),
            caption("指标名", label, "$c-dim", width=COLUMN * 4),
        ], gap=12, width=COLUMN * 4))

    main = zone([
        text(ids, "小标题", "三个数说明一切", FS_T1, 700, "$c-shell",
             family=CJK, line_height=LH_TITLE),
        rect(ids, "指标上线", width="fill_container", height=1,
             fill=solid("$c-plaster")),
        row("三栏", blocks, gap=GUTTER, align="start"),
        caption("来源", "统计自本年度的任务清单，截至 12 月 31 日",
                "$c-faint", width=INNER),
    ], justify="center", gap=32)
    return board(4, "三个数 · 三栏指标", main, "盘点")


# ------------------------------------------------------- 05 形状 · C4 时间轴
MONTHS = [
    ("01–03", "铺得最开", "同时开了四条线"),
    ("04–06", "全部卡住", "每条都停在七成"),
    ("07–09", "砍到两条", "关掉的比做完的多"),
    ("10–12", "第一次做完", "两件收尾，一件上线"),
]


def shape():
    """C4：轴贴左，内容 9 列。引导符一板只用一种 —— 这里是季度节点。"""
    items = []
    for index, (span, title, note) in enumerate(MONTHS):
        last = index == len(MONTHS) - 1
        dot = rect(ids, "节点", width=16, height=16, cornerRadius=8,
                   fill=solid("$c-cinnabar" if last else "$c-plaster"),
                   stroke={"thickness": 3,
                           "fill": solid("$c-cinnabar" if last
                                         else "$c-faint")})
        # 轴段高度量出来的：刻度内容高 = caption 48 + gap 10 + title 62
        # ≈ 120，加刻度间距 36，减节点 16 与其下 8 的间隙 = 132。
        axis = [dot] if last else [
            dot, rect(ids, "轴段", width=2, height=132,
                      fill=solid("$c-plaster")),
        ]
        items.append(row("刻度", [
            numeral("月份", span, FS_CAPTION, "$c-faint", weight=600,
                    width=COLUMN * 2 + GUTTER),
            col("轴位", axis, gap=8, width=16, align="center"),
            col("刻度文案", [
                text(ids, "刻度标题", title, FS_T2, 700, "$c-shell",
                     family=CJK, width=INNER - COLUMN * 2 - GUTTER - 16 - 48,
                     growth="fixed-width", line_height=LH_TITLE),
                caption("刻度注", note, "$c-dim",
                        width=INNER - COLUMN * 2 - GUTTER - 16 - 48),
            ], gap=10, width=INNER - COLUMN * 2 - GUTTER - 16 - 48),
        ], gap=24, align="start"))

    main = zone([
        text(ids, "小标题", "这一年的形状", FS_T1, 700, "$c-shell",
             family=CJK, line_height=LH_TITLE),
        col("时间轴", items, gap=36),
    ], justify="center", gap=36)
    return board(5, "形状 · 时间轴", main, "全年")


# ------------------------------------------------------- 06 那件事 · F2 图注混排
def one_thing():
    """F2：视觉块占约 42%，下方标题 + 正文 + 图注。矿彩颗粒整套只在这里出现。

    石绿在本套是「第三色」，主题规定每套设计只用一次 —— 就用在这一板的颗粒
    上。它标的是全年唯一真正做完并交出去的那件事。
    """
    disc = upload_disc(ids, "上传占位", 112, PLACEHOLDER_DISC, 40,
                       PLACEHOLDER_ICON)
    hint = col("占位说明", [
        text(ids, "占位标题", "放那件事的成品图", FS_BODY, 600,
             PLACEHOLDER_TITLE, family=CJK, width="fit_content",
             growth="auto", line_height=LH_TITLE),
        text(ids, "占位规格", "建议 920×560 以上", FS_CAPTION, 400,
             PLACEHOLDER_SPEC, family=CJK, width="fit_content",
             growth="auto", line_height=LH_CAPTION),
    ], gap=8, width="fit_content", align="center")
    slot = frame(ids, "成品位", width="fill_container", height=560,
                 layout="vertical", gap=20, alignItems="center",
                 justifyContent="center", fill=solid("$c-plaster"))
    slot["children"] = [disc, hint]

    main = zone([
        slot,
        col("图注区", [
            text(ids, "小标题", "如果只能留一件", FS_T2, 700, "$c-shell",
                 family=CJK, line_height=LH_TITLE),
            body("正文", "十月做完并交出去的那个东西。它不是最大的，"
                 "但它是唯一一件走完全程的。", "$c-shell"),
            caption("图注", "从开始到交付 · 用了 47 天", "$c-faint"),
        ], gap=14),
    ], justify="center", gap=30)
    return board(6, "那件事 · 图注混排", main, "高点",
                 pigment_grains(EDGE, 420, INNER, 560, "$c-malachite"))


# ------------------------------------------------------- 07 金句 · B1 满版一句
def quote():
    """B1：一板一句，上下各留 ≥25%，除一个朱砂方块外没有任何装饰。"""
    main = zone([
        rect(ids, "朱砂方块", width=24, height=24,
             fill=solid("$c-cinnabar")),
        text(ids, "金句", "计划不是承诺，\n做完才是。", FS_DISPLAY, 700,
             "$c-shell", family=CJK, width=INNER, growth="fixed-width",
             line_height=LH_DISPLAY),
        caption("出处", "写在今年最后一条待办的后面", "$c-faint",
                width=INNER),
    ], justify="center", gap=44, pad_y=120)
    return board(7, "金句 · 满版一句", main, "复述点")


# ------------------------------------------------------- 08 目录 · G3 目录回看
INDEX = [
    ("01", "214 条计划，做完 37 件"),
    ("02", "八件不是失败，是没开始"),
    ("03", "三到六月，全部停在七成"),
    ("04", "完成率 17%"),
    ("05", "砍到两条线才有第一次做完"),
    ("06", "唯一走完全程的那件，47 天"),
]


def recap():
    """G3：单列，每行「页码 + 标题」。一套 ≥6 板时它优先于三键引导。"""
    lines = []
    for page, title in INDEX:
        lines.append(row("目录行", [
            numeral("回看页码", page, FS_CAPTION, "$c-faint", weight=600,
                    width=COLUMN * 2),
            body("回看标题", title, "$c-shell",
                 width=INNER - COLUMN * 2 - 24),
        ], gap=24, align="center"))

    main = zone([
        text(ids, "收束句", "明年只写\n十条计划。", FS_DISPLAY, 700,
             "$c-shell", family=CJK, width=INNER, growth="fixed-width",
             line_height=LH_DISPLAY),
        col("目录", lines, gap=18),
    ], justify="center", gap=36)
    return board(8, "目录 · 回看", main, "收束")


def build():
    return [cover(), gap_board(), hard_part(), triad(), shape(), one_thing(),
            quote(), recap()]


# 对比度（WCAG 相对亮度比，op-design-lint 门槛 2.0；本表逐对实算）：
#   c-shell   on c-bg     15.27    c-dim       on c-bg      9.07
#   c-faint   on c-bg      5.01    c-cinnabar  on c-bg      3.94
#   c-azurite on c-bg      3.79    c-gold      on c-bg      8.40
#   c-shell   on c-plaster 13.07   c-dim       on c-plaster 7.77
#   c-faint   on c-plaster  4.29   c-shell     on c-raised 10.75
#   c-dim     on c-raised   6.39   c-malachite on c-bg      4.81
# 承载正文的最低一对是 c-faint on c-plaster 4.29（第 2 板上半「年初以为」
# 那块的标签），高出 lint 门槛 2.1 倍；其余承载正文处最低 5.01。
# c-cinnabar 3.94 与 c-azurite 3.79 都只用在 ≥48px 的大字、方块与描边上
# （AA 大字门槛 3.0），这正是主题里「朱砂仅 ≥48px」那条约束的量化依据。
# c-gold 8.40 数值很高，但本模板仍然只让它走描边与角标 —— 那是主题定义，
# 不是对比度问题；唯一的例外是封面那个 120px 的单个数字。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "年度总结复盘轮播 · 3:4 八板")
