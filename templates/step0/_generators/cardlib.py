"""卡片体系的共享契约 —— 画幅、安全区、网格、字阶、字族。

来源是 `openpencil-docs/openpencil/generation/card-system-0808.md` 的 §4.0
（全局网格与字阶）与 §5（平台规格）。六套卡片模板都从这里取值，spec 改了
只需要改这一个文件 —— 把 §4.0 抄六遍，第七套模板就会和前六套对不齐。

oplib 是所有生成器共用的**节点构造**层；这一层是**卡片体系专用**的度量层，
只被 tpl_*_card.py 引用。
"""

# ---------------------------------------------------------------- 字族
#
# spec §3 给每套主题指定了具体的 CJK 字族（汇文明朝体 / 霞鹜文楷 / 得意黑
# / 源流明体 / 京华老宋体 …），本机一个都没有装。实测（fontprobe）：
#
#   "Noto Serif SC"  → 落到系统默认无衬线，**衬线意图完全丢失**
#   "Songti SC"      → 真宋体 ✓
#   "STSong"         → 真宋体 ✓
#   "Noto Sans SC"   → 无衬线（实际是 PingFang SC）✓
#
# 所以凡是 spec 要「明朝 / 宋 / 楷」气质的地方一律写 `Songti SC`，而不是照
# 抄 spec 的字族名 —— 照抄的结果是全部退化成同一个黑体，主题之间的字体差
# 异归零。spec 自己也写明字体名是「示例锚点」，与 huashu 的口径一致。
#
# 这条替身关系必须留在注释里：将来真装上了那些字族，改这三个常量即可，
# 不必回头翻六个模板。
SERIF = "Songti SC"     # 承接 spec 的 明朝体 / 老宋 / 源流明 / 文楷 意图
SANS = "Noto Sans SC"   # 承接 思源黑 / 霞鹜新晰黑 / 未来荧黑 意图
NUM = "Inter"           # 数字与拉丁一律走西文族（中文排印规范的 fallback 链）


# ---------------------------------------------------------------- 画幅
class Canvas:
    """一个发布规格：画幅 + 安全区 + 12 列网格。

    安全区来自 spec §5 的平台规格表。**下边距故意大于上边距** —— 信息流
    详情页底部会叠互动浮层，多留的那一截是给它的。
    """

    def __init__(self, name, width, height, pad_x, pad_top, pad_bottom):
        self.name = name
        self.width = width
        self.height = height
        self.pad_x = pad_x
        self.pad_top = pad_top
        self.pad_bottom = pad_bottom

    @property
    def inner(self):
        """内容宽 = 12 列 + 11 沟槽。"""
        return self.width - self.pad_x * 2

    @property
    def padding(self):
        """oplib 的 [top, right, bottom, left]。"""
        return [self.pad_top, self.pad_x, self.pad_bottom, self.pad_x]

    def cols(self, n):
        """n 列的像素宽（含中间的沟槽）。12 列 = 满栏。"""
        col = (self.inner - GUTTER * 11) / 12
        return round(col * n + GUTTER * (n - 1))


GUTTER = 16

# spec §5 平台规格表。4:5 是本仓在 spec 之外补的一档（小红书封面推荐比例），
# 安全区按 3:4 与 1:1 的中间值取，理由写在用到它的模板里。
VERTICAL = Canvas("3:4", 1080, 1440, 80, 96, 128)
SQUARE = Canvas("1:1", 1080, 1080, 80, 88, 112)
PORTRAIT45 = Canvas("4:5", 1080, 1350, 80, 92, 120)


# ---------------------------------------------------------------- 字阶
#
# spec §4.0 的 8 档，(fontSize, lineHeight, letterSpacing)。
#
# **任何小于 32px 的文字在本体系里是错误**，不是「小字」—— 1080 宽的卡片在
# 390pt 的手机上缩 2.77 倍，32px 落到约 11.6pt，那已经是注释可读的下限。
#
# 行高遵循「中文行高比西文高 0.2」：西文正文 1.5 → 中文 1.7。
# 字距遵循「CJK 负字距不超过 -0.02em」：只有 ≥120px 的展示档给 -0.01em，
# 48px 以下一律 0 或微正（正文加 0.02em 提透气度）。
SCALE = {
    "display-xl": (168, 1.05, -1.7),
    "display-l": (120, 1.10, -1.2),
    "display": (88, 1.15, 0),
    "title-1": (64, 1.25, 0),
    "title-2": (48, 1.30, 0),
    "body-l": (40, 1.70, 0.8),
    "body": (36, 1.70, 0.7),
    "caption": (32, 1.50, 0.6),
}

MIN_FONT_SIZE = 32


def step(name):
    """取一档字阶，返回 (size, line_height, spacing)。"""
    if name not in SCALE:
        raise KeyError(f"{name} 不是 spec §4.0 的字阶档；单页最多用 4 档")
    return SCALE[name]


def typed(ids, text_fn, name, content, scale_name, weight, color, *,
          family=SANS, width="fill_container", growth="fixed-width",
          align=None):
    """按字阶档建一个文本节点。

    比直接调 oplib.text 多一层，是为了让「用了哪一档」在代码里显式可数 ——
    spec 规定单页最多 4 档，数不出来就守不住。
    """
    size, line_height, spacing = step(scale_name)
    return text_fn(ids, name, content, size, weight, color, family=family,
                   line_height=line_height, width=width, growth=growth,
                   align=align, spacing=spacing)
