import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/tauri";
import { useEffect, useMemo, useRef, useState } from "react";

import { ColorWheel } from "./components/ColorWheel";
import {
    cloneEffects,
    compileEffect,
    CompiledEffect,
    Effect,
    frameColors,
    hexToRgb,
    LightTarget,
    makeEffect,
    MODES,
    Preset,
    RgbMode,
} from "./rgb";

import "./App.css";

const STORAGE_EFFECTS = "quady.effects";
const STORAGE_SWATCHES = "quady.swatches";
const STORAGE_PRESETS = "quady.presets";

const PRESET_SWATCHES = [
    "#ff3b30",
    "#ff9500",
    "#ffd60a",
    "#34c759",
    "#00c7be",
    "#30b0ff",
    "#0a84ff",
    "#5856d6",
    "#af52de",
    "#ff2d92",
    "#ffffff",
];

function loadEffects(): Effect[] {
    try {
        const raw = localStorage.getItem(STORAGE_EFFECTS);
        if (raw) {
            const parsed = JSON.parse(raw) as Effect[];
            if (Array.isArray(parsed) && parsed.length > 0) return parsed;
        }
    } catch {
        // fall through to default
    }
    return [makeEffect("Cycle")];
}

function loadSwatches(): string[] {
    try {
        const raw = localStorage.getItem(STORAGE_SWATCHES);
        if (raw) return JSON.parse(raw) as string[];
    } catch {
        // ignore
    }
    return [];
}

function loadPresets(): Preset[] {
    try {
        const raw = localStorage.getItem(STORAGE_PRESETS);
        if (raw) {
            const parsed = JSON.parse(raw) as Preset[];
            if (Array.isArray(parsed)) return parsed;
        }
    } catch {
        // ignore
    }
    return [];
}

// ------------------------------------------------------------------- icons

function ModeIcon({ mode }: { mode: RgbMode }) {
    const stroke = {
        fill: "none",
        stroke: "currentColor",
        strokeWidth: 1.6,
        strokeLinecap: "round" as const,
        strokeLinejoin: "round" as const,
    };
    return (
        <svg className="mode-icon" viewBox="0 0 16 16" width="15" height="15">
            {mode === "Solid" && (
                <circle cx="8" cy="8" r="4.5" fill="currentColor" />
            )}
            {mode === "Blink" && (
                <>
                    <circle cx="8" cy="8" r="2.8" fill="currentColor" />
                    <path
                        {...stroke}
                        d="M8 1.2v2M8 12.8v2M1.2 8h2M12.8 8h2M3.2 3.2l1.4 1.4M11.4 11.4l1.4 1.4M12.8 3.2l-1.4 1.4M4.6 11.4l-1.4 1.4"
                    />
                </>
            )}
            {mode === "Cycle" && (
                <>
                    <path {...stroke} d="M13.4 8.6A5.5 5.5 0 1 1 12 4.2" />
                    <path {...stroke} d="M12.4 1.6v2.8H9.6" />
                </>
            )}
            {mode === "Wave" && (
                <path
                    {...stroke}
                    d="M1.2 8c1.9-3.6 3.9-3.6 5.7 0s3.8 3.6 5.7 0"
                />
            )}
            {mode === "Lightning" && (
                <path
                    d="M9.2 1 3.6 9h3.2l-.9 6L12.4 7H9.2l1-6z"
                    fill="currentColor"
                />
            )}
            {mode === "Pulse" && (
                <>
                    <circle cx="8" cy="8" r="2.2" fill="currentColor" />
                    <circle {...stroke} cx="8" cy="8" r="5.4" />
                </>
            )}
        </svg>
    );
}

// -------------------------------------------------------------------- stage

interface StageProps {
    /** Effect driving each LED group, or null when the group is untargeted. */
    topEffect: Effect | null;
    bottomEffect: Effect | null;
    /** The effect currently being edited (for the target highlight). */
    selected: Effect | null;
    selecting: boolean;
    onPick: (which: "top" | "bottom") => void;
}

