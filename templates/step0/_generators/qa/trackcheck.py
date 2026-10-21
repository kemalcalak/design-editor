#!/usr/bin/env python3
"""trackcheck — CJK 负字距闸。

规则（与 `skills/domains/cjk-typography.md` 和 `deckkit.py` 同源）：

    <48px            letterSpacing 一律 0，绝不为负（负字距让 CJK 字面相撞）
    >=48px           允许到 |letterSpacing| <= fontSize × 0.02（即 -0.02em），可带小数

**判定用比值，不先 round 上限。** 72px 的上限是 1.44，所以 -1.4 合法、-2 不合法；
先 `round(72 × -0.02) = -1` 再比会把 -1.4 误判成越界——那是规则的 bug，不是稿子的。
0809 全库扫描里这一类假阳性占了含汉字命中的绝大多数。

**只检含汉字的节点。** 拉丁/数字 display（页码、序号、Stat Value、Price Value）用
-0.03~-0.05em 是正常排印，拿 CJK 的上限去卡它们是误伤：同一次扫描里 19 套模板、
160 个节点属于这一类，全部豁免。

这道闸在 0809 之前不存在，规则只在 `deckkit.py` 里、且只有 import 了 deckkit 的
生成器才受约束（64 套里只有 2 套）——minimal-keynote 的四处越界就是这么漂进去的。
"""
import json
import pathlib
import sys

CAP_EM = 0.02          # -0.02em
MIN_SIZE = 48          # 小于此字号一律 0
EPS = 1e-6             # 浮点比较容差：48 × 0.02 在二进制里不是精确值


def has_han(text):
    return isinstance(text, str) and any("一" <= ch <= "鿿" for ch in text)


def violations(doc):
    """返回 [(name, fontSize, letterSpacing, cap, text)]。"""
    out = []

    def walk(node):
        if isinstance(node, dict):
            spacing = node.get("letterSpacing")
            size = node.get("fontSize")
            text = node.get("content") or node.get("text")
            if (
                isinstance(spacing, (int, float))
                and spacing < 0
                and isinstance(size, (int, float))
                and has_han(text)
            ):
                cap = size * CAP_EM
                if size < MIN_SIZE or abs(spacing) > cap + EPS:
                    out.append((node.get("name") or "?", size, spacing,
                                0 if size < MIN_SIZE else round(-cap, 2), text[:16]))
            for value in node.values():
                walk(value)
        elif isinstance(node, list):
            for value in node:
                walk(value)

    walk(doc)
    return out


def main(paths):
    total = 0
    for path in paths:
        p = pathlib.Path(path)
        try:
            doc = json.loads(p.read_text(encoding="utf-8"))
        except (OSError, ValueError) as exc:
            print(f"--- {p.stem} · 读不出来：{exc}")
            total += 1
            continue
        hits = violations(doc)
        if not hits:
            continue
        total += len(hits)
        print(f"--- {p.stem} · 负字距越界 {len(hits)} 处 ---")
        for name, size, spacing, cap, text in dict.fromkeys(hits):
            print(f"    字距越界 {name}: {size}px 的 {spacing} 越过上限 {cap}"
                  f"（文案 {text!r}）")
    return total


if __name__ == "__main__":
    sys.exit(1 if main(sys.argv[1:]) else 0)
