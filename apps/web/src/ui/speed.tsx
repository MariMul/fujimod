function speedColor(speed: number) {
  if (speed > 1) {
    return '#30d158';
  } else if (speed >= 0.75) {
    return 'var(--dim)';
  } else if (speed <= 0.2) {
    return 'var(--red)';
  } else {
    const warmth = (0.75 - speed) / 0.55;
    const [from, to, amount]: [string, string, number] =
      warmth < 0.5
        ? ['var(--dim)', '#ff9f0a', warmth * 2]
        : ['#ff9f0a', 'var(--red)', (warmth - 0.5) * 2];
    return `color-mix(in srgb, ${from}, ${to} ${amount * 100}%)`;
  }
}
export function SpeedGauge(props: { speed: number }) {
  return (
    <span
      id="camera-speed"
      style={{
        'white-space': 'nowrap',
        'font-variant-numeric': 'tabular-nums',
        color: speedColor(props.speed),
      }}
    >
      {Math.round(Math.max(0, props.speed) * 100)}% speed
    </span>
  );
}
