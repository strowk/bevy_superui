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
function hpColor(f) {
	f = Math.max(0, Math.min(1, f));
	const r = Math.round((.95 * (1 - f * f) + .1) * 255);
	const g = Math.round((.3 + .62 * f) * 255);
	const b = Math.round(.3 * 255);
	return `rgb(${r}, ${g}, ${b})`;
}
function mmss(sec) {
	const m = Math.floor(sec / 60), s = Math.floor(sec % 60);
	return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}
function PlayerStatus(props) {
	const f = props.f;
	const hpFrac = () => f().player_hp / f().player_max_hp;
	const xpFrac = () => f().xp % 100 / 100;
	return (() => {
		const _el0 = $ss.el("div");
		$ss.attr(_el0, "class", "panel");
		$ss.attr(_el0, "id", "player-status");
		$ss.child(_el0, (() => {
			const _el1 = $ss.el("span");
			$ss.attr(_el1, "class", "label");
			$ss.child(_el1, $ss.txt("HP"));
			return _el1;
		})());
		$ss.child(_el0, (() => {
			const _el2 = $ss.el("div");
			$ss.attr(_el2, "class", "bar-track");
			$ss.child(_el2, (() => {
				const _el3 = $ss.el("div");
				$ss.attr(_el3, "class", "bar-fill");
				$ss.attr(_el3, "id", "hp-fill");
				$ss.bind(_el3, "style", () => `width: ${Math.round(100 * hpFrac())}%; background-color: ${hpColor(hpFrac())}`);
				return _el3;
			})());
			return _el2;
		})());
		$ss.child(_el0, (() => {
			const _el4 = $ss.el("span");
			$ss.attr(_el4, "class", "label");
			$ss.child(_el4, $ss.txt("XP"));
			return _el4;
		})());
		$ss.child(_el0, (() => {
			const _el5 = $ss.el("div");
			$ss.attr(_el5, "class", "bar-track");
			$ss.child(_el5, (() => {
				const _el6 = $ss.el("div");
				$ss.attr(_el6, "class", "bar-fill xp");
				$ss.attr(_el6, "id", "xp-fill");
				$ss.bind(_el6, "style", () => `width: ${Math.round(100 * xpFrac())}%`);
				return _el6;
			})());
			return _el5;
		})());
		$ss.child(_el0, (() => {
			const _el7 = $ss.el("span");
			$ss.attr(_el7, "class", "badge");
			$ss.attr(_el7, "id", "weapon-badge");
			$ss.insert(_el7, () => f().active_weapon || "—");
			return _el7;
		})());
		$ss.child(_el0, (() => {
			const _el8 = $ss.el("span");
			$ss.attr(_el8, "class", "ammo");
			$ss.attr(_el8, "id", "ammo");
			$ss.insert(_el8, () => f().reloading ? "reloading…" : `${f().ammo} / ${f().ammo_size}`);
			return _el8;
		})());
		return _el0;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#PlayerStatus", PlayerStatus);
function Meters(props) {
	const f = props.f;
	return (() => {
		const _el9 = $ss.el("div");
		$ss.attr(_el9, "class", "panel");
		$ss.attr(_el9, "id", "meters");
		$ss.child(_el9, (() => {
			const _el10 = $ss.el("span");
			$ss.insert(_el10, () => `Wave ${f().wave}   Kills ${f().kills}   DPS ${Math.round(f().dps)}   ${mmss(f().elapsed)}`);
			return _el10;
		})());
		return _el9;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#Meters", Meters);
function CombatLog(props) {
	return (() => {
		const _el12 = $ss.el("div");
		$ss.attr(_el12, "class", "panel");
		$ss.attr(_el12, "id", "combat-log");
		$ss.insert(_el12, () => $ss.cmp(For, {
			get each() {
				return props.f().log;
			},
			get children() {
				return (line) => (() => {
					const _el11 = $ss.el("span");
					$ss.attr(_el11, "class", "log-line");
					$ss.bind(_el11, "style", () => `opacity: ${line.alpha}`);
					$ss.insert(_el11, () => line.text);
					return _el11;
				})();
			}
		}));
		return _el12;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#CombatLog", CombatLog);
function Hud(props) {
	return (() => {
		const _el13 = $ss.el("div");
		$ss.attr(_el13, "id", "playing");
		$ss.child(_el13, $ss.cmp(PlayerStatus, { get f() {
			return props.f;
		} }));
		$ss.child(_el13, $ss.cmp(Meters, { get f() {
			return props.f;
		} }));
		$ss.child(_el13, $ss.cmp(CombatLog, { get f() {
			return props.f;
		} }));
		return _el13;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#Hud", Hud);
function MainMenu() {
	const [settingsOpen, setSettingsOpen] = createSignal(false);
	return (() => {
		const _el14 = $ss.el("div");
		$ss.attr(_el14, "class", "screen");
		$ss.attr(_el14, "id", "main-menu");
		$ss.child(_el14, (() => {
			const _el15 = $ss.el("h1");
			$ss.attr(_el15, "class", "title");
			$ss.attr(_el15, "id", "title");
			$ss.child(_el15, $ss.txt("HORDE"));
			return _el15;
		})());
		$ss.child(_el14, (() => {
			const _el16 = $ss.el("span");
			$ss.attr(_el16, "class", "subtitle");
			$ss.child(_el16, $ss.txt("survive the swarm"));
			return _el16;
		})());
		$ss.child(_el14, (() => {
			const _el17 = $ss.el("button");
			$ss.attr(_el17, "class", "menu-btn");
			$ss.attr(_el17, "id", "start");
			$ss.on(_el17, "click", () => intent("StartGame"));
			$ss.child(_el17, $ss.txt("Start  (Enter)"));
			return _el17;
		})());
		$ss.child(_el14, (() => {
			const _el18 = $ss.el("button");
			$ss.attr(_el18, "class", "menu-btn");
			$ss.attr(_el18, "id", "open-settings");
			$ss.on(_el18, "click", () => setSettingsOpen(true));
			$ss.child(_el18, $ss.txt("Settings"));
			return _el18;
		})());
		$ss.child(_el14, (() => {
			const _el19 = $ss.el("button");
			$ss.attr(_el19, "class", "menu-btn");
			$ss.attr(_el19, "id", "quit");
			$ss.on(_el19, "click", () => intent("Quit"));
			$ss.child(_el19, $ss.txt("Quit"));
			return _el19;
		})());
		$ss.insert(_el14, () => $ss.cmp(Show, {
			get when() {
				return settingsOpen();
			},
			get children() {
				return $ss.cmp(Settings, { onClose: () => setSettingsOpen(false) });
			}
		}));
		return _el14;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#MainMenu", MainMenu);
// Placeholder; real body added in Task B6.
function Settings(props) {
	return (() => {
		const _el20 = $ss.el("div");
		$ss.attr(_el20, "class", "modal");
		$ss.attr(_el20, "id", "settings");
		$ss.child(_el20, (() => {
			const _el21 = $ss.el("button");
			$ss.attr(_el21, "id", "settings-close");
			$ss.on(_el21, "click", () => props.onClose());
			$ss.child(_el21, $ss.txt("Close"));
			return _el21;
		})());
		return _el20;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#Settings", Settings);
function App() {
	const [frame, setFrame] = createSignal(EMPTY);
	bevy.on("frame", (f) => setFrame(f));
	const state = createMemo(() => frame().state);
	return (() => {
		const _el24 = $ss.el("div");
		$ss.attr(_el24, "id", "hud");
		$ss.insert(_el24, () => $ss.cmp(Switch, { get children() {
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
						return $ss.cmp(Hud, { get f() {
							return frame;
						} });
					}
				}),
				$ss.cmp(Match, {
					get when() {
						return state() === "Paused";
					},
					get children() {
						return (() => {
							const _el22 = $ss.el("div");
							$ss.attr(_el22, "id", "paused");
							return _el22;
						})();
					}
				}),
				$ss.cmp(Match, {
					get when() {
						return state() === "GameOver";
					},
					get children() {
						return (() => {
							const _el23 = $ss.el("div");
							$ss.attr(_el23, "id", "game-over");
							return _el23;
						})();
					}
				})
			]);
		} }));
		return _el24;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#App", App);
render(() => $ss.cmp(App, {}), document.getElementById("root"));
