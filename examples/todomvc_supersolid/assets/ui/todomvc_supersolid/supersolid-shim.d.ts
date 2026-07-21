// Ambient declarations so the editor can resolve the "supersolid" import and
// type-check JSX without a real package graph. superui's transpiler is the
// real consumer; this only silences the VSCode language server.

declare module "supersolid" {
  export function createSignal<T>(value?: T): [() => T, (next: T) => void];
  export function createMemo<T>(fn: () => T): () => T;
  export function render(root: () => unknown, mount: unknown): void;

  // Control-flow components — permissive on purpose.
  export const For: (props: {
    each: readonly unknown[];
    children: (item: any, index: () => number) => unknown;
  }) => unknown;
  export const Show: (props: { when: unknown; children: unknown }) => unknown;
  export const Index: (props: {
    each: readonly unknown[];
    children: (item: () => any, index: number) => unknown;
  }) => unknown;
  export const Switch: (props: { children: unknown }) => unknown;
}

// With `jsx: preserve` and no jsxImportSource, TS uses the GLOBAL JSX namespace
// for intrinsic elements. Keep it permissive: this is markup for a browser-like
// DOM, not React, so we don't want React's element typings.
declare namespace JSX {
  interface IntrinsicElements {
    [name: string]: any;
  }
  type Element = any;
  interface ElementChildrenAttribute {
    children: {};
  }
}
