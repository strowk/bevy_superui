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
function WeaponBar(props) {
	return (() => {
		const _el14 = $ss.el("div");
		$ss.attr(_el14, "id", "weapon-bar");
		$ss.insert(_el14, () => $ss.cmp(For, {
			get each() {
				return props.f().inventory;
			},
			get children() {
				return (slot) => (() => {
					const _el13 = $ss.el("button");
					$ss.bind(_el13, "class", () => slot.active ? "slot active" : "slot");
					$ss.bind(_el13, "data-index", () => slot.index);
					$ss.on(_el13, "click", () => intent("SwitchWeapon", slot.index));
					$ss.insert(_el13, () => `${slot.index + 1}. ${slot.name}`);
					return _el13;
				})();
			}
		}));
		return _el14;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#WeaponBar", WeaponBar);
function Minimap(props) {
	return (() => {
		const _el16 = $ss.el("div");
		$ss.attr(_el16, "class", "panel");
		$ss.attr(_el16, "id", "minimap");
		$ss.insert(_el16, () => $ss.cmp(For, {
			get each() {
				return props.f().blips;
			},
			get children() {
				return (b) => (() => {
					const _el15 = $ss.el("div");
					$ss.bind(_el15, "class", () => "blip " + b.kind);
					$ss.bind(_el15, "style", () => `left: ${Math.round(b.mx * 100)}%; top: ${Math.round(b.my * 100)}%`);
					return _el15;
				})();
			}
		}));
		return _el16;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#Minimap", Minimap);
function Nameplates(props) {
	return (() => {
		const _el19 = $ss.el("div");
		$ss.attr(_el19, "class", "overlay");
		$ss.attr(_el19, "id", "nameplates");
		$ss.insert(_el19, () => $ss.cmp(For, {
			get each() {
				return props.f().enemies;
			},
			get children() {
				return (e) => (() => {
					const _el17 = $ss.el("div");
					$ss.attr(_el17, "class", "nameplate");
					$ss.bind(_el17, "data-id", () => e.id);
					$ss.bind(_el17, "style", () => `left: ${Math.round(e.sx - 22)}px; top: ${Math.round(e.sy - 30)}px`);
					$ss.child(_el17, (() => {
						const _el18 = $ss.el("div");
						$ss.attr(_el18, "class", "np-fill");
						$ss.bind(_el18, "style", () => `width: ${Math.round(e.frac * 100)}%; background-color: ${hpColor(e.frac)}`);
						return _el18;
					})());
					return _el17;
				})();
			}
		}));
		return _el19;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#Nameplates", Nameplates);
function DamageNumbers(props) {
	return (() => {
		const _el21 = $ss.el("div");
		$ss.attr(_el21, "class", "overlay");
		$ss.attr(_el21, "id", "damage-numbers");
		$ss.insert(_el21, () => $ss.cmp(For, {
			get each() {
				return props.f().damage_numbers;
			},
			get children() {
				return (d) => (() => {
					const _el20 = $ss.el("span");
					$ss.bind(_el20, "class", () => d.crit ? "dmg crit" : "dmg");
					$ss.bind(_el20, "data-id", () => d.id);
					$ss.bind(_el20, "style", () => `left: ${Math.round(d.sx)}px; top: ${Math.round(d.sy)}px; opacity: ${d.alpha}`);
					$ss.insert(_el20, () => d.text);
					return _el20;
				})();
			}
		}));
		return _el21;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#DamageNumbers", DamageNumbers);
function Hud(props) {
	return (() => {
		const _el22 = $ss.el("div");
		$ss.attr(_el22, "id", "playing");
		$ss.child(_el22, $ss.cmp(PlayerStatus, { get f() {
			return props.f;
		} }));
		$ss.child(_el22, $ss.cmp(Meters, { get f() {
			return props.f;
		} }));
		$ss.child(_el22, $ss.cmp(CombatLog, { get f() {
			return props.f;
		} }));
		$ss.child(_el22, $ss.cmp(WeaponBar, { get f() {
			return props.f;
		} }));
		$ss.child(_el22, $ss.cmp(Minimap, { get f() {
			return props.f;
		} }));
		$ss.child(_el22, $ss.cmp(Nameplates, { get f() {
			return props.f;
		} }));
		$ss.child(_el22, $ss.cmp(DamageNumbers, { get f() {
			return props.f;
		} }));
		return _el22;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#Hud", Hud);
function MainMenu() {
	const [settingsOpen, setSettingsOpen] = createSignal(false);
	return (() => {
		const _el23 = $ss.el("div");
		$ss.attr(_el23, "class", "screen");
		$ss.attr(_el23, "id", "main-menu");
		$ss.child(_el23, (() => {
			const _el24 = $ss.el("h1");
			$ss.attr(_el24, "class", "title");
			$ss.attr(_el24, "id", "title");
			$ss.child(_el24, $ss.txt("HORDE"));
			return _el24;
		})());
		$ss.child(_el23, (() => {
			const _el25 = $ss.el("span");
			$ss.attr(_el25, "class", "subtitle");
			$ss.child(_el25, $ss.txt("survive the swarm"));
			return _el25;
		})());
		$ss.child(_el23, (() => {
			const _el26 = $ss.el("button");
			$ss.attr(_el26, "class", "menu-btn");
			$ss.attr(_el26, "id", "start");
			$ss.on(_el26, "click", () => intent("StartGame"));
			$ss.child(_el26, $ss.txt("Start  (Enter)"));
			return _el26;
		})());
		$ss.child(_el23, (() => {
			const _el27 = $ss.el("button");
			$ss.attr(_el27, "class", "menu-btn");
			$ss.attr(_el27, "id", "open-settings");
			$ss.on(_el27, "click", () => setSettingsOpen(true));
			$ss.child(_el27, $ss.txt("Settings"));
			return _el27;
		})());
		$ss.child(_el23, (() => {
			const _el28 = $ss.el("button");
			$ss.attr(_el28, "class", "menu-btn");
			$ss.attr(_el28, "id", "quit");
			$ss.on(_el28, "click", () => intent("Quit"));
			$ss.child(_el28, $ss.txt("Quit"));
			return _el28;
		})());
		$ss.insert(_el23, () => $ss.cmp(Show, {
			get when() {
				return settingsOpen();
			},
			get children() {
				return $ss.cmp(Settings, { onClose: () => setSettingsOpen(false) });
			}
		}));
		return _el23;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#MainMenu", MainMenu);
// Placeholder; real body added in Task B6.
function Settings(props) {
	return (() => {
		const _el29 = $ss.el("div");
		$ss.attr(_el29, "class", "modal");
		$ss.attr(_el29, "id", "settings");
		$ss.child(_el29, (() => {
			const _el30 = $ss.el("button");
			$ss.attr(_el30, "id", "settings-close");
			$ss.on(_el30, "click", () => props.onClose());
			$ss.child(_el30, $ss.txt("Close"));
			return _el30;
		})());
		return _el29;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#Settings", Settings);
function Pause() {
	return (() => {
		const _el31 = $ss.el("div");
		$ss.attr(_el31, "class", "screen dim");
		$ss.attr(_el31, "id", "paused");
		$ss.child(_el31, (() => {
			const _el32 = $ss.el("h2");
			$ss.attr(_el32, "class", "screen-title");
			$ss.child(_el32, $ss.txt("Paused"));
			return _el32;
		})());
		$ss.child(_el31, (() => {
			const _el33 = $ss.el("button");
			$ss.attr(_el33, "class", "menu-btn");
			$ss.attr(_el33, "id", "resume");
			$ss.on(_el33, "click", () => intent("Resume"));
			$ss.child(_el33, $ss.txt("Resume  (Esc)"));
			return _el33;
		})());
		$ss.child(_el31, (() => {
			const _el34 = $ss.el("button");
			$ss.attr(_el34, "class", "menu-btn");
			$ss.attr(_el34, "id", "restart");
			$ss.on(_el34, "click", () => intent("Restart"));
			$ss.child(_el34, $ss.txt("Restart"));
			return _el34;
		})());
		$ss.child(_el31, (() => {
			const _el35 = $ss.el("button");
			$ss.attr(_el35, "class", "menu-btn");
			$ss.attr(_el35, "id", "pause-quit");
			$ss.on(_el35, "click", () => intent("Quit"));
			$ss.child(_el35, $ss.txt("Quit"));
			return _el35;
		})());
		return _el31;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#Pause", Pause);
function GameOver(props) {
	const f = props.f;
	return (() => {
		const _el36 = $ss.el("div");
		$ss.attr(_el36, "class", "screen dim");
		$ss.attr(_el36, "id", "game-over");
		$ss.child(_el36, (() => {
			const _el37 = $ss.el("h2");
			$ss.attr(_el37, "class", "screen-title danger");
			$ss.child(_el37, $ss.txt("You Died"));
			return _el37;
		})());
		$ss.child(_el36, (() => {
			const _el38 = $ss.el("div");
			$ss.attr(_el38, "class", "panel stats");
			$ss.child(_el38, (() => {
				const _el39 = $ss.el("span");
				$ss.insert(_el39, () => `Kills: ${f().kills}`);
				return _el39;
			})());
			$ss.child(_el38, (() => {
				const _el40 = $ss.el("span");
				$ss.insert(_el40, () => `Wave reached: ${f().wave}`);
				return _el40;
			})());
			$ss.child(_el38, (() => {
				const _el41 = $ss.el("span");
				$ss.insert(_el41, () => `Pickups: ${f().pickups}`);
				return _el41;
			})());
			$ss.child(_el38, (() => {
				const _el42 = $ss.el("span");
				$ss.insert(_el42, () => `Time survived: ${mmss(f().elapsed)}`);
				return _el42;
			})());
			return _el38;
		})());
		$ss.child(_el36, (() => {
			const _el43 = $ss.el("button");
			$ss.attr(_el43, "class", "menu-btn");
			$ss.attr(_el43, "id", "go-restart");
			$ss.on(_el43, "click", () => intent("Restart"));
			$ss.child(_el43, $ss.txt("Restart  (Enter)"));
			return _el43;
		})());
		$ss.child(_el36, (() => {
			const _el44 = $ss.el("button");
			$ss.attr(_el44, "class", "menu-btn");
			$ss.attr(_el44, "id", "go-quit");
			$ss.on(_el44, "click", () => intent("Quit"));
			$ss.child(_el44, $ss.txt("Quit"));
			return _el44;
		})());
		return _el36;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#GameOver", GameOver);
function App() {
	const [frame, setFrame] = createSignal(EMPTY);
	bevy.on("frame", (f) => setFrame(f));
	const state = createMemo(() => frame().state);
	return (() => {
		const _el45 = $ss.el("div");
		$ss.attr(_el45, "id", "hud");
		$ss.insert(_el45, () => $ss.cmp(Switch, { get children() {
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
						return $ss.cmp(Pause, {});
					}
				}),
				$ss.cmp(Match, {
					get when() {
						return state() === "GameOver";
					},
					get children() {
						return $ss.cmp(GameOver, { get f() {
							return frame;
						} });
					}
				})
			]);
		} }));
		return _el45;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#App", App);
render(() => $ss.cmp(App, {}), document.getElementById("root"));
