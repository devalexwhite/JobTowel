import { mkdir, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { watch } from 'node:fs';
import path from 'node:path';
import * as sass from 'sass';
import * as esbuild from 'esbuild';

const root = fileURLToPath(new URL('../', import.meta.url));
const output = path.join(root, 'static/assets');
const watching = process.argv.includes('--watch');
await mkdir(output, { recursive: true });

async function buildStyles() {
  const result = sass.compile(path.join(root, 'frontend/scss/app.scss'), {
    loadPaths: [path.join(root, 'node_modules')],
    style: watching ? 'expanded' : 'compressed',
    // Bootstrap 5 uses Sass APIs deprecated ahead of its next major release.
    silenceDeprecations: ['import', 'global-builtin', 'color-functions', 'if-function'],
    quietDeps: true,
  });
  await writeFile(path.join(output, 'app.css'), result.css);
  console.log('Built app.css');
}

await buildStyles();
const options = {
  entryPoints: [path.join(root, 'frontend/js/app.js')],
  bundle: true,
  inject: [path.join(root, 'frontend/js/jquery.js')],
  minify: !watching,
  sourcemap: watching,
  outfile: path.join(output, 'app.js'),
  logLevel: 'info',
};
if (watching) {
  const context = await esbuild.context(options);
  await context.watch();
  // Serialize Sass rebuilds, including edits made during a build.
  let rebuilding = Promise.resolve();
  const watcher = watch(path.join(root, 'frontend/scss'), { recursive: true }, () => {
    rebuilding = rebuilding.then(buildStyles).catch(error => console.error(error.message));
  });
  console.log('Watching frontend assets. Run cargo run in another terminal.');
  for (const signal of ['SIGINT', 'SIGTERM']) {
    process.once(signal, async () => { watcher.close(); await context.dispose(); process.exit(0); });
  }
} else {
  await esbuild.build(options);
}
