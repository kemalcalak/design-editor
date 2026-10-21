#!/usr/bin/env python3
"""datareport-grid-carousel.op — 数据报告轮播（6 板 · 1080×1440 · 3:4）

叙事类型：**数据报告**。这一档最容易翻车的地方不是图表画得丑，是**连续
数据页**：读者划到第三张柱状图就跳过了。所以 card-system §4.2 定了一条硬
规则 —— D 族每出现一次，前后必须各有一板非 D。本套严格执行：第 2 板与第
4 板是数据，中间隔着第 3 板的文字说明，那一板的任务就是**教读者怎么读**
后面那组数。

轮播还给了数据一个长图没有的优势：**一板一个数**。长图上五个数字并排，
读者一个都记不住；轮播里划一次只看见一个，最后那张目录才把它们收拢。

### 主题：T5 网格汉字 `grid-hanzi`（亮 / 中性 / 数据·方法论）

**采样 → 收敛 → 论证**（沿用 card-system-0808 §3 T5）

  - 采样：**不采实物色**。这是 10 套主题里唯一的无采样主题 —— 国际主义的
    前提就是色彩不承载文化信息，只承载信号。
  - 收敛：中性序列 L 1.00 / 0.96 / 0.86 / 0.52 / 0.18（chroma 全 0，真中
    性）；单支信号红 H28 C0.190。
  - 论证：**信号红的 chroma 拉到 0.19（超出主色区间上沿）是刻意的** ——
    在一个完全无彩的系统里，红是唯一的信号，它必须是纯信号强度而不是「一
    个红」。这与印刷色 chroma 纪律冲突，冲突处以「只用小面积（≤3%）」化
    解。选它承载数据：数据的可信感来自秩序，而秩序在视觉上就是对齐、字重
    和留白 —— 加任何一个装饰，读者都会开始怀疑你在替数字化妆。

**最近邻论证**：库里最近的是本批的 `highlighter-notebook`（#FAFAF8）。两
者都近白，但那套有横格、页边红线、三支高亮带，本套**一个装饰都没有**：只
有 1px 线、实心黑横杠、一个红方块。缩略图里一张是「写满的纸」，一张是
「空白加一条黑杠」。与 `knowledge-carousel` 的分野更简单：那套有圆角卡片
和靛蓝，本套圆角一律 0、无彩。

### 母版规则（六板的硬约束）

**每板固定，不许变**
  1. 画幅 1080×1440，安全边距 左右 80 / 上 96 / 下 128。12 列 × 62 + 11
     沟槽 × 16 = 920。
  2. **黑横杠**：页眉正下方一条高 12px、宽为列宽整数倍（4 列 = 264）的实
     心黑杠。它是本套唯一的「图形」，六板一条不差。
  3. 页眉：左「数据报告 · 六板」，右「NN / 06」（DM Mono，Caption 32px）。
  4. 页脚：1px `c-rule` 线 + 署名 + **来源行**。
  5. 字族：汉字 Noto Sans SC / **一切数字走 DM Mono**（等宽，1 和 8 同宽，
     列才不抖 —— 数字不等宽，数据的可信感直接打折）。
  6. **标题一律左对齐**。国际主义是左对齐系统，居中即偏离。
  7. **信号红一板只出现一处**。

**允许变，且只有这些能变**
  - 主体区的信息结构（下面六板各用一条配方）。
  - 黑横杠之下那行小标题的内容。
  - 信号红落在哪一处。

### 配方编排（card-system-0808 §4.2）

    01 A2 数字锚点 → 02 D4 三栏指标 → 03 F1 段落纯排
    04 D2 条形对照 → 05 B1 满版一句 → 06 G3 目录回看

首板 A 族、末板 G 族；相邻两板不同族；**D 族出现两次（第 2、4 板），前后
各被非 D 包夹**（F1 在中间，B1 在其后）；B 族 1 次且落在后 1/3（第 5 板）；
覆盖 A/D/F/B/G 五族。

### 负约束（本模板明令不做的事）

  - **圆角一律 0**。这不是风格偏好，是本主题的定义 —— 一个圆角就把国际主
    义换成了「现代 UI」。
  - **无阴影、无渐变、无纹理**。允许的图形只有三个：1px 线、实心黑横杠、
    信号红方块。
  - **不用饼图**（手机上读不准角度）、不用 3D、不用带阴影的柱。
  - **不做彩虹分类**。条形与方阵只用「中性 + 一支信号红」，红只标那一条要
    你看的。
  - **红只出现一次**。第二次出现，它就不再是信号。
  - 标题不居中；不用任何非 90° 的旋转。
  - 每板正文不超过 4 行；每张数据板必须带来源行。

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

from oplib import Ids, color_vars, frame, rect, solid, text, write_doc

ids = Ids()

VARS = color_vars({
    "c-bg":     "#FFFFFF",   # white       页面底
    "c-panel":  "#F2F2F2",   # paper.grey  分区块
    "c-rule":   "#D1D1D1",   # rule        分隔线 / 表格线
    "c-mid":    "#696969",   # grey.mid    次级文字
    "c-ink":    "#121212",   # black       主文字 / 粗横杠
    "c-signal": "#CC342C",   # signal.red  信号：一板一处
})

CJK = "Noto Sans SC"
# 等宽数字。card-system 点名 IBM Plex Mono；仓里烤进桌面端的等宽族是
# DM Mono（crates/op-host-desktop/assets/fonts/DMMono-*.ttf），字面宽度
# 关系一致。用没打包的字族会退回系统字体，数字就不再等宽 —— 那正是这套
# 主题最不能丢的一条。
NUM = "DM Mono"

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

FS_DISPLAY_L, FS_DISPLAY, FS_T1, FS_T2 = 120, 88, 64, 48
FS_BODY, FS_CAPTION = 40, 32
LH_DISPLAY_L, LH_DISPLAY = 1.10, 1.15
LH_TITLE, LH_BODY, LH_CAPTION = 1.3, 1.7, 1.5

SERIES = "数据报告 · 六板"
TOTAL = 6
BAR_W, BAR_H = COLUMN * 4 + GUTTER * 3, 12   # 黑横杠：4 列宽


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


def caption(name, content, color="$c-mid", *, family=CJK, weight=400,
            width="fit_content"):
    return text(ids, name, content, FS_CAPTION, weight, color, family=family,
                width=width, growth="auto" if width == "fit_content"
                else "fixed-width", line_height=LH_CAPTION)


def body(name, content, color="$c-ink", *, weight=400,
         width="fill_container"):
    return text(ids, name, content, FS_BODY, weight, color, family=CJK,
                width=width, growth="auto" if width == "fit_content"
                else "fixed-width", line_height=LH_BODY)


def numeral(name, value, size, color="$c-ink", *, weight=700, width=None):
    """数字。永远走 DM Mono，永远负字距 —— 这是本套仅有的字距例外。"""
    return text(ids, name, value, size, weight, color, family=NUM,
                width=width or "fit_content",
                growth="auto" if width is None else "fixed-width",
                line_height=1.0, spacing=-2)


def signal_square(size=24):
    """信号红方块。边长 = 基线单位 ×3，一板只准出现一次。"""
    return rect(ids, "信号方块", width=size, height=size,
                fill=solid("$c-signal"))


# ----------------------------------------------------------------- 母版部件
def header(page, label):
    """页眉 + 黑横杠 + 小标题。三件一体，六板位置与尺寸完全锁死。"""
    return col("页眉", [
        row("页眉行", [
            caption("系列名", SERIES, "$c-mid"),
            caption("页码", f"{page:02d}/{TOTAL:02d}", "$c-mid", family=NUM),
        ], gap=24, justify="space_between"),
        rect(ids, "黑横杠", width=BAR_W, height=BAR_H, fill=solid("$c-ink")),
        caption("板签", label, "$c-ink", weight=600),
    ], gap=20)


def footer(source):
    """页脚。来源行是 D 族的硬要求 —— 没有来源的数字不值得相信。"""
    return col("页脚", [
        rect(ids, "分隔线", width="fill_container", height=1,
             fill=solid("$c-rule")),
        row("署名行", [
            caption("账号名", "@ 你的账号名", "$c-ink", weight=600),
            caption("来源", source, "$c-mid"),
        ], gap=24, justify="space_between"),
    ], gap=16)


def board(page, name, label, main, source):
    content = frame(ids, f"{name} · 内容", width="fill_container",
                    height="fill_container", layout="vertical",
                    padding=[TOP, EDGE, BOT, EDGE], gap=0,
                    alignItems="start", fill=[])
    content["children"] = [header(page, label), main, footer(source)]
    shell = frame(ids, f"{page:02d} {name}", width=W, height=H,
                  layout="vertical", fill=solid("$c-bg"), clipContent=True)
    shell["children"] = [content]
    shell["x"] = ((page - 1) % BOARDS_PER_ROW) * (W + GAP)
    shell["y"] = ((page - 1) // BOARDS_PER_ROW) * (H + ROW_GAP)
    return shell


def zone(children, *, justify="center", gap=32, pad_y=48):
    node = frame(ids, "主体", width="fill_container", height="fill_container",
                 layout="vertical", gap=gap, padding=[pad_y, 0],
                 justifyContent=justify, alignItems="start", fill=[])
    node["children"] = children
    return node


# ------------------------------------------------------- 01 封面 · A2 数字锚点
def cover():
    """A2：巨数字 7 列在左，标题 5 列在右对齐底。层级 = 数字 → 单位 → 标题。

    数字锚点是数据档唯一正确的封面：读者三秒内该看见的是**那个数**，不是
    你的标题。所以巨数字走 Display L（120px），标题退到 Title 1。
    """
    figure = row("数字组", [
        numeral("巨数字", "68", FS_DISPLAY_L),
        col("单位组", [
            numeral("单位", "%", FS_T1, "$c-signal"),
            caption("单位说明", "受访者", "$c-mid"),
        ], gap=6, width="fit_content"),
    ], gap=16, align="end")

    main = zone([
        figure,
        text(ids, "主标", "说自己每天\n都在被通知打断", FS_T1, 700, "$c-ink",
             family=CJK, width=INNER, growth="fixed-width",
             line_height=LH_TITLE),
        body("副标", "六板讲清楚这组数背后的三件事。", "$c-mid"),
    ], justify="center", gap=36)
    return board(1, "封面 · 数字锚点", "封面", main, "样本 n=1,204")


# ------------------------------------------------------- 02 概览 · D4 三栏指标
METRICS = [
    ("68", "%", "每天被打断", "↑"),
    ("4.2", "次", "平均每小时", "↑"),
    ("23", "分", "重回专注耗时", "—"),
]


def triad():
    """D4：三等分（各 4 列），纵向对齐基线。趋势符只用 ↑ ↓ —— 不上色。

    D 族硬规则：数字走等宽、每板必须有来源行、颜色只用中性 + 一支信号红。
    这里红只落在第一栏的趋势符上（整板唯一一处），因为它是要你看的那个。
    """
    blocks = []
    for index, (value, unit, label, trend) in enumerate(METRICS):
        hot = index == 0
        block = col(f"指标 {index + 1}", [
            row("数值行", [
                numeral("数值", value, FS_DISPLAY),
                numeral("单位", unit, FS_T2, "$c-mid"),
            ], gap=8, align="end", width="fit_content"),
            caption("趋势", trend, "$c-signal" if hot else "$c-mid",
                    family=NUM, weight=600),
            caption("指标名", label, "$c-ink", width=COLUMN * 4),
        ], gap=12, width=COLUMN * 4)
        blocks.append(block)

    main = zone([
        text(ids, "小标题", "三个数，先摆在这", FS_T1, 700, "$c-ink",
             family=CJK, line_height=LH_TITLE),
        rect(ids, "指标上线", width="fill_container", height=1,
             fill=solid("$c-rule")),
        row("三栏", blocks, gap=GUTTER, align="start"),
    ], justify="center", gap=36)
    return board(2, "概览 · 三栏指标", "指标", main, "2026 专注度调查")


# ------------------------------------------------------- 03 读法 · F1 段落纯排
PARAS = [
    "先看第二个数：每小时 4.2 次，意味着在一个完整的工作时段里，"
    "你几乎没有一段连续超过十五分钟的时间。",
    "再把它和第三个数放一起：每次打断之后要 23 分钟才回得来。"
    "两个数相乘，一天里真正能用的时间就出来了。",
]


def reading():
    """F1：一板一个小标题，两段正文，段首用信号红方块标记第一段。

    这一板存在的唯一理由是把第 2 板和第 4 板隔开（D 族不许连排），但它不
    是填充 —— 它教读者怎么把两个数乘起来。数据档里最值钱的从来是这一步。
    """
    blocks = []
    for index, para in enumerate(PARAS):
        marker = signal_square(20) if index == 0 else rect(
            ids, "段首标记", width=20, height=20, fill=solid("$c-rule"))
        blocks.append(row("段落", [
            col("段首标记位", [marker], gap=0, width=20),
            body("正文", para, "$c-ink", width=INNER - 20 - 24),
        ], gap=24, align="start"))

    main = zone([
        text(ids, "小标题", "这组数该怎么读", FS_T1, 700, "$c-ink",
             family=CJK, line_height=LH_TITLE),
        col("正文区", blocks, gap=36),
    ], justify="center", gap=40)
    return board(3, "读法 · 段落纯排", "怎么读", main, "同上")


# ------------------------------------------------------- 04 对照 · D2 条形对照
BARS = [
    ("即时通讯", 0.46, True),
    ("邮件提醒", 0.24, False),
    ("日程弹窗", 0.17, False),
    ("其他", 0.13, False),
]
BAR_TRACK = COLUMN * 8 + GUTTER * 7   # 条形占 8 列
BAR_LABEL = COLUMN * 3 + GUTTER * 2   # 标签占 3 列
# 百分比数值排在轨道**内部**的右端，不另占一列：3 列标签 + 8 列轨道已经
# 吃满 12 列，再挂一个 fit_content 的数值，整行就比 920 宽 —— 几何审计
# 会直接判「fixed column widths sum wider than the resolved row」。


def bar_compare():
    """D2：条形起点对齐，标签 3 列在左。层级 = 最长条 → 其余 → 标签 → 轴说明。

    只有最长那条是信号红（整板唯一一处红），其余全走中性 —— 排名本身就是
    层级，不需要给每一条配一个颜色。零圆角、零阴影：本主题里柱子就是矩形。
    """
    rows = []
    for label, ratio, hot in BARS:
        track = frame(ids, f"轨道 · {label}", width=BAR_TRACK, height=52,
                      layout="horizontal", gap=0, alignItems="center",
                      fill=solid("$c-panel"))
        track["justifyContent"] = "space_between"
        track["padding"] = [0, 16, 0, 0]
        track["children"] = [
            rect(ids, "条形", width=round(BAR_TRACK * ratio), height=52,
                 fill=solid("$c-signal" if hot else "$c-ink")),
            numeral("条数值", f"{round(ratio * 100)}%", FS_CAPTION,
                    "$c-mid", weight=600),
        ]
        rows.append(row(f"条 · {label}", [
            caption("条标签", label, "$c-ink", width=BAR_LABEL),
            track,
        ], gap=GUTTER, align="center"))

    main = zone([
        text(ids, "小标题", "打断从哪来", FS_T1, 700, "$c-ink",
             family=CJK, line_height=LH_TITLE),
        col("条形组", rows, gap=20),
        caption("轴说明", "占全部打断次数的比例，四类合计 100%", "$c-mid",
                width=INNER),
    ], justify="center", gap=32)
    return board(4, "对照 · 条形", "来源分布", main, "同上，多选题")


# ------------------------------------------------------- 05 结论 · B1 满版一句
def conclusion():
    """B1：一板一句，上下各留 ≥25%，除一个红方块外没有任何装饰。"""
    main = zone([
        signal_square(24),
        text(ids, "金句", "被打断的不是时间，\n是那条正在想的线。",
             FS_DISPLAY, 700, "$c-ink", family=CJK, width=INNER,
             growth="fixed-width", line_height=LH_DISPLAY),
        caption("出处", "本次调查的开放题里，出现最多的一句原话", "$c-mid",
                width=INNER),
    ], justify="center", gap=44, pad_y=120)
    return board(5, "结论 · 满版一句", "结论", main, "开放题 n=612")


# ------------------------------------------------------- 06 目录 · G3 目录回看
INDEX = [
    ("01", "68% 每天被打断"),
    ("02", "4.2 次 / 小时，23 分钟回神"),
    ("03", "两个数相乘才是真实损耗"),
    ("04", "46% 的打断来自即时通讯"),
    ("05", "断掉的是思路不是时间"),
]


def recap():
    """G3：单列，每行「页码 + 标题」。一套 ≥6 板时它优先于三键引导。"""
    lines = []
    for page, title in INDEX:
        lines.append(row("目录行", [
            numeral("回看页码", page, FS_CAPTION, "$c-mid", weight=600,
                    width=COLUMN * 2),
            body("回看标题", title, "$c-ink", width=INNER - COLUMN * 2 - 24),
        ], gap=24, align="center"))

    main = zone([
        text(ids, "收束句", "五个数，\n一张表收回去。", FS_DISPLAY, 700,
             "$c-ink", family=CJK, width=INNER, growth="fixed-width",
             line_height=LH_DISPLAY),
        col("目录", lines, gap=20),
    ], justify="center", gap=40)
    return board(6, "目录 · 回看", "回看", main, "全部数据见评论区链接")


def build():
    return [cover(), triad(), reading(), bar_compare(), conclusion(),
            recap()]


# 对比度（WCAG 相对亮度比，op-design-lint 门槛 2.0；本表逐对实算）：
#   c-ink    on c-bg    18.73    c-mid    on c-bg     5.49
#   c-signal on c-bg     5.13    c-ink    on c-panel  16.73
#   c-mid    on c-panel  4.90    c-rule   on c-bg      1.53
#   c-bg     on c-ink   18.73    c-bg     on c-signal  5.13
# 承载正文的最低一对是 c-mid on c-panel 4.90（第 4 板轨道底上的百分比），
# 高出 lint 门槛 2.45 倍；其余承载正文处最低 5.13，已过 WCAG AA。整套只有
# 一支有彩色，且它对白底 5.13 —— 信号红拉到 chroma 0.19 换来的正是这个：
# 一支在无彩系统里既醒目又读得清的信号。
# c-rule 1.53 低于门槛是对的：它只作 1px 分隔线，属非文字图形，本模板明令
# 它永不承载文字。本套没有「换主色」这件事 —— 中性序列就是层级系统，能换
# 的只有信号红那一支，换时守住对白底 ≥4.5 即可。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "数据报告轮播 · 3:4 六板")
