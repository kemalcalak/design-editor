#!/usr/bin/env python3
"""bookreview-silk-carousel.op — 书 / 影评拆解轮播（5 板 · 1080×1440 · 3:4）

叙事类型：**拆解**。书评影评最常见的失败是复述剧情 —— 那是摘要不是评。
拆解的动作是「把一件作品拆成几个能拿走的零件」，所以本套的骨架是：一个
钩子 → 一段带注解的原文 → 三个洞见 → 一句书摘 → 收束。第 2 板那个侧栏
是全套的方法论所在：**正文是作者的，侧栏是你的**，两栏并置就是「评」这个
动作本身。

五板是刻意的下限。书评的读者是奔着「值不值得看」来的，超过五板他就该去
读原作了。

### 主题：T2 绢本青绿 `qinglv-silk`（亮 / 安静 / 长文）

**采样 → 收敛 → 论证**（沿用 card-system-0808 §3 T2）

  - 采样：老化绢本的暖黄底、石青、石绿、山脚赭石、绢上墨（偏褐，不是纯
    黑）。
  - 收敛：底 = 绢黄 L0.93 C0.030；有彩两支（青 H240 / 绿 H158，H 差 82°）；
    中性 = 褐墨序列。
  - 论证：**墨色取 L0.30 且带 0.020 暖 chroma** —— 绢面吸墨少、反射暖，纯
    黑写在绢上会「浮」，褐墨才坐得住。这一步是本主题和任何「青绿配色」的
    分野。选它承载书评：绢本是长阅读的载体，青绿山水又是中文里「远观一件
    完整的东西」的视觉母题 —— 拆解一部作品，正是站远一点看它的形状。

**最近邻论证**：库里最近的亮暖底是 `knowledge-card-vertical`（#FFF7F0，
L0.97 C0.012 暖白）与本批的 `cream-journal`（#F6F0E0，L0.955 C0.022）。本
套 `#F0E7D2` 是 L0.93 C0.030 —— 三者在「白到奶油」这条线上按 chroma 排成
0.012 / 0.022 / 0.030 的阶梯，是人眼在缩略图尺寸下真正能排序的维度。装饰
语言上更是三套东西：那张卡零装饰、手帐贴胶带、本套压一道山形渐层。

### 母版规则（五板的硬约束）

**每板固定，不许变**
  1. 画幅 1080×1440，安全边距 左右 80 / 上 96 / **下 300**。
  2. **山形渐层**：每板底部一条 300px 高的山形 `path`（占 20.8%，落在配方
     要求的 18-28% 区间内），填 `linear(90°, 石青 → 石绿)`。它是本套的签
     名结构，也是下边距被拉到 300 的原因 —— **山在下面，正文永远不压到它**。
  3. 页眉：左「拆书 · 五板」，右「NN / 05」（Inter，Caption 32px）。
  4. 页脚：一条 1px 赭石分隔线 + 署名，坐在山形之上。
  5. 字族：汉字 Noto Sans SC / 数字与页码 Inter。
  6. 石青与石绿**永远成对出现**（单独用青即偏离主题），且只出现在山形、
     侧栏标记、引号这三处。

**允许变，且只有这些能变**
  - 主体区的信息结构（下面五板各用一条配方）。
  - 山形的峰线（五板各一条，峰位不同 —— 同一座山画五遍就成了印章）。
  - **绢纹**：整套只用一次，只在封面（第 1 板）铺满。

### 配方编排（card-system-0808 §4.2）

    01 A5 竖排书脊 → 02 F3 侧栏注解 → 03 C1 编号纵列
    04 B2 引号框景 → 05 G1 签名收束

首板 A 族、末板 G 族；相邻两板不同族；覆盖 A/F/C/B/G 五族（5 板要求 ≥4）；
B 族出现 1 次且落在后 1/3（第 4 板，是全套的复述点）。

### 负约束（本模板明令不做的事）

  - **不画松、鹤、亭、舟、印章**。一旦出现这些具象符号，主题就从「绢本
    重彩的色彩结构」塌成「新中式装修」。
  - **青绿渐层不可用于文字色，也不可作大面积底** —— 它是「山」，山在下面。
    整套只有山形那一处用渐变。
  - **不用红色做印章式点缀**。那是另一套语汇，会把主题拉去传统海报。
  - 不用竖排 + 印章 + 水墨的「新中式三件套」。竖排在本套只出现在封面书脊
    那一处，且不配印章。
  - 不复述剧情、不打星级评分、不写「必读 / 神作 / 封神」这类词。
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

from oplib import (Ids, color_vars, frame, group, linear, path, rect, solid,
                   text, write_doc)

ids = Ids()

VARS = color_vars({
    "c-bg":        "#F0E7D2",   # silk           绢黄
    "c-deep":      "#E3D6BC",   # silk.deep      分区 / 卡片
    "c-ink":       "#362C24",   # ink.brown      主文字（褐墨）
    "c-soft":      "#665B54",   # ink.soft       次级文字
    "c-azurite":   "#347CA9",   # azurite.hi     石青（渐层上段）
    "c-azurite-d": "#114C71",   # azurite.deep   石青（承载白字）
    "c-malachite": "#5E9877",   # malachite.hi   石绿（渐层下段）
    "c-malachite-d": "#376A50",  # malachite.deep 石绿（承载白字）
    "c-ochre":     "#B59979",   # ochre.mount    赭石：分隔线 / 山脚
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
TOP, BOT = 96, 300            # 下边距被山形吃掉，见母版规则 2
INNER = W - EDGE * 2
COLUMN, GUTTER = 62, 16

FS_DISPLAY, FS_T1, FS_T2 = 88, 64, 48
FS_BODY, FS_CAPTION = 40, 32
LH_DISPLAY, LH_TITLE, LH_BODY, LH_CAPTION = 1.15, 1.3, 1.7, 1.5

SERIES = "拆书 · 五板"
TOTAL = 5
MOUNT_H = 300


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


# ----------------------------------------------------------------- 绢本语汇
# 山形填色写字面值而不是设计变量：渐变 stop 是整套里唯一「两支色必须成对」
# 的地方，把它绑成变量，用户改一支就会拆散这一对，而这一对正是主题的定义
# （单独用青即偏离主题）。写死是为了让它改不动。
AZURITE_HI, MALACHITE_HI = "#347CA9", "#5E9877"

# 五条峰线。同一座山画五遍就成了印章，所以每板一条，峰位各不相同。
# 每条是 (x 比例, 峰高比例) 的序列，x 覆盖 0→1，y 越小峰越高。
RIDGES = [
    [(0.00, 0.62), (0.18, 0.28), (0.34, 0.55), (0.58, 0.12),
     (0.78, 0.48), (1.00, 0.34)],
    [(0.00, 0.40), (0.22, 0.66), (0.44, 0.20), (0.66, 0.58),
     (0.86, 0.30), (1.00, 0.52)],
    [(0.00, 0.55), (0.14, 0.18), (0.40, 0.62), (0.62, 0.36),
     (0.84, 0.14), (1.00, 0.46)],
    [(0.00, 0.24), (0.26, 0.58), (0.48, 0.32), (0.70, 0.64),
     (0.88, 0.22), (1.00, 0.40)],
    [(0.00, 0.48), (0.20, 0.34), (0.42, 0.68), (0.64, 0.22),
     (0.82, 0.56), (1.00, 0.26)],
]


def mountain(page):
    """山形：一条折线峰脊 + 左右下三边闭合，填石青→石绿的竖向渐层。

    `d` 由比例算出而不是手写坐标，所以改画幅时峰形自动跟着走。渐层方向按
    主题定义：**青在上、绿在下**（石青是渐层上段），反过来山就翻过来了。
    """
    ridge = RIDGES[(page - 1) % len(RIDGES)]
    pts = [f"M {round(x * W, 1)} {round(y * MOUNT_H, 1)}"
           if i == 0 else f"L {round(x * W, 1)} {round(y * MOUNT_H, 1)}"
           for i, (x, y) in enumerate(ridge)]
    pts += [f"L {W} {MOUNT_H}", f"L 0 {MOUNT_H}", "Z"]
    node = path(ids, f"山形 {page}", " ".join(pts), width=W, height=MOUNT_H,
                fill=linear(90, [(0, AZURITE_HI), (1, MALACHITE_HI)]))
    node["x"], node["y"] = 0, H - MOUNT_H
    return node


def silk_weave(spacing=16):
    """绢纹：等距横线，赭石压到 10%。整套只用一次，只在封面。

    用节点 opacity 而不是把赭石调淡：绢纹是织物的物理纹理，压在底色上应该
    随底色走；调淡颜色就把它变成了「另一个更浅的赭石」，换底色时会露馅。
    """
    lines = []
    for i in range(H // spacing):
        line = rect(ids, f"绢纹 {i + 1}", width=W, height=1,
                    fill=solid("$c-ochre"))
        line["x"], line["y"] = 0, i * spacing
        line["opacity"] = 0.1
        lines.append(line)
    return lines


# ----------------------------------------------------------------- 母版部件
def header(page):
    return row("页眉", [
        caption("系列名", SERIES, "$c-soft"),
        caption("页码", f"{page:02d} / {TOTAL:02d}", "$c-soft", family=NUM),
    ], gap=24, justify="space_between")


def footer():
    return col("页脚", [
        rect(ids, "赭石分隔线", width="fill_container", height=1,
             fill=solid("$c-ochre")),
        caption("账号名", "@ 你的账号名", "$c-soft", weight=600),
    ], gap=18)


def board(page, name, main, extra_decor=None):
    """一板。山形永远在装饰层，正文柱的下界因此被抬到 H-300。"""
    content = frame(ids, f"{name} · 内容", width="fill_container",
                    height="fill_container", layout="vertical",
                    padding=[TOP, EDGE, BOT, EDGE], gap=0,
                    alignItems="start", fill=[])
    content["children"] = [header(page), main, footer()]

    decor = frame(ids, f"{name} · 装饰", width="fill_container",
                  height="fill_container", layout="none", fill=[])
    decor["children"] = (extra_decor or []) + [mountain(page)]

    shell = frame(ids, f"{page:02d} {name}", width=W, height=H, layout="none",
                  fill=solid("$c-bg"), clipContent=True)
    # 装饰层是最后一个孩子 —— jian 里 index 0 最上，写反了山会盖住正文。
    shell["children"] = [content, decor]
    shell["x"] = ((page - 1) % BOARDS_PER_ROW) * (W + GAP)
    shell["y"] = ((page - 1) // BOARDS_PER_ROW) * (H + ROW_GAP)
    return shell


def zone(children, *, justify="center", gap=32, pad_y=40):
    node = frame(ids, "主体", width="fill_container", height="fill_container",
                 layout="vertical", gap=gap, padding=[pad_y, 0],
                 justifyContent=justify, alignItems="start", fill=[])
    node["children"] = children
    return node


# ------------------------------------------------------- 01 封面 · A5 竖排书脊
SPINE = "拆书"


def spine():
    """竖排书脊：一字一节点纵排，贴右缘。整套只用一次，且不配印章。"""
    glyphs = [text(ids, f"书脊字 {i}", ch, FS_T1, 700, "$c-azurite-d",
                   family=CJK, width="fit_content", growth="auto",
                   line_height=1.0)
              for i, ch in enumerate(SPINE, 1)]
    return col("竖排书脊", glyphs, gap=16, width="fit_content", align="center")


def cover():
    """A5：竖排贴右 2 列，左 9 列留主标与副标。标题占画面约 34%。"""
    left = col("封面文案", [
        caption("眉标", "读完之后能拿走的三个零件", "$c-malachite-d",
                weight=600),
        text(ids, "主标", "这本书\n真正在讲的\n是选择", FS_DISPLAY, 700,
             "$c-ink", family=CJK, width=COLUMN * 8 + 112,
             growth="fixed-width", line_height=LH_DISPLAY),
        body("副标", "不复述情节，只拆它的结构。", "$c-soft",
             width=COLUMN * 8 + 112),
    ], gap=30)

    main = zone([
        row("封面版心", [left, spine()], gap=40, align="start",
            justify="space_between"),
    ], justify="center", gap=0)
    return board(1, "封面 · 竖排书脊", main, silk_weave())


# ------------------------------------------------------- 02 原文 · F3 侧栏注解
# F3 原配方是「主栏 8 列 / 侧栏 3 列」。这里改成 7 / 4，是被字号下限逼
# 出来的：3 列 = 218px，除以 Caption 档 32px 只剩 6 个汉字一行 —— 那不
# 是窄栏，是没法读。4 列 = 296px 给到 9 字，是 1080 画幅上「侧栏还能放
# 得下一句话」的物理下限。宁可动栏宽，也不动 32px 那条线。
MAIN_COL = COLUMN * 7 + GUTTER * 6          # 530
SIDE_COL = COLUMN * 4 + GUTTER * 3          # 296
COL_GAP = W - EDGE * 2 - MAIN_COL - SIDE_COL


def margin_note():
    """F3：主栏 8 列 + 侧栏 3 列，中间 1 列沟槽。

    这一板是全套的方法论：**主栏是作者的，侧栏是你的**。所以两栏的字号必
    须拉开（Body L vs Caption），否则读者分不清哪句是引文哪句是你的判断 ——
    那正是书评最容易糊掉的一处。
    """
    main_text = col("主栏", [
        caption("栏标", "原文", "$c-azurite-d", weight=600),
        body("引文", "他花了整整一章去解释为什么没有选另外一条路。"
             "读到这里才发现，前面所有铺垫都是为这一次不选做准备。",
             "$c-ink", width=MAIN_COL),
        rect(ids, "栏内分隔", width=MAIN_COL, height=1,
             fill=solid("$c-ochre")),
        body("引文续", "作者全程没有评价它。", "$c-ink", width=MAIN_COL),
    ], gap=20, width=MAIN_COL)

    side = col("侧栏", [
        caption("栏标", "我的注", "$c-malachite-d", weight=600),
        caption("注解 1", "不评价，等于把判断整个留给读者。", "$c-soft",
                width=SIDE_COL),
        caption("注解 2", "这也是全书唯一的一次视角外移。", "$c-soft",
                width=SIDE_COL),
    ], gap=18, width=SIDE_COL)

    body_row = row("双栏", [
        main_text,
        rect(ids, "栏线", width=1, height=280, fill=solid("$c-ochre")),
        side,
    ], gap=round((COL_GAP - 1) / 2), align="start")

    main = zone([
        text(ids, "小标题", "先看它怎么写", FS_T1, 700, "$c-ink",
             family=CJK, line_height=LH_TITLE),
        body_row,
    ], justify="center", gap=40)
    return board(2, "原文 · 侧栏注解", main)


# ------------------------------------------------------- 03 洞见 · C1 编号纵列
INSIGHTS = [
    ("一", "结构是论点", "章节顺序本身在说话，别只读句子。"),
    ("二", "留白是态度", "作者不写的部分，比写了的更能定位他。"),
    ("三", "结尾回指开头", "读完再翻回第一页，那句话变了意思。"),
]


def insights():
    """C1：编号 2 列 + 内容 10 列。引导符一板只用一种 —— 这里是汉字数字。

    编号用「一二三」而不是「01 02 03」：本套的字族是绢本气，阿拉伯数字在
    这个语境里是另一套语汇（它属于数据档）。
    """
    items = []
    for no, title, note in INSIGHTS:
        badge = frame(ids, f"序号 {no}", width=COLUMN + GUTTER,
                      height=COLUMN + GUTTER, layout="horizontal",
                      alignItems="center", justifyContent="center",
                      fill=solid("$c-deep"))
        badge["children"] = [
            text(ids, "序号字", no, FS_T2, 700, "$c-azurite-d", family=CJK,
                 width="fit_content", growth="auto", line_height=1.0),
        ]
        items.append(row("洞见", [
            badge,
            col("洞见文案", [
                text(ids, "洞见标题", title, FS_T2, 700, "$c-ink",
                     family=CJK, line_height=LH_TITLE),
                body("洞见注解", note, "$c-soft"),
            ], gap=10, width=INNER - (COLUMN + GUTTER) - 28),
        ], gap=28, align="start"))

    main = zone([
        text(ids, "小标题", "三个能拿走的零件", FS_T1, 700, "$c-ink",
             family=CJK, line_height=LH_TITLE),
        col("洞见列表", items, gap=36),
    ], justify="center", gap=40)
    return board(3, "洞见 · 编号纵列", main)


# ------------------------------------------------------- 04 书摘 · B2 引号框景
def quote_frame():
    """B2：金句在框内 8 列，署名在框外右下。框由石青石绿成对描出。

    B 族硬规则：一板只放一句、≤24 字、署名必须是 Caption 档不许和金句争。
    """
    box = frame(ids, "引号框", width="fill_container", height="fit_content",
                layout="vertical", gap=24, padding=[48, 44],
                alignItems="start", fill=solid("$c-deep"),
                stroke={"thickness": 2, "fill": solid("$c-malachite")})
    box["children"] = [
        text(ids, "引号", "「", FS_DISPLAY, 700, "$c-azurite", family=CJK,
             width="fit_content", growth="auto", line_height=1.0),
        # 金句用框的内宽而不是 MAIN_COL：530 除以 Title 1 档 64px 只剩 8 字
        # 一行，一句 9 字的金句会被折成 8+1，最后那个句号自己站一行。
        text(ids, "金句", "不选，也是选完了。", FS_T1, 700, "$c-ink",
             family=CJK, width="fill_container", growth="fixed-width",
             line_height=LH_TITLE),
    ]

    main = zone([
        box,
        row("署名行", [caption("署名", "—— 原书第七章", "$c-soft")],
            gap=0, justify="end"),
    ], justify="center", gap=22)
    return board(4, "书摘 · 引号框景", main)


# ------------------------------------------------------- 05 收束 · G1 签名收束
def closing():
    """G1：收束句脱离前文也读得通；行动一句话，不做 CTA 块。"""
    main = zone([
        text(ids, "收束句", "值得读的书\n会改写你的旧记忆。",
             FS_DISPLAY, 700, "$c-ink", family=CJK, width=INNER,
             growth="fixed-width", line_height=LH_DISPLAY),
        col("行动", [
            rect(ids, "短线", width=COLUMN * 2 + GUTTER, height=3,
                 fill=solid("$c-malachite-d")),
            body("行动说明", "读完回头翻第一页，看看它是不是变了意思。",
                 "$c-soft"),
        ], gap=20),
    ], justify="center", gap=48)
    return board(5, "收束 · 签名", main)


def build():
    return [cover(), margin_note(), insights(), quote_frame(), closing()]


# 对比度（WCAG 相对亮度比，op-design-lint 门槛 2.0；本表逐对实算）：
#   c-ink        on c-bg    11.06    c-soft        on c-bg    5.35
#   c-azurite-d  on c-bg     7.43    c-malachite-d on c-bg    5.11
#   c-ink        on c-deep    9.47   c-soft        on c-deep  4.58
#   c-azurite-d  on c-deep    6.36   c-azurite     on c-deep  3.17
#   c-ochre      on c-bg      2.19   c-malachite   on c-bg    2.74
# 承载正文的最低一对是 c-soft on c-deep 4.58（第 2 板侧栏注解与第 4 板框内
# 的次级文字），已过 WCAG AA 正文门槛。c-azurite on c-deep 3.17 只用在第 4
# 板那个 88px 的引号上 —— 正好过 AA 大字门槛 3.0，且整套仅此一处；这也是
# 「青绿不做文字色」那条负约束的量化依据：石青只在够大、够少的地方成立。
# c-ochre 2.19 只作 1px 分隔线与栏线，属非文字图形，本模板明令它永不承载
# 文字。山形那一处渐层之上没有任何文字，故不入表。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "书影评拆解轮播 · 3:4 五板")
