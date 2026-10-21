#!/usr/bin/env python3
"""slide-deck.op — 16:9 演示文稿模板（封面 + 目录 + 要点 + 数据 + 图表 + 结尾）

暖白商务风。这一版把「能用」推到「好看」，改动集中在三件事上：

1. **底不再是一块死白。** 每帧退成 layout:none 外壳（`oplib.stack`），内容
   层照常做 flex，装饰层单独一层放一个巨大的淡蓝圆弧。整套只有这一种装饰
   语言，六页各换一个位置和大小，克制但贯穿。
2. **字阶拉开到 4 倍以上。** 封面 132 / 正文 30，页标题 80 / 正文 28。原来
   104 与 34 只差 3 倍，投出来「大标题」并不大。展示字统一收紧字距
   （-2 ~ -3）与行高（1.05-1.1）。
3. **数据页改成大数字组合。** 灰卡换成透明卡 + 顶端强调线，数值 168px 用
   墨色（对比度 15:1），变化量做成强调色胶囊 —— 强调色只出现在「变化」和
   「装饰线」两个固定位置，全套一致。

排版仍遵循 skills/domains/slides.md 的硬契约：
  - 每帧 1920×1080，内容距边缘 ≥100px（这里 120）
  - 正文 ≥24（取 26-30），标题 ≥40，关键数字 80-200（数据页取 168）
  - 行高：展示字 1.05-1.1，正文放宽到 1.45+
  - 最多 2 个字体族，靠字重而非字号数量做层级
  - 2-3 个主色 + 中性色，强调色只用于强调

白底上的对比度（WCAG，实测见文件末尾）最低一对是 4.60:1，全部高于 4.5。
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import (Ids, color_vars, frame, linear, radial, rect, solid, stack,
                   text, write_doc)

ids = Ids()

VARS = color_vars({
    "c-bg":          "#FFFFFF",
    # 封面渐变的远端，同时也是装饰圆弧的芯色。比 c-accent-soft 更淡：它要铺
    # 满半页，稍微重一点整页就发蓝。
    "c-bg-tint":     "#EDF1FD",
    "c-surface":     "#F4F6F9",
    "c-ink":         "#0B1220",
    "c-muted":       "#5A6B85",
    "c-accent":      "#2F5BEA",
    # 柱状图与胶囊的渐变深端。同色相压暗，不引入第二个色相。
    "c-accent-deep": "#1B3FBF",
    "c-accent-soft": "#E4EAFD",
    "c-border":      "#DCE2EC",
    # 装饰圆弧的芯色与外沿。**唯一用 alpha 的两个变量**：圆弧要压在渐变底
    # 上，实色的外沿必然与底色对不上，会露出一圈可见的硬边（第一版实测就是
    # 这样）。渐到全透明才是真正的光晕。这里可以放心用 alpha，因为装饰是文
    # 字的**兄弟**不是祖先，lint 找底色只走祖先链，碰不到它。
    "c-arc":         "#2F5BEA26",
    "c-arc-out":     "#2F5BEA00",
    # 结尾页（墨底）上的装饰圆弧芯色。深色页上的装饰不能用亮蓝，会变成一盏
    # 灯；提亮墨色本身才是「同一种装饰语言换个底」——墨底是实色，没有渐变
    # 对不上的问题，所以这一枚仍用实色。
    "c-ink-lift":    "#16203A",
})

# 幻灯片尺寸与安全区。EDGE 同时是每帧的 padding —— 契约要求 ≥100。
W, H = 1920, 1080
EDGE = 120


def slide(name, *, fill="$c-bg", gap=56, justify="start", decor=()):
    """一帧幻灯片。固定 1920×1080，绝不 fit_content：投影比例是硬约束。"""
    body = frame(ids, f"{name} · 正文", width="fill_container",
                 height="fill_container", layout="vertical",
                 padding=[EDGE, EDGE], gap=gap, justifyContent=justify,
                 fill=[])
    body["children"] = []
    shell = stack(ids, name, body, list(decor), width=W, height=H,
                  fill=fill if isinstance(fill, list) else solid(fill))
    # 调用方往 `.children` 上追加内容，指的一直是正文层。
    shell["content"] = body
    return shell


def arc(size, x, y, *, core="$c-arc", edge="$c-arc-out", alpha=1.0):
    """一枚巨大的淡色圆弧。整套 deck 唯一的装饰语言。

    做成径向渐变而不是实色圆：实色圆会在底上露出一道硬边，读起来像误放的
    形状；渐到透明才是一片光晕。半径取 0.5 让渐变正好在圆边收干净
    （TileMode::Clamp 会把边界外全刷成最后一个 stop）。
    """
    node = rect(ids, "装饰圆弧", width=size, height=size, cornerRadius=size // 2,
                fill=radial([(0.0, core), (1.0, edge)]))
    node["x"], node["y"] = x, y
    if alpha < 1.0:
        node["opacity"] = alpha
    return node


def hairline(width="fill_container", color="$c-border", thickness=2):
    return rect(ids, "分隔线", width=width, height=thickness, fill=solid(color))


def row(name, *, gap=40, align="start", width="fill_container",
        height="fit_content", fill=None, **props):
    # `fill=None` means a transparent structural wrapper; an explicit fill is
    # what makes a row a painted surface (card, panel).
    node = frame(ids, name, width=width, height=height, layout="horizontal",
                 gap=gap, alignItems=align, fill=fill or [], **props)
    node["children"] = []
    return node


def col(name, *, gap=16, width="fill_container", height="fit_content",
        fill=None, **props):
    node = frame(ids, name, width=width, height=height, layout="vertical",
                 gap=gap, fill=fill or [], **props)
    node["children"] = []
    return node


def eyebrow(label):
    """小标签。ALL CAPS 只在这种小标签上使用（契约允许的唯一例外）。"""
    node = frame(ids, "标签", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[10, 20], cornerRadius=999,
                 fill=solid("$c-accent-soft"))
    node["children"] = [
        text(ids, "标签文字", label, 24, 600, "$c-accent",
             width="fit_content", growth="auto", spacing=1.2),
    ]
    return node


def accent_bar(width=140, height=8):
    return rect(ids, "强调条", width=width, height=height, cornerRadius=999,
                fill=solid("$c-accent"))


def page_head(title, subtitle=None):
    """页标题。80px + 字距 -2：投影上这一档才读得出「这是页标题」。"""
    head = col("页头", gap=18)
    head["children"] = [
        text(ids, "页标题", title, 80, 700, "$c-ink", line_height=1.1,
             spacing=-2),
    ]
    if subtitle:
        head["children"].append(
            text(ids, "页副标题", subtitle, 30, 400, "$c-muted",
                 line_height=1.45))
    return head


# ------------------------------------------------------------------ 01 封面
def cover():
    # 135° = 左上 → 右下（`.op` 的角度约定：0° 自下而上、90° 自左向右，见
    # op-host-native/src/backend/skia/gradient.rs）。标题在左上的纯白一侧，
    # 淡蓝沉到右下，正好接住那一枚装饰圆弧。
    s = slide("01 封面", fill=linear(135, [(0.0, "$c-bg"), (1.0, "$c-bg-tint")]),
              justify="space_between", gap=48,
              decor=[arc(1240, 1160, 460), arc(560, 1500, 60, alpha=0.7)])
    body = col("封面文案", gap=36)
    body["children"] = [
        text(ids, "主标题", "把复杂的事\n讲成一句话", 132, 700, "$c-ink",
             line_height=1.06, spacing=-3),
        accent_bar(),
        text(ids, "副标题", "副标题写清楚这次要让听众记住的那个结论。", 30, 400,
             "$c-muted", line_height=1.45),
    ]
    meta = row("落款", gap=32, align="center", justifyContent="space_between")
    meta["children"] = [
        text(ids, "演讲者", "演讲者姓名 · 团队", 28, 500, "$c-ink",
             width="fit_content", growth="auto"),
        text(ids, "日期", "2026.08 · 季度汇报", 28, 400, "$c-muted",
             width="fit_content", growth="auto"),
    ]
    s["content"]["children"] = [eyebrow("2026 · 季度汇报"), body, meta]
    return s


# ------------------------------------------------------------------ 02 目录
AGENDA = [
    ("01", "背景与问题", "我们面对的现状是什么"),
    ("02", "解决方案", "做了哪三件关键的事"),
    ("03", "结果与数据", "带来了多少可衡量的变化"),
    ("04", "下一步", "接下来三个月的计划"),
]


def agenda():
    """四行清单，不是四个格子。

    原来的四列卡片把每条挤到 380px 宽，说明文字折成三行，读着像表单。改成
    通栏四行之后每条有 1680px 可用，序号能放到 72px，行与行之间只靠一条细
    线分开 —— 留白本身成了结构。
    """
    s = slide("02 目录", gap=48,
              decor=[arc(1000, 1320, -260, alpha=0.85)])
    rows = []
    for index, (no, title, desc) in enumerate(AGENDA):
        if rows:
            rows.append(hairline())
        item = row(f"目录项 {no}", gap=48, align="center", padding=[24, 0])
        item["children"] = [
            text(ids, "序号", no, 72, 700, "$c-accent", width=120,
                 line_height=1.0, spacing=-2),
            col("条目文案", gap=8),
        ]
        item["children"][1]["children"] = [
            text(ids, "条目标题", title, 40, 600, "$c-ink", line_height=1.2),
            text(ids, "条目说明", desc, 28, 400, "$c-muted", line_height=1.45),
        ]
        rows.append(item)
    listing = col("目录列表", gap=0, height="fill_container",
                  justifyContent="center")
    listing["children"] = rows
    s["content"]["children"] = [page_head("目录"), listing]
    return s


# ------------------------------------------------------------------ 03 要点
POINTS = [
    ("更快", "把等待时间从 8 秒压到 1.2 秒，用户不再中途离开。"),
    ("更稳", "失败率降到千分之一以内。"),
    ("更省", "同样的任务量，只用一半算力。"),
]


def points():
    s = slide("03 要点", gap=48, decor=[arc(880, -300, 640, alpha=0.8)])
    grid = row("要点网格", gap=36, align="stretch")
    for index, (title, desc) in enumerate(POINTS, 1):
        item = col("要点", gap=24, padding=[44, 40], cornerRadius=24,
                   height="fill_container", fill=solid("$c-surface"))
        item["children"] = [
            # 顶端一道强调线代替原来的整圈描边：卡片之间的分界靠留白就够，
            # 描边只会让三张卡看起来像三个输入框。
            rect(ids, "卡首强调线", width=64, height=6, cornerRadius=999,
                 fill=solid("$c-accent")),
            text(ids, "要点序号", f"0{index}", 28, 600, "$c-accent",
                 width="fit_content", growth="auto", line_height=1.2,
                 spacing=1.2),
            text(ids, "要点标题", title, 44, 600, "$c-ink", line_height=1.2),
            text(ids, "要点说明", desc, 28, 400, "$c-muted", line_height=1.55),
        ]
        grid["children"].append(item)
    s["content"]["children"] = [
        page_head("我们做了三件事", "每一件都能单独讲清楚，也都能被验证。"),
        col("要点区", gap=0, height="fill_container",
            justifyContent="center"),
    ]
    s["content"]["children"][1]["children"] = [grid]
    return s


# ------------------------------------------------------------------ 04 数据
METRICS = [
    ("1.2", "s", "首屏加载", "较上季度 −85%"),
    ("99.9", "%", "关键路径成功率", "连续 90 天达标"),
    ("2.1", "x", "人均处理量", "同等人力下"),
]

# 大数字 lineHeight 1.0 时基线离行盒底部约 0.2em；单位字号小得多，`end`
# 对齐会把它压到基线以下。这个补偿把单位的行盒底抬回大数字的基线附近。
# jian 没有真正的 baseline 对齐（AlignItems::from_css 把 "baseline" 折成
# End），所以只能这样补。
VALUE_SIZE = 168
UNIT_SIZE = 52
UNIT_LIFT = round((VALUE_SIZE - UNIT_SIZE) * 0.2)


def metric_card(value, unit, label, delta):
    """大数字 + 小标签的组合。卡不是一块灰底，是一条强调线加一堆留白。

    数值用墨色而不是强调色：168px 的色块面积太大，强调色铺到这个尺寸就不再
    是「强调」了。强调色收回到变化量胶囊和顶端那条线上，全套三处用法一致。
    """
    unit_box = col("单位", gap=0, width="fit_content",
                   padding=[0, 0, UNIT_LIFT, 0])
    unit_box["children"] = [
        text(ids, "单位文字", unit, UNIT_SIZE, 600, "$c-muted",
             width="fit_content", growth="auto", line_height=1.0),
    ]
    value_row = row("数值", gap=6, align="end", width="fit_content")
    value_row["children"] = [
        text(ids, "数值文字", value, VALUE_SIZE, 700, "$c-ink",
             width="fit_content", growth="auto", line_height=1.0, spacing=-5),
        unit_box,
    ]

    chip = frame(ids, "变化胶囊", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[8, 18], cornerRadius=999,
                 fill=solid("$c-accent-soft"))
    chip["children"] = [
        text(ids, "变化文字", delta, 24, 600, "$c-accent", width="fit_content",
             growth="auto", line_height=1.3),
    ]

    card = col("数据卡", gap=24, height="fill_container",
               justifyContent="center")
    card["children"] = [
        rect(ids, "卡首强调线", width="fill_container", height=6,
             cornerRadius=999, fill=solid("$c-accent")),
        value_row,
        text(ids, "指标名", label, 32, 600, "$c-ink", line_height=1.25),
        chip,
    ]
    return card


def metrics():
    s = slide("04 数据", gap=48, decor=[arc(1100, 1400, 520, alpha=0.85)])
    grid = row("数据网格", gap=56, align="stretch", height="fill_container")
    grid["children"] = [metric_card(*entry) for entry in METRICS]
    s["content"]["children"] = [
        page_head("结果是可衡量的", "三个指标，都能在后台直接调出来。"),
        grid,
    ]
    return s


# ------------------------------------------------------------------ 05 图表
BARS = [180, 250, 220, 330, 460]

CHART_NOTES = [
    ("持续增长", "连续五个周期上升，没有出现回落。"),
    ("拐点在第四期", "投入的三项改动都在这一期生效。"),
    ("可以外推", "按当前斜率，下季度可达成目标线。"),
]


def chart():
    s = slide("05 图表", gap=48, decor=[arc(760, -220, -180, alpha=0.75)])
    body = row("图表区", gap=64, align="stretch", height="fill_container")

    # 图表占位：一组等宽柱子。替换成真实数据时改高度即可，不必重建结构。
    # 最高的一根走强调色渐变（深→浅，自下而上），其余留素色 —— 图表里的
    # 「强调」和页面上的强调色是同一套。
    plot = frame(ids, "图表占位", width="fill_container",
                 height="fill_container", layout="horizontal", gap=32,
                 alignItems="end", padding=[48, 48], cornerRadius=28,
                 fill=solid("$c-surface"))
    plot["children"] = [
        rect(ids, f"柱 {i + 1}", width="fill_container", height=height,
             cornerRadius=16,
             # 0° = 自下而上：深端坐在柱底，柱顶收到主强调色。
             fill=(linear(0, [(0.0, "$c-accent-deep"), (1.0, "$c-accent")])
                   if i == len(BARS) - 1 else solid("$c-accent-soft")))
        for i, height in enumerate(BARS)
    ]

    notes = col("图表说明", gap=0, width=560, justifyContent="center")
    for index, (title, desc) in enumerate(CHART_NOTES):
        if index:
            notes["children"].append(hairline())
        item = col("说明项", gap=10, padding=[26, 0])
        item["children"] = [
            text(ids, "说明标题", title, 32, 600, "$c-ink", line_height=1.25),
            text(ids, "说明正文", desc, 27, 400, "$c-muted", line_height=1.5),
        ]
        notes["children"].append(item)

    body["children"] = [plot, notes]
    s["content"]["children"] = [
        page_head("趋势说明结论", "把图表要说明的那句话写在这里，别让听众自己找。"),
        body,
    ]
    return s


# ------------------------------------------------------------------ 06 结尾
def closing():
    s = slide("06 结尾", fill="$c-ink", justify="space_between", gap=48,
              decor=[arc(1400, 1120, 300, core="$c-ink-lift", edge="$c-ink"),
                     arc(620, -240, -200, core="$c-ink-lift", edge="$c-ink")])
    body = col("结尾文案", gap=36, height="fill_container",
               justifyContent="center")
    body["children"] = [
        accent_bar(),
        text(ids, "结语", "谢谢，欢迎提问。", 120, 700, "#FFFFFF",
             line_height=1.06, spacing=-3),
        text(ids, "联系方式", "name@example.com · 团队主页", 30, 400,
             "#9FB0CC", line_height=1.45),
    ]
    s["content"]["children"] = [body]
    return s


# Horizontal gap between boards. Without explicit positions every frame
# defaults to the origin and the deck stacks into one visible slide — the
# other five are underneath it (reported 2026-08-02).
BOARD_GAP_X = 120

# Vertical gap. Larger than the horizontal one because the canvas paints each
# frame's name ABOVE the frame at a fixed screen-space offset that does not
# scale with zoom (canvas_frame_labels.rs: the label box top sits 21 px above
# the frame). At the zoom where a 3x2 deck is framed to the screen (~0.14-0.22
# — the deck is 6000 doc px across, fitted into a 944-1424 px canvas region), a
# 120 doc px row gap is only ~16-26 screen px, i.e. barely wider than the
# label band itself, so every second-row label sat on the bottom edge of the
# row above (reported 2026-08-02). The extra 240 doc px reads as 33-52 screen
# px there, clearing the label with room to spare. Mirrors
# `scaffold.rs::ROW_LABEL_HEADROOM`, which fixes the generated multi-screen
# canvas the same way.
BOARD_GAP_Y = BOARD_GAP_X + 240

# Boards per row. Six 1920px slides in one strip are ~12,000px across, which
# can only be read by panning; wrapping them into a 3x2 grid matches the
# preview thumbnail and follows the same convention the generator uses for
# multi-screen roots (`scaffold.rs::MAX_ROW_WIDTH`, which wraps 1920px boards
# by width rather than by design type).
BOARDS_PER_ROW = 3


def build():
    boards = [cover(), agenda(), points(), metrics(), chart(), closing()]
    for index, board in enumerate(boards):
        board.pop("content", None)
        board["x"] = (index % BOARDS_PER_ROW) * (W + BOARD_GAP_X)
        board["y"] = (index // BOARDS_PER_ROW) * (H + BOARD_GAP_Y)
    return boards


# 对比度（WCAG 相对亮度比，投影场景自设门槛 4.5；渐变底按每个 stop 分别量，
# 取最差值 —— lint 只读 stops[0]，靠不住）：
#   c-ink    on c-bg / c-bg-tint   19.80 / 17.99
#   c-muted  on c-bg / c-bg-tint    5.44 /  4.94
#   c-ink    on c-surface          18.06     c-muted on c-surface  4.97
#   c-accent on c-accent-soft       4.60  ← 最低一对（胶囊与标签里的强调字）
#   #FFFFFF  on c-ink              19.80     #9FB0CC on c-ink       9.19
# 装饰圆弧不参与：它是 c-bg→c-bg-tint 之间的一层光晕，最深处就是 c-bg-tint，
# 上面那两行已经把最差情况量过了。换强调色时先量 c-accent on c-accent-soft。

if __name__ == "__main__":
    dst = sys.argv[1]
    write_doc(dst, VARS, build(), "演示文稿 · 16:9 模板")
