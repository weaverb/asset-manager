import type { ComponentType } from "react";
import {
  SILHOUETTE_PREVIEW_WIDTHS,
  type SilhouetteIconProps,
} from "../icons/silhouettes";
import {
  AccessoryAccIcon,
  AowIcon,
  PartScrewIcon,
  PccSubIcon,
  PistolBerettaIcon,
  PistolCartridgeIcon,
  RevolverXvrIcon,
  RifleBoltIcon,
  RifleSemiAutoIcon,
  RifleCartridgeIcon,
  ShotgunCartridgeIcon,
  SubtypeOtherIcon,
} from "../icons";

const ROWS: { label: string; Icon: ComponentType<SilhouetteIconProps> }[] = [
  { label: "Pistol cartridge", Icon: PistolCartridgeIcon },
  { label: "Rifle cartridge", Icon: RifleCartridgeIcon },
  { label: "Shotgun shell", Icon: ShotgunCartridgeIcon },
  { label: "Pistol (M9 SVG)", Icon: PistolBerettaIcon },
  { label: "Rifle (semi-auto / AR-15 SVG)", Icon: RifleSemiAutoIcon },
  { label: "Rifle (bolt-action SVG)", Icon: RifleBoltIcon },
  { label: "Revolver (revolver_icon.svg)", Icon: RevolverXvrIcon },
  { label: "PCC / Sub (sub_icon.svg)", Icon: PccSubIcon },
  { label: "Other / AOW (aow_icon.svg)", Icon: AowIcon },
  { label: "Accessory (acc_icon.svg)", Icon: AccessoryAccIcon },
  { label: "Ammunition other (tile)", Icon: SubtypeOtherIcon },
  { label: "Part (bolt SVG)", Icon: PartScrewIcon },
];

/**
 * Dev-only: grid of silhouette SVGs at multiple sizes to verify scaling and colors.
 * Shown inside `SilhouetteSvgBrowserModal` from Settings (development builds only).
 */
export function SilhouetteSvgBrowser() {
  return (
    <div className="svg-browser">
      <p className="svg-browser-lead">
        Each cell is a fixed square <strong>N×N px</strong> (64, 128, 256, 512).
        Icons use <code className="mono-inline">layoutBoxPx</code> so artwork scales
        with <code className="mono-inline">meet</code> /{" "}
        <code className="mono-inline">mask-size: contain</code> inside the box.
        Inline paths use <code className="mono-inline">currentColor</code>; bundled
        SVGs use a CSS mask so they tint with swatch{" "}
        <code className="mono-inline">color</code> in light and dark.
      </p>
      <div className="svg-browser-scroll">
        <table className="svg-browser-table">
          <thead>
            <tr>
              <th scope="col" className="svg-browser-th svg-browser-th--label">
                Icon
              </th>
              {SILHOUETTE_PREVIEW_WIDTHS.map((px) => (
                <th key={px} scope="col" className="svg-browser-th">
                  {px}×{px}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {ROWS.map(({ label, Icon }) => (
              <tr key={label}>
                <th scope="row" className="svg-browser-th svg-browser-th--label">
                  {label}
                </th>
                {SILHOUETTE_PREVIEW_WIDTHS.map((px) => (
                  <td key={px} className="svg-browser-td">
                    <div className="svg-browser-swatch">
                      <div
                        className="svg-browser-swatch-inner"
                        style={{
                          width: px,
                          height: px,
                          minWidth: px,
                          minHeight: px,
                          boxSizing: "border-box",
                        }}
                      >
                        <Icon
                          layoutBoxPx={px}
                          title={`${label} in ${px}×${px} px box`}
                        />
                      </div>
                    </div>
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
