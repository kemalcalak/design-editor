#!/usr/bin/env python3
"""dossier-linen-deck.op — 卷宗亚麻 · 高密度文档备忘档（1920×1080 · 8 页）

deck 体系 D11。落格 high formality × **high** density × light。
定位：每一页都是一张可独立抽出来读的备忘录纸，不是幻灯片 —— 零 bullet、
编号段落、纯表格、异步阅读优先。

### 这一档要示范的那件事

**高密度 ≠ 缩字号。** 整套的正文密度是 deck 体系里最高的，而字号地板与其
他档完全相同（正文 30 是绝对下限）。密度来自三处，一处都不来自把字缩小：

  1. **行长**：正文块收窄到 900px（30px 字号下正好 30 汉字/行），一页能排
     下的段落数因此上去了，而每一行仍然好读。
  2. **段落结构**：层级只由段号深度表达（§1 / §1.1），零 bullet 符号 ——
     符号本身是要占位的，编号不占。
  3. **留白分配**：左边距 216 全给装订线，右边距只有 120。不对称让出的那
     96px 不是浪费，它是段号的悬挂缩进。

### 锚点与色彩推导（采样 → 收敛 → 论证）

  - **采样**：亚麻纹公文封的暖白、档案墨的近黑暖调、装订线绳的锈色。
  - **收敛**：底 = 亚麻 oklch(0.950 0.014 82)；**唯一有彩色 = 锈色 H35
    C0.090**，且只走线与页码；中性走暖灰序列（H60，与底同暖向）。
  - **论证一**：这是体系里唯一的单色档。备忘录的说服力来自结构不来自视
    觉；任何第二支颜色都会诱导读者去找「重点色」，而这一档的重点应该在句
    子里。
  - **论证二**：锈色 chroma 只到 0.090。拉到 0.13+ 装订线就从「绳」变成
    「红线标注」，语义完全改变。
  - **论证三**：底色 chroma 0.014 是亚麻纸的黄 —— 比奶油纸更灰、比新闻纸
    更暖，要读作「公文用纸」而不是「手账内页」。

### 招牌母题（识别度载体，每页复现且位置零漂移）

  1. **装订线 binding-rail** —— 距左缘 120px 一条贯穿全高的 2px 锈色竖
     线，线上等距三处 12×12 的方孔（亚麻底 + 1px 细线描边）。
  2. **编号段落 numbered-clause** —— mono 段号挂在装订线**左侧**的悬挂缩
     进里（右对齐至装订线 −24px），段落正文从装订线 +96px 起。
  3. **骑缝页标 folio-tab** —— 右上角 `p. N / 8` + 其下一条 48×1 细线；左
     上角对称位置放文档代号（letterSpacing +2）。

### 负约束（Strictly avoid，逐条对应 spec §2 D11）

  - **全篇零 bullet 符号**。只有编号段落、完整段落与带线表格。
  - **数据只走表格，绝不出图表** —— 图表在这一档是「演示」，而这一档不是
    演示。
  - 不用第二支有彩色；锈色之外一律中性。
  - 不用圆角、阴影、渐变、图标、照片、卡片。
  - 装订线与正文起点每页同一个 x，零漂移。
  - 段号最多三级。
  - 高密度不许触碰字号地板：正文 30 是绝对下限，装不下就删段落或加页。
  - 页标写实际总页数（`p. 3 / 8`），不写单侧页码。
  - 不写「如上页所述」这类依赖顺序的表述 —— 每页能被单独抽出来读。
  - 表格不用彩色表头、不用整框边线；只用上下横线与斑马行。

### 工程约束（spec §6，逐条踩过的坑）

  - **字体走保底层**：模板里只写 `Noto Sans SC`（中文）与 `Inter`（数字与
    拉丁）。spec 的首选字体（思源宋体 / 霞鹜文楷 / IBM Plex Mono）不在渲
    染字体包内，写了会静默回退到 Roboto，并触发「量 A 字体画 B 字体」的裁
    切与偏心。
  - **数值列宽写死**：Inter 在当前栈里不保证等宽数字，表格列宽一律固定
    px，不靠 fit_content 指望数字自己对齐。
  - **不用 line 节点、不用 dashPattern**：所有线（装订线、表格横线、签署
    位）都是 rectangle。
  - **相邻单元格留 4px 真格线**（`SIBLING_JAM_GAP = 3`）：行底色从缝隙透
    出来，既是几何合规也是这一档「不画竖格线」的实现方式。
  - **顶层一次性给出整棵 8 帧树**，每帧显式写 x/y（板位 2040 × 1440）。

字号与行高（§3.4/§3.5 硬地板：任意文本 ≥20，注释 ≥22，全 deck 最大 ≥60，
最大字号 ≥ 2.5 × 正文）：
  文档标题 76 / 1.10 / −2 · 节标题 44 / 1.25 · 正文 30 / 1.80
  表格 28 / 1.45 · 段号 mono 26 · 页标与脚注 mono 24 / 1.5
  76 = 2.53 × 30，正好过 §3.4 的层级比；spec §2 的 72 只到 2.40 倍，这里取
  76 是为了同时满足两条契约（差额写在这里，不是随手改的）。

对比度（WCAG 相对亮度比，实测见文件末尾）：承载正文的最低一对是 4.29。
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from oplib import Ids, color_vars, frame, rect, solid, text, write_doc

ids = Ids()

VARS = color_vars({
    "c-linen":      "#F3EEE4",   # 页面底 · 亚麻纸
    "c-band":       "#EFE9DE",   # 表格斑马行
    "c-deep":       "#E6DFD3",   # 引文块 / 决策块
    "c-ink":        "#201C19",   # 主文字        14.63 : linen
    "c-ink-soft":   "#56524E",   # 次级文字       6.70
    "c-ink-faint":  "#746F6B",   # 页标 / 段号    4.29
    "c-rule":       "#C0BDB8",   # 细线（非文字）
    "c-thread":     "#864737",   # 唯一有彩：装订线 / 页码 / 段号  6.11
})

CJK = "Noto Sans SC"
NUM = "Inter"

W, H = 1920, 1080
PAGES = 8
DOC_CODE = "OPS-2026-041"

# 版式语法。左边距 216 = 装订线 120 + 正文让出的 96，右边距只有 120 ——
# 这个不对称是本档最强的身份信号。
PAD_TOP, PAD_BOTTOM, PAD_RIGHT = 96, 96, 120
RAIL_X = 120                 # 装订线的 x，每页零漂移
NUM_W = 96                   # 悬挂段号列：右缘正好落在装订线左侧 24px
NUM_GAP = 120                # 段号列右缘 → 正文起点（= RAIL_X + 96 = 216）
BODY_W = W - PAD_RIGHT - NUM_W - NUM_GAP        # 1584
BODY_X = NUM_W + NUM_GAP                        # 216

# 正文块的行长闸。CJK 正文 ≤30 汉字/行（§3.8-3）：30px 字号下就是 900px。
# 内容宽有 1584，但「容器有多宽就排多宽」正是长段落最常见的失败。
TEXT_W = 900

FS_DOC, FS_SEC, FS_BODY = 76, 44, 30
# 段号、表头、页标、脚注**共用 26 这一档**。它们本来可以分成 26 / 24 两
# 档，但表格页上已经有 44 / 30 / 28 三档，再分就是单页 5 档，越过 §3.4 的
# 「单页字阶档数 ≤4」。26 仍落在注释档 24–26 的目标区间里。
FS_TABLE, FS_CLAUSE, FS_META = 28, 26, 26
LH_DISPLAY, LH_SEC, LH_BODY, LH_TABLE, LH_META = 1.10, 1.25, 1.80, 1.45, 1.50

GRID = 4        # 单元格之间的真格线；≥ SIBLING_JAM_GAP(3)


# ------------------------------------------------------------------ 骨架
def col(name, children, *, gap=0, width="fill_container", fill=None, **props):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="vertical", gap=gap, fill=fill or [], **props)
    node["children"] = children
    return node


def row(name, children, *, gap=0, align="start", width="fill_container",
        fill=None, **props):
    node = frame(ids, name, width=width, height="fit_content",
                 layout="horizontal", gap=gap, alignItems=align,
                 fill=fill or [], **props)
    node["children"] = children
    return node


def rule(width, thickness=1, color="$c-rule"):
    """一条横线。**只用 rectangle**：`line` 在 layout:none 下吃的是文档绝对
    坐标，会跑到画布的另一个角落去。"""
    return rect(ids, "细线", width=width, height=thickness, fill=solid(color))


# 拉丁与数字在 CJK 字体里约占半个字身。取 0.6 而不是 0.5 是留一档余量：
# 这个系数只决定断言的松紧，宁可早报一行也不要漏一行。
LATIN_EM = 0.6


def line_em(line):
    """一行文字的宽度，单位是「字」。CJK 记 1，其余记 LATIN_EM。"""
    return sum(1.0 if ch > "⿿" else LATIN_EM for ch in line)


def fits(content, size, width):
    """产稿期的折行断言：改文案当场报错，不必跑完闸再回来猜哪一行折了。

    量的是**渲染真实宽度**（拉丁半宽），不是 cjkcheck 的 1em 保守模型 ——
    后者会把「上季度共 1342 条上报……」这种混排句判成折行，而它实际排得
    下。两者是互补的：这里守「不许发生自动折行」，闸守「万一折了会不会出
    孤字末行 / 行首标点」。

    断点一律写死在文案里，所以每一硬行都要自己排得下。
    """
    if not isinstance(width, (int, float)) or not isinstance(content, str):
        return
    if not content.strip():
        return
    per = width / size
    for line in content.split("\n"):
        if line_em(line) > per:
            raise ValueError(
                f"断行超容量：{line!r} 折合 {line_em(line):.1f} 字 > 每行 "
                f"{per:.1f} 字（宽 {width} · 字号 {size}）。按 §3.1 缩短文"
                f"案、换页型或拆页，不缩字号。")


def body(content, *, size=FS_BODY, weight=400, color="$c-ink",
         width=TEXT_W, lh=LH_BODY, family=CJK, spacing=0, align=None,
         name="正文"):
    """一段正文。断点一律写死在文案里 —— 当前 wrap_text 对 CJK 是逐字符断
    行、没有标点避头尾，交给自动折行会在「。」前面断开。"""
    fits(content, size, width)
    return text(ids, name, content, size, weight, color, family=family,
                line_height=lh, width=width, growth="fixed-width",
                align=align, spacing=spacing)


def clause(number, children):
    """一个编号段：mono 段号悬挂在装订线左侧，正文从装订线 +96 起。

    段号的行盒高度对齐正文首行（26 × lh = 30 × 1.8），否则段号会比它领的
    那一行高出几个像素，八页看下来是可见的抖动。
    """
    num = text(ids, "段号", number, FS_CLAUSE, 500, "$c-thread", family=NUM,
               line_height=round(FS_BODY * LH_BODY / FS_CLAUSE, 3),
               width=NUM_W, growth="fixed-width", align="right")
    return row("编号段", [num, col("段落", children, gap=18)], gap=NUM_GAP)


def section(number, title, children, *, gap=28):
    """节标题 + 其下的内容。节号同样走悬挂缩进，层级只由段号深度表达。

    `children` 里的编号段自己带悬挂列，**这里不再统一缩进** —— 缩两次的结
    果是段号被推进正文里，悬挂缩进就没有了。不带段号的块（表格、引文块、
    决策块）由调用方用 `indent()` 显式让出 216。
    """
    num = text(ids, "节号", number, FS_CLAUSE, 500, "$c-thread", family=NUM,
               line_height=round(FS_SEC * LH_SEC / FS_CLAUSE, 3),
               width=NUM_W, growth="fixed-width", align="right")
    fits(title, FS_SEC, BODY_W)
    head = text(ids, "节标题", title, FS_SEC, 600, "$c-ink", family=CJK,
                line_height=LH_SEC, width=BODY_W, growth="fixed-width")
    return col("节", [
        row("节标题行", [num, head], gap=NUM_GAP),
        col("节内容", children, gap=gap),
    ], gap=gap)


def indent(children, *, gap=0, **props):
    """没有段号的块。用 padding 让出 216，不塞一个空的占位节点进来。"""
    return col("正文块", children, gap=gap, padding=[0, 0, 0, BODY_X], **props)


def meta_line(label, value, *, value_family=NUM):
    """收件人 / 日期 / 密级这类元信息行。中文标签走 CJK、值走西文族 ——
    让中文字体去渲染数字会字宽不齐，列就抖了。"""
    return row("元信息行", [
        text(ids, "元信息标签", label, FS_META, 400, "$c-ink-faint",
             family=CJK, width="fit_content", growth="auto",
             line_height=LH_META),
        text(ids, "元信息值", value, FS_META, 500, "$c-ink-soft",
             family=value_family, width="fit_content", growth="auto",
             line_height=LH_META),
    ], gap=16, align="start")


# ------------------------------------------------------------------ 骑缝页标
def folio(index):
    """右上角页标 + 左上角文档代号。两者共同构成「这是一份文件的第 N 页」。"""
    code = text(ids, "文档代号", DOC_CODE, FS_META, 500, "$c-ink-faint",
                family=NUM, width="fit_content", growth="auto",
                line_height=LH_META, spacing=2)
    page_no = text(ids, "页标", f"p. {index} / {PAGES}", FS_META, 500,
                   "$c-thread", family=NUM, width="fit_content",
                   growth="auto", line_height=LH_META)
    tab = col("骑缝页标", [page_no, rule(48)], gap=10, width="fit_content",
              alignItems="end")
    # 页眉与正文同起点（装订线 +96），否则文档代号会一路顶到画布左缘去。
    return indent([
        row("页眉", [code, tab], gap=48, width="fill_container",
            justifyContent="space_between"),
    ])


# ------------------------------------------------------------------ 装订线
def binding_rail():
    """装订线 + 三处装订孔。孔写在数组前面 —— layout:none 的 z-order 是
    children[0] 在最上，写反了孔就被线盖住。"""
    holes = []
    for y in (270, 540, 810):
        hole = rect(ids, "装订孔", width=12, height=12,
                    fill=solid("$c-linen"),
                    stroke={"thickness": 1, "fill": solid("$c-rule")})
        hole["x"], hole["y"] = RAIL_X - 5, y - 6
        holes.append(hole)
    thread = rect(ids, "装订线", width=2, height=H, fill=solid("$c-thread"))
    thread["x"], thread["y"] = RAIL_X, 0
    return [*holes, thread]


def page(name, index, children, *, gap=40, justify="start"):
    """一页备忘录纸。固定 1920×1080，绝不 fit_content。"""
    content = frame(ids, f"{name} · 正文", width="fill_container",
                    height="fill_container", layout="vertical",
                    padding=[PAD_TOP, PAD_RIGHT, PAD_BOTTOM, 0], gap=gap,
                    justifyContent=justify, fill=[])
    content["children"] = [folio(index), *children]
    ornament = frame(ids, f"{name} · 装订", width="fill_container",
                     height="fill_container", layout="none", fill=[])
    ornament["children"] = binding_rail()
    shell = frame(ids, name, width=W, height=H, layout="none",
                  fill=solid("$c-linen"), clipContent=True)
    shell["children"] = [content, ornament]
    return shell


# ------------------------------------------------------------------ 带线表格
def cell(content, width_px, *, weight=400, color="$c-ink", family=CJK,
         size=FS_TABLE, align=None):
    """一个单元格。底色留给行，单元格自己透明 —— 这一档不画竖格线，格与
    格之间靠 4px 的真缝隙分开。

    那 4px 只负责**分格**；两列文字之间真正的呼吸量是两侧各 24 的内边距
    （文字到文字 52px），不是靠 4px 贴着 SIBLING_JAM_GAP 的下限过关。
    """
    fits(content, size, width_px - 48)
    box = frame(ids, "单元格", width=width_px, height="fit_content",
                layout="vertical", padding=[16, 24], gap=0, fill=[])
    box["children"] = [
        text(ids, "单元格文字", content, size, weight, color, family=family,
             line_height=LH_TABLE, width="fill_container",
             growth="fixed-width", align=align),
    ]
    return box


def table(columns, header, rows, *, emphasis=None):
    """带线表格：只有上下横线与斑马行，没有彩色表头、没有整框边线。

    `columns` 是 (宽度, 对齐, 字族) 三元组，宽度**写死** —— 渲染栈没有
    tabular 数字，指望 fit_content 让数字自己对齐必然抖。
    `emphasis` 给最后一行（推荐行 / 合计行）加粗并在其上加一条 2px 锈线。
    """
    assert sum(w for w, _, _ in columns) + GRID * (len(columns) - 1) == BODY_W

    def line_row(cells, fill=None):
        return row("表行", cells, gap=GRID, align="stretch", fill=fill)

    head_cells = [
        cell(label, w, weight=600, color="$c-ink-soft", family=CJK,
             size=FS_CLAUSE, align=align)
        for label, (w, align, _) in zip(header, columns)
    ]
    out = [rule(BODY_W, 2), line_row(head_cells), rule(BODY_W, 1)]
    for index, values in enumerate(rows):
        last = emphasis and index == len(rows) - 1
        if last:
            out.append(rule(BODY_W, 2, "$c-thread"))
        cells = [
            cell(value, w, weight=600 if last else 400, family=family,
                 align=align)
            for value, (w, align, family) in zip(values, columns)
        ]
        band = None if last else (solid("$c-band") if index % 2 == 0 else None)
        out.append(line_row(cells, fill=band))
    out.append(rule(BODY_W, 1))
    return col("表格", out, gap=0)


# ------------------------------------------------------------------ 01 首页
SUMMARY = ("巡检是我们唯一能在事故发生前拿到的一手信息。\n"
           "过去两年它以人工抽查的方式运行，覆盖到的门店不足四成，\n"
           "上报的问题在纸质表单与工作群之间流转，闭环时长无从统计。\n"
           "本备忘给出三个方案的取舍、所需投入，\n"
           "以及三项需要在本月内批复的事项。")


def cover():
    title = body("把门店巡检从抽查\n改为全量记录", size=FS_DOC, weight=600,
                 lh=LH_DISPLAY, spacing=-2, width=BODY_W, name="文档标题")
    return page("01 文件首页", 1, [
        indent([
            # 起首线在标题**上方**：它是信笺抬头与正文的分界，不是「每个标
            # 题下面一条横线」那条 AI 指纹。
            rule(BODY_W, 2, "$c-thread"),
            title,
            body(SUMMARY, name="摘要"),
            col("签发信息", [
                meta_line("收件人", "运营中心 · 门店运营组 · 信息技术部",
                          value_family=CJK),
                meta_line("签发日期", "2026-08-09"),
                meta_line("密级", "内部 · 限项目组", value_family=CJK),
            ], gap=12),
        ], gap=44),
    ], gap=40, justify="space_between")


# ------------------------------------------------------------------ 02 背景
def background():
    return page("02 背景", 2, [
        section("§1", "背景与问题", [
            clause("§1.1", [body(
                "人工抽查按片区轮换，每家门店平均 47 天才被巡到一次。\n"
                "到店的检查项由巡检员当场手写，回办公室再录入系统，\n"
                "两个环节之间平均隔了 1.8 天，照片与表单常常对不上号。\n"
                "抽查本身没有错，错的是把抽查的结果当成了全量的结论。")]),
            clause("§1.2", [body(
                "问题上报走工作群，群里同时还有排班、报修与日常通知。\n"
                "信息一旦被后面的消息顶下去，就等于没有发生过。\n"
                "上季度共 1342 条上报，能追溯到闭环记录的只有 611 条。")]),
            clause("§1.3", [body(
                "各区域对同一个检查项的判定尺度并不一致：\n"
                "华东把冷链温度超标记为一般问题，华南记为严重问题，\n"
                "汇总到集团时，两者被加在了同一个数字里。")]),
        ], gap=32),
    ], gap=48)


# ------------------------------------------------------------------ 03 数据
REGION_COLS = [
    (388, None, CJK),      # 片区
    (300, "right", NUM),   # 月均巡检次数
    (280, "right", NUM),   # 问题上报条数
    (280, "right", NUM),   # 整改闭环率
    (320, "right", NUM),   # 平均闭环时长
]
REGION_HEAD = ["片区", "月均巡检次数", "上报条数", "闭环率（%）",
               "闭环时长（天）"]
REGION_ROWS = [
    ["华东", "412", "386", "46", "5.2"],
    ["华南", "358", "341", "51", "4.6"],
    ["华北", "296", "302", "39", "6.8"],
    ["西南", "187", "154", "44", "5.9"],
    ["东北", "121", "98", "33", "8.1"],
    ["全国", "1374", "1281", "44", "5.9"],
]


def facts():
    return page("03 现状数据", 3, [
        section("§2", "现状：五个片区的四项读数", [
            indent([
                table(REGION_COLS, REGION_HEAD, REGION_ROWS, emphasis=True),
                body("数据来源：巡检系统 2026 年第二季度导出；判定口径见 "
                     "§1.3。闭环率按「有整改记录且经复核」计，与月报一致。",
                     size=FS_META, color="$c-ink-faint", lh=LH_META,
                     width=BODY_W, name="来源行"),
            ], gap=24),
            clause("§2.1", [body(
                "巡检次数与上报条数几乎同步下降，闭环率却没有跟着走：\n"
                "东北片区巡到的次数最少，闭环时长反而最长 8.1 天。\n"
                "这说明瓶颈不在巡得够不够勤，而在巡完之后没人接得住。")]),
        ], gap=32),
    ], gap=40)


# ------------------------------------------------------------------ 04 分析
def analysis():
    quote = col("引文块", [
        body("我们不缺巡检，缺的是能被复算的巡检。",
             size=FS_BODY, weight=600, width=1100, lh=1.5, name="引文"),
        body("—— 华东区运营负责人，6 月复盘会记录", size=FS_META,
             color="$c-ink-soft", lh=LH_META, width=1100, name="引文出处"),
    ], gap=14, fill=solid("$c-deep"), padding=[32, 40], width=1200)
    return page("04 分析", 4, [
        section("§3", "为什么抽查救不回来", [
            clause("§3.1", [body(
                "抽查的样本由排班决定，而排班避开了最难去的那些门店。\n"
                "越是偏远、越是人手紧的门店，被巡到的间隔越长，\n"
                "于是问题最多的地方，恰好是我们了解得最少的地方。\n"
                "这不是执行不力，是抽样方法自带的偏差。")]),
            clause("§3.2", [body(
                "闭环率停在 44%，不是因为没人整改，\n"
                "而是因为整改完成后没有一个必须回填的动作。\n"
                "没有回填，就没有记录；没有记录，复算就无从谈起。")]),
            indent([quote]),
        ], gap=28),
    ], gap=40)


# ------------------------------------------------------------------ 05 方案
PLAN_COLS = [
    (384, None, CJK),
    (396, None, CJK),
    (396, None, CJK),
    (396, None, CJK),
]
PLAN_HEAD = ["评估维度", "甲 · 扩编巡检员", "乙 · 外包第三方", "丙 · 自检加抽核"]
PLAN_ROWS = [
    ["覆盖率", "提升有限", "可达九成", "全量覆盖"],
    ["单店月成本", "高", "中", "低"],
    ["落地周期", "三个月", "六周", "十周"],
    ["数据一致性", "依赖个人经验", "依赖合同条款", "由系统约束"],
    ["主要风险", "招聘周期偏长", "数据不在自己手里", "需要门店配合"],
    ["结论", "不建议", "备选", "推荐"],
]


def options():
    return page("05 方案对比", 5, [
        section("§4", "三个方案，按同一组维度逐行回答", [
            indent([
                table(PLAN_COLS, PLAN_HEAD, PLAN_ROWS, emphasis=True),
                body("推荐方案丙。它是三者里唯一让「记录」成为流程副产品的"
                     "一个，其余两个都还要靠人记得去做。",
                     size=FS_META, color="$c-ink-soft", lh=LH_META,
                     width=BODY_W, name="推荐说明"),
            ], gap=24),
            clause("§4.1", [body(
                "成本按 240 家门店试算：甲需增编 11 人，年支出约 190 万；\n"
                "乙的报价随门店数线性上涨，丙在第二年起低于两者的一半。")]),
        ], gap=32),
    ], gap=40)


# ------------------------------------------------------------------ 06 叙述
def prospect():
    signature = col("署名引语", [
        body("把巡检做成一件不需要提醒的事，\n是这次改造真正的目标。",
             size=FS_BODY, weight=500, color="$c-ink-soft", width=1100,
             lh=1.6, name="引语"),
        body("—— 项目组，2026 年 8 月", size=FS_META, color="$c-ink-faint",
             lh=LH_META, width=1100, name="引语署名"),
    ], gap=12, padding=[0, 0, 0, 36])
    bar = rect(ids, "引语竖线", width=2, height=132, fill=solid("$c-thread"))
    return page("06 逆向叙述", 6, [
        section("§5", "若此事已成", [
            clause("§5.1", [body(
                "到明年三月，每家门店在每个自然周都会留下完整的巡检记录。\n"
                "记录不再依赖谁记得拍照，而是由流程本身产生：\n"
                "检查项没有勾完，当天的巡检就不算结束。\n"
                "区域经理在周一早晨看到的不是一份汇总表，\n"
                "而是上周所有未闭环问题按门店排开的清单。\n"
                "我们不再讨论覆盖率，因为覆盖率恒等于百分之百；\n"
                "要讨论的是闭环时长，以及它为什么在某些门店偏长。")]),
            indent([row("署名行", [bar, signature], gap=0, align="start")]),
        ], gap=32),
    ], gap=40, justify="center")


# ------------------------------------------------------------------ 07 疑问
QA = [
    ("问：全量记录会不会把巡检变成打卡？",
     "不会。检查项从 42 项压到 18 项，每项都要有照片或读数支撑，\n"
     "勾选式的应付会在抽核环节被直接退回。"),
    ("问：门店自检的结果可信吗？",
     "抽核比例定在 15%，由区域经理随机指定，结果与自检并列存档。\n"
     "连续两次抽核不一致的门店，恢复为人工巡检。"),
    ("问：需要门店多花多少时间？",
     "单次自检 12 分钟，比现行的纸质表单少 4 分钟。\n"
     "时间省在录入上，不省在检查上。"),
    ("问：这套记录会被用来考核门店吗？",
     "前两个季度只用于发现问题，不进考核。\n"
     "考核口径需要另行讨论，不在本备忘的范围内。"),
]


def questions():
    blocks = []
    for index, (question, answer) in enumerate(QA):
        if index:
            blocks.append(indent([rule(BODY_W, 1)]))
        blocks.append(clause(f"§6.{index + 1}", [
            body(question, weight=600, lh=1.4, name="问"),
            body(answer, color="$c-ink-soft", lh=1.65, name="答"),
        ]))
    return page("07 疑问", 7, [
        section("§6", "已经被问到的四个问题", blocks, gap=18),
    ], gap=28)


# ------------------------------------------------------------------ 08 决议
DECISIONS = [
    ("批准方案丙，在华东与华南两区先行试点。",
     "运营中心 · 周敏", "2026-08-20"),
    ("批准巡检系统改造预算 86 万元，分两期拨付。",
     "财务部 · 陈立", "2026-08-27"),
    ("批准抽核比例与退回规则写入门店运营手册。",
     "门店运营组 · 李哲", "2026-09-10"),
]


def resolution():
    items = []
    for index, (what, who, due) in enumerate(DECISIONS):
        if index:
            items.append(rule(1200 - 80, 1))
        items.append(col("待批准事项", [
            row("事项行", [
                text(ids, "事项序号", f"{index + 1}", FS_CLAUSE, 500,
                     "$c-thread", family=NUM, width=40, growth="fixed-width",
                     line_height=round(FS_BODY * 1.5 / FS_CLAUSE, 3)),
                body(what, width=1000, lh=1.5, name="事项"),
            ], gap=16),
            # 中文标签走 CJK、日期走西文族，各自一个节点：把
            # 「期限 2026-08-20」写成一个 Inter 节点，中文会掉进回退字体，
            # 同一行里出现两种字面（实测可见）。
            row("责任行", [
                text(ids, "决策人", f"决策人 {who}", FS_META, 400,
                     "$c-ink-soft", family=CJK, width="fit_content",
                     growth="auto", line_height=LH_META),
                text(ids, "期限标签", "期限", FS_META, 400, "$c-ink-soft",
                     family=CJK, width="fit_content", growth="auto",
                     line_height=LH_META),
                text(ids, "期限日期", due, FS_META, 500, "$c-ink-soft",
                     family=NUM, width="fit_content", growth="auto",
                     line_height=LH_META),
            ], gap=12, padding=[0, 0, 0, 56]),
        ], gap=10))
    block = col("决策块", items, gap=20, fill=solid("$c-deep"),
                padding=[36, 40], width=1200)
    sign = col("签署位", [
        rule(480, 1),
        row("签署标签", [
            text(ids, "签署", "批准签署", FS_META, 400, "$c-ink-faint",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=LH_META),
            text(ids, "签署日期", "日期", FS_META, 400, "$c-ink-faint",
                 family=CJK, width="fit_content", growth="auto",
                 line_height=LH_META),
        ], gap=280),
    ], gap=12, width=480)
    return page("08 决议", 8, [
        section("§7", "待批准事项", [
            indent([block, sign], gap=32),
            # 末页是**陈述**不是信息：把开头立起来的那件事在这里收掉，而不
            # 是停在一张待办清单上。
            clause("§7.1", [body(
                "三项批复完成后，本备忘归档为定稿并抄送三个部门；\n"
                "未获批复的部分按原流程继续执行，下一次复核安排在 11 月。")]),
        ], gap=32),
    ], gap=36)


# ------------------------------------------------------------------ 板位
# 3 板一行。行距 1440（而不是 1200）：画布在帧上方以固定屏幕偏移画帧名，
# 在 3 宽 deck 铺满屏幕的缩放下，120 文档 px 的行距只合约 16 屏幕 px，第二
# 行的帧名会压到上一行的板上。
BOARD_STEP_X, BOARD_STEP_Y, PER_ROW = 2040, 1440, 3


def build():
    boards = [cover(), background(), facts(), analysis(),
              options(), prospect(), questions(), resolution()]
    for index, board in enumerate(boards):
        board["x"] = (index % PER_ROW) * BOARD_STEP_X
        board["y"] = (index // PER_ROW) * BOARD_STEP_Y
    return boards


# 对比度（WCAG 相对亮度比；投影场景正文自设门槛 4.5）：
#   c-ink       on c-linen / c-band / c-deep   14.63 / 14.00 / 12.77
#   c-ink-soft  on c-linen / c-band / c-deep    6.70 /  6.41 /  5.85
#   c-ink-faint on c-linen / c-band             4.29 /  4.11   ← 最低一对
#   c-thread    on c-linen / c-deep             6.11 /  5.34
# c-ink-faint 只承载页标、脚注与元信息标签（≥24px 的注释档），4.29 高于大
# 字 3.0 的地板；正文一律走 c-ink 或 c-ink-soft。
# c-rule 不参与：它是非文字装饰线（高 ≤2px），按 §4.4 的豁免规则排除。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, build(), "卷宗亚麻 · 文档备忘 deck")
