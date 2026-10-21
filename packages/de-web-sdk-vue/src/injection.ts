import type { InjectionKey, ShallowRef } from 'vue';
import type { OpViewer } from '@kemalcalak/de-web-sdk';
export const viewerKey: InjectionKey<ShallowRef<OpViewer | null>> = Symbol('de-web-sdk-viewer');
