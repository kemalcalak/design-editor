#!/usr/bin/env python3
"""ecosystem-map-infographic.op — 行业地图 / 生态位长图（1080×N 竖版）

信息图这一档的第十张，回答**「这个行当分成哪几块，谁在哪一块」** —— 一张
把一条链上的四个位置摊开、并且指出哪里还空着的地图。

### 最近邻差异（为什么它不是 ranking 或 concept 换个版）

  - **对 ranking：它不排序，它分区。** 榜单回答「先看哪个」，条目之间可比
    较、有名次；这张回答「在哪一格」，四个分区之间**不可比较** —— 起草区
    和复盘区没有谁更好。所以它没有徽章、没有名次数字，主元素是**二乘二的
    阵列**：位置本身就是信息，横平竖直地摊开，读者一眼看到全貌。
  - **对 concept：它比的不是两个东西的内部差异。** concept 逐维度拆一组概
    念；这张列的是**同一条链上的相邻环节**，每格里再挂三个位点。所以它没
    有表头、没有两栏，卡片里是清单不是对照。
  - **底色档也不同。** 同档另外九张不是近白就是近黑，这张用**中明度石板
    灰**（$c-bg 与 $c-ink 之间只有 11.16:1，其余浅底张都在 15 以上）——
    白卡浮在灰底上，才有「区块贴在图板上」的感觉，也让整档并排时它一眼可
    辨。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：地图的底色不能是纸白 —— 白底上白卡等于没有卡。取中明度石板
    灰做图板，蓝（~205°）做分区标识，是工程图纸的配色来源。
  - **收敛**：一个色相四档明度（$c-zone-1..4，由深到浅），加一条冷灰序
    列。四档全部在白卡上量过（最浅一档 2.29:1）。
  - **论证**：四个分区**是有先后的**（起草 → 成形 → 发布 → 复盘），所以
    这里用同色相的明度梯而不是四个色相 —— 明度梯自带顺序，四个色相则会暗
    示四者互不相干。同时分区身份从不只靠颜色：每格都有编号和名称，色块只
    是快速定位用的锚。

### 负约束（本模板明令不做的事）

  - **不给四个分区配四个色相。** 理由见上：分区有先后，色相没有。
  - 不画连接箭头、不画流程线。这是一张地图不是流程图 —— 谁先谁后由编号
    和阅读顺序给，画上箭头会让人以为必须依次经过。
  - 不画真实地图、不用点状世界地图、不用发散连线（「全球贸易网络风」的三
    件套），这张图里的「地图」是抽象分区，不是地理。
  - 不用蓝紫渐变、霓虹线条、复杂背景纹理，不用 emoji 当图标、不用伪 3D。
  - 一格三个位点封顶，写不下就换掉最弱的那个，不缩字号也不加第四行。
  - **必须写空位。** 只画格子不指空位的地图是行业介绍，不是地图 —— 读者要
    的是「哪里还没人做」。
  - 不写 AI 套话（「生态闭环 / 全链路赋能」），每个空位都要写成一句能验证
    的观察。

硬契约：
  - 内容距边缘 ≥80px（这里 80）
  - 配色全部走 color_vars，改主色改 $c-accent 与 $c-zone-1..4 五处
  - 正文与背景对比度 ≥2.0（本配色最低一对见文件末尾注释）
  - **CJK 行高比西文全线高 0.2**：页头大标题 1.2，区块标题 1.3，正文 1.7
  - **CJK 负字距不超过 -0.02em**；只有西文编号沿用西文 display 的收紧
  - 汉字走 Noto Sans SC，数字与拉丁走 Inter
  - 顶层 frame 必须显式写 x/y，否则多帧会全部堆在原点
  - 根高固定：ROOT_H 是量出来的（见文件末尾），改内容后要重量一次
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import (Ids, color_vars, frame, icon_font, rect, solid, text,
                   write_doc)

ids = Ids()

VARS = color_vars({
    # 中明度石板底。这一档里唯一不是近白也不是近黑的底 —— 白卡要浮起来，
    # 底就不能也是白的。
    "c-bg":          "#CBD5DD",
    "c-surface":     "#FFFFFF",
    "c-band":        "#0E1D28",
    "c-band-muted":  "#93A7B4",
    "c-ink":         "#10202B",
    "c-muted":       "#3E515B",
    "c-accent":      "#1D6FA5",
    "c-accent-deep": "#14547E",
    "c-accent-soft": "#E3F0F8",
    "c-border":      "#AFC0CB",
    # 四个分区的明度梯（深 → 浅）。同一个色相，因为四个分区是有先后的；
    # 四个色相会把它们说成互不相干的四件事。
    "c-zone-1":      "#0E3F60",
    "c-zone-2":      "#1D6FA5",
    "c-zone-3":      "#4C90BC",
    "c-zone-4":      "#7FB2D0",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W = 1080
EDGE = 80
INNER = W - EDGE * 2

LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.3, 1.7

# 二乘二阵列：两列，列间距 20。卡片一律 fill_container，行内两张同宽。
ZONE_GAP = 20

# 量出来的根高（做法同同档另外九张：根设 fit_content 渲一次读 PNG 高度）。
ROOT_H = 2206

# (编号, 分区名, 这一格解决什么, [位点 ×3], 分区色)
ZONES = [
    ("01", "起草区", "从空白到第一版，越快越好。",
     ["大纲与选题库", "AI 起草与改写", "语音转文字"], "$c-zone-1"),
    ("02", "成形区", "把内容变成能直接看的东西。",
     ["排版与模板", "配图与图表", "封面生成"], "$c-zone-2"),
    ("03", "发布区", "一次做完，多个地方能用。",
     ["多平台分发", "尺寸批量适配", "定时与草稿箱"], "$c-zone-3"),
    ("04", "复盘区", "知道下一次该改哪儿。",
     ["数据看板", "评论与私信汇总", "选题命中率"], "$c-zone-4"),
]

GAPS = [
    ("成形区和发布区之间没人接",
     "改一次内容要手动导出五种尺寸，至今没有「改一处、全套跟着变」。"),
    ("复盘区的结论回不到起草区",
     "数据看板和选题库是两张互不相认的表，上一期的结论下一期用不上。"),
]


def band(name, *, fill, pad, gap, children, align="start"):
    """一个通栏区块。fill 决定它是不是一块有颜色的带 —— 结构容器不写 fill。"""
    node = frame(ids, name, width="fill_container", height="fit_content",
                 layout="vertical", padding=pad, gap=gap, alignItems=align,
                 fill=fill)
    node["children"] = children
    return node


def col(name, children, *, gap=16, width="fill_container", align="start",
        height="fit_content", **props):
    node = frame(ids, name, width=width, height=height, layout="vertical",
                 gap=gap, alignItems=align, fill=[], **props)
    node["children"] = children
    return node


def row(name, children, *, gap=24, align="center", width="fill_container",
        **props):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="horizontal", gap=gap, alignItems=align, fill=[],
                 **props)
    node["children"] = children
    return node


def chip(label, *, bg, fg, size=24):
    node = frame(ids, "胶囊", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[10, 22], cornerRadius=999,
                 alignItems="center", justifyContent="center", fill=solid(bg))
    node["children"] = [
        text(ids, "胶囊文字", label, size, 600, fg, family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
    ]
    return node


def section_head(title, note):
    return col("区块头", [
        rect(ids, "强调短线", width=72, height=8, cornerRadius=999,
             fill=solid("$c-accent")),
        text(ids, "区块标题", title, 46, 700, "$c-ink", family=CJK,
             line_height=LH_HEAD),
        text(ids, "区块说明", note, 27, 400, "$c-muted", family=CJK,
             line_height=LH_BODY),
    ], gap=16)


# ------------------------------------------------------------------ 01 页头
def header():
    return band("01 页头", fill=solid("$c-band"), pad=[76, EDGE, 68, EDGE],
                gap=26, children=[
        chip("行业地图 · 四个位置", bg="$c-accent", fg="$c-surface"),
        text(ids, "主标题", "这条链上\n只有四个位置", 76, 700, "$c-surface",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.4),
        text(ids, "副标题", "每格三个位点，最后一节写还空着的两处。",
             28, 400, "$c-band-muted", family=CJK, line_height=LH_BODY),
    ])


# ------------------------------------------------------------------ 02 图例
def legend():
    """四档明度的图例。分区身份从不只靠颜色 —— 编号和名称同时给。"""
    items = []
    for number, name, _, _, color in ZONES:
        items.append(row("图例项", [
            rect(ids, "图例色块", width=20, height=20, cornerRadius=6,
                 fill=solid(color)),
            text(ids, "图例编号", number, 22, 700, "$c-muted", family=NUM,
                 width="fit_content", growth="auto", line_height=1.4),
            text(ids, "图例名", name, 24, 500, "$c-ink", family=CJK,
                 width="fit_content", growth="auto", line_height=1.4),
        ], gap=10, width="fit_content"))
    return row("图例", items, gap=32)


# ------------------------------------------------------------------ 03 分区
def zone_card(number, name, purpose, spots, color):
    spot_rows = []
    for spot in spots:
        spot_rows.append(row("位点", [
            rect(ids, "位点点", width=10, height=10, cornerRadius=3,
                 fill=solid(color)),
            text(ids, "位点名", spot, 24, 500, "$c-ink", family=CJK,
                 line_height=1.5),
        ], gap=12))

    card = col(f"分区 {number}", [
        rect(ids, "分区色条", width="fill_container", height=8,
             cornerRadius=999, fill=solid(color)),
        row("分区头", [
            text(ids, "分区编号", number, 24, 700, "$c-muted", family=NUM,
                 width="fit_content", growth="auto", line_height=1.4),
            text(ids, "分区名", name, 32, 700, "$c-ink", family=CJK,
                 width="fit_content", growth="auto", line_height=LH_HEAD),
        ], gap=12),
        text(ids, "分区说明", purpose, 25, 400, "$c-muted", family=CJK,
             line_height=LH_BODY),
        col("位点列表", spot_rows, gap=10),
    ], gap=16, padding=[26, 28, 28, 28], cornerRadius=20)
    card["fill"] = solid("$c-surface")
    card["stroke"] = {"thickness": 2, "fill": solid("$c-border")}
    card["height"] = "fill_container"
    return card


def zones():
    grid_rows = []
    for start in (0, 2):
        pair = [zone_card(*entry) for entry in ZONES[start:start + 2]]
        grid_rows.append(row(f"分区行 {start // 2 + 1}", pair, gap=ZONE_GAP,
                             align="stretch"))
    return band("03 分区", fill=[], pad=[64, EDGE, 0, EDGE], gap=30,
                children=[
        section_head("四个格子，按先后排",
                     "编号即先后：从起草到复盘。格子之间不比大小，只分位置。"),
        legend(),
        col("分区阵列", grid_rows, gap=ZONE_GAP),
    ])


# ------------------------------------------------------------------ 04 空位
def gaps():
    items = []
    for title, detail in GAPS:
        items.append(row("空位项", [
            icon_font(ids, "标记", "scan-search", 30, "$c-accent-deep"),
            col("空位文案", [
                text(ids, "空位标题", title, 28, 600, "$c-ink", family=CJK,
                     line_height=1.4),
                text(ids, "空位说明", detail, 26, 400, "$c-muted", family=CJK,
                     line_height=LH_BODY),
            ], gap=8),
        ], gap=16, align="start"))
    panel = col("空位面板", items, gap=24, padding=[36, 34], cornerRadius=22)
    panel["fill"] = solid("$c-accent-soft")
    return band("04 空位", fill=[], pad=[64, EDGE, 64, EDGE], gap=32,
                children=[
        section_head("还空着的两处",
                     "都在两格之间 —— 单格里的位点已经很挤，缝隙里还没人。"),
        panel,
    ])


# ------------------------------------------------------------------ 05 页脚
def footer():
    return band("05 页脚", fill=solid("$c-band"), pad=[44, EDGE], gap=12,
                children=[
        text(ids, "口径", "分区按工作流环节划，同一产品跨两格时归它收入占多的那格。",
             24, 400, "$c-band-muted", family=CJK, line_height=1.6),
        row("署名行", [
            text(ids, "账号名", "@ 你的账号名", 26, 600, "$c-surface",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
            text(ids, "更新说明", "每半年重画一次这张图", 24, 400,
                 "$c-band-muted", family=CJK, width="fit_content",
                 growth="auto", line_height=1.4),
        ], gap=16, width="fill_container"),
    ])


def build():
    page = frame(ids, "行业地图长图", width=W, height=ROOT_H,
                 layout="vertical", gap=0, fill=solid("$c-bg"),
                 clipContent=True)
    page["children"] = [header(), zones(), gaps(), footer()]
    page["x"], page["y"] = 0, 0
    return [page]


# 对比度（WCAG 相对亮度比，op-design-lint 的门槛是 2.0；数值实测）：
#   c-ink         on c-bg          11.16   c-muted      on c-bg          5.57
#   c-ink         on c-surface     16.62   c-muted      on c-surface     8.29
#   c-surface     on c-band        17.14   c-band-muted on c-band        6.88
#   c-accent-deep on c-surface      8.08   c-accent     on c-surface     5.43
#   c-accent-deep on c-accent-soft  6.96   c-ink        on c-accent-soft 14.31
#   c-surface     on c-accent       5.43   c-muted      on c-accent-soft 5.74
#   c-zone-1 on c-surface 11.06        c-zone-2 on c-surface 5.43
#   c-zone-3 on c-surface  3.49        c-zone-4 on c-surface 2.29
#   c-border on c-surface  1.87
# 承载文字的最低一对是 5.43。四档分区色只画色条与色点（非文字图形），最
# 浅一档 2.29 是这条明度梯的下限 —— 再浅第四格的色条在白卡上就断了。
# c-border 是卡片描边，在中明度底上卡片本身已经靠 16.62 的明度差浮起来，
# 描边只是收边。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "行业地图长图")
