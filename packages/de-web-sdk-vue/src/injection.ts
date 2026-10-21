import type { InjectionKey, ShallowRef } from 'vue';
import type { OpViewer } from '@zseven-w/de-web-sdk';
export const viewerKey: InjectionKey<ShallowRef<OpViewer | null>> = Symbol('op-web-sdk-viewer');
