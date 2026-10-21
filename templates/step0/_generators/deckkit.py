"""D7/D8 两套 deck 的共享度量层 —— 画幅、板位、字阶行高、CJK 行长、虚线。

来源是 `openpencil-docs/openpencil/generation/deck-system-0809.md` 的 §3
（deck 通用契约）与 §6（工程约束移交）。cardlib 是卡片体系的度量层、只被
tpl_*_card.py 引用；这一层是 **deck 体系专用**，只被 deck_*.py 引用。两者
都建在 oplib 的节点构造层之上，互不依赖。

把它单独拆出来而不是各自抄一遍，是因为 §3 的三条铁律里有两条（字号地板、
CJK 行长）是**算出来的数**而不是记住的数：抄第二遍就会有第二个版本。

### spec 的两处算术需要说明

§2 通用前提写「D7 12 列 × 128 + 11 沟槽 × 24 = 1680」与「D8 12 列 × 132 +
11 沟槽 × 24 = 1728」。两式都不自洽（12×128+264 = 1800、12×132+264 = 1848）。
可信的是**内容宽**（由边距推出：1920−240 = 1680、1920−192 = 1728）与**沟槽
24**，列宽是被这两个数推出来的余数：D7 col = (1680−264)/12 = 118，D8 col =
(1728−264)/12 = 122。本文件按内容宽与沟槽算列，不用 spec 写的列宽字面值。
"""

# ---------------------------------------------------------------- 画幅与板位
#
# 1920×1080 固定舞台。投影比例是硬约束，任何一页 fit_content 都会让这套 deck
# 在放映时变成不同尺寸的纸。
W, H = 1920, 1080

# §6.15：3 板一行，x 步长 2040、y 步长 1440。y 步长比 x 多 240 不是手滑 ——
# 画布在帧上方以**屏幕空间**固定偏移画帧名，deck 缩到能整屏看时 120 文档像素
# 只剩十几个屏幕像素，第二行的帧名会压到上一行的板上。
BOARDS_PER_ROW = 3
BOARD_STRIDE_X = 2040
BOARD_STRIDE_Y = 1440

CJK = "Noto Sans SC"
NUM = "Inter"


