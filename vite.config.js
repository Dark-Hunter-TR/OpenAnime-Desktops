import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [sveltekit()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
    proxy: {
      '/preview-proxy': {
        target: 'https://openani.me',
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/preview-proxy/, ''),
        selfHandleResponse: true,
        configure: (proxy, _options) => {
          proxy.on('proxyReq', (proxyReq, _req, _res) => {
            proxyReq.setHeader('User-Agent', 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36 OpenAnime/0.1.0');
            proxyReq.removeHeader('accept-encoding');
          });
          proxy.on('proxyRes', (proxyRes, _req, res) => {
            const chunks = [];
            proxyRes.on('data', (chunk) => {
              chunks.push(chunk);
            });
            proxyRes.on('end', () => {
              const body = Buffer.concat(chunks);
              const contentType = proxyRes.headers['content-type'] || '';
              
              Object.keys(proxyRes.headers).forEach((key) => {
                if (key !== 'x-frame-options' && key !== 'content-security-policy' && key !== 'cross-origin-opener-policy') {
                  res.setHeader(key, proxyRes.headers[key]);
                }
              });
              res.setHeader('access-control-allow-origin', '*');
              
              if (contentType.includes('text/html')) {
                let html = body.toString('utf8');
                
                // Remove existing base tags if any
                html = html.replace(/<base href="[^"]*">/gi, '');
                
                // Inject base tag pointing to our proxy path
                if (html.includes('<head>')) {
                  html = html.replace('<head>', '<head><base href="/preview-proxy/">');
                } else if (html.includes('<HEAD>')) {
                  html = html.replace('<HEAD>', '<HEAD><base href="/preview-proxy/">');
                }
                
                // Rewrite absolute links starting with / to go through proxy
                html = html.replace(/(href|src|action|data-src)=["']\/((?!preview-proxy)[^"'].*?)["']/g, '$1="/preview-proxy/$2"');
                
                res.end(html);
              } else {
                res.end(body);
              }
            });
          });
        }
      }
    }
  },
  // Aggressive tree shaking, minification, and chunk size optimization for minimal bundle footprint
  esbuild: {
    drop: ["console", "debugger"],
    minifyIdentifiers: true,
    minifySyntax: true,
    minifyWhitespace: true,
    legalComments: "none",
    treeShaking: true,
  },
  build: {
    target: "esnext",
    minify: "esbuild",
    cssCodeSplit: false,
    reportCompressedSize: false,
    chunkSizeWarningLimit: 100,
  },
}));
