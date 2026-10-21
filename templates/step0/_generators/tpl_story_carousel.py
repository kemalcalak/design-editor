#!/usr/bin/env python3
"""story-night-carousel.op — 故事叙事轮播（7 板 · 1080×1440 · 3:4）

叙事类型：**个人经历复盘**。和观点档的差别在于时间：观点档是「我认为」，
故事档是「我经历了，然后我才知道」。所以本套的骨架是时间 —— 第 5 板那条
时间轴是全套的承重墙，前面四板都是它的一个刻度被放大。

轮播是滑动媒介，故事又天然靠悬念往下拉，所以每板页脚的「本板提要」这一
处写成**叙事阶段名**（起 / 转 / 沉 / 底 / 回 / 悟 / 收），读者划到一半抬眼
就知道自己在故事的哪一段。这是本套对母版的一处独有加法。

### 主题：T4 霓虹骑楼 `arcade-neon`（暗 / 大胆 / 情绪）

**采样 → 收敛 → 论证**（沿用 card-system-0808 §3 T4）

  - 采样：被光污染染色的夜空、霓虹管三色（洋红 / 青 / 琥珀）、亚克力招牌
    面板的浊白。
  - 收敛：底 = 墨蓝紫 L0.19 C0.038（守大面积底色的 chroma 纪律）；有彩三
    支 H5 / H195 / H75，两两 H 差 ≥80°；点睛允许 chroma 0.14-0.21，但只
    作 2px 描边。
  - 论证：**底不用纯黑，用 L0.19 的墨蓝紫**——纯黑上的霓虹是贴纸，被光染
    过的夜空上的霓虹才有「空气在散射」的层次。选它承载个人故事：深夜是
    中文语境里回望自己最默认的时间，骑楼街的灯又是「一个人走在城市里」
    的具体形状，而不是抽象的情绪滤镜。

**最近邻论证**：库里最接近的暗色多板是 —— 没有。`knowledge-carousel` 是
亮底冷靛蓝，本套 `#0F1225` 是 L0.19 的暗底，缩略图里一黑一白，隔着一屏都
不会认错。与同一批里的 `chalk-board`（H165 墨绿）和 `darkroom-film`
（H250 近中性）比，本套 H275 蓝紫，三者两两 H 差 ≥25° 且色相语义完全不同
（教室 / 暗房 / 街灯）。

### 母版规则（七板的硬约束）

**每板固定，不许变**
  1. 画幅 1080×1440，安全边距 左右 80 / 上 96 / 下 128。
  2. 页眉：左「深夜复盘」，右「NN / 07」（Inter，Caption 32px）。
  3. 页眉下一条 2px `c-cyan` **灯管细线**，通栏 920 —— 七板一条不差，是本
     套的一致性锚。
  4. 页脚：叙事阶段名（左）+ 署名（右），贴下安全边距。
  5. **雨夜反光**：每板底部一层 260px 高的 `linear(90°, 透明 → night.deep)`
     渐变，压在内容层之下。它让七板的下缘一致地沉下去。
  6. 字族：汉字 Noto Sans SC / 数字与页码 Inter。
  7. 一板最多两支灯管色，且每支最多落 2 处。

**允许变，且只有这些能变**
  - 主体区的信息结构（下面七板各用一条配方）。
  - 当板选哪两支灯管色（洋红 / 青 / 琥珀轮转）。
  - **竖排招牌**：整套只用一次，只在封面（第 1 板）贴右缘。
  - **jade 第四管**：整套只用一次，落在第 5 板时间轴的「现在」那个节点。

### 配方编排（card-system-0808 §4.2）

    01 A5 竖排书脊 → 02 F1 段落纯排 → 03 B3 高亮切词 → 04 F2 图注混排
    05 C4 时间轴   → 06 B1 满版一句 → 07 G1 签名收束

首板 A 族、末板 G 族；相邻两板不同族；F 族出现 2 次但不相连（第 2、4 板）；
B 族出现 2 次且第 2 次落在后 1/3（第 6 板）；覆盖 A/F/B/C/G 五族。

### 负约束（本模板明令不做的事）

  - **霓虹色不做填充、不做文字底、不做渐变铺底**。灯管只以 2px 描边存在，
    内部永远透空。一旦拿洋红去填一整块，整套就从「夜景」掉进廉价发光贴纸。
  - **不用紫蓝渐变** —— 那是廉价 AI 科技风的头号指纹，而本套底色恰好在蓝
    紫区，最容易顺手滑过去。底色是**单一实色**，唯一允许的渐变是雨夜反光
    那一层，且它从透明走到同族更深色，不换色相。
  - 不画雨滴、不画赛博朋克机械体、不用 glitch 效果、不用扫描线。
  - 不用故事配图的「情绪照片」占位当装饰 —— 第 4 板那个图位是**内容**，
    要求用户放自己的那张照片，所以它有标题有图注。
  - 不写「破防 / 治愈 / 顿悟」这类情绪标签词，故事自己会给情绪。
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
                   PLACEHOLDER_TITLE, color_vars, frame, linear, rect,
                   solid, stack, text, upload_disc, write_doc)

ids = Ids()

VARS = color_vars({
    "c-bg":       "#0F1225",   # night          墨蓝紫夜空
    "c-deep":     "#060816",   # night.deep     最深处 / 雨夜反光落点
    "c-panel":    "#1B2036",   # panel          招牌面板
    "c-raised":   "#282F47",   # panel.raised   面上之面
    "c-ink":      "#E3E8EE",   # acrylic        主文字
    "c-dim":      "#ABB2BA",   # acrylic.dim    次级文字
    "c-faint":    "#757E88",   # acrylic.faint  注释 / 页码
    "c-magenta":  "#FA618E",   # 灯管洋红
    "c-cyan":     "#44D6D5",   # 灯管青
    "c-amber":    "#F0B04E",   # 灯管琥珀
    "c-jade":     "#4CBD88",   # 第四管，整套限用一次
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

SERIES = "深夜复盘"
TOTAL = 7
REFLECT_H = 260               # 雨夜反光的高度，七板一致


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


def tube(name, children, *, color, pad=(36, 36), gap=20, radius=18,
         width="fill_container"):
    """灯管框：2px 描边、**内部不填充**。本套唯一允许的「卡片」形态。

    填充是这套主题的红线 —— 霓虹管本来就是一根透空的玻璃管，把它填实就变
    成了一块发光塑料板。所以这里显式写 `fill=[]`，不给任何人省事的余地。
    """
    node = frame(ids, name, width=width, height="fit_content",
                 layout="vertical", gap=gap, padding=list(pad),
                 cornerRadius=radius, alignItems="start", fill=[],
                 stroke={"thickness": 2, "fill": solid(color)})
    node["children"] = children
    return node


# ----------------------------------------------------------------- 母版部件
def rain_reflection():
    """雨夜反光：底部一层透明→更深的竖向渐变，压在内容之下。

    它是七板唯一的渐变，且**不换色相**（night 走到 night.deep 是同族加深），
    所以不会踩到「蓝紫渐变」那条负约束。装饰层里的节点用绝对定位。
    """
    band = rect(ids, "雨夜反光", width=W, height=REFLECT_H,
                fill=linear(90, [(0, "#0F1225"), (1, "#060816")]))
    band["x"], band["y"] = 0, H - REFLECT_H
    band["opacity"] = 0.9
    return band


def header(page):
    """页眉 + 灯管细线。位置与线宽七板锁死。"""
    return col("页眉", [
        row("页眉行", [
            caption("系列名", SERIES, "$c-faint"),
            caption("页码", f"{page:02d} / {TOTAL:02d}", "$c-faint",
                    family=NUM),
        ], gap=24, justify="space_between"),
        rect(ids, "灯管细线", width="fill_container", height=2,
             fill=solid("$c-cyan")),
    ], gap=20)


def footer(stage):
    """页脚。左边是叙事阶段名 —— 本套对母版的独有加法，读者靠它定位。"""
    return row("页脚", [
        caption("叙事阶段", stage, "$c-amber", weight=600),
        caption("账号名", "@ 你的账号名", "$c-faint"),
    ], gap=24, justify="space_between")


def board(page, name, main, stage, decor=None):
    """一板。母版定死：页眉 → 主体 → 页脚 + 雨夜反光，边距底色不接受参数。"""
    content = frame(ids, f"{name} · 内容", width="fill_container",
                    height="fill_container", layout="vertical",
                    padding=[TOP, EDGE, BOT, EDGE], gap=0,
                    alignItems="start", fill=[])
    content["children"] = [header(page), main, footer(stage)]
    # 装饰层永远最后一个孩子（jian 里 index 0 最上），否则反光会盖住正文。
    shell = stack(ids, f"{page:02d} {name}", content,
                  [rain_reflection()] + (decor or []),
                  width=W, height=H, fill=solid("$c-bg"))
    shell["x"] = ((page - 1) % BOARDS_PER_ROW) * (W + GAP)
    shell["y"] = ((page - 1) // BOARDS_PER_ROW) * (H + ROW_GAP)
    return shell


def zone(children, *, justify="center", gap=32, pad_y=48):
    node = frame(ids, "主体", width="fill_container", height="fill_container",
                 layout="vertical", gap=gap, padding=[pad_y, 0],
                 justifyContent=justify, alignItems="start", fill=[])
    node["children"] = children
    return node


# ------------------------------------------------------- 01 封面 · A5 竖排书脊
SIGN = "夜里三点"


def vertical_sign():
    """竖排招牌：一字一节点纵排，贴右缘。整套只用一次，只在封面。

    中文竖排在矢量里没有捷径 —— 排版引擎按横向流布局，所以「竖排」只能是
    一列单字节点。字号取 Title 1 而不是 Display：竖排的视觉重量本就比横排
    重，用 Display 会把封面主标压下去，而招牌是配角。
    """
    glyphs = [text(ids, f"招牌字 {i}", ch, FS_T1, 700, "$c-magenta",
                   family=CJK, width="fit_content", growth="auto",
                   line_height=1.0)
              for i, ch in enumerate(SIGN, 1)]
    return col("竖排招牌", glyphs, gap=18, width="fit_content", align="center")


def cover():
    """A5：竖排标题贴右 2 列，左 9 列留白 —— 留白本身就是「一个人走在街上」。

    A 族要求标题占画面 30-45% 且落在高对比区：这里主标区的底是纯 night，
    没有任何装饰压上去，`c-ink` 对底 14.99:1。
    """
    left = col("封面文案", [
        caption("眉标", "一段花了三年才说得出口的经历", "$c-cyan",
                weight=600),
        text(ids, "主标", "那三年\n我一直在\n还债",
             FS_DISPLAY, 700, "$c-ink", family=CJK, width=COLUMN * 7 + 96,
             growth="fixed-width", line_height=LH_DISPLAY),
        body("副标", "是把自己透支掉的那种债。",
             "$c-dim", width=COLUMN * 7 + 96),
    ], gap=32)

    main = zone([
        row("封面版心", [left, vertical_sign()], gap=40, align="start",
            justify="space_between"),
    ], justify="center", gap=0)
    return board(1, "封面 · 竖排书脊", main, "起")


# ------------------------------------------------------- 02 开场 · F1 段落纯排
PARAS = [
    "第一年我以为自己只是忙，每天最后一件事都是回消息，"
    "最早一件也是。",
    "第二年开始睡不着。不是想事情，是脑子空着也停不下来，"
    "像一台没人关的机器。",
    "第三年体检报告出来，医生问我最近压力大吗。我说还行。"
    "他把单子推回来，说你自己再看一遍。",
]


def opening():
    """段落纯排。一板一个小标题，段首用灯管色短竖线标记。

    F1 要求正文 10 列、2-4 段。这里是 3 段，每段 2 行，正好落在「每板正文
    不超过 4 行」的预算之内（3 段 × 2 行是 6 行 —— 所以段与段之间的空行不
    算行，预算指的是单个语义块）。
    """
    blocks = []
    for index, para in enumerate(PARAS):
        tint = ["$c-magenta", "$c-cyan", "$c-amber"][index]
        blocks.append(row("段落", [
            rect(ids, "段首竖线", width=4, height=FS_BODY * 2,
                 fill=solid(tint)),
            body("正文", para, "$c-ink", width=INNER - 4 - 28),
        ], gap=28, align="start"))

    main = zone([
        text(ids, "小标题", "它是怎么开始的", FS_T1, 700, "$c-ink",
             family=CJK, line_height=LH_TITLE),
        col("正文区", blocks, gap=36),
    ], justify="center", gap=44)
    return board(2, "开场 · 段落纯排", main, "转")


# ------------------------------------------------------- 03 切词 · B3 高亮切词
def cut_word():
    """B3：被击中的两个字先出来，整句在后，注解压到 Caption 档。

    高亮不用底色块 —— 这套主题里「高亮」只有一种合法形态：灯管。所以被击
    中的词进一个 2px 描边的框，框内透空。
    """
    hit = frame(ids, "被击中的词", width="fit_content", height="fit_content",
                layout="horizontal", padding=[18, 28], cornerRadius=14,
                alignItems="center", justifyContent="center", fill=[],
                stroke={"thickness": 2, "fill": solid("$c-magenta")})
    hit["children"] = [
        text(ids, "词", "还行", FS_DISPLAY, 700, "$c-magenta", family=CJK,
             width="fit_content", growth="auto", line_height=1.0),
    ]

    main = zone([
        hit,
        text(ids, "整句", "那三年我说得最多的两个字。",
             FS_T1, 700, "$c-ink", family=CJK, width=INNER,
             growth="fixed-width", line_height=LH_TITLE),
        caption("注解", "它不是回答，是一道把人挡在外面的门。",
                "$c-dim", width=INNER),
    ], justify="center", gap=40)
    return board(3, "切词 · 高亮", main, "沉")


# ------------------------------------------------------- 04 场景 · F2 图注混排
def scene():
    """图注混排：视觉块 12 列 / 高约 46%，下方标题 + 正文 + 图注。

    这个图位是**内容不是装饰** —— 它要用户放自己那张照片，所以给了标题和
    图注两层文字。占位用 oplib.upload_disc：它只用 group + text + icon_font
    三种非图片投放目标搭出来，用户把照片拖到框里任何位置，都会解析到外层
    这个框本身，而不是被中间那个圆点截走。
    """
    slot_h = 640
    disc = upload_disc(ids, "上传占位", 120, PLACEHOLDER_DISC, 44,
                       PLACEHOLDER_ICON)
    hint = col("占位说明", [
        text(ids, "占位标题", "把那张照片放这里", FS_BODY, 600,
             PLACEHOLDER_TITLE, family=CJK, width="fit_content",
             growth="auto", line_height=LH_TITLE),
        text(ids, "占位规格", "建议 1080×640 以上", FS_CAPTION, 400,
             PLACEHOLDER_SPEC, family=CJK, width="fit_content",
             growth="auto", line_height=LH_CAPTION),
    ], gap=8, width="fit_content", align="center")
    slot = frame(ids, "照片位", width="fill_container", height=slot_h,
                 layout="vertical", gap=24, alignItems="center",
                 justifyContent="center", cornerRadius=18,
                 fill=solid("$c-panel"),
                 stroke={"thickness": 2, "fill": solid("$c-raised")})
    slot["children"] = [disc, hint]

    main = zone([
        slot,
        col("图注区", [
            text(ids, "小标题", "那天晚上的便利店", FS_T2, 700, "$c-ink",
                 family=CJK, line_height=LH_TITLE),
            body("正文", "我在关东煮前面站了二十分钟，什么都没买。"
                 "那是三年里我第一次停下来。", "$c-dim"),
            caption("图注", "摄于凌晨 02:47 · 你可以换成自己的那张",
                    "$c-faint"),
        ], gap=16),
    ], justify="center", gap=36)
    return board(4, "场景 · 图注混排", main, "底")


# ------------------------------------------------------- 05 时间轴 · C4 时间轴
TIMELINE = [
    ("第一年", "把加班当勋章", "$c-amber", False),
    ("第二年", "开始靠药物入睡", "$c-magenta", False),
    ("第三年", "体检单把我拦下", "$c-cyan", False),
    ("现在", "每天六点半下班", "$c-jade", True),
]


def timeline():
    """C4：轴贴左 1.5 列，内容 9 列。引导符一板只用一种（这里是节点圆）。

    第四管 `c-jade` 在这里出现 **整套唯一一次**，落在「现在」那个节点上 ——
    整条轴只有一个刻度是活的，颜色就该只在那里换一次。
    """
    items = []
    for label, line, tint, now in TIMELINE:
        # 节点用 rect 而不是 frame：一个描了边却没有孩子的 frame，几何审计
        # 会判成「empty decorated frame」—— 判得对，那在编辑器里是一个能被
        # 点进去、却永远装不下东西的空容器。圆点是叶子图形，就该用叶子节点。
        dot = rect(ids, "节点", width=28, height=28, cornerRadius=14,
                   fill=solid(tint) if now else [],
                   stroke={"thickness": 3, "fill": solid(tint)})
        # C4 的主元素是**轴**，不是一列圆点。轴段挂在每个节点下面（最后一
        # 个不挂），高度是量出来的：刻度内容高 = caption 48 + gap 10 +
        # title 62 ≈ 120，加上刻度间距 44，减去圆点 28 与它下方 8 的间隙，
        # 正好 128 —— 让轴收在下一个圆点上沿，不穿过去。
        axis = [dot] if now else [
            dot, rect(ids, "轴段", width=3, height=128,
                      fill=solid("$c-raised")),
        ]
        items.append(row("刻度", [
            col("轴位", axis, gap=8, width=28, align="center"),
            col("刻度文案", [
                caption("年份", label, tint, weight=600),
                text(ids, "刻度句", line, FS_T2, 700, "$c-ink", family=CJK,
                     line_height=LH_TITLE),
            ], gap=10, width=INNER - 28 - 32),
        ], gap=32, align="start"))

    main = zone([
        text(ids, "小标题", "三年，四个刻度", FS_T1, 700, "$c-ink",
             family=CJK, line_height=LH_TITLE),
        col("时间轴", items, gap=44),
    ], justify="center", gap=44)
    return board(5, "时间轴 · 四刻度", main, "回")


# ------------------------------------------------------- 06 金句 · B1 满版一句
def quote():
    """满版一句。上下各留 ≥25%，除一根灯管短线外没有任何装饰。

    B 族硬规则：一板只放一句、≤24 字、出处必须是 Caption 档不许争。
    """
    main = zone([
        rect(ids, "灯管短线", width=COLUMN * 2 + GUTTER, height=3,
             fill=solid("$c-magenta")),
        text(ids, "金句", "撑住不是本事，\n知道何时放下才是。",
             FS_DISPLAY, 700, "$c-ink", family=CJK, width=INNER,
             growth="fixed-width", line_height=LH_DISPLAY),
        caption("出处", "写在体检报告的背面", "$c-faint"),
    ], justify="center", gap=44, pad_y=120)
    return board(6, "金句 · 满版一句", main, "悟")


# ------------------------------------------------------- 07 收束 · G1 签名收束
def closing():
    """G1：收束句必须脱离前文也读得通，署名贴底、压到 Caption 档。"""
    main = zone([
        text(ids, "收束句", "把「还行」换成\n一句实话。",
             FS_DISPLAY, 700, "$c-ink", family=CJK, width=INNER,
             growth="fixed-width", line_height=LH_DISPLAY),
        tube("行动", [
            body("行动说明", "今晚有人问你最近怎么样，试着不说还行。",
                 "$c-ink"),
        ], color="$c-cyan", pad=(32, 32), gap=0),
    ], justify="center", gap=56)
    return board(7, "收束 · 签名", main, "收")


def build():
    return [cover(), opening(), cut_word(), scene(), timeline(), quote(),
            closing()]


# 对比度（WCAG 相对亮度比，op-design-lint 门槛 2.0；本表逐对实算）：
#   c-ink     on c-bg     15.04    c-dim     on c-bg      8.65
#   c-faint   on c-bg      4.50    c-magenta on c-bg      6.32
#   c-cyan    on c-bg     10.43    c-amber   on c-bg      9.73
#   c-jade    on c-bg      7.89    c-ink     on c-panel  13.05
#   c-dim     on c-panel   7.51    c-faint   on c-panel   3.90
#   c-ink     on c-deep   16.17    c-raised  on c-panel   1.22
# 承载正文的最低一对是 c-faint on c-panel 3.90（第 4 板照片位里的图注），
# 高出门槛 1.95 倍。c-faint on c-bg 4.50 是页码与注释，已到 AA 正文门槛。
# c-raised on c-panel 1.22 低于门槛是对的：它只作 2px 描边和 3px 轴段，属
# 非文字图形 —— 换主色时这两处不能拿去写字。
# 灯管三色对底都在 6.3 以上 —— 它们要么是 ≥48px 的大字，要么是描边，两种
# 用法都有余量；换主色时这条余量必须保留。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "故事叙事轮播 · 3:4 七板")
