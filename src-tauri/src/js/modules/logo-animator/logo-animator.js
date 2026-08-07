// === OpenAnime Logo Animator ===
// "Süper Açılış" > "Muptezel Anime" varyantının render motoru. Statik bir
// GIF yerine tam ekran overlay üzerinde <canvas> ile GERÇEK ZAMANLI çizilir
// (bkz. super-opening.js > playLogoAnimatorIntro). Kaynak: radial-spin-twist
// preset + render referansı (WebGL shader tabanlı "twist" efekti, iki doku:
// dönen kollar + merkez nokta — bkz. textures.js) — tek dosyada birleştirildi,
// kendi klasöründe (js/modules/logo-animator/) tutulur, diğer modüllerle
// karışmaz. İsimler COMMON_INIT_SCRIPT içindeki paylaşımlı scope'ta çakışmayı
// önlemek için önekli (OPENANIME_LOGO_ / OpenAnimeLogo).

const OPENANIME_LOGO_CONFIG = {
  effectMode: "spinTwist",
  invertTwist: false,
  easing: "easeInOut",
  edgeLag: 1,
  loopDegrees: 360,
  bendMax: 1,
  amount: 1,
  speed: 3,
  loopSeconds: 2.5,
  holdSeconds: 1,
  centerOnTop: true,
};

const OPENANIME_LOGO_EFFECT_MODE_IDS = {
  spinTwist: 0,
  stackUnstack: 1,
  stackCollapse: 2,
  pulseRipple: 3,
};

const OPENANIME_LOGO_VERTEX_SHADER = `
attribute vec2 a_position;
attribute vec2 a_texCoord;
varying vec2 v_texCoord;

void main() {
  gl_Position = vec4(a_position, 0.0, 1.0);
  v_texCoord = a_texCoord;
}
`;

