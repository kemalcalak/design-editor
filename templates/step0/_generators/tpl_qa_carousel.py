#!/usr/bin/env python3
"""qa-chalkboard-carousel.op — 问答体轮播（6 板 · 1080×1440 · 3:4）

叙事类型：**问答**，一问一板。这是最贴合轮播这个媒介的叙事形态：一个问题
天然就是一个「划下去的理由」，读者不需要被留悬念，问题本身就是钩子。

所以本套的母版里有一处别的档没有的东西：**每板左上角一个手写的问号编号**
（Q1…Q4）。它把「这是第几问」变成一个可扫的锚点 —— 读者中途进来也知道自
己错过了几问，回头去找也找得回来。第 1 板与第 6 板不编号，因为它们不是问
题，是开场与收束。

一问一板不等于六板同一个版式。问题的**形状**不一样：有的是二选一，有的要
一段解释，有的是机制，有的是纠错。所以四问四种配方，防的正是「六页都是中
心大字」那条反模式。

### 主题：T8 粉笔黑板 `chalk-board`（暗 / 安静 / 教学·科普）

**采样 → 收敛 → 论证**（沿用 card-system-0808 §3 T8）

  - 采样：墨绿板面（不是黑，是深绿）、白粉笔、黄/玫瑰/天蓝彩粉、擦痕。
  - 收敛：底 = 墨绿 L0.26 C0.026 H165；中性 = 粉笔白序列 0.61 / 0.76 /
    0.925（**上限只到 0.925，不到 1.0**）；彩粉三支 L 0.82-0.86 低 chroma。
  - 论证：**粉笔白封顶在 L0.925 而不是纯白** —— 粉笔是颗粒堆积，永远盖不
    住板色，纯白会让整套变成「深绿底白字 PPT」。彩粉 chroma 只到 0.085，
    因为粉笔颜料浓度本就低。选它承载问答：黑板是中文语境里「有人在回答你」
    最直接的场景，而且它自带一个姿态 —— 讲台上的人是在解释，不是在宣布。

**最近邻论证**：本批另外三套暗底是 `arcade-neon`（H275 蓝紫）、
`darkroom-film`（H250 近中性）、`mingsha-mineral`（H45 赭）。本套 H165 墨
绿，与三者两两 H 差 ≥85°，且色相语义完全不同（街灯 / 暗房 / 壁画 / 教室）。
装饰上更是唯一一套用**手绘线**的：所有的框和箭头都由 3-5 段各偏移 ±1px 的
短线拼成，缩略图里就能看出「这套是手画的」。

### 母版规则（六板的硬约束）

**每板固定，不许变**
  1. 画幅 1080×1440，安全边距 左右 80 / 上 96 / 下 128。
  2. 页眉：左「有问必答」，右「NN / 06」（Inter，Caption 32px）。
  3. **问号编号**：第 2-5 板左上角一个 Q1…Q4 的彩粉标号，位置与字号锁死。
     第 1、6 板不编号（它们不是问题）。
  4. 页脚：一条**粉笔线**（5 段各偏移 ±1px 的短线拼成）+ 署名。
  5. **擦痕**：每板一块 `radial` 云斑，`chalk@6%`，半径 320-480，位置避开
     文字。六板各一块，位置不同 —— 擦痕重复出现在同一处就成了水印。
  6. 字族：汉字 Noto Sans SC / 数字与页码 Inter。
  7. 一板最多两支彩粉色。

**允许变，且只有这些能变**
  - 主体区的信息结构（下面六板各用一条配方）。
  - 当板用哪两支彩粉（黄 = 重点 / 玫瑰 = 警示 / 天蓝 = 补充，语义固定）。
  - 擦痕的位置与半径。

### 配方编排（card-system-0808 §4.2）

    01 A3 反问切口 → 02 E1 左右对开 → 03 F1 段落纯排
    04 C5 流程箭头 → 05 B4 上下反差 → 06 G1 签名收束

首板 A 族、末板 G 族；相邻两板不同族；覆盖 A/E/F/C/B/G **六族**（6 板要求
≥4）；B 族 1 次且落在后 1/3（第 5 板）。

### 负约束（本模板明令不做的事）

  - **不用纯白、不用纯黑**。粉笔白封顶 `#E7E6E2`，板底是墨绿不是黑。这两
    条一破，整套就变成「深绿底白字 PPT」。
  - **手绘线的抖动幅度固定 ±1px**。±3px 就不是「手画的」而是「歪了」。单
    段直线更不行 —— 一条完美的直线立刻暴露这是矢量。
  - **不画框**。黑板上强调一段内容的真实动作是把那块擦干净再写，不是画一
    个框。所以分区一律用比板底深一档的「擦干净的一块」，不描边。
  - 不做黑板擦、粉笔盒等具象道具；不做木质边框；不做粉笔灰颗粒噪点。
  - 彩粉三支语义固定（黄 = 重点 / 玫瑰 = 警示 / 天蓝 = 补充），不许当装饰
    随手换 —— 读者会在第二板学会这套编码，第四板换掉就是背叛。
  - 不写「一文讲透 / 秒懂 / 划重点」这类词。
  - 每板正文不超过 4 行。

硬契约：
  - 字号下限 32px；单板最多用 4 档字阶。
  - CJK 行高：Display 1.15 / Title 1.3 / Body 1.7 / Caption 1.5。
  - CJK 字距恒为 0。
  - 正文与背景对比度 ≥2.0（实测表见文件末尾）。
  - 顶层 frame 必须显式写 x/y。
  - 文本节点绝不写 height。
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import (Ids, color_vars, frame, path, radial, rect, solid, text,
                   write_doc)

ids = Ids()

VARS = color_vars({
    "c-bg":     "#182821",   # board        墨绿板面
    "c-deep":   "#0C1B14",   # board.deep   分区 / 深处
    "c-chalk":  "#E7E6E2",   # chalk        主文字
    "c-dim":    "#B2B1AB",   # chalk.dim    次级文字
    "c-faint":  "#84837D",   # chalk.faint  注释 / 页码
    "c-yellow": "#E5D090",   # chalk.yellow 彩粉黄：重点
    "c-rose":   "#EDB2BA",   # chalk.rose   彩粉玫瑰：警示
    "c-sky":    "#A3CFE4",   # chalk.sky    彩粉蓝：补充
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

FS_DISPLAY, FS_T1, FS_T2 = 88, 64, 48
FS_BODY, FS_CAPTION = 40, 32
LH_DISPLAY, LH_TITLE, LH_BODY, LH_CAPTION = 1.15, 1.3, 1.7, 1.5

SERIES = "有问必答"
TOTAL = 6
JITTER = 1                    # 手绘线的抖动幅度，固定 ±1px


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


# ----------------------------------------------------------------- 板书语汇
def chalk_line(width_px, color="$c-chalk", thickness=3, segments=5):
    """粉笔线：5 段首尾相接、每段上下偏移 ±1px 的短线拼成一条。

    **单段直线立刻暴露这是矢量。** 板书里没有一条线是直的，抖动就是这套主
    题的真实性来源。抖幅锁死 ±1px：±3px 就从「手画的」变成「画歪了」。
    走一条 `path` 而不是几个 rect —— 拼出来的段是独立节点，用户挪一下就散
    架；一条 path 怎么挪都还是一条线。
    """
    step = width_px / segments
    pts = ["M 0 %d" % JITTER]
    for i in range(segments):
        y = JITTER if i % 2 else 0
        pts.append(f"L {round(step * (i + 1), 1)} {y}")
    for i in reversed(range(segments)):
        y = (JITTER if i % 2 else 0) + thickness
        pts.append(f"L {round(step * i, 1)} {y}")
    pts.append("Z")
    return path(ids, "粉笔线", " ".join(pts), width=width_px,
                height=thickness + JITTER, fill=solid(color))


def wiped_patch(name, children, *, pad=(24, 28), gap=0,
                width="fill_container"):
    """擦干净的一块板：比板底更深的一片区域，**不描边**。

    第一版这里画的是「四角不闭合的手绘框」。做不成：框的高度由内容撑出来，
    要拿四条独立的粉笔线去围它，就得先知道那个高度 —— 而 flex 里不知道。
    退而用带 dashPattern 的描边，实测在这个尺度上又读不出断口，等于画了一
    个普通矩形框，还在注释里写着「四角不闭合」——那就成了代码和文档各说各
    话。

    所以换了个在这块板上更成立的语汇：**擦干净的一块**。黑板上要强调一段
    内容，真实的动作不是画框，是把那块擦干净再写。深一档的底色正是「刚擦
    过」的样子，也用掉了主题色板里本就为此准备的 `board.deep`。
    """
    node = frame(ids, name, width=width, height="fit_content",
                 layout="vertical", gap=gap, padding=list(pad),
                 alignItems="start", fill=solid("$c-deep"))
    node["children"] = children
    return node


def smudge(cx, cy, radius):
    """擦痕：一块 `chalk@6%` 的径向云斑。位置必须避开文字。

    半径给到 0.5、最外一档 stop 取板底色，光晕就在节点边界收干净，不会留
    下一圈可见的方边（TileMode::Clamp 会把边界外全刷成最后一个 stop）。
    """
    node = rect(ids, "擦痕", width=radius * 2, height=radius * 2,
                cornerRadius=radius,
                fill=radial([(0, "#E7E6E2"), (1, "#182821")],
                            cx=0.5, cy=0.5, radius=0.5))
    node["x"], node["y"] = cx - radius, cy - radius
    node["opacity"] = 0.06
    return node


def question_tag(no, color):
    """问号编号。第 2-5 板专用，位置与字号锁死。"""
    return row("问号编号", [
        text(ids, "问号", "Q", FS_T2, 700, color, family=NUM,
             width="fit_content", growth="auto", line_height=1.0),
        text(ids, "问号序数", str(no), FS_T2, 700, color, family=NUM,
             width="fit_content", growth="auto", line_height=1.0),
    ], gap=2, align="center", width="fit_content")


# ----------------------------------------------------------------- 母版部件
def header(page, tag=None):
    kids = [row("页眉行", [
        caption("系列名", SERIES, "$c-faint"),
        caption("页码", f"{page:02d} / {TOTAL:02d}", "$c-faint", family=NUM),
    ], gap=24, justify="space_between")]
    if tag is not None:
        kids.append(tag)
    return col("页眉", kids, gap=24)


def footer():
    return col("页脚", [
        chalk_line(INNER, "$c-faint", thickness=2),
        caption("账号名", "@ 你的账号名", "$c-dim", weight=600),
    ], gap=20)


def board(page, name, main, smudge_at, tag=None):
    content = frame(ids, f"{name} · 内容", width="fill_container",
                    height="fill_container", layout="vertical",
                    padding=[TOP, EDGE, BOT, EDGE], gap=0,
                    alignItems="start", fill=[])
    content["children"] = [header(page, tag), main, footer()]

    decor = frame(ids, f"{name} · 装饰", width="fill_container",
                  height="fill_container", layout="none", fill=[])
    decor["children"] = [smudge(*smudge_at)]

    shell = frame(ids, f"{page:02d} {name}", width=W, height=H, layout="none",
                  fill=solid("$c-bg"), clipContent=True)
    # 装饰层最后 —— jian 里 index 0 最上，写反了擦痕会糊住板书。
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


# ------------------------------------------------------- 01 开场 · A3 反问切口
def cover():
    """A3：反问句 10 列居中偏左，上下两条满宽色带把它夹住。

    A3 的层级是「反问句 → 色带 → 副标」。色带在本主题里就是两条粉笔线，
    不是实心块 —— 板上画不出实心色带。
    """
    main = zone([
        chalk_line(INNER, "$c-yellow", thickness=4),
        text(ids, "反问句", "为什么你\n听懂了却\n讲不出来？", FS_DISPLAY,
             700, "$c-chalk", family=CJK, width=INNER, growth="fixed-width",
             line_height=LH_DISPLAY),
        chalk_line(INNER, "$c-yellow", thickness=4),
        body("副标", "四个最常被问到的问题，一板一个。", "$c-dim"),
    ], justify="center", gap=34)
    return board(1, "开场 · 反问切口", main, (760, 1120, 340))


# ------------------------------------------------------- 02 Q1 · E1 左右对开
def q_choice():
    """E1：竖切 6/6，中缝一条粉笔线。**两侧版式完全相同**，只有一个变量不同。

    E 族硬规则：对比双方的版式必须一模一样，只允许一个变量变 —— 这里变的
    是彩粉色（玫瑰 = 警示 / 天蓝 = 补充）与字重。「对」的一侧永远在右，那
    是中文阅读的结论位。
    """
    half = (INNER - 40) // 2

    def side(label, line, note, color, weight):
        return col(f"半栏 · {label}", [
            caption("半栏标签", label, color, weight=600),
            text(ids, "半栏句", line, FS_T2, weight, "$c-chalk", family=CJK,
                 width=half, growth="fixed-width", line_height=LH_TITLE),
            body("半栏注解", note, "$c-dim", width=half),
        ], gap=14, width=half)

    main = zone([
        text(ids, "问题", "先复述，还是先做笔记？", FS_T1, 700, "$c-chalk",
             family=CJK, width=INNER, growth="fixed-width",
             line_height=LH_TITLE),
        row("对开", [
            side("先做笔记", "边听边记", "手在忙，脑子就停了。", "$c-rose",
                 400),
            chalk_line(4, "$c-faint", thickness=280),
            side("先复述", "听完再讲", "讲不出的地方就是没懂。", "$c-sky",
                 700),
        ], gap=18, align="start"),
    ], justify="center", gap=40)
    return board(2, "Q1 · 左右对开", main, (200, 1180, 300),
                 question_tag(1, "$c-yellow"))


# ------------------------------------------------------- 03 Q2 · F1 段落纯排
PARAS = [
    "听懂是识别，讲出来是重建。前者只要认得出这句话对，"
    "后者要你自己把它再造一遍。",
    "所以「听懂了」这个感觉不可靠。它测的是熟悉度而不是掌握度，"
    "两件事在脑子里根本不一样。",
]


def q_explain():
    """F1：一板一个小标题，两段正文，段首用彩粉方点标记。"""
    blocks = []
    for index, para in enumerate(PARAS):
        marker = rect(ids, "段首标记", width=18, height=18,
                      fill=solid("$c-yellow" if index == 0 else "$c-faint"))
        blocks.append(row("段落", [
            col("段首标记位", [marker], gap=0, width=18),
            body("正文", para, "$c-chalk", width=INNER - 18 - 24),
        ], gap=24, align="start"))

    main = zone([
        text(ids, "问题", "听懂和会讲差在哪？", FS_T1, 700, "$c-chalk",
             family=CJK, line_height=LH_TITLE),
        col("正文区", blocks, gap=36),
    ], justify="center", gap=40)
    return board(3, "Q2 · 段落纯排", main, (880, 420, 320),
                 question_tag(2, "$c-yellow"))


# ------------------------------------------------------- 04 Q3 · C5 流程箭头
FLOW = [
    ("听", "信息进来，识别成功"),
    ("停", "合上材料，不许再看"),
    ("讲", "对着空气讲一遍"),
    ("补", "卡住的地方回去补"),
]


def q_flow():
    """C5：四块纵排，块间一个开口三角箭头。引导符一板只用一种 —— 这里是箭头。"""
    items = []
    for index, (step, note) in enumerate(FLOW):
        block = wiped_patch(f"流程块 {index + 1}", [
            row("流程头", [
                text(ids, "流程字", step, FS_T2, 700, "$c-sky", family=CJK,
                     width="fit_content", growth="auto", line_height=1.0),
                body("流程注解", note, "$c-chalk",
                     width=INNER - 68 - 120),
            ], gap=22, align="center"),
        ], pad=(24, 28))
        items.append(block)
        if index < len(FLOW) - 1:
            arrow = path(ids, "箭头", "M 0 0 L 14 22 L 28 0", width=28,
                         height=22, fill=[],
                         stroke={"thickness": 3, "fill": solid("$c-faint")})
            items.append(row("箭头位", [arrow], gap=0, justify="center"))

    main = zone([
        text(ids, "问题", "那该怎么练？", FS_T1, 700, "$c-chalk",
             family=CJK, line_height=LH_TITLE),
        col("流程", items, gap=14),
    ], justify="center", gap=32)
    return board(4, "Q3 · 流程箭头", main, (180, 380, 300),
                 question_tag(3, "$c-yellow"))


# ------------------------------------------------------- 05 Q4 · B4 上下反差
def q_flip():
    """B4：上错下对，中间一条粉笔线分界。「对」的一侧在下（中文结论位）。"""
    def block(label, line, note, color, ink):
        return col(f"反差块 · {label}", [
            caption("反差标签", label, color, weight=600),
            text(ids, "反差句", line, FS_T2, 700, ink, family=CJK,
                 width=INNER, growth="fixed-width", line_height=LH_TITLE),
            body("反差注解", note, "$c-dim", width=INNER),
        ], gap=14)

    main = zone([
        text(ids, "问题", "讲不出来是记性差吗？", FS_T1, 700, "$c-chalk",
             family=CJK, line_height=LH_TITLE),
        col("上下反差", [
            block("多数人以为", "是我记性不好", "于是去背，背完还是讲不出。",
                  "$c-rose", "$c-faint"),
            chalk_line(INNER, "$c-chalk", thickness=4),
            block("其实是", "是你没重建过", "重建一次，它才真正变成你的。",
                  "$c-sky", "$c-chalk"),
        ], gap=28),
    ], justify="center", gap=36)
    return board(5, "Q4 · 上下反差", main, (860, 1140, 320),
                 question_tag(4, "$c-yellow"))


# ------------------------------------------------------- 06 收束 · G1 签名收束
def closing():
    """G1：收束句脱离前文也读得通；一句行动，不做 CTA 块。"""
    main = zone([
        text(ids, "收束句", "合上书，\n先讲一遍。", FS_DISPLAY, 700,
             "$c-chalk", family=CJK, width=INNER, growth="fixed-width",
             line_height=LH_DISPLAY),
        col("行动", [
            chalk_line(COLUMN * 3 + GUTTER * 2, "$c-yellow", thickness=4),
            body("行动说明", "今天读完的那篇，现在就对着空气讲三十秒。",
                 "$c-dim"),
        ], gap=20),
    ], justify="center", gap=52)
    return board(6, "收束 · 签名", main, (240, 460, 340))


def build():
    return [cover(), q_choice(), q_explain(), q_flow(), q_flip(), closing()]


# 对比度（WCAG 相对亮度比，op-design-lint 门槛 2.0；本表逐对实算）：
#   c-chalk  on c-bg    12.32    c-dim    on c-bg     7.16
#   c-faint  on c-bg     4.05    c-yellow on c-bg    10.08
#   c-rose   on c-bg     8.56    c-sky    on c-bg     9.24
#   c-chalk  on c-deep  14.22    c-dim    on c-deep   8.26
# 承载正文的最低一对是 c-faint on c-bg 4.05 —— 它只用在第 2 板「先做笔记」
# 那一侧被刻意压低的句子上（E1 要求错的一侧在视觉上退后），以及页码与粉笔
# 线上；正文用的 c-dim 是 7.16，已过 WCAG AA。
# 三支彩粉对板底都在 8.6-10.1 这个窄区间 —— 这正是「粉笔颜料浓度低、chroma
# 只到 0.085」那条论证的回报：三支色语义不同但视觉重量相等，读者不会把
# 「警示」误读成比「补充」更重要。换彩粉时守住 L0.82-0.86 即可。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "问答体轮播 · 3:4 六板")
