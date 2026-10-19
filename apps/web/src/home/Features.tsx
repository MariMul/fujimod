import { For, Show, createSignal, onCleanup } from 'solid-js';
import catalog from '../../data/features.json';
import featureIcons from '../ui/assets/feature-icons.svg?url';
import './features.css';

type Card = (typeof catalog.cards)[keyof typeof catalog.cards];
type Feature = (typeof catalog.features)[number];

const GROUPS: Record<string, string> = {
  pictures: 'Making pictures',
  handling: 'Handling and display',
  problems: 'When something goes wrong',
};
const MANUAL = 'https://fujifilm-dsc.com/en/manual/x70/';
const cards = catalog.cards as Record<string, Card>;

/** What a hover card shows: a control, or one option of it. */
type Target = { card: string; choice?: string; anchor: HTMLElement };

/** An aspect ratio choice ("ratio:65:24") drawn as its frame. */
function RatioFrame(props: { ratio: string }) {
  const frame = () => {
    const [w, h] = props.ratio.split(':').map(Number);
    const scale = 14 / Math.max(w, h);
    return { width: w * scale, height: h * scale };
  };
  return (
    <svg class="catalog-ratio" viewBox="0 0 14 14" aria-hidden="true">
      <rect
        x={(14 - frame().width) / 2}
        y={(14 - frame().height) / 2}
        width={frame().width}
        height={frame().height}
        rx="1"
      />
    </svg>
  );
}

function Icon(props: { name: string }) {
  return (
    <Show
      when={props.name.startsWith('ratio:')}
      fallback={
        <svg class="catalog-icon" aria-hidden="true">
          <use href={`${featureIcons}#${props.name}`} />
        </svg>
      }
    >
      <RatioFrame ratio={props.name.slice(6)} />
    </Show>
  );
}

/** Keep slot ranges such as C1–C7 on one line. */
function Text(props: { text: string }) {
  return (
    <For each={props.text.split(/(C\d–C\d)/)}>
      {(part, index) => (index() % 2 ? <span class="nowrap">{part}</span> : part)}
    </For>
  );
}

function CardBody(props: { target: Target }) {
  const card = () => cards[props.target.card];
  const choice = () => props.target.choice;
  const path = () => [...card().path, card().title, ...(choice() ? [choice()!] : [])].join(' › ');
  return (
    <>
      <div class="catalog-card-title">{choice() ?? card().title}</div>
      <Show
        when={choice()}
        fallback={
          <Show when={card().changes} fallback={<p>{card().summary}</p>}>
            <p>In FujiMod: {card().changes}</p>
            <a href={`${MANUAL}${card().manual}/index.html`} target="_blank" rel="noreferrer">
              X70 manual
            </a>
          </Show>
        }
      >
        <p>{(card().choices as Record<string, string>)[choice()!]}</p>
      </Show>
      <div class="catalog-card-path">{path()}</div>
      <Show when={!choice() && card().related.length}>
        <ul class="catalog-card-related">
          <For each={card().related}>
            {(related) => (
              <li>
                <b>{cards[related.card].title}</b>: {related.reason}
              </li>
            )}
          </For>
        </ul>
      </Show>
    </>
  );
}

export function FeatureCatalog() {
  const [target, setTarget] = createSignal<Target>();
  const [position, setPosition] = createSignal({ left: 0, top: 0 });
  let timer: ReturnType<typeof setTimeout> | undefined;
  let card!: HTMLDivElement;
  onCleanup(() => clearTimeout(timer));

  const place = (anchor: HTMLElement) => {
    const r = anchor.getBoundingClientRect();
    const width = Math.min(340, innerWidth - 16);
    const height = card?.offsetHeight ?? 0;
    let top = r.bottom + 10;
    if (top + height > innerHeight - 8) top = Math.max(8, r.top - height - 10);
    setPosition({ left: Math.min(Math.max(8, r.left + r.width / 2 - width / 2), innerWidth - width - 8), top });
  };
  const show = (next: Target) => {
    setTarget(next);
    place(next.anchor);
    requestAnimationFrame(() => place(next.anchor));
  };
  const later = (action: () => void, ms: number) => {
    clearTimeout(timer);
    timer = setTimeout(action, ms);
  };
  // Hover after a short delay; keyboard focus and taps open at once.
  const refProps = (card: string, choice?: string) => ({
    class: 'catalog-ref',
    tabindex: 0,
    onMouseEnter: (event: MouseEvent) => {
      const anchor = event.currentTarget as HTMLElement;
      later(() => show({ card, choice, anchor }), 250);
    },
    onMouseLeave: () => later(() => setTarget(), 150),
    onFocus: (event: FocusEvent) => show({ card, choice, anchor: event.currentTarget as HTMLElement }),
    onBlur: () => later(() => setTarget(), 150),
    onClick: (event: MouseEvent) => {
      const anchor = event.currentTarget as HTMLElement;
      if (target()?.anchor === anchor) setTarget();
      else show({ card, choice, anchor });
    },
  });

  const tile = (feature: Feature) => (
    <article class="catalog-tile">
      <h4>
        <Icon name={feature.icon} />
        <Show when={feature.card} fallback={<span>{feature.title}</span>}>
          <span {...refProps(feature.card!)}>{feature.title}</span>
        </Show>
      </h4>
      <p>
        <Text text={feature.summary} />
      </p>
      <For each={feature.controls}>
        {(control) => {
          const choices = control.card
            ? (cards[control.card].choices as Record<string, string>)
            : {};
          const chips = (
            <div class="catalog-chips">
              <For each={control.choices}>
                {(choice) => {
                  const body = (
                    <>
                      <Show when={choice.icon}>
                        <Icon name={choice.icon!} />
                      </Show>
                      {choice.label}
                    </>
                  );
                  return choice.label in choices ? (
                    <span
                      {...refProps(control.card!, choice.label)}
                      class={['catalog-chip catalog-ref', { stock: choice.stock }]}
                    >
                      {body}
                    </span>
                  ) : (
                    <span class={['catalog-chip', { stock: choice.stock }]}>{body}</span>
                  );
                }}
              </For>
              <For each={control.actions}>
                {(action) => <span class="catalog-chip action">{action}</span>}
              </For>
            </div>
          );
          return control.primary ? (
            chips
          ) : (
            <div class="catalog-sub">
              <Show when={control.card} fallback={<h5>{control.title}</h5>}>
                <h5>
                  <span {...refProps(control.card!)}>{control.title}</span>
                </h5>
              </Show>
              {chips}
            </div>
          );
        }}
      </For>
      <Show when={feature.path.length}>
        <div class="catalog-where">{feature.path.join(' › ')}</div>
      </Show>
    </article>
  );

  return (
    <section class="catalog" id="features" aria-labelledby="catalog-heading">
      <h2 id="catalog-heading">Features</h2>
      <For each={catalog.groups}>
        {(group) => (
          <Show when={catalog.features.some((feature) => feature.group === group)}>
            <h3>{GROUPS[group]}</h3>
            <div class="catalog-grid">
              <For each={catalog.features.filter((feature) => feature.group === group)}>
                {tile}
              </For>
            </div>
          </Show>
        )}
      </For>
      <div
        ref={card}
        class={['catalog-card', { on: !!target() }]}
        role="tooltip"
        style={{ left: `${position().left}px`, top: `${position().top}px` }}
        onMouseEnter={() => clearTimeout(timer)}
        onMouseLeave={() => later(() => setTarget(), 150)}
      >
        <Show when={target()}>{(current) => <CardBody target={current()} />}</Show>
      </div>
    </section>
  );
}
