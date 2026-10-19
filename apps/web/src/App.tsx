import { lazy, Loading } from 'solid-js';
import { getRequestEvent } from '@solidjs/web';
const Home = lazy(() => import('./home/Page'));
const DevTools = lazy(() => import('./dev-tools/Page'));
const BugReport = lazy(() => import('./bug/Page'));
export default function App() {
  const path = new URL(getRequestEvent()?.request.url ?? window.location.href).pathname;
  const Page =
    path === '/dev-tools/' || path === '/dev-tools'
      ? DevTools
      : path === '/bug/' || path === '/bug'
        ? BugReport
        : Home;
  return (
    <Loading>
      <Page />
    </Loading>
  );
}
