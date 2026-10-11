export function PauseMenu(props) {
  return (
    <div class="screen overlay">
      <div class="pause-card">
        <div class="card-head">
          <div class="blip"></div>
          <span class="card-head-text">// OPERATIONS SUSPENDED</span>
        </div>
        <div class="pause-title">PAUSED</div>
        <div class="pause-sub">SIMULATION HALTED // LIFE SUPPORT NOMINAL</div>
        <div class="btn-col">
          <div class="btn primary" onClick={() => props.nav("main")}>{"> RESUME OPERATIONS"}</div>
          <div class="btn">SAVE STATE</div>
          <div class="btn" onClick={() => props.nav("settings")}>SYSTEMS CONFIG</div>
          <div class="btn quiet" onClick={() => props.nav("main")}>ABANDON TO MENU</div>
        </div>
      </div>
    </div>
  );
}
