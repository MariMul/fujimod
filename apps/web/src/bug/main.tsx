import { render } from '@solidjs/web';
import { BugReport } from './Page';
const dispose = render(() => <BugReport />, document.getElementById('bug-report')!);
if (import.meta.hot) import.meta.hot.dispose(dispose);
