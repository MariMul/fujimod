// @generated
// This file is code generated. Do not edit directly.
// Update the feature documentation sources and regenerate the catalog.
import { For, Show, createSignal, onCleanup, untrack } from 'solid-js';
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
  media: Media | null;
};
/** Preview recording: clean clip, still, and each option's range in the clip. */
type Media = { video: string; still: string; clips: Record<string, [number, number]> };
/** What a hover card shows: a control, or one option of it. */
type Target = { card: string; choice?: string; anchor: HTMLElement };

const MANUAL = 'https://fujifilm-dsc.com/en/manual/x70/';
const MEDIA = '/artifacts/docs/';
/** An X70 manual page, optionally with an anchor ("menu_setup/screen_set-up#framing_guidline"). */
const manualUrl = (manual: string) => {
  const [page, anchor] = manual.split('#');
  return `${MANUAL}${page}/index.html${anchor ? `#${anchor}` : ''}`;
};
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
    media: {
      video: '8f2228a9aa821724e27d820e.mp4',
      still: '7344d905c6565071d4d8884a.webp',
      clips: {
        '4:3': [2.4, 3.6],
        '16:9': [4.4, 5.6],
        '65:24': [6.333, 7.533],
        '17:6': [8.2, 9.4],
        '3:4': [10.2, 11.4],
        '1:1': [12.133, 13.333],
        '7:6': [14.0, 15.2],
        '5:4': [15.867, 17.067],
      },
    },
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
    media: {
      video: 'a599c8ea004343cc7bf5c5f2.mp4',
      still: 'c93cab41b0bcc26ed5aea52b.webp',
      clips: {
        MASKED: [2.067, 3.533],
        DARKENED: [4.233, 5.733],
        'WHITE BORDER': [6.367, 7.867],
      },
    },
  },
  shape_desqueeze_item: {
    title: 'DESQUEEZE',
    path: ['SHOOTING MENU', 'SHAPE & ASPECT'],
    summary: 'Choose the squeeze factor of your anamorphic adapter.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
    media: {
      video: '31aa03626b40a910b877a7f5.mp4',
      still: '07fd4db58388f822468f4871.webp',
      clips: {},
    },
  },
  shape_jpeg_item: {
    title: 'DESQUEEZE JPEG',
    path: ['SHOOTING MENU', 'SHAPE & ASPECT'],
    summary: 'Choose whether JPEG pictures are also recorded at their corrected width.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
    media: {
      video: '562d594eca1215d6ea709d19.mp4',
      still: 'c5103abae1e1a79a4c81f5fd.webp',
      clips: {},
    },
  },
  c: {
    title: 'FRAMING GUIDELINE',
    path: ['SET-UP', 'SCREEN SET-UP'],
    summary: null,
    choices: {
      'GOLDEN RATIO': 'Lines that divide the frame at the golden ratio.',
    },
    related: [],
    manual: 'menu_setup/screen_set-up#framing_guidline',
    changes:
      'Adds GOLDEN RATIO, which divides the frame at the golden ratio. Guides follow the chosen aspect ratio.',
    media: {
      video: 'a16bcfc714c15f7b7d15904b.mp4',
      still: '55e64c975294de3fe1a29ddf.webp',
      clips: {
        'GOLDEN RATIO': [0.0, 3.0],
      },
    },
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
    media: {
      video: '0d1dee8fd5ba8dc790a09efe.mp4',
      still: '87e626806aba3963efd315b6.webp',
      clips: {},
    },
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
    media: {
      video: 'e144c7f61367a8ac9ae8ec87.mp4',
      still: '82a20c7ea43ac8224478d2e0.webp',
      clips: {},
    },
  },
  film_editor_item: {
    title: 'FILM EDITOR',
    path: ['SHOOTING MENU'],
    summary: 'Create and edit film simulations, saved as files on the memory card.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
    media: {
      video: '2606e857d463afd72ae889b6.mp4',
      still: 'f96885559d7ea08b8b14940c.webp',
      clips: {},
    },
  },
  photometry: {
    title: 'PHOTOMETRY',
    path: ['SHOOTING MENU'],
    summary: null,
    choices: {
      HIGHLIGHT:
        'Exposure is reduced when bright areas dominate the scene. A small bright detail may still be overexposed.',
    },
    related: [],
    manual: 'menu_shooting/photometry',
    changes:
      'Adds HIGHLIGHT, which reduces exposure when bright areas dominate the scene. The shooting, Fn and Q displays mark it with an asterisk in brackets.',
    media: {
      video: 'fba276f690101281f306f16e.mp4',
      still: 'b299b67ddaa0124df953f638.webp',
      clips: {
        HIGHLIGHT: [5.4, 6.6],
      },
    },
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
    media: {
      video: '99e638a541d48837efdaacd5.mp4',
      still: '6c69d6d3a2e2dc5c16b13e11.webp',
      clips: {},
    },
  },
  bindings_row_exposure_dial: {
    title: 'EV DIAL',
    path: ['SET-UP', 'BUTTON/DIAL SETTING'],
    summary: 'Choose the setting the exposure compensation dial adjusts.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
    media: {
      video: '19f720b7c6ddb0b29729302b.mp4',
      still: '05c772863c343af831174d4a.webp',
      clips: {},
    },
  },
  bindings_row_command_dial: {
    title: 'COMMAND CONTROL',
    path: ['SET-UP', 'BUTTON/DIAL SETTING'],
    summary: 'Choose the setting the command control adjusts.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
    media: {
      video: 'c85986eacecaf8b482ed5a31.mp4',
      still: 'a50d677c3c00ac62e8e71465.webp',
      clips: {},
    },
  },
  control_ring_setting: {
    title: 'CONTROL RING SETTING',
    path: ['SHOOTING MENU'],
    summary: null,
    choices: {},
    related: [],
    manual: 'menu_shooting/control_ring_setting',
    changes:
      'Adds EXPOSURE COMP. and the FujiMod settings ASPECT RATIO, SELECT RECIPE and SURROUND VIEW.',
    media: {
      video: '4fc0400b834e42c1f928a895.mp4',
      still: '36df5a09eb7a8fe1a582b153.webp',
      clips: {},
    },
  },
  menu_theme_item: {
    title: 'MENU THEME',
    path: ['SET-UP'],
    summary: 'Choose the appearance of the menus.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
    media: {
      video: '8f2581c314f148e8932546bb.mp4',
      still: 'fab7ea3af462c39c7be32fcd.webp',
      clips: {},
    },
  },
  edit_save_custom_setting: {
    title: 'EDIT/SAVE CUSTOM SETTING',
    path: ['SHOOTING MENU'],
    summary: null,
    choices: {},
    related: [],
    manual: 'menu_shooting/edit-save_custom_setting',
    changes:
      'Each slot C1–C7 holds a recipe saved as a named file on the memory card. Choose SAVE CURRENT SETTINGS, SAVE AS NEW RECIPE, CHOOSE FROM SD CARD or CLEAR SLOT.',
    media: {
      video: '8791cbbad1c9851095aeadbf.mp4',
      still: 'dbf279fdb2d032d3521cf2f6.webp',
      clips: {},
    },
  },
  snap_menu_item: {
    title: 'MF SNAP FOCUS',
    path: ['SHOOTING MENU'],
    summary: 'Choose the distance the lens moves to when you press AE/AF-L in manual focus mode.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
    media: {
      video: 'e30c206a4d6ae0e1f6cb4b38.mp4',
      still: '61012a6defebe45e0b455162.webp',
      clips: {},
    },
  },
  bug_report_shortcut: {
    title: 'BUG REPORT SHORTCUT',
    path: ['SET-UP', 'FUJIMOD DEVTOOLS'],
    summary: 'Choose whether the side Fn button and DISP/BACK save a bug report.',
    choices: {},
    related: [],
    manual: null,
    changes: null,
    media: {
      video: '2b974b2380611cabd247d8f0.mp4',
      still: 'b9305752ea40f97ee080cb67.webp',
      clips: {},
    },
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

function CardVideo(props: { media: Media; choice?: string }) {
  let video: HTMLVideoElement | undefined;
  // A release without docs media (or a missing file) shows no video at all.
  const [failed, setFailed] = createSignal(false);
  // Read once: the card's own range, still valid while its video is closing.
  const range = untrack(() => (props.choice ? props.media.clips[props.choice] : undefined));
  const clip = () => range;
  return (
    <Show when={!failed()}>
      <video
        ref={(element) => {
          video = element;
        }}
        class="catalog-card-video"
        src={MEDIA + props.media.video}
        poster={MEDIA + props.media.still}
        muted
        autoplay
        loop
        playsinline
        preload="metadata"
        onLoadedMetadata={() => {
          const range = clip();
          if (video && range) video.currentTime = range[0];
        }}
        onTimeUpdate={() => {
          const range = clip();
          if (video && range && video.currentTime >= range[1]) video.currentTime = range[0];
        }}
        onError={() => setFailed(true)}
      />
    </Show>
  );
}

/** A tile's preview: plays inline wherever the tile is visible, no hover needed. */
function TileVideo(props: { card: string }) {
  const media = () => CARDS[props.card].media!;
  const [failed, setFailed] = createSignal(false);
  return (
    <Show when={!failed()}>
      <video
        class="catalog-tile-video"
        src={MEDIA + media().video}
        poster={MEDIA + media().still}
        muted
        autoplay
        loop
        playsinline
        preload="metadata"
        onError={() => setFailed(true)}
      />
    </Show>
  );
}

function CardBody(props: { target: Target }) {
  const card = () => CARDS[props.target.card];
  const choice = () => props.target.choice;
  const path = () => [...card().path, card().title, ...(choice() ? [choice()!] : [])].join(' › ');
  return (
    <>
      <Show when={card().media}>{(media) => <CardVideo media={media()} choice={choice()} />}</Show>
      <div class="catalog-card-title">{choice() ?? card().title}</div>
      <Show
        when={choice()}
        fallback={
          <Show when={card().changes} fallback={<p>{card().summary}</p>}>
            <p>In FujiMod: {card().changes}</p>
            <a href={manualUrl(card().manual!)} target="_blank" rel="noreferrer">
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
          <TileVideo card="film_simulation" />
          <h4>
            <Icon name="film" />
            <Ref card="film_simulation">FILM SIMULATION</Ref>
          </h4>
          <p>
            Simulate the effects of additional types of film. Create your own film simulations in
            FILM EDITOR.
          </p>
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
          <TileVideo card="photometry" />
          <h4>
            <Icon name="highlight" />
            <Ref card="photometry">PHOTOMETRY</Ref>
          </h4>
          <p>Choose HIGHLIGHT to reduce exposure when bright areas dominate the scene.</p>
          <div class="catalog-chips">
            <span class="catalog-chip stock">MULTI</span>
            <span class="catalog-chip stock">SPOT</span>
            <span class="catalog-chip stock">AVERAGE</span>
            <Ref card="photometry" choice="HIGHLIGHT" class="catalog-chip">
              HIGHLIGHT
            </Ref>
          </div>
          <div class="catalog-where">SHOOTING MENU</div>
        </article>
        <article class="catalog-tile">
          <TileVideo card="edit_save_custom_setting" />
          <h4>
            <Icon name="recipes" />
            <span>Unlimited Recipes</span>
          </h4>
          <p>
            Store custom settings as named files on the memory card and load them into{' '}
            <span class="nowrap">C1–C7</span>.
          </p>
          <div class="catalog-chips">
            <span class="catalog-chip action">SAVE CURRENT SETTINGS</span>
            <span class="catalog-chip action">SAVE AS NEW RECIPE</span>
            <span class="catalog-chip action">CHOOSE FROM SD CARD</span>
          </div>
          <div class="catalog-where">SHOOTING MENU › EDIT/SAVE CUSTOM SETTING</div>
        </article>
        <article class="catalog-tile">
          <TileVideo card="shape_aspect_item" />
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
          <TileVideo card="c" />
          <h4>
            <Icon name="golden-ratio" />
            <Ref card="c">FRAMING GUIDELINE</Ref>
          </h4>
          <p>Choose a guide to display in the shooting view, including the golden ratio.</p>
          <div class="catalog-chips">
            <span class="catalog-chip stock">GRID 9</span>
            <span class="catalog-chip stock">GRID 24</span>
            <span class="catalog-chip stock">HD FRAMING</span>
            <Ref card="c" choice="GOLDEN RATIO" class="catalog-chip">
              GOLDEN RATIO
            </Ref>
          </div>
          <div class="catalog-where">SET-UP › SCREEN SET-UP</div>
        </article>
        <article class="catalog-tile">
          <TileVideo card="shape_desqueeze_item" />
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
          <TileVideo card="snap_menu_item" />
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
          <TileVideo card="bindings_row_exposure_dial" />
          <h4>
            <Icon name="controls" />
            <span>Unlocked Controls</span>
          </h4>
          <p>
            Assign ISO, white balance, film simulation, ASPECT RATIO, SELECT RECIPE or other
            settings to the exposure compensation dial, command control or control ring.
          </p>
        </article>
        <article class="catalog-tile">
          <TileVideo card="hdmi_rec_behavior_item" />
          <h4>
            <Icon name="hdmi" />
            <Ref card="hdmi_rec_behavior_item">HDMI OUTPUT</Ref>
          </h4>
          <p>Display the shooting view on an HDMI device.</p>
          <div class="catalog-where">SET-UP</div>
        </article>
        <article class="catalog-tile">
          <TileVideo card="menu_theme_item" />
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
          <TileVideo card="bug_report_shortcut" />
          <h4>
            <Icon name="bug" />
            <span>Bug Reports</span>
          </h4>
          <p>
            Hold the side Fn button and press DISP/BACK to save a diagnostic report to the memory
            card and display a QR code.
          </p>
          <div class="catalog-where">SET-UP › FUJIMOD DEVTOOLS › BUG REPORT SHORTCUT</div>
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
