// Entry point. Order matters: styles, platform, motion (reads the tokens), input policy,
// error reporting - then mount. `?gallery` mounts the component gallery instead; it exists
// only in development and harness builds (`__GALLERY__` is false in the release build, so
// the dynamic import and its chunk are removed).

import './styles/tokens.css';
import './styles/base.css';
import './styles/motion.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { installInput } from './lib/input/input';
import { installErrorReporting } from './lib/ipc/api';
import { installMotion } from './lib/motion/motion';
import { applyPlatform } from './lib/platform';

applyPlatform();
installMotion();
installInput();
installErrorReporting();

const target = document.getElementById('app');
if (target === null) throw new Error('#app is missing in index.html');

if (__GALLERY__ && new URLSearchParams(location.search).has('gallery')) {
  const { default: Gallery } = await import('./features/gallery/Gallery.svelte');
  mount(Gallery, { target });
} else {
  mount(App, { target });
}
