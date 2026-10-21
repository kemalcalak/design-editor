#!/usr/bin/env python3
"""tidemark-slate-deck.op — 潮痕石板 · 数据评审档（D8，7 页 1920×1080）

落格：medium-high formality × **high** density × light。
定位：把「这个周期发生了什么、相对上一个周期在哪」一次说清的评审档 ——
指标带刻度、状态有语义、路线走泳道。

### 锚点

港池的潮痕石。被反复涨落染出刻痕的石板灰、刻在石上的潮位标记、航道上的三色
浮标。锚的是**「刻痕就是上一次的位置」**这件事 —— 评审的本质不是报数，是报
「相对上次」。不锚海浪、船只、灯塔等任何具象物。

### 三个招牌母题

1. **潮位磁贴**（02 页）—— 四层：标签 → mono 数值 → **一条贯穿磁贴宽度的刻度
   线，目标画成刻度上的一个位置** → 变化量 chip。目标不写成「目标：X」这行
   字，而是画成一个位置 —— 这样「离目标多远」是被**看见**的，不是被读出来
   的。这也是本档与通行 KPI 卡的分野所在。
2. **航标点**（04 / 05 页）—— 严格语义的三色圆点与药丸。硬规则：**任何带航标
   点的行必须同时有「责任人」与「到期」两列**，缺任一项就把点删掉 —— 无主的
   状态比没有状态更糟。
3. **潮汐泳道**（06 页）—— 左侧泳道名列 + 右侧时间区；条 = 淡蓝底 + 左端一段
   实色；里程碑 = 旋转 45° 的方块。**里程碑与状态点必须形状可辨**，不能只靠
   颜色区分（色觉障碍与投影衰减下颜色是最先失效的那一维）。

### 负约束（spec §2 D8 Strictly avoid，逐条落实）

  - 航标三色**绝不作图表系列色**，只作点与药丸。图表永远单色阶。
  - 图不用网格线、图例、y 轴刻度；数值直接标在柱端。
  - 不用饼图 / 环形图 / 3D / 渐变柱 / 阴影柱。
  - 磁贴之外零阴影（本稿更进一步：**整套零阴影** —— 磁贴与页底之间的明度差
    已经够把它托起来，再压一层投影是替可读性买不需要的保险）。
  - 有航标点的行必须有责任人与到期。
  - 页标题写结论不写栏目名。
  - 潮痕蓝之外只有中性与三支语义色，无第二支非语义有彩色。
  - 数字一律 mono，且数值列宽写死（§6.10：渲染栈不保证 tabular 数字）。
  - 不做双色柱对照；对照走灰色影子柱。
  - 磁贴固定 4 张一行 —— 3 张留尴尬空档，5 张把 mono 64 逼到换行。

### 与 spec 的两处落地取舍

  - **磁贴标签不加 letterSpacing。** spec 写「24px 大写标签 letterSpacing +2」，
    那条是给大写拉丁写的；§3.8.2 同时规定「<48px 的中文一律 0」。本档文案全
    中文，取后者 —— 中文加正字距会把词间关系拆散。
  - **药丸高 36 / 圆角 18，不是 spec 写的 28 / 14。** 24px 中文的行盒就有 34px
    高，28 的药丸会把字压到贴边。字号地板（§3.4）比装置尺寸硬，让装置。

### 密度核对（§3.2，槽位按「一条要点计 1」）

    01 封面 5/5 · 02 全景磁贴 14/16 · 03 趋势 5/8 · 04 明细表 10/14
    05 风险 5/9 · 06 泳道 9/10 · 07 决议 5/6

对比度实测见文件末尾。
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from deckkit import (CJK, H, NUM, W, assert_type_scale, audit_document,
                     cjk_width, dash_col, latin_track, lh, place_boards,
                     track)
from oplib import Ids, color_vars, frame, rect, solid, text, write_doc

ids = Ids()

# 色板来自 spec §2 D8（oklch 实算）。两条推导必须留下：
#   · 底走 H240 冷向、chroma 只有 0.004。数据页面上任何暖底都会让数字读起来
#     「被氛围包装过」；接近中性的冷底是唯一不干扰数值判断的底。
#   · 三支航标色**不是配色，是语义**。它们不参与「一主一辅一强调」的分配，
#     只作圆点与药丸；一旦绿柱黄柱红柱同框，语义就被稀释成分类，而分类色恰
#     恰是我们不需要的（图表只用单色阶）。
VARS = color_vars({
    "c-wash":       "#F1F4F6",   # 页面底
    "c-panel":      "#E4E8EB",   # 磁贴 / 卡面
    "c-rule":       "#CED1D4",   # 分隔线 / 表格线
    "c-ink":        "#19212A",   # 主文字            14.71 : wash
    "c-ink-soft":   "#515961",   # 次级文字           6.44
    "c-ink-faint":  "#737A81",   # 标签 / 页码        3.94（仅 ≥24px）
    "c-tide":       "#006C9B",   # 主 accent          5.25
    "c-tide-deep":  "#004C77",   # 承载白字的蓝块     8.27（白字）
    "c-tide-wash":  "#D6ECF9",   # 图表非 key 系列    13.33（ink）
    "c-buoy-green": "#35824B",   # 语义：正常         4.28
    "c-buoy-amber": "#AF7100",   # 语义：风险         3.67
    "c-buoy-red":   "#B24037",   # 语义：阻塞         5.16
})

# 高密度档收窄边距，但不越过 §4.1 的 72 软线。下 > 上仍然成立（投影下沿遮挡）。
PAD_TOP, PAD_BOTTOM, PAD_X = 88, 104, 96
INNER = W - PAD_X * 2          # 1728
GUTTER = 16                    # 高密度档的呼吸单位比其他档小一级
COLGAP = 24                    # 列与列之间的沟槽

# 圆角只有两档：磁贴 8、药丸 18。写成常量是为了让「又多了一个圆角值」在 diff
# 里可数 —— §3.3 把圆角值列进不允许跨页变的一栏。
R_TILE, R_PILL = 8, 18

FS_COVER = 88         # 封面主标
FS_TITLE = 56         # 页标题（比 D7 小一档：密度高，标题必须让位给内容）
FS_LEDE = 36          # 引导句 / 注解标题 / 风险条标题
FS_VALUE = 64         # 磁贴数值（mono）
FS_BODY = 32          # 决议正文
FS_TABLE = 28         # 表格正文 / 表头 / 风险说明
FS_META = 26          # 磁贴副行 / 说明
FS_NOTE = 24          # 标签 / 来源 / 页码

DECK_LABEL = "合章电子签 · 2026 Q2 业务评审"


# ------------------------------------------------------------------ 骨架
def slide(name, *, gap=COLGAP, justify="start"):
    body = frame(ids, f"{name} · 正文", width="fill_container",
                 height="fill_container", layout="vertical",
                 padding=[PAD_TOP, PAD_X, PAD_BOTTOM, PAD_X], gap=gap,
                 justifyContent=justify, alignItems="start", fill=[])
    body["children"] = []
    shell = frame(ids, name, width=W, height=H, layout="none",
                  fill=solid("$c-wash"), clipContent=True)
    shell["children"] = [body]
    shell["content"] = body
    return shell


def col(name, children, *, gap=GUTTER, width="fill_container",
        height="fit_content", align="start", fill=None, **props):
    node = frame(ids, name, width=width, height=height, layout="vertical",
                 gap=gap, alignItems=align, fill=fill or [], **props)
    node["children"] = children
    return node


def row(name, children, *, gap=COLGAP, width="fill_container",
        height="fit_content", align="start", fill=None, **props):
    node = frame(ids, name, width=width, height=height, layout="horizontal",
                 gap=gap, alignItems=align, fill=fill or [], **props)
    node["children"] = children
    return node


def hrule(color="$c-rule", thickness=1, width="fill_container"):
    """横线用 rectangle，不用 `line`（§6.3：line 在 layout:none 下吃绝对坐标）。"""
    return rect(ids, "横线", width=width, height=thickness, fill=solid(color))


def title(content):
    return text(ids, "页标题", content, FS_TITLE, 700, "$c-ink", family=CJK,
                line_height=lh(FS_TITLE), spacing=track(FS_TITLE), width=INNER)


def note(content):
    return text(ids, "来源脚注", content, FS_NOTE, 400, "$c-ink-faint",
                family=CJK, line_height=lh(FS_NOTE),
                width=cjk_width(FS_NOTE, INNER))


def footer(page):
    """页脚家具。§3.3 把页码位置列进不允许跨页变的一栏，02–07 的 y 完全相同。"""
    left = text(ids, "页脚眉标", DECK_LABEL, FS_NOTE, 400, "$c-ink-faint",
                family=CJK, width="fit_content", growth="auto",
                line_height=lh(FS_NOTE))
    folio = text(ids, "页码", f"{page:02d} / 07", FS_NOTE, 500, "$c-ink-faint",
                 family=NUM, width="fit_content", growth="auto",
                 line_height=lh(FS_NOTE), spacing=latin_track("07"))
    return row("页脚", [left, folio], gap=COLGAP, align="center",
               justifyContent="space_between")


def buoy_dot(color, size=12):
    return rect(ids, "航标点", width=size, height=size,
                cornerRadius=size // 2, fill=solid(color))


def buoy_pill(label, color):
    """航标药丸。高 36 / 圆角 18 —— 24px 中文的行盒就有 34px 高，spec 写的
    28 会把字压到贴边。字号地板比装置尺寸硬，让装置。"""
    node = frame(ids, "航标药丸", width="fit_content", height=36,
                 layout="horizontal", padding=[0, 16], gap=0,
                 cornerRadius=R_PILL, alignItems="center",
                 justifyContent="center", fill=solid(color))
    node["children"] = [
        text(ids, "航标文字", label, FS_NOTE, 600, "$c-wash", family=CJK,
             width="fit_content", growth="auto", line_height=1.2),
    ]
    return node


# ------------------------------------------------------------------ 01 封面
def cover():
    chip = frame(ids, "周期标记", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[8, 16], gap=0,
                 cornerRadius=R_TILE, alignItems="center",
                 justifyContent="center", fill=solid("$c-tide-wash"))
    chip["children"] = [
        text(ids, "周期文字", "2026 Q2", FS_NOTE, 600, "$c-tide-deep",
             family=NUM, width="fit_content", growth="auto", line_height=1.3,
             spacing=latin_track("2026 Q2")),
    ]

    lede = col("主张", [
        text(ids, "主标题", "签得多了\n但签得更慢了", FS_COVER, 700, "$c-ink",
             family=CJK, line_height=lh(FS_COVER), spacing=track(FS_COVER),
             width=INNER),
        text(ids, "范围说明", "本次评审覆盖企业版四条业务线，数据截至 6 月 30 日。",
             FS_BODY, 400, "$c-ink-soft", family=CJK,
             line_height=lh(FS_BODY, body=True),
             width=cjk_width(FS_BODY, INNER)),
    ], gap=COLGAP)

    # 一条潮痕：整幅横线 + 一段实色刻痕。封面唯一的非文字元素，也是把「刻痕
    # 就是上一次的位置」这个锚点先说一遍 —— 后面每一张磁贴都在重复它。
    tide_mark = rect(ids, "潮痕刻度", width=220, height=4, fill=solid("$c-tide"))
    tide_mark["x"], tide_mark["y"] = 1040, 0
    line = rect(ids, "潮痕底线", width=INNER, height=2, fill=solid("$c-rule"))
    line["x"], line["y"] = 0, 1
    tide = frame(ids, "潮痕", width=INNER, height=4, layout="none", fill=[])
    tide["children"] = [tide_mark, line]

    meta = row("落款", [
        text(ids, "汇报人", "增长与运营部 · 周砚", FS_META, 500, "$c-ink-soft",
             family=CJK, width="fit_content", growth="auto",
             line_height=lh(FS_META)),
        text(ids, "日期", "2026.07.09", FS_META, 500, "$c-ink-faint",
             family=NUM, width="fit_content", growth="auto",
             line_height=lh(FS_META), spacing=latin_track("2026.07.09")),
    ], gap=COLGAP, align="center", justifyContent="space_between")

    s = slide("01 封面", gap=COLGAP, justify="space_between")
    s["content"]["children"] = [chip, lede, col("页尾", [tide, meta],
                                                gap=COLGAP)]
    return s


# -------------------------------------------------------------- 02 全景磁贴
# (标签, 数值, 变化, 比较词, 语义, 实际占比, 目标占比)
# 占比是刻度线上的位置，不是数值本身 —— 四个指标量纲不同，能同框比较的只有
# 「离目标多远」这一件事，所以刻度线量的是它。
TILES = [
    ("季度签署量（万份）", "18.6", "+31%", "对比上季", "green", 0.78, 0.83),
    ("平均签署时长（小时）", "6.4", "+0.9", "比上季更慢", "amber", 0.80, 0.50),
    ("一次签成率", "91.2%", "+1.8", "个百分点", "green", 0.91, 0.93),
    ("已上线集成（个）", "11", "-7", "低于目标", "red", 0.55, 0.90),
]

BUOY = {"green": "$c-buoy-green", "amber": "$c-buoy-amber",
        "red": "$c-buoy-red"}

TILE_W = (INNER - COLGAP * 3) // 4      # 414
TILE_PAD = 28
TILE_INNER = TILE_W - TILE_PAD * 2      # 358
TILE_H = 360


def tidemark_scale(value_ratio, target_ratio):
    """潮位刻度线：底线 + 已达段 + 目标竖标。

    §6.4：`layout:none` 的 children[0] 在**最上**（与业界惯例相反）。目标竖标
    必须压在已达段与底线之上，所以它写在数组第一个 —— 写反了目标就被自己的
    进度条盖掉，而「离目标多远」正是这个装置存在的全部理由。
    """
    mark = rect(ids, "目标刻痕", width=3, height=14,
                fill=solid("$c-tide-deep"))
    mark["x"] = round(target_ratio * (TILE_INNER - 3), 2)
    mark["y"] = 0
    reached = rect(ids, "已达段", width=round(value_ratio * TILE_INNER, 2),
                   height=2, fill=solid("$c-tide"))
    reached["x"], reached["y"] = 0, 6
    base = rect(ids, "刻度底线", width=TILE_INNER, height=2,
                fill=solid("$c-rule"))
    base["x"], base["y"] = 0, 6
    node = frame(ids, "潮位刻度", width=TILE_INNER, height=14, layout="none",
                 fill=[])
    node["children"] = [mark, reached, base]
    return node


def tidemark_tile(label, value, delta, word, semantic, value_ratio,
                  target_ratio):
    chip = row("变化 chip", [
        buoy_dot(BUOY[semantic]),
        text(ids, "变化数值", delta, FS_META, 600, "$c-ink", family=NUM,
             width="fit_content", growth="auto", line_height=1.3,
             spacing=latin_track(delta)),
        text(ids, "比较词", word, FS_META, 400, "$c-ink-soft", family=CJK,
             width="fit_content", growth="auto", line_height=1.3),
    ], gap=10, width="fit_content", align="center")

    reading = col("读数", [
        text(ids, "磁贴数值", value, FS_VALUE, 700, "$c-ink", family=NUM,
             line_height=1.05, spacing=latin_track(value, 0)),
        tidemark_scale(value_ratio, target_ratio),
    ], gap=GUTTER + 4, width=TILE_INNER)

    # 三段分布而不是顺序堆叠：标签钉在顶、读数居中、变化量钉在底。四张贴子的
    # 三条基线因此跨贴对齐，扫一行就能横着比 —— 这是把「列对齐」这条本档承诺
    # 从表格延伸到磁贴。
    return col("潮位磁贴", [
        text(ids, "磁贴标签", label, FS_NOTE, 500, "$c-ink-soft", family=CJK,
             line_height=lh(FS_NOTE)),
        reading,
        chip,
    ], gap=GUTTER, width=TILE_W, height=TILE_H, justifyContent="space_between",
        padding=[TILE_PAD, TILE_PAD], cornerRadius=R_TILE,
        fill=solid("$c-panel"))


def overview():
    s = slide("02 全景磁贴", gap=COLGAP * 2, justify="space_between")
    s["content"]["children"] = [
        title("签署量涨了三成，签完的时间也一起涨了"),
        row("磁贴行", [tidemark_tile(*t) for t in TILES], gap=COLGAP,
            align="stretch"),
        col("页尾", [
            note("数据来源：合章数据平台，口径为企业版付费租户，"
                 "2026 年 4 月 1 日至 6 月 30 日。"),
            footer(2),
        ], gap=GUTTER),
    ]
    return s


# -------------------------------------------------------------- 03 趋势页
# 单色阶：只有 key 那两根走 accent，其余走 tide.wash。**不做双色柱对照** ——
# 一旦出现第二支彩色，读者会去找「这两个颜色分别代表什么」，而这张图要说的
# 只有一件事：最后两个月在加速。
TREND = [("一月", "2.4", 258, False), ("二月", "2.9", 312, False),
         ("三月", "2.7", 291, False), ("四月", "3.2", 345, False),
         ("五月", "3.4", 366, True), ("六月", "3.9", 420, True)]

CHART_W = 1240
ANNOTATE_W = INNER - CHART_W - COLGAP * 2      # 440


def trend_chart():
    bar_w = (CHART_W - COLGAP * (len(TREND) - 1)) // len(TREND)
    bars = []
    for label, value, height, key in TREND:
        bars.append(col(f"{label} 柱", [
            # 数值直接标在柱端，不做 y 轴、不做网格线、不做图例（§3.6）。
            text(ids, "柱值", value, FS_META, 600,
                 "$c-ink" if key else "$c-ink-soft", family=NUM,
                 line_height=1.2, align="center", spacing=latin_track(value)),
            rect(ids, "月柱", width=bar_w, height=height,
                 fill=solid("$c-tide" if key else "$c-tide-wash")),
            text(ids, "月份", label, FS_NOTE, 400, "$c-ink-faint", family=CJK,
                 line_height=lh(FS_NOTE), align="center"),
        ], gap=8, width=bar_w, align="center", justifyContent="end",
            height="fill_container"))
    return row("趋势柱图", bars, gap=COLGAP, width=CHART_W,
               height=420 + 34 + 36 + 16, align="end")


TREND_NOTES = [
    ("中小客户是自己找上门的", "六月新签里 71% 没有销售触达，\n来自模板市场与搜索。"),
    ("大客户的量卡在集成上", "两家头部客户的用印审批\n还在他们自己的 OA 里。"),
]


def trend():
    blocks = []
    for head, desc in TREND_NOTES:
        if blocks:
            blocks.append(hrule())
        blocks.append(col("注解组", [
            text(ids, "注解标题", head, FS_LEDE, 600, "$c-ink", family=CJK,
                 line_height=lh(FS_LEDE), width=ANNOTATE_W),
            text(ids, "注解说明", desc, FS_META, 400, "$c-ink-soft",
                 family=CJK, line_height=lh(FS_META, body=True),
                 width=ANNOTATE_W),
        ], gap=8))

    s = slide("03 趋势", gap=COLGAP * 2, justify="space_between")
    s["content"]["children"] = [
        col("论证", [
            title("签署量每个月都在涨，涨的全是中小客户"),
            row("趋势区", [trend_chart(),
                          col("注解栏", blocks, gap=COLGAP,
                              width=ANNOTATE_W)],
                gap=COLGAP * 2, align="start"),
        ], gap=COLGAP * 2),
        col("页尾", [
            note("数据来源：合章数据平台，按签署完成时间归月，单位 万份。"),
            footer(3),
        ], gap=GUTTER),
    ]
    return s


# -------------------------------------------------------------- 04 明细表
# 列宽写死并且必须加起来等于 INNER。§6.10：渲染栈不保证 tabular 数字，日期列
# 与状态列都靠定宽站住，不能指望 fit_content 让它们自己对齐。
DETAIL_COLS = [420, 200, 220, 220, 572]
assert sum(DETAIL_COLS) + COLGAP * (len(DETAIL_COLS) - 1) == INNER

DETAIL_HEAD = ["集成对象", "状态", "责任人", "到期", "当前卡点"]
# 母题 2 的硬规则：**带航标点的行必须同时有责任人与到期**。这张表的四列结构
# 就是那条规则的物理形态 —— 缺任一列，这一行的状态就是无主的。
DETAIL_ROWS = [
    ("钉钉待办同步", "正常", "green", "陆迎", "07-18", "联调已过，等对方发版"),
    ("企业微信审批", "正常", "green", "陆迎", "07-25", "灰度中，两家客户在跑"),
    ("金蝶云星空", "风险", "amber", "沈知白", "07-11", "对方接口文档改了三次"),
    ("用友 U8C", "风险", "amber", "沈知白", "07-04", "已逾期一周，等对方排期"),
    ("飞书多维表", "正常", "green", "高见", "08-08", "需求已确认，尚未开工"),
    ("泛微 OA", "阻塞", "red", "高见", "06-20", "对方要求私有化部署"),
    ("致远 A8", "阻塞", "red", "关屿", "06-27", "客户侧改了用印流程"),
]


def detail_table():
    head = row("表头", [
        text(ids, "表头单元", label, FS_TABLE, 600, "$c-ink-soft", family=CJK,
             line_height=lh(FS_TABLE), width=width)
        for label, width in zip(DETAIL_HEAD, DETAIL_COLS)
    ], gap=COLGAP, align="center")

    parts = [head, hrule("$c-tide", 2)]
    for index, (name, state, semantic, owner, due, blocker) in enumerate(
            DETAIL_ROWS):
        status = row("状态单元", [
            buoy_dot(BUOY[semantic]),
            text(ids, "状态文字", state, FS_TABLE, 500, "$c-ink", family=CJK,
                 width="fit_content", growth="auto", line_height=lh(FS_TABLE)),
        ], gap=10, width=DETAIL_COLS[1], align="center")
        parts.append(row("表行", [
            text(ids, "集成名", name, FS_TABLE, 500, "$c-ink", family=CJK,
                 line_height=lh(FS_TABLE), width=DETAIL_COLS[0]),
            status,
            text(ids, "责任人", owner, FS_TABLE, 400, "$c-ink-soft",
                 family=CJK, line_height=lh(FS_TABLE), width=DETAIL_COLS[2]),
            text(ids, "到期", due, FS_TABLE, 500, "$c-ink-soft", family=NUM,
                 line_height=lh(FS_TABLE), width=DETAIL_COLS[3],
                 spacing=latin_track(due)),
            text(ids, "卡点", blocker, FS_TABLE, 400, "$c-ink-soft",
                 family=CJK, line_height=lh(FS_TABLE), width=DETAIL_COLS[4]),
        ], gap=COLGAP, align="center"))
        # 末行无线：最后一条线画出去，表就读作一个被框住的块，而不是一列还能
        # 往下续的记录。
        if index < len(DETAIL_ROWS) - 1:
            parts.append(hrule())
    return col("集成明细", parts, gap=GUTTER - 4)


def detail():
    s = slide("04 明细表", gap=COLGAP * 2, justify="space_between")
    s["content"]["children"] = [
        col("论证", [title("十一个集成里，四个已经逾期"), detail_table()],
            gap=COLGAP * 2),
        col("页尾", [
            note("数据来源：集成交付看板，状态截至 7 月 8 日 18:00。"),
            footer(4),
        ], gap=GUTTER),
    ]
    return s


# -------------------------------------------------------------- 05 风险页
RISK_STATE_W = 140
RISK_TEXT_W = 1180

RISKS = [
    ("阻塞", "red", "两家 OA 厂商要求私有化部署",
     "涉及四家年费合计两千四百万的客户，都在等这条。",
     "八月前给出私有化版本的报价与交付边界。", "关屿", "08-15"),
    ("风险", "amber", "平均签署时长从 4.1 小时涨到 6.4",
     "投诉集中在等对方签，不在我们这一段。",
     "九月上线到期提醒与代签授权，先在华东试。", "周砚", "09-30"),
    ("风险", "amber", "模板市场的增长没有人守",
     "六月 71% 的新签来自这里，但它没有归属团队。",
     "七月内定归属，先归运营，季度末重评。", "周砚", "07-31"),
]


def risk():
    items = []
    for state, semantic, head, impact, plan, owner, due in RISKS:
        if items:
            items.append(hrule())
        # 责任人与到期紧跟航标点，同一行右端 —— 母题 2 的硬规则在这一页的形态。
        owner_line = row("归属", [
            text(ids, "责任人", owner, FS_META, 500, "$c-ink-soft",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=lh(FS_META)),
            text(ids, "到期", due, FS_META, 500, "$c-ink-faint", family=NUM,
                 width="fit_content", growth="auto", line_height=lh(FS_META),
                 spacing=latin_track(due)),
        ], gap=GUTTER, width="fit_content", align="center")

        items.append(row("风险条", [
            col("状态列", [buoy_pill(state, BUOY[semantic])], gap=0,
                width=RISK_STATE_W),
            col("风险文案", [
                text(ids, "风险标题", head, FS_LEDE, 600, "$c-ink",
                     family=CJK, line_height=lh(FS_LEDE),
                     width=cjk_width(FS_LEDE, RISK_TEXT_W)),
                text(ids, "风险影响", impact, FS_TABLE, 400, "$c-ink-soft",
                     family=CJK, line_height=lh(FS_TABLE, body=True),
                     width=cjk_width(FS_TABLE, RISK_TEXT_W)),
                text(ids, "风险应对", plan, FS_TABLE, 400, "$c-ink",
                     family=CJK, line_height=lh(FS_TABLE, body=True),
                     width=cjk_width(FS_TABLE, RISK_TEXT_W)),
            ], gap=8, width=RISK_TEXT_W),
            col("归属列", [owner_line], gap=0, width="fill_container",
                align="end"),
        ], gap=COLGAP, align="start"))

    s = slide("05 风险", gap=COLGAP * 2, justify="space_between")
    s["content"]["children"] = [
        col("论证", [title("三个风险，两个我们自己能解"),
                    col("风险列表", items, gap=COLGAP + 16)], gap=COLGAP * 2),
        footer(5),
    ]
    return s


# -------------------------------------------------------------- 06 泳道路线
LANE_NAME_W = 200
LANE_TIME_W = INNER - LANE_NAME_W - COLGAP     # 1504
LANE_H = 56
BAR_H = 40
MONTHS = ["七月", "八月", "九月"]
# (泳道名, 起点, 终点, 里程碑) —— 比例是相对整个季度的位置。
LANES = [
    ("集成交付", 0.00, 0.83, 0.66),
    ("私有化版本", 0.16, 1.00, 0.97),
    ("签署提速", 0.33, 1.00, 0.72),
    ("模板市场归属", 0.00, 0.44, 0.41),
]


def lane_track(start, end, milestone):
    """一条泳道。

    条用 frame 而不是 rectangle：左端那 6px 实色段是**子节点**，而
    §6.13「rectangle 是容器但不递归渲染子节点」意味着放进 rect 里的东西不会
    显示。clipContent 让实色段跟着圆角收边。

    月份分隔用一串短 rectangle 拼的虚线（§6.1：`dashPattern` 是死字段）。
    里程碑是旋转 45° 的方块而不是圆点：**里程碑与状态点必须形状可辨**，不能
    只靠颜色 —— 颜色是投影衰减与色觉障碍下最先失效的那一维。
    """
    bar_x = round(start * LANE_TIME_W, 2)
    bar_w = round((end - start) * LANE_TIME_W, 2)
    bar = frame(ids, "泳道条", width=bar_w, height=BAR_H, layout="horizontal",
                gap=0, cornerRadius=R_TILE, clipContent=True,
                fill=solid("$c-tide-wash"))
    bar["children"] = [rect(ids, "起点段", width=6, height=BAR_H,
                            fill=solid("$c-tide"))]
    bar["x"], bar["y"] = bar_x, (LANE_H - BAR_H) / 2

    flag = rect(ids, "里程碑", width=16, height=16, rotation=45,
                fill=solid("$c-tide-deep"))
    flag["x"] = round(milestone * LANE_TIME_W - 8, 2)
    flag["y"] = (LANE_H - 16) / 2

    kids = [flag, bar]
    for index in (1, 2):
        dash = dash_col(rect, ids, "月份分隔", "$c-rule", length=LANE_H)
        dash["x"] = round(index * LANE_TIME_W / 3, 2)
        dash["y"] = 0
        kids.append(dash)

    cell = frame(ids, "泳道时间区", width=LANE_TIME_W, height=LANE_H,
                 layout="none", fill=[])
    cell["children"] = kids
    return cell


def roadmap():
    month_w = (LANE_TIME_W - GUTTER * 2) // 3
    months = row("月份头", [
        frame(ids, "月份头占位", width=LANE_NAME_W, height=1, layout="none",
              fill=[]),
        row("月份标签", [
            text(ids, "月份名", label, FS_NOTE, 500, "$c-ink-faint",
                 family=CJK, line_height=lh(FS_NOTE), width=month_w,
                 align="center")
            for label in MONTHS
        ], gap=GUTTER, width=LANE_TIME_W),
    ], gap=COLGAP, align="center")

    lanes = []
    for name, start, end, milestone in LANES:
        lanes.append(row("泳道", [
            text(ids, "泳道名", name, FS_META, 500, "$c-ink", family=CJK,
                 line_height=lh(FS_META), width=LANE_NAME_W),
            lane_track(start, end, milestone),
        ], gap=COLGAP, align="center"))

    flag = rect(ids, "图例里程碑", width=16, height=16, rotation=45,
                fill=solid("$c-tide-deep"))
    sample = frame(ids, "图例条", width=48, height=20, layout="horizontal",
                   gap=0, cornerRadius=R_TILE, clipContent=True,
                   fill=solid("$c-tide-wash"))
    sample["children"] = [rect(ids, "图例起点段", width=6, height=20,
                               fill=solid("$c-tide"))]
    legend = row("图例", [
        sample,
        text(ids, "图例文字", "进行中", FS_NOTE, 400, "$c-ink-faint",
             family=CJK, width="fit_content", growth="auto",
             line_height=lh(FS_NOTE)),
        frame(ids, "图例间隔", width=24, height=1, layout="none", fill=[]),
        flag,
        text(ids, "图例文字", "里程碑", FS_NOTE, 400, "$c-ink-faint",
             family=CJK, width="fit_content", growth="auto",
             line_height=lh(FS_NOTE)),
    ], gap=12, width="fit_content", align="center")

    s = slide("06 泳道路线", gap=COLGAP * 2, justify="space_between")
    s["content"]["children"] = [
        title("三季度只做三件事，其余往后放"),
        col("泳道区", [months] + lanes, gap=COLGAP),
        col("页尾", [legend, footer(6)], gap=COLGAP),
    ]
    return s


# -------------------------------------------------------------- 07 决议页
DECISIONS = [
    ("私有化版本做不做，\n做到什么边界", "关屿", "07-12"),
    ("模板市场归运营\n还是归产品", "周砚", "07-12"),
    ("两家逾期集成\n是否换供应商", "沈知白", "07-19"),
]


def resolution():
    card_w = (INNER - COLGAP * 2) // 3
    cards = []
    for line, owner, due in DECISIONS:
        cards.append(col("待决事项", [
            hrule("$c-tide", 2, width=card_w),
            text(ids, "决议", line, FS_BODY, 600, "$c-ink", family=CJK,
                 line_height=lh(FS_BODY, body=True),
                 width=cjk_width(FS_BODY, card_w)),
            row("决策归属", [
                text(ids, "决策人", owner, FS_META, 500, "$c-ink-soft",
                     family=CJK, width="fit_content", growth="auto",
                     line_height=lh(FS_META)),
                text(ids, "期限", due, FS_META, 500, "$c-ink-faint",
                     family=NUM, width="fit_content", growth="auto",
                     line_height=lh(FS_META), spacing=latin_track(due)),
            ], gap=GUTTER, width="fit_content", align="center"),
        ], gap=COLGAP, width=card_w))

    s = slide("07 决议", gap=COLGAP * 2, justify="space_between")
    s["content"]["children"] = [
        title("这三件今天要定，定不了就带走"),
        row("待决区", cards, gap=COLGAP, align="start"),
        col("页尾", [
            text(ids, "收束句", "下个季度的增长在集成上，不在获客上。",
                 FS_LEDE, 500, "$c-ink-soft", family=CJK,
                 line_height=lh(FS_LEDE, body=True),
                 width=cjk_width(FS_LEDE, INNER)),
            footer(7),
        ], gap=COLGAP),
    ]
    return s


assert_type_scale([FS_COVER, FS_TITLE, FS_LEDE, FS_VALUE, FS_BODY, FS_TABLE,
                   FS_META, FS_NOTE], where="tidemark-slate-deck")


def build():
    boards = [cover(), overview(), trend(), detail(), risk(), roadmap(),
              resolution()]
    for board in boards:
        board.pop("content", None)
    place_boards(boards)
    # 本档只批准两个圆角值：磁贴 8、药丸 18。多出第三个就说明有人又发明了一
    # 档，§3.3 把圆角值列进「不允许跨页变」的一栏，这里是那条的执行点。
    return audit_document(boards, where="tidemark-slate-deck",
                          allowed_radii={R_TILE, R_PILL})


# 对比度（WCAG 相对亮度比，实算）：
#   c-ink       on c-wash    14.71     c-ink       on c-panel   13.19
#   c-ink-soft  on c-wash     6.44     c-ink-soft  on c-panel    5.77
#   c-ink-faint on c-wash     3.94  ← 只承载 ≥24px 的标签/来源/页码
#   c-tide      on c-wash     5.25     c-ink       on c-tide-wash 13.33
#   c-wash on c-buoy-green    4.27     c-wash on c-buoy-amber     4.05
#   c-wash on c-buoy-red      5.70
# 琥珀从 L0.660 压到 L0.600 换来 3.67 : wash —— 一支只在色相上成立、明度上读
# 不清的语义色，在色觉障碍与投影衰减下会整支失效。药丸上它承载的是反白文字，
# 那一对是 4.05。
# `c-rule` 与刻度底线是**线不是字**（§4.4 对比豁免第 2 条），不参与对比核算。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "潮痕石板 · 数据评审档 16:9")
