import { createSignal } from "supersolid";
import { ConfigRow, Segmented, Toggle, Slider } from "./widgets";
import { RES_OPTS, DIFF_OPTS } from "./data";

export function Settings(props) {
  // Local, interactive config state — flipping these updates only the affected
  // widget (fine-grained reactivity), no full re-render.
  const [resolution, setResolution] = createSignal("1920");
  const [difficulty, setDifficulty] = createSignal("standard");
  const [vsync, setVsync] = createSignal(true);
  const [fps, setFps] = createSignal(true);
  const [camera, setCamera] = createSignal(false);

  return (
    <div class="screen overlay">
      <div class="settings-card">
        <div class="settings-head">
          <span class="settings-title">SYSTEMS CONFIG</span>
          <div class="close-btn" onClick={() => props.nav("main")}>{"< RETURN [ESC]"}</div>
        </div>
        <div class="settings-body">
          <div class="settings-col">
            <div class="section">DISPLAY</div>
            <ConfigRow label="Resolution">
              <Segmented options={RES_OPTS} value={resolution()} onSelect={setResolution} />
            </ConfigRow>
            <ConfigRow label="V-Sync">
              <Toggle on={vsync()} onClick={() => setVsync(!vsync())} />
            </ConfigRow>
            <ConfigRow label="FPS counter">
              <Toggle on={fps()} onClick={() => setFps(!fps())} />
            </ConfigRow>
            <ConfigRow label="Screen shake">
              <Slider value={60} />
            </ConfigRow>
          </div>
          <div class="settings-col">
            <div class="section">AUDIO</div>
            <ConfigRow label="Master">
              <Slider value={80} />
            </ConfigRow>
            <ConfigRow label="Effects">
              <Slider value={70} />
            </ConfigRow>
            <ConfigRow label="Music">
              <Slider value={45} />
            </ConfigRow>
            <div class="section gap">GAMEPLAY</div>
            <ConfigRow label="Difficulty">
              <Segmented options={DIFF_OPTS} value={difficulty()} onSelect={setDifficulty} />
            </ConfigRow>
            <ConfigRow label="Camera follow">
              <Toggle on={camera()} onClick={() => setCamera(!camera())} />
            </ConfigRow>
          </div>
        </div>
      </div>
    </div>
  );
}
