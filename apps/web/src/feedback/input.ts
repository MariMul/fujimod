import presentation from '../../../../bindings/python/python/fujimod/presentation/controls.json';

type Control = { label: string; text?: string; icon?: string };
type Shape = { lines?: number[][][]; circle?: number[] };
const controls: Record<string, Control> = presentation.controls;
const shapes: Record<string, Shape> = presentation.icons;
export const badgeStyle = presentation.badge;
export function inputName(id: string): string {
  const control = controls[id];
  if (!control) throw new Error(`Missing input presentation: ${id}`);
  return control.label;
}
export function inputIcon(id: string): SVGSVGElement | undefined {
  const name = controls[id]?.icon, shape = name && shapes[name];
  if (!shape) return;
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('width', String(badgeStyle.icon_size));
  svg.setAttribute('height', String(badgeStyle.icon_size));
  svg.setAttribute('fill', 'none');
  svg.setAttribute('stroke', 'currentColor');
  svg.setAttribute('stroke-width', '2');
  svg.setAttribute('stroke-linejoin', 'round');
  svg.setAttribute('stroke-linecap', 'round');
  svg.setAttribute('aria-hidden', 'true');
  for (const line of shape.lines ?? []) {
    const node = document.createElementNS(svg.namespaceURI, 'polyline');
    node.setAttribute('points', line.map(point => point.join(',')).join(' '));
    svg.append(node);
  }
  if (shape.circle) {
    const node = document.createElementNS(svg.namespaceURI, 'circle');
    for (const [i, attr] of ['cx', 'cy', 'r'].entries()) node.setAttribute(attr, String(shape.circle[i]));
    node.setAttribute('fill', 'currentColor');
    svg.append(node);
  }
  return svg;
}
export function showInputBadge(node: HTMLElement, id: string) {
  const spec = controls[id];
  node.setAttribute('aria-label', inputName(id));
  const icon = inputIcon(id);
  node.replaceChildren(icon ?? document.createTextNode(spec.text ?? spec.label));
  const s = badgeStyle;
  Object.assign(node.style, {
    border: `${s.border_width}px solid ${s.border}`, borderRadius: `${s.radius}px`,
    padding: `${s.padding_y}px ${s.padding_x}px`, background: s.background,
    color: s.foreground, font: `600 ${s.font_size}px/${s.icon_size}px system-ui`,
    bottom: `${s.bottom}px`,
  });
}
