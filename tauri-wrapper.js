import { spawn } from 'child_process';
import path from 'path';
import fs from 'fs';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const isLinux = process.platform === 'linux';
const binDir = path.join(__dirname, 'src-tauri', 'bin');
const lddScriptPath = path.join(binDir, 'ldd');

if (isLinux) {
  // Create temporary bin directory and write a mock 'ldd' script
  if (!fs.existsSync(binDir)) {
    fs.mkdirSync(binDir, { recursive: true });
  }

  const lddScriptContent = `#!/bin/sh
# Find the real ldd by searching PATH (excluding our own bin directory)
REAL_LDD=""
IFS=:
for dir in $PATH; do
  if [ "$dir" != "${binDir}" ] && [ -x "$dir/ldd" ]; then
    REAL_LDD="$dir/ldd"
    break
  fi
done

if [ -z "$REAL_LDD" ]; then
  REAL_LDD="/usr/bin/ldd"
fi

# Execute real ldd and filter out GStreamer libraries
"$REAL_LDD" "$@" | grep -E -v "libgstreamer-1.0.so|libgstapp-1.0.so|libgstbase-1.0.so|libgstaudio-1.0.so|libgstvideo-1.0.so"
`;

  fs.writeFileSync(lddScriptPath, lddScriptContent, { mode: 0o755 });
}

// Prepend the temporary bin directory to PATH if on Linux
const env = { ...process.env };
if (isLinux) {
  env.PATH = `${binDir}${path.delimiter}${env.PATH || ''}`;
}

// Forward the command to tauri CLI
const args = process.argv.slice(2);
const tauriJsPath = path.join(__dirname, 'node_modules', '@tauri-apps', 'cli', 'tauri.js');
const child = spawn('node', [tauriJsPath, ...args], {
  stdio: 'inherit',
  env,
});

child.on('close', (code) => {
  // Clean up the temporary ldd script
  if (isLinux) {
    try {
      if (fs.existsSync(lddScriptPath)) {
        fs.unlinkSync(lddScriptPath);
      }
      if (fs.existsSync(binDir)) {
        fs.rmdirSync(binDir);
      }
    } catch (e) {}
  }
  process.exit(code || 0);
});
