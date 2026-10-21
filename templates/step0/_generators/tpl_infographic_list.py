#!/usr/bin/env python3
"""pitfall-list-infographic.op — 避坑清单信息长图（1080×N 竖版）

信息图这一档的第三张，和另外两张凑成一组阅读动机：

  - data-report-infographic ——「看数字」，靠图表；冷调青绿。
  - steps-flow-infographic ——「学方法」，靠序号流程；暖调柑橘。
  - 这一张 ——「抄清单」，靠排行；**无彩**。

三张共用同一套信息图语法（深色页头 → 若干区块 → 深色收尾），差异做在色温
与主元素上：前两张一冷一暖，这张干脆没有色相 —— 三张并排陈列时，一眼就能
按「绿 / 橙 / 灰」把它们分开。

版式取小红书分类学的 list（排行/枚举，4-7 条），内容取避坑向：六条按踩坑
频率排序，每条给「错在哪」和「改成这样」，末尾附一张四行自检表。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：不从内容采色 —— 这一张的内容本身没有色彩记忆（「做图常犯的
    错」不是一个有颜色的东西）。硬给它一个色相就是从模型先验里抽签。
  - **收敛**：0 个有彩色 + 一条中性明度序列 L 0.18 / 0.44 / 0.64 / 0.88 /
    0.92 / 0.955 / 1.0，chroma 全部 ≤0.004（微暖，避免冷灰的塑料感）。
  - **论证**：避坑清单承载的是**二值信息** —— 这样是错的、那样是对的。二
    值信息用二值明度（近黑 / 白）表达最直接；再引入一个色相，读者就得先学
    一层「红代表什么、绿代表什么」的编码，然后才开始读内容。层级改由**明
    度块、字重和超大序号**承担，它们不需要图例。

### 负约束（本模板明令不做的事）

  - **不用任何有彩色。** 一个都不给 —— 这是本模板的核心约束，破了它整张
    图的论证就不成立了。
  - 不用红绿对错配色。避坑清单最容易滑向「错误区红、正确区绿」，那是另一
    档（对比图）的语言，而且会把无彩这条约束一次性废掉。
  - 不用蓝紫渐变、霓虹线条、复杂背景纹理（廉价 AI 科技风的三件套）。
  - 不用 emoji 当图标，不用装饰性插画，不用伪 3D。
  - 不用阴影。卡片边界由 1px hairline 或明度反差给。
  - 一条只讲一个坑：说明超过两行就该拆成两条，不缩字号。
  - 不写 AI 套话（「赋能 / 一站式 / 打造闭环」），每条都写成能照着改的动作。

硬契约：
  - 内容距边缘 ≥80px（这里 80）
  - 配色全部走 color_vars，全是中性明度序列，没有「主色」可换
  - 正文与背景对比度 ≥2.0（本配色最低一对见文件末尾注释）
  - **CJK 行高比西文全线高 0.2**：页头大标题 1.2，区块标题 1.3，正文 1.7
  - **CJK 负字距不超过 -0.02em**；只有西文数字沿用西文 display 的收紧
  - 汉字走 Noto Sans SC，数字与拉丁走 Inter —— 等价于中文排印规范里
    「西文在前、中文在后」的 fallback 链，只是在 .op 里按节点写死
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
    "c-bg":        "#F1F0EE",
    "c-card":      "#FFFFFF",
    "c-panel":     "#E5E4E2",
    "c-line":      "#D8D7D5",
    "c-ink":       "#121210",
    "c-muted":     "#535250",
    "c-faint":     "#8D8C89",
    # 反白页面上的次级文字。比 c-faint 亮一档 —— 同一个灰压在近黑上会掉到
    # 4.58，压在白上又太重，深浅两套次级色是必要的。
    "c-inv-muted": "#A5A4A2",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W = 1080
EDGE = 80
INNER = W - EDGE * 2

# CJK 行高阶梯（西文 +0.2）。全篇只用这三档。
LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.3, 1.7

# 量出来的根高（做法同另外两张：根设 fit_content 渲一次读 PNG 高度）。
ROOT_H = 3226

# (排名, 坑, 错在哪, 改成这样)。前三名走反白重灾区块，后三名走白卡 ——
# 排行本身就是层级，用明度把它画出来，不用给它配一个颜色。
PITFALLS = [
    ("01", "一页想讲完所有事",
     "把三段话塞进一张图，字缩到 20px，手机上谁都不会读。",
     "一页只讲一个点，讲不完就加一页。"),
    ("02", "标题写成了说明书",
     "「关于如何提升时间管理效率的方法」——没人会为它停下。",
     "先写结论那句话，再删到十个字以内。"),
    ("03", "颜色一次上五个",
     "每个模块一个颜色，看起来热闹，实际上等于一个都没强调。",
     "一个强调色，只给最重要的那一处。"),
    ("04", "正文和背景对比不够",
     "浅灰字压在浅灰底上，设计稿里好看，发出去在阳光下看不见。",
     "正文对比度至少 4.5:1，先量再发。"),
    ("05", "装饰压在文字上",
     "光效、纹理、大图铺满整页，标题只好往上叠，读起来费劲。",
     "给标题留一块干净的高对比区，装饰退到它外面。"),
    ("06", "每页都换一套版式",
     "六页像六个模板拼起来的，读者每翻一页都要重新找重点在哪。",
     "边距、字体、页码位置锁死，只让主视觉变。"),
]

# 自检表。四行，够快能在发布前跑一遍。
CHECKS = [
    "标题十个字以内，且是结论不是主题",
    "每页正文不超过四行",
    "强调色只出现在一处",
    "手机上退后一臂距离仍能读清正文",
]


def band(name, *, fill, pad, gap, children, align="start"):
    """一个通栏区块。fill 决定它是不是一块有颜色的带 —— 结构容器不写 fill。"""
    node = frame(ids, name, width="fill_container", height="fit_content",
                 layout="vertical", padding=pad, gap=gap, alignItems=align,
                 fill=fill)
    node["children"] = children
    return node


def col(name, children, *, gap=16, width="fill_container", align="start",
        **props):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="vertical", gap=gap, alignItems=align, fill=[],
                 **props)
    node["children"] = children
    return node


def row(name, children, *, gap=24, align="center", width="fill_container",
        **props):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="horizontal", gap=gap, alignItems=align, fill=[],
                 **props)
    node["children"] = children
    return node


def tag(label, *, bg, fg):
    node = frame(ids, "标签", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[10, 22], cornerRadius=999,
                 alignItems="center", justifyContent="center", fill=solid(bg))
    node["children"] = [
        text(ids, "标签文字", label, 24, 600, fg, family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
    ]
    return node


def section_head(title, note):
    """区块标题。一条近黑短线 + 标题 + 一句说明，三处一模一样。"""
    return col("区块头", [
        rect(ids, "强调短线", width=72, height=8, fill=solid("$c-ink")),
        text(ids, "区块标题", title, 46, 700, "$c-ink", family=CJK,
             line_height=LH_HEAD),
        text(ids, "区块说明", note, 27, 400, "$c-muted", family=CJK,
             line_height=LH_BODY),
    ], gap=16)


# ------------------------------------------------------------------ 01 页头
def header():
    return band("01 页头", fill=solid("$c-ink"), pad=[76, EDGE, 68, EDGE],
                gap=26, children=[
        tag("避坑清单 · 做图篇", bg="$c-card", fg="$c-ink"),
        text(ids, "主标题", "做图最常踩的\n六个坑", 76, 700, "$c-card",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.4),
        text(ids, "副标题", "按踩的人数从多到少排，每条都给一句能照着改的话。",
             28, 400, "$c-inv-muted", family=CJK, line_height=LH_BODY),
    ])


# ------------------------------------------------------------------ 02 排行
def pitfall_item(rank, title, wrong, fix, heavy):
    """一条坑。前三名反白（重灾区），后三名白卡 —— 排名即明度。"""
    ink = "$c-card" if heavy else "$c-ink"
    muted = "$c-inv-muted" if heavy else "$c-muted"

    head = row("坑头", [
        # 排名是信息不是装饰：深浅两种卡上都用 c-faint。第一版给白卡用了
        # c-line（1.06:1），那个数字实际上是看不见的。
        text(ids, "排名", rank, 56, 700, "$c-faint", family=NUM, width=104,
             line_height=1.0, spacing=-2),
        text(ids, "坑标题", title, 38, 700, ink, family=CJK,
             line_height=LH_HEAD),
    ], gap=20, align="center")

    fix_row = row("改法", [
        icon_font(ids, "箭头", "arrow-right", 26, ink),
        text(ids, "改法文字", fix, 27, 600, ink, family=CJK,
             line_height=LH_BODY),
    ], gap=14, align="start")

    item = col("坑", [
        head,
        text(ids, "错在哪", wrong, 27, 400, muted, family=CJK,
             line_height=LH_BODY),
        rect(ids, "改法上线", width="fill_container", height=2,
             fill=solid("$c-faint" if heavy else "$c-line")),
        fix_row,
    ], gap=18, padding=[32, 32])
    item["fill"] = solid("$c-ink" if heavy else "$c-card")
    if not heavy:
        item["stroke"] = {"thickness": 2, "fill": solid("$c-line")}
    return item


def pitfalls():
    items = [pitfall_item(rank, title, wrong, fix, index < 3)
             for index, (rank, title, wrong, fix) in enumerate(PITFALLS)]
    return band("02 排行", fill=[], pad=[64, EDGE, 0, EDGE], gap=32, children=[
        section_head("从最常踩的那个说起",
                     "前三条是重灾区，单这三条就能解决大半问题。"),
        col("排行列表", items, gap=18),
    ])


# ------------------------------------------------------------------ 03 自检
def checks():
    items = []
    for line in CHECKS:
        box = frame(ids, "方框", width=40, height=40, layout="horizontal",
                    alignItems="center", justifyContent="center",
                    fill=solid("$c-ink"))
        box["children"] = [icon_font(ids, "勾", "check", 24, "$c-card")]
        items.append(row("自检项", [
            box,
            text(ids, "自检文字", line, 28, 500, "$c-ink", family=CJK,
                 line_height=LH_BODY),
        ], gap=18, align="start"))
    panel = col("自检面板", items, gap=20, padding=[36, 34])
    panel["fill"] = solid("$c-panel")
    return band("03 自检", fill=[], pad=[64, EDGE, 72, EDGE], gap=28,
                children=[
        section_head("发之前跑一遍", "四行，一分钟。过不了就别急着发。"),
        panel,
    ])


# ------------------------------------------------------------------ 04 页脚
def footer():
    return band("04 页脚", fill=solid("$c-ink"), pad=[44, EDGE], gap=12,
                children=[
        text(ids, "结语", "第七个坑是「觉得自己不会踩前六个」。", 30, 600,
             "$c-card", family=CJK, line_height=LH_HEAD),
        row("署名行", [
            text(ids, "账号名", "@ 你的账号名", 26, 600, "$c-card",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
            text(ids, "更新说明", "每周一张能照着改的图", 24, 400,
                 "$c-inv-muted", family=CJK, width="fit_content",
                 growth="auto", line_height=1.4),
        ], gap=16, width="fill_container"),
    ])


def build():
    page = frame(ids, "避坑清单长图", width=W, height=ROOT_H, layout="vertical",
                 gap=0, fill=solid("$c-bg"), clipContent=True)
    page["children"] = [header(), pitfalls(), checks(), footer()]
    page["x"], page["y"] = 0, 0
    return [page]


# 对比度（WCAG 相对亮度比，op-design-lint 的门槛是 2.0；数值实测）：
#   c-ink       on c-bg     16.45    c-muted on c-bg     7.37
#   c-ink       on c-card   18.58    c-muted on c-card   8.32
#   c-ink       on c-panel  15.02    c-muted on c-panel  6.73
#   c-card      on c-ink    18.58    c-inv-muted on c-ink 8.46
#   c-faint     on c-ink     4.58    c-faint on c-bg     3.59
# 承载正文的最低一对是 6.73。c-faint 只用在反白块里的排名数字与那条细线上
# ——数字是 56px 的西文粗体（AA 大字门槛 3.0），细线是非文字图形，两者都
# 有余量。整张图没有有彩色，所以「换主色」这件事不存在：要改只能整体调明
# 度序列，而序列本身就是层级。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "避坑清单信息长图")
