const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const $ = (id) => document.getElementById(id);

const state = { running: false };

function formatHashrate(hs) {
  if (hs >= 1e9) return (hs / 1e9).toFixed(2) + " GH/s";
  if (hs >= 1e6) return (hs / 1e6).toFixed(2) + " MH/s";
  if (hs >= 1e3) return (hs / 1e3).toFixed(2) + " kH/s";
  return Math.round(hs) + " H/s";
}

function seg(groupId) {
  return document.querySelector(`#${groupId} .on`).dataset.v;
}

function collectSettings() {
  return {
    address: $("address").value.trim(),
    mode: seg("mode") === "solo" ? "Solo" : "Pool",
    device: seg("device") === "gpu" ? "Gpu" : "Cpu",
    threads: parseInt($("threads").value, 10),
    gpu_backend: $("backend").value || "Auto",
    pool_endpoint: $("pool-endpoint").value.trim(),
    worker_name: $("worker").value.trim(),
    solo_rpc_url: $("rpc").value.trim(),
  };
}

function selectSeg(groupId, v) {
  document.querySelectorAll(`#${groupId} button`).forEach((b) =>
    b.classList.toggle("on", b.dataset.v === v));
  $("solo-row").classList.toggle("hidden", seg("mode") !== "solo");
  const gpu = seg("device") === "gpu";
  $("cpu-row").classList.toggle("hidden", gpu);
  $("gpu-row").classList.toggle("hidden", !gpu);
}

async function validate() {
  const input = $("address").value;
  try {
    await invoke("validate_address_cmd", { input });
    $("addr-status").textContent = "✓";
    $("addr-error").textContent = "";
    $("start").disabled = false;
  } catch (e) {
    $("addr-status").textContent = input.trim() ? "✗" : "";
    $("addr-error").textContent = input.trim() ? String(e) : "";
    $("start").disabled = true;
  }
}

async function start() {
  const s = collectSettings();
  try {
    await invoke("start_mining", { s });
    state.running = true;
    $("start").textContent = "STOP";
    $("start").classList.add("running");
    $("stats").classList.remove("hidden");
    $("addr-error").textContent = "";
  } catch (e) {
    $("addr-error").textContent = String(e);
  }
}

async function stop() {
  await invoke("stop_mining");
  state.running = false;
  $("start").textContent = "START";
  $("start").classList.remove("running");
  $("status").textContent = "stopped";
  $("conn").classList.remove("on");
  window.matrixIntensity(0.3);
}

window.addEventListener("DOMContentLoaded", async () => {
  const s = await invoke("get_settings");
  $("address").value = s.address;
  $("rpc").value = s.solo_rpc_url;
  $("threads").value = s.threads ?? 4;
  $("threads-n").textContent = $("threads").value;
  $("pool-endpoint").value = s.pool_endpoint;
  $("worker").value = s.worker_name;
  const backends = navigator.platform.startsWith("Mac")
    ? ["Auto", "Metal"]
    : ["Auto", "Cuda", "Opencl"];
  $("backend").innerHTML = backends.map((b) => `<option>${b}</option>`).join("");
  selectSeg("mode", s.mode === "Solo" ? "solo" : "pool");
  selectSeg("device", s.device === "Gpu" ? "gpu" : "cpu");
  validate();

  $("address").addEventListener("input", validate);
  $("paste").addEventListener("click", async () => {
    try {
      $("address").value = await navigator.clipboard.readText();
      validate();
    } catch (_) { /* clipboard permission denied — user can type */ }
  });
  $("threads").addEventListener("input", () => ($("threads-n").textContent = $("threads").value));
  document.querySelectorAll("#mode button").forEach((b) =>
    b.addEventListener("click", () => selectSeg("mode", b.dataset.v)));
  document.querySelectorAll("#device button").forEach((b) =>
    b.addEventListener("click", () => { if (!b.disabled) selectSeg("device", b.dataset.v); }));
  $("start").addEventListener("click", () => (state.running ? stop() : start()));

  let startedAt = null;
  await listen("miner-event", (ev) => {
    const e = ev.payload;
    if (e.Stats) {
      $("hashrate").textContent = formatHashrate(e.Stats.hashrate_hs);
      $("shares").textContent =
        `ok ${e.Stats.shares_accepted} / rej ${e.Stats.shares_rejected}` +
        (e.Stats.blocks_found ? ` / blocks ${e.Stats.blocks_found}` : "");
      if (e.Stats.last_error) $("addr-error").textContent = e.Stats.last_error;
    } else if (e.BlockLine !== undefined) {
      // Verbatim block output: hash, utreexo merkle root, nonce, height, …
      // exactly as the miner prints it.
      $("blocks").classList.remove("hidden");
      const bl = $("blocklog");
      bl.textContent = (bl.textContent + "\n" + e.BlockLine)
        .split("\n").slice(-400).join("\n");
      bl.scrollTop = bl.scrollHeight;
    } else if (e.RawLine !== undefined) {
      const log = $("rawlog");
      log.textContent = (log.textContent + "\n" + e.RawLine)
        .split("\n").slice(-200).join("\n");
      log.scrollTop = log.scrollHeight;
    } else if (e.Status) {
      $("status").textContent = e.Status;
      $("conn").classList.toggle("on", e.Status === "running");
      if (e.Status === "running") {
        startedAt = Date.now();
        window.matrixIntensity(1.0);
      }
      if (e.Status === "crash-loop") stop();
    }
  });
  setInterval(() => {
    if (state.running && startedAt)
      $("uptime").textContent = Math.floor((Date.now() - startedAt) / 1000) + "s";
  }, 1000);
});
