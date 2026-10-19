// @generated
// This file is code generated. Do not edit directly.
// Update the feature documentation sources and regenerate the catalog.
import { For, Show, createSignal, onCleanup } from 'solid-js';
import type { JSX } from '@solidjs/web';
import featureIcons from '../ui/assets/feature-icons.svg?url';
import './features.css';

type Card = {
  title: string;
  path: string[];
  summary: string | null;
  choices: Record<string, string>;
  related: { card: string; reason: string }[];
  manual: string | null;
  changes: string | null;
};
/** What a hover card shows: a control, or one option of it. */
type Target = { card: string; choice?: string; anchor: HTMLElement };

const MANUAL = 'https://fujifilm-dsc.com/en/manual/x70/';
const CARDS: Record<string, Card> = {
  shape_aspect_item: {
    title: 'ASPECT RATIO',
    path: ['SHOOTING MENU', 'SHAPE & ASPECT'],
    summary: 'Choose the shape of still pictures: wide, square or tall.',
    choices: {
      '3:2': "The sensor's full frame, uncropped.",
      '16:9': 'Widescreen, like a TV or movie.',
      '1:1': 'A square frame.',
      '4:3': 'A taller frame, like most phones.',
      '65:24': 'A panoramic strip, as on the Hasselblad XPan.',
      '17:6': 'A wide panoramic strip.',
      '3:4': 'A tall portrait frame.',
      '7:6': 'A near-square frame, like 6×7 film.',
      '5:4': 'A frame like 4×5 large-format film.',
    },
    related: [
      {
        card: 'image_size',
        reason: 'the crop fits within the image size',
      },
      {
        card: 'shape_surround_item',
        reason: 'how the cropped-out area is shown',
      },
    ],
    manual: null,
    changes: null,
  },
  shape_surround_item: {
    title: 'SURROUND VIEW',
    path: ['SHOOTING MENU', 'SHAPE & ASPECT'],
    summary: 'Choose how the area outside the frame is shown.',
    choices: {
      MASKED: 'The area outside the frame is black.',
      DARKENED: 'The area outside the frame is darkened.',
      'WHITE BORDER': 'A white line marks the frame.',
    },
    related: [],
    manual: null,
    changes: null,
  },
  shape_desqueeze_item: {
    title: 'DESQUEEZE',
    path: ['SHOOTING MENU', 'SHAPE & ASPECT'],
    summary: 'Choose the squeeze factor of your anamorphic adapter.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
  },
  shape_jpeg_item: {
    title: 'DESQUEEZE JPEG',
    path: ['SHOOTING MENU', 'SHAPE & ASPECT'],
    summary: 'Choose whether JPEG pictures are also recorded at their corrected width.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
  },
  image_size: {
    title: 'IMAGE SIZE',
    path: ['SHOOTING MENU'],
    summary: null,
    choices: {},
    related: [],
    manual: 'menu_shooting/image_size',
    changes:
      'Aspect ratio selection moved to SHAPE & ASPECT > ASPECT RATIO; IMAGE SIZE now sets L, M or S only.',
  },
  film_simulation: {
    title: 'FILM SIMULATION',
    path: ['SHOOTING MENU'],
    summary: null,
    choices: {},
    related: [],
    manual: 'menu_shooting/film_simulation',
    changes:
      'Adds ETERNA, Neutral and approximations of four film simulations from newer cameras, and films saved on the memory card. Approximations are marked (approx.) in the camera.',
  },
  film_editor_item: {
    title: 'FILM EDITOR',
    path: ['SHOOTING MENU'],
    summary: 'Create and edit film simulations, saved as files on the memory card.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
  },
  hdmi_rec_behavior_item: {
    title: 'HDMI OUTPUT',
    path: ['SET-UP'],
    summary: 'Choose whether an HDMI device shows the shooting view or playback only.',
    choices: {
      'LIVE VIEW': 'The HDMI device shows the shooting view.',
      'PLAYBACK ONLY': 'The HDMI device shows playback only, as on an unmodified camera.',
    },
    related: [],
    manual: null,
    changes: null,
  },
  bindings_row_exposure_dial: {
    title: 'EV DIAL',
    path: ['SET-UP', 'BUTTON/DIAL SETTING'],
    summary: 'Choose the setting the exposure compensation dial adjusts.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
  },
  bindings_row_command_dial: {
    title: 'COMMAND CONTROL',
    path: ['SET-UP', 'BUTTON/DIAL SETTING'],
    summary: 'Choose the setting the command control adjusts.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
  },
  control_ring_setting: {
    title: 'CONTROL RING SETTING',
    path: ['SHOOTING MENU'],
    summary: null,
    choices: {},
    related: [],
    manual: 'menu_shooting/control_ring_setting',
    changes: 'Adds exposure compensation and mod actions to the control ring settings.',
  },
  menu_theme_item: {
    title: 'MENU THEME',
    path: ['SET-UP'],
    summary: 'Choose the appearance of the menus.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
  },
  edit_save_custom_setting: {
    title: 'EDIT/SAVE CUSTOM SETTING',
    path: ['SHOOTING MENU'],
    summary: null,
    choices: {},
    related: [],
    manual: 'menu_shooting/edit-save_custom_setting',
    changes:
      'Each slot C1–C7 holds a recipe saved as a named file on the memory card, including ISO, white balance and white balance shift. Choose SAVE CURRENT SETTINGS, SAVE AS NEW RECIPE, CHOOSE FROM SD CARD or CLEAR SLOT.',
  },
  snap_menu_item: {
    title: 'MF SNAP FOCUS',
    path: ['SHOOTING MENU'],
    summary: 'Choose the distance the lens moves to when you press AE/AF-L in manual focus mode.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
  },
  bug_report_shortcut: {
    title: 'BUG REPORT SHORTCUT',
    path: ['SET-UP', 'FUJIMOD DEVTOOLS'],
    summary: 'Choose whether the side Fn button and DISP/BACK save a bug report.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
  },
};