function LightsStage({
    topEffect,
    bottomEffect,
    selected,
    selecting,
    onPick,
}: StageProps) {
    const stageRef = useRef<HTMLElement>(null);
    const topRef = useRef<HTMLDivElement>(null);
    const bottomRef = useRef<HTMLDivElement>(null);

    const topCompiled = useMemo(
        () => (topEffect ? compileEffect(topEffect) : null),
        [topEffect],
    );
    const bottomCompiled = useMemo(
        () => (bottomEffect ? compileEffect(bottomEffect) : null),
        [bottomEffect],
    );

    useEffect(() => {
        const paint = (el: HTMLDivElement | null, c: string) => {
            if (!el) return;
            el.style.background = `radial-gradient(circle at 38% 34%, ${c}, ${c} 55%, transparent 130%)`;
            el.style.boxShadow = `0 0 34px 8px ${c}b0, 0 0 110px 34px ${c}45`;
        };
        let raf = 0;
        const start = performance.now();
        const colorAt = (
            compiled: CompiledEffect | null,
            t: number,
            which: "top" | "bottom",
        ) => {
            if (!compiled) return "#000000";
            const c = frameColors(compiled, t);
            return which === "top" ? c.top : c.bottom;
        };
        const tick = (now: number) => {
            const t = now - start;
            const top = colorAt(topCompiled, t, "top");
            const bottom = colorAt(bottomCompiled, t, "bottom");
            paint(topRef.current, top);
            paint(bottomRef.current, bottom);
            if (stageRef.current) {
                stageRef.current.style.setProperty("--amb-top", `${top}1f`);
                stageRef.current.style.setProperty(
                    "--amb-bottom",
                    `${bottom}1a`,
                );
            }
            raf = requestAnimationFrame(tick);
        };
        raf = requestAnimationFrame(tick);
        return () => cancelAnimationFrame(raf);
    }, [topCompiled, bottomCompiled]);

    const target = selected?.target ?? "all";
    const lightClass = (which: "top" | "bottom") =>
        [
            "light",
            selecting ? "pickable" : "",
            !selecting && target === which ? "targeted" : "",
        ]
            .filter(Boolean)
            .join(" ");

    return (
        <main className="stage" ref={stageRef}>
            {selecting && (
                <div className="stage-hint">
                    <span className="hint-dot" />
                    Click a light to target it
                </div>
            )}
            <div className="lights">
                <div
                    className={lightClass("top")}
                    title="Top light"
                    onClick={() => selecting && onPick("top")}
                >
                    <div className="light-core" ref={topRef} />
                </div>
                <div
                    className={lightClass("bottom")}
                    title="Bottom light"
                    onClick={() => selecting && onPick("bottom")}
                >
                    <div className="light-core" ref={bottomRef} />
                </div>
            </div>
        </main>
    );
}

// ------------------------------------------------------------------ presets

interface PresetsMenuProps {
    presets: Preset[];
    activeId: string | null;
    onApply: (id: string) => void;
    onSave: (name: string) => void;
    onDelete: (id: string) => void;
}

