#!/usr/bin/env python3
"""app-onboarding-triptych.op — App 新手引导三屏（1080×1440，3:4 单卡）

教程档里唯一一张**给做产品的人用**的模板：不是教读者操作，是教读者怎么写
自己 App 的引导页。所以主视觉是三台并排的手机，屏幕里是空的图位——用户把
自己的三张引导图拖进去，下面配文案，一张就能拿去评审或发出来。

风格取「极简产品发布会风」：纯白页面、单一电光蓝、器物居中。九张教程里只
有这一张用**纯白**背景，其余都是有色纸底——发布会那套语言的地基就是白。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：从产品发布会的物理现场采——白背板、深色器物、一束单色射灯。
  - **收敛**：四档冷中性（白 1.0 / 面 0.95 / 线 0.90 / 墨 0.08）+ 1 个有彩
    色电光蓝 #2F5BFF。
  - **论证**：三台手机并排最大的风险是**三屏一样重**，读者不知道先看哪一
    台。这里用蓝只做一件事：标出「第 1 屏」的序号方块与那条进度点——把顺序
    画出来。屏幕本身、机身、文案全是中性，颜色不参与竞争。纯白而不是浅灰
    底：手机机身用的是浅灰描边，底再灰一档，机身就浮不出来了。

### 负约束（本模板明令不做的事）

  - **不画伪 UI。** 手机屏里只有图位，不虚构状态栏图标、假按钮、假列表。
    虚构的界面细节在 296px 宽的屏里必然不可读，而且会和用户真正拖进来的
    截图打架。
  - 不做 3D 透视机身、不做倾斜摆位、不做屏幕反光高光——那是渲染图的语言，
    不是版式的语言。
  - 不用第二个有彩色。蓝只标顺序。
  - 不用渐变机身、不用外发光、不用圆点阵背景。
  - 三屏封顶。第四屏说明引导写长了——引导页的转化在前三屏就决定了。
  - 每屏文案：一句标题（不超过 10 字）+ 一句说明（不超过 22 字）。写不下
    的内容属于产品文档，不属于引导页。

硬契约：
  - 内容距边缘 ≥72px（这里 72）
  - 配色全部走 color_vars；换主色只需改 c-accent 与 c-accent-ink
  - 正文与背景对比度 ≥2.0（实测表见文件末尾）
  - **CJK 行高比西文全线高 0.2**：display 1.2，标题 1.35，正文 1.7
  - **CJK 负字距不超过 -0.02em**（62px 标题 → -1.2px = -0.019em）
  - 汉字走 Noto Sans SC，数字与拉丁走 Inter
  - 顶层 frame 显式写 x/y
  - 三个屏幕图位实测各 268×396（0.68:1）；高度由页面余量反解，见 SCREEN_H
  - 同一行的三台手机必须同为 fill_container，否则先声明的那台会吃掉整行
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import (Ids, color_vars, frame, icon_font, rect, solid, stroke,
                   text, write_doc)

ids = Ids()

VARS = color_vars({
    "c-bg":         "#FFFFFF",
    "c-surface":    "#F3F4F7",
    "c-slot":       "#E9EBF0",
    "c-line":       "#DDE0E7",
    "c-ink":        "#14161A",
    "c-muted":      "#666D79",
    "c-accent":     "#2F5BFF",
    "c-accent-ink": "#FFFFFF",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W, H = 1080, 1440
EDGE = 72

LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.35, 1.7

PAGE_GAP = 28

# 屏幕图位高度写死：三台手机必须等高，交给 flex 分配余量会被各自文案的
# 换行数拉开（菜谱那张踩过一次）。396 是**量出来**的，不是算出来的：把它
# 设成任意值渲一遍，读 snapshot_layout 里四个块的高度和，跟 1440 的差额
# 直接回补到这个数上（迭代两次就收敛）。改页头或规矩面板任何一行都要重量
# ——超了会被 clipContent 悄悄裁掉，页脚先没。
SCREEN_H = 396

# (序号, 一句标题 ≤10 字, 一句说明 ≤22 字, 图位提示)
SCREENS = [
    ("1", "先说清你是谁", "一句话讲明白这个 App 解决什么问题。", "第 1 屏截图"),
    ("2", "再给一个动作", "只留一个按钮，别让人挑。", "第 2 屏截图"),
    ("3", "最后请求权限", "在真正要用到的那一刻才要，通过率高得多。",
     "第 3 屏截图"),
]

RULES = [
    ("跳过按钮一定要留", "藏起跳过只会让人卸载，不会让人读完。"),
    ("三屏共用一套版式", "换插画不换结构，翻页时视线不用重新找。"),
    ("最后一屏别放注册墙", "先让人进去看一眼，再谈账号。"),
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


# ------------------------------------------------------------------ 页头
def header():
    tag = frame(ids, "标签", width="fit_content", height="fit_content",
                layout="horizontal", padding=[10, 20], cornerRadius=999,
                alignItems="center", justifyContent="center",
                fill=solid("$c-surface"))
    tag["children"] = [
        text(ids, "标签文字", "新手引导 · 三屏封顶", 23, 600, "$c-accent",
             family=CJK, width="fit_content", growth="auto", line_height=1.4),
    ]
    return col("页头", [
        tag,
        text(ids, "主标题", "三屏之内\n把你的 App 讲完", 62, 700, "$c-ink",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.2),
        text(ids, "副标题",
             "把自己的三张引导图拖进屏幕里，文案照着下面的格式改。",
             26, 400, "$c-muted", family=CJK, line_height=LH_BODY),
    ], gap=18)


# ------------------------------------------------------------------ 三屏
def phone(no, title, desc, hint, active):
    """一台手机。屏里只有图位——见负约束：不画伪 UI。"""
    slot = frame(ids, "屏幕图位", width="fill_container", height=SCREEN_H,
                 layout="vertical", gap=10, alignItems="center",
                 justifyContent="center", cornerRadius=18,
                 fill=solid("$c-slot"))
    slot["children"] = [
        icon_font(ids, "图位图标", "image", 34, "$c-muted"),
        text(ids, "图位提示", hint, 21, 600, "$c-muted", family=CJK,
             align="center", line_height=1.4),
    ]
    # 听筒条：唯一一处「像手机」的细节，一个 8px 高的圆角块，到此为止。
    notch = rect(ids, "听筒条", width=64, height=8, cornerRadius=4,
                 fill=solid("$c-line"))
    body = col("机身", [notch, slot], gap=14, align="center", padding=[14, 14],
               height="fit_content")
    body["fill"] = solid("$c-bg")
    body["cornerRadius"] = 28
    body["stroke"] = stroke("$c-line", 3)

    badge = frame(ids, "屏序号", width=40, height=40, layout="horizontal",
                  alignItems="center", justifyContent="center",
                  cornerRadius=10,
                  fill=solid("$c-accent" if active else "$c-surface"))
    badge["children"] = [
        text(ids, "序号", no, 21, 700,
             "$c-accent-ink" if active else "$c-muted", family=NUM,
             width="fit_content", growth="auto", line_height=1.0),
    ]

    return col("引导屏", [
        body,
        row("屏标题行", [
            badge,
            text(ids, "屏标题", title, 26, 600, "$c-ink", family=CJK,
                 line_height=LH_HEAD),
        ], gap=12, align="center"),
        text(ids, "屏说明", desc, 21, 400, "$c-muted", family=CJK,
             line_height=LH_BODY),
    ], gap=16, width="fill_container")


def triptych():
    phones = [phone(no, title, desc, hint, index == 0)
              for index, (no, title, desc, hint) in enumerate(SCREENS)]
    return row("三屏", phones, gap=24, align="start")


# ------------------------------------------------------------------ 规矩
def rules():
    items = []
    for title, desc in RULES:
        item = row("规矩项", [
            icon_font(ids, "规矩图标", "check", 24, "$c-accent"),
            col("规矩文案", [
                text(ids, "规矩标题", title, 25, 600, "$c-ink", family=CJK,
                     line_height=LH_HEAD),
                text(ids, "规矩说明", desc, 22, 400, "$c-muted", family=CJK,
                     line_height=LH_BODY),
            ], gap=4),
        ], gap=14, align="start", width="fill_container")
        items.append(item)
    panel = col("规矩面板", items, gap=14, padding=[24, 30])
    panel["fill"] = solid("$c-surface")
    panel["cornerRadius"] = 18
    return panel


# ------------------------------------------------------------------ 页脚
def footer():
    return row("页脚", [
        text(ids, "账号名", "@ 你的账号名", 23, 600, "$c-muted", family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
        text(ids, "用法提示", "浅灰屏幕是图片位，直接把截图拖上去", 21, 400,
             "$c-muted", family=CJK, width="fit_content", growth="auto",
             line_height=1.4),
    ], gap=16, justifyContent="space_between")


def build():
    page = frame(ids, "App 新手引导三屏", width=W, height=H, layout="vertical",
                 padding=[80, EDGE, 72, EDGE], gap=PAGE_GAP, fill=solid("$c-bg"),
                 clipContent=True)
    page["children"] = [header(), triptych(), rules(), footer()]
    page["x"], page["y"] = 0, 0
    return [page]


# 对比度（WCAG 相对亮度比，op-design-lint 门槛 2.0；数值由脚本实测）：
#   c-ink    on c-bg      18.11   c-muted on c-bg      5.21
#   c-ink    on c-surface 16.47   c-muted on c-surface 4.74
#   c-ink    on c-slot    15.19   c-muted on c-slot    4.37
#   c-accent on c-bg       5.17   c-accent on c-surface 4.70
#   c-accent-ink on c-accent 5.17
# 最低一对 4.37 是屏幕图位里那句 21px/600 的占位提示——它是空态标签，拖进
# 截图后就没了，且按 WCAG 大字（≥18.66px 粗体）算门槛是 3.0。真正长期存在
# 的正文（屏说明、规矩说明）压在 c-bg / c-surface 上，4.74 起。c-line 只
# 用于机身描边与听筒条，是非文字图形。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "App 新手引导三屏 · 3:4 单卡")
