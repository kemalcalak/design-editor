#!/usr/bin/env python3
"""journal-checklist-card.op — 清单卡 · 知识库风（4:5 单张，1080×1350）

第三种中文卡片气质，和另外两套的分工是「卡片用来干什么」：

  - knowledge-card（暖纸底 · 编号要点）——讲方法，读者是来学的；
  - punch-quote-card（墨底 · 大字报）——喊态度，读者是来被戳的；
  - 这一套（浅灰底 · 白卡清单）——记事，读者是来抄作业的。

所以版式的主角是**可勾的行**而不是标题：一张浅灰工作区上的白色清单卡，五
条待办，前两条已经打勾（实心方框 + 白钩，文案退成灰），后三条留空。读者把
文案换掉就是自己的周计划。

画幅取 1080×1350（4:5）而不是既有的 3:4/1:1 —— 这是小红书封面推荐比例，
也让「卡片」这一档在画幅上不只有一种选择。（视觉体系的默认基线是 3:4，那
条约束是给**多页轮播**用的——一套卡必须画幅一致；这是单张卡，不在其列。）

### 风格来源与一次推翻

第一版做的是「手账胶带风」：米黄横格纸 + 两条倾斜胶带 + 红笔批注。推翻
了 —— 中文图文视觉体系的默认审美基线把「儿童卡通感、塑料质感」列为禁止
项，风格库里凡带贴纸/手绘的档也一律把「贴纸感」写进负面提示词。横格纸 +
胶带 + 红笔三件套叠在一起，正好落在那条线的另一边。

改走体系里为「工具清单 / 方法清单 / 资料整理」明确点名的那一档 —— **Notion
高级卡片风**：米白浅灰底、圆角很小的白卡、清晰的标题区/标签区/条目区，常
用元素就是 checkbox、标签、分割线、引用块。它是这类内容的对口风格，不是我
临时挑的。

### 配色推导（采样 → 收敛 → 论证）

  - **采样**：内容是一页周计划。文化语境取「旧钢笔墨水的灰蓝」——不是 SaaS
    默认蓝，那个是从模型先验里抽签抽出来的。
  - **收敛**：1 个有彩色 + 1 组中性明度序列。灰蓝 oklch(0.45 0.05 240)；
    中性走 L 0.16 / 0.42 / 0.62 / 0.90 / 0.955 / 1.0。
  - **论证**：chroma 只给 0.05，比「品牌主色 0.08-0.15」还低一档。这张卡的
    主角是结构（勾选框、分割线、标签），有彩色只负责标记「哪一条是重点」，
    压得住才不抢结构的戏。

### 负约束（本模板明令不做的事）

  - 不用纸纹、格线、胶带、贴纸、伪手写笔迹 —— 一条都不留。
  - 不用阴影。卡片的边界由 1px hairline 描边给，不由 `effects` 给
    （体系对这一档的负面提示词就写着「过度阴影」）。
  - 不用大圆角。白卡 cornerRadius 12，方框 8 —— 「圆角很小的白卡」是这一
    档的原话，药丸形状（999）只留给标签，且标签是小面积。
  - 不用第二个有彩色。灰蓝之外全是中性明度序列。
  - 不用 emoji 当图标，不用装饰性插画。
  - 不写 AI 套话（「赋能 / 一站式 / 打造闭环」），文案写真会写进周计划的话。

硬契约：
  - 内容距边缘 ≥80px（这里 96/88，视觉体系给 1080 宽的安全边距下限是左右
    72 / 上下 80）
  - 配色全部走 color_vars，改主色只改 $c-accent 一处
  - 正文与背景对比度 ≥2.0（承载文字的最低一对是 5.05，见文件末尾注释）
  - **字阶走 card-system spec §4.0 的档位**（`cardlib.SCALE`）：主标题
    title-1(64) / 条目 body-l(40) / 副标题 body-l(40) / 说明与标签
    caption(32)。原来的 25-27px 正文低于体系的 32px 下限。C3 勾选清单的档
    位是 Title 2 / Body L / Caption，条目在这里降一档到 Body L —— 五条带
    注解的行按 Title 2 排会把 1350 高的卡片顶穿，spec §5.2 的处理方式是减
    行不减字号，而这张卡的五条是它的产品定义，所以改降条目档位并收紧行距。
  - **CJK 行高比西文全线高 0.2**：标题 1.3，条目 1.35，正文 1.7
  - **CJK 不用负字距**：24-48px 的中文标题一律 0（方块字距天然均匀）
  - 汉字走 Noto Sans SC，数字与拉丁走 Inter
  - 顶层 frame 必须显式写 x/y，否则多帧会全部堆在原点
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from cardlib import step
from oplib import (Ids, color_vars, frame, icon_font, rect, solid, text,
                   write_doc)

ids = Ids()

VARS = color_vars({
    "c-bg":       "#EEF0F3",
    "c-card":     "#FFFFFF",
    "c-ink":      "#0C0D0F",
    "c-muted":    "#4A4D51",
    # 已勾掉那两行的文字。它**应该**比未完成的行弱，但仍要读得出来，
    # 所以停在 3.62 而不是继续压。
    "c-done":     "#83878B",
    "c-line":     "#DCDEE0",
    # 空方框的描边。非文字图形，AA 门槛 3.0 —— 3.65 刚过一点，再浅它就
    # 消失在白卡里，读不出「这里可以勾」。
    "c-box":      "#7D8792",
    "c-accent":   "#3B596E",
    "c-accent-soft": "#DEEAF3",
})

CJK = "Noto Sans SC"
NUM = "Inter"

W, H = 1080, 1350
PAD_Y, PAD_X = 96, 88

# CJK 行高阶梯（西文 +0.2）。全篇只用这三档。
LH_HEAD, LH_ITEM, LH_BODY = 1.3, 1.35, 1.7

TODOS = [
    ("把这周要交的东西列出来", "只列交付物，不列过程", True),
    ("给最难的那件事定个开始时间", "写死到几点，不写「上午」", True),
    ("砍掉一件本来就不该做的事", "每周至少砍一件", False),
    ("留两个小时不安排任何事", "留白也要写进日程才守得住", False),
    ("周日晚上花十分钟复盘", "只问三件事：成了、卡了、下周", False),
]


def block(name, children, gap=18, align="start", **props):
    """透明结构容器。没有 fill 才是结构层；一旦有 fill 它就是一张卡面了。"""
    node = frame(ids, name, width="fill_container", height="fit_content",
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


def hairline(color="$c-line", thickness=2):
    return rect(ids, "分隔线", width="fill_container", height=thickness,
                fill=solid(color))


def checkbox(done):
    """清单方框。勾选态是实心墨块 + 白钩，未选态是一圈描边。

    两态**故意用不同的节点类型**。勾选态必须是 frame：frame 会递归渲染子
    节点，方框里那个钩才显示得出来。未选态是空的，而一个带描边、带圆角、没
    有内容的 frame 正是几何检测器要抓的东西（`empty decorated frame` —— 十
    次里九次是布局写漏了），所以它写成 rectangle：一个有描边的矩形是一个
    形状，不是一个空容器。两者盒模型一致（44×44、cornerRadius 8），排在一
    列里对得齐。
    """
    if done:
        node = frame(ids, "方框 · 已勾", width=44, height=44,
                     layout="horizontal", alignItems="center",
                     justifyContent="center", cornerRadius=8,
                     fill=solid("$c-accent"))
        node["children"] = [icon_font(ids, "勾", "check", 26, "$c-card")]
        return node
    return rect(ids, "方框 · 待办", width=44, height=44, cornerRadius=8,
                fill=[], stroke={"thickness": 3, "fill": solid("$c-box")})


def todo_row(title, note, done):
    return row("待办", [
        checkbox(done),
        block("待办文案", [
            text(ids, "待办标题", title, step("body-l")[0], 600,
                 "$c-done" if done else "$c-ink", family=CJK,
                 line_height=LH_ITEM),
            text(ids, "待办说明", note, step("caption")[0], 400,
                 "$c-done" if done else "$c-muted", family=CJK,
                 line_height=LH_BODY),
        ], gap=8),
    ], gap=24, align="start")


def checklist():
    """白色清单卡。整张图唯一有 fill 的内容容器。

    条目之间用一条 hairline 而不是纯留白：这一档的常用元素里就有分割线，
    而且清单是「一条一条」的东西，线让它读起来像可以逐行打勾的表。
    """
    items = []
    for index, todo in enumerate(TODOS):
        if index:
            items.append(hairline())
        items.append(todo_row(*todo))
    node = frame(ids, "清单卡", width="fill_container", height="fit_content",
                 layout="vertical", padding=[32, 36], gap=18, cornerRadius=12,
                 fill=solid("$c-card"))
    node["stroke"] = {"thickness": 2, "fill": solid("$c-line")}
    node["children"] = items
    return node


def tag(label):
    """小标签。整张卡唯一的圆角胶囊，面积小到不与「小圆角」的骨架冲突。"""
    node = frame(ids, "标签", width="fit_content", height="fit_content",
                 layout="horizontal", padding=[9, 20], gap=0,
                 cornerRadius=999, alignItems="center",
                 justifyContent="center", fill=solid("$c-accent-soft"))
    node["children"] = [
        text(ids, "标签文字", label, step("caption")[0], 600, "$c-accent",
             family=CJK,
             width="fit_content", growth="auto", line_height=1.4),
    ]
    return node


def header():
    meta = row("元信息", [
        tag("周计划"),
        text(ids, "日期", "2026 / 08 · 第 2 周", step("caption")[0], 500,
             "$c-muted",
             family=NUM, width="fit_content", growth="auto", line_height=1.4),
    ], gap=16, width="fit_content")
    return block("卡头", [
        meta,
        text(ids, "主标题", "本周只做这五件事", step("title-1")[0], 700,
             "$c-ink", family=CJK, line_height=LH_HEAD),
        text(ids, "副标题", "写不下的都不是这周的事。", step("body-l")[0], 400,
             "$c-muted", family=CJK, line_height=LH_BODY),
    ], gap=20)


def quote_note():
    """引用块。左侧一条竖线 + 一句话 —— 这一档的常用元素之一，也是整张卡
    收尾的地方：它不是新信息，是对上面那张表的一句注解。"""
    bar = rect(ids, "引用竖线", width=6, height="fill_container",
               fill=solid("$c-accent"))
    return row("引用块", [
        bar,
        text(ids, "引用文字", "打完前两个，这周就已经赢了一半。",
             step("body-l")[0], 500, "$c-muted", family=CJK,
             line_height=LH_BODY),
    ], gap=22, align="stretch")


def card():
    node = frame(ids, "清单卡片 · 知识库风", width=W, height=H,
                 layout="vertical", padding=[PAD_Y, PAD_X], gap=0,
                 justifyContent="space_between", alignItems="start",
                 fill=solid("$c-bg"), clipContent=True)
    node["children"] = [header(), checklist(), quote_note()]
    node["x"], node["y"] = 0, 0
    return node


# 对比度（WCAG 相对亮度比，op-design-lint 的门槛是 2.0；数值实测）：
#   c-ink    on c-bg / c-card   14.14 / 15.98    c-accent on c-card        5.46
#   c-muted  on c-bg / c-card    5.05 /  5.70    c-accent on c-accent-soft 4.65
#   c-done   on c-card           3.62            c-card   on c-accent      5.46
#   c-box    on c-card           3.65            c-ink    on c-accent-soft 13.44
# 承载文字的最低一对是 c-muted on c-bg 的 5.05。两处**故意**低于 AA：c-done
# 是已勾掉那两行（它就该弱），c-box 是空方框描边（非文字图形，门槛 3.0）。
# 换主色时先量 c-accent on c-accent-soft —— 标签是唯一把小字压在浅底上的地方。

if __name__ == "__main__":
    write_doc(sys.argv[1], VARS, [card()], "清单卡片 · 知识库风 4:5")
