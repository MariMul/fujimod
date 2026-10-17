import { render } from '@solidjs/web';
import { DevTools } from './Page';
const dispose = render(() => <DevTools />, document.getElementById('dev-tools')!);
if (import.meta.hot) import.meta.hot.dispose(dispose);
