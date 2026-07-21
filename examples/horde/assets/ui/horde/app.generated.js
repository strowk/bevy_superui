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
function intent(kind, index) {
	bevy.send("HordeIntent", {
		kind,
		index: index || 0
	});
}
function MainMenu() {
	const [settingsOpen, setSettingsOpen] = createSignal(false);
	return (() => {
		const _el0 = $ss.el("div");
		$ss.attr(_el0, "class", "screen");
		$ss.attr(_el0, "id", "main-menu");
		$ss.child(_el0, (() => {
			const _el1 = $ss.el("h1");
			$ss.attr(_el1, "class", "title");
			$ss.attr(_el1, "id", "title");
			$ss.child(_el1, $ss.txt("HORDE"));
			return _el1;
		})());
		$ss.child(_el0, (() => {
			const _el2 = $ss.el("span");
			$ss.attr(_el2, "class", "subtitle");
			$ss.child(_el2, $ss.txt("survive the swarm"));
			return _el2;
		})());
		$ss.child(_el0, (() => {
			const _el3 = $ss.el("button");
			$ss.attr(_el3, "class", "menu-btn");
			$ss.attr(_el3, "id", "start");
			$ss.on(_el3, "click", () => intent("StartGame"));
			$ss.child(_el3, $ss.txt("Start  (Enter)"));
			return _el3;
		})());
		$ss.child(_el0, (() => {
			const _el4 = $ss.el("button");
			$ss.attr(_el4, "class", "menu-btn");
			$ss.attr(_el4, "id", "open-settings");
			$ss.on(_el4, "click", () => setSettingsOpen(true));
			$ss.child(_el4, $ss.txt("Settings"));
			return _el4;
		})());
		$ss.child(_el0, (() => {
			const _el5 = $ss.el("button");
			$ss.attr(_el5, "class", "menu-btn");
			$ss.attr(_el5, "id", "quit");
			$ss.on(_el5, "click", () => intent("Quit"));
			$ss.child(_el5, $ss.txt("Quit"));
			return _el5;
		})());
		$ss.insert(_el0, () => $ss.cmp(Show, {
			get when() {
				return settingsOpen();
			},
			get children() {
				return $ss.cmp(Settings, { onClose: () => setSettingsOpen(false) });
			}
		}));
		return _el0;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#MainMenu", MainMenu);
// Placeholder; real body added in Task B6.
function Settings(props) {
	return (() => {
		const _el6 = $ss.el("div");
		$ss.attr(_el6, "class", "modal");
		$ss.attr(_el6, "id", "settings");
		$ss.child(_el6, (() => {
			const _el7 = $ss.el("button");
			$ss.attr(_el7, "id", "settings-close");
			$ss.on(_el7, "click", () => props.onClose());
			$ss.child(_el7, $ss.txt("Close"));
			return _el7;
		})());
		return _el6;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#Settings", Settings);
function App() {
	const [frame, setFrame] = createSignal(EMPTY);
	bevy.on("frame", (f) => setFrame(f));
	const state = createMemo(() => frame().state);
	return (() => {
		const _el11 = $ss.el("div");
		$ss.attr(_el11, "id", "hud");
		$ss.insert(_el11, () => $ss.cmp(Switch, { get children() {
			return $ss.frag([
				$ss.cmp(Match, {
					get when() {
						return state() === "MainMenu";
					},
					get children() {
						return $ss.cmp(MainMenu, {});
					}
				}),
				$ss.cmp(Match, {
					get when() {
						return state() === "Playing";
					},
					get children() {
						return (() => {
							const _el8 = $ss.el("div");
							$ss.attr(_el8, "id", "playing");
							return _el8;
						})();
					}
				}),
				$ss.cmp(Match, {
					get when() {
						return state() === "Paused";
					},
					get children() {
						return (() => {
							const _el9 = $ss.el("div");
							$ss.attr(_el9, "id", "paused");
							return _el9;
						})();
					}
				}),
				$ss.cmp(Match, {
					get when() {
						return state() === "GameOver";
					},
					get children() {
						return (() => {
							const _el10 = $ss.el("div");
							$ss.attr(_el10, "id", "game-over");
							return _el10;
						})();
					}
				})
			]);
		} }));
		return _el11;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#App", App);
render(() => $ss.cmp(App, {}), document.getElementById("root"));
