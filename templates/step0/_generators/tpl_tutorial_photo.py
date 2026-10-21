#!/usr/bin/env python3
"""photo-composition-tutorial.op — 手机摄影构图教学（1080×1440，3:4 五帧）

教程档里唯一一张**把教学内容画进主视觉**的模板。别的教程是「图 + 说明」，
构图教学的说明写得再好也没用——构图必须**画在取景框上**才说得清。所以每
一页的主视觉是一个深色取景框，荧光绿参考线压在图片位之上：用户把自己的照
片拖进去，线还在，立刻能看出自己拍的东西落在哪一格。

风格取「黑白灰 + 荧光绿冲击风」：整篇零第二有彩色，荧光绿只出现在参考线、
序号与一个关键词上。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：不从内容采色——摄影教学的内容色就是照片本身，版面必须让位。
    取一档相机/取景器语境里有真实出处的荧光绿（对焦框、峰值对焦的高亮色）。
  - **收敛**：一条中性明度序列（L 0.06 / 0.09 / 0.42 / 0.85 / 0.91 / 0.96）
    + 1 个有彩色 #C8FF3D。
  - **论证**：荧光绿在浅底上只有 1.1:1，肉眼近乎不可见——所以取景框**必须
    是深色**，参考线才成立。这不是审美选择，是被这个强调色反推出来的结构：
    先定了「线要看得见」，才定了「框是黑的」。反过来，深框压在近白页面上又
    自动获得了封面级的对比，整套不需要再加任何装饰。

### 负约束（本模板明令不做的事）

  - **参考线不用 `line` 节点画。** 试过，不成立：`line` 的 x/y 在渲染时被
    当作**文档绝对坐标**，不吃父节点的布局偏移（用 snapshot_layout 验证：
    同一个 layout:none 框里，`图片位` 拿到绝对坐标 1272/242，四条 `line`
    仍是 312/0 这样的局部值），所以第 2-4 帧的线全被画到画布左边、被
    clipContent 裁掉。参考线一律用 `rectangle`，rect 在 layout:none 下是
    父相对定位、`rotation` 绕自身中心，斜线也能画。
  - 代价要说清：rect 是合法的图片拖放目标
    （`image_drop.rs::node_accepts_image_drop`）。四条 4px 参考线合计只占
    取景框面积的 1.4%，拖照片时正好砸中某一条的概率极低；真砸中了是那条线
    被填成图片，撤销一次重拖即可。
  - 不用第二个有彩色。荧光绿之外的一切都是中性明度。
  - 不在浅底上写荧光绿文字（1.1:1，等于没写）。荧光绿承载文字时底一律是
    近黑。
  - 不用渐变、不用发光、不用胶片颗粒滤镜、不用相机 emoji。
  - 一页只讲一个构图法。四个法则挤一页是这类内容最常见的死法。
  - 不写「氛围感 / 高级感 / 出片」这类形容词，每条要点都写成可执行动作。

硬契约：
  - 内容距边缘 ≥72px（这里 72）
  - 配色全部走 color_vars；换主色只需改 c-accent 与 c-accent-ink
  - 正文与背景对比度 ≥2.0（实测表见文件末尾）
  - **CJK 行高比西文全线高 0.2**：display 1.2，标题 1.35，正文 1.7
  - **CJK 负字距不超过 -0.02em**（88px 标题 → -1.7px = -0.019em）
  - 汉字走 Noto Sans SC，数字与拉丁走 Inter
  - 顶层 frame 显式写 x/y，五帧按 W+GAP 横排
  - 取景框写死 936×819（1.14:1）——参考线按它算绝对坐标；这个数由页面
    余量反解，见 FRAME_H 注释
"""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import (Ids, color_vars, frame, icon_font, rect, solid, text,
                   write_doc)

ids = Ids()

