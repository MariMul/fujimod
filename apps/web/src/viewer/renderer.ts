export type RendererBackend = 'webgl2' | 'webgpu';

export function rendererBackend(search: string): RendererBackend {
  const backend = new URLSearchParams(search).get('renderer') ?? 'webgl2';
  if (backend !== 'webgl2' && backend !== 'webgpu') throw Error(`Unknown 3D renderer: ${backend}`);
  return backend;
}

export function rendererURL(href: string, backend: string): string {
  const url = new URL(href);
  url.searchParams.set('renderer', backend);
  rendererBackend(url.search);
  return url.href;
}