function Icon(props: { name: string }) {
  return (
    <svg class="catalog-icon" aria-hidden="true">
      <use href={`${featureIcons}#${props.name}`} />
    </svg>
  );
}

/** An aspect ratio choice drawn as its frame. */
function Ratio(props: { width: number; height: number }) {
  const scale = () => 14 / Math.max(props.width, props.height);
  return (
    <svg class="catalog-ratio" viewBox="0 0 14 14" aria-hidden="true">
      <rect
        x={(14 - props.width * scale()) / 2}
        y={(14 - props.height * scale()) / 2}
        width={props.width * scale()}
        height={props.height * scale()}
        rx="1"
      />
    </svg>
  );
}

function CardBody(props: { target: Target }) {
  const card = () => CARDS[props.target.card];
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
        <p>{card().choices[choice()!]}</p>
      </Show>
      <div class="catalog-card-path">{path()}</div>
      <Show when={!choice() && card().related.length}>
        <ul class="catalog-card-related">
          <For each={card().related}>
            {(related) => (
              <li>
                <b>{CARDS[related.card].title}</b>: {related.reason}
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
  let card: HTMLDivElement | undefined;
  onCleanup(() => clearTimeout(timer));

  const place = (anchor: HTMLElement) => {
    const r = anchor.getBoundingClientRect();
    const width = Math.min(340, innerWidth - 16);
    const height = card?.offsetHeight ?? 0;
    let top = r.bottom + 10;
    if (top + height > innerHeight - 8) top = Math.max(8, r.top - height - 10);
    setPosition({
      left: Math.min(Math.max(8, r.left + r.width / 2 - width / 2), innerWidth - width - 8),
      top,
    });
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
  /** A control or option with a hover card: after a short hover, on focus or on tap. */
  const Ref = (props: { card: string; choice?: string; class?: string; children: JSX.Element }) => (
    <span
      class={props.class ? `${props.class} catalog-ref` : 'catalog-ref'}
      tabindex="0"
      onMouseEnter={(event) => {
        const anchor = event.currentTarget;
        later(() => show({ card: props.card, choice: props.choice, anchor }), 250);
      }}
      onMouseLeave={() => later(() => setTarget(), 150)}
      onFocus={(event) =>
        show({ card: props.card, choice: props.choice, anchor: event.currentTarget })
      }
      onBlur={() => later(() => setTarget(), 150)}
      onClick={(event) => {
        const anchor = event.currentTarget;
        if (target()?.anchor === anchor) setTarget();
        else show({ card: props.card, choice: props.choice, anchor });
      }}
    >
      {props.children}
    </span>
  );

  return (
    <section class="catalog" id="features" aria-labelledby="catalog-heading">
      <h2 id="catalog-heading">Features</h2>
      <h3>Making pictures</h3>
      <div class="catalog-grid">
        <article class="catalog-tile">
          <h4>
            <Icon name="film" />
            <Ref card="film_simulation">FILM SIMULATION</Ref>
          </h4>
          <p>Simulate the effects of additional types of film.</p>
          <div class="catalog-chips">
            <span class="catalog-chip">
              <Icon name="eterna" />
              ETERNA
            </span>
            <span class="catalog-chip">
              <Icon name="classic-neg" />
              Classic Neg. (approx.)
            </span>
            <span class="catalog-chip">
              <Icon name="bleach-bypass" />
              Bleach Bypass (approx.)
            </span>
            <span class="catalog-chip">
              <Icon name="nostalgic-neg" />
              Nostalgic Neg. (approx.)
            </span>
            <span class="catalog-chip">
              <Icon name="reala-ace" />
              REALA ACE (approx.)
            </span>
            <span class="catalog-chip">Neutral</span>
          </div>
          <div class="catalog-where">SHOOTING MENU</div>
        </article>
        <article class="catalog-tile">
          <h4>
            <Icon name="film" />
            <Ref card="film_editor_item">FILM EDITOR</Ref>
          </h4>
          <p>Create and edit your own film simulations, saved as files on the memory card.</p>
          <div class="catalog-where">SHOOTING MENU</div>
        </article>
        <article class="catalog-tile">
          <h4>
            <Icon name="recipes" />
            <span>Unlimited Recipes</span>
          </h4>
          <p>
            Store custom settings as named files on the memory card and load them into{' '}
            <span class="nowrap">C1–C7</span>. ISO, white balance and white balance shift are saved
            with each recipe.
          </p>
          <div class="catalog-chips">
            <span class="catalog-chip action">SAVE CURRENT SETTINGS</span>
            <span class="catalog-chip action">SAVE AS NEW RECIPE</span>
            <span class="catalog-chip action">CHOOSE FROM SD CARD</span>
          </div>
          <div class="catalog-where">SHOOTING MENU › EDIT/SAVE CUSTOM SETTING</div>
        </article>
        <article class="catalog-tile">
          <h4>
            <Icon name="aspect" />
            <Ref card="shape_aspect_item">ASPECT RATIO</Ref>
          </h4>
          <p>Choose the aspect ratio of still pictures.</p>
          <div class="catalog-chips">
            <Ref card="shape_aspect_item" choice="3:2" class="catalog-chip stock">
              <Ratio width={3} height={2} />
              3:2
            </Ref>
            <Ref card="shape_aspect_item" choice="16:9" class="catalog-chip stock">
              <Ratio width={16} height={9} />
              16:9
            </Ref>
            <Ref card="shape_aspect_item" choice="1:1" class="catalog-chip stock">
              <Ratio width={1} height={1} />
              1:1
            </Ref>
            <Ref card="shape_aspect_item" choice="4:3" class="catalog-chip">
              <Ratio width={4} height={3} />
              4:3
            </Ref>
            <Ref card="shape_aspect_item" choice="65:24" class="catalog-chip">
              <Ratio width={65} height={24} />
              65:24
            </Ref>
            <Ref card="shape_aspect_item" choice="17:6" class="catalog-chip">
              <Ratio width={17} height={6} />
              17:6
            </Ref>
            <Ref card="shape_aspect_item" choice="3:4" class="catalog-chip">
              <Ratio width={3} height={4} />
              3:4
            </Ref>
            <Ref card="shape_aspect_item" choice="7:6" class="catalog-chip">
              <Ratio width={7} height={6} />
              7:6
            </Ref>
            <Ref card="shape_aspect_item" choice="5:4" class="catalog-chip">
              <Ratio width={5} height={4} />
              5:4
            </Ref>
          </div>
          <div class="catalog-sub">
            <h5>
              <Ref card="shape_surround_item">SURROUND VIEW</Ref>
            </h5>
            <div class="catalog-chips">
              <Ref card="shape_surround_item" choice="MASKED" class="catalog-chip">
                MASKED
              </Ref>
              <Ref card="shape_surround_item" choice="DARKENED" class="catalog-chip">
                DARKENED
              </Ref>
              <Ref card="shape_surround_item" choice="WHITE BORDER" class="catalog-chip">
                WHITE BORDER
              </Ref>
            </div>
          </div>
          <div class="catalog-where">{'SHOOTING MENU › SHAPE & ASPECT'}</div>
        </article>
        <article class="catalog-tile">
          <h4>
            <Icon name="anamorphic" />
            <span>Anamorphic Lenses</span>
          </h4>
          <p>
            Display and record pictures taken through an anamorphic adapter at their corrected
            width.
          </p>
          <div class="catalog-sub">
            <h5>
              <Ref card="shape_desqueeze_item">DESQUEEZE</Ref>
            </h5>
            <div class="catalog-chips">
              <span class="catalog-chip">1.25X</span>
              <span class="catalog-chip">1.33X</span>
              <span class="catalog-chip">1.50X</span>
              <span class="catalog-chip">1.60X</span>
              <span class="catalog-chip">1.80X</span>
              <span class="catalog-chip">2.00X</span>
            </div>
          </div>
          <div class="catalog-sub">
            <h5>
              <Ref card="shape_jpeg_item">DESQUEEZE JPEG</Ref>
            </h5>
            <div class="catalog-chips">
              <span class="catalog-chip">ON</span>
              <span class="catalog-chip">OFF</span>
            </div>
          </div>
          <div class="catalog-where">{'SHOOTING MENU › SHAPE & ASPECT'}</div>
        </article>
        <article class="catalog-tile">
          <h4>
            <Icon name="snap" />
            <Ref card="snap_menu_item">MF SNAP FOCUS</Ref>
          </h4>
          <p>In manual focus mode, press AE/AF-L to focus at a preset distance.</p>
          <div class="catalog-where">SHOOTING MENU</div>
        </article>
      </div>
      <h3>Handling and display</h3>
      <div class="catalog-grid">
        <article class="catalog-tile">
          <h4>
            <Icon name="controls" />
            <span>Unlocked Controls</span>
          </h4>
          <p>
            Customize every button, dial, and lever. Adjust ISO by turning the exposure dial, or
            switch between picture and movie mode.
          </p>
        </article>
        <article class="catalog-tile">
          <h4>
            <Icon name="hdmi" />
            <Ref card="hdmi_rec_behavior_item">HDMI OUTPUT</Ref>
          </h4>
          <p>Display the shooting view on an HDMI device.</p>
          <div class="catalog-where">SET-UP</div>
        </article>
        <article class="catalog-tile">
          <h4>
            <Icon name="theme" />
            <Ref card="menu_theme_item">MENU THEME</Ref>
          </h4>
          <p>Choose the appearance of the menus.</p>
          <div class="catalog-where">SET-UP</div>
        </article>
        <article class="catalog-tile">
          <h4>
            <Icon name="autofocus" />
            <span>Autofocus</span>
          </h4>
          <p>
            In AF-S mode, when phase detection fails, the camera estimates focus and measures again
            before using contrast detection.
          </p>
        </article>
      </div>
      <h3>When something goes wrong</h3>
      <div class="catalog-grid">
        <article class="catalog-tile">
          <h4>
            <Icon name="bug" />
            <span>Bug Reports</span>
          </h4>
          <p>
            Hold the side Fn button and press DISP/BACK to save a diagnostic report to the memory
            card and display a QR code.
          </p>
        </article>
      </div>
      <div
        ref={(element) => {
          card = element;
        }}
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
