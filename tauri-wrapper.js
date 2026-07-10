import { spawn } from 'child_process';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// Set GStreamer environment variable for the bundler to include bad plugins
process.env.GSTREAMER_INCLUDE_BAD_PLUGINS = '1';

// Forward the command to tauri CLI
const args = process.argv.slice(2);
const tauriJsPath = path.join(__dirname, 'node_modules', '@tauri-apps', 'cli', 'tauri.js');
const child = spawn('node', [tauriJsPath, ...args], {
  stdio: 'inherit',
  env: process.env,
});

child.on('close', (code) => {
  process.exit(code || 0);
});
