import { MenuItem } from "./widgets";

export function MainMenu(props) {
  return (
    <div class="screen main">
      <div class="brand">
        <div class="emblem">
          <div class="emblem-inner">
            <div class="emblem-core"></div>
          </div>
        </div>
        <div class="brand-text">
          <div class="title">
            <span>VOID</span>
            <span class="accent">FARER</span>
          </div>
          <div class="tagline">
            <span>DEEP-VOID STATION ASSEMBLY</span>
            <span class="cursor">_</span>
          </div>
        </div>
      </div>
      <div class="divider"></div>
      <div class="menu-list">
        <MenuItem num="01" title="NEW EXPEDITION" hint="LAUNCH >" primary={true}
                  onClick={() => props.nav("over")} />
        <MenuItem num="02" title="LOAD ARCHIVE" hint="3 SAVES >"
                  onClick={() => props.nav("over")} />
        <MenuItem num="03" title="SYSTEMS CONFIG" hint=">"
                  onClick={() => props.nav("settings")} />
        <MenuItem num="04" title="POWER DOWN" hint="QUIT >" danger={true}
                  onClick={() => props.nav("over")} />
      </div>
    </div>
  );
}
