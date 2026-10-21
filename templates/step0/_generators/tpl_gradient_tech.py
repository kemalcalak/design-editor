#!/usr/bin/env python3
"""gradient-tech.op — 渐变科技风 deck（6 页 1920×1080）

深底 + 大面积渐变 + 玻璃拟态卡，开发者产品口吻。这套同时是引擎能力的展台：
线性渐变、径向光晕、玻璃卡三样都是 schema 原生支持、渲染栈真画得出来的
（渲染探针见交付说明）。

这一版把渐变从「深紫到深蓝」推成真正的**紫 → 青**对角全幅，饱和度拉到
对比度预算允许的上限（见下面 GRADIENT 的注释），并给每页加了一冷一暖两团
径向光晕（`oplib.stack` 的装饰层）。标题字重与字号同步拉满：封面 112、
页标题 76，都收紧字距到 -2 ~ -3。

## 玻璃卡为什么不用真 alpha

玻璃拟态的自然写法是半透明白 `#FFFFFF1F`。渲染没问题，但 op-design-lint
的对比度检测器 `parse_hex_color` 会丢掉 alpha 通道，把这张卡当成**纯白不
透明**：卡里的白字于是被判成 1.00:1，而真实合成结果是深紫上的白字。后果
不只是误报——它会让这张卡内的所有文字都失去有效检测，真出现低对比配色也
照样不报。

所以这里的玻璃卡用「不透明渐变」：stops 是「白 @ GLASS_ALPHA 合成到页面
渐变对应 stop」的结果，光学上与真 alpha 一致（对照渲染验证过），但不含
alpha 通道。代价是 stops 是烤死的——改了下面的 GRADIENT 就必须重跑本生成
器，不能只改 .op 里的变量。

排版遵循 skills/domains/slides.md 的硬契约：
  - 每帧固定 1920×1080，绝不 fit_content；内容距边 ≥100（这里 120）
  - 正文 ≥24（取 26-30），标题 ≥40，关键数字 80-200（数据页取 132）
  - 行高：标题 1.1-1.2，正文 1.45+
  - 最多 2 个字体族（Noto Sans SC 排中文，Inter 排数字与代码）
  - 强调色只用于强调

渐变底上的对比度按**每个 stop 分别实测**（比单点检测更严），最低一对
4.74:1，全部高于 4.5。数值见文件末尾。
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import (Ids, color_vars, frame, radial, rect, solid, stack, text,
                   write_doc)

ids = Ids()

# 页面渐变的三个 stop：紫 → 靛 → 青。玻璃卡的合成色由它们算出来，所以
# 这里是整套配色的唯一真源。
#
# 这一版把饱和度拉到「在预算内能拉到的最满」：色相真的从紫（276°）走到
# 青（203°），三个 stop 的彩度都接近满格。**上限不是审美而是算出来的**——
# 玻璃卡把底色往白里提 12%，青色那一端提完之后最亮，强调色压在那儿的比值
# 就是整套的下限。试过更亮的一版（#3A0CA3 / #4B23C9 / #0B6BC4）：好看，但
# 强调色掉到 2.23:1，白正文也只剩 4.30:1，整套报废。现在这组在青端仍有
# 4.74:1。改这三个值必须重跑文件末尾那张表。
GRADIENT = [(0.0, "#43056E"), (0.55, "#2A0C7A"), (1.0, "#00415F")]
# 玻璃的白覆盖率。这个数是承重的：0.12 时强调色压在最亮那段玻璃上是
# 4.74:1，提到 0.18 就掉破 4.5 —— 想让玻璃更亮，得先把强调色一起调。
GLASS_ALPHA = 0.12
GLASS_EDGE_ALPHA = 0.30


def over(alpha, background):
    """白色以 alpha 覆盖 background 后的不透明结果（直接的 source-over）。"""
    channels = [int(background[i:i + 2], 16) for i in (1, 3, 5)]
    return "#" + "".join(
        f"{round(alpha * 255 + (1 - alpha) * c):02X}" for c in channels
    )


VARS = color_vars({
    "c-grad-a":    GRADIENT[0][1],
    "c-grad-b":    GRADIENT[1][1],
    "c-grad-c":    GRADIENT[2][1],
    "c-ink":       "#FFFFFF",
    "c-muted":     "#C9D4F5",
    # 比上一版的 #3AC8FF 更亮一档。不是为了更抢眼，是为了在更饱和的底上
    # 买回对比度余量：同一个底，#3AC8FF 只有 4.04:1，这一档是 4.74:1。
    "c-accent":    "#63D8FF",
    # 对比条里「被比下去」的那根。用正文的 c-muted 会让基线条比强调条还
    # 抢眼（它更长），所以单独给一档更收的合成色；条不是文字，不参与对比
    # 度门槛。
    "c-bar-base":  over(0.45, GRADIENT[1][1]),
    # 玻璃卡的三段合成色（由 GRADIENT + GLASS_ALPHA 算出，改了要重跑）。
    "c-glass-a":   over(GLASS_ALPHA, GRADIENT[0][1]),
    "c-glass-b":   over(GLASS_ALPHA, GRADIENT[1][1]),
    "c-glass-c":   over(GLASS_ALPHA, GRADIENT[2][1]),
    # 描边不参与对比度检测，这里可以放心用真 alpha 取那道细白边。
    "c-glass-edge": "#FFFFFF4D",
    # 装饰光晕的两端。光晕是文字的**兄弟**不是祖先，lint 找底色只走祖先链
    # 碰不到它；而渐到全透明是唯一能让光晕压在整页渐变上不露出圆形硬边的
    # 写法（实色外沿必然与底色对不上）。一冷一暖，对应渐变的两端。
    "c-glow-cyan":     "#63D8FF33",
    "c-glow-cyan-out": "#63D8FF00",
    "c-glow-violet":     "#B14DFF38",
    "c-glow-violet-out": "#B14DFF00",
})

W, H = 1920, 1080
EDGE = 120

CJK = "Noto Sans SC"
NUM = "Inter"

# 角度 135 = 左上 → 右下（渲染实测）。
PAGE_ANGLE = 135


def page_fill():
    return [{
        "type": "linear_gradient",
        "angle": PAGE_ANGLE,
        "stops": [{"offset": offset, "color": f"$c-grad-{key}"}
                  for (offset, _), key in zip(GRADIENT, "abc")],
    }]


def glass_fill():
    """玻璃卡体。见文件头：不透明渐变，不是 alpha。"""
    return [{
        "type": "linear_gradient",
        "angle": PAGE_ANGLE,
        "stops": [{"offset": offset, "color": f"$c-glass-{key}"}
                  for (offset, _), key in zip(GRADIENT, "abc")],
    }]


def glass_edge():
    return {"thickness": 2, "fill": solid("$c-glass-edge")}


def glow(size, x, y, *, warm=False):
    """一团光晕。渐到全透明，所以压在整页渐变上也不会露出圆形硬边。

    半径取 0.5：径向渐变正好在圆边收干净，边界外那圈被 TileMode::Clamp 刷
    成最后一个 stop（全透明），什么都不留下。
    """
    core = "$c-glow-violet" if warm else "$c-glow-cyan"
    edge = "$c-glow-violet-out" if warm else "$c-glow-cyan-out"
    node = rect(ids, "光晕", width=size, height=size, cornerRadius=size // 2,
                fill=radial([(0.0, core), (1.0, edge)]))
    node["x"], node["y"] = x, y
    return node


def slide(name, *, gap=56, justify="start", decor=()):
    """一帧。固定 1920×1080，底是整页渐变，装饰层单独一层压在内容之下。"""
    body = frame(ids, f"{name} · 正文", width="fill_container",
                 height="fill_container", layout="vertical",
                 padding=[EDGE, EDGE], gap=gap, justifyContent=justify,
                 alignItems="start", fill=[])
    body["children"] = []
    shell = stack(ids, name, body, list(decor), width=W, height=H,
                  fill=page_fill())
    shell["content"] = body
    return shell


def col(name, children, *, gap=16, width="fill_container",
        height="fit_content", align="start", fill=None, **props):
    node = frame(ids, name, width=width, height=height, layout="vertical",
                 gap=gap, alignItems=align, fill=fill or [], **props)
    node["children"] = children
    return node


def row(name, children, *, gap=32, align="start", width="fill_container",
        height="fit_content", fill=None, **props):
    node = frame(ids, name, width=width, height=height, layout="horizontal",
                 gap=gap, alignItems=align, fill=fill or [], **props)
    node["children"] = children
    return node


def glass_card(name, children, *, gap=18, height="fit_content",
               padding=(40, 32), **props):
    return col(name, children, gap=gap, height=height,
               padding=[padding[0], padding[1]], cornerRadius=20,
               fill=glass_fill(), stroke=glass_edge(), **props)


def centered_body(name, child):
    """内容块在页标题以下的整块空间里垂直居中，页标题每页同高。"""
    return col(name, [child], gap=0, height="fill_container",
               justifyContent="center")


def eyebrow(label):
    node = frame(ids, "标签", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[10, 22], gap=0,
                 cornerRadius=999, alignItems="center",
                 justifyContent="center", fill=glass_fill(),
                 stroke=glass_edge())
    node["children"] = [
        text(ids, "标签文字", label, 26, 600, "$c-accent", family=CJK,
             width="fit_content", growth="auto", line_height=1.4,
             spacing=1.0),
    ]
    return node


def page_head(title, subtitle=None, *, size=76):
    kids = [text(ids, "页标题", title, size, 700, "$c-ink", family=CJK,
                 line_height=1.12, spacing=-2)]
    if subtitle:
        kids.append(text(ids, "页副标题", subtitle, 30, 400, "$c-muted",
                         family=CJK, line_height=1.45))
    return col("页头", kids, gap=18)


def mono(name, content, size=26, color="$c-accent"):
    """代码腔的一行。Inter 不是等宽，但整套只允许两个字族，用字重区分。"""
    return text(ids, name, content, size, 500, color, family=NUM,
                width="fit_content", growth="auto", line_height=1.5)


# ------------------------------------------------------------------ 01 封面
def cover():
    lede = col("主张", [
        text(ids, "主标题", "让每一次调用\n都在 20 毫秒内返回", 112, 700,
             "$c-ink", family=CJK, line_height=1.08, spacing=-3),
        text(ids, "副标题", "边缘推理网关 v3.0，现已开放公测。", 32, 400,
             "$c-muted", family=CJK, line_height=1.45),
    ], gap=32)

    install = glass_card("安装行", [
        mono("安装命令", "$ npm i -g edgegate && edgegate up", 28),
    ], gap=0, padding=(26, 32), width="fit_content")

    s = slide("01 封面", gap=48, justify="space_between",
              decor=[glow(1500, 1020, -400), glow(900, -320, 560, warm=True)])
    s["content"]["children"] = [eyebrow("v3.0 · 公测"), lede, install]
    return s


# ------------------------------------------------------------------ 02 痛点
PAINS = [
    ("冷启动吃掉了首包",
     "函数实例每次唤醒要 800ms，用户看到的却是整整一秒的白屏。"),
    ("账单没法归因",
     "月底一张总额，分不清哪个模型、哪条链路花掉的。"),
    ("换云就得重写",
     "每家的运行时都不一样，迁移一次等于把接入层重做一遍。"),
]


def problems():
    cards = [
        glass_card("痛点卡", [
            text(ids, "痛点序号", f"0{index}", 30, 700, "$c-accent",
                 family=NUM, width="fit_content", growth="auto",
                 line_height=1.1),
            text(ids, "痛点标题", title, 36, 600, "$c-ink", family=CJK,
                 line_height=1.25),
            text(ids, "痛点说明", desc, 27, 400, "$c-muted", family=CJK,
                 line_height=1.55),
        ], gap=18, height="fill_container", padding=(44, 36))
        for index, (title, desc) in enumerate(PAINS, 1)
    ]

    s = slide("02 痛点", decor=[glow(1200, -400, -320, warm=True),
                             glow(900, 1300, 620)])
    s["content"]["children"] = [
        page_head("在边缘跑推理，卡在三件事上",
                  "都不是模型的问题，是它前面那一层的问题。"),
        centered_body("痛点区", row("痛点网格", cards, gap=32,
                                 align="stretch")),
    ]
    return s


# ------------------------------------------------------------------ 03 架构
LAYERS = [
    ("接入层", "Gateway", "一个 Anycast 入口，就近落到最快的可用区。"),
    ("调度层", "Scheduler", "按延迟预算和单价实时选模型，超时自动降级。"),
    ("运行层", "Runtime", "常驻实例池，冷启动摊薄到 3 毫秒以内。"),
]


def architecture():
    cards = []
    for index, (name, en, desc) in enumerate(LAYERS, 1):
        head = row("层头", [
            text(ids, "层名", name, 36, 600, "$c-ink", family=CJK,
                 width="fit_content", growth="auto", line_height=1.25),
            mono("层英文名", en, 26, "$c-accent"),
        ], gap=16, align="center", width="fit_content")
        cards.append(glass_card("架构层", [
            head,
            text(ids, "层说明", desc, 27, 400, "$c-muted", family=CJK,
                 line_height=1.55),
        ], gap=12, padding=(32, 36)))

    s = slide("03 架构", decor=[glow(1300, 1120, -420),
                             glow(820, -260, 640, warm=True)])
    s["content"]["children"] = [
        page_head("三层，一层只做一件事",
                  "自上而下，每一层都能单独替换。"),
        centered_body("架构区", col("架构堆叠", cards, gap=20)),
    ]
    return s


# ------------------------------------------------------------------ 04 性能
METRICS = [
    ("20", "ms", "P50 端到端", "含调度与网络"),
    ("3", "ms", "冷启动", "常驻池摊薄后"),
    ("62", "%", "成本下降", "同等 QPS 对比自建"),
]

# 对比条：条长按真实数值成比例（越短越快），数字写在标签里。
BAR_TRACK = 1600
BAR_OURS_MS = 20
BAR_BASE_MS = 210


def metric_card(value, unit, label, note):
    head = row("数值", [
        text(ids, "数值文字", value, 132, 700, "$c-accent", family=NUM,
             width="fit_content", growth="auto", line_height=1.0),
        # 单位跟着数字基线走：lineHeight 1.0 下 `end` 会把小字压到基线以
        # 下，这层只有下内距的盒子把它抬回来（jian 没有真 baseline 对齐）。
        col("单位", [
            text(ids, "单位文字", unit, 40, 600, "$c-accent", family=NUM,
                 width="fit_content", growth="auto", line_height=1.0),
        ], gap=0, width="fit_content", padding=[0, 0, round((132 - 40) * 0.2), 0]),
    ], gap=8, align="end", width="fit_content")

    return glass_card("性能卡", [
        head,
        text(ids, "指标名", label, 30, 600, "$c-ink", family=CJK,
             line_height=1.25),
        text(ids, "指标说明", note, 26, 400, "$c-muted", family=CJK,
             line_height=1.45),
    ], gap=16, height="fill_container", padding=(40, 32))


def bar(label, milliseconds, color):
    width = round(BAR_TRACK * milliseconds / BAR_BASE_MS)
    return col("对比条组", [
        text(ids, "条目标签", label, 26, 500, "$c-muted", family=CJK,
             line_height=1.4),
        rect(ids, "对比条", width=width, height=16, cornerRadius=999,
             fill=solid(color)),
    ], gap=10)


def performance():
    bars = glass_card("延迟对比", [
        text(ids, "对比标题", "端到端延迟对比", 30, 600, "$c-ink",
             family=CJK, line_height=1.25),
        bar("本产品 · 20ms", BAR_OURS_MS, "$c-accent"),
        bar("自建方案 · 210ms", BAR_BASE_MS, "$c-bar-base"),
    ], gap=20, padding=(32, 40))

    # 三张卡与对比条合成一块「性能区」：直接摊在帧下会让页头、网格、对比
    # 卡成为三个平级兄弟，其中只有对比卡有圆角，mixed-sibling-corner-radius
    # 会照实报出来（也确实读着像手滑）。
    body = col("性能区", [
        row("性能网格", [metric_card(*entry) for entry in METRICS],
            gap=32, align="stretch"),
        bars,
    ], gap=32)

    s = slide("04 性能", gap=44, decor=[glow(1100, -360, -300),
                                     glow(980, 1240, 560, warm=True)])
    s["content"]["children"] = [
        page_head("数字是压测出来的，不是估的"),
        centered_body("性能居中", body),
    ]
    return s


# ------------------------------------------------------------------ 05 客户
LOGO_SLOTS = 8


def customers():
    # 每个格子是 frame 而不是 rectangle：frame 会递归渲染子节点（里面那行
    # 提示才显示得出来），同时它是合法的图片拖放目标，把客户 logo 拖上去
    # 就直接替换掉占位。
    slots = []
    for index in range(LOGO_SLOTS):
        slot = frame(ids, f"Logo 位 {index + 1}", width="fill_container",
                     height="fill_container", layout="vertical", gap=0,
                     alignItems="center", justifyContent="center",
                     cornerRadius=16, fill=glass_fill(), stroke=glass_edge())
        slot["children"] = [
            text(ids, "占位提示", "拖入 Logo", 24, 500, "$c-muted",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
        ]
        slots.append(slot)

    grid = col("Logo 网格", [
        row("Logo 行", slots[0:4], gap=28, align="stretch",
            height="fill_container"),
        row("Logo 行", slots[4:8], gap=28, align="stretch",
            height="fill_container"),
    ], gap=28, height=420)

    s = slide("05 客户", decor=[glow(1400, 900, -500, warm=True),
                             glow(760, -220, 620)])
    s["content"]["children"] = [
        page_head("已经在生产环境上跑",
                  "八家团队，日均 27 亿次调用。"),
        centered_body("Logo 区", grid),
    ]
    return s


# ------------------------------------------------------------------ 06 CTA
def call_to_action():
    lede = col("结语", [
        text(ids, "结语标题", "十分钟接完\n剩下的交给我们", 96, 700,
             "$c-ink", family=CJK, line_height=1.1, spacing=-3),
        text(ids, "结语副标题", "公测期不限调用量，接入文档一页写完。",
             30, 400, "$c-muted", family=CJK, line_height=1.45),
    ], gap=30)

    actions = row("行动区", [
        glass_card("安装卡", [
            mono("安装命令", "$ npm i -g edgegate", 28),
        ], gap=0, padding=(26, 32), width="fit_content"),
        glass_card("文档卡", [
            mono("文档地址", "docs.edgegate.dev", 28, "$c-ink"),
        ], gap=0, padding=(26, 32), width="fit_content"),
    ], gap=24, align="center", width="fit_content")

    s = slide("06 CTA", gap=48, justify="space_between",
              decor=[glow(1600, -560, -480, warm=True),
                     glow(1000, 1180, 520)])
    s["content"]["children"] = [eyebrow("公测开放中"), lede, actions]
    return s


# 顶层 frame 必须显式写 x/y：缺省时每帧都落在原点，六页会叠成一页。纵向
# gap 比横向多留 240，是给画布上固定在帧顶部的帧名标签让位。
BOARD_GAP_X = 120
BOARD_GAP_Y = BOARD_GAP_X + 240
BOARDS_PER_ROW = 3


def build():
    boards = [cover(), problems(), architecture(), performance(),
              customers(), call_to_action()]
    for index, board in enumerate(boards):
        board.pop("content", None)
        board["x"] = (index % BOARDS_PER_ROW) * (W + BOARD_GAP_X)
        board["y"] = (index // BOARDS_PER_ROW) * (H + BOARD_GAP_Y)
    return boards


# 对比度（WCAG，按渐变**三个 stop 分别量**取最差值，比 lint 只读 stops[0]
# 严得多；门槛 4.5）：
#   压在页面渐变上（#43056E / #2A0C7A / #00415F）：
#     c-ink    14.14 / 14.66 / 10.93      c-muted   9.57 / 9.93 / 7.40
#     c-accent  8.61 /  8.94 /  6.66
#   压在玻璃卡上（白 @0.12 合成后的 #5A237F / #44298A / #1F5872）：
#     c-ink    10.60 / 10.88 /  7.79      c-muted   7.18 / 7.37 / 5.27
#     c-accent  6.46 /  6.63 /  4.74  ← 最低一对（青色那一端最亮）
# 三个承重参数，动哪个都要把上面这张表重跑：GRADIENT（底色饱和度上限就是
# 它定的）、GLASS_ALPHA（提到 0.18 就掉破 4.5）、c-accent（从 #3AC8FF 提到
# #63D8FF 才在新底色上买回余量）。
# 光晕不参与：它是纯装饰兄弟，且渐到全透明。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "渐变科技风 · 16:9")