VARS = color_vars({
    "c-bg":         "#F4F4F2",
    "c-card":       "#FFFFFF",
    "c-line":       "#DCDCD8",
    "c-ink":        "#101010",
    "c-frame":      "#171715",
    "c-muted":      "#676764",
    # 深框里的次级文字。同一个 c-muted 压在近黑上只有 3.4:1，深浅两套次级
    # 色是必要的（和 pitfall-list 那张同一个理由）。
    "c-inv-muted":  "#9C9C98",
    "c-accent":     "#C8FF3D",
    "c-accent-ink": "#141C00",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W, H, GAP = 1080, 1440, 120

# 3 板一行 —— 与 deck 体系（deckkit.BOARDS_PER_ROW）同一约定：多板模板在画布上
# 分行铺开，而不是拖成一长排。行间距比列间距多 240 不是手滑：画布在帧上方以
# **屏幕空间**固定偏移画帧名，缩到能整屏看时 120 文档像素只剩十几个屏幕像素，
# 第二行的帧名会压到上一行的板上。
BOARDS_PER_ROW = 3
ROW_GAP = GAP + 240
EDGE = 72

LH_DISPLAY, LH_HEAD, LH_BODY = 1.2, 1.35, 1.7

# 取景框写死尺寸。参考线全是相对这个框的绝对坐标。
# 819 是解出来的，不是挑的：法则页固定部分（页头 118 + 要点区 186 +
# 页脚 33）+ 上下 padding 152 + 三个 44 的 gap = 621，1440 减掉就是它。
# 改页头/要点区任何一行的字号或行数，都要把这个数重解一次。
FRAME_W, FRAME_H = 936, 819
GUIDE = 4

PAGE_GAP = 44
TOTAL = 5


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


def bar(name, x, y, w, h, color="$c-accent", rotation=None):
    """一条参考线，实体 rect。见文件头负约束：不用 `line` 节点。"""
    node = rect(ids, name, x=x, y=y, width=w, height=h, fill=solid(color))
    if rotation is not None:
        node["rotation"] = rotation
    return node


def diag(name, x1, y1, x2, y2, thickness=GUIDE, color="$c-accent"):
    """两点之间的斜线。rect 的 rotation 绕自身中心转，所以按中点摆位。"""
    dx, dy = x2 - x1, y2 - y1
    length = math.hypot(dx, dy)
    angle = math.degrees(math.atan2(dy, dx))
    cx, cy = (x1 + x2) / 2, (y1 + y2) / 2
    return bar(name, round(cx - length / 2, 2), round(cy - thickness / 2, 2),
               round(length, 2), thickness, color=color,
               rotation=round(angle, 3))


def page(name, children, *, index, fill="$c-bg", pad_top=80):
    node = frame(ids, name, width=W, height=H, layout="vertical",
                 padding=[pad_top, EDGE, 72, EDGE], gap=PAGE_GAP,
                 fill=solid(fill), clipContent=True)
    node["children"] = children
    node["x"] = (index % BOARDS_PER_ROW) * (W + GAP)
    node["y"] = (index // BOARDS_PER_ROW) * (H + ROW_GAP)
    return node


def footer(no, *, ink="$c-muted"):
    return row("页脚", [
        text(ids, "账号名", "@ 你的账号名", 24, 600, ink, family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
        text(ids, "页码", f"{no:02d} / {TOTAL:02d}", 24, 600, ink, family=NUM,
             width="fit_content", growth="auto", line_height=1.4),
    ], gap=16, justifyContent="space_between")


def tag(label, *, bg, fg):
    node = frame(ids, "标签", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[10, 20], cornerRadius=6,
                 alignItems="center", justifyContent="center", fill=solid(bg))
    node["children"] = [
        text(ids, "标签文字", label, 24, 700, fg, family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
    ]
    return node


# ------------------------------------------------------------------ 01 封面
COVER_RULES = ["三分法", "引导线", "框住主体"]


def cover():
    head = row("封面头", [
        tag("构图 · 五分钟", bg="$c-accent", fg="$c-accent-ink"),
        text(ids, "封面页码", "01 / 05", 24, 600, "$c-inv-muted", family=NUM,
             width="fit_content", growth="auto", line_height=1.4),
    ], gap=16, justifyContent="space_between")

    title = col("封面标题", [
        text(ids, "标题上", "同一个场景", 88, 700, "$c-card", family=CJK,
             line_height=LH_DISPLAY, spacing=-1.7),
        text(ids, "标题下", "为什么你拍得平", 88, 700, "$c-accent",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.7),
        text(ids, "封面副标题",
             "不是相机的问题。三条构图法则，今天就能用上。",
             28, 400, "$c-inv-muted", family=CJK, line_height=LH_BODY),
    ], gap=14)

    items = []
    for index, rule in enumerate(COVER_RULES, 1):
        items.append(row("法则预告", [
            text(ids, "预告序号", f"{index:02d}", 30, 700, "$c-accent",
                 family=NUM, width=64, line_height=1.0),
            text(ids, "预告名", rule, 34, 600, "$c-card", family=CJK,
                 line_height=LH_HEAD),
        ], gap=8))
    preview = col("法则预告区", items, gap=22)

    body = col("封面主体", [title, preview], gap=64,
               height="fill_container", justifyContent="space_between")
    return page("01 封面", [head, body, footer(1, ink="$c-inv-muted")],
                index=0, fill="$c-ink", pad_top=88)


# ------------------------------------------------------- 02-04 构图法则页
def guides_thirds():
    """三分法：两竖两横，把框切成九宫格。"""
    xs = [round(FRAME_W / 3), round(FRAME_W * 2 / 3)]
    ys = [round(FRAME_H / 3), round(FRAME_H * 2 / 3)]
    return ([bar("竖线", x, 0, GUIDE, FRAME_H) for x in xs]
            + [bar("横线", 0, y, FRAME_W, GUIDE) for y in ys])


def guides_leading():
    """引导线：两条从底边收敛到上三分之一的斜线，外加那个汇合标。"""
    vx, vy = FRAME_W / 2, round(FRAME_H * 0.32)
    return [
        diag("左引导", 0, FRAME_H, vx, vy),
        diag("右引导", FRAME_W, FRAME_H, vx, vy),
        bar("汇合横标", round(vx) - 44, vy, 88, GUIDE),
    ]


def guides_frame():
    """框住主体：四条边围出一个内框，主体要落在框里。"""
    m = 96
    span_w, span_h = FRAME_W - m * 2, FRAME_H - m * 2
    return [
        bar("内框上", m, m, span_w, GUIDE),
        bar("内框下", m, FRAME_H - m - GUIDE, span_w, GUIDE),
        bar("内框左", m, m, GUIDE, span_h),
        bar("内框右", FRAME_W - m - GUIDE, m, GUIDE, span_h),
    ]


def viewfinder(guides, hint):
    """取景框 = 参考线（在上）+ 图片位（在下）。

    layout:none 下 children[0] 最靠前，所以参考线必须排在图片位前面。
    """
    slot = frame(ids, "图片位", x=0, y=0, width=FRAME_W, height=FRAME_H,
                 layout="vertical", gap=14, alignItems="center",
                 justifyContent="center", fill=solid("$c-frame"))
    slot["children"] = [
        icon_font(ids, "图片位图标", "image", 52, "$c-inv-muted"),
        text(ids, "图片位提示", hint, 28, 600, "$c-card", family=CJK,
             align="center", line_height=LH_HEAD),
        text(ids, "图片位规格", "拖进来后参考线还在，直接对照",
             23, 400, "$c-inv-muted", family=CJK, align="center",
             line_height=LH_BODY),
    ]
    box = frame(ids, "取景框", width=FRAME_W, height=FRAME_H, layout="none",
                cornerRadius=8, fill=[], clipContent=True)
    box["children"] = guides + [slot]
    return box


def point(kind, label, desc):
    """一条要点。做/不做用图标 + 底色区分，不引第二个色相。"""
    good = kind == "do"
    chip = frame(ids, "要点标记", width=44, height=44, layout="horizontal",
                 alignItems="center", justifyContent="center", cornerRadius=8,
                 fill=solid("$c-accent" if good else "$c-ink"))
    chip["children"] = [
        icon_font(ids, "要点图标", "check" if good else "x", 24,
                  "$c-accent-ink" if good else "$c-card"),
    ]
    return row("要点", [
        chip,
        col("要点文案", [
            text(ids, "要点标题", label, 28, 600, "$c-ink", family=CJK,
                 line_height=LH_HEAD),
            text(ids, "要点说明", desc, 24, 400, "$c-muted", family=CJK,
                 line_height=LH_BODY),
        ], gap=4),
    ], gap=16, align="start")


def rule_page(no, name, lead, guides, hint, do, dont):
    head = col("页头", [
        row("法则标题行", [
            text(ids, "法则序号", f"{no - 1:02d}", 44, 700, "$c-ink",
                 family=NUM, width=88, line_height=1.0, spacing=-1),
            text(ids, "法则名", name, 46, 700, "$c-ink", family=CJK,
                 line_height=LH_HEAD),
        ], gap=8, align="center"),
        text(ids, "法则说明", lead, 26, 400, "$c-muted", family=CJK,
             line_height=LH_BODY),
    ], gap=12)

    points = col("要点区", [point("do", *do), point("dont", *dont)], gap=20)
    return page(f"{no:02d} {name}",
                [head, viewfinder(guides, hint), points, footer(no)],
                index=no - 1)


# ------------------------------------------------------------------ 05 收尾
CHECKS = [
    ("拍之前先决定主体是谁", "一张只留一个主体，其余全是背景。"),
    ("把主体挪出正中心", "正中心是最安全也最平的位置。"),
    ("按快门前退后半步", "多留出的边框，后期还能裁；裁没了补不回来。"),
]


def recap():
    head = col("页头", [
        tag("收尾", bg="$c-ink", fg="$c-accent"),
        text(ids, "收尾标题", "下次举起手机前\n先过这三句", 64, 700, "$c-ink",
             family=CJK, line_height=LH_DISPLAY, spacing=-1.2),
    ], gap=20)

    items = []
    for index, (title, desc) in enumerate(CHECKS, 1):
        badge = frame(ids, "序号方", width=52, height=52, layout="horizontal",
                      alignItems="center", justifyContent="center",
                      cornerRadius=8, fill=solid("$c-accent"))
        badge["children"] = [
            text(ids, "序号", str(index), 26, 700, "$c-accent-ink",
                 family=NUM, width="fit_content", growth="auto",
                 line_height=1.0),
        ]
        card = row("自检项", [
            badge,
            col("自检文案", [
                text(ids, "自检标题", title, 30, 600, "$c-ink", family=CJK,
                     line_height=LH_HEAD),
                text(ids, "自检说明", desc, 24, 400, "$c-muted", family=CJK,
                     line_height=LH_BODY),
            ], gap=6),
        ], gap=18, align="start", padding=[26, 26])
        card["fill"] = solid("$c-card")
        card["cornerRadius"] = 14
        items.append(card)

    cta = col("关注卡", [
        text(ids, "关注标题", "拍完发出来", 40, 700, "$c-accent", family=CJK,
             line_height=LH_HEAD),
        text(ids, "关注副文案", "评论区贴图，我挨个说说该怎么裁。", 26, 400,
             "$c-inv-muted", family=CJK, line_height=LH_BODY),
    ], gap=10, padding=[36, 36])
    cta["fill"] = solid("$c-ink")
    cta["cornerRadius"] = 16

    body = col("收尾主体", [col("自检区", items, gap=18), cta], gap=32,
               height="fill_container", justifyContent="space_between")
    return page("05 收尾", [head, body, footer(5)], index=4)


def build():
    return [
        cover(),
        rule_page(2, "三分法",
                  "把画面切成九格，主体放在线上或交点上，别放正中间。",
                  guides_thirds(), "拖一张风景或人像进来",
                  ("主体压在竖线上", "地平线压在上横线或下横线，不居中。"),
                  ("主体钉死在正中心", "对称构图另说，日常随手拍会显得呆。")),
        rule_page(3, "引导线",
                  "让路、栏杆、影子这些线条从画面底部指向主体。",
                  guides_leading(), "拖一张有路或走廊的照片",
                  ("蹲低一点再拍", "机位越低，地面的线条收得越急。"),
                  ("站直了平拍", "平视时线条被压扁，几乎没有纵深。")),
        rule_page(4, "框住主体",
                  "用门框、树枝、窗户在主体外面再套一层框。",
                  guides_frame(), "拖一张有门洞或树枝的照片",
                  ("前景压暗一点", "外框暗、主体亮，视线自己就走进去了。"),
                  ("外框喧宾夺主", "外框占超过一半画面，主体就被吃掉了。")),
        recap(),
    ]


# 对比度（WCAG 相对亮度比，op-design-lint 门槛 2.0；数值由脚本实测）：
#   c-ink       on c-bg     17.28   c-muted     on c-bg      5.15
#   c-ink       on c-card   19.03   c-muted     on c-card    5.67
#   c-card      on c-ink    19.03   c-inv-muted on c-ink     6.91
#   c-card      on c-frame  17.95   c-inv-muted on c-frame   6.52
#   c-accent    on c-ink    16.15   c-accent-ink on c-accent 14.91
#   c-accent    on c-frame  15.24   c-accent    on c-bg       1.07  ← 禁用
# 承载正文的最低一对是 5.15。最后一对是这套配色的雷：荧光绿压在浅底上只有
# 1.07:1，所以整篇没有一处浅底荧光绿文字——荧光绿要么当参考线（非文字图
# 形，压在 c-frame 上有 15.24），要么写在近黑上。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "手机摄影构图教学 · 3:4 五帧")
