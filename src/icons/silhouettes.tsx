import type { CSSProperties, ReactNode, SVGAttributes } from "react";
import accIconUrl from "./acc_icon.svg?url";
import aowIconUrl from "./aow_icon.svg?url";
import ar15IconUrl from "./AR15_icon.svg?url";
import boltrifleIconUrl from "./boltrifle_icon.svg?url";
import m9IconUrl from "./m9_icon.svg?url";
import partIconUrl from "./part_icon.svg?url";
import revolverIconUrl from "./revolver_icon.svg?url";
import subIconUrl from "./sub_icon.svg?url";
import pistolAmmoIconUrl from "./pistol_ammo_icon.svg?url";
import rifleAmmoIconUrl from "./rifle_ammo_icon.svg?url";
import shotgunAmmoIconUrl from "./shotgun_ammo_icon.svg?url";

/** Horizontal viewport: width:height 2:1; both dimensions ≥ 64px → min 128×64. */
export const SILHOUETTE_HORIZONTAL_MIN_W = 128;
export const SILHOUETTE_HORIZONTAL_MIN_H = 64;

/** Vertical viewport: width:height 1:2; both dimensions ≥ 64px → min 64×128. */
export const SILHOUETTE_VERTICAL_MIN_W = 64;
export const SILHOUETTE_VERTICAL_MIN_H = 128;

/** Dev SVG browser: square preview box sizes (`layoutBoxPx`); also used as column headers. */
export const SILHOUETTE_PREVIEW_WIDTHS = [64, 128, 256, 512] as const;

/** All-assets table: square cell; silhouette keeps 2:1 or 1:2 inside via meet/contain. */
export const ASSET_TABLE_ROW_ICON_BOX_PX = 64;

export type SilhouetteIconProps = Omit<
  SVGAttributes<SVGSVGElement>,
  "viewBox" | "xmlns" | "children"
> & {
  /**
   * Target **width** of the layout box (number = px). The box keeps a fixed aspect ratio
   * (horizontal 2:1, vertical 1:2), is clamped so neither side is below 64px, and the
   * artwork scales with `meet` / `contain` so the full graphic stays visible.
   * Ignored when `layoutBoxPx` is set.
   */
  size?: number | string;
  /**
   * Square viewport (px). Artwork is scaled with `meet` / `mask-size: contain` so intrinsic
   * 2:1 or 1:2 silhouettes stay proportional and centered inside the cell (e.g. asset table).
   */
  layoutBoxPx?: number;
  /** Accessible label; omit when decorative. */
  title?: string;
};

function squareLayoutBox(layoutBoxPx: number): { width: number; height: number } {
  const n = Math.max(1, Math.round(layoutBoxPx));
  return { width: n, height: n };
}

/** Resolved pixel width/height for numeric `size`; string sizes use CSS only. */
function silhouetteBoxStyle(
  size: number | string | undefined,
  horizontal: boolean,
  layoutBoxPx?: number,
): { width: number; height: number } | null {
  if (layoutBoxPx != null && layoutBoxPx > 0) {
    return squareLayoutBox(layoutBoxPx);
  }
  if (typeof size === "string") {
    return null;
  }
  const s =
    size ?? (horizontal ? SILHOUETTE_HORIZONTAL_MIN_W : SILHOUETTE_VERTICAL_MIN_W);
  if (horizontal) {
    const w = Math.max(s, SILHOUETTE_HORIZONTAL_MIN_W);
    return { width: w, height: Math.max(Math.round(w / 2), SILHOUETTE_HORIZONTAL_MIN_H) };
  }
  const w = Math.max(s, SILHOUETTE_VERTICAL_MIN_W);
  const h = Math.max(w * 2, SILHOUETTE_VERTICAL_MIN_H);
  return { width: w, height: h };
}

function silhouetteBoxCss(
  size: number | string | undefined,
  horizontal: boolean,
  layoutBoxPx?: number,
): CSSProperties {
  if (layoutBoxPx != null && layoutBoxPx > 0) {
    const b = squareLayoutBox(layoutBoxPx);
    return { width: b.width, height: b.height };
  }
  const px = silhouetteBoxStyle(size, horizontal, undefined);
  if (px) {
    return { width: px.width, height: px.height };
  }
  const w =
    typeof size === "string"
      ? size
      : horizontal
        ? `${SILHOUETTE_HORIZONTAL_MIN_W}px`
        : `${SILHOUETTE_VERTICAL_MIN_W}px`;
  if (horizontal) {
    return {
      width: w,
      aspectRatio: "2 / 1",
      minWidth: SILHOUETTE_HORIZONTAL_MIN_W,
      minHeight: SILHOUETTE_HORIZONTAL_MIN_H,
      height: "auto",
    };
  }
  return {
    width: w,
    aspectRatio: "1 / 2",
    minWidth: SILHOUETTE_VERTICAL_MIN_W,
    minHeight: SILHOUETTE_VERTICAL_MIN_H,
    height: "auto",
  };
}

