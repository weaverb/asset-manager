import type { Asset } from "../types";
import type { SilhouetteIconProps } from "../icons/silhouettes";
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

export type AssetKindIconProps = SilhouetteIconProps & {
  asset: Pick<Asset, "kind" | "subtype">;
};

/**
 * Silhouette icon from asset kind + subtype (firearm, ammunition).
 * Parts use a fixed icon; accessory subtype is ignored for the icon (all use `acc_icon.svg`).
 */
export function AssetKindIcon({ asset, ...rest }: AssetKindIconProps) {
  const s = asset.subtype?.toLowerCase().trim() ?? "";

  if (asset.kind === "firearm") {
    switch (s) {
      case "pistol":
        return <PistolBerettaIcon {...rest} />;
      case "semi_auto":
        return <RifleSemiAutoIcon {...rest} />;
      case "bolt_action":
      case "rifle":
        return <RifleBoltIcon {...rest} />;
      case "revolver":
        return <RevolverXvrIcon {...rest} />;
      case "shotgun":
        return <ShotgunCartridgeIcon {...rest} />;
      case "pcc_sub":
        return <PccSubIcon {...rest} />;
      case "other":
      default:
        return <AowIcon {...rest} />;
    }
  }

  if (asset.kind === "accessory") {
    return <AccessoryAccIcon {...rest} />;
  }

  if (asset.kind === "part") {
    return <PartScrewIcon {...rest} />;
  }

  if (asset.kind === "ammunition") {
    switch (s) {
      case "pistol":
        return <PistolCartridgeIcon {...rest} />;
      case "shotgun":
        return <ShotgunCartridgeIcon {...rest} />;
      case "other":
        return <SubtypeOtherIcon {...rest} />;
      case "rifle":
      default:
        return <RifleCartridgeIcon {...rest} />;
    }
  }

  return <SubtypeOtherIcon {...rest} />;
}