const OPENANIME_LOGO_FRAGMENT_SHADER = `
precision mediump float;

varying vec2 v_texCoord;

uniform sampler2D u_spinTexture;
uniform sampler2D u_centerTexture;

uniform float u_time;
uniform float u_loopDegrees;
uniform float u_edgeLag;
uniform float u_bendMax;
uniform float u_amount;
uniform float u_speed;
uniform float u_loopSeconds;
uniform float u_holdSeconds;
uniform bool u_invertTwist;
uniform bool u_autoAnimate;
uniform int u_effectMode;
uniform int u_easing;
uniform float u_progress;
uniform bool u_centerOnTop;
uniform vec4 u_bgColor;

const float PI = 3.14159265359;

void main() {
  vec2 uv = v_texCoord;
  vec2 center = vec2(0.5, 0.5);
  vec2 p = uv - center;
  float r = length(p);
  float angle = atan(p.y, p.x);

  float cycleTime = u_loopSeconds + u_holdSeconds;
  float t = 0.0;

  if (u_autoAnimate) {
    t = mod(u_time * u_speed, max(0.001, cycleTime));
  } else {
    t = u_progress * cycleTime;
  }

  float phase = 0.0;
  if (t <= u_loopSeconds && u_loopSeconds > 0.0) {
    phase = t / u_loopSeconds;
  } else {
    phase = 1.0;
  }

  float easedPhase = phase;
  if (u_easing == 1) {
    easedPhase = phase < 0.5
      ? 4.0 * phase * phase * phase
      : 1.0 - pow(-2.0 * phase + 2.0, 3.0) / 2.0;
  }

  float maxRotRad = (u_loopDegrees * PI) / 180.0;
  float currentRotRad = easedPhase * maxRotRad;
  float normR = clamp(r / 0.5, 0.0, 1.0);

  float sampledAngle = angle;
  float scaleFactor = 1.0;
  float centerScale = 1.0;

  if (u_effectMode == 0) {
    float twistFactor = u_amount * u_bendMax * pow(normR, max(0.05, u_edgeLag));
    float twistPulse = sin(easedPhase * PI);
    float totalTwist = twistFactor * twistPulse * (u_invertTwist ? -1.0 : 1.0);
    sampledAngle = angle + currentRotRad - totalTwist;

  } else if (u_effectMode == 1) {
    float fanProgress = clamp(easedPhase / 0.75, 0.0, 1.0);
    float F = sin(fanProgress * PI * 0.5);

    float aNorm = mod(angle + PI * 0.25, 2.0 * PI);
    float sectorIndex = floor(aNorm / (PI * 0.5));

    float armAngleShift = (1.0 - F) * sectorIndex * (PI * 0.5);
    float twistFactor = u_amount * (1.0 - F) * pow(normR, max(0.05, u_edgeLag));

    sampledAngle = angle + currentRotRad - armAngleShift - twistFactor * (u_invertTwist ? -1.0 : 1.0);

    float popP = clamp((easedPhase - 0.35) / 0.5, 0.0, 1.0);
    centerScale = sin(popP * PI * 0.5);

  } else if (u_effectMode == 2) {
    float collapseProgress = clamp(easedPhase / 0.75, 0.0, 1.0);
    float F = 1.0 - sin(collapseProgress * PI * 0.5);

    float aNorm = mod(angle + PI * 0.25, 2.0 * PI);
    float sectorIndex = floor(aNorm / (PI * 0.5));

    float armAngleShift = (1.0 - F) * sectorIndex * (PI * 0.5);
    float twistFactor = u_amount * (1.0 - F) * pow(normR, max(0.05, u_edgeLag));

    sampledAngle = angle + currentRotRad - armAngleShift + twistFactor * (u_invertTwist ? -1.0 : 1.0);

    centerScale = clamp(1.0 - easedPhase / 0.45, 0.0, 1.0);

  } else if (u_effectMode == 3) {
    float pulse = sin(easedPhase * PI * 2.0) * 0.18 * u_amount;
    scaleFactor = 1.0 + pulse;
    float twistFactor = u_amount * u_bendMax * sin(normR * 6.0 - easedPhase * PI * 2.0);
    sampledAngle = angle + currentRotRad - twistFactor * 0.3 * (u_invertTwist ? -1.0 : 1.0);
  }

  vec2 scaledP = p / max(0.01, scaleFactor);
  float finalR = length(scaledP);
  vec2 sampledUV = center + vec2(cos(sampledAngle), sin(sampledAngle)) * finalR;

  vec4 spinColor = vec4(0.0);
  if (sampledUV.x >= 0.0 && sampledUV.x <= 1.0 && sampledUV.y >= 0.0 && sampledUV.y <= 1.0) {
    spinColor = texture2D(u_spinTexture, sampledUV);
  }

  vec2 centerP = p / max(0.01, centerScale);
  vec2 centerUV = center + centerP;
  vec4 centerColor = vec4(0.0);
  if (centerUV.x >= 0.0 && centerUV.x <= 1.0 && centerUV.y >= 0.0 && centerUV.y <= 1.0) {
    centerColor = texture2D(u_centerTexture, centerUV);
  }

  vec4 finalColor;
  if (u_centerOnTop) {
    float outAlpha = centerColor.a + spinColor.a * (1.0 - centerColor.a);
    vec3 outRGB = centerColor.rgb * centerColor.a + spinColor.rgb * spinColor.a * (1.0 - centerColor.a);
    finalColor = vec4(outRGB, outAlpha);
  } else {
    float outAlpha = spinColor.a + centerColor.a * (1.0 - spinColor.a);
    vec3 outRGB = spinColor.rgb * spinColor.a + centerColor.rgb * centerColor.a * (1.0 - spinColor.a);
    finalColor = vec4(outRGB, outAlpha);
  }

  if (u_bgColor.a > 0.0) {
    vec3 bgPremult = u_bgColor.rgb * u_bgColor.a;
    vec3 blendedRGB = finalColor.rgb + bgPremult * (1.0 - finalColor.a);
    float blendedAlpha = finalColor.a + u_bgColor.a * (1.0 - finalColor.a);
    finalColor = vec4(blendedRGB, blendedAlpha);
  }

  gl_FragColor = finalColor;
}
`;

/**
 * Verilen <canvas> üzerinde WebGL bağlamı kurar, shader programını derler,
 * iki dokuyu (spin kolları + merkez nokta) belleğe yükler ve bir render
 * kontrolcüsü döner. Canvas/context başarısız olursa null döner — çağıran
 * taraf (super-opening.js) bu durumda animasyonu atlayıp açılışı bitirir.
 * @param {HTMLCanvasElement} canvas
 * @returns {{ ready: Promise<void>, renderFrame: (progress: number) => void } | null}
 */
