// OpenAnime navigation profiler. Observers and sampling run only while enabled.
(function () {
  "use strict";
  const root = window, STORAGE_KEY = "__oaProf", MAX_SAMPLES = 500;
  let active = false, startedAt = 0, previousFrame = 0, longTaskObserver = null, longTaskSupported = false, sequence = 0;
  const counters = Object.create(null), timings = Object.create(null), pendingTimers = new Map();
  const navigation = [], longTasks = [], slowFrames = [];
  function enabled() {
    try { return root.__OA_PROFILE_ENV__ === true || localStorage.getItem(STORAGE_KEY) === "1"; }
    catch (_) { return root.__OA_PROFILE_ENV__ === true; }
  }
  function push(list, value) { list.push(value); if (list.length > MAX_SAMPLES) list.shift(); }
  function count(name, amount) { counters[name] = (counters[name] || 0) + (amount == null ? 1 : amount); }
  function timing(name, ms) {
    const item = timings[name] || (timings[name] = { count: 0, totalMs: 0, maxMs: 0 });
    item.count++; item.totalMs += ms; if (ms > item.maxMs) item.maxMs = ms;
  }
  function frameTick(ts) {
    if (!active) return;
    if (previousFrame) {
      const interval = ts - previousFrame;
      if (interval > 50) push(slowFrames, { atMs: +(ts - startedAt).toFixed(2), intervalMs: +interval.toFixed(2), hidden: !!document.hidden });
    }
    previousFrame = ts;
    requestAnimationFrame(frameTick);
  }
  function activate() {
    if (active || !enabled()) return active;
    active = true; startedAt = performance.now(); previousFrame = 0;
    if (typeof PerformanceObserver === "function") {
      try {
        longTaskObserver = new PerformanceObserver(function (list) {
          for (const e of list.getEntries()) push(longTasks, { atMs: +(e.startTime - startedAt).toFixed(2), durationMs: +e.duration.toFixed(2), name: e.name || "longtask" });
        });
        longTaskObserver.observe({ entryTypes: ["longtask"] }); longTaskSupported = true;
      } catch (_) { longTaskSupported = false; }
    }
    if (typeof requestAnimationFrame === "function") requestAnimationFrame(frameTick);
    return true;
  }
  function ensure() { if (!active && enabled()) activate(); return active; }
  function time(name, fn) {
    if (typeof fn !== "function") return undefined;
    if (!ensure()) return fn();
    const t = performance.now(); let result;
    try { result = fn(); } catch (e) { timing(name, performance.now() - t); throw e; }
    if (result && typeof result.then === "function") return result.finally(function () { timing(name, performance.now() - t); });
    timing(name, performance.now() - t); return result;
  }
  function start(name) { return ensure() ? { name: name, at: performance.now() } : null; }
  function end(token) { if (token && ensure()) timing(token.name, performance.now() - token.at); }
  function timerStart(name) {
    if (!ensure()) return null;
    const token = { name: name, at: performance.now(), id: ++sequence };
    pendingTimers.set(token.id, token); count("timers." + name + ".scheduled"); return token;
  }
  function timerFinish(token, outcome) {
    if (!token || !pendingTimers.has(token.id)) return;
    pendingTimers.delete(token.id); count("timers." + token.name + "." + outcome);
    timing("timers." + token.name + ".delay", performance.now() - token.at);
  }
  function markNavigation(kind) {
    if (!ensure()) return;
    push(navigation, { sequence: ++sequence, kind: kind, atMs: +(performance.now() - startedAt).toFixed(2), href: String(location.href) });
    count("navigation." + kind);
  }
  function snapshot() {
    const longTotal = longTasks.reduce((s, x) => s + x.durationMs, 0), frameTotal = slowFrames.reduce((s, x) => s + x.intervalMs, 0);
    const summary = {};
    Object.keys(timings).forEach(k => { const x = timings[k]; summary[k] = { count: x.count, totalMs: +x.totalMs.toFixed(2), avgMs: +(x.totalMs / x.count).toFixed(2), maxMs: +x.maxMs.toFixed(2) }; });
    return {
      durationMs: +(performance.now() - startedAt).toFixed(2), navigationCount: navigation.length,
      longTaskCount: longTasks.length, longTaskTotalMs: +longTotal.toFixed(2),
      slowFrameCount: slowFrames.length, slowFrameTotalMs: +frameTotal.toFixed(2),
      observerCallbackCount: Object.keys(counters).filter(k => /observer\.callback$/.test(k)).reduce((s, k) => s + counters[k], 0),
      timings: summary
    };
  }
  function readBaseline() { try { const x = localStorage.getItem(STORAGE_KEY + "Baseline"); return x ? JSON.parse(x) : null; } catch (_) { return null; } }
  function setBaseline(value) { try { localStorage.setItem(STORAGE_KEY + "Baseline", JSON.stringify(value)); return true; } catch (_) { return false; } }
  function report() {
    ensure(); const metrics = snapshot(), baseline = readBaseline(), delta = {};
    ["navigationCount","longTaskCount","longTaskTotalMs","slowFrameCount","slowFrameTotalMs","observerCallbackCount"].forEach(k => {
      if (baseline && baseline.metrics && Number.isFinite(Number(baseline.metrics[k]))) delta[k] = { baseline: Number(baseline.metrics[k]), profiled: metrics[k], delta: +(metrics[k] - Number(baseline.metrics[k])).toFixed(2) };
    });
    return {
      schemaVersion: 1, mode: "profiled", enabled: enabled(),
      baseline: baseline || { mode: "normal-unprofiled", provided: false, note: "Normal mod ölçümünü DevTools Performance ile alıp setBaseline({mode:'normal-unprofiled',metrics:{...}}) ile ekleyin." },
      comparison: { ready: !!(baseline && baseline.metrics), baselineMode: baseline && baseline.mode || "normal-unprofiled", metrics: delta },
      metrics, counters: Object.assign({}, counters), navigation: navigation.slice(),
      timers: { pending: Array.from(pendingTimers.values()).map(t => ({ name: t.name, pendingMs: +(performance.now() - t.at).toFixed(2) })) },
      longTask: { supported: longTaskSupported, count: longTasks.length, entries: longTasks.slice() },
      animationFrames: { supported: typeof requestAnimationFrame === "function", thresholdMs: 50, count: slowFrames.length, entries: slowFrames.slice(), fallbackForLongTaskUnsupported: !longTaskSupported }
    };
  }
  function reset() {
    Object.keys(counters).forEach(k => delete counters[k]); Object.keys(timings).forEach(k => delete timings[k]);
    pendingTimers.clear(); navigation.length = 0; longTasks.length = 0; slowFrames.length = 0; sequence = 0;
    startedAt = performance.now(); previousFrame = 0; return true;
  }
  root.__oaProf = {
    isEnabled: enabled, refresh: activate,
    count: function (name, amount) { if (ensure()) count(name, amount); },
    time, start, end, timerStart,
    timerEnd: token => timerFinish(token, "executed"),
    timerCancel: token => timerFinish(token, "cancelled"),
    markNavigation, report, reset, setBaseline,
    clearBaseline: function () { try { localStorage.removeItem(STORAGE_KEY + "Baseline"); return true; } catch (_) { return false; } }
  };
  activate();
})();
