#!/usr/bin/env python3
"""pitch-deck-dark.op — 深色路演 deck（封面 / 问题 / 方案 / 数据 / 里程碑 / CTA）

已有的 slide-deck 是暖白商务风，适合汇报；这一套是它的反面：黑底、单强调
色、大字号、少而重的信息。路演是「在别人只肯给你八分钟的房间里，把一件
事讲到对方想追问」——所以每页只有一个视觉锚点，其余全部让位。

这一版把「深色」从「一块死黑」做成有层次的深色：

1. **底是对角微光渐变**，不是纯色。近黑的石墨蓝 → 略偏蓝的深靛 → 回到近
   黑，135° 铺满整页。投影时这一点点方向感就是「专业深色」与「PPT 默认
   黑底」的差别。
2. **章节页有 ghost 大数字**。480px 的页码压在内容之下（`oplib.stack` 的
   装饰层），用强调色 + 6% 图层透明度 —— 不是把颜色调灰，是把整层调淡，
   所以换强调色时它自己跟着走。
3. **强调色做成霓虹细条**。卡首那道线从强调色渐到全透明，像一条点亮的
   边，而不是一根填色矩形。
4. **字阶拉开**：封面 128 / 正文 30（4.3 倍），页标题 76 / 正文 28。展示字
   统一收紧字距到 -2 ~ -3。

排版遵循 skills/domains/slides.md 的硬契约：
  - 每帧固定 1920×1080，绝不 fit_content；内容距边缘 ≥100（这里 120）
  - 正文 ≥24（取 26-30），标题 ≥40，关键数字 80-200（数据页取 148）
  - 行高：展示字 1.06-1.15，正文 1.4+
  - 最多 2 个字体族（Noto Sans SC 排中文，Inter 排数字），靠字重分层
  - 强调色只用于强调；正文一律中性色

深底上的对比度（WCAG，实测见文件末尾）最低一对是 4.73:1，全部高于 4.5。
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import (Ids, color_vars, frame, linear, radial, rect, solid, stack,
                   stroke, text, write_doc)

ids = Ids()

# 近黑的石墨蓝 + 一个电光蓝。深色 deck 最容易翻车的地方是「深色 = 一堆
# 灰」：所以底色只有两级（bg / surface），层级靠字重和强调色拉，不靠再多
# 铺几层灰面。
VARS = color_vars({
    "c-bg":          "#0B1220",
    # 页面渐变的中段与末段。跨度刻意小（亮度差不到 4%）：再大一点，页与页
    # 之间就会看出「这页比那页亮」，一整套投下来像没校准的投影仪。
    "c-bg-mid":      "#111C33",
    "c-bg-end":      "#0A1424",
    "c-surface":     "#18263F",
    "c-ink":         "#F2F6FF",
    "c-muted":       "#93A4C4",
    "c-accent":      "#4D8DFF",
    "c-accent-soft": "#16233D",
    "c-border":      "#24314D",
    # 光晕与霓虹细条的两端。带 alpha —— 它们是文字的**兄弟**不是祖先，
    # lint 找底色只走祖先链，碰不到这两个值；而渐到全透明是唯一能让光晕在
    # 渐变底上不露硬边的写法。
    "c-glow":        "#4D8DFF29",
    "c-glow-out":    "#4D8DFF00",
})

W, H = 1920, 1080
EDGE = 120

# 中文字族写死：oplib 的默认族按字号/字重挑（<34 且 <600 走 Inter），那条
# 规则是给中英混排写的；这套 deck 是纯中文正文，小字号也必须落在有汉字的
# 字族上，否则要靠 fallback 兜。
CJK = "Noto Sans SC"
# 数字、单位符号走 Inter：等宽感更强，KPI 排出来才齐。
NUM = "Inter"

# 135° = 左上 → 右下（`.op` 的角度约定：0° 自下而上、90° 自左向右，见
# op-host-native/src/backend/skia/gradient.rs）。
PAGE_ANGLE = 135


def page_fill():
    return linear(PAGE_ANGLE, [(0.0, "$c-bg"), (0.55, "$c-bg-mid"),
                               (1.0, "$c-bg-end")])


def slide(name, *, gap=56, justify="start", decor=()):
    """一帧幻灯片。固定 1920×1080 —— 投影比例是硬约束，不是建议。"""
    body = frame(ids, f"{name} · 正文", width="fill_container",
                 height="fill_container", layout="vertical",
                 padding=[EDGE, EDGE], gap=gap, justifyContent=justify,
                 alignItems="start", fill=[])
    body["children"] = []
    shell = stack(ids, name, body, list(decor), width=W, height=H,
                  fill=page_fill())
    shell["content"] = body
    return shell


def glow(size, x, y):
    """一团光晕。渐到全透明，所以压在渐变底上也不会露出圆形硬边。"""
    node = rect(ids, "光晕", width=size, height=size, cornerRadius=size // 2,
                fill=radial([(0.0, "$c-glow"), (1.0, "$c-glow-out")]))
    node["x"], node["y"] = x, y
    return node


def ghost_number(label, x, y, *, size=480):
    """章节页背景上的巨大页码。

    做法是「满色 + 图层透明度」而不是「把颜色调到接近底色」：前者换强调色
    时自己跟着走，后者要手算一遍；而且 lint 量的是文字的原色（它不看节点
    opacity），满色写法不会在检测器里留下一条假的低对比记录。
    """
    node = text(ids, "ghost 页码", label, size, 700, "$c-accent", family=NUM,
                width="fit_content", growth="auto", line_height=1.0,
                spacing=-round(size * 0.05))
    node["x"], node["y"] = x, y
    node["opacity"] = 0.07
    return node


def col(name, children, *, gap=16, width="fill_container",
        height="fit_content", align="start", fill=None, **props):
    """纵向容器。`fill=None` 才是结构层；一旦有 fill 它就是一张卡面了。"""
    node = frame(ids, name, width=width, height=height, layout="vertical",
                 gap=gap, alignItems=align, fill=fill or [], **props)
    node["children"] = children
    return node


def row(name, children, *, gap=40, align="start", width="fill_container",
        height="fit_content", fill=None, **props):
    node = frame(ids, name, width=width, height=height, layout="horizontal",
                 gap=gap, alignItems=align, fill=fill or [], **props)
    node["children"] = children
    return node


def neon_rule(width=72, height=4):
    """霓虹细条。强调色渐到透明，读起来像一段被点亮的边而不是一块填色。"""
    return rect(ids, "霓虹线", width=width, height=height, cornerRadius=999,
                fill=linear(90, [(0.0, "$c-accent"), (1.0, "$c-glow-out")]))


def eyebrow(label):
    """小标签。整套 deck 唯一的胶囊，用来交代「这是什么场合」。"""
    node = frame(ids, "标签", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[10, 22], gap=0,
                 cornerRadius=999, alignItems="center",
                 justifyContent="center", fill=solid("$c-accent-soft"))
    node["children"] = [
        text(ids, "标签文字", label, 26, 600, "$c-accent", family=CJK,
             width="fit_content", growth="auto", line_height=1.4,
             spacing=1.0),
    ]
    return node


def accent_bar(width=160, height=8):
    return rect(ids, "强调条", width=width, height=height, cornerRadius=999,
                fill=solid("$c-accent"))


def divider():
    return rect(ids, "分隔线", width="fill_container", height=2,
                fill=solid("$c-border"))


def centered_body(name, child):
    """把内容块撑到页标题以下的整块空间里再垂直居中。

    页标题每页固定在同一高度（重复）是 deck 读起来整齐的前提；正文块又不
    该被拉长到失真。两个诉求靠这一层透明容器同时满足。
    """
    return col(name, [child], gap=0, height="fill_container",
               justifyContent="center")


def page_head(title, subtitle=None, *, size=76):
    kids = [text(ids, "页标题", title, size, 700, "$c-ink", family=CJK,
                 line_height=1.12, spacing=-2)]
    if subtitle:
        kids.append(text(ids, "页副标题", subtitle, 30, 400, "$c-muted",
                         family=CJK, line_height=1.45))
    return col("页头", kids, gap=18)


# ------------------------------------------------------------------ 01 封面
def cover():
    lede = col("主张", [
        text(ids, "主标题", "把三天的报价\n压进三分钟", 128, 700, "$c-ink",
             family=CJK, line_height=1.08, spacing=-3),
        accent_bar(),
        text(ids, "副标题", "我们在做的，是让每一家工厂当场就能报出价来。",
             30, 400, "$c-muted", family=CJK, line_height=1.45),
    ], gap=36)

    meta = row("落款", [
        text(ids, "演讲者", "启明智造 · 创始人 陈知远", 28, 500, "$c-ink",
             family=CJK, width="fit_content", growth="auto"),
        text(ids, "场合", "2026.08 · 种子轮路演", 28, 400, "$c-muted",
             family=CJK, width="fit_content", growth="auto"),
    ], gap=32, align="center", justifyContent="space_between")

    s = slide("01 封面", gap=48, justify="space_between",
              decor=[glow(1500, 980, -420), glow(760, 1360, 620)])
    s["content"]["children"] = [eyebrow("种子轮 · 融资路演"), lede, meta]
    return s


# ------------------------------------------------------------------ 02 问题
PAINS = [
    ("算不清", "一份报价要问遍四个部门，等两天才知道能不能接。"),
    ("报不准", "凭经验拍的价，做完才发现毛利被工时和损耗吃光了。"),
    ("跟不上", "客户当场要数，我们只能说「回去核一下」——单子就这么走了。"),
]


def problem():
    rows = []
    for index, (title, desc) in enumerate(PAINS, 1):
        if rows:
            rows.append(divider())
        rows.append(row(f"痛点 {index}", [
            text(ids, "痛点序号", f"0{index}", 72, 700, "$c-accent",
                 family=NUM, width="fit_content", growth="auto",
                 line_height=1.0, spacing=-2),
            col("痛点文案", [
                text(ids, "痛点标题", title, 44, 600, "$c-ink", family=CJK,
                     line_height=1.2),
                text(ids, "痛点说明", desc, 28, 400, "$c-muted", family=CJK,
                     line_height=1.5),
            ], gap=12),
        ], gap=48, align="start"))

    s = slide("02 问题", gap=56, decor=[ghost_number("02", 1240, 470),
                                      glow(900, -300, 520)])
    s["content"]["children"] = [
        page_head("报价这一步，卡住了整条链路",
                  "三个问题，每一个都在直接吃掉订单。"),
        col("痛点列表", rows, gap=0, height="fill_container",
            justifyContent="space_between"),
    ]
    return s


# ------------------------------------------------------------------ 03 方案
SELLING_POINTS = [
    ("三分钟出价", "上传图纸，直接算出工时与材料。"),
    ("毛利先看见", "价格旁就是毛利率，接不接一眼定。"),
    ("越报越准", "每完成一单回流成本，模型自己修。"),
]


def solution():
    cards = []
    for title, desc in SELLING_POINTS:
        cards.append(col("卖点卡", [
            neon_rule(),
            text(ids, "卖点标题", title, 40, 600, "$c-ink", family=CJK,
                 line_height=1.2),
            text(ids, "卖点说明", desc, 27, 400, "$c-muted", family=CJK,
                 line_height=1.55),
        ], gap=24, height="fill_container", padding=[48, 40],
            cornerRadius=20, fill=solid("$c-surface")))

    s = slide("03 方案", gap=56, decor=[ghost_number("03", 1330, 20),
                                      glow(1000, -260, -280)])
    s["content"]["children"] = [
        page_head("把报价变成一次点击",
                  "工艺库 + 实时成本模型，客户还在电话里，价就出来了。"),
        centered_body("卖点区", row("卖点网格", cards, gap=36,
                                 align="stretch")),
    ]
    return s


# ------------------------------------------------------------------ 04 数据
METRICS = [
    ("42", "倍", "报价提速", "两天的活儿，三分钟出结果"),
    ("97", "%", "报价准确率", "与实际成本偏差不超过 3%"),
    ("2.3", "亿元", "在跑的订单金额", "来自 46 家制造企业"),
]

# 大数字 lineHeight 1.0 时基线离行盒底部约 0.2em；单位字号小得多，`end`
# 对齐会把它压到基线以下。这个补偿把单位的行盒底抬回大数字的基线附近，
# 让「42」和「倍」看起来站在同一条线上。jian 没有真正的 baseline 对齐
# （AlignItems::from_css 把 "baseline" 折成 End），所以只能这样补。
VALUE_SIZE = 148
UNIT_SIZE = 46
UNIT_BASELINE_LIFT = round((VALUE_SIZE - UNIT_SIZE) * 0.2)


def unit_family(unit):
    """汉字单位（亿元）排中文族，符号单位（× %）排 Inter 跟数字同源。"""
    return CJK if any(ord(ch) > 0x2FFF for ch in unit) else NUM


def metric_card(value, unit, label, note):
    unit_box = frame(ids, "单位", width="fit_content", height="fit_content",
                     layout="vertical", gap=0,
                     padding=[0, 0, UNIT_BASELINE_LIFT, 0], fill=[])
    unit_box["children"] = [
        text(ids, "单位文字", unit, UNIT_SIZE, 600, "$c-accent",
             family=unit_family(unit), width="fit_content", growth="auto",
             line_height=1.0),
    ]
    return col("数据卡", [
        neon_rule(),
        row("数值", [
            text(ids, "数值文字", value, VALUE_SIZE, 700, "$c-accent",
                 family=NUM, width="fit_content", growth="auto",
                 line_height=1.0, spacing=-4),
            unit_box,
        ], gap=10, align="end", width="fit_content"),
        text(ids, "指标名", label, 32, 600, "$c-ink", family=CJK,
             line_height=1.25),
        text(ids, "指标说明", note, 26, 400, "$c-muted", family=CJK,
             line_height=1.45),
    ], gap=20, height="fill_container", justifyContent="center",
        padding=[52, 44], cornerRadius=24, fill=solid("$c-surface"))


def metrics():
    s = slide("04 数据", gap=56, decor=[ghost_number("04", 1330, 20),
                                      glow(1100, 1200, -340)])
    s["content"]["children"] = [
        page_head("半年里，它已经在跑",
                  "不是路演里的假设，是 46 家工厂每天在用的数。"),
        centered_body("数据区", row("数据网格",
                                 [metric_card(*entry) for entry in METRICS],
                                 gap=36, align="stretch")),
    ]
    return s


# --------------------------------------------------------------- 05 里程碑
MILESTONES = [
    ("2026 Q3", "工艺库上线", "覆盖钣金、机加两条主线", True),
    ("2026 Q4", "接入 ERP", "报价直接落成订单", True),
    ("2027 Q1", "开放 API", "让客户在自己系统里报价", False),
    ("2027 Q2", "行业模板", "从制造扩到工程与装修", False),
]

# 时间轴列之间不留 gap：轴线是每列自己那一段拼起来的，列间一旦有间隙，
# 轴就断成四截。列内文案靠右 padding 让出呼吸，轴线段照样贯穿。
TIMELINE_TEXT_INSET = 56
DOT = 28


def milestone_column(date, title, desc, reached):
    """一段时间轴。圆点与文案都左对齐，轴线从圆点向右延伸到下一个圆点。"""
    if reached:
        dot = rect(ids, "节点圆", width=DOT, height=DOT, cornerRadius=DOT // 2,
                   fill=solid("$c-accent"))
    else:
        dot = rect(ids, "节点圆", width=DOT, height=DOT, cornerRadius=DOT // 2,
                   fill=solid("$c-bg"), stroke=stroke("$c-accent", 4))

    axis = row("轴段", [
        dot,
        rect(ids, "轴线", width="fill_container", height=4,
             fill=solid("$c-border")),
    ], gap=0, align="center", height="fit_content")

    # 上下两块文案带右内缩，轴段一律不写 padding —— 三个同类兄弟里只有两个
    # 可归一化的 padding 值，mixed-sibling-padding 因此不会误判（同一个值
    # 出现两次，另一个不参与统计）。
    head = col("节点日期", [
        text(ids, "日期", date, 28, 600, "$c-accent", family=NUM,
             line_height=1.2, spacing=0.5),
    ], gap=0, padding=[0, TIMELINE_TEXT_INSET, 0, 0])
    body = col("节点文案", [
        text(ids, "节点标题", title, 36, 600, "$c-ink", family=CJK,
             line_height=1.25),
        text(ids, "节点说明", desc, 26, 400, "$c-muted", family=CJK,
             line_height=1.5),
    ], gap=10, padding=[0, TIMELINE_TEXT_INSET, 0, 0])

    return col("里程碑", [head, axis, body], gap=28)


def roadmap():
    s = slide("05 里程碑", gap=56, decor=[ghost_number("05", 1330, 20),
                                       glow(880, -280, 560)])
    s["content"]["children"] = [
        page_head("接下来十二个月",
                  "前两步已经走完，这轮钱花在后两步上。"),
        centered_body("时间轴区",
                      row("时间轴",
                          [milestone_column(*entry) for entry in MILESTONES],
                          gap=0, align="start")),
    ]
    return s


# ---------------------------------------------------------------- 06 结尾
def closing():
    lede = col("结语", [
        accent_bar(),
        text(ids, "结语标题", "如果你也觉得\n这事早该被解决", 100, 700,
             "$c-ink", family=CJK, line_height=1.1, spacing=-3),
        text(ids, "结语副标题", "我们缺的不是订单，是把它做快的那笔钱。",
             30, 400, "$c-muted", family=CJK, line_height=1.45),
    ], gap=36)

    # 名字与两行联络方式分成两层，不是排版洁癖：三个平级文本一旦字号 32 /
    # 30 / 30，sibling-inconsistency 会把那个 32 当成手滑。分层之后 32 是
    # 标题、30 是并列的两行，各自内部一致。
    contact = col("联系方式", [
        text(ids, "联系人", "陈知远 · 创始人", 32, 600, "$c-ink", family=CJK,
             line_height=1.3),
        col("联络方式", [
            text(ids, "邮箱", "zhiyuan@qimingzz.com", 30, 500, "$c-accent",
                 family=NUM, line_height=1.4),
            text(ids, "电话", "+86 138 0000 0000", 30, 400, "$c-muted",
                 family=NUM, line_height=1.4),
        ], gap=10, width="fit_content"),
    ], gap=18, width="fit_content")

    # 二维码占位用 frame 而不是 rectangle：frame 会递归渲染子节点，里面那行
    # 提示才显示得出来；frame 同时是合法的图片拖放目标，把二维码图片拖上去
    # 就直接替换掉这个占位。
    qr = frame(ids, "二维码占位", width=200, height=200, layout="vertical",
               gap=0, alignItems="center", justifyContent="center",
               cornerRadius=16, fill=solid("$c-accent-soft"),
               stroke=stroke("$c-accent", 2))
    qr["children"] = [
        text(ids, "二维码提示", "扫码看 Demo", 26, 600, "$c-accent",
             family=CJK, width="fit_content", growth="auto",
             line_height=1.3),
    ]

    card = row("联系卡", [contact, qr], gap=48, align="center",
               justifyContent="space_between", padding=[48, 56],
               cornerRadius=24, fill=solid("$c-surface"))

    s = slide("06 结尾", gap=56, justify="space_between",
              decor=[glow(1600, -520, -460), glow(820, 1300, 540)])
    s["content"]["children"] = [lede, card]
    return s


# 顶层 frame 必须显式写 x/y：缺省时每帧都落在原点，六页会叠成一页，另外
# 五页藏在下面（2026-08-02 实录）。横向 gap 与纵向 gap 的取值同 tpl_slides：
# 纵向多留 240，是给画布上固定在帧顶部的帧名标签让位——标签是屏幕空间的，
# 不随缩放变小，deck 缩到能整屏看时 120 文档像素只剩十几个屏幕像素。
BOARD_GAP_X = 120
BOARD_GAP_Y = BOARD_GAP_X + 240
BOARDS_PER_ROW = 3


def build():
    boards = [cover(), problem(), solution(), metrics(), roadmap(), closing()]
    for index, board in enumerate(boards):
        board.pop("content", None)
        board["x"] = (index % BOARDS_PER_ROW) * (W + BOARD_GAP_X)
        board["y"] = (index // BOARDS_PER_ROW) * (H + BOARD_GAP_Y)
    return boards


# 对比度（WCAG 相对亮度比，投影场景自设门槛 4.5，远高于 op-design-lint 的
# 2.5）。页面底是渐变，所以按**三个 stop 分别量、取最差**——lint 只读
# stops[0]，靠它兜不住：
#   c-ink   on 页面渐变    17.30 / 15.87 / 16.72
#   c-muted on 页面渐变     7.44 /  6.82 /  7.19
#   c-accent on 页面渐变    5.86 /  5.38 /  5.67
#   c-ink   on c-surface   13.98    c-muted on c-surface      6.01
#   c-accent on c-surface   4.73  ← 最低一对    c-accent on c-accent-soft 4.90
# 数据页的 148px 大数字用的是 c-accent on c-surface = 4.73:1。换强调色时
# 先拿这一对量一遍：深底上亮色越纯，比值掉得越快。
# 光晕与 ghost 页码不参与：前者是纯装饰兄弟，后者是「满色 + 7% 图层透明
# 度」，两者都不承担阅读。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "路演 deck · 深色 16:9")
