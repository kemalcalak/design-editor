import { describe, expect, it } from 'vitest';

import type { ImageFillBody, ImageNode, PathNode, PenPage } from '../src/ops-types.js';

type IsOptional<T, K extends keyof T> = {} extends Pick<T, K> ? true : false;

const legacyPage: PenPage = {
  id: 'page-1',
  name: 'Page 1',
  children: [],
  state: null,
  lifecycle: null,
};

const legacyTileFill: ImageFillBody = {
  url: 'data:image/png;base64,AA==',
  mode: 'tile',
  originalSize: null,
  transform: null,
  explain: null,
  opacity: null,
  blendMode: null,
  exposure: null,
  contrast: null,
  saturation: null,
  temperature: null,
  tint: null,
  highlights: null,
  shadows: null,
};

const newFieldsStayOptional: [
  IsOptional<PenPage, 'backgroundColor'>,
  IsOptional<ImageFillBody, 'tileScale'>,
  IsOptional<ImageNode, 'maskType'>,
  IsOptional<ImageNode, 'blendMode'>,
  IsOptional<PathNode, 'fillRule'>,
  IsOptional<PathNode, 'mask'>,
] = [true, true, true, true, true, true];

describe('generated schema backward compatibility', () => {
  it('accepts pre-upgrade page and image-fill object literals', () => {
    expect(legacyPage.name).toBe('Page 1');
    expect(legacyTileFill.mode).toBe('tile');
    expect(newFieldsStayOptional).toEqual([true, true, true, true, true, true]);
  });
});