function initOpenAnimeLogoAnimator(canvas) {
  const gl = canvas.getContext("webgl", {
    alpha: true,
    premultipliedAlpha: false,
    preserveDrawingBuffer: true,
  });
  if (!gl) {
    console.warn("[Logo Animator] WebGL desteklenmiyor.");
    return null;
  }

  function compileShader(type, source) {
    const shader = gl.createShader(type);
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
      console.warn("[Logo Animator] Shader derleme hatası:", gl.getShaderInfoLog(shader));
      gl.deleteShader(shader);
      return null;
    }
    return shader;
  }

  const vertexShader = compileShader(gl.VERTEX_SHADER, OPENANIME_LOGO_VERTEX_SHADER);
  const fragmentShader = compileShader(gl.FRAGMENT_SHADER, OPENANIME_LOGO_FRAGMENT_SHADER);
  if (!vertexShader || !fragmentShader) return null;

  const program = gl.createProgram();
  gl.attachShader(program, vertexShader);
  gl.attachShader(program, fragmentShader);
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    console.warn("[Logo Animator] Program linkleme hatası:", gl.getProgramInfoLog(program));
    return null;
  }

  const positionBuffer = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, positionBuffer);
  gl.bufferData(
    gl.ARRAY_BUFFER,
    new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]),
    gl.STATIC_DRAW
  );
  const positionLocation = gl.getAttribLocation(program, "a_position");

  const uvBuffer = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, uvBuffer);
  gl.bufferData(
    gl.ARRAY_BUFFER,
    new Float32Array([0, 0, 1, 0, 0, 1, 0, 1, 1, 0, 1, 1]),
    gl.STATIC_DRAW
  );
  const uvLocation = gl.getAttribLocation(program, "a_texCoord");

  function loadTexture(base64) {
    const texture = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, texture);
    // Yüklenene kadar 1x1 şeffaf geçici piksel
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, 1, 1, 0, gl.RGBA, gl.UNSIGNED_BYTE, new Uint8Array([0, 0, 0, 0]));

    const ready = new Promise((resolve) => {
      const img = new Image();
      img.onload = () => {
        gl.bindTexture(gl.TEXTURE_2D, texture);
        gl.pixelStorei(gl.UNPACK_FLIP_Y_WEBGL, true);
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, img);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
        gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
        resolve();
      };
      img.onerror = () => resolve();
      img.src = base64;
    });

    return { texture, ready };
  }

  // data: URI'ler — ağ isteği değildir, WebView2'nin Private Network Access
  // korumasına takılmaz (bkz. textures.js).
  const spin = loadTexture(OPENANIME_LOGO_SPIN_TEXTURE_B64);
  const centerTex = loadTexture(OPENANIME_LOGO_CENTER_TEXTURE_B64);

  const uniformNames = [
    "u_resolution", "u_spinTexture", "u_centerTexture", "u_time",
    "u_loopDegrees", "u_edgeLag", "u_bendMax", "u_amount", "u_speed",
    "u_loopSeconds", "u_holdSeconds", "u_invertTwist", "u_autoAnimate",
    "u_effectMode", "u_easing", "u_progress", "u_centerOnTop", "u_bgColor",
  ];
  const uniforms = {};
  uniformNames.forEach((name) => {
    uniforms[name] = gl.getUniformLocation(program, name);
  });

  /**
   * @param {number} progress - 0.0 (animasyon başı) - 1.0 (tam döngü + bekleme sonu)
   */
  function renderFrame(progress) {
    const cfg = OPENANIME_LOGO_CONFIG;

    gl.viewport(0, 0, canvas.width, canvas.height);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.useProgram(program);

    gl.enableVertexAttribArray(positionLocation);
    gl.bindBuffer(gl.ARRAY_BUFFER, positionBuffer);
    gl.vertexAttribPointer(positionLocation, 2, gl.FLOAT, false, 0, 0);

    gl.enableVertexAttribArray(uvLocation);
    gl.bindBuffer(gl.ARRAY_BUFFER, uvBuffer);
    gl.vertexAttribPointer(uvLocation, 2, gl.FLOAT, false, 0, 0);

    gl.uniform2f(uniforms.u_resolution, canvas.width, canvas.height);
    gl.uniform1f(uniforms.u_time, progress * (cfg.loopSeconds + cfg.holdSeconds));
    gl.uniform1f(uniforms.u_loopDegrees, cfg.loopDegrees);
    gl.uniform1f(uniforms.u_edgeLag, cfg.edgeLag);
    gl.uniform1f(uniforms.u_bendMax, cfg.bendMax);
    gl.uniform1f(uniforms.u_amount, cfg.amount);
    gl.uniform1f(uniforms.u_speed, cfg.speed);
    gl.uniform1f(uniforms.u_loopSeconds, cfg.loopSeconds);
    gl.uniform1f(uniforms.u_holdSeconds, cfg.holdSeconds);
    gl.uniform1i(uniforms.u_invertTwist, cfg.invertTwist ? 1 : 0);
    // Açılış tek seferlik oynadığı için otomatik döngü kapalı — ilerleme
    // dışarıdan (requestAnimationFrame ile) sürülür.
    gl.uniform1i(uniforms.u_autoAnimate, 0);
    gl.uniform1i(uniforms.u_effectMode, OPENANIME_LOGO_EFFECT_MODE_IDS[cfg.effectMode] || 0);
    gl.uniform1i(uniforms.u_easing, cfg.easing === "easeInOut" ? 1 : 0);
    gl.uniform1f(uniforms.u_progress, progress);
    gl.uniform1i(uniforms.u_centerOnTop, cfg.centerOnTop ? 1 : 0);
    gl.uniform4f(uniforms.u_bgColor, 0, 0, 0, 0);

    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, spin.texture);
    gl.uniform1i(uniforms.u_spinTexture, 0);

    gl.activeTexture(gl.TEXTURE1);
    gl.bindTexture(gl.TEXTURE_2D, centerTex.texture);
    gl.uniform1i(uniforms.u_centerTexture, 1);

    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
  }

  return {
    ready: Promise.all([spin.ready, centerTex.ready]).then(() => {}),
    renderFrame,
  };
}