function PresetsMenu({
    presets,
    activeId,
    onApply,
    onSave,
    onDelete,
}: PresetsMenuProps) {
    const [open, setOpen] = useState(false);
    const [name, setName] = useState("");
    const active = presets.find((p) => p.id === activeId);

    const commitSave = () => {
        const trimmed = name.trim();
        if (!trimmed) return;
        onSave(trimmed);
        setName("");
    };

    return (
        <div className="presets-wrap">
            <button
                className="presets-toggle"
                onClick={() => setOpen((v) => !v)}
                title="Presets"
            >
                <svg viewBox="0 0 16 16" width="13" height="13">
                    <path
                        d="M3 3.5h10M3 8h10M3 12.5h10"
                        stroke="currentColor"
                        strokeWidth="1.5"
                        strokeLinecap="round"
                    />
                </svg>
                <span className="presets-label">
                    {active ? active.name : "Presets"}
                </span>
                <svg viewBox="0 0 16 16" width="11" height="11">
                    <path
                        d="M4 6l4 4 4-4"
                        stroke="currentColor"
                        strokeWidth="1.6"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        fill="none"
                    />
                </svg>
            </button>
            {open && (
                <>
                    <div
                        className="menu-backdrop"
                        onClick={() => setOpen(false)}
                    />
                    <div className="presets-menu">
                        {presets.length === 0 && (
                            <div className="presets-empty">
                                No presets saved yet
                            </div>
                        )}
                        {presets.map((p) => (
                            <div
                                key={p.id}
                                className={`presets-item${p.id === activeId ? " active" : ""}`}
                                onClick={() => {
                                    onApply(p.id);
                                    setOpen(false);
                                }}
                            >
                                <span className="presets-check">
                                    {p.id === activeId ? "✓" : ""}
                                </span>
                                <span className="presets-name">{p.name}</span>
                                <button
                                    className="presets-del"
                                    title="Delete preset"
                                    onClick={(e) => {
                                        e.stopPropagation();
                                        onDelete(p.id);
                                    }}
                                >
                                    ×
                                </button>
                            </div>
                        ))}
                        <div className="presets-save">
                            <input
                                type="text"
                                placeholder="Save current as…"
                                value={name}
                                onChange={(e) => setName(e.target.value)}
                                onKeyDown={(e) => {
                                    if (e.key === "Enter") commitSave();
                                }}
                            />
                            <button
                                disabled={!name.trim()}
                                onClick={commitSave}
                            >
                                Save
                            </button>
                        </div>
                    </div>
                </>
            )}
        </div>
    );
}

// ---------------------------------------------------------------------- app

const initialEffects = loadEffects();