const defaults = {
  fill: "currentColor",
  xmlns: "http://www.w3.org/2000/svg",
} as const;

function wrap(
  props: SilhouetteIconProps,
  viewBox: string,
  horizontal: boolean,
  children: ReactNode,
) {
  const {
    size,
    title,
    className,
    style,
    layoutBoxPx,
    preserveAspectRatio = "xMidYMid meet",
    ...rest
  } = props;
  const box = silhouetteBoxCss(size, horizontal, layoutBoxPx);
  const sizeStyle: CSSProperties = { ...box, ...style };

  return (
    <svg
      {...defaults}
      viewBox={viewBox}
      preserveAspectRatio={preserveAspectRatio}
      className={className}
      style={sizeStyle}
      role={title ? "img" : "presentation"}
      aria-hidden={title ? undefined : true}
      {...rest}
    >
      {title ? <title>{title}</title> : null}
      {children}
    </svg>
  );
}

/**
 * Bundled SVG as a silhouette: `mask-image` + `background-color: currentColor`
 * so light mode picks up parent grays (e.g. table / drawer) instead of dark embedded fills.
 */
function wrapSvgUrl(
  props: SilhouetteIconProps,
  src: string,
  horizontal: boolean,
) {
  const { size, title, className, style, layoutBoxPx } = props;
  const box = silhouetteBoxCss(size, horizontal, layoutBoxPx);
  const sizeStyle: CSSProperties = { ...box, ...style };

  const maskUrl = `url(${JSON.stringify(src)})`;

  return (
    <span
      className={["silhouette-svg-mask", className].filter(Boolean).join(" ")}
      style={{
        ...sizeStyle,
        WebkitMaskImage: maskUrl,
        maskImage: maskUrl,
      }}
      role={title ? "img" : undefined}
      aria-label={title || undefined}
      aria-hidden={title ? undefined : true}
    />
  );
}

/** Pistol cartridge — `pistol_ammo_icon.svg`, vertical. */
export function PistolCartridgeIcon(props: SilhouetteIconProps) {
  return wrapSvgUrl(props, pistolAmmoIconUrl, false);
}

/** Rifle cartridge — `rifle_ammo_icon.svg`, vertical. */
export function RifleCartridgeIcon(props: SilhouetteIconProps) {
  return wrapSvgUrl(props, rifleAmmoIconUrl, false);
}

/** Shotgun shell — `shotgun_ammo_icon.svg`, vertical. */
export function ShotgunCartridgeIcon(props: SilhouetteIconProps) {
  return wrapSvgUrl(props, shotgunAmmoIconUrl, false);
}

/** Pistol — `m9_icon.svg` (muzzle right). */
export function PistolBerettaIcon(props: SilhouetteIconProps) {
  return wrapSvgUrl(props, m9IconUrl, true);
}

/** Semi-auto rifle — `AR15_icon.svg` (muzzle right). */
export function RifleSemiAutoIcon(props: SilhouetteIconProps) {
  return wrapSvgUrl(props, ar15IconUrl, true);
}

/** Bolt-action rifle — `boltrifle_icon.svg` (muzzle right). */
export function RifleBoltIcon(props: SilhouetteIconProps) {
  return wrapSvgUrl(props, boltrifleIconUrl, true);
}

/** Revolver — `revolver_icon.svg` (XVR-class side view, muzzle right). */
export function RevolverXvrIcon(props: SilhouetteIconProps) {
  return wrapSvgUrl(props, revolverIconUrl, true);
}

/** Firearm subtype “other” / AOW — `aow_icon.svg` (horizontal). */
export function AowIcon(props: SilhouetteIconProps) {
  return wrapSvgUrl(props, aowIconUrl, true);
}

/** Firearm subtype PCC / subgun — `sub_icon.svg` (horizontal). */
export function PccSubIcon(props: SilhouetteIconProps) {
  return wrapSvgUrl(props, subIconUrl, true);
}

/** All accessory assets — `acc_icon.svg` (subtype affects form only, not the icon). */
export function AccessoryAccIcon(props: SilhouetteIconProps) {
  return wrapSvgUrl(props, accIconUrl, true);
}

/** Generic “other” subtype marker (rounded tile). */
export function SubtypeOtherIcon(props: SilhouetteIconProps) {
  return wrap(
    props,
    "0 0 24 24",
    false,
    <path d="M5 3 L19 3 C20.5 3 21 3.5 21 5 L21 19 C21 20.5 20.5 21 19 21 L5 21 C3.5 21 3 20.5 3 19 L3 5 C3 3.5 3.5 3 5 3 Z" />,
  );
}

/** Part / bolt — `part_icon.svg`, vertical. */
export function PartScrewIcon(props: SilhouetteIconProps) {
  return wrapSvgUrl(props, partIconUrl, false);
}
