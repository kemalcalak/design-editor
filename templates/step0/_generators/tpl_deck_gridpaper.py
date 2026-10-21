#!/usr/bin/env python3
"""gridpaper-graphite-deck.op — 方格石墨 · 学术答辩档（8 页 1920×1080）

spec: openpencil-docs/openpencil/generation/deck-system-0809.md §2 · D10
落格：high formality × medium-high density × light

锚点是实验记录本，锚的是**「过程被记录下来因此可被复核」**这件事 ——
不锚烧杯、分子式、齿轮这类学科符号。三个母题：

  方格纸   48px 见方的极淡格线，所有元素落在格上；对齐是**可被看见的**
  标签舌   贴在内容块左上角、向左出血 24px 的一枚斜切标签，语义固定
           蓝=定义/方法 · 绿=结果/示例 · 琥珀=局限/存疑
  图注双联 展品与它的 so-what 强制成对：**没有 so-what 的展品不许上页**；
           反过来，遮住展品后那句话仍然成立，说明展品冗余，删图
  引证槽   页底一条 1px 细线之下的引证带。**任何出现数据、图、他人结论
           的页面必须有引证槽**，空槽即缺陷

### 网格与版心的对账（这一档唯一需要算的地方）

spec 给的边距是「上 96 / 下 112 / 左右 112」，但 1920−112×2 = 1696 不是 48
的整数倍 —— 若照抄，「所有元素落在 48 的整数倍上」这条就在第一页破功。
本档的取法：**左 112 起排、内容宽取 35 格 = 1680，右边距因此是 128**。
上下同理：上 96（2 格）+ 内容 864（18 格）+ 下 120。左右不等本身还顺手
破掉了「万物居中且四边等距」这条 AI 指纹（spec §3.7 第 8 条）。

格线画成满幅而不是只画版心：记录本的格是纸的属性，不是版面的属性。
y=720 那一根格线同时充当 04 页柱图的基线 —— 图不另画轴，**基线就是纸上
本来就有的那条线**，这是「方格纸」这个母题少数几个能同时省掉装饰又增加
可信度的地方。

### 密度槽位的计数口径

spec §3.2 把「一个数值 / 一条要点」各计 1 槽。柱图的 5 个数值标 + 5 个轴标
若逐个计数，任何带图的页都会当场超载 —— 本档按**一个展品计 1 槽**、
**一个编号条目计 1 槽**来数（与「一条要点计 1」同构）。逐页槽数写在各页
函数的注释里，全部落在 spec 给的上限之内。

### 工程约束（spec §6）

- 6.3 线一律 `rectangle`，不用 `line`（layout:none 下 `line` 吃文档绝对坐标）。
- 6.4 layout:none 的 children[0] 在最上 —— 每页先内容、后页脚家具、
  **最后才是格线**，否则格线会盖在正文上。
- 6.9 字体两层，产稿用保底层 `Noto Sans SC` + `Inter`。
- 6.10 渲染栈没有 tabular 数字：05 页表格的数值列宽全部写死。
- 6.13 rectangle 不递归渲染子节点 —— 标签舌、展品底一律 `frame`。
- 6.15 板位 x = (i%3)*2040、y = (i//3)*1440。

对比度实测见文件末尾，最低一对 3.83（`graphite.faint`，只承载 ≥24px 的
引证 / 页码，属 spec §4.4 的合法例外）。
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import Ids, color_vars, frame, path, rect, solid, text, write_doc

ids = Ids()

# ---------------------------------------------------------------- 色板
#
# spec §2 D10 的 oklch 表。两处推导值得在代码里留一句：
#   grid.line 只比底低 0.05 —— 格子必须「在那里但不参与竞争」，一旦能被
#   主动读到就从可信度变成了噪声；
#   graphite 用带 chroma 的冷灰而不是纯黑 —— 模拟石墨反光，这是与「白底
#   黑字学术 PPT」唯一但足够的分野。
VARS = color_vars({
    "c-paper":     "#F4F8F8",  # 页面底
    "c-grid":      "#E0E8E9",  # 方格线（间距 48）
    "c-panel":     "#E8EEEF",  # 展品底 / 方法块
    "c-hair":      "#CBD0D1",  # 分隔线 / 表格线
    "c-graphite":  "#1F2329",  # 主文字
    "c-soft":      "#575B60",  # 次级文字
    "c-faint":     "#797E83",  # 引证 / 页码（仅 ≥24px）
    "c-blue":      "#1D6294",  # 索引：定义 / 方法（主 accent）
    "c-green":     "#33724C",  # 索引：结果 / 示例
    "c-amber":     "#A17221",  # 索引：局限 / 存疑
})

W, H = 1920, 1080
GRID = 48

# 版心：左 112 起、35 格宽、上 96 起、18 格高。右 128 / 下 120 是余数，
# 不是随手写的 —— 见模块 docstring 的对账。
LEFT, TOP = 112, 96
CONTENT_W, CONTENT_H = GRID * 35, GRID * 18          # 1680 × 864
RIGHT, BOTTOM = LEFT + CONTENT_W, TOP + CONTENT_H    # 1792 / 960

CJK = "Noto Sans SC"
NUM = "Inter"

# ---------------------------------------------------------------- 字阶
COVER = (84, 1.10, -2)     # round(84 × -0.02) = -2
TITLE = (60, 1.18, 0)      # 论断句页标题，≤2 行
EXHIBIT = (36, 1.35, 0)    # 展品标题 / 条目标题
BODY = (30, 1.70, 0)       # 正文，**30 是地板不是目标**（spec §2 D10 第 6 条）
CITE = (24, 1.45, 0)       # 引证 / 页码 / 轴标 / 引用列表


def th(step, lines=1):
    size, line_height, _ = step
    return round(size * line_height * lines)


# ---------------------------------------------------------------- 文本
def txt(name, content, step, weight, color, *, inner, family=CJK,
        align=None, width="fill_container"):
    """文本节点，产稿时执行行长闸。

    这里的断言比 `qa/cjkcheck.py` 更严：**不看汉字占比，每一条硬换行都要
    落在容量内**。cjkcheck 会跳过拉丁占比高的串（按 1em 一个字算它们是假
    阳性），但拉丁比汉字窄，用汉字容量去卡它只会更保守 —— 保守的断言不会
    放过真问题，只会多改几句文案。
    """
    size, line_height, spacing = step
    per = int(inner // size)
    # `fit_content` 的节点是收缩包裹的，永远不会折行 —— cjkcheck 也正是按
    # `growth: auto` 跳过它们的。对它断言只会得到假阳性。
    for line in ([] if width == "fit_content" else content.split("\n")):
        assert len(line) <= per, (
            f"{name}: 「{line}」{len(line)} 字 > 该块容量 {per} 字"
            f"（inner={inner} fs={size}）—— 改文案或换页型，不许缩字号"
        )
    return text(ids, name, content, size, weight, color, family=family,
                line_height=line_height, spacing=spacing, width=width,
                growth="auto" if width == "fit_content" else "fixed-width",
                align=align)


def num(name, content, step, weight, color, *, width="fit_content",
        align=None):
    """数字与拉丁走西文族。跨行跨列要对齐的数值列宽一律写死（spec §6.10）。"""
    size, line_height, spacing = step
    return text(ids, name, content, size, weight, color, family=NUM,
                line_height=line_height, spacing=spacing, width=width,
                growth="auto" if width == "fit_content" else "fixed-width",
                align=align)


# ---------------------------------------------------------------- 结构
def block(name, x, y, width, children, *, gap=24, padding=None, fill=None,
          align="start"):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="vertical", gap=gap, alignItems=align,
                 fill=fill or [])
    if padding:
        node["padding"] = padding
    node["x"], node["y"] = x, y
    node["children"] = children
    return node


def row(name, x, y, width, children, *, gap=24, padding=None, fill=None,
        align="start"):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="horizontal", gap=gap, alignItems=align,
                 fill=fill or [])
    if padding:
        node["padding"] = padding
    if x is not None:
        node["x"], node["y"] = x, y
    node["children"] = children
    return node


def hline(x, y, width, *, thickness=1, color="$c-hair"):
    node = rect(ids, "细线", width=width, height=thickness,
                fill=solid(color))
    node["x"], node["y"] = x, y
    return node


# ---------------------------------------------------------------- 方格纸
def grid_field():
    """满幅方格。**必须是每页 children 的最后一批** —— jian 的 index 0 在最上，
    写在前面格线就会横穿正文，看起来像被划掉了。

    格子锚在版心原点 (112, 96) 而不是画布原点：内容的左基线因此正好落在
    一根格线上，「元素落在格上」这句话在第一眼就能被验证。
    """
    lines = []
    for k in range(-2, 38):
        x = LEFT + k * GRID
        if 0 <= x <= W:
            node = rect(ids, "格线 · 竖", width=1, height=H,
                        fill=solid("$c-grid"))
            node["x"], node["y"] = x, 0
            lines.append(node)
    for k in range(-2, 21):
        y = TOP + k * GRID
        if 0 <= y <= H:
            node = rect(ids, "格线 · 横", width=W, height=1,
                        fill=solid("$c-grid"))
            node["x"], node["y"] = 0, y
            lines.append(node)
    return lines


def page(name):
    node = frame(ids, name, width=W, height=H, layout="none",
                 fill=solid("$c-paper"), clipContent=True)
    node["children"] = []
    return node


def finish(p, index, citation=None):
    """页脚家具 + 格线，每页最后一步。

    引证槽的细线**每页都画**（含没有引证的封面与结论页）：页码与页标位置
    属于「不允许跨页变」的一栏（spec §3.3），线是那条基线的一部分；变的
    只是线下写不写引证。
    """
    p["children"].append(hline(LEFT, BOTTOM, CONTENT_W))
    if citation:
        p["children"].append(block("引证槽", LEFT, BOTTOM + 24, 1240, [
            txt("引证", citation, CITE, 400, "$c-faint", inner=1240),
        ]))
    p["children"].append(block("页码", RIGHT - 200, BOTTOM + 24, 200, [
        num("页码", f"{index} / 8", CITE, 400, "$c-faint", width=200,
            align="right"),
    ]))
    p["children"] += grid_field()
    return p


# ---------------------------------------------------------------- 标签舌
TAB_H = 40
TAB_BLEED = 24


def index_tab(x, y, label, color, *, width=208):
    """索引标签舌。左端 12px 斜切（非圆角），语义由颜色固定。

    **向左出血 24px 是关键**：它让标签读作「贴上去的」而不是「排进去的」，
    同时天然破坏了统一左边距 —— 每页的左基线因此不是一条死线。

    高度取 40 而不是 spec 写的 32：标签内的文字要站在 24px 的字号地板之上
    （spec §3.4 <20 直接 FAIL、20–28 是 WARN 带），32 的舌高塞不下 24 的字
    还留呼吸。这是往上调，不是往下让。
    """
    shape = path(ids, "标签舌 · 形", f"M12 0 L{width} 0 L{width} {TAB_H} L0 {TAB_H} Z",
                 width=width, height=TAB_H, fill=solid(color))
    shape["x"], shape["y"] = 0, 0
    caption = txt("标签舌 · 字", label, CITE, 600, "$c-paper", inner=width - 44,
                  width="fit_content")
    caption["x"], caption["y"] = 30, 6
    node = frame(ids, f"标签舌 · {label}", width=width, height=TAB_H,
                 layout="none", fill=[])
    node["x"], node["y"] = x - TAB_BLEED, y
    # children[0] 最上：字压在形上。
    node["children"] = [caption, shape]
    return node


# ================================================================ 01 封面
def cover():
    """槽位 5：短线不计（非文本）+ 主标 / 副标 / 作者 / 单位日期 / 页码。
    重心压在版心下半 —— 整套八页里唯一一页把主体放在下三分之一。"""
    p = page("01 封面")
    mark = rect(ids, "起始短线", width=GRID, height=6, fill=solid("$c-blue"))
    mark["x"], mark["y"] = LEFT, 384
    p["children"].append(mark)

    p["children"].append(block("主标", LEFT, 432, CONTENT_W, [
        txt("主标题", "夜间照明强度与城市鸟类\n鸣唱起始时间的关系", COVER,
            700, "$c-graphite", inner=CONTENT_W),
    ]))
    p["children"].append(block("副标", LEFT, 672, CONTENT_W, [
        txt("副标题", "三座城市 24 个样点、两个繁殖季的同步声学与照度监测",
            BODY, 400, "$c-soft", inner=CONTENT_W),
    ]))
    p["children"].append(row("落款", LEFT, 816, CONTENT_W, [
        txt("作者", "陆停云 · 沈砚秋", CITE, 500, "$c-graphite",
            inner=400, width="fit_content"),
        txt("单位", "城市生态研究所 · 声景实验室", CITE, 400, "$c-faint",
            inner=520, width="fit_content"),
        num("日期", "2026.08", CITE, 400, "$c-faint"),
    ], gap=32, align="center"))
    return finish(p, 1)


# =========================================================== 02 问题与缺口
GAP_COLS = [
    ("01", "夜间照明仍在扩张", [
        "全球人工夜间照明面积每年",
        "增长约百分之二。三座城市的",
        "地面照度中位数十年间由",
        "5 lux 升至 12 lux。",
    ]),
    ("02", "觅食有数据，鸣唱没有", [
        "既有工作集中在觅食与迁徙",
        "时序；鸣唱起始时间只有零散",
        "记录，且多数未同步测量样",
        "点的实际照度。",
    ]),
    ("03", "把两者同点同时量", [
        "本研究在同一样点同步记录",
        "地面照度与鸣唱起始时间，",
        "给出两者的剂量—反应关系，",
        "而不是相关系数。",
    ]),
]


def problem():
    """三段编号：现状 / 缺口 / 本文回答。槽位 6（标题 + 3 段 + 引证 + 页码），
    上限 6 —— 这一页正好用满，再加一句就该拆页。"""
    p = page("02 问题与缺口")
    p["children"].append(block("页标题", LEFT, TOP, CONTENT_W, [
        txt("标题", "已有研究说明照明推迟了觅食，\n却没有回答鸣唱是否同步位移",
            TITLE, 700, "$c-graphite", inner=CONTENT_W),
    ]))

    col_w = 528
    for index, (no, head, lines) in enumerate(GAP_COLS):
        x = LEFT + index * (col_w + GRID)
        p["children"].append(block(f"缺口 {no}", x, 384, col_w, [
            num("序号", no, CITE, 500, "$c-faint", width=col_w),
            txt("小标题", head, EXHIBIT, 600, "$c-graphite", inner=col_w),
            txt("说明", "\n".join(lines), BODY, 400, "$c-soft", inner=col_w),
        ], gap=20))
    return finish(p, 2,
                  "[1] 陈黎 等，2024，城市生态学报，41(3): 217–229")


# ================================================================ 03 方法
# 每步的说明压成**一行**：四步的说明各占两行时，流程块会从 528 一路排到
# 1083，正好压进页底的引证槽（第一版实测被 audit 抓到四条 TEXT OVERLAP）。
# 按 spec §3.1 的顺序，第一动作是缩短文案 —— 不是缩字号、也不是删引证槽。
METHOD_STEPS = [
    ("01", "布点与配对", "八个样点覆盖 0.5 至 50 lux，亮暗成对"),
    ("02", "同步测量", "声学记录仪与照度计同点同夜同步记录"),
    ("03", "起始时间判读", "两名判读者独立标注，分歧交第三人裁定"),
    ("04", "统计模型", "混合线性模型，照度取对数，控温度与云量"),
]


def method():
    """方法块（蓝标签舌）+ 四步编号行。槽位 8（标题 + 方法块 + 4 步 + 引证 +
    页码 = 8，上限 9）。编号行是横排三段式，与 02 的三栏不同型。"""
    p = page("03 方法")
    p["children"].append(block("页标题", LEFT, TOP, CONTENT_W, [
        txt("标题", "在同一样点上把照度与鸣唱起始时间同步记录",
            TITLE, 700, "$c-graphite", inner=CONTENT_W),
    ]))

    p["children"].append(index_tab(LEFT, 200, "定义 · 方法", "$c-blue"))
    p["children"].append(block("方法块", LEFT, 240, CONTENT_W, [
        txt("方法陈述",
            "本文把「鸣唱起始时间」定义为日落次日拂晓、样点上出现的第一段"
            "持续两秒以上的种内鸣唱；\n「地面照度」取记录仪水平面上一米处的"
            "照度中位数。两个量都在同一样点、同一夜内取得。",
            BODY, 400, "$c-graphite", inner=CONTENT_W - 96),
    ], padding=[48, 48], fill=solid("$c-panel")))

    steps = []
    for index, (no, head, note) in enumerate(METHOD_STEPS):
        if index:
            steps.append(rect(ids, "步间细线", width="fill_container",
                              height=1, fill=solid("$c-hair")))
        steps.append(row(f"步骤 {no}", None, None, "fill_container", [
            num("序号", no, CITE, 500, "$c-faint", width=72),
            txt("步骤名", head, EXHIBIT, 600, "$c-graphite", inner=336,
                width=336),
            txt("步骤说明", note, BODY, 400, "$c-soft", inner=1200,
                width=1200),
        ], gap=24, padding=[18, 0], align="start"))
    p["children"].append(block("流程", LEFT, 528, CONTENT_W, steps, gap=0))
    return finish(p, 3,
                  "[2] 方沐 等，2023，动物行为学报，18(2): 96–108")


# =========================================================== 04 结果 · 图
# 五个照度档的鸣唱提前量（分钟）。12 px / 分钟，柱顶因此都落在整数像素上。
BARS = [("0.5 lux", 2.1), ("1 lux", 5.8), ("5 lux", 13.6),
        ("20 lux", 22.4), ("50 lux", 30.9)]
BAR_W, BAR_GAP, BASELINE, PX_PER_MIN = 144, GRID, 720, 12


def result_chart():
    """图左文右。展品占 20 格、解读占 14 格（58:42，不做 50/50）。

    图上没有 y 轴、没有网格线、没有图例 —— 数值直接标在柱顶，而基线用的
    就是纸上那根 y=720 的格线。只有末档走 accent 蓝，其余四档走中性：
    结论在「最高一档」，着色即立场，立场只表达一次。

    槽位 8（标题 + 展品 + 图注双联 + 右标题 + 右说明×2 + 引证 + 页码）。
    展品按一个单位计 1 槽，口径见模块 docstring。
    """
    p = page("04 结果 · 图")
    p["children"].append(block("页标题", LEFT, TOP, CONTENT_W, [
        txt("标题", "照度每上升十倍，鸣唱起始时间提前约九分钟",
            TITLE, 700, "$c-graphite", inner=CONTENT_W),
    ]))

    for index, (label, minutes) in enumerate(BARS):
        x = LEFT + index * (BAR_W + BAR_GAP)
        height = round(minutes * PX_PER_MIN)
        top = BASELINE - height
        is_key = index == len(BARS) - 1
        bar = rect(ids, f"柱 {label}", width=BAR_W, height=height,
                   fill=solid("$c-blue" if is_key else "$c-hair"))
        bar["x"], bar["y"] = x, top
        p["children"].append(bar)
        value = block(f"柱值 {label}", x, top - 56, BAR_W, [
            num("数值", f"{minutes:.1f}", BODY, 600,
                "$c-blue" if is_key else "$c-graphite", width=BAR_W,
                align="center"),
        ])
        p["children"].append(value)
        p["children"].append(block(f"轴标 {label}", x, 736, BAR_W, [
            num("轴标", label, CITE, 400, "$c-soft", width=BAR_W,
                align="center"),
        ]))

    # 图注双联：展品下一条 2px 蓝顶线，线下**一句结论**（不是图题）。
    # 判据：遮住上面这张图，这句话就不成立 —— 成立的话说明图是冗余的。
    p["children"].append(hline(LEFT, 816, 960, thickness=2, color="$c-blue"))
    p["children"].append(block("图注双联", LEFT, 832, 960, [
        txt("so-what", "提前量在 5 lux 之后加速，说明存在阈值而非线性响应；"
            "\n照明规划的着力点因此在低照度段，不在总量。",
            BODY, 400, "$c-graphite", inner=960),
    ]))

    read_x, read_w = 1120, 672
    p["children"].append(block("解读标题", read_x, 288, read_w, [
        txt("解读标题", "阈值出现在\n城市常见照度区间内", EXHIBIT, 600,
            "$c-graphite", inner=read_w),
    ]))
    p["children"].append(block("解读一", read_x, 432, read_w, [
        txt("解读一", "0.5 与 1 lux 之间差异不显著；\n"
            "5 lux 起提前量增幅变陡，\n"
            "而 5 至 20 lux 正是三座城市\n住宅区道路照明的常见区间。",
            BODY, 400, "$c-soft", inner=read_w),
    ]))
    p["children"].append(block("解读二", read_x, 684, read_w, [
        txt("解读二", "把照度压到 5 lux 以下，提前量\n"
            "的中位数回到十分钟以内 ——\n这是本文唯一可操作的阈值。",
            BODY, 400, "$c-soft", inner=read_w),
    ]))
    return finish(p, 4,
                  "[3] 照度与声学原始数据，城市生态研究所声景实验室，2026")


# =========================================================== 05 结果 · 表
TABLE_COLS = (384, 216, 372, 324, 312)
TABLE_HEAD = ("照度档（lux）", "样点数", "提前量中位数（分）", "四分位距", "p 值")
TABLE_ROWS = [
    ("0.5 – 1", "5", "2.1", "1.4", "0.31"),
    ("1 – 5", "5", "5.8", "2.0", "0.04"),
    ("5 – 15", "5", "13.6", "3.1", "< 0.01"),
    ("15 – 30", "5", "22.4", "3.8", "< 0.01"),
    ("30 – 50", "4", "30.9", "4.6", "< 0.01"),
]


def table_row(name, cells, *, weight, color, fill=None, head=False):
    kids = []
    for index, (width, content) in enumerate(zip(TABLE_COLS, cells)):
        # 表头与数据用**同一条对齐规则**（首列左、数值列右）。第一版表头
        # 一律左对齐，结果「样点数」贴在列首、「5」贴在列尾，读起来像两张
        # 表叠在一起 —— 数值列的表头必须跟着数值走。
        align = "left" if index == 0 else "right"
        if head:
            kids.append(txt(f"{name} · 格", content, BODY, weight, color,
                            inner=width, width=width, align=align))
        else:
            # 数值列全部走西文族 + 写死列宽：渲染栈没有 tabular 数字，
            # 靠 fit_content 指望它们自己对齐，一行长一点整张表就抖。
            kids.append(num(f"{name} · 格", content, BODY, weight, color,
                            width=width, align=align))
    return row(name, None, None, "fill_container", kids, gap=24,
               padding=[22, 16], fill=fill, align="start")


def result_table():
    """表格。表头 2px 蓝下线、行间 1px 细线、**末行无线**、斑马走展品底。
    整框边线一根没有。槽位 10（标题 + 表头 + 5 行 + 双联 + 引证 + 页码）。

    与 04 的分工是硬规则：趋势用图，**精确数值比较才用表**，两者不许同页。
    """
    p = page("05 结果 · 表")
    p["children"].append(block("页标题", LEFT, TOP, CONTENT_W, [
        txt("标题", "分档统计给出的提前量与照度呈单调关系",
            TITLE, 700, "$c-graphite", inner=CONTENT_W),
    ]))

    rows = [table_row("表头", TABLE_HEAD, weight=600, color="$c-graphite",
                      head=True),
            rect(ids, "表头线", width="fill_container", height=2,
                 fill=solid("$c-blue"))]
    for index, cells in enumerate(TABLE_ROWS):
        if index:
            rows.append(rect(ids, "行间细线", width="fill_container",
                             height=1, fill=solid("$c-hair")))
        rows.append(table_row(f"表行 {index + 1}", cells, weight=400,
                              color="$c-graphite",
                              fill=solid("$c-panel") if index % 2 else None))
    p["children"].append(block("表格", LEFT, 240, CONTENT_W, rows, gap=0))

    p["children"].append(hline(LEFT, 840, CONTENT_W, thickness=2,
                               color="$c-blue"))
    p["children"].append(block("图注双联", LEFT, 856, CONTENT_W, [
        txt("so-what", "1 至 5 lux 之间 p 值跨过 0.05，"
            "这一档是统计上的分界，也是照明规范该写的那个数。",
            BODY, 400, "$c-graphite", inner=CONTENT_W),
    ]))
    return finish(p, 5,
                  "[3] 照度与声学原始数据，城市生态研究所声景实验室，2026")


# ================================================================ 06 局限
LIMITS = [
    ("样本偏在温带", [
        "三座城市都位于北纬三十",
        "到四十度之间。热带与高",
        "纬度城市的鸣唱物候受日",
        "长驱动更强，不宜外推。",
    ]),
    ("只覆盖六个物种", [
        "六个物种约占样点内个体",
        "的七成，剩下三成、尤其",
        "是夜行性种类的响应仍是",
        "空白。",
    ]),
    ("照度不等于光谱", [
        "本文只记录了照度，未区",
        "分光谱成分。蓝光占比可",
        "能才是更直接的驱动量，",
        "这需要另一组实验。",
    ]),
]


def limitation():
    """三条局限，各带一枚琥珀标签舌。槽位 7（标题 + 3 条 + 引证 + 页码）。
    局限页写在结论页之前而不是被藏进附录 —— 答辩的可信度来自它。"""
    p = page("06 局限")
    p["children"].append(block("页标题", LEFT, TOP, CONTENT_W, [
        txt("标题", "三个条件限制了这个结论能推广到多远",
            TITLE, 700, "$c-graphite", inner=CONTENT_W),
    ]))

    col_w = 528
    for index, (head, lines) in enumerate(LIMITS):
        x = LEFT + index * (col_w + GRID)
        p["children"].append(index_tab(x, 288, f"局限 0{index + 1}",
                                       "$c-amber", width=168))
        p["children"].append(block(f"局限 {index + 1}", x, 360, col_w, [
            txt("局限标题", head, EXHIBIT, 600, "$c-graphite", inner=col_w),
            txt("局限说明", "\n".join(lines), BODY, 400, "$c-soft",
                inner=col_w),
        ], gap=20))
    p["children"].append(block("适用范围", LEFT, 744, CONTENT_W, [
        txt("适用范围", "这三条都不改变结论的方向，只限定它的适用范围："
            "温带、六个物种、以照度为自变量。",
            BODY, 400, "$c-graphite", inner=CONTENT_W),
    ]))
    return finish(p, 6,
                  "[4] 韦知远，2025，光生物学与物候，7(1): 33–48")


# ================================================================ 07 结论
CONCLUSIONS = [
    "照度与鸣唱提前量之间是单调的剂量—反应关系",
    "把地面照度压到 5 lux 以下，提前量回到十分钟以内",
    "调整灯具遮光角比压低总功率更省电，也更有效",
]


def conclusion():
    """收尾页是结论不是「谢谢」—— Q&A 期间它会一直挂在屏幕上，
    那段时间应该被结论占据（spec §2 D10 Strictly avoid 1）。
    槽位 6（标题 + 3 条 + 意义 + 页码）。"""
    p = page("07 结论")
    p["children"].append(index_tab(LEFT, 96, "结果 · 结论", "$c-green",
                                   width=192))
    p["children"].append(block("页标题", LEFT, 168, CONTENT_W, [
        txt("标题", "把灯调暗两档，可以把鸣唱提前量压回十分钟以内",
            TITLE, 700, "$c-graphite", inner=CONTENT_W),
    ]))

    items = []
    for index, line in enumerate(CONCLUSIONS):
        if index:
            items.append(rect(ids, "条间细线", width="fill_container",
                              height=1, fill=solid("$c-hair")))
        items.append(row(f"结论 {index + 1}", None, None, "fill_container", [
            num("序号", f"0{index + 1}", CITE, 500, "$c-faint", width=72),
            txt("结论", line, EXHIBIT, 500, "$c-graphite", inner=1584,
                width=1584),
        ], gap=24, padding=[26, 0], align="start"))
    p["children"].append(block("结论列表", LEFT, 336, CONTENT_W, items, gap=0))

    p["children"].append(block("意义", LEFT, 720, CONTENT_W, [
        txt("意义", "这意味着城市照明规划可以不必在道路亮度与生态成本之间"
            "二选一：\n把同样的流明压低照射角、留在路面上，两个目标能同时"
            "达成。",
            BODY, 400, "$c-soft", inner=CONTENT_W),
    ]))
    return finish(p, 7)


# ================================================================ 08 引用
# 每条两行：**作者与年份一行、出处一行**。这是引用列表本来的排法，也顺手
# 把每一行都压进 34 字的容量内 —— 引证的可核对性来自结构，不来自挤在一行。
REFERENCES = [
    ("[1] 陈黎，周砚，2024", "城市生态学报，41(3): 217–229"),
    ("[2] 方沐，柯砚池，2023", "动物行为学报，18(2): 96–108"),
    ("[3] 声景实验室，2026", "内部数据集 SL-26-04"),
    ("[4] 韦知远，2025", "光生物学与物候，7(1): 33–48"),
    ("[5] 苏槿，2022", "声景与城市，3(4): 51–66"),
    ("[6] 何砚舟，罗青，2021", "鸟类学研究，12(2): 140–152"),
    ("[7] 池屿，2024", "照明工程学报，35(6): 88–97"),
    ("[8] 林停舟，2023", "生态学杂志，42(9): 2201–2213"),
    ("[9] 邬砚，2020", "城市环境与健康，9(3): 12–24"),
    ("[10] 谢岚溪，2025", "夜间生态学评论，2(1): 5–19"),
]


def references():
    """全页引证。两栏五行，24px —— 这一页是唯一允许整页只有一种字号的页，
    因为它本来就不是幻灯片，是一份可被逐条核对的清单。
    槽位 13（标题 + 10 条 + 联系 + 页码），上限 14。"""
    p = page("08 引用")
    p["children"].append(block("页标题", LEFT, TOP, CONTENT_W, [
        txt("标题", "引用文献", EXHIBIT, 600, "$c-graphite", inner=CONTENT_W),
    ]))

    col_w = 816
    for index, (head, source) in enumerate(REFERENCES):
        x = LEFT + (index // 5) * (col_w + GRID)
        y = 240 + (index % 5) * 120
        p["children"].append(block(f"引用 {index + 1}", x, y, col_w, [
            txt("引用 · 作者", head, CITE, 500, "$c-graphite", inner=col_w),
            txt("引用 · 出处", source, CITE, 400, "$c-soft", inner=col_w),
        ], gap=4))

    p["children"].append(block("联系", LEFT, 864, CONTENT_W, [
        txt("联系", "通信作者：陆停云（城市生态研究所 · 声景实验室）",
            CITE, 400, "$c-faint", inner=CONTENT_W),
    ]))
    return finish(p, 8)


# ---------------------------------------------------------------- 板位
BOARD_X, BOARD_Y, PER_ROW = 2040, 1440, 3


def build():
    boards = [cover(), problem(), method(), result_chart(), result_table(),
              limitation(), conclusion(), references()]
    for index, board in enumerate(boards):
        board["x"] = (index % PER_ROW) * BOARD_X
        board["y"] = (index // PER_ROW) * BOARD_Y
    return boards


# 对比度（WCAG 相对亮度比，spec §2 D10 色板表实算值）：
#   c-graphite on c-paper  14.75
#   c-soft     on c-paper   6.39
#   c-faint    on c-paper   3.83  ← 仅承载 ≥24px 的引证 / 页码 / 轴标
#   c-graphite on c-panel  13.46（方法块、表格斑马行）
#   c-blue     on c-paper   6.08（标签舌反向同为 6.08）
#   c-green    on c-paper   5.37
#   c-amber    on c-paper   3.97 ← 标签舌上的 24px 纸色字，属大字档（≥3.0）
#   c-grid / c-hair 只画线不承载文字，按 spec §4.4 第 2 条豁免。
#
# 字号地板（spec §3.4）：最小 24，最大 84 ≥ 60 的层级线，84 / 30 = 2.8 ≥ 2.5。
# 单页字阶档数：01=3 / 02=4 / 03=4 / 04=4 / 05=3 / 06=4 / 07=4 / 08=2，均 ≤4。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "方格石墨 · 学术答辩档")
