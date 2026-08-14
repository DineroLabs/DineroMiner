(() => {
  const canvas = document.getElementById("matrix");
  const ctx = canvas.getContext("2d");
  const CHARS = "0123456789abcdef";
  const FONT = 12, FPS = 14;
  let cols = [], intensity = 0.3, timer = null;

  function resize() {
    canvas.width = innerWidth;
    canvas.height = innerHeight;
    const n = Math.floor(innerWidth / FONT);
    cols = Array.from({ length: n }, () => Math.floor((Math.random() * innerHeight) / FONT));
  }
  function frame() {
    ctx.fillStyle = "rgba(10,10,10,0.12)"; // trails fade into bg
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    ctx.font = FONT + "px monospace";
    for (let i = 0; i < cols.length; i++) {
      if (Math.random() > intensity * 0.75) continue; // intensity thins the rain
      const ch = CHARS[Math.floor(Math.random() * CHARS.length)];
      const head = Math.random() < 0.04;
      ctx.fillStyle = head ? "rgba(230,230,225,0.8)" : "rgba(120,120,115,0.28)";
      ctx.fillText(ch, i * FONT, cols[i] * FONT);
      cols[i] = cols[i] * FONT > canvas.height && Math.random() > 0.975 ? 0 : cols[i] + 1;
    }
  }
  function run() {
    clearInterval(timer);
    timer = setInterval(frame, 1000 / FPS);
  }
  document.addEventListener("visibilitychange", () =>
    document.hidden ? clearInterval(timer) : run());
  addEventListener("resize", resize);
  window.matrixIntensity = (v) => { intensity = v; };
  resize();
  run();
})();