def place_boards(boards):
    """给每页写死 x/y。

    顶层 frame 缺 x/y 时全部落在原点，七页会叠成一页、另外六页藏在下面。
    """
    for index, board in enumerate(boards):
        board["x"] = (index % BOARDS_PER_ROW) * BOARD_STRIDE_X
        board["y"] = (index // BOARDS_PER_ROW) * BOARD_STRIDE_Y
    return boards


# ---------------------------------------------------------------- 行高
#
# §3.5 的行高按**字号**分段，不按「标题/正文」分类 —— 这正是 §5.3 C1 要修的
# 那处语料冲突：1.3–1.4 对 24–40px 的界面标题正确，对 88–116px 的 deck
# display 会撑出一整行的空洞。
LH_DISPLAY_XL = 1.08   # ≥88px
LH_DISPLAY = 1.12      # 64–87px
LH_TITLE = 1.20        # 48–63px
LH_SUBTITLE = 1.35     # 40–47px
LH_BODY = 1.75         # CJK 正文（西文 1.5–1.6 → 中文 +0.2）
LH_NOTE = 1.50         # 注释 / 来源 / 页码


def lh(size, *, body=False):
    """按字号取行高。`body=True` 表示这块是成段中文，走 1.75 而不是标签行高。"""
    if size >= 88:
        return LH_DISPLAY_XL
    if size >= 64:
        return LH_DISPLAY
    if size >= 48:
        return LH_TITLE
    if size >= 40:
        return LH_SUBTITLE
    if body:
        return LH_BODY if size >= 28 else LH_NOTE
    return 1.40


# ---------------------------------------------------------------- 字距
#
# §3.8.2：我们的 schema 里 letterSpacing 是 **px 绝对值不是 em**。
#   <48px  一律 0，绝不为负 —— 负字距在 CJK 正文上直接让字面相撞。
#   ≥48px  允许到 |letterSpacing| <= fontSize × 0.02（即 -0.02em），可带小数。
# 大写拉丁小标签（不含汉字）允许 +1~+2 的正字距；含汉字的一律 0。
#
# 判定用**比值**，不要先把上限 round 掉：72px 的上限是 1.44，-1.4 合法、-2 不合法。
# 先 round 会把 -1.4 误判成越界（round(-1.44) = -1），那是规则的 bug 不是稿子的。
# 分界只留 48 一道：比值本身在小字号上就是自限的（48px 只允许 0.96px，不到一个
# 像素），再加一道 64 的门槛只是多一个阈值，并不多挡住什么。
DISPLAY_TRACK = -0.02
TRACK_MIN_SIZE = 48
TRACK_EPS = 1e-6


def track(size):
    """display 档的负字距上限（含小数）。小于 48px 一律 0。"""
    return round(size * DISPLAY_TRACK, 2) if size >= TRACK_MIN_SIZE else 0


def latin_track(content, value=1.0):
    """纯拉丁/数字小标签才给正字距；混了汉字就归零。"""
    return value if all(ch < "⺀" for ch in content) else 0


def has_han(content):
    """文案是否含汉字。负字距上限只约束 CJK，拉丁/数字节点豁免。"""
    return isinstance(content, str) and any("一" <= ch <= "鿿" for ch in content)


# ---------------------------------------------------------------- CJK 行长
#
# §3.8.3：CJK 正文每行 ≤30 汉字。1680 的内容宽在 32px 字号下是 52 字/行，
# **远超舒适区** —— 所以正文块必须自己收窄，不能因为容器有那么宽就排那么宽。
# 这是本体系里唯一一条「容器宽 ≠ 文本宽」的硬规则。
CJK_MAX_CHARS = 30


def cjk_width(size, cap=None):
    """给定字号下，一行不超过 30 汉字的最大文本宽。`cap` 是容器给的上限。"""
    limit = size * CJK_MAX_CHARS
    return min(limit, cap) if cap else limit


# ---------------------------------------------------------------- 字号地板
#
# §3.4 的硬 FAIL 线。这里只做断言用 —— 产稿时把每一个字号过一遍，比渲染完
# 再看检测器报告便宜得多。
FONT_HARD_FLOOR = 20
FONT_WARN_FLOOR = 28
DECK_MIN_MAX_FONT = 60


def assert_type_scale(sizes, *, where):
    """字号地板自检：任何 <20 直接抛；全 deck 最大字号 <60 也抛。

    「全 deck 最大字号 <60」查的不是某一个节点，是**层级缺失** —— 一份连一个
    60px 以上标题都没有的 deck，说明它被平铺成了文档。
    """
    for size in sizes:
        if size < FONT_HARD_FLOOR:
            raise ValueError(f"{where}: {size}px < {FONT_HARD_FLOOR} 硬地板")
    if max(sizes) < DECK_MIN_MAX_FONT:
        raise ValueError(f"{where}: 最大字号 {max(sizes)} < {DECK_MIN_MAX_FONT}")
    body = min(s for s in sizes if s >= FONT_WARN_FLOOR)
    if max(sizes) < body * 2.5:
        raise ValueError(f"{where}: 最大字号 {max(sizes)} < 2.5 × 正文 {body}")
    return sizes


# ---------------------------------------------------------------- 虚线
#
# §6.1：`dashPattern` 是死字段，schema 里有、渲染栈不认。写了只会得到一条实线
# 或者什么都没有。所有虚线必须用**一串短 rectangle** 拼。
#
# 拼法用 flex 而不是绝对坐标：一串等距短段正好是「gap 固定的一行/一列」，
# 交给布局算比自己写 N 个 x 少一个出错面。横向那一版按需再加：本批只有
# D8 的月份分隔用得上竖虚线。


def dash_col(rect_fn, ids, name, color, *, length, dash=8, gap=6,
             thickness=1):
    """竖直虚线：一列等距短段，交给 flex 排。"""
    from oplib import frame, solid
    count = max(1, (length + gap) // (dash + gap))
    node = frame(ids, name, width="fit_content", height="fit_content",
                 layout="vertical", gap=gap, alignItems="center", fill=[])
    node["children"] = [
        rect_fn(ids, f"{name} · 段", width=thickness, height=dash,
                fill=solid(color))
        for _ in range(count)
    ]
    return node


# ---------------------------------------------------------------- 产稿期自检
#
# 这一节是把「事后 grep 核对负约束」提前成**生成期断言**。差别不在勤快：
# grep 是一次性的，改一句文案没人会重跑；断言挂在 build() 上，任何一次改稿
# 都必须先过它。同理，CJK 折行也不该等 cjkreal 闸来报 —— 闸要跑 MCP、要渲染，
# 一轮几十秒；同一个折行模型写进这里，改文案当场就红。
#
# 三条判据的来源：
#   · 负约束     spec §2 各档 Strictly avoid + §3.7 的 AI-slop 清单
#   · 折行模型   qa/cjkcheck.py（每行容量 = 文本宽 // 字号，同时试 per 与 per+1）
#   · 字距/字族  §3.8

NO_LINE_START = "，。、；：？！）」』】》〉·…—"
ALLOWED_FAMILIES = {"Noto Sans SC", "Inter"}
# 「汉字占比 <0.6 就不按 1em/字 估」与 cjkcheck 同口径：拉丁半宽，混排串按
# 全宽算会得到一堆假阳性。
CJK_RATIO_GATE = 0.6


def _flatten(node, out):
    if isinstance(node, dict):
        out.append(node)
        for kid in node.get("children") or []:
            _flatten(kid, out)
    elif isinstance(node, list):
        for kid in node:
            _flatten(kid, out)
    return out


def _char_em(ch):
    """一个字符占几个 em。

    cjkcheck 按「1em 一个字」估，那是**故意保守**的：它是闸的第一道，宁可多
    报也不漏，误报再交给 cjkreal 用真实渲染高度筛掉（gate.sh 记的实测是 13 报
    → 11 真）。产稿期这道断言不能这么干 —— 它会在中英混排的句子上无差别拦
    人，逼着为一个并不存在的折行去改文案。

    所以这里按真实字宽建模：CJK 全宽、拉丁与数字约半宽。U+2E80 以下走 0.55，
    以上（含 U+FF0C 这类全角标点）走 1.0。
    """
    return 0.55 if ch < "⺀" else 1.0


def _wrap(content, width_em):
    """按字宽贪心折行，显式 \\n 强制断行。返回 [(行, 是否引擎折的)]。

    `soft` 这一位是关键：作者写 \\n 让某几个字独占一行是版式意图，引擎把
    「。」甩到下一行才是事故。孤字规则只对 soft 行成立。
    """
    lines = []
    for hard in content.split("\n"):
        if not hard:
            lines.append(("", False))
            continue
        line, used, soft = "", 0.0, False
        for ch in hard:
            em = _char_em(ch)
            if line and used + em > width_em:
                lines.append((line, soft))
                line, used, soft = ch, em, True
            else:
                line += ch
                used += em
        lines.append((line, soft))
    return lines


def audit_document(children, *, where, allowed_radii=frozenset()):
    """产稿期终审。任何一条不过就抛，绝不产出半合格的 .op。

    `allowed_radii` 是这一档**显式批准**的圆角值集合。正圆（cornerRadius 恰为
    宽高之半且宽高相等）永远放行 —— 「不用圆角」约束的是圆角矩形容器，圆点
    要圆就必须写这个字段，两者不是一回事。
    """
    nodes = _flatten(children, [])
    problems = []

    for node in nodes:
        name = node.get("name", node.get("id", "?"))

        # —— 负约束：这些字段一旦出现，档位身份就变了
        for dead in ("shadow", "opacity", "dashPattern"):
            if dead in node:
                problems.append(f"{name}: 出现禁用字段 {dead}")
        if node.get("type") == "line":
            problems.append(f"{name}: 用了 line 节点（§6.3 一律走 rectangle）")
        for paint in (node.get("fill") or []):
            if isinstance(paint, dict):
                if "gradient" in str(paint.get("type", "")):
                    problems.append(f"{name}: 出现渐变填充")
                color = paint.get("color")
                if isinstance(color, str) and color.startswith("#"):
                    problems.append(f"{name}: 写死 hex {color}，未走设计变量")

        # —— 圆角：只放行本档批准的值与正圆
        radius = node.get("cornerRadius")
        if radius:
            w, h = node.get("width"), node.get("height")
            round_dot = (isinstance(w, (int, float)) and w == h
                         and abs(radius - w / 2) < 0.51)
            if not round_dot and radius not in allowed_radii:
                problems.append(f"{name}: cornerRadius={radius} 不在本档批准的 "
                                f"{sorted(allowed_radii) or '零圆角'} 内，也不是正圆")

        if node.get("type") != "text":
            continue

        size = node.get("fontSize")
        content = node.get("content")
        width = node.get("width")

        if node.get("fontFamily") not in ALLOWED_FAMILIES:
            problems.append(f"{name}: 字族 {node.get('fontFamily')} 不在保底两支内")
        if size < FONT_HARD_FLOOR:
            problems.append(f"{name}: {size}px < {FONT_HARD_FLOOR} 硬地板")

        # —— fit_content 与 growth 必须成对。配成 fixed-width 时 jian 会把节点
        # 压得比内容还窄，文字折出孤字末行 —— 而且它在 audit 上是干净的，
        # 只有 CJK 闸或出图才看得见。
        auto = node.get("textGrowth") == "auto"
        if (width == "fit_content") != auto:
            problems.append(f"{name}: width={width!r} 与 growth="
                            f"{node.get('textGrowth')!r} 不成对")

        # —— 字距：<48px 一律 0 且绝不为负；≥48px 不得比 -0.02em 更负。
        # 只管含汉字的节点：拉丁/数字的 display 用 -0.03~-0.05em 是正常排印，
        # 拿 CJK 的上限去卡页码和 Stat Value 是误伤。
        spacing = node.get("letterSpacing", 0)
        if spacing < 0 and has_han(content) and (
            size < TRACK_MIN_SIZE or abs(spacing) > size * -DISPLAY_TRACK + TRACK_EPS
        ):
            problems.append(f"{name}: {size}px 的负字距 {spacing} 越界")

        if not isinstance(content, str) or not isinstance(width, (int, float)):
            continue
        han = sum(1 for ch in content if "一" <= ch <= "鿿")
        if not content.strip() or han / len(content) < CJK_RATIO_GATE:
            continue

        # —— 行长 ≤30 汉字。量的是「这个宽度最多能排几个全宽字」，与实际内容
        # 无关 —— 它约束的是版式，不是这一句文案。
        #
        # 只对正文档（<48px）成立。§3.8.3 那条规则解决的是「读到行尾找不回下一
        # 行开头」，那是成段文字的问题；display 档本来就只有十几个字、一到两
        # 行，把它按 30 字卡宽只会逼出一堆无意义的定宽。
        capacity = width / size
        if size < 48 and capacity > CJK_MAX_CHARS:
            problems.append(f"{name}: 每行可排 {capacity:.0f} 字 > "
                            f"{CJK_MAX_CHARS} 上限")

        # —— 折行模型：容量本身与再紧一档都要干净。末字标点悬挂会让实际每行多
        # 塞进小半个字，只按标称容量试会漏掉正好卡在边界上的那一类。
        for room in (capacity, capacity - 0.5):
            if room < 1:
                continue
            lines = _wrap(content, room)
            if len(lines) < 2:
                continue
            for index, (line, soft) in enumerate(lines[1:], 1):
                if soft and line and line[0] in NO_LINE_START:
                    problems.append(f"{name}: 第 {index + 1} 行起于 "
                                    f"{line[0]!r}（每行 {room:.1f} 字）")
            tail, tail_soft = lines[-1]
            if tail_soft and 0 < len(tail) <= 4:
                problems.append(f"{name}: 孤字末行 {tail!r}"
                                f"（每行 {room:.1f} 字）全文={content[:24]!r}")

    if problems:
        raise ValueError(f"{where} 产稿自检未过（{len(problems)} 条）：\n  "
                         + "\n  ".join(dict.fromkeys(problems)))
    return children
