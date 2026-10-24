---
name: codegen-flutter
description: Flutter/Dart code generation rules — widget tree with BoxDecoration and EdgeInsets
phase: [generation]
trigger:
  flags: [isCodeGen]
priority: 20
budget: 2000
category: knowledge
---

# Flutter (Dart) Code Generation

Generate Flutter widget trees using Material Design widgets.

## Output Format

- Dart file (`.dart`)
- `StatelessWidget` class with `build()` method returning widget tree
- Import `package:flutter/material.dart`
- Import `dart:math` for path/polygon rendering

## Layout Mapping

- `layout: "vertical"` → `Column(children: [...])`
- `layout: "horizontal"` → `Row(children: [...])`
- No layout / stacked children → `Stack(children: [...])` with `Positioned()` wrappers
- `gap: N` → `SizedBox(height: N)` between children (Column) or `SizedBox(width: N)` between children (Row)
- `justifyContent: "start"` → `mainAxisAlignment: MainAxisAlignment.start`
- `justifyContent: "center"` → `mainAxisAlignment: MainAxisAlignment.center`
- `justifyContent: "end"` → `mainAxisAlignment: MainAxisAlignment.end`
- `justifyContent: "space_between"` → `mainAxisAlignment: MainAxisAlignment.spaceBetween`
- `justifyContent: "space_around"` → `mainAxisAlignment: MainAxisAlignment.spaceAround`
- `alignItems: "start"` → `crossAxisAlignment: CrossAxisAlignment.start`
- `alignItems: "center"` → `crossAxisAlignment: CrossAxisAlignment.center`
- `alignItems: "end"` → `crossAxisAlignment: CrossAxisAlignment.end`
- Always include `mainAxisSize: MainAxisSize.min` on Column/Row

## Container & Decoration

- Container nodes → `Container()` widget with named parameters
- `width: N` → `width: N`
- `height: N` → `height: N`
- `clipContent: true` → `clipBehavior: Clip.hardEdge`
- Styling via `decoration: BoxDecoration(...)` parameter

## Color & Fill Mapping

- Solid fill `#RRGGBB` → `Color(0xFFRRGGBB)` (prefix FF for full alpha)
- 8-digit hex `#RRGGBBAA` → `Color(0xAARRGGBB)` (alpha moved to front)
- Variable ref `$name` → `Color(0x00000000) /* var(--name) */` (placeholder with comment)
- Text fill → `color: Color(0xFFhex)` in `TextStyle`
- Linear gradient → `gradient: LinearGradient(colors: [Color(...), Color(...)])`
- Radial gradient → `gradient: RadialGradient(colors: [Color(...), Color(...)])`
- Angular gradient → `gradient: SweepGradient(center: Alignment(cx * 2 - 1, cy * 2 - 1), transform: GradientRotation((rotation - 90) * pi / 180), colors: [...], stops: [...])`

## Border & Stroke Mapping

- `stroke.thickness + stroke.color` → `border: Border.all(color: Color(...), width: N)`
- Variable ref thickness → `/* var(--name) */ 1` placeholder

## Corner Radius

- Uniform → `borderRadius: BorderRadius.circular(N)`
- Per-corner → `borderRadius: BorderRadius.only(topLeft: Radius.circular(TL), topRight: Radius.circular(TR), bottomRight: Radius.circular(BR), bottomLeft: Radius.circular(BL))`

## Effects

- Drop shadow → `boxShadow: [BoxShadow(color: Color(...), blurRadius: N, offset: Offset(X, Y))]`
- Blur → `BackdropFilter(filter: ImageFilter.blur(sigmaX: N, sigmaY: N), child: ...)`

## Typography

- Text nodes → `Text('content', style: TextStyle(...))`
- `fontSize` → `fontSize: N`
- `fontWeight` → `fontWeight: FontWeight.wN00` (w100 through w900)
- `fontStyle: "italic"` → `fontStyle: FontStyle.italic`
- `fontFamily` → `fontFamily: 'Name'`
- `letterSpacing` → `letterSpacing: N`
- `lineHeight` → `height: lineHeight` (multiplier in TextStyle)
- `textAlign` → `textAlign: TextAlign.left|center|right|justify`
- `underline` → `decoration: TextDecoration.underline`
- `strikethrough` → `decoration: TextDecoration.lineThrough`
- Combined → `decoration: TextDecoration.combine([TextDecoration.underline, TextDecoration.lineThrough])`
- Fixed-size text → wrap in `SizedBox(width: N, height: N, child: Text(...))`

## Padding

- Uniform → `padding: EdgeInsets.all(N)`
- Symmetric → `padding: EdgeInsets.symmetric(vertical: V, horizontal: H)`
- Per-side `[top, right, bottom, left]` → `padding: EdgeInsets.fromLTRB(left, top, right, bottom)`
- Variable ref → `EdgeInsets.all(/* var(--name) */ 0)` placeholder

## Dimensions

- Fixed → `width: N, height: N` on Container
- Text sizing → wrap in `SizedBox`

## Image Handling

- Network URL → `Image.network('url', width: N, height: N, fit: BoxFit.cover)`
- Asset → `Image.asset('path', width: N, height: N, fit: BoxFit.cover)`
- Data URI → `Image.memory(base64Decode('...'))`
- `objectFit: "fit"` → `BoxFit.contain`
- `objectFit: "crop"` → `BoxFit.cover`
- Corner radius on images → `ClipRRect(borderRadius: BorderRadius.circular(N), child: Image(...))`

## Opacity & Transform

- Opacity → `Opacity(opacity: N, child: widget)` wrapper
- Rotation → `Transform.rotate(angle: N * pi / 180, child: widget)` wrapper
- Applied as wrapper widgets around the base widget

## Positioning

- Absolute children → `Positioned(left: X, top: Y, child: widget)` inside `Stack`

## Ellipse

- Circle/ellipse → `Container` with `BoxDecoration(shape: BoxShape.circle)`

## Icon Handling

- Icon font nodes → `Icon(LucideIcons.icon_name, size: N, color: Color(...))`
- Icon name: kebab-case converted to snake_case

## Path & Polygon

- Path nodes → `CustomPaint(size: Size(W, H), painter: _PathPainter(pathData, color))`
- Polygon nodes → `CustomPaint(size: Size(W, H), painter: _PolygonPainter(sides, color))`
- Include helper `CustomPainter` classes at bottom of file

## Responsive Design

- Use `MediaQuery.of(context).size` for screen dimensions
- `LayoutBuilder` for parent-relative sizing
- `Flexible` and `Expanded` for proportional layouts
