// Default/empty frame until the first bevy.on("frame") arrives.
const EMPTY = {
	state: "MainMenu",
	player_hp: 0,
	player_max_hp: 1,
	xp: 0,
	level: 0,
	wave: 0,
	kills: 0,
	pickups: 0,
	active_weapon: null,
	ammo: 0,
	ammo_size: 0,
	reloading: false,
	cooldown_frac: 0,
	dps: 0,
	elapsed: 0,
	inventory: [],
	enemies: [],
	damage_numbers: [],
	blips: [],
	log: []
};
function App() {
	const [frame, setFrame] = createSignal(EMPTY);
	// Rust pushes the whole UiSnapshot+state here every frame (design §2).
	bevy.on("frame", (f) => setFrame(f));
	return (() => {
		const _el0 = $ss.el("div");
		$ss.attr(_el0, "id", "hud");
		$ss.child(_el0, (() => {
			const _el1 = $ss.el("h1");
			$ss.attr(_el1, "id", "title");
			$ss.child(_el1, $ss.txt("HORDE"));
			return _el1;
		})());
		$ss.child(_el0, (() => {
			const _el2 = $ss.el("span");
			$ss.attr(_el2, "id", "state");
			$ss.insert(_el2, () => frame().state);
			return _el2;
		})());
		$ss.child(_el0, (() => {
			const _el3 = $ss.el("div");
			$ss.attr(_el3, "id", "spike-track");
			$ss.child(_el3, (() => {
				const _el4 = $ss.el("div");
				$ss.attr(_el4, "id", "spike-fill");
				$ss.bind(_el4, "style", () => `width: ${Math.round(100 * frame().player_hp / frame().player_max_hp)}%`);
				return _el4;
			})());
			return _el3;
		})());
		return _el0;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#App", App);
render(() => $ss.cmp(App, {}), document.getElementById("root"));