function App() {
    const [effects, setEffects] = useState<Effect[]>(initialEffects);
    const [selectedId, setSelectedId] = useState<string | null>(
        initialEffects[0]?.id ?? null,
    );
    const [colorIndex, setColorIndex] = useState(0);
    const [addOpen, setAddOpen] = useState(false);
    const [selecting, setSelecting] = useState(false);
    const [swatches, setSwatches] = useState<string[]>(loadSwatches);
    const [presets, setPresets] = useState<Preset[]>(loadPresets);
    const [activePresetId, setActivePresetId] = useState<string | null>(null);
    const [statusText, setStatusText] = useState("backend unavailable");

    const sel = effects.find((e) => e.id === selectedId) ?? null;
    const stopIndex = sel ? Math.min(colorIndex, sel.colors.length - 1) : 0;
    const currentColor = sel?.colors[stopIndex] ?? "#ff0000";

    useEffect(() => {
        localStorage.setItem(STORAGE_EFFECTS, JSON.stringify(effects));
    }, [effects]);

    useEffect(() => {
        pushEffectsToBackend(effects);
    }, [effects]);

    useEffect(() => {
        let mounted = true;
        const refreshStatus = async () => {
            try {
                const status = await invoke<string>("device_status");
                if (mounted) setStatusText(status);
            } catch {
                if (mounted) setStatusText("backend unavailable");
            }
        };

        refreshStatus();
        const interval = window.setInterval(refreshStatus, 5000);
        return () => {
            mounted = false;
            window.clearInterval(interval);
        };
    }, []);

    useEffect(() => {
        localStorage.setItem(STORAGE_SWATCHES, JSON.stringify(swatches));
    }, [swatches]);

    useEffect(() => {
        localStorage.setItem(STORAGE_PRESETS, JSON.stringify(presets));
    }, [presets]);

    const pushEffectsToBackend = async (all: Effect[]) => {
        try {
            const cfgs = all.map((effect) => ({
                mode: effect.mode,
                colors: effect.colors.map((hex) => {
                    const [r, g, b] = hexToRgb(hex);
                    return (r << 16) | (g << 8) | b;
                }),
                speed: effect.speed,
                brightness: effect.brightness,
                target: effect.target,
            }));
            await invoke("apply_effects", { cfgs });
        } catch (error) {
            console.error("Failed to apply effects:", error);
        }
    };

    const updateSel = (patch: Partial<Effect>) => {
        if (!sel) return;
        setEffects((all) =>
            all.map((e) => (e.id === sel.id ? { ...e, ...patch } : e)),
        );
    };

    const addEffect = (mode: RgbMode) => {
        const fx = makeEffect(mode);
        setEffects((all) => [...all, fx]);
        setSelectedId(fx.id);
        setColorIndex(0);
        setAddOpen(false);
    };

    const removeEffect = (id: string) => {
        setEffects((all) => {
            const next = all.filter((e) => e.id !== id);
            if (id === selectedId) {
                setSelectedId(next[0]?.id ?? null);
                setColorIndex(0);
            }
            return next;
        });
    };

    const setStopColor = (hex: string) => {
        if (!sel) return;
        const colors = sel.colors.map((c, i) => (i === stopIndex ? hex : c));
        updateSel({ colors });
    };

    const addStopAt = (fraction: number) => {
        if (!sel || sel.mode === "Solid") return;
        const n = sel.colors.length;
        const idx = Math.min(n, Math.max(1, Math.round(fraction * n)));
        const colors = [...sel.colors];
        colors.splice(idx, 0, currentColor);
        updateSel({ colors });
        setColorIndex(idx);
    };

    const removeStop = (idx: number) => {
        if (!sel || sel.colors.length <= 1) return;
        const colors = sel.colors.filter((_, i) => i !== idx);
        updateSel({ colors });
        setColorIndex(Math.max(0, Math.min(idx, colors.length - 1)));
    };

    const savePreset = (name: string) => {
        const preset: Preset = {
            id: `ps-${Math.random().toString(36).slice(2, 9)}`,
            name,
            effects: cloneEffects(effects),
        };
        setPresets((all) => {
            // Overwrite a preset with the same name instead of duplicating.
            const existing = all.find((p) => p.name === name);
            if (existing) {
                setActivePresetId(existing.id);
                return all.map((p) =>
                    p.name === name ? { ...preset, id: existing.id } : p,
                );
            }
            setActivePresetId(preset.id);
            return [...all, preset];
        });
    };

    const applyPreset = (id: string) => {
        const preset = presets.find((p) => p.id === id);
        if (!preset) return;
        const next = cloneEffects(preset.effects);
        setEffects(next);
        setSelectedId(next[0]?.id ?? null);
        setColorIndex(0);
        setSelecting(false);
        setActivePresetId(id);
    };

    // Keep a ref to the latest applyPreset so the tray listener (subscribed
    // once) always resolves against current preset state.
    const applyPresetRef = useRef(applyPreset);
    applyPresetRef.current = applyPreset;

    useEffect(() => {
        const unlisten = listen<string>("apply-preset", (event) => {
            applyPresetRef.current(event.payload);
        });
        return () => {
            unlisten.then((fn) => fn());
        };
    }, []);

    // Mirror the preset list (and which one is active) into the menu-bar tray.
    useEffect(() => {
        invoke("set_tray_presets", {
            presets: presets.map((p) => ({
                id: p.id,
                name: p.name,
                active: p.id === activePresetId,
            })),
        }).catch(() => {
            // Tray unavailable (e.g. running in the browser) — ignore.
        });
    }, [presets, activePresetId]);

    const deletePreset = (id: string) => {
        setPresets((all) => all.filter((p) => p.id !== id));
        setActivePresetId((cur) => (cur === id ? null : cur));
    };

    // Editing the effects means we're no longer on a saved preset verbatim.
    useEffect(() => {
        setActivePresetId((cur) => {
            if (!cur) return cur;
            const preset = presets.find((p) => p.id === cur);
            if (!preset) return null;
            const strip = (list: Effect[]) =>
                list.map(({ id: _id, ...rest }) => rest);
            const same =
                JSON.stringify(strip(preset.effects)) ===
                JSON.stringify(strip(effects));
            return same ? cur : null;
        });
    }, [effects, presets]);

    // Same resolution rule as the backend: the last effect in the list whose
    // target includes an LED group drives that group.
    const effectFor = (which: LightTarget): Effect | null => {
        for (let i = effects.length - 1; i >= 0; i--) {
            const e = effects[i];
            if (e.target === "all" || e.target === which) return e;
        }
        return null;
    };

    const isGradientMode = sel?.mode === "Cycle" || sel?.mode === "Wave";

    const gradient =
        sel && sel.colors.length > 1
            ? `linear-gradient(90deg, ${sel.colors
                  .map((c, i) => `${c} ${(i / (sel.colors.length - 1)) * 100}%`)
                  .join(", ")})`
            : currentColor;

    return (
        <div className="app">
            <header className="topbar">
                <span className="wordmark">Quady</span>
                <span className="topbar-sub">HyperX QuadCast S</span>
                <PresetsMenu
                    presets={presets}
                    activeId={activePresetId}
                    onApply={applyPreset}
                    onSave={savePreset}
                    onDelete={deletePreset}
                />
                <span className="topbar-status">{statusText}</span>
            </header>

            <LightsStage
                topEffect={effectFor("top")}
                bottomEffect={effectFor("bottom")}
                selected={sel}
                selecting={selecting}
                onPick={(which) => {
                    updateSel({ target: which });
                    setSelecting(false);
                }}
            />

            <footer className="controls">
                {/* ------------------------------------------------------ EFFECTS */}
                <section className="panel panel-effects">
                    <h2>Effects</h2>
                    <div className="fx-add-wrap">
                        <button
                            className="fx-row fx-add"
                            onClick={() => setAddOpen((v) => !v)}
                        >
                            <svg viewBox="0 0 16 16" width="15" height="15">
                                <path
                                    d="M8 2.5v11M2.5 8h11"
                                    stroke="currentColor"
                                    strokeWidth="1.6"
                                    strokeLinecap="round"
                                />
                            </svg>
                            Add Effect
                        </button>
                        {addOpen && (
                            <>
                                <div
                                    className="menu-backdrop"
                                    onClick={() => setAddOpen(false)}
                                />
                                <div className="fx-menu">
                                    {MODES.map((m) => (
                                        <button
                                            key={m.mode}
                                            onClick={() => addEffect(m.mode)}
                                        >
                                            <ModeIcon mode={m.mode} />
                                            <span className="fx-menu-name">
                                                {m.mode}
                                            </span>
                                            <span className="fx-menu-hint">
                                                {m.hint}
                                            </span>
                                        </button>
                                    ))}
                                    <button disabled>
                                        <svg
                                            className="mode-icon"
                                            viewBox="0 0 16 16"
                                            width="15"
                                            height="15"
                                        >
                                            <path
                                                d="M2 12V9M5 12V5M8 12V7M11 12V3M14 12v-6"
                                                stroke="currentColor"
                                                strokeWidth="1.6"
                                                strokeLinecap="round"
                                            />
                                        </svg>
                                        <span className="fx-menu-name">
                                            Visualizer
                                        </span>
                                        <span className="fx-menu-hint">
                                            Coming soon
                                        </span>
                                    </button>
                                </div>
                            </>
                        )}
                    </div>
                    <div className="fx-list">
                        {effects.map((fx) => (
                            <div
                                key={fx.id}
                                className={`fx-row${fx.id === selectedId ? " selected" : ""}`}
                                onClick={() => {
                                    setSelectedId(fx.id);
                                    setColorIndex(0);
                                }}
                            >
                                <ModeIcon mode={fx.mode} />
                                <span className="fx-name">{fx.mode}</span>
                                <button
                                    className="fx-del"
                                    title="Remove effect"
                                    onClick={(e) => {
                                        e.stopPropagation();
                                        removeEffect(fx.id);
                                    }}
                                >
                                    ×
                                </button>
                            </div>
                        ))}
                    </div>
                </section>

                {/* ------------------------------------------------------- TARGET */}
                <section
                    className={`panel panel-target${sel ? "" : " disabled"}`}
                >
                    <h2>Target</h2>
                    <div className="seg">
                        <button
                            className={sel?.target === "all" ? "active" : ""}
                            onClick={() => {
                                updateSel({ target: "all" });
                                setSelecting(false);
                            }}
                        >
                            All Lights
                        </button>
                        <button
                            className={
                                sel && sel.target !== "all"
                                    ? "active"
                                    : selecting
                                      ? "active"
                                      : ""
                            }
                            onClick={() => sel && setSelecting(true)}
                        >
                            Selection
                        </button>
                    </div>
                    <h2 className="sub">Opacity</h2>
                    <div className="slider-block">
                        <input
                            type="range"
                            min={0}
                            max={100}
                            disabled={!sel}
                            value={sel?.brightness ?? 100}
                            onChange={(e) =>
                                updateSel({
                                    brightness: Number(e.target.value),
                                })
                            }
                        />
                        <div className="slider-labels">
                            <span>hidden</span>
                            <span>visible</span>
                        </div>
                    </div>
                </section>

                {/* -------------------------------------------------------- COLOR */}
                <section
                    className={`panel panel-color${sel ? "" : " disabled"}`}
                >
                    <h2>Color</h2>
                    {isGradientMode ? (
                        <div
                            className="grad-bar"
                            style={{ background: gradient }}
                            title="Click to add a color stop"
                            onClick={(e) => {
                                if (
                                    !(e.target instanceof HTMLElement) ||
                                    e.target.closest(".grad-stop")
                                )
                                    return;
                                const rect =
                                    e.currentTarget.getBoundingClientRect();
                                addStopAt((e.clientX - rect.left) / rect.width);
                            }}
                        >
                            {sel?.colors.map((c, i) => {
                                const n = sel.colors.length;
                                const left = n === 1 ? 50 : (i / (n - 1)) * 100;
                                return (
                                    <div
                                        key={i}
                                        className={`grad-stop${i === stopIndex ? " selected" : ""}`}
                                        style={{
                                            left: `${left}%`,
                                            background: c,
                                        }}
                                        title="Click to edit · double-click to remove"
                                        onClick={(e) => {
                                            e.stopPropagation();
                                            setColorIndex(i);
                                        }}
                                        onDoubleClick={(e) => {
                                            e.stopPropagation();
                                            removeStop(i);
                                        }}
                                    />
                                );
                            })}
                        </div>
                    ) : (
                        <div className="stop-chips">
                            {sel?.colors.map((c, i) => (
                                <button
                                    key={i}
                                    className={`stop-chip${i === stopIndex ? " selected" : ""}`}
                                    style={{ background: c }}
                                    title={
                                        sel.colors.length > 1
                                            ? "Click to edit · double-click to remove"
                                            : "Click to edit"
                                    }
                                    onClick={() => setColorIndex(i)}
                                    onDoubleClick={() => removeStop(i)}
                                />
                            ))}
                            {sel && sel.mode !== "Solid" && (
                                <button
                                    className="stop-chip add"
                                    title="Add a color"
                                    onClick={() => {
                                        const colors = [
                                            ...sel.colors,
                                            currentColor,
                                        ];
                                        updateSel({ colors });
                                        setColorIndex(colors.length - 1);
                                    }}
                                >
                                    +
                                </button>
                            )}
                        </div>
                    )}
                    <div className="color-row">
                        <ColorWheel
                            color={currentColor}
                            onChange={setStopColor}
                            disabled={!sel}
                        />
                        <div className="swatches">
                            {[...PRESET_SWATCHES, ...swatches].map((c) => (
                                <button
                                    key={c}
                                    className="swatch"
                                    style={{ background: c }}
                                    title={c}
                                    onClick={() => setStopColor(c)}
                                />
                            ))}
                            <button
                                className="swatch add"
                                title="Save current color"
                                onClick={() =>
                                    setSwatches((s) =>
                                        s.includes(currentColor) ||
                                        PRESET_SWATCHES.includes(currentColor)
                                            ? s
                                            : [...s.slice(-7), currentColor],
                                    )
                                }
                            >
                                +
                            </button>
                        </div>
                    </div>
                </section>

                {/* -------------------------------------------------------- SPEED */}
                <section
                    className={`panel panel-speed${!sel || sel.mode === "Solid" ? " disabled" : ""}`}
                >
                    <h2>Speed</h2>
                    <div className="slider-block">
                        <input
                            type="range"
                            min={0}
                            max={100}
                            disabled={!sel || sel.mode === "Solid"}
                            value={sel?.speed ?? 50}
                            onChange={(e) =>
                                updateSel({ speed: Number(e.target.value) })
                            }
                        />
                        <div className="slider-labels">
                            <span>slow</span>
                            <span>fast</span>
                        </div>
                    </div>
                </section>
            </footer>
        </div>
    );
}

export default App;
