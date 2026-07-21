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
					$ss.bind(_el11, "style", () => `color: rgba(237, 245, 255, ${line.alpha})`);
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
		$ss.insert(_el16, () => $ss.cmp(Index, {
			get each() {
				return props.f().blips;
			},
			get children() {
				return (b) => (() => {
					const _el15 = $ss.el("div");
					$ss.bind(_el15, "class", () => "blip " + b().kind);
					$ss.bind(_el15, "style", () => `left: ${Math.round(b().mx * 100)}%; top: ${Math.round(b().my * 100)}%`);
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
		$ss.insert(_el19, () => $ss.cmp(Index, {
			get each() {
				return props.f().enemies;
			},
			get children() {
				return (e) => (() => {
					const _el17 = $ss.el("div");
					$ss.attr(_el17, "class", "nameplate");
					$ss.bind(_el17, "data-id", () => e().id);
					$ss.bind(_el17, "style", () => `left: ${Math.round(e().sx - 22)}px; top: ${Math.round(e().sy - 30)}px`);
					$ss.child(_el17, (() => {
						const _el18 = $ss.el("div");
						$ss.attr(_el18, "class", "np-fill");
						$ss.bind(_el18, "style", () => `width: ${Math.round(e().frac * 100)}%; background-color: ${hpColor(e().frac)}`);
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
		$ss.insert(_el21, () => $ss.cmp(Index, {
			get each() {
				return props.f().damage_numbers;
			},
			get children() {
				return (d) => (() => {
					const _el20 = $ss.el("span");
					$ss.bind(_el20, "class", () => d().crit ? "dmg crit" : "dmg");
					$ss.bind(_el20, "data-id", () => d().id);
					$ss.bind(_el20, "style", () => `left: ${Math.round(d().sx)}px; top: ${Math.round(d().sy)}px; color: rgba(${d().crit ? "255, 199, 71" : "237, 245, 255"}, ${d().alpha})`);
					$ss.insert(_el20, () => d().text);
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
function Inventory(props) {
	return (() => {
		const _el28 = $ss.el("div");
		$ss.attr(_el28, "class", "modal dim");
		$ss.attr(_el28, "id", "inventory");
		$ss.child(_el28, (() => {
			const _el29 = $ss.el("h2");
			$ss.attr(_el29, "class", "screen-title");
			$ss.child(_el29, $ss.txt("Inventory (I to close)"));
			return _el29;
		})());
		$ss.child(_el28, (() => {
			const _el30 = $ss.el("div");
			$ss.attr(_el30, "class", "inv-grid");
			$ss.insert(_el30, () => $ss.cmp(For, {
				get each() {
					return props.f().inventory;
				},
				get children() {
					return (w) => (() => {
						const _el23 = $ss.el("div");
						$ss.bind(_el23, "class", () => w.active ? "inv-card active" : "inv-card");
						$ss.child(_el23, (() => {
							const _el24 = $ss.el("span");
							$ss.attr(_el24, "class", "inv-name");
							$ss.insert(_el24, () => w.name);
							return _el24;
						})());
						$ss.child(_el23, (() => {
							const _el25 = $ss.el("span");
							$ss.attr(_el25, "class", "inv-stat");
							$ss.insert(_el25, () => `DMG ${Math.round(w.dmg)}   RoF ${w.rof.toFixed(2)}s`);
							return _el25;
						})());
						$ss.child(_el23, (() => {
							const _el26 = $ss.el("span");
							$ss.attr(_el26, "class", "inv-stat");
							$ss.insert(_el26, () => `Spread ${w.spread.toFixed(2)}   x${w.projectiles}`);
							return _el26;
						})());
						$ss.child(_el23, (() => {
							const _el27 = $ss.el("span");
							$ss.attr(_el27, "class", "inv-stat");
							$ss.insert(_el27, () => `Mag ${w.mag}   Reload ${w.reload.toFixed(1)}s`);
							return _el27;
						})());
						return _el23;
					})();
				}
			}));
			return _el30;
		})());
		$ss.child(_el28, (() => {
			const _el31 = $ss.el("button");
			$ss.attr(_el31, "class", "menu-btn");
			$ss.attr(_el31, "id", "inv-close");
			$ss.on(_el31, "click", () => props.onClose());
			$ss.child(_el31, $ss.txt("Close"));
			return _el31;
		})());
		return _el28;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#Inventory", Inventory);
function MainMenu() {
	const [settingsOpen, setSettingsOpen] = createSignal(false);
	return (() => {
		const _el32 = $ss.el("div");
		$ss.attr(_el32, "class", "screen");
		$ss.attr(_el32, "id", "main-menu");
		$ss.child(_el32, (() => {
			const _el33 = $ss.el("h1");
			$ss.attr(_el33, "class", "title");
			$ss.attr(_el33, "id", "title");
			$ss.child(_el33, $ss.txt("HORDE"));
			return _el33;
		})());
		$ss.child(_el32, (() => {
			const _el34 = $ss.el("span");
			$ss.attr(_el34, "class", "subtitle");
			$ss.child(_el34, $ss.txt("survive the swarm"));
			return _el34;
		})());
		$ss.child(_el32, (() => {
			const _el35 = $ss.el("button");
			$ss.attr(_el35, "class", "menu-btn");
			$ss.attr(_el35, "id", "start");
			$ss.on(_el35, "click", () => intent("StartGame"));
			$ss.child(_el35, $ss.txt("Start  (Enter)"));
			return _el35;
		})());
		$ss.child(_el32, (() => {
			const _el36 = $ss.el("button");
			$ss.attr(_el36, "class", "menu-btn");
			$ss.attr(_el36, "id", "open-settings");
			$ss.on(_el36, "click", () => setSettingsOpen(true));
			$ss.child(_el36, $ss.txt("Settings"));
			return _el36;
		})());
		$ss.child(_el32, (() => {
			const _el37 = $ss.el("button");
			$ss.attr(_el37, "class", "menu-btn");
			$ss.attr(_el37, "id", "quit");
			$ss.on(_el37, "click", () => intent("Quit"));
			$ss.child(_el37, $ss.txt("Quit"));
			return _el37;
		})());
		$ss.insert(_el32, () => $ss.cmp(Show, {
			get when() {
				return settingsOpen();
			},
			get children() {
				return $ss.cmp(Settings, { onClose: () => setSettingsOpen(false) });
			}
		}));
		return _el32;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#MainMenu", MainMenu);
function Settings(props) {
	const [cap, setCap] = createSignal(0);
	return (() => {
		const _el38 = $ss.el("div");
		$ss.attr(_el38, "class", "modal dim");
		$ss.attr(_el38, "id", "settings");
		$ss.child(_el38, (() => {
			const _el39 = $ss.el("h2");
			$ss.attr(_el39, "class", "screen-title");
			$ss.child(_el39, $ss.txt("Settings"));
			return _el39;
		})());
		$ss.child(_el38, (() => {
			const _el40 = $ss.el("div");
			$ss.attr(_el40, "class", "settings-row");
			$ss.child(_el40, (() => {
				const _el41 = $ss.el("button");
				$ss.attr(_el41, "id", "cap-dec");
				$ss.on(_el41, "click", () => bevy.send("AdjustEnemyCap", { delta: -20 }));
				$ss.child(_el41, $ss.txt("−"));
				return _el41;
			})());
			$ss.child(_el40, (() => {
				const _el42 = $ss.el("span");
				$ss.attr(_el42, "id", "cap-label");
				$ss.child(_el42, $ss.txt("Enemy cap ±20"));
				return _el42;
			})());
			$ss.child(_el40, (() => {
				const _el43 = $ss.el("button");
				$ss.attr(_el43, "id", "cap-inc");
				$ss.on(_el43, "click", () => bevy.send("AdjustEnemyCap", { delta: 20 }));
				$ss.child(_el43, $ss.txt("+"));
				return _el43;
			})());
			return _el40;
		})());
		$ss.child(_el38, (() => {
			const _el44 = $ss.el("span");
			$ss.attr(_el44, "class", "inv-stat");
			$ss.child(_el44, $ss.txt("UI backend: supersolid (TSX)"));
			return _el44;
		})());
		$ss.child(_el38, (() => {
			const _el45 = $ss.el("button");
			$ss.attr(_el45, "class", "menu-btn");
			$ss.attr(_el45, "id", "settings-close");
			$ss.on(_el45, "click", () => props.onClose());
			$ss.child(_el45, $ss.txt("Close"));
			return _el45;
		})());
		return _el38;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#Settings", Settings);
function Pause() {
	return (() => {
		const _el46 = $ss.el("div");
		$ss.attr(_el46, "class", "screen dim");
		$ss.attr(_el46, "id", "paused");
		$ss.child(_el46, (() => {
			const _el47 = $ss.el("h2");
			$ss.attr(_el47, "class", "screen-title");
			$ss.child(_el47, $ss.txt("Paused"));
			return _el47;
		})());
		$ss.child(_el46, (() => {
			const _el48 = $ss.el("button");
			$ss.attr(_el48, "class", "menu-btn");
			$ss.attr(_el48, "id", "resume");
			$ss.on(_el48, "click", () => intent("Resume"));
			$ss.child(_el48, $ss.txt("Resume  (Esc)"));
			return _el48;
		})());
		$ss.child(_el46, (() => {
			const _el49 = $ss.el("button");
			$ss.attr(_el49, "class", "menu-btn");
			$ss.attr(_el49, "id", "restart");
			$ss.on(_el49, "click", () => intent("Restart"));
			$ss.child(_el49, $ss.txt("Restart"));
			return _el49;
		})());
		$ss.child(_el46, (() => {
			const _el50 = $ss.el("button");
			$ss.attr(_el50, "class", "menu-btn");
			$ss.attr(_el50, "id", "pause-quit");
			$ss.on(_el50, "click", () => intent("Quit"));
			$ss.child(_el50, $ss.txt("Quit"));
			return _el50;
		})());
		return _el46;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#Pause", Pause);
function GameOver(props) {
	const f = props.f;
	return (() => {
		const _el51 = $ss.el("div");
		$ss.attr(_el51, "class", "screen dim");
		$ss.attr(_el51, "id", "game-over");
		$ss.child(_el51, (() => {
			const _el52 = $ss.el("h2");
			$ss.attr(_el52, "class", "screen-title danger");
			$ss.child(_el52, $ss.txt("You Died"));
			return _el52;
		})());
		$ss.child(_el51, (() => {
			const _el53 = $ss.el("div");
			$ss.attr(_el53, "class", "panel stats");
			$ss.child(_el53, (() => {
				const _el54 = $ss.el("span");
				$ss.insert(_el54, () => `Kills: ${f().kills}`);
				return _el54;
			})());
			$ss.child(_el53, (() => {
				const _el55 = $ss.el("span");
				$ss.insert(_el55, () => `Wave reached: ${f().wave}`);
				return _el55;
			})());
			$ss.child(_el53, (() => {
				const _el56 = $ss.el("span");
				$ss.insert(_el56, () => `Pickups: ${f().pickups}`);
				return _el56;
			})());
			$ss.child(_el53, (() => {
				const _el57 = $ss.el("span");
				$ss.insert(_el57, () => `Time survived: ${mmss(f().elapsed)}`);
				return _el57;
			})());
			return _el53;
		})());
		$ss.child(_el51, (() => {
			const _el58 = $ss.el("button");
			$ss.attr(_el58, "class", "menu-btn");
			$ss.attr(_el58, "id", "go-restart");
			$ss.on(_el58, "click", () => intent("Restart"));
			$ss.child(_el58, $ss.txt("Restart  (Enter)"));
			return _el58;
		})());
		$ss.child(_el51, (() => {
			const _el59 = $ss.el("button");
			$ss.attr(_el59, "class", "menu-btn");
			$ss.attr(_el59, "id", "go-quit");
			$ss.on(_el59, "click", () => intent("Quit"));
			$ss.child(_el59, $ss.txt("Quit"));
			return _el59;
		})());
		return _el51;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#GameOver", GameOver);
function App() {
	const [frame, setFrame] = createSignal(EMPTY);
	bevy.on("frame", (f) => setFrame(f));
	const state = createMemo(() => frame().state);
	const [invOpen, setInvOpen] = createSignal(false);
	bevy.on("toggleInventory", () => setInvOpen((o) => !o));
	return (() => {
		const _el61 = $ss.el("div");
		$ss.attr(_el61, "id", "hud");
		$ss.insert(_el61, () => $ss.cmp(Switch, { get children() {
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
							const _el60 = $ss.el("div");
							$ss.attr(_el60, "id", "playing-root");
							$ss.child(_el60, $ss.cmp(Hud, { get f() {
								return frame;
							} }));
							$ss.insert(_el60, () => $ss.cmp(Show, {
								get when() {
									return invOpen();
								},
								get children() {
									return $ss.cmp(Inventory, {
										get f() {
											return frame;
										},
										onClose: () => setInvOpen(false)
									});
								}
							}));
							return _el60;
						})();
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
		return _el61;
	})();
}
$ss.hot("assets/ui/horde\\app.tsx#App", App);
render(() => $ss.cmp(App, {}), document.getElementById("root"));
