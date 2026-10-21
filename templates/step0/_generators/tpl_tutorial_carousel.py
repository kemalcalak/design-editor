#!/usr/bin/env python3
"""tutorial-journal-carousel.op — 教程轮播（6 板 · 1080×1440 · 3:4）

叙事类型：**教程**，但和信息长图那一档的教程不是一回事。长图的教程可以把
八个步骤铺在一屏里让人上下找；轮播的教程必须**一板一步**——读者的手指就
是进度条，划一次等于做完一步。所以本套的节奏是「钩子 → 备料 → 第一步做
给你看 → 剩下三步递进 → 一句提醒 → 三个动作」，每板结束时读者手上都该多
出一个已完成的动作。

这也是本套和 `tutorial` 档模板的分野：那边是「查得到」，这边是「跟得上」。

### 主题：T6 奶油手帐 `cream-journal`（亮 / 安静 / 生活·教程）

**采样 → 收敛 → 论证**（沿用 card-system-0808 §3 T6）

  - 采样：奶油色内页纸、三种低饱和纸胶带（杏 / 鼠尾草绿 / 淡紫）、2B 铅笔
    灰、缝线的锈红。
  - 收敛：底 = 奶油 L0.955 C0.022；三支胶带色 L 全锁在 0.86、chroma
    0.045-0.070；中性 = 铅笔灰序列；点睛 = 锈红。
  - 论证：**三支胶带色的明度锁死在同一档 L0.86** —— 胶带是同一种材料的不
    同颜色，明度不齐就变成「三个色块」而不是「三条胶带」。材料一致性优先
    于色彩对比，这是本主题的核心决定。选它承载教程：手帐是中文语境里
    「一个人把方法记下来给自己看」的形状，教程要的正是这种「可照抄」的
    亲切，而不是课堂的权威感。

**最近邻论证**：库里最近的是 `knowledge-card-vertical`（暖纸 #FFF7F0 +
单一朱红强调）。两者都是暖亮底，分野在**装饰语言**：那张卡整页零装饰、
靠序号圆和短线立层级；本套的层级由胶带、活页孔、缝线这套纸质语汇承担，
且底色 #F6F0E0 比它更黄一档（C0.022 vs C0.012）。缩略图里一张是「白卡」，
一张是「本子」。

### 母版规则（六板的硬约束）

**每板固定，不许变**
  1. 画幅 1080×1440，安全边距 左右 80 / 上 96 / 下 128。左侧另留 56px 给
     活页孔，所以正文实际左界是 136。
  2. **活页孔**：左缘 6 个直径 28 的孔，间距 96，纵向居中。六板一个不差 ——
     它是本套最强的一致性信号，也是「这是同一个本子的六页」的物理依据。
  3. 页眉：左「跟着做 · 六板」，右「NN / 06」（Inter，Caption 32px）。
  4. 页脚：一道 14 段的**缝线**（锈红短虚线）+ 署名。
  5. 字族：汉字 Noto Sans SC / 数字与页码 Inter。
  6. 每板 1-2 条胶带，旋转角只在 1-3°，且**不可平行等距**。

**允许变，且只有这些能变**
  - 主体区的信息结构（下面六板各用一条配方）。
  - 当板用哪支胶带色、贴在哪、转几度。
  - **锈红点睛**：每板最多一处（页脚缝线不计，它是母版）。

### 配方编排（card-system-0808 §4.2）

    01 A1 巨字压顶 → 02 C3 勾选清单 → 03 F2 图注混排
    04 C2 卡片阶梯 → 05 B3 高亮切词 → 06 G2 三键引导

首板 A 族、末板 G 族；相邻两板不同族；覆盖 A/C/F/B/G 五族；B 族出现 1 次
且落在后 1/3（第 5 板）。C 族出现 2 次但不相连（第 2、4 板）。
本主题禁配 A2 / D 族 / E3 / C5，编排已全部避开。

### 负约束（本模板明令不做的事）

  - **胶带不可平行等距**。两条胶带若角度相同、间距相同，纸质感立刻塌成
    「模板里的两个装饰条」——手贴的东西不会那么齐。
  - **旋转角只在 1-3°**。超过 3° 就从「随手贴」变成「歪了」，那是失误不是
    质感。
  - 不用手绘涂鸦、不用贴纸 emoji、不用彩虹配色。三支胶带色一板最多用两支。
  - 不用阴影堆出来的「立体便签」。纸的层次由明度差和胶带给，不由投影给。
  - 不做流程图与箭头链（C5 是本主题禁配）——手帐不画流程图，它列清单。
  - 不写「亲测有效 / 保姆级 / 一看就会」这类标题党词。
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

from oplib import (Ids, PLACEHOLDER_DISC, PLACEHOLDER_ICON, PLACEHOLDER_SPEC,
                   PLACEHOLDER_TITLE, color_vars, frame, group, icon_font,
                   path, rect, solid, text, upload_disc, write_doc)

ids = Ids()

VARS = color_vars({
    "c-bg":      "#F6F0E0",   # cream         奶油内页
    "c-deep":    "#EAE1CB",   # cream.deep    卡片 / 内页面
    "c-apricot": "#F3C7A2",   # tape.apricot  胶带 A
    "c-sage":    "#C2D9BD",   # tape.sage     胶带 B
    "c-lilac":   "#D8CEEE",   # tape.lilac    胶带 C
    "c-ink":     "#39342F",   # ink.pencil    主文字
    "c-soft":    "#69625B",   # ink.soft      次级文字
    "c-rust":    "#A65438",   # thread.rust   点睛：一板一处
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
RING_GUTTER = 56              # 活页孔占掉的左侧宽度
TEXT_EDGE = EDGE + RING_GUTTER
TOP, BOT = 96, 128
INNER = W - TEXT_EDGE - EDGE  # 884
COLUMN, GUTTER = 62, 16

FS_DISPLAY, FS_T1, FS_T2 = 88, 64, 48
FS_BODY, FS_CAPTION = 40, 32
LH_DISPLAY, LH_TITLE, LH_BODY, LH_CAPTION = 1.15, 1.3, 1.7, 1.5

SERIES = "跟着做 · 六板"
TOTAL = 6
RING_D, RING_STEP, RING_N = 28, 96, 6


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


def caption(name, content, color="$c-soft", *, family=CJK, weight=400,
            width="fit_content"):
    return text(ids, name, content, FS_CAPTION, weight, color, family=family,
                width=width, growth="auto" if width == "fit_content"
                else "fixed-width", line_height=LH_CAPTION)


def body(name, content, color="$c-soft", *, weight=400,
         width="fill_container"):
    return text(ids, name, content, FS_BODY, weight, color, family=CJK,
                width=width, growth="auto" if width == "fit_content"
                else "fixed-width", line_height=LH_BODY)


def sheet(name, children, *, gap=20, pad=(32, 34), fill="$c-deep",
          width="fill_container"):
    """内页面。纸上贴的一张纸，靠明度差立层级，不靠阴影。"""
    node = frame(ids, name, width=width, height="fit_content",
                 layout="vertical", gap=gap, padding=list(pad),
                 cornerRadius=6, alignItems="start", fill=solid(fill))
    node["children"] = children
    return node


# ----------------------------------------------------------------- 纸质语汇
def tape_path_d(w, h, teeth=5, depth=9):
    """一条纸胶带的轮廓：上下两边平直，左右两端各 5 齿锯齿。

    锯齿走 `path` 的 `d` 而不是拿几个小方块拼 —— 拼出来的齿是独立节点，用
    户拖动胶带时会散架；一条 path 是一个整体，怎么挪都还是一条胶带。
    """
    step = h / teeth
    pts = [f"M 0 0", f"L {w} 0"]
    for i in range(teeth):                       # 右端：外→内交替
        y = round(step * (i + 1), 2)
        x = w - depth if i % 2 == 0 else w
        pts.append(f"L {x} {y}")
    pts.append(f"L 0 {h}")
    for i in range(teeth):                       # 左端：回程同样交替
        y = round(h - step * (i + 1), 2)
        x = depth if i % 2 == 0 else 0
        pts.append(f"L {x} {y}")
    pts.append("Z")
    return " ".join(pts)


# 正文柱：胶带层压在内容之上，所以胶带**绝不能**落进这个矩形。
CONTENT_BOX = (TEXT_EDGE, TOP, W - EDGE, H - BOT)
# 1-3° 旋转会让长边的端点往外甩 w·sin(3°)；300 宽的胶带甩出约 16px，留 24。
TILT_SLACK = 24


def tape(name, x, y, w, h, color, angle):
    """一条胶带。半透明 + 1-3° 旋转，贴在内容层**之上**。

    透明度用节点 opacity 而不是把颜色调淡：胶带压住的是纸，纸纹要透出来才
    像胶带。底色不受影响 —— lint 量对比度只往祖先链上走，装饰兄弟不参与。

    **位置有硬约束**：胶带层在正文之上，一条贴进正文柱的胶带会直接盖掉字
    （第一版就把第 4 板标题的最后两个字压没了，几何审计还查不出来 —— 它
    只管文字压文字）。真实的本子上胶带也本来就贴在页边、压住纸角，所以这
    里把它钉死在正文柱外，并在构建时断言，让「贴歪了」变成构建失败而不是
    发出去之后才发现。
    """
    left, top, right, bottom = CONTENT_BOX
    box = (x - TILT_SLACK, y - TILT_SLACK, x + w + TILT_SLACK,
           y + h + TILT_SLACK)
    assert box[2] <= left or box[0] >= right or box[3] <= top \
        or box[1] >= bottom, f"{name} 压进正文柱：{box} vs {CONTENT_BOX}"
    assert 1 <= abs(angle) <= 3, f"{name} 旋转角 {angle} 超出 1-3°"
    node = path(ids, name, tape_path_d(w, h), width=w, height=h,
                fill=solid(color))
    node["x"], node["y"] = x, y
    node["rotation"] = angle
    node["opacity"] = 0.85
    return node


def ring_holes():
    """活页孔：左缘 6 个孔，纵向居中。六板逐板重建，位置由常量算出，不手写。"""
    total = RING_D + RING_STEP * (RING_N - 1)
    top = round((H - total) / 2)
    holes = []
    for i in range(RING_N):
        hole = {
            "type": "ellipse", "id": ids("e"), "name": f"活页孔 {i + 1}",
            "width": RING_D, "height": RING_D, "fill": solid("$c-bg"),
            "stroke": {"thickness": 2, "fill": solid("$c-deep")},
            "x": round(EDGE - RING_D / 2 + 6), "y": top + RING_STEP * i,
        }
        holes.append(hole)
    return holes


def stitch_line(color="$c-rust", segments=14):
    """缝线：14 段短虚线排成一行。

    不用 `line` 节点加 dashPattern —— 那是描边属性，在 flex 行里宽度行为不
    可预期。14 个等宽小方块用 space_between 排开，是纯 flex 的、量得准的。
    """
    dashes = [rect(ids, f"缝 {i + 1}", width=22, height=3,
                   fill=solid(color)) for i in range(segments)]
    return row("缝线", dashes, gap=0, justify="space_between",
               width="fill_container")


# ----------------------------------------------------------------- 母版部件
def header(page):
    return row("页眉", [
        caption("系列名", SERIES, "$c-soft"),
        caption("页码", f"{page:02d} / {TOTAL:02d}", "$c-soft", family=NUM),
    ], gap=24, justify="space_between")


def footer():
    return col("页脚", [
        stitch_line(),
        caption("账号名", "@ 你的账号名", "$c-soft", weight=600),
    ], gap=20)


def board(page, name, main, tapes):
    """一板。活页孔在最底层，正文居中，胶带贴在最上层。

    这里没用 oplib.stack：stack 只给「内容 + 一层装饰（在下）」，而胶带按
    定义必须压在内容**上面**。所以外壳自己搭三层，靠 children 顺序定 z ——
    jian 的兄弟顺序是 index 0 最上（canvas_viewport_paint_mask 反着遍历）。
    胶带层用 group：group 不是图片投放目标（image_drop.rs 不匹配 Group），
    所以它盖在整板上也不会截走用户拖进来的图。

    两个装饰层都上 `locked`：它们是 fill_container 的满幅壳子，压在正文之
    上，但自己一寸也不画（真正的墨在里面那几条胶带 / 六个孔上）。locked 让
    壳子自身不可选，壳内的胶带和孔照常可选（hit_test_walk 的 locked 判定在
    子节点遍历之后），于是拖动装饰不会拽走整层，框选也不会一网兜住整板。
    """
    content = frame(ids, f"{name} · 内容", width="fill_container",
                    height="fill_container", layout="vertical",
                    padding=[TOP, EDGE, BOT, TEXT_EDGE], gap=0,
                    alignItems="start", fill=[])
    content["children"] = [header(page), main, footer()]

    tape_layer = group(ids, f"{name} · 胶带", width="fill_container",
                       height="fill_container", layout="none", fill=[],
                       locked=True)
    tape_layer["children"] = tapes

    holes = frame(ids, f"{name} · 活页孔", width="fill_container",
                  height="fill_container", layout="none", fill=[],
                  locked=True)
    holes["children"] = ring_holes()

    shell = frame(ids, f"{page:02d} {name}", width=W, height=H, layout="none",
                  fill=solid("$c-bg"), clipContent=True)
    shell["children"] = [tape_layer, content, holes]
    shell["x"] = ((page - 1) % BOARDS_PER_ROW) * (W + GAP)
    shell["y"] = ((page - 1) // BOARDS_PER_ROW) * (H + ROW_GAP)
    return shell


def zone(children, *, justify="center", gap=32, pad_y=48):
    node = frame(ids, "主体", width="fill_container", height="fill_container",
                 layout="vertical", gap=gap, padding=[pad_y, 0],
                 justifyContent=justify, alignItems="start", fill=[])
    node["children"] = children
    return node


# ------------------------------------------------------- 01 封面 · A1 巨字压顶
def cover():
    """A1：主标压顶占约 38%，副标与眉标在下。一板一个钩子，不并列卖点。"""
    main = zone([
        caption("眉标", "四步整理法 · 每板一步", "$c-rust", weight=600),
        text(ids, "主标", "十分钟\n把桌面清空", FS_DISPLAY, 700, "$c-ink",
             family=CJK, width=INNER, growth="fixed-width",
             line_height=LH_DISPLAY),
        body("副标", "不是收纳技巧，是一套决定东西去哪的顺序。", "$c-soft"),
    ], justify="center", gap=36)
    return board(1, "封面 · 巨字压顶", main, [
        tape("胶带 · 杏", 742, -48, 300, 84, "$c-apricot", 2),
        tape("胶带 · 淡紫", -58, 1336, 214, 76, "$c-lilac", -3),
    ])


# ------------------------------------------------------- 02 备料 · C3 勾选清单
CHECKS = [
    "一个空纸箱，装待定的东西",
    "一个垃圾袋，装确定要扔的",
    "一块抹布，最后一步用",
    "一个计时器，定十分钟",
    "把手机放到另一个房间",
]


def prep():
    """C3：单列勾选框，5 条，每条 1 行。引导符一板只用一种 —— 这里是方框。"""
    items = []
    for line in CHECKS:
        box = rect(ids, "勾选框", width=36, height=36, cornerRadius=4,
                   fill=[], stroke={"thickness": 3, "fill": solid("$c-ink")})
        items.append(row("清单项", [
            box,
            body("清单文字", line, "$c-ink", width=INNER - 36 - 24 - 68),
        ], gap=24, align="center"))

    main = zone([
        text(ids, "小标题", "开始前，先备这五样", FS_T1, 700, "$c-ink",
             family=CJK, line_height=LH_TITLE),
        sheet("备料清单", [col("清单", items, gap=22)], pad=(34, 34)),
    ], justify="center", gap=40)
    return board(2, "备料 · 勾选清单", main, [
        tape("胶带 · 鼠尾草", 604, 1338, 336, 80, "$c-sage", -2),
    ])


# ------------------------------------------------------- 03 第一步 · F2 图注混排
def step_one():
    """F2：视觉块占约 44%，下面是标题 + 正文 + 图注。图位是内容不是装饰。"""
    disc = upload_disc(ids, "上传占位", 116, PLACEHOLDER_DISC, 42,
                       PLACEHOLDER_ICON)
    hint = col("占位说明", [
        text(ids, "占位标题", "拍一张开始前的桌面", FS_BODY, 600,
             PLACEHOLDER_TITLE, family=CJK, width="fit_content",
             growth="auto", line_height=LH_TITLE),
        text(ids, "占位规格", "建议 884×580 以上", FS_CAPTION, 400,
             PLACEHOLDER_SPEC, family=CJK, width="fit_content",
             growth="auto", line_height=LH_CAPTION),
    ], gap=8, width="fit_content", align="center")
    slot = frame(ids, "照片位", width="fill_container", height=580,
                 layout="vertical", gap=22, alignItems="center",
                 justifyContent="center", cornerRadius=6,
                 fill=solid("$c-deep"))
    slot["children"] = [disc, hint]

    main = zone([
        slot,
        col("图注区", [
            text(ids, "步骤标题", "第一步 · 全部拿下来", FS_T2, 700,
                 "$c-ink", family=CJK, line_height=LH_TITLE),
            body("正文", "桌面清成一块空板，连你觉得本来就该在那儿的几样也拿"
                 "下来。留着的东西会替你做决定。", "$c-ink"),
            caption("图注", "拍下来 · 十分钟后你会想看这张", "$c-soft"),
        ], gap=14),
    ], justify="center", gap=32)
    return board(3, "第一步 · 图注混排", main, [
        tape("胶带 · 杏", -62, -44, 262, 78, "$c-apricot", 3),
    ])


# ------------------------------------------------------- 04 递进 · C2 卡片阶梯
LADDER = [
    ("第二步", "分三堆", "常用、待定、不要。三堆，没有第四堆。",
     "$c-bg"),
    ("第三步", "先处理不要那堆", "直接进垃圾袋封口，不要再看第二眼。",
     "$c-deep"),
    ("第四步", "常用的才回桌面", "待定那箱封好，一个月没开就扔。",
     "$c-apricot"),
]


def ladder():
    """C2：三张卡纵排，逐张左缩进一列、底色加深一档 —— 缩进本身就是递进。"""
    cards = []
    for index, (label, title, note, fill) in enumerate(LADDER):
        card = sheet(f"阶梯卡 {index + 1}", [
            caption("阶梯标签", label, "$c-rust" if index == 2 else "$c-soft",
                    weight=600),
            text(ids, "阶梯标题", title, FS_T2, 700, "$c-ink", family=CJK,
                 line_height=LH_TITLE),
            body("阶梯注解", note, "$c-soft"),
        ], gap=12, pad=(28, 30), fill=fill)
        if fill == "$c-bg":
            card["stroke"] = {"thickness": 2, "fill": solid("$c-deep")}
        wrap = row(f"阶梯行 {index + 1}", [card], gap=0, align="start")
        if index:
            wrap["padding"] = [0, 0, 0, (COLUMN + GUTTER) * index]
        cards.append(wrap)

    main = zone([
        text(ids, "小标题", "剩下三步，一层比一层狠", FS_T1, 700, "$c-ink",
             family=CJK, line_height=LH_TITLE),
        col("阶梯", cards, gap=20),
    ], justify="center", gap=36)
    return board(4, "递进 · 卡片阶梯", main, [
        tape("胶带 · 淡紫", 1032, 470, 252, 74, "$c-lilac", -2),
    ])


# ------------------------------------------------------- 05 提醒 · B3 高亮切词
def reminder():
    """B3：被击中的两个字先出来，整句在后，注解压到 Caption。

    高亮在本主题里只有一种合法形态 —— 一小段胶带压在词底下。这里把它做成
    行内的一个 `fit_content` 容器，而不是装饰层的绝对定位，因为它属于内容。
    """
    hit = frame(ids, "被击中的词", width="fit_content", height="fit_content",
                layout="horizontal", padding=[14, 26], cornerRadius=4,
                alignItems="center", justifyContent="center",
                fill=solid("$c-sage"))
    hit["children"] = [
        text(ids, "词", "待定", FS_DISPLAY, 700, "$c-ink", family=CJK,
             width="fit_content", growth="auto", line_height=1.0),
    ]

    main = zone([
        hit,
        text(ids, "整句", "最容易漏掉的是那一箱。", FS_T1, 700,
             "$c-ink", family=CJK, width=INNER, growth="fixed-width",
             line_height=LH_TITLE),
        caption("注解", "在箱子上写好日期，一个月后不用再犹豫。", "$c-soft",
                width=INNER),
    ], justify="center", gap=40)
    return board(5, "提醒 · 高亮切词", main, [
        tape("胶带 · 杏", 812, -42, 282, 76, "$c-apricot", 2),
    ])


# ------------------------------------------------------- 06 收尾 · G2 三键引导
ACTIONS = [
    ("bookmark", "收藏", "留着下次照做"),
    ("message-circle", "评论", "说说卡在第几步"),
    ("user-plus", "关注", "下一板讲抽屉"),
]


def closing():
    """G2：三块等分、等高，三个动作必须**不同**（收藏 / 评论 / 关注）。"""
    blocks = []
    for glyph, label, note in ACTIONS:
        block = frame(ids, f"动作 · {label}", width="fill_container",
                      height="fill_container", layout="vertical", gap=14,
                      padding=[28, 20], cornerRadius=6, alignItems="center",
                      justifyContent="center", fill=solid("$c-deep"))
        block["children"] = [
            icon_font(ids, "动作图标", glyph, 44, "$c-ink"),
            text(ids, "动作名", label, FS_T2, 700, "$c-ink", family=CJK,
                 width="fit_content", growth="auto", line_height=1.0),
            text(ids, "动作说明", note, FS_CAPTION, 400, "$c-soft",
                 family=CJK, width="fill_container", growth="fixed-width",
                 align="center", line_height=LH_CAPTION),
        ]
        blocks.append(block)

    main = zone([
        text(ids, "收束句", "先清空，\n再决定放什么回去。", FS_DISPLAY, 700,
             "$c-ink", family=CJK, width=INNER, growth="fixed-width",
             line_height=LH_DISPLAY),
        row("三键", blocks, gap=20, align="start"),
    ], justify="center", gap=52)
    return board(6, "收尾 · 三键引导", main, [
        tape("胶带 · 鼠尾草", 386, 1340, 320, 82, "$c-sage", -3),
    ])


def build():
    return [cover(), prep(), step_one(), ladder(), reminder(), closing()]


# 对比度（WCAG 相对亮度比，op-design-lint 门槛 2.0；本表逐对实算）：
#   c-ink  on c-bg      10.82    c-soft on c-bg       5.28
#   c-rust on c-bg       4.68    c-ink  on c-deep     9.45
#   c-soft on c-deep     4.61    c-rust on c-deep     4.09
#   c-ink  on c-apricot  7.91    c-soft on c-apricot  3.86
#   c-ink  on c-sage     8.18    c-ink  on c-lilac    8.20
#   c-deep on c-bg       1.14
# 承载正文的最低一对是 c-soft on c-apricot 3.86 —— 第 4 板最后一张阶梯卡
# 的注解，高出 lint 门槛 1.93 倍。其余承载正文处最低 4.61。三支胶带色的
# 明度锁在同一档 L0.86，所以 c-ink 压在它们上面永远落在 7.9-8.2 这个窄区
# 间 —— 换胶带色只要守住 L0.86，这张表就不用重量，这正是「材料一致性优先
# 于色彩对比」那条论证在对比度上的回报。c-deep on c-bg 1.14 低于门槛是对
# 的：那是纸上叠纸的明度差，属非文字图形，绝不拿它写字。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "教程轮播 · 3:4 六板")
