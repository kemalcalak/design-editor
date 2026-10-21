#!/usr/bin/env python3
"""event-poster-deck.op — 活动策划 deck · 公告海报（16:9 六页）

既有的五套 deck 全是「工作场合」：商务汇报（slide-deck）、融资路演
（pitch-deck-dark）、课堂讲义（lecture-deck-light）、发布会主题演讲
（minimal-keynote）、开发者产品发布（gradient-tech）。平民线缺的是**生活
场合**的那一套 —— 市集、社团活动、公司团建、开业招商，做的人不是产品经理
而是主办方，要交代的是「什么时候、在哪儿、有什么、怎么来、多少钱」。

所以这套 deck 的结构和商务 deck 是两回事：没有目录页、没有数据页、没有趋
势图，六页分别是 封面 / 亮点 / 日程 / 场地交通 / 票种 / 结尾，每一页都对应
主办方真的要回答的一个问题。

视觉语言取「包豪斯公告海报」——**展览与集市公告海报本来就是这套语言的原
生用途**：近白展墙底、决断的红蓝色块、零圆角、顶天立地的粗黑标题、色块本
身就是版式结构。对齐 style-guide `bauhaus-geometric-light`，这也是这张模
板卡上「基于此模板生成」按钮真正会复现的东西。

和 minimal-keynote 的白底不是一回事：那套整本靠留白与细线做层级，没有一块
饱和色；这套靠**大色块**做层级，白只是色块之间的展墙。

### 配色推导（采样 → 收敛 → 论证）

第一版用的是紫→品红渐变。推翻了：小红书视觉体系把「廉价蓝紫渐变」列在禁
止项第一条，风格库的默认审美禁区也点名「激进紫渐变万能公式」。那不是配色，
是模型先验里抽签抽到的那几个网红色。

  - **采样**：内容自带的色彩记忆 —— 露天市集的遮阳棚红白条纹，与老码头的
    铁件蓝。两个都从「城市周末市集 · 老码头广场」这个内容里来，不是发明的。
  - **收敛**：oklch 压到 2 个有彩色 + 1 组中性明度序列。棚布红
    oklch(0.52 0.15 32)，码头铁蓝 oklch(0.42 0.11 250)，色相角相差 218°；
    中性走 L 0.17 / 0.40 / 0.68 / 0.92 / 0.975 一条明度序列。
  - **论证**：chroma 压在 0.11-0.15 —— 那是油墨印在纸上的饱和度上限，屏幕
    RGB 冲到 0.25 以上就是塑料感。这两个色是「印出来的海报」，不是「屏幕上
    的按钮」。

### 负约束（本模板明令不做的事）

  - 不用任何渐变。六页零 `linear_gradient` / `radial_gradient`。
  - 不用圆角。`cornerRadius` 全篇为 0 —— 包豪斯的几何精度里没有圆角软化，
    药丸形状（`cornerRadius: 999`）尤其禁止。
  - 不用阴影。层级由色块与留白给，不由 `effects` 给。
  - 不用光晕装饰。装饰只能是**实心几何形**（正圆、粗条），它们参与构图，
    不是背景氛围。
  - 不用第三个有彩色。红与蓝之外只有中性明度序列。
  - 不写 AI 套话（「赋能 / 无缝 / 一站式 / 打造闭环」），文案只写主办方真会
    说的话。
  - 不用 emoji 当图标。

排版遵循 skills/domains/slides.md 与中文排印规范：
  - 每帧 1920×1080，内容距边缘 ≥100px（这里 120）
  - 正文 ≥24（取 27-32），标题 ≥40，展示字 100+
  - **CJK 行高比西文全线高 0.2**：页标题 1.2，小节标题 1.3，正文 1.7；
    封面与结尾的巨字（132/148px）收到 1.12，仍在 CJK 展示字的 1.1-1.2 区间内
  - **CJK 负字距不超过 -0.02em**（汉字是满格设计，再负就笔画相撞）；
    只有西文数字沿用西文的 display 收紧
  - 汉字走 Noto Sans SC，数字与拉丁走 Inter —— 等价于中文排印规范里
    「西文在前、中文在后」的 fallback 链，只是在 .op 里按节点写死

白底与红底上的对比度（WCAG，实测见文件末尾）最低一对是 4.76:1。
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import (Ids, color_vars, frame, icon_font, rect, solid, stack, text,
                   write_doc)

ids = Ids()

VARS = color_vars({
    # 近白展墙，不是纯白：#FFF 是实验室白，微暖的 L0.975 才是纸。
    "c-bg":        "#F9F6F2",
    "c-panel":     "#E6E4E1",
    "c-ink":       "#110F0D",
    "c-muted":     "#4A4743",
    "c-faint":     "#9A9894",
    "c-red":       "#AE3F2C",
    "c-red-deep":  "#902A19",
    "c-red-pale":  "#FEE0D9",
    "c-blue":      "#0E4F86",
    "c-blue-deep": "#01345E",
    "c-inv":       "#FFFFFF",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W, H = 1920, 1080
EDGE = 120
INNER = W - EDGE * 2

# CJK 行高阶梯（西文 +0.2）。全篇只用这三档。
LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.3, 1.7


def cjk_track(size):
    """CJK 展示字的负字距上限：-0.02em，再负笔画就相撞。

    只对 >60px 的展示字给负值；24-48px 的标题按规范一律 0（方块字距天然
    均匀，不需要西文式 tracking）。
    """
    return -round(size * 0.018, 1) if size > 60 else 0


def slide(name, *, fill="$c-bg", gap=48, justify="start", decor=()):
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


def disc(size, x, y, color):
    """一枚实心正圆。整套 deck 的装饰只有两种形：正圆和粗条。

    实心而不是光晕：光晕是氛围，正圆是构图元素 —— 它和标题一起决定这一页
    的重心在哪，读者能说出「左边是字、右边是那个红圆」。
    """
    node = rect(ids, "圆块", width=size, height=size, cornerRadius=size // 2,
                fill=solid(color))
    node["x"], node["y"] = x, y
    return node


def slab(width, height, x, y, color):
    """一条粗色块。零圆角，边缘就是边缘。"""
    node = rect(ids, "色块", width=width, height=height, fill=solid(color))
    node["x"], node["y"] = x, y
    return node


def col(name, children, *, gap=16, width="fill_container",
        height="fit_content", align="start", **props):
    node = frame(ids, name, width=width, height=height, layout="vertical",
                 gap=gap, alignItems=align, fill=[], **props)
    node["children"] = children
    return node


def row(name, children, *, gap=32, align="center", width="fill_container",
        height="fit_content", **props):
    node = frame(ids, name, width=width, height=height, layout="horizontal",
                 gap=gap, alignItems=align, fill=[], **props)
    node["children"] = children
    return node


def tag(label, *, bg, fg, size=26):
    """方标签。药丸形状是被本模板明令禁止的 —— 零圆角是这套语言的骨头。"""
    node = frame(ids, "标签", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[12, 24], gap=12,
                 alignItems="center", justifyContent="center", fill=solid(bg))
    node["children"] = [
        text(ids, "标签文字", label, size, 600, fg, family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
    ]
    return node


def hairline(color="$c-panel", thickness=2):
    return rect(ids, "分隔线", width="fill_container", height=thickness,
                fill=solid(color))


def page_head(title, subtitle=None):
    """页标题。一条红粗条压在标题上方 —— 六页同一个位置、同一个尺寸。"""
    items = [
        rect(ids, "页首红条", width=96, height=14, fill=solid("$c-red")),
        text(ids, "页标题", title, 80, 700, "$c-ink", family=CJK,
             line_height=LH_DISPLAY, spacing=cjk_track(80)),
    ]
    if subtitle:
        items.append(text(ids, "页副标题", subtitle, 30, 400, "$c-muted",
                          family=CJK, line_height=LH_BODY))
    return col("页头", items, gap=20)


# ------------------------------------------------------------------ 01 封面
def cover():
    """封面：左侧顶天立地的字，右侧一枚红圆，底部一条蓝色信息条。

    标题占画面近半（公告海报的比例），主视觉是那枚红圆，它不解释什么，
    它就是海报上那个「停一下」的点。
    """
    # 装饰只有一枚红圆。第一版还叠了一块出血的蓝方，它和底部那条蓝信息条
    # 在右下角撞成一个说不清的 L 形 —— 构成里两个形必须有关系，说不出关系
    # 就删掉一个。
    s = slide("01 封面", justify="space_between", gap=56,
              decor=[disc(660, 1120, 180, "$c-red")])
    body = col("封面文案", [
        text(ids, "主标题", "把周末\n还给街道", 132, 700, "$c-ink", family=CJK,
             line_height=1.12, spacing=cjk_track(132)),
        text(ids, "副标题", "8 月 22–23 日 · 老码头文化广场 · 免费入场",
             34, 500, "$c-muted", family=CJK, line_height=LH_BODY),
    ], gap=36)

    strip = row("信息条", [
        text(ids, "主办", "主办：某某文化 · 协办：某某社区", 28, 500, "$c-inv",
             family=CJK, width="fit_content", growth="auto",
             line_height=1.4),
        text(ids, "截止", "摊主报名截止 8 月 18 日", 28, 400, "$c-red-pale",
             family=CJK, width="fit_content", growth="auto",
             line_height=1.4),
    ], gap=32, justifyContent="space_between", padding=[24, 32])
    strip["fill"] = solid("$c-blue")

    s["content"]["children"] = [
        tag("2026 城市周末市集", bg="$c-red", fg="$c-inv"),
        body,
        strip,
    ]
    return s


# ------------------------------------------------------------------ 02 亮点
# (图标, 色块, 标题, 说明)。三栏各顶一条色块 —— 色块就是这一页的结构，
# 不需要卡片、描边或阴影来围出三个区域。
HIGHLIGHTS = [
    ("music", "$c-red", "40 组独立摊主",
     "手作、黑胶、旧书和现烤面包摊。"),
    ("users", "$c-blue", "6 场街头演出",
     "从下午三点唱到入夜，两个舞台不重样。"),
    ("heart", "$c-ink", "带孩子也能来",
     "两片草地全天开放，随时能坐下来歇脚。"),
]


def highlights():
    # 无装饰：这一页已经有三条顶端色块在做结构，再加一个形就是噪声。
    s = slide("02 亮点", gap=56)
    grid = row("亮点网格", [], gap=48, align="stretch")
    for index, (icon, color, title, desc) in enumerate(HIGHLIGHTS, 1):
        item = col("亮点栏", [
            rect(ids, "栏首色块", width="fill_container", height=16,
                 fill=solid(color)),
            icon_font(ids, "亮点图标", icon, 52, color),
            text(ids, "亮点序号", f"{index:02d}", 28, 700, "$c-faint",
                 family=NUM, line_height=1.2),
            text(ids, "亮点标题", title, 46, 700, "$c-ink", family=CJK,
                 line_height=LH_HEAD),
            text(ids, "亮点说明", desc, 29, 400, "$c-muted", family=CJK,
                 line_height=LH_BODY),
        ], gap=22, height="fill_container")
        grid["children"].append(item)
    s["content"]["children"] = [
        page_head("为什么值得来一趟", "三件事说完，剩下的留给现场。"),
        col("亮点区", [grid], gap=0, height="fill_container",
            justifyContent="center"),
    ]
    return s


# ------------------------------------------------------------------ 03 日程
# (时间, 内容, 地点)。主办方真正被问得最多的问题就是「几点在哪儿」。
AGENDA = [
    ("周六 14:00", "开市 · 摊位全部就位", "中央广场"),
    ("周六 17:30", "街头演出第一场", "北侧舞台"),
    ("周日 11:00", "手作工作坊（需现场登记）", "东侧长廊"),
    ("周日 19:00", "闭幕演出 · 全场合唱", "中央广场"),
]

COL_TIME, COL_WHAT, COL_WHERE, COL_GAP = 340, 880, 380, 40


def agenda_row(when, what, where, *, header=False):
    ink = "$c-inv" if header else "$c-ink"
    item = row("日程行", [
        text(ids, "时间", when, 30 if header else 32, 600 if header else 500,
             ink, family=CJK, width=COL_TIME, line_height=1.4),
        text(ids, "内容", what, 30 if header else 32, 600 if header else 400,
             ink, family=CJK, width=COL_WHAT, line_height=1.4),
        text(ids, "地点", where, 30 if header else 28, 600 if header else 400,
             ink if header else "$c-muted", family=CJK, width=COL_WHERE,
             line_height=1.4),
    ], gap=COL_GAP, padding=[26, 32] if header else [26, 32])
    if header:
        item["fill"] = solid("$c-red")
    return item


def agenda():
    s = slide("03 日程", gap=48, decor=[slab(240, 240, 1680, 0, "$c-blue")])
    rows = [agenda_row("时间", "内容", "地点", header=True)]
    for index, entry in enumerate(AGENDA):
        rows.append(hairline())
        rows.append(agenda_row(*entry))
    listing = col("日程表", rows, gap=0, height="fill_container",
                  justifyContent="center")
    s["content"]["children"] = [
        page_head("两天的安排", "全程免费，工作坊需要在现场登记。"),
        listing,
    ]
    return s


# ------------------------------------------------------------------ 04 场地
ROUTES = [
    ("navigation", "$c-red", "地铁", "2 号线 老码头站 B 口，出站步行 6 分钟。"),
    ("compass", "$c-blue", "公交", "18 / 42 / 105 路，文化广场站下车即到。"),
    ("map-marker", "$c-ink", "自驾", "广场地下车库，活动期间前两小时免费。"),
]


def venue():
    s = slide("04 场地", gap=48,
              decor=[slab(96, H, W - 96, 0, "$c-blue")])

    # 地图占位。零圆角的实心浅色块 + 居中标注 —— 一眼能看出「这里要换成你
    # 自己的地图截图」，而且它是合法的图片拖放目标（frame），拖一张图上去
    # 就直接替换掉这块占位。
    placeholder = frame(ids, "地图占位", width="fill_container",
                        height="fill_container", layout="vertical", gap=18,
                        alignItems="center", justifyContent="center",
                        fill=solid("$c-panel"))
    placeholder["children"] = [
        icon_font(ids, "定位", "map-marker", 60, "$c-red"),
        text(ids, "占位标题", "把场地地图拖到这里", 32, 600, "$c-ink",
             family=CJK, align="center", line_height=LH_HEAD),
        text(ids, "占位说明", "建议 4:3，截图或手绘都行", 27, 400, "$c-muted",
             family=CJK, align="center", line_height=LH_BODY),
    ]

    routes = col("交通", [], gap=0, width=760, height="fill_container",
                 justifyContent="center")
    for index, (icon, color, title, desc) in enumerate(ROUTES):
        if index:
            routes["children"].append(hairline())
        square = frame(ids, "交通图标底", width=76, height=76,
                       layout="horizontal", alignItems="center",
                       justifyContent="center", fill=solid(color))
        square["children"] = [icon_font(ids, "交通图标", icon, 36, "$c-inv")]
        entry = row("交通项", [
            square,
            col("交通文案", [
                text(ids, "交通方式", title, 34, 700, "$c-ink", family=CJK,
                     line_height=LH_HEAD),
                text(ids, "交通说明", desc, 27, 400, "$c-muted", family=CJK,
                     line_height=LH_BODY),
            ], gap=8),
        ], gap=28, align="center", padding=[28, 0])
        routes["children"].append(entry)

    body = row("场地区", [placeholder, routes], gap=64, align="stretch",
               height="fill_container")
    s["content"]["children"] = [
        page_head("怎么来", "地铁最省事，自驾请提前查看车库剩余车位。"),
        body,
    ]
    return s


# ------------------------------------------------------------------ 05 票种
# (名称, 价格, 权益, 是否高亮)。只有一档满版红 —— 三档都强调等于没有推荐。
TICKETS = [
    ("散客入场", "免费", ["现场扫码登记", "两天均可进出", "不含工作坊"], False),
    ("工作坊套票", "¥ 68", ["含两场手作工作坊", "材料全包", "限 60 人"], True),
    ("摊主席位", "¥ 240", ["3×3 米标准摊位", "含桌椅与电源", "需资质审核"], False),
]


def ticket_card(name, price, perks, highlight):
    ink = "$c-inv" if highlight else "$c-ink"
    muted = "$c-red-pale" if highlight else "$c-muted"
    items = [
        text(ids, "票种名", name, 34, 600, ink, family=CJK,
             line_height=LH_HEAD),
        text(ids, "价格", price, 68, 700, ink,
             family=NUM if price.startswith("¥") else CJK,
             line_height=1.15,
             spacing=-2 if price.startswith("¥") else 0),
        rect(ids, "价格下线", width="fill_container", height=3,
             fill=solid("$c-red-pale" if highlight else "$c-faint")),
    ]
    for perk in perks:
        items.append(row("权益项", [
            icon_font(ids, "对勾", "check", 26, ink),
            text(ids, "权益文字", perk, 27, 400, muted, family=CJK,
                 line_height=LH_BODY),
        ], gap=14, align="start"))

    card = col("票种栏", items, gap=20, height="fill_container",
               padding=[44, 40])
    # 非高亮栏用浅灰底而不是描边：一圈 3px 黑框会把票种读成输入框，
    # 而「两块灰夹一块红」本身就说清了哪一档是推荐档。
    card["fill"] = solid("$c-red" if highlight else "$c-panel")
    return card


def tickets():
    # 三栏彼此等高（对齐最高的一栏），整组的高度仍由内容决定，再由外面那层
    # fill_container 的空容器把它垂直居中。把网格也设成 fill_container 会让
    # 三栏一直拉到页脚，权益列表底下留出一大片空白。
    s = slide("05 票种", gap=48)
    grid = row("票种网格", [ticket_card(*entry) for entry in TICKETS], gap=40,
               align="stretch")
    s["content"]["children"] = [
        page_head("怎么参加", "散客不用买票，工作坊和摊位需要提前登记。"),
        col("票种区", [grid], gap=0, height="fill_container",
            justifyContent="center"),
    ]
    return s


# ------------------------------------------------------------------ 06 结尾
def closing():
    """满版红 + 反白巨字。字本身就是主视觉（type-as-hero），不加任何装饰。"""
    s = slide("06 结尾", fill="$c-red", justify="space_between", gap=48,
              decor=[disc(520, 1480, -160, "$c-red-deep")])
    body = col("结尾文案", [
        text(ids, "结语", "周六见。", 148, 700, "$c-inv", family=CJK,
             line_height=1.12, spacing=cjk_track(148)),
        text(ids, "结尾说明", "带上朋友，带上袋子，别带太多现金。",
             34, 400, "$c-red-pale", family=CJK, line_height=LH_BODY),
    ], gap=32, height="fill_container", justifyContent="center")
    contact = row("联系方式", [
        tag("公众号：某某文化", bg="$c-inv", fg="$c-red-deep"),
        text(ids, "电话", "现场咨询 400-000-0000", 28, 400, "$c-red-pale",
             family=CJK, width="fit_content", growth="auto",
             line_height=1.4),
    ], gap=28, justifyContent="start")
    s["content"]["children"] = [body, contact]
    return s


# 画布上的排布。照抄 tpl_slides 的三条常数与理由：横向间隔 120；纵向要额外
# 加 240，因为画布把帧名画在帧的**上方**且偏移不随缩放变化，六帧拼到屏上时
# 120 doc px 只有十几个屏幕像素，第二行的帧名会压在第一行的底边上；每行三
# 帧是因为六帧排成一条 12,000px 的长带只能靠平移阅读。
BOARD_GAP_X = 120
BOARD_GAP_Y = BOARD_GAP_X + 240
BOARDS_PER_ROW = 3


def build():
    boards = [cover(), highlights(), agenda(), venue(), tickets(), closing()]
    for index, board in enumerate(boards):
        board.pop("content", None)
        board["x"] = (index % BOARDS_PER_ROW) * (W + BOARD_GAP_X)
        board["y"] = (index // BOARDS_PER_ROW) * (H + BOARD_GAP_Y)
    return boards


# 对比度（WCAG 相对亮度比，投影场景自设门槛 4.5；数值实测）：
#   c-ink   on c-bg      17.75    c-muted    on c-bg        8.57
#   c-ink   on c-panel   15.07    c-muted    on c-panel     7.28
#   c-red   on c-bg       5.50    c-blue     on c-bg        7.86
#   c-inv   on c-red      5.92    c-inv      on c-blue      8.46
#   c-red-pale on c-red    4.76   c-red-deep on c-inv       7.35
#   c-ink   on c-red-pale 15.37
# 最低一对 4.76 是满版红上的浅红辅助文字（结尾页说明行与票种权益）。全篇
# 没有渐变，所以不存在「按每个 stop 分别量」的问题 —— 每一块底色都是实色，
# lint 读到的就是真值。换主色时先量 c-inv on c-red 和 c-red-pale on c-red。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "活动策划 deck · 公告海报 16:9")
