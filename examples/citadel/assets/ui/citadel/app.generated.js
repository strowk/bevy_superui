// Empty frame shape (mirrors FrameDto). Keeps the first render before any
// `frame` event well-formed so every list renders an (empty) container.
const EMPTY = {
	clock: 0,
	tick: 0,
	resources: [],
	buildings: [],
	units: [],
	techs: [],
	events: []
};
function mmss(sec) {
	const m = Math.floor(sec / 60), s = Math.floor(sec % 60);
	return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}
// Round a resource value to a compact string (e.g. 12.3k).
function amt(v) {
	if (v >= 1e4) return `${(v / 1e3).toFixed(1)}k`;
	return `${Math.round(v)}`;
}
function rate(v) {
	return v >= 0 ? `+${v.toFixed(1)}` : v.toFixed(1);
}
// Synthesize a plausible per-card cost line from tier + category. Purely static
// (depends only on stable fields) so it never re-runs per frame.
function costLine(b) {
	const base = b.tier * 40;
	const a = base;
	const c = base + (b.category === "military" ? 25 : b.category === "science" ? 15 : 10);
	return `${a} min   ${c} all`;
}
// ── Top ledger: ~8 resource chips + mission clock ───────────────────────────
// These read frame() directly (few nodes) — the live top-level region.
function Ledger(props) {
	const f = props.f;
	return (() => {
		const _el7 = $ss.el("div");
		$ss.attr(_el7, "id", "ledger");
		$ss.child(_el7, (() => {
			const _el8 = $ss.el("div");
			$ss.attr(_el8, "class", "ledger-brand");
			$ss.child(_el8, (() => {
				const _el9 = $ss.el("span");
				$ss.attr(_el9, "class", "brand-mark");
				$ss.child(_el9, $ss.txt("◆"));
				return _el9;
			})());
			$ss.child(_el8, (() => {
				const _el10 = $ss.el("span");
				$ss.attr(_el10, "class", "brand-name");
				$ss.child(_el10, $ss.txt("CITADEL"));
				return _el10;
			})());
			return _el8;
		})());
		$ss.child(_el7, (() => {
			const _el11 = $ss.el("div");
			$ss.attr(_el11, "class", "chips");
			$ss.insert(_el11, () => $ss.cmp(Keyed, {
				get each() {
					return f().resources;
				},
				by: "id",
				get children() {
					return (r) => (() => {
						const _el0 = $ss.el("div");
						$ss.attr(_el0, "class", "chip");
						$ss.bind(_el0, "data-id", () => r.id);
						$ss.child(_el0, (() => {
							const _el1 = $ss.el("span");
							$ss.attr(_el1, "class", "chip-icon");
							$ss.insert(_el1, () => r.icon);
							return _el1;
						})());
						$ss.child(_el0, (() => {
							const _el2 = $ss.el("div");
							$ss.attr(_el2, "class", "chip-body");
							$ss.child(_el2, (() => {
								const _el3 = $ss.el("span");
								$ss.attr(_el3, "class", "chip-name");
								$ss.insert(_el3, () => r.name);
								return _el3;
							})());
							$ss.child(_el2, (() => {
								const _el4 = $ss.el("div");
								$ss.attr(_el4, "class", "chip-nums");
								$ss.child(_el4, (() => {
									const _el5 = $ss.el("span");
									$ss.attr(_el5, "class", "chip-val");
									$ss.insert(_el5, () => amt(r.current));
									return _el5;
								})());
								$ss.child(_el4, (() => {
									const _el6 = $ss.el("span");
									$ss.bind(_el6, "class", () => r.rate >= 0 ? "chip-rate up" : "chip-rate down");
									$ss.insert(_el6, () => rate(r.rate));
									return _el6;
								})());
								return _el4;
							})());
							return _el2;
						})());
						return _el0;
					})();
				}
			}));
			return _el11;
		})());
		$ss.child(_el7, (() => {
			const _el12 = $ss.el("div");
			$ss.attr(_el12, "class", "clock-box");
			$ss.child(_el12, (() => {
				const _el13 = $ss.el("span");
				$ss.attr(_el13, "class", "clock-label");
				$ss.child(_el13, $ss.txt("MISSION"));
				return _el13;
			})());
			$ss.child(_el12, (() => {
				const _el14 = $ss.el("span");
				$ss.attr(_el14, "class", "clock");
				$ss.attr(_el14, "id", "mission-clock");
				$ss.insert(_el14, () => mmss(f().clock));
				return _el14;
			})());
			return _el12;
		})());
		return _el7;
	})();
}
$ss.hot("assets/ui/citadel\\app.tsx#Ledger", Ledger);
// ── Left: tech rail ─────────────────────────────────────────────────────────
function TechRail(props) {
	const f = props.f;
	return (() => {
		const _el22 = $ss.el("div");
		$ss.attr(_el22, "class", "rail");
		$ss.attr(_el22, "id", "tech-rail");
		$ss.child(_el22, (() => {
			const _el23 = $ss.el("div");
			$ss.attr(_el23, "class", "rail-head");
			$ss.child(_el23, (() => {
				const _el24 = $ss.el("span");
				$ss.attr(_el24, "class", "rail-title");
				$ss.child(_el24, $ss.txt("RESEARCH"));
				return _el24;
			})());
			$ss.child(_el23, (() => {
				const _el25 = $ss.el("span");
				$ss.attr(_el25, "class", "rail-sub");
				$ss.child(_el25, $ss.txt("tech tree"));
				return _el25;
			})());
			return _el23;
		})());
		$ss.child(_el22, (() => {
			const _el26 = $ss.el("div");
			$ss.attr(_el26, "class", "rail-list");
			$ss.insert(_el26, () => $ss.cmp(Keyed, {
				get each() {
					return f().techs;
				},
				by: "id",
				get children() {
					return (t) => (() => {
						const _el15 = $ss.el("div");
						$ss.bind(_el15, "class", () => "tech " + t.state);
						$ss.bind(_el15, "data-id", () => t.id);
						$ss.child(_el15, (() => {
							const _el16 = $ss.el("span");
							$ss.attr(_el16, "class", "tech-dot");
							return _el16;
						})());
						$ss.child(_el15, (() => {
							const _el17 = $ss.el("div");
							$ss.attr(_el17, "class", "tech-body");
							$ss.child(_el17, (() => {
								const _el18 = $ss.el("span");
								$ss.attr(_el18, "class", "tech-name");
								$ss.insert(_el18, () => t.name);
								return _el18;
							})());
							$ss.child(_el17, (() => {
								const _el19 = $ss.el("div");
								$ss.attr(_el19, "class", "tech-track");
								$ss.child(_el19, (() => {
									const _el20 = $ss.el("div");
									$ss.attr(_el20, "class", "tech-fill");
									$ss.bind(_el20, "style", () => `width: ${Math.round(t.progress * 100)}%`);
									return _el20;
								})());
								return _el19;
							})());
							return _el17;
						})());
						$ss.child(_el15, (() => {
							const _el21 = $ss.el("span");
							$ss.attr(_el21, "class", "tech-state");
							$ss.insert(_el21, () => t.state);
							return _el21;
						})());
						return _el15;
					})();
				}
			}));
			return _el26;
		})());
		return _el22;
	})();
}
$ss.hot("assets/ui/citadel\\app.tsx#TechRail", TechRail);
// ── Center: production grid of building cards ───────────────────────────────
function ProductionGrid(props) {
	const f = props.f;
	return (() => {
		const _el43 = $ss.el("div");
		$ss.attr(_el43, "id", "production");
		$ss.child(_el43, (() => {
			const _el44 = $ss.el("div");
			$ss.attr(_el44, "class", "prod-head");
			$ss.child(_el44, (() => {
				const _el45 = $ss.el("span");
				$ss.attr(_el45, "class", "prod-title");
				$ss.child(_el45, $ss.txt("PRODUCTION"));
				return _el45;
			})());
			$ss.child(_el44, (() => {
				const _el46 = $ss.el("span");
				$ss.attr(_el46, "class", "prod-sub");
				$ss.child(_el46, $ss.txt("imperial construction registry"));
				return _el46;
			})());
			return _el44;
		})());
		$ss.child(_el43, (() => {
			const _el47 = $ss.el("div");
			$ss.attr(_el47, "class", "grid");
			$ss.attr(_el47, "id", "build-grid");
			$ss.insert(_el47, () => $ss.cmp(Keyed, {
				get each() {
					return f().buildings;
				},
				by: "id",
				get children() {
					return (b) => (() => {
						const _el27 = $ss.el("div");
						$ss.bind(_el27, "class", () => "card " + b.category + " tier-" + b.tier + " " + b.state);
						$ss.bind(_el27, "data-id", () => b.id);
						$ss.child(_el27, (() => {
							const _el28 = $ss.el("div");
							$ss.attr(_el28, "class", "card-top");
							$ss.child(_el28, (() => {
								const _el29 = $ss.el("span");
								$ss.attr(_el29, "class", "card-name");
								$ss.insert(_el29, () => b.name);
								return _el29;
							})());
							$ss.child(_el28, (() => {
								const _el30 = $ss.el("span");
								$ss.bind(_el30, "class", () => "tier-dot t" + b.tier);
								return _el30;
							})());
							return _el28;
						})());
						$ss.child(_el27, (() => {
							const _el31 = $ss.el("div");
							$ss.attr(_el31, "class", "card-tags");
							$ss.child(_el31, (() => {
								const _el32 = $ss.el("span");
								$ss.bind(_el32, "class", () => "tag cat " + b.category);
								$ss.insert(_el32, () => b.category);
								return _el32;
							})());
							$ss.child(_el31, (() => {
								const _el33 = $ss.el("span");
								$ss.attr(_el33, "class", "tag lvl");
								$ss.insert(_el33, () => `Lvl ${b.level}`);
								return _el33;
							})());
							$ss.child(_el31, (() => {
								const _el34 = $ss.el("span");
								$ss.bind(_el34, "class", () => b.affordable ? "tag afford ok" : "tag afford no");
								$ss.insert(_el34, () => b.affordable ? "ready" : "short");
								return _el34;
							})());
							return _el31;
						})());
						$ss.child(_el27, (() => {
							const _el35 = $ss.el("div");
							$ss.attr(_el35, "class", "card-cost");
							$ss.child(_el35, (() => {
								const _el36 = $ss.el("span");
								$ss.attr(_el36, "class", "cost-label");
								$ss.child(_el36, $ss.txt("COST"));
								return _el36;
							})());
							$ss.child(_el35, (() => {
								const _el37 = $ss.el("span");
								$ss.attr(_el37, "class", "cost-vals");
								$ss.insert(_el37, () => costLine(b));
								return _el37;
							})());
							return _el35;
						})());
						$ss.child(_el27, (() => {
							const _el38 = $ss.el("div");
							$ss.attr(_el38, "class", "card-track");
							$ss.child(_el38, (() => {
								const _el39 = $ss.el("div");
								$ss.attr(_el39, "class", "card-fill");
								$ss.bind(_el39, "style", () => `width: ${Math.round(b.progress * 100)}%`);
								return _el39;
							})());
							return _el38;
						})());
						$ss.child(_el27, (() => {
							const _el40 = $ss.el("div");
							$ss.attr(_el40, "class", "card-foot");
							$ss.child(_el40, (() => {
								const _el41 = $ss.el("span");
								$ss.bind(_el41, "class", () => "badge st " + b.state);
								$ss.insert(_el41, () => b.state);
								return _el41;
							})());
							$ss.child(_el40, (() => {
								const _el42 = $ss.el("span");
								$ss.attr(_el42, "class", "card-tierlab");
								$ss.insert(_el42, () => `T${b.tier}`);
								return _el42;
							})());
							return _el40;
						})());
						return _el27;
					})();
				}
			}));
			return _el47;
		})());
		return _el43;
	})();
}
$ss.hot("assets/ui/citadel\\app.tsx#ProductionGrid", ProductionGrid);
// ── Right: unit roster ──────────────────────────────────────────────────────
function UnitRoster(props) {
	const f = props.f;
	return (() => {
		const _el53 = $ss.el("div");
		$ss.attr(_el53, "class", "side-panel");
		$ss.attr(_el53, "id", "roster");
		$ss.child(_el53, (() => {
			const _el54 = $ss.el("div");
			$ss.attr(_el54, "class", "side-head");
			$ss.child(_el54, (() => {
				const _el55 = $ss.el("span");
				$ss.attr(_el55, "class", "side-title");
				$ss.child(_el55, $ss.txt("FLEET ROSTER"));
				return _el55;
			})());
			return _el54;
		})());
		$ss.child(_el53, (() => {
			const _el56 = $ss.el("div");
			$ss.attr(_el56, "class", "roster-list");
			$ss.insert(_el56, () => $ss.cmp(Keyed, {
				get each() {
					return f().units;
				},
				by: "id",
				get children() {
					return (u) => (() => {
						const _el48 = $ss.el("div");
						$ss.bind(_el48, "class", () => "unit " + u.status);
						$ss.bind(_el48, "data-id", () => u.id);
						$ss.child(_el48, (() => {
							const _el49 = $ss.el("span");
							$ss.attr(_el49, "class", "unit-glyph");
							$ss.child(_el49, $ss.txt("▣"));
							return _el49;
						})());
						$ss.child(_el48, (() => {
							const _el50 = $ss.el("span");
							$ss.attr(_el50, "class", "unit-name");
							$ss.insert(_el50, () => u.name);
							return _el50;
						})());
						$ss.child(_el48, (() => {
							const _el51 = $ss.el("span");
							$ss.attr(_el51, "class", "unit-count");
							$ss.insert(_el51, () => `x${u.count}`);
							return _el51;
						})());
						$ss.child(_el48, (() => {
							const _el52 = $ss.el("span");
							$ss.bind(_el52, "class", () => "unit-status " + u.status);
							$ss.insert(_el52, () => u.status);
							return _el52;
						})());
						return _el48;
					})();
				}
			}));
			return _el56;
		})());
		return _el53;
	})();
}
$ss.hot("assets/ui/citadel\\app.tsx#UnitRoster", UnitRoster);
// ── Right: build queue (buildings currently `building`) ─────────────────────
function BuildQueue(props) {
	const f = props.f;
	const queued = createMemo(() => f().buildings.filter((b) => b.state === "building"));
	return (() => {
		const _el62 = $ss.el("div");
		$ss.attr(_el62, "class", "side-panel");
		$ss.attr(_el62, "id", "queue");
		$ss.child(_el62, (() => {
			const _el63 = $ss.el("div");
			$ss.attr(_el63, "class", "side-head");
			$ss.child(_el63, (() => {
				const _el64 = $ss.el("span");
				$ss.attr(_el64, "class", "side-title");
				$ss.child(_el64, $ss.txt("BUILD QUEUE"));
				return _el64;
			})());
			return _el63;
		})());
		$ss.child(_el62, (() => {
			const _el65 = $ss.el("div");
			$ss.attr(_el65, "class", "queue-list");
			$ss.insert(_el65, () => $ss.cmp(Keyed, {
				get each() {
					return queued();
				},
				by: "id",
				get children() {
					return (b) => (() => {
						const _el57 = $ss.el("div");
						$ss.bind(_el57, "class", () => "qrow tier-" + b.tier);
						$ss.bind(_el57, "data-id", () => b.id);
						$ss.child(_el57, (() => {
							const _el58 = $ss.el("span");
							$ss.attr(_el58, "class", "q-name");
							$ss.insert(_el58, () => b.name);
							return _el58;
						})());
						$ss.child(_el57, (() => {
							const _el59 = $ss.el("div");
							$ss.attr(_el59, "class", "q-track");
							$ss.child(_el59, (() => {
								const _el60 = $ss.el("div");
								$ss.attr(_el60, "class", "q-fill");
								$ss.bind(_el60, "style", () => `width: ${Math.round(b.progress * 100)}%`);
								return _el60;
							})());
							return _el59;
						})());
						$ss.child(_el57, (() => {
							const _el61 = $ss.el("span");
							$ss.attr(_el61, "class", "q-pct");
							$ss.insert(_el61, () => `${Math.round(b.progress * 100)}%`);
							return _el61;
						})());
						return _el57;
					})();
				}
			}));
			return _el65;
		})());
		return _el62;
	})();
}
$ss.hot("assets/ui/citadel\\app.tsx#BuildQueue", BuildQueue);
// ── Right: event log (fading lines) ─────────────────────────────────────────
function EventLog(props) {
	const f = props.f;
	return (() => {
		const _el67 = $ss.el("div");
		$ss.attr(_el67, "class", "side-panel");
		$ss.attr(_el67, "id", "events");
		$ss.child(_el67, (() => {
			const _el68 = $ss.el("div");
			$ss.attr(_el68, "class", "side-head");
			$ss.child(_el68, (() => {
				const _el69 = $ss.el("span");
				$ss.attr(_el69, "class", "side-title");
				$ss.child(_el69, $ss.txt("DISPATCHES"));
				return _el69;
			})());
			return _el68;
		})());
		$ss.child(_el67, (() => {
			const _el70 = $ss.el("div");
			$ss.attr(_el70, "class", "event-list");
			$ss.insert(_el70, () => $ss.cmp(Keyed, {
				get each() {
					return f().events;
				},
				by: "id",
				get children() {
					return (e) => (() => {
						const _el66 = $ss.el("span");
						$ss.attr(_el66, "class", "event-line");
						$ss.bind(_el66, "data-id", () => e.id);
						$ss.bind(_el66, "style", () => `color: rgba(206, 221, 245, ${Math.max(.28, 1 - e.age * .06).toFixed(3)})`);
						$ss.insert(_el66, () => "> " + e.text);
						return _el66;
					})();
				}
			}));
			return _el70;
		})());
		return _el67;
	})();
}
$ss.hot("assets/ui/citadel\\app.tsx#EventLog", EventLog);
function App() {
	const [frame, setFrame] = createSignal(EMPTY);
	bevy.on("frame", (f) => {
		// Events have no id in the DTO; synthesize a stable key from index so
		// <Keyed> has a `by` field.
		if (f.events) f.events.forEach((e, i) => {
			e.id = i;
		});
		setFrame(f);
	});
	return (() => {
		const _el71 = $ss.el("div");
		$ss.attr(_el71, "id", "hud");
		$ss.insert(_el71, () => $ss.cmp(Ledger, { get f() {
			return frame;
		} }));
		$ss.child(_el71, (() => {
			const _el72 = $ss.el("div");
			$ss.attr(_el72, "id", "body");
			$ss.insert(_el72, () => $ss.cmp(TechRail, { get f() {
				return frame;
			} }));
			$ss.insert(_el72, () => $ss.cmp(ProductionGrid, { get f() {
				return frame;
			} }));
			$ss.child(_el72, (() => {
				const _el73 = $ss.el("div");
				$ss.attr(_el73, "id", "right-column");
				$ss.insert(_el73, () => $ss.cmp(UnitRoster, { get f() {
					return frame;
				} }));
				$ss.insert(_el73, () => $ss.cmp(BuildQueue, { get f() {
					return frame;
				} }));
				$ss.insert(_el73, () => $ss.cmp(EventLog, { get f() {
					return frame;
				} }));
				return _el73;
			})());
			return _el72;
		})());
		return _el71;
	})();
}
$ss.hot("assets/ui/citadel\\app.tsx#App", App);
render(() => $ss.cmp(App, {}), document.getElementById("root"));
