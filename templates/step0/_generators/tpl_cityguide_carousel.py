#!/usr/bin/env python3
"""cityguide-film-carousel.op — 城市 / 旅行指南轮播（7 板 · 1080×1440 · 3:4）

叙事类型：**指南**。旅行内容的读者分两类：一类在做梦，一类在做计划。指南
档必须同时喂饱两个 —— 所以本套的节奏是**照片与动线交替**：第 2、4 板是地
点（给做梦的），第 3 板是一日动线、第 5 板是吃住对照（给做计划的），中间
不让任何一类连吃两口。

轮播在这一档有个长图给不了的东西：**照片可以真的占满一屏**。所以本套两个
图位都给到画幅的 46%，且都带图注 —— 图是内容不是背景板。

### 主题：T9 胶片冲印 `darkroom-film`（暗 / 中性 / 产品·摄影·案例）

**采样 → 收敛 → 论证**（沿用 card-system-0808 §3 T9）

  - 采样：片基的近黑、银盐灰阶、相纸白边、C-41 冲印的色偏方向、暗房安全
    灯。
  - 收敛：底 = 片基 L0.18 C0.012（近中性）；中性 = 银灰序列；两支「色偏」
    不是装饰色而是**光学事实** —— 暗部青 H210 / 高光暖 H70；一支安全灯红
    作点睛。
  - 论证：**色偏不作为强调色使用，只叠在暗部与高光上** —— 它们模拟的是显
    影药水的偏色，不是设计师选的颜色。这个区分让整套像「一张真照片的处理
    结果」而不是「用了青橙配色」。选它承载旅行：胶片是「去过」这件事在中
    文语境里最强的物证形态，而指南要的正是「我真的走过这条线」。

**最近邻论证**：本批另外三套暗底是 `arcade-neon`（H275 蓝紫）、
`chalk-board`（H165 墨绿）、`mingsha-mineral`（H45 赭）。本套 `#0E1217` 是
L0.18 C0.012 的近中性 —— 它是四套暗底里**唯一几乎没有色相**的一套，缩略图
里另外三套都带着明显的色，只有它是灰的。结构上更是唯一一套有**白边**的：
整板外圈一道 24px 相纸白，隔一屏都认得出。

### 母版规则（七板的硬约束）

**每板固定，不许变**
  1. 画幅 1080×1440。**相纸白边 24px** 包住整板，白边之内才是片基。正文
     安全边距因此是「白边 24 + 片窗 56/72/104」，合计仍是 左右 80 / 上 96
     / 下 128。
  2. **齿孔**：上下白边上各一排 8×12、圆角 2、间距 24 的孔，填片基色。等
     距且贴边 —— 七板一个不差。
  3. 页眉：左「城市指南」，右「NN / 07」（DM Mono，Caption 32px）。
  4. **边码**：页脚左侧一行 DM Mono 的胶片边码（`07A · 24×36` 这种形状）。
     这是本套最便宜也最有效的真实性来源 —— 胶片边码本来就是等宽的。
  5. **色偏**：每板顶部叠一层 `shadow.cyan@10%`、底部叠一层
     `highlight.warm@8%`。方向固定（暗部偏青、高光偏暖），不许对调。
  6. 字族：汉字 Noto Sans SC / **一切数字与边码走 DM Mono**。

**允许变，且只有这些能变**
  - 主体区的信息结构（下面七板各用一条配方）。
  - 边码的内容（每板一个不同的编号）。
  - **安全灯红 `c-safelight`**：整套只出现一次，落在封面的眉标上。

### 配方编排（card-system-0808 §4.2）

    01 A1 巨字压顶 → 02 F2 图注混排 → 03 C4 时间轴 → 04 F2 图注混排
    05 E3 表格两列 → 06 B1 满版一句 → 07 G3 目录回看

首板 A 族、末板 G 族；相邻两板不同族；F 族出现 2 次但**不相连**（第 2、4
板，中间隔着 C4）；B 族 1 次且落在后 1/3（第 6 板）；覆盖 A/F/C/E/B/G
**六族**。

### 负约束（本模板明令不做的事）

  - **不做划痕、噪点、漏光**。那些是位图滤镜，矢量做出来一定假 —— 本套的
    胶片感全部来自结构（白边、齿孔、边码）与色偏，不来自「做旧」。
  - **色偏方向不许对调**。暗部偏青、高光偏暖是 C-41 的物理事实；反过来就
    不是冲印偏色，是随手选的青橙。
  - **安全灯红一套只出现一次**。它是暗房里唯一的光源，出现第二次就不是暗
    房了。
  - **齿孔必须等距且贴边**，不可作为装饰散布到画面中间。
  - 不用「黑白 + 单一强调色」的常见套路 —— 本套的层次来自银灰阶，不来自
    强调色。
  - 不写「小众宝藏 / 出片神地 / 人少景美」这类词；每个地点都要给「什么时
    候去、待多久」。
  - 每板正文不超过 4 行。

硬契约：
  - 字号下限 32px；单板最多用 4 档字阶。
  - CJK 行高：Display 1.15 / Title 1.3 / Body 1.7 / Caption 1.5。
  - CJK 字距恒为 0；只有 DM Mono 数字沿用西文收紧。
  - 正文与背景对比度 ≥2.0（实测表见文件末尾）。
  - 顶层 frame 必须显式写 x/y。
  - 文本节点绝不写 height。
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import (Ids, PLACEHOLDER_DISC, PLACEHOLDER_ICON, PLACEHOLDER_SPEC,
                   PLACEHOLDER_TITLE, color_vars, frame, linear, rect, solid,
                   text, upload_disc, write_doc)

ids = Ids()

VARS = color_vars({
    "c-bg":        "#0E1217",   # film.base      片基
    "c-gate":      "#1A2026",   # film.gate      片窗 / 卡片
    "c-print":     "#F1F0ED",   # border.print   相纸白边
    "c-silver":    "#CED1D5",   # silver         主文字
    "c-dim":       "#95999D",   # silver.dim     次级文字 / 边码
    "c-shadow":    "#2E545B",   # shadow.cyan    暗部色偏（叠加层）
    "c-highlight": "#EBD3B8",   # highlight.warm 高光色偏（叠加层）
    "c-safelight": "#BC4A3B",   # safelight      安全灯红：一套限一处
})

CJK = "Noto Sans SC"
# 边码与一切数字走等宽。card-system 点名 IBM Plex Mono；仓里烤进桌面端的
# 等宽族是 DM Mono，宽度关系一致。换成没打包的族会退回系统字体，边码就不
# 再等宽 —— 那是本套真实性的第一根柱子。
NUM = "DM Mono"

W, H, GAP = 1080, 1440, 120

# 3 板一行 —— 与 deck 体系（deckkit.BOARDS_PER_ROW）同一约定：多板模板在画布上
# 分行铺开，而不是拖成一长排。行间距比列间距多 240 不是手滑：画布在帧上方以
# **屏幕空间**固定偏移画帧名，缩到能整屏看时 120 文档像素只剩十几个屏幕像素，
# 第二行的帧名会压到上一行的板上。
BOARDS_PER_ROW = 3
ROW_GAP = GAP + 240
BORDER = 24                   # 相纸白边
EDGE = 80 - BORDER            # 片窗内的左右内边距，合计仍是 80
TOP, BOT = 96 - BORDER, 128 - BORDER
INNER = W - 80 * 2
COLUMN, GUTTER = 62, 16

FS_DISPLAY, FS_T1, FS_T2 = 88, 64, 48
FS_BODY, FS_CAPTION = 40, 32
LH_DISPLAY, LH_TITLE, LH_BODY, LH_CAPTION = 1.15, 1.3, 1.7, 1.5

SERIES = "城市指南"
TOTAL = 7
HOLE_W, HOLE_H, HOLE_STEP, HOLE_R = 8, 12, 24, 2
CAST_H = 240                  # 色偏叠加层的高度，上下各一
# 片窗的尺寸。色偏层挂在片窗里，坐标就得用片窗自己的原点 ——
# 拿整板的 W/H 去摆，等于把白边的厚度算了两次，右缘和下缘会被裁掉。
GATE_W, GATE_H = W - BORDER * 2, H - BORDER * 2


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


def caption(name, content, color="$c-dim", *, family=CJK, weight=400,
            width="fit_content"):
    return text(ids, name, content, FS_CAPTION, weight, color, family=family,
                width=width, growth="auto" if width == "fit_content"
                else "fixed-width", line_height=LH_CAPTION)


def body(name, content, color="$c-dim", *, weight=400,
         width="fill_container"):
    return text(ids, name, content, FS_BODY, weight, color, family=CJK,
                width=width, growth="auto" if width == "fit_content"
                else "fixed-width", line_height=LH_BODY)


# ----------------------------------------------------------------- 暗房语汇
def sprockets():
    """齿孔：上下白边上各一排，等距贴边。位置由常量算出，不手写。

    孔是**片基色**打在**相纸白**上 —— 顺序反了就成了白点撒在黑边上，那是
    另一种东西（打孔纸）。等距是硬约束：齿孔一旦不等距，观众第一眼就知道
    这不是胶片，而说不出哪里不对。
    """
    holes = []
    count = W // HOLE_STEP
    left = round((W - (count - 1) * HOLE_STEP - HOLE_W) / 2)
    for i in range(count):
        for edge_y in (round((BORDER - HOLE_H) / 2),
                       H - BORDER + round((BORDER - HOLE_H) / 2)):
            hole = rect(ids, "齿孔", width=HOLE_W, height=HOLE_H,
                        cornerRadius=HOLE_R, fill=solid("$c-bg"))
            hole["x"], hole["y"] = left + i * HOLE_STEP, edge_y
            holes.append(hole)
    return holes


def colour_cast():
    """色偏：顶部叠青、底部叠暖。方向固定，不许对调。

    这两层不是「装饰色」，是显影药水的偏色 —— 所以它们从有色走到全透明，
    而不是从有色走到另一支有色。渐变色用字面值：这一对方向是主题的定义，
    绑成变量就等于允许用户把它调反。
    """
    top = rect(ids, "暗部色偏", width=GATE_W, height=CAST_H,
               fill=linear(90, [(0, "#2E545B"), (1, "#0E1217")]))
    top["x"], top["y"] = 0, 0
    top["opacity"] = 0.10
    bottom = rect(ids, "高光色偏", width=GATE_W, height=CAST_H,
                  fill=linear(90, [(0, "#0E1217"), (1, "#EBD3B8")]))
    bottom["x"], bottom["y"] = 0, GATE_H - CAST_H
    bottom["opacity"] = 0.08
    return [top, bottom]


def photo_slot(height, hint_title, spec):
    """图位。图是内容不是背景板，所以它有自己的标题与规格提示。

    占位用 oplib.upload_disc：它只由 group + text + icon_font 三种**非图片
    投放目标**搭成，用户把照片拖到框里任何位置都会解析到外层这个框，而不
    是被中间那个圆点截走。
    """
    disc = upload_disc(ids, "上传占位", 116, PLACEHOLDER_DISC, 42,
                       PLACEHOLDER_ICON)
    hint = col("占位说明", [
        text(ids, "占位标题", hint_title, FS_BODY, 600, PLACEHOLDER_TITLE,
             family=CJK, width="fit_content", growth="auto",
             line_height=LH_TITLE),
        text(ids, "占位规格", spec, FS_CAPTION, 400, PLACEHOLDER_SPEC,
             family=NUM, width="fit_content", growth="auto",
             line_height=LH_CAPTION),
    ], gap=8, width="fit_content", align="center")
    slot = frame(ids, "片窗", width="fill_container", height=height,
                 layout="vertical", gap=20, alignItems="center",
                 justifyContent="center", fill=solid("$c-gate"))
    slot["children"] = [disc, hint]
    return slot


# ----------------------------------------------------------------- 母版部件
def header(page):
    return row("页眉", [
        caption("系列名", SERIES, "$c-dim"),
        caption("页码", f"{page:02d} / {TOTAL:02d}", "$c-dim", family=NUM),
    ], gap=24, justify="space_between")


def footer(edge_code):
    return row("页脚", [
        caption("边码", edge_code, "$c-dim", family=NUM),
        caption("账号名", "@ 你的账号名", "$c-silver", weight=600),
    ], gap=24, justify="space_between")


def board(page, name, main, edge_code):
    """一板。外壳是相纸白，白边之内才是片基 —— 白边是本套的第一识别信号。

    齿孔层与色偏层都上 `locked`：两者都是铺满整板的壳子，自己一寸不画（墨
    在里面那 24 个孔 / 两条色带上）。齿孔层还压在正文之上，不锁的话它自身
    就是一块盖住全板的透明板。locked 让壳子不可选而壳内的孔照常可选
    （hit_test_walk 的 locked 判定在子节点遍历之后）。
    """
    content = frame(ids, f"{name} · 内容", width="fill_container",
                    height="fill_container", layout="vertical",
                    padding=[TOP, EDGE, BOT, EDGE], gap=0,
                    alignItems="start", fill=[])
    content["children"] = [header(page), main, footer(edge_code)]

    gate = frame(ids, f"{name} · 片基", width="fill_container",
                 height="fill_container", layout="none",
                 fill=solid("$c-bg"), clipContent=True)
    # 装饰（色偏）最后 —— jian 里 index 0 最上，写反了会盖住正文。
    cast = frame(ids, f"{name} · 色偏", width="fill_container",
                 height="fill_container", layout="none", fill=[],
                 locked=True)
    cast["children"] = colour_cast()
    gate["children"] = [content, cast]

    shell = frame(ids, f"{page:02d} {name}", width=W, height=H, layout="none",
                  fill=solid("$c-print"), padding=BORDER, clipContent=True)
    holes = frame(ids, f"{name} · 齿孔", width=W, height=H, layout="none",
                  fill=[], locked=True)
    holes["children"] = sprockets()
    holes["x"], holes["y"] = -BORDER, -BORDER
    shell["children"] = [holes, gate]
    shell["x"] = ((page - 1) % BOARDS_PER_ROW) * (W + GAP)
    shell["y"] = ((page - 1) // BOARDS_PER_ROW) * (H + ROW_GAP)
    return shell


def zone(children, *, justify="center", gap=32, pad_y=40):
    node = frame(ids, "主体", width="fill_container", height="fill_container",
                 layout="vertical", gap=gap, padding=[pad_y, 0],
                 justifyContent=justify, alignItems="start", fill=[])
    node["children"] = children
    return node


# ------------------------------------------------------- 01 封面 · A1 巨字压顶
def cover():
    """A1：主标压顶约 36%，主视觉在下 —— 且主视觉与标题有语义关系，不是装饰。"""
    main = zone([
        caption("眉标", "两天一夜 · 全程步行可达", "$c-safelight",
                weight=600),
        text(ids, "主标", "在这座城\n只走一条巷", FS_DISPLAY, 700,
             "$c-silver", family=CJK, width=INNER, growth="fixed-width",
             line_height=LH_DISPLAY),
        body("副标", "七板给完动线、时间和吃住，照着走就行。", "$c-dim"),
        photo_slot(460, "放这条巷的全景", "1080×460 以上"),
    ], justify="center", gap=30)
    return board(1, "封面 · 巨字压顶", main, "01A · 24×36")


# ------------------------------------------------------- 02 地点一 · F2 图注混排
def place_one():
    """F2：视觉块占约 46%，下方标题 + 正文 + 图注。每个地点都给「几点去、待多久」。"""
    main = zone([
        photo_slot(620, "放巷口那家早点铺", "1080×620 以上"),
        col("图注区", [
            text(ids, "地点名", "巷口 · 一家没有招牌的早点铺", FS_T2, 700,
                 "$c-silver", family=CJK, width=INNER,
                 growth="fixed-width", line_height=LH_TITLE),
            body("正文", "六点半开门，卖完就收。坐下之前先看看墙上那块小黑"
                 "板，写着当天有什么。", "$c-silver"),
            caption("图注", "建议 06:40 到 · 停留 30 分钟", "$c-dim"),
        ], gap=14),
    ], justify="center", gap=30)
    return board(2, "地点一 · 图注混排", main, "02A · 24×36")


# ------------------------------------------------------- 03 动线 · C4 时间轴
ROUTE = [
    ("06:40", "巷口早点铺", "先吃再走，人最少"),
    ("08:00", "沿巷慢走到尽头", "两侧都是住家，别赶"),
    ("10:30", "老茶馆歇脚", "二楼靠窗，看得见整条巷"),
    ("15:00", "回到巷口", "光转过来了，这时才好拍"),
]


def route():
    """C4：轴贴左，内容 9 列。引导符一板只用一种 —— 这里是时间节点。

    时间用 DM Mono：一列时刻只要不等宽，读者就没法一眼比出间隔，动线也就
    失去了「几点到几点」的功能。
    """
    items = []
    for index, (clock, place, note) in enumerate(ROUTE):
        last = index == len(ROUTE) - 1
        dot = rect(ids, "节点", width=14, height=14, cornerRadius=7,
                   fill=solid("$c-silver"))
        # 轴段高度是量出来的：刻度内容高 = caption 48 + gap 10 + title 62
        # ≈ 120，加刻度间距 36，减去节点 14 与它下方 8 的间隙 = 134。
        axis = [dot] if last else [
            dot, rect(ids, "轴段", width=2, height=134,
                      fill=solid("$c-gate")),
        ]
        items.append(row("刻度", [
            text(ids, "时刻", clock, FS_CAPTION, 600, "$c-dim", family=NUM,
                 width=COLUMN * 2, growth="fixed-width",
                 line_height=LH_CAPTION),
            col("轴位", axis, gap=8, width=14, align="center"),
            col("刻度文案", [
                text(ids, "地点", place, FS_T2, 700, "$c-silver", family=CJK,
                     width=INNER - COLUMN * 2 - 14 - 48, growth="fixed-width",
                     line_height=LH_TITLE),
                caption("刻度注", note, "$c-dim",
                        width=INNER - COLUMN * 2 - 14 - 48),
            ], gap=10, width=INNER - COLUMN * 2 - 14 - 48),
        ], gap=24, align="start"))

    main = zone([
        text(ids, "小标题", "一天的动线", FS_T1, 700, "$c-silver",
             family=CJK, line_height=LH_TITLE),
        col("动线", items, gap=36),
    ], justify="center", gap=36)
    return board(3, "动线 · 时间轴", main, "03A · 24×36")


# ------------------------------------------------------- 04 地点二 · F2 图注混排
def place_two():
    """F2 第二次。和第 2 板版式完全相同 —— 同一档内容就该用同一个版式。"""
    main = zone([
        photo_slot(620, "放老茶馆二楼靠窗那张桌", "1080×620 以上"),
        col("图注区", [
            text(ids, "地点名", "巷中 · 二楼靠窗的老茶馆", FS_T2, 700,
                 "$c-silver", family=CJK, width=INNER,
                 growth="fixed-width", line_height=LH_TITLE),
            body("正文", "上楼右手第二张桌能看到整条巷。茶按壶算，"
                 "坐一下午没人赶你。", "$c-silver"),
            caption("图注", "建议 10:30 到 · 停留 90 分钟", "$c-dim"),
        ], gap=14),
    ], justify="center", gap=30)
    return board(4, "地点二 · 图注混排", main, "04A · 24×36")


# ------------------------------------------------------- 05 吃住 · E3 表格两列
STAY = [
    ("住哪", "巷尾民宿", "连锁酒店"),
    ("吃什么", "早点铺与茶馆", "商圈餐厅"),
    ("怎么走", "全程步行", "打车绕外环"),
]


def stay():
    """E3：表头 + 三行两列，斑马底。两列版式完全相同，只有字重与色阶不同。

    E 族硬规则：「对」的一侧永远在右（中文阅读的结论位），且区分不能只靠
    图标 —— 这里靠的是字重与银灰阶的明度差。
    """
    head = row("表头", [
        caption("表头 · 项", "项目", "$c-dim", weight=600, width=COLUMN * 3),
        caption("表头 · 推荐", "推荐", "$c-dim", weight=600,
                width=COLUMN * 4),
        caption("表头 · 备选", "备选", "$c-dim", weight=600,
                width=COLUMN * 4),
    ], gap=GUTTER, align="center")

    rows = [head, rect(ids, "表头线", width="fill_container", height=2,
                       fill=solid("$c-silver"))]
    for index, (item, first, second) in enumerate(STAY):
        line = row(f"表行 {index + 1}", [
            caption("项", item, "$c-dim", width=COLUMN * 3),
            caption("推荐", first, "$c-silver", weight=600,
                    width=COLUMN * 4),
            caption("备选", second, "$c-dim", width=COLUMN * 4),
        ], gap=GUTTER, align="center", padding=[18, 14])
        if index % 2 == 0:
            line["fill"] = solid("$c-gate")
        rows.append(line)

    main = zone([
        text(ids, "小标题", "吃住怎么定", FS_T1, 700, "$c-silver",
             family=CJK, line_height=LH_TITLE),
        col("对照表", rows, gap=0),
        caption("表注", "推荐一列都在这条巷里，走过去不超过十分钟。",
                "$c-dim", width=INNER),
    ], justify="center", gap=32)
    return board(5, "吃住 · 表格两列", main, "05A · 24×36")


# ------------------------------------------------------- 06 金句 · B1 满版一句
def quote():
    """B1：一板一句，上下各留 ≥25%，除一条银线外没有任何装饰。"""
    main = zone([
        rect(ids, "银线", width=COLUMN * 2 + GUTTER, height=2,
             fill=solid("$c-silver")),
        text(ids, "金句", "一天走一条巷，\n比赶十个点记得久。",
             FS_DISPLAY, 700, "$c-silver", family=CJK, width=INNER,
             growth="fixed-width", line_height=LH_DISPLAY),
        caption("出处", "第三次去这座城之后才想明白的事", "$c-dim",
                width=INNER),
    ], justify="center", gap=40, pad_y=110)
    return board(6, "金句 · 满版一句", main, "06A · 24×36")


# ------------------------------------------------------- 07 目录 · G3 目录回看
INDEX = [
    ("02", "巷口早点铺 · 06:40"),
    ("03", "一天动线四个点"),
    ("04", "老茶馆二楼 · 10:30"),
    ("05", "住巷尾，全程步行"),
    ("06", "一天一条巷"),
]


def recap():
    """G3：单列，每行「页码 + 标题」。一套 ≥6 板时它优先于三键引导。"""
    lines = []
    for page, title in INDEX:
        lines.append(row("目录行", [
            text(ids, "回看页码", page, FS_CAPTION, 600, "$c-dim",
                 family=NUM, width=COLUMN * 2, growth="fixed-width",
                 line_height=LH_CAPTION),
            body("回看标题", title, "$c-silver",
                 width=INNER - COLUMN * 2 - 24),
        ], gap=24, align="center"))

    main = zone([
        text(ids, "收束句", "这条巷，\n照着这张表走。", FS_DISPLAY, 700,
             "$c-silver", family=CJK, width=INNER, growth="fixed-width",
             line_height=LH_DISPLAY),
        col("目录", lines, gap=20),
    ], justify="center", gap=40)
    return board(7, "目录 · 回看", main, "07A · 24×36")


def build():
    return [cover(), place_one(), route(), place_two(), stay(), quote(),
            recap()]


# 对比度（WCAG 相对亮度比，op-design-lint 门槛 2.0；本表逐对实算）：
#   c-silver    on c-bg  12.26    c-dim    on c-bg      6.55
#   c-safelight on c-bg   3.74    c-silver on c-gate   10.72
#   c-dim       on c-gate  5.73   c-print  on c-bg     16.49
#   c-highlight on c-bg   13.01   c-shadow on c-bg      2.27
# 承载正文的最低一对是 c-dim on c-gate 5.73（第 5 板斑马行与图注），已过
# WCAG AA 正文门槛。c-safelight 3.74 只用在封面眉标那一处，且是 600 字重
# 的 32px —— 整套仅此一次，正是「暗房里只有一盏安全灯」这条论证的落点。
# c-shadow 2.27 刚过门槛，但它从来不是文字色，只作 10% 的暗部叠加层，
# 那一层之下也没有任何文字（色偏两条带都落在正文柱的上下留白里）。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "城市指南轮播 · 3:4 七板")
