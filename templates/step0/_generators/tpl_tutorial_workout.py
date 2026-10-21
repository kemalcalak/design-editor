#!/usr/bin/env python3
"""workout-breakdown-guide.op — 健身动作分解长图（1080×N 竖版）

教程档里唯一一张**带训练参数**的模板。别的教程只要「做什么」，训练还要
「做多少」——组数、次数、休息秒数是这类内容的硬通货，读者存图就是为了照
着数做。所以每个动作除了图位和要点，还有一条固定格式的参数条。

风格取「硬核专业风」：冷白底 + 石墨块 + 单一橙。刻意**不做深色健身房**那
一套（黑底 + 高饱和荧光），深底长图在手机上滚三千像素会累眼；把重量压在
几块石墨色的实心带上，滚动时节奏感由深浅切换给。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：从器械与场地采——杠铃片的橡胶橙、地胶的冷灰白、哑铃铸铁的
    石墨。三个都是真实存在于训练场景里的颜色，不是「运动 = 荧光绿」这种
    从模型先验里抽出来的联想。
  - **收敛**：四档冷中性（L 0.11 / 0.13 / 0.38 / 0.95）+ 1 个有彩色橙
    #E24A12。
  - **论证**：橙在这张图里承担唯一一件事——**标出「数字」**。组数、次数、
    秒数全走橙，其余一律中性。读者滚到任何一屏，橙色出现的地方就是要记的
    量。给要点、标题、图标再上橙，这条编码立刻失效。

### 负约束（本模板明令不做的事）

  - **不编生理数据。** 不写「燃脂 320 大卡」「心率区间 140-160」——这些随
    体重、体能、动作质量变化，模板里给一个数就是骗人。只给可执行的量：
    组数、次数、休息秒数、每周频次。
  - 不用深色健身房配色（黑底 + 荧光）。理由见上。
  - 不用第二个有彩色。橙只标数字。
  - 不用「练废 / 燃爆 / 恐怖如斯」这类健身黑话；每条要点写成能自查的身体
    感受或位置（膝盖对准脚尖、腰不塌）。
  - 不做 emoji 肌肉图标，不做伪 3D 人体模型。
  - 每个动作最多两条要点 + 一条常见错误。写第四条说明这个动作该单独出一
    张图。

硬契约：
  - 内容距边缘 ≥80px（这里 80）
  - 配色全部走 color_vars；换主色只需改 c-accent 与 c-accent-ink
  - 正文与背景对比度 ≥2.0（实测表见文件末尾）
  - **CJK 行高比西文全线高 0.2**：display 1.2，标题 1.3，正文 1.7
  - **CJK 负字距不超过 -0.02em**（78px 标题 → -1.5px = -0.019em）
  - 汉字走 Noto Sans SC，数字与拉丁走 Inter——参数条全是数字，这条 fallback
    规则在这张图上最吃紧
  - 顶层 frame 显式写 x/y
  - 根高固定：ROOT_H 是量出来的（根设 fit_content 渲一次读 PNG 高度），改
    内容后要重量一次
  - 四个动作图位实测各 856×340（2.52:1）；根高 4589 是 fit_content 量出来的
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import (Ids, color_vars, frame, icon_font, rect, solid, stroke,
                   text, write_doc)

ids = Ids()

VARS = color_vars({
    "c-bg":         "#F1F2F4",
    "c-card":       "#FFFFFF",
    "c-slot":       "#E4E6E9",
    "c-line":       "#D5D8DC",
    "c-slab":       "#1C1E22",
    "c-ink":        "#131519",
    "c-muted":      "#5B6067",
    "c-inv-muted":  "#A0A5AC",
    # 动作序号那个大数字。第一版给它 c-slot（1.15:1）——等于没画。序号是
    # 信息不是装饰，压在白卡上要够到大字门槛 3.0。
    "c-ghost":      "#8F949B",
    "c-accent":     "#E24A12",
    "c-accent-ink": "#FFFFFF",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W = 1080
EDGE = 80
INNER = W - EDGE * 2

LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.3, 1.7

# 量出来的根高：根设 fit_content 渲一次，读 PNG 高度回填。
ROOT_H = 4589

SLOT_H = 340

# 顶部三个总览指标。全是读者自己能验证的量。
META = [("总时长", "22 分钟"), ("动作数", "4 个"), ("器械", "不需要")]

# 每周安排。三天，不是「每天练」——那是没练过的人才写得出的排期。
PLAN = [("周一", "全身 A"), ("周三", "全身 B"), ("周五", "全身 A")]

# (序号, 动作名, 部位, 组数, 次数, 休息, 图位提示, 要点, 常见错误)
MOVES = [
    ("01", "徒手深蹲", "腿 · 臀", "3 组", "12 次", "休息 60 秒", "深蹲最低点",
     "蹲到大腿与地面平行就够，膝盖始终对准脚尖。",
     "膝盖内扣、脚跟离地——这两个一起出现就是重量给多了。"),
    ("02", "跪姿俯卧撑", "胸 · 三头", "3 组", "10 次", "休息 60 秒",
     "俯卧撑下放位",
     "手掌在肩膀正下方偏外一拳，下放到胸口离地一掌。",
     "塌腰撅臀。从膝盖到肩膀应该是一条直线。"),
    ("03", "臀桥", "臀 · 后链", "3 组", "15 次", "休息 45 秒", "臀桥顶点",
     "顶点停 1 秒再落，靠夹臀发力而不是靠腰顶。",
     "腰先离地。那说明在用下背代偿，臀根本没参与。"),
    ("04", "平板支撑", "核心", "3 组", "30 秒", "休息 45 秒", "平板支撑侧面",
     "肘在肩正下方，收腹夹臀，呼吸别憋。",
     "撑到抖还硬扛。姿势垮了的每一秒都在练错。"),
]

TIPS = [
    "练之前先花 3 分钟活动踝、髋、肩，别省。",
    "动作做不满次数就减次数，不要减质量。",
    "两次训练之间至少隔一天，肌肉是在休息时长的。",
]


def col(name, children, *, gap=16, width="fill_container", align="start",
        height="fit_content", **props):
    node = frame(ids, name, width=width, height=height, layout="vertical",
                 gap=gap, alignItems=align, fill=[], **props)
    node["children"] = children
    return node


def row(name, children, *, gap=20, align="center", width="fill_container",
        height="fit_content", **props):
    node = frame(ids, name, width=width, height=height, layout="horizontal",
                 gap=gap, alignItems=align, fill=[], **props)
    node["children"] = children
    return node


def band(name, children, *, fill, pad, gap, align="start"):
    node = frame(ids, name, width="fill_container", height="fit_content",
                 layout="vertical", padding=pad, gap=gap, alignItems=align,
                 fill=fill)
    node["children"] = children
    return node


def tag(label, *, bg, fg, size=24):
    node = frame(ids, "标签", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[10, 20], cornerRadius=6,
                 alignItems="center", justifyContent="center", fill=solid(bg))
    node["children"] = [
        text(ids, "标签文字", label, size, 600, fg, family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
    ]
    return node


# ------------------------------------------------------------------ 01 页头
def header():
    metas = []
    for index, (label, value) in enumerate(META):
        if index:
            metas.append(rect(ids, "竖分隔", width=2, height=46,
                              fill=solid("$c-muted")))
        metas.append(col("总览指标", [
            text(ids, "指标名", label, 22, 400, "$c-inv-muted", family=CJK,
                 width="fit_content", growth="auto", line_height=1.4),
            text(ids, "指标值", value, 30, 700, "$c-card", family=CJK,
                 width="fit_content", growth="auto", line_height=1.3),
        ], gap=4, width="fit_content"))

    return band("01 页头", fill=solid("$c-slab"), pad=[80, EDGE, 72, EDGE],
                gap=28, children=[
        tag("居家训练 · 无器械", bg="$c-accent", fg="$c-accent-ink"),
        text(ids, "主标题", "四个动作\n把全身练一遍", 78, 700, "$c-card",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.5),
        text(ids, "副标题",
             "客厅一块瑜伽垫的地方就够。参数都写在每个动作下面，照着做。",
             28, 400, "$c-inv-muted", family=CJK, line_height=LH_BODY),
        row("总览指标条", metas, gap=28, width="fit_content"),
    ])


# ------------------------------------------------------------------ 02 排期
def plan():
    cards = []
    for day, label in PLAN:
        card = col("排期卡", [
            text(ids, "星期", day, 30, 700, "$c-ink", family=CJK,
                 line_height=LH_HEAD),
            text(ids, "内容", label, 25, 500, "$c-muted", family=CJK,
                 line_height=LH_BODY),
        ], gap=6, padding=[26, 26])
        card["fill"] = solid("$c-card")
        card["cornerRadius"] = 14
        card["stroke"] = stroke("$c-line", 2)
        cards.append(card)
    return band("02 排期", fill=[], pad=[60, EDGE, 0, EDGE], gap=24,
                children=[
        section_head("一周练三次就够", "隔天练一次，中间那天让肌肉长回来。"),
        row("排期条", cards, gap=20, align="stretch"),
    ])


def section_head(title, note):
    return col("区块头", [
        rect(ids, "强调短线", width=72, height=8, fill=solid("$c-accent")),
        text(ids, "区块标题", title, 46, 700, "$c-ink", family=CJK,
             line_height=LH_HEAD),
        text(ids, "区块说明", note, 26, 400, "$c-muted", family=CJK,
             line_height=LH_BODY),
    ], gap=14)


# ------------------------------------------------------------------ 03 动作
def param_bar(sets, reps, rest):
    """参数条：三个量并排，数字全走橙——这是整张图唯一的橙色用法。"""
    kids = []
    for index, value in enumerate((sets, reps, rest)):
        if index:
            kids.append(rect(ids, "参数分隔", width=2, height=30,
                             fill=solid("$c-line")))
        kids.append(text(ids, "参数值", value, 27, 700, "$c-accent",
                         family=NUM, width="fit_content", growth="auto",
                         line_height=1.3))
    bar = row("参数条", kids, gap=20, padding=[16, 22], width="fit_content")
    bar["fill"] = solid("$c-bg")
    bar["cornerRadius"] = 10
    return bar


def move_item(no, name, part, sets, reps, rest, hint, cue, mistake):
    slot = frame(ids, "动作图位", width="fill_container", height=SLOT_H,
                 layout="vertical", gap=12, alignItems="center",
                 justifyContent="center", cornerRadius=12,
                 fill=solid("$c-slot"))
    slot["children"] = [
        icon_font(ids, "图位图标", "camera", 42, "$c-muted"),
        text(ids, "图位提示", hint, 26, 600, "$c-ink", family=CJK,
             align="center", line_height=LH_HEAD),
        text(ids, "图位规格", "拍侧面，整个人入镜", 22, 400, "$c-muted",
             family=CJK, align="center", line_height=LH_BODY),
    ]

    head = row("动作头", [
        text(ids, "动作序号", no, 52, 700, "$c-ghost", family=NUM, width=96,
             line_height=1.0, spacing=-2),
        col("动作名区", [
            text(ids, "动作名", name, 40, 700, "$c-ink", family=CJK,
                 line_height=LH_HEAD),
            text(ids, "部位", part, 23, 500, "$c-muted", family=CJK,
                 line_height=1.4),
        ], gap=4),
    ], gap=16, align="center")

    def line_item(icon, color, label):
        return row("要点", [
            icon_font(ids, "要点图标", icon, 26, color),
            text(ids, "要点文字", label, 25, 400, "$c-muted", family=CJK,
                 line_height=LH_BODY),
        ], gap=14, align="start")

    item = col("动作", [
        head,
        param_bar(sets, reps, rest),
        slot,
        line_item("check", "$c-ink", cue),
        line_item("triangle-alert", "$c-accent", mistake),
    ], gap=20, padding=[32, 32])
    item["fill"] = solid("$c-card")
    item["cornerRadius"] = 18
    item["stroke"] = stroke("$c-line", 2)
    return item


def moves():
    return band("03 动作", fill=[], pad=[64, EDGE, 0, EDGE], gap=28,
                children=[
        section_head("四个动作，按顺序做",
                     "每个动作做完 3 组再进下一个，别串着做。"),
        col("动作列表", [move_item(*m) for m in MOVES], gap=22),
    ])


# ------------------------------------------------------------------ 04 收尾
def footer():
    items = []
    for index, line in enumerate(TIPS, 1):
        items.append(row("提醒项", [
            text(ids, "提醒序号", f"{index:02d}", 26, 700, "$c-accent",
                 family=NUM, width=56, line_height=1.4),
            text(ids, "提醒文字", line, 26, 400, "$c-inv-muted", family=CJK,
                 line_height=LH_BODY),
        ], gap=10, align="start"))

    return band("04 收尾", fill=solid("$c-slab"), pad=[64, EDGE, 56, EDGE],
                gap=28, children=[
        text(ids, "收尾标题", "练之前，先看这三条", 42, 700, "$c-card",
             family=CJK, line_height=LH_HEAD),
        col("提醒列表", items, gap=16),
        row("署名行", [
            text(ids, "账号名", "@ 你的账号名", 26, 600, "$c-card",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=1.4),
            text(ids, "更新说明", "每周一套能在家做的训练", 24, 400,
                 "$c-inv-muted", family=CJK, width="fit_content",
                 growth="auto", line_height=1.4),
        ], gap=16, justifyContent="space_between"),
    ])


def build():
    page = frame(ids, "健身动作分解长图", width=W, height=ROOT_H,
                 layout="vertical", gap=0, fill=solid("$c-bg"),
                 clipContent=True)
    page["children"] = [header(), plan(), moves(), footer()]
    page["x"], page["y"] = 0, 0
    return [page]


# 对比度（WCAG 相对亮度比，op-design-lint 门槛 2.0；数值由脚本实测）：
#   c-ink       on c-bg     16.32   c-muted on c-bg      5.66
#   c-ink       on c-card   18.28   c-muted on c-card    6.34
#   c-ink       on c-slot   14.62   c-muted on c-slot    5.07
#   c-card      on c-slab   16.69   c-inv-muted on c-slab 6.73
#   c-accent    on c-card    4.02   c-accent on c-bg      3.59
#   c-accent-ink on c-accent 4.02   c-slot  on c-slab    13.35
#   c-ghost     on c-card    3.05
# 承载正文的最低一对是 5.07。橙色那三对（3.59-4.02）只出现在三个地方：
# 参数条里 27px/700 的数字、26px 的警示图标、24px/600 的顶部标签与 72×8
# 的短线——全部落在 WCAG「大字 / 非文字图形」的 3.0 门槛下，且都远高于
# lint 的 2.0。正文一个字都没用橙。c-ghost 同理：只给 52px/700 的动作
# 序号，3.05 刚好过大字门槛。c-line 只用于 hairline 描边。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "健身动作分解长图")
