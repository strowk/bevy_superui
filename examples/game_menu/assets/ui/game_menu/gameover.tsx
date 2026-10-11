import { Stat } from "./widgets";

export function GameOver(props) {
  return (
    <div class="screen over">
      <div class="over-tag">
        <div class="blip danger"></div>
        <span class="over-tag-text">// TELEMETRY DOWNLINK TERMINATED</span>
      </div>
      <div class="over-title">SIGNAL LOST</div>
      <div class="over-sub">HULL INTEGRITY 0% // STATION DESTROYED IN SECTOR 7G</div>
      <div class="statrow">
        <Stat label="TIME SURVIVED" value="00:42:17" />
        <Stat label="MODULES DEPLOYED" value="23" />
        <Stat label="THREATS NEUTRALIZED" value="148" accent={true} />
      </div>
      <div class="action-row">
        <div class="action solid" onClick={() => props.nav("main")}>{"> RELAUNCH EXPEDITION"}</div>
        <div class="action" onClick={() => props.nav("main")}>LOAD ARCHIVE</div>
        <div class="action" onClick={() => props.nav("main")}>MAIN MENU</div>
      </div>
    </div>
  );
}
